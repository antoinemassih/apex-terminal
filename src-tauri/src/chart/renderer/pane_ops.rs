//! Pane layout OPERATIONS (WS-E E2 extraction from gpu.rs).
//! (Named pane_ops to avoid colliding with the existing `pane_layout` module
//! that defines the PaneLayout TYPE.)
//!
//! PaneLayout undo/redo, stable pane-id allocation/sync, chart push/remove/
//! truncate, and the per-frame `compute_pane_rects_for_frame` layout solve.
//! Extracted verbatim to shrink gpu.rs; re-exported there so every
//! `gpu::compute_pane_rects_for_frame()` / `gpu::alloc_pane_id()` / ... caller
//! (and gpu.rs's own bare call) is unchanged. This is a pure move — zero
//! runtime cost (same code, cross-module calls inline identically).

use super::gpu::{Chart, Watchlist, Layout};

// ─── P17 #3 — PaneLayout undo/redo helpers ────────────────────────────────
const PANE_LAYOUT_UNDO_CAP: usize = 32;

/// Snapshot the current pane_layout onto the undo stack BEFORE applying a
/// destructive operation (split, close, template change). The redo stack is
/// cleared because the user's just diverged from the previous history. Cap
/// the undo stack at PANE_LAYOUT_UNDO_CAP entries (oldest evicted FIFO).
pub(crate) fn pane_layout_record_undo(wl: &mut Watchlist) {
    let snap = wl.pane_layout.clone();
    wl.pane_layout_undo.push(snap);
    if wl.pane_layout_undo.len() > PANE_LAYOUT_UNDO_CAP {
        let drop_n = wl.pane_layout_undo.len() - PANE_LAYOUT_UNDO_CAP;
        wl.pane_layout_undo.drain(..drop_n);
    }
    wl.pane_layout_redo.clear();
}

/// Pop the most recent undo snapshot; push the current state onto redo.
/// Returns `true` if anything was undone.
///
/// Entries that reference pane IDs no longer in `pane_ids` (i.e. charts that
/// were closed) are silently discarded: we have no chart data to restore, so
/// applying them would produce a layout leaf that dead-slot cleanup would
/// immediately remove — appearing to the user as "undo did nothing". Skipping
/// those entries is the least-surprising safe behavior.
pub(crate) fn pane_layout_undo(wl: &mut Watchlist) -> bool {
    let live_ids: std::collections::HashSet<u64> = wl.pane_ids.iter().copied().collect();
    while wl.pane_layout_undo.last().map_or(false, |snap| {
        snap.as_ref().map_or(false, |pl| pl.iter_slots().any(|(_, s)| !live_ids.contains(&s)))
    }) {
        wl.pane_layout_undo.pop();
    }
    if let Some(prev) = wl.pane_layout_undo.pop() {
        let current = wl.pane_layout.clone();
        wl.pane_layout_redo.push(current);
        wl.pane_layout = prev;
        true
    } else {
        false
    }
}

/// Pop the most recent redo snapshot; push the current state onto undo.
/// Dead-entry skipping mirrors `pane_layout_undo` for the same reason.
pub(crate) fn pane_layout_redo(wl: &mut Watchlist) -> bool {
    let live_ids: std::collections::HashSet<u64> = wl.pane_ids.iter().copied().collect();
    while wl.pane_layout_redo.last().map_or(false, |snap| {
        snap.as_ref().map_or(false, |pl| pl.iter_slots().any(|(_, s)| !live_ids.contains(&s)))
    }) {
        wl.pane_layout_redo.pop();
    }
    if let Some(next) = wl.pane_layout_redo.pop() {
        let current = wl.pane_layout.clone();
        wl.pane_layout_undo.push(current);
        wl.pane_layout = next;
        true
    } else {
        false
    }
}

// ─── P17 #1 — stable pane-ID helpers ───────────────────────────────────────
//
// `Watchlist::pane_ids` runs in lockstep with `panes: Vec<Chart>`. Each entry
// is a fresh u64 minted from `next_pane_id`. PaneLayout's PaneSlot now carries
// these IDs (not raw indices), so close/split mutations never invalidate other
// panes' slot references. The chart_idx_for() helper resolves an ID to its
// current position in `panes` for the per-frame render loop.

/// Ensure `pane_ids` has exactly one fresh id per chart in `panes`. Called
/// lazily at the top of the frame so workspaces loaded before P17 (no
/// pane_ids field on disk) get migrated transparently; subsequent splits /
/// closes maintain the invariant manually via push_chart / remove_chart_at.
pub(crate) fn ensure_pane_ids_synced(wl: &mut Watchlist, n: usize) {
    while wl.pane_ids.len() < n {
        let id = wl.next_pane_id;
        wl.next_pane_id += 1;
        wl.pane_ids.push(id);
    }
    if wl.pane_ids.len() > n {
        wl.pane_ids.truncate(n);
    }
}

/// Look up the current Vec<Chart> index for a stable pane id. Returns None
/// if the id is dead (chart was closed / not yet materialized).
#[inline]
pub(crate) fn chart_idx_for(wl: &Watchlist, id: u64) -> Option<usize> {
    wl.pane_ids.iter().position(|&x| x == id)
}

/// Allocate a fresh id for a new chart. Caller is responsible for pushing the
/// Chart into `panes` and the id into `wl.pane_ids` at matching positions.
#[inline]
pub(crate) fn alloc_pane_id(wl: &mut Watchlist) -> u64 {
    let id = wl.next_pane_id;
    wl.next_pane_id += 1;
    id
}

/// Push a new chart + matching id in one step. Returns the new id.
pub(crate) fn push_chart_with_id(panes: &mut Vec<Chart>, wl: &mut Watchlist, chart: Chart) -> u64 {
    panes.push(chart);
    let id = alloc_pane_id(wl);
    wl.pane_ids.push(id);
    id
}

/// Remove `panes[idx]` and the matching pane_ids entry. Returns the
/// removed id so callers can update PaneLayout to drop the matching leaf.
pub(crate) fn remove_chart_at(panes: &mut Vec<Chart>, wl: &mut Watchlist, idx: usize) -> Option<u64> {
    if idx >= panes.len() { return None; }
    panes.remove(idx);
    if idx < wl.pane_ids.len() {
        Some(wl.pane_ids.remove(idx))
    } else {
        None
    }
}

/// Truncate panes + pane_ids to `len` together. For template-switch orphan
/// removal. Returns the dropped ids (so callers can drop matching leaves).
pub(crate) fn truncate_charts(panes: &mut Vec<Chart>, wl: &mut Watchlist, len: usize) -> Vec<u64> {
    if panes.len() <= len { return Vec::new(); }
    let dropped: Vec<u64> = wl.pane_ids.get(len..).map(|s| s.to_vec()).unwrap_or_default();
    panes.truncate(len);
    wl.pane_ids.truncate(len);
    dropped
}

// ─── PaneLayout integration helpers (Phase 1 — PaneGrid migration) ──────────
//
// The chart-app's render loop calls into these to read pane rects + drive
// dragger overlays from `Watchlist::pane_layout` when present, falling back
// to the legacy `Layout` template + 8-fraction path when not. Keeping the
// branching here means core.rs only needs surgical call-site changes.

/// Lazy materialization: if `pane_layout` is `None`, build it from the
/// current legacy `Layout` template + the current pane_ids list. Called
/// once per frame from core.rs; cheap no-op when pane_layout is already
/// populated. Old workspaces that never had PaneLayout get upgraded
/// silently the first time this fires. Assumes ensure_pane_ids_synced
/// has already run (so pane_ids.len() == pane_count).
pub(crate) fn ensure_pane_layout(wl: &mut Watchlist, legacy: Layout, pane_count: usize) {
    if wl.pane_layout.is_some() { return; }
    // Collect stable IDs to use as PaneSlots, padding with sentinel 0 if
    // the template asks for more leaves than charts exist (caller's
    // template should match pane_count to avoid the padding path).
    let mut slot_ids: Vec<u64> = wl.pane_ids.iter().copied().take(pane_count.max(1)).collect();
    while slot_ids.len() < pane_count.max(1) { slot_ids.push(0); }
    wl.pane_layout = Some(
        crate::chart_renderer::pane_layout::PaneLayout::from_template(legacy, &slot_ids)
    );
}

/// Compute pane rects for the current frame, indexed by chart_idx so the
/// existing `pane_rects[i] = rect for chart i` contract is preserved.
///
/// - When `pane_layout` is set: walks the tree depth-first, then scatters
///   each leaf's rect into the output at the position `chart_idx` points at.
///   This means panes may render in any tree order while the lookup table
///   stays index-aligned with the chart-storage Vec.
/// - When `pane_layout` is unset: legacy `Layout::pane_rects` path (one
///   rect per chart, ordered 0..visible_count).
pub(crate) fn compute_pane_rects_for_frame(
    wl: &Watchlist,
    legacy: Layout,
    full_rect: egui::Rect,
    visible_count: usize,
    gap: f32,
) -> Vec<egui::Rect> {
    if let Some(ref pl) = wl.pane_layout {
        // P17 #1 — slots are stable IDs. Translate via pane_ids → current
        // chart_idx, then scatter rects so pane_rects[i] is the rect for
        // chart i (preserves legacy contract for downstream consumers).
        let cap = visible_count.max(pl.pane_count()).max(wl.pane_ids.len());
        let mut out = vec![full_rect; cap];
        for (pane_id, rect) in pl.pane_rects(full_rect, gap) {
            if let Some(idx) = chart_idx_for(wl, pane_id) {
                if idx < out.len() {
                    out[idx] = rect;
                }
            }
        }
        out
    } else {
        legacy.pane_rects(
            full_rect, visible_count,
            wl.pane_split.h, wl.pane_split.v, wl.pane_split.h2, wl.pane_split.v2,
            wl.pane_split.v3, wl.pane_split.v4, wl.pane_split.v5, wl.pane_split.v6,
        )
    }
}
