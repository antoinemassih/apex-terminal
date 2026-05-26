//! `PaneLayout` — chart-app adapter over `ui_kit::widgets::pane_grid::PaneState`.
//!
//! Replaces the legacy `Layout` enum + 8 split-fraction floats with a true
//! recursive split tree. Leaf content is just `usize` (an index into the
//! chart-app's existing `Vec<Chart>` storage) so chart data ownership and
//! mutation patterns are unchanged — only the topology + dividers + rect
//! computation move under PaneState.
//!
//! ## Why an adapter (not direct PaneGrid usage)
//!
//! `PaneGrid::show` takes a render closure that receives `&mut T`. The chart
//! pane render path needs `&mut Vec<Chart>` (for linked-pane lookups, layout
//! context, and the global pane index) which can't coexist with `PaneGrid`'s
//! mutable borrow of `PaneState<usize>`. So we:
//!
//!  1. Use `PaneState<usize>` purely for topology.
//!  2. Expose `pane_rects(rect) -> Vec<(usize, Rect)>` so `core.rs` can keep
//!     its existing iteration pattern: index into `Vec<Chart>` and call
//!     `render_chart_pane` per pane with its rect.
//!  3. Render divider drag-handles + right-click split menu via
//!     `show_overlays(ui)` after the panes paint.
//!
//! ## Preset templates
//!
//! `Layout`'s 19 named templates (`One`, `Two`, `TwoH`, `Three`, `ThreeL`…)
//! become preset initializers via [`PaneLayout::from_template`]. One-click
//! selection from the toolbar still works; power users right-click any
//! pane and pick "Split Horizontal" / "Split Vertical" / "Close" to build
//! arbitrary trees.

use egui::{CursorIcon, Pos2, Rect, Sense, Stroke, Ui};
use serde::{Deserialize, Serialize};

use crate::chart_renderer::gpu::Layout;
use crate::ui_kit::widgets::pane_grid::{Axis, Node, PaneId, PaneState, SplitId, split_rect};
use crate::ui_kit::widgets::theme::ComponentTheme;

/// Leaf content — index into the chart-app's `Vec<Chart>` storage.
pub type PaneSlot = usize;

/// Recursive split-pane layout for the chart area. Owns the topology +
/// split ratios; chart data lives elsewhere indexed by `PaneSlot`.
#[derive(Debug, Serialize, Deserialize)]
pub struct PaneLayout {
    state: PaneState<PaneSlot>,
}

// `Clone` and `PartialEq` are required by the LayoutState aggregate
// (Watchlist is a Clone+PartialEq snapshot for the persist store). Both
// PaneState<T> and Node<T> implement neither by derive — so we round-trip
// via serde to clone, and compare via serde-serialized equality.
impl Clone for PaneLayout {
    fn clone(&self) -> Self {
        // serde round-trip is fine here — PaneLayout clones only happen on
        // persist sync (off the render hot path).
        let json = serde_json::to_string(&self.state).expect("serialize PaneState");
        let state: PaneState<PaneSlot> = serde_json::from_str(&json).expect("deserialize PaneState");
        Self { state }
    }
}

impl PartialEq for PaneLayout {
    fn eq(&self, other: &Self) -> bool {
        let a = serde_json::to_string(&self.state).ok();
        let b = serde_json::to_string(&other.state).ok();
        a.is_some() && a == b
    }
}

impl Default for PaneLayout {
    fn default() -> Self {
        let (state, _root) = PaneState::new(0_usize);
        Self { state }
    }
}

impl PaneLayout {
    /// Single pane pointing at `chart_idx`.
    pub fn single(chart_idx: PaneSlot) -> Self {
        let (state, _root) = PaneState::new(chart_idx);
        Self { state }
    }

    /// Build a layout from a legacy `Layout` template. `chart_indices` supplies
    /// the chart-storage indices to populate each leaf (in template order).
    /// If `chart_indices` is shorter than the template's pane count, the
    /// missing slots default to `0`.
    pub fn from_template(template: Layout, chart_indices: &[PaneSlot]) -> Self {
        let n = template.max_panes();
        let slots: Vec<PaneSlot> = (0..n).map(|i| chart_indices.get(i).copied().unwrap_or(0)).collect();
        let root = build_template_tree(template, &slots);
        let mut layout = Self::default();
        layout.state.replace_root(root);
        layout
    }

    /// Replace topology with a fresh template (e.g. toolbar dropdown click).
    /// Preserves the existing chart-indices order.
    pub fn replace_with_template(&mut self, template: Layout, chart_indices: &[PaneSlot]) {
        let n = template.max_panes();
        let slots: Vec<PaneSlot> = (0..n).map(|i| chart_indices.get(i).copied().unwrap_or(0)).collect();
        let root = build_template_tree(template, &slots);
        self.state.replace_root(root);
    }

    /// Compute the rect for each visible pane, ordered by depth-first walk.
    /// `gap` is the gutter between sibling panes (carved from the split line).
    pub fn pane_rects(&self, full_rect: Rect, gap: f32) -> Vec<(PaneSlot, Rect)> {
        self.state.iter_panes_with_rects(full_rect, gap)
            .into_iter()
            .map(|(_pane_id, slot, rect)| (*slot, rect))
            .collect()
    }

    /// Number of visible panes.
    pub fn pane_count(&self) -> usize { self.state.pane_count() }

    /// Mutable iterator over (PaneId, &mut chart_idx) so callers can re-map
    /// slot indices after the chart storage Vec is mutated (e.g. close).
    pub fn iter_slots_mut(&mut self) -> impl Iterator<Item = (PaneId, &mut PaneSlot)> {
        self.state.iter_panes_mut()
    }

    /// Iterator over (PaneId, chart_idx) — read-only walk.
    pub fn iter_slots(&self) -> impl Iterator<Item = (PaneId, PaneSlot)> + '_ {
        self.state.iter_panes().map(|(id, slot)| (id, *slot))
    }

    /// Split the pane at `slot` (depth-first index into the visible panes).
    /// `new_chart_idx` is the chart-storage index for the new leaf.
    /// Returns the new pane's slot (== `new_chart_idx`) on success.
    pub fn split_pane(&mut self, target_slot: PaneSlot, axis: Axis, new_chart_idx: PaneSlot) -> Option<PaneId> {
        // Find the PaneId whose slot == target_slot
        let pane_id = self.state.iter_panes()
            .find(|(_id, s)| **s == target_slot)
            .map(|(id, _)| id)?;
        self.state.split(pane_id, axis, new_chart_idx)
    }

    /// Close the pane referencing `slot`. Returns the removed slot value
    /// (so callers know which chart to drop from storage).
    pub fn close_pane(&mut self, target_slot: PaneSlot) -> Option<PaneSlot> {
        let pane_id = self.state.iter_panes()
            .find(|(_id, s)| **s == target_slot)
            .map(|(id, _)| id)?;
        self.state.close(pane_id)
    }

    /// Render interactive divider drag-handles + right-click context menu
    /// overlays on top of the pane area. Call this AFTER the panes paint
    /// so the overlays sit above pane content.
    ///
    /// `pane_areas` is the same `Vec<(PaneSlot, Rect)>` returned by
    /// `pane_rects()` for the current frame. Returns `true` if topology
    /// changed (so the host can persist the new layout to disk).
    pub fn show_overlays(
        &mut self,
        ui: &mut Ui,
        full_rect: Rect,
        gap: f32,
        theme: &dyn ComponentTheme,
    ) -> bool {
        let mut changed = false;

        let mut drag_targets: Vec<(SplitId, Rect, Axis, f32, Rect)> = Vec::new();
        collect_split_handles(&self.state, full_rect, gap, &mut drag_targets);

        let hit_band = 8.0_f32;
        let hl_band  = 3.0_f32;
        let stroke_w = crate::ui_kit::style::stroke_std();
        let border_col = theme.border();

        for (sid, mid_rect, axis, ratio, parent_rect) in drag_targets {
            let (drag_rect, hl_rect, divider_line) = match axis {
                Axis::Horizontal => {
                    let cx = mid_rect.center().x;
                    (
                        Rect::from_min_max(
                            Pos2::new(cx - hit_band, mid_rect.top()),
                            Pos2::new(cx + hit_band, mid_rect.bottom()),
                        ),
                        Rect::from_min_max(
                            Pos2::new(cx - hl_band, mid_rect.top()),
                            Pos2::new(cx + hl_band, mid_rect.bottom()),
                        ),
                        // Sub-pixel-perfect vertical divider line at the split.
                        [Pos2::new(cx, mid_rect.top()), Pos2::new(cx, mid_rect.bottom())],
                    )
                }
                Axis::Vertical => {
                    let cy = mid_rect.center().y;
                    (
                        Rect::from_min_max(
                            Pos2::new(mid_rect.left(),  cy - hit_band),
                            Pos2::new(mid_rect.right(), cy + hit_band),
                        ),
                        Rect::from_min_max(
                            Pos2::new(mid_rect.left(),  cy - hl_band),
                            Pos2::new(mid_rect.right(), cy + hl_band),
                        ),
                        // Horizontal divider line.
                        [Pos2::new(mid_rect.left(), cy), Pos2::new(mid_rect.right(), cy)],
                    )
                }
            };

            // ── Permanent divider line (design-system token) ──────────────
            // Painted every frame using stroke_std() + theme.border() so the
            // separation between panes is always visible, not just on hover.
            ui.painter().line_segment(divider_line, Stroke::new(stroke_w, border_col));

            let resp = ui.interact(
                drag_rect,
                egui::Id::new(("pane_layout_split", sid.0)),
                Sense::click_and_drag(),
            );

            // ── Interactive feedback (over the permanent line) ───────────
            // Hover = faint accent tint; active drag = stronger accent.
            if resp.hovered() || resp.dragged() {
                ui.ctx().set_cursor_icon(match axis {
                    Axis::Horizontal => CursorIcon::ResizeHorizontal,
                    Axis::Vertical   => CursorIcon::ResizeVertical,
                });
                let alpha: u8 = if resp.dragged() { 180 } else { 90 };
                let col = theme.accent();
                ui.painter().rect_filled(
                    hl_rect,
                    egui::CornerRadius::ZERO,
                    egui::Color32::from_rgba_unmultiplied(col.r(), col.g(), col.b(), alpha),
                );
            }

            if resp.dragged() {
                if let Some(pos) = ui.ctx().input(|i| i.pointer.interact_pos()) {
                    let new_ratio = match axis {
                        Axis::Horizontal => {
                            let w = (parent_rect.width() - gap).max(1.0);
                            ((pos.x - parent_rect.min.x) / w).clamp(0.1, 0.9)
                        }
                        Axis::Vertical => {
                            let h = (parent_rect.height() - gap).max(1.0);
                            ((pos.y - parent_rect.min.y) / h).clamp(0.1, 0.9)
                        }
                    };
                    if (new_ratio - ratio).abs() > 1e-3 {
                        self.state.resize(sid, new_ratio);
                        changed = true;
                    }
                }
            }
        }

        let _ = Stroke::NONE; // suppress unused-import warning in some builds
        changed
    }
}

// ─── Template → tree builders ────────────────────────────────────────────────

fn build_template_tree(template: Layout, slots: &[PaneSlot]) -> Node<PaneSlot> {
    use Axis::*;
    // Helper to build a leaf with a sequential PaneId so the IDs are stable.
    let leaf = |i: usize| -> Node<PaneSlot> {
        Node::leaf(PaneId(i as u32), slots.get(i).copied().unwrap_or(0))
    };
    let split = |id: u32, axis: Axis, ratio: f32, a: Node<PaneSlot>, b: Node<PaneSlot>| -> Node<PaneSlot> {
        Node::split_node(SplitId(id), axis, ratio, a, b)
    };

    match template {
        // 1 pane
        Layout::One => leaf(0),

        // 2 panes
        Layout::Two  => split(0, Horizontal, 0.5, leaf(0), leaf(1)),
        Layout::TwoH => split(0, Vertical,   0.5, leaf(0), leaf(1)),

        // 3 panes
        Layout::Three  => split(0, Vertical, 0.6, leaf(0), split(1, Horizontal, 0.5, leaf(1), leaf(2))),
        Layout::ThreeL => split(0, Horizontal, 0.5, leaf(0), split(1, Vertical, 0.5, leaf(1), leaf(2))),
        Layout::ThreeR => split(0, Horizontal, 0.5, split(1, Vertical, 0.5, leaf(0), leaf(1)), leaf(2)),

        // 4 panes
        Layout::Four  => split(0, Vertical, 0.5,
            split(1, Horizontal, 0.5, leaf(0), leaf(1)),
            split(2, Horizontal, 0.5, leaf(2), leaf(3)),
        ),
        Layout::FourL => split(0, Horizontal, 0.5, leaf(0),
            split(1, Vertical, 0.333, leaf(1), split(2, Vertical, 0.5, leaf(2), leaf(3))),
        ),

        // 5 panes
        Layout::FiveC => split(0, Horizontal, 0.25,
            split(1, Vertical, 0.5, leaf(0), leaf(1)),
            split(2, Horizontal, 0.667,
                leaf(2),
                split(3, Vertical, 0.5, leaf(3), leaf(4)),
            ),
        ),
        Layout::FiveL => split(0, Horizontal, 0.4,
            split(1, Vertical, 0.5, leaf(0), leaf(1)),
            split(2, Vertical, 0.333,
                leaf(2),
                split(3, Vertical, 0.5, leaf(3), leaf(4)),
            ),
        ),
        Layout::FiveW => split(0, Vertical, 0.5,
            leaf(0),
            split(1, Vertical, 0.5,
                split(2, Horizontal, 0.5, leaf(1), leaf(2)),
                split(3, Horizontal, 0.5, leaf(3), leaf(4)),
            ),
        ),
        Layout::FiveR => split(0, Vertical, 0.333,
            split(1, Horizontal, 0.5, leaf(0), leaf(1)),
            split(2, Vertical, 0.5,
                leaf(2),
                split(3, Horizontal, 0.5, leaf(3), leaf(4)),
            ),
        ),

        // 6 panes
        Layout::Six => split(0, Vertical, 0.5,
            split(1, Horizontal, 0.333, leaf(0), split(2, Horizontal, 0.5, leaf(1), leaf(2))),
            split(3, Horizontal, 0.333, leaf(3), split(4, Horizontal, 0.5, leaf(4), leaf(5))),
        ),
        Layout::SixH => split(0, Vertical, 0.5,
            split(1, Horizontal, 0.333, leaf(0), split(2, Horizontal, 0.5, leaf(1), leaf(2))),
            split(3, Horizontal, 0.333, leaf(3), split(4, Horizontal, 0.5, leaf(4), leaf(5))),
        ),
        Layout::SixL => split(0, Horizontal, 0.4,
            split(1, Vertical, 0.5, leaf(0), leaf(1)),
            split(2, Vertical, 0.25,
                leaf(2),
                split(3, Vertical, 0.333,
                    leaf(3),
                    split(4, Vertical, 0.5, leaf(4), leaf(5)),
                ),
            ),
        ),

        // 7-9 panes
        Layout::Seven => split(0, Vertical, 0.5,
            leaf(0),
            split(1, Vertical, 0.5,
                split(2, Horizontal, 0.333, leaf(1), split(3, Horizontal, 0.5, leaf(2), leaf(3))),
                split(4, Horizontal, 0.333, leaf(4), split(5, Horizontal, 0.5, leaf(5), leaf(6))),
            ),
        ),
        Layout::EightH => split(0, Horizontal, 0.5,
            split(1, Vertical, 0.25,
                leaf(0),
                split(2, Vertical, 0.333, leaf(1), split(3, Vertical, 0.5, leaf(2), leaf(3))),
            ),
            split(4, Vertical, 0.25,
                leaf(4),
                split(5, Vertical, 0.333, leaf(5), split(6, Vertical, 0.5, leaf(6), leaf(7))),
            ),
        ),
        Layout::Nine => split(0, Vertical, 0.333,
            split(1, Horizontal, 0.333, leaf(0), split(2, Horizontal, 0.5, leaf(1), leaf(2))),
            split(3, Vertical, 0.5,
                split(4, Horizontal, 0.333, leaf(3), split(5, Horizontal, 0.5, leaf(4), leaf(5))),
                split(6, Horizontal, 0.333, leaf(6), split(7, Horizontal, 0.5, leaf(7), leaf(8))),
            ),
        ),
    }
}

// ─── Split-handle collector ──────────────────────────────────────────────────

/// Collect drag-handle hit-band rects for every interior Split node.
/// Yields (SplitId, midline_rect, axis, ratio, parent_rect).
fn collect_split_handles<T>(
    state: &PaneState<T>,
    full_rect: Rect,
    gap: f32,
    out: &mut Vec<(SplitId, Rect, Axis, f32, Rect)>,
) {
    for (sid, axis, ratio, parent_rect) in state.iter_splits_with_rects(full_rect, gap) {
        let (ra, rb) = split_rect(parent_rect, axis, ratio, gap);
        let mid = match axis {
            Axis::Horizontal => Rect::from_min_max(
                Pos2::new(ra.right(), parent_rect.top()),
                Pos2::new(rb.left(),  parent_rect.bottom()),
            ),
            Axis::Vertical => Rect::from_min_max(
                Pos2::new(parent_rect.left(),  ra.bottom()),
                Pos2::new(parent_rect.right(), rb.top()),
            ),
        };
        out.push((sid, mid, axis, ratio, parent_rect));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_pane_default() {
        let l = PaneLayout::single(0);
        assert_eq!(l.pane_count(), 1);
        let rects = l.pane_rects(Rect::from_min_size(Pos2::ZERO, egui::vec2(100.0, 100.0)), 0.0);
        assert_eq!(rects.len(), 1);
        assert_eq!(rects[0].0, 0);
    }

    #[test]
    fn template_one_is_single_pane() {
        let l = PaneLayout::from_template(Layout::One, &[0]);
        assert_eq!(l.pane_count(), 1);
    }

    #[test]
    fn template_two_horizontal_split() {
        let l = PaneLayout::from_template(Layout::Two, &[0, 1]);
        assert_eq!(l.pane_count(), 2);
        let r = l.pane_rects(Rect::from_min_size(Pos2::ZERO, egui::vec2(100.0, 100.0)), 0.0);
        assert_eq!(r.len(), 2);
        // First pane left half, second pane right half
        assert!(r[0].1.right() <= r[1].1.left() + 1.0);
    }

    #[test]
    fn template_nine_has_nine_panes() {
        let l = PaneLayout::from_template(Layout::Nine, &(0..9).collect::<Vec<_>>());
        assert_eq!(l.pane_count(), 9);
    }

    #[test]
    fn split_pane_via_id() {
        let mut l = PaneLayout::single(0);
        let new_id = l.split_pane(0, Axis::Horizontal, 1);
        assert!(new_id.is_some());
        assert_eq!(l.pane_count(), 2);
    }

    #[test]
    fn close_returns_slot() {
        let mut l = PaneLayout::from_template(Layout::Two, &[0, 1]);
        let closed = l.close_pane(1);
        assert_eq!(closed, Some(1));
        assert_eq!(l.pane_count(), 1);
    }

    #[test]
    fn close_last_pane_is_noop() {
        let mut l = PaneLayout::single(0);
        let closed = l.close_pane(0);
        assert_eq!(closed, None);
        assert_eq!(l.pane_count(), 1);
    }

    // ── P16 fix #6: multi-step sequences ───────────────────────────────────

    #[test]
    fn split_then_close_then_split_keeps_tree_consistent() {
        let mut l = PaneLayout::single(0);
        assert!(l.split_pane(0, Axis::Horizontal, 1).is_some());
        assert_eq!(l.pane_count(), 2);
        // close the newly added pane
        assert_eq!(l.close_pane(1), Some(1));
        assert_eq!(l.pane_count(), 1);
        // split again — should work without panicking and produce a fresh leaf
        assert!(l.split_pane(0, Axis::Vertical, 2).is_some());
        assert_eq!(l.pane_count(), 2);
        // slots reachable via iteration are exactly {0, 2}
        let slots: std::collections::HashSet<usize> = l.iter_slots().map(|(_, s)| s).collect();
        assert_eq!(slots, [0, 2].into_iter().collect());
    }

    #[test]
    fn template_replace_resets_tree_without_leaking_ids() {
        let mut l = PaneLayout::from_template(Layout::Two, &[0, 1]);
        assert_eq!(l.pane_count(), 2);
        l.replace_with_template(Layout::Four, &[0, 1, 2, 3]);
        assert_eq!(l.pane_count(), 4);
        let slots: std::collections::HashSet<usize> = l.iter_slots().map(|(_, s)| s).collect();
        assert_eq!(slots, [0, 1, 2, 3].into_iter().collect());
    }

    #[test]
    fn split_assigns_new_unique_pane_id() {
        let mut l = PaneLayout::single(0);
        let id1 = l.split_pane(0, Axis::Horizontal, 1).expect("first split");
        let id2 = l.split_pane(0, Axis::Vertical,   2).expect("second split");
        assert_ne!(id1, id2);
        assert_eq!(l.pane_count(), 3);
    }

    #[test]
    fn pane_rects_after_split_cover_full_area() {
        let mut l = PaneLayout::from_template(Layout::One, &[0]);
        l.split_pane(0, Axis::Horizontal, 1);
        let area = Rect::from_min_size(Pos2::ZERO, egui::vec2(800.0, 400.0));
        let rects = l.pane_rects(area, 0.0);
        assert_eq!(rects.len(), 2);
        let total_w: f32 = rects.iter().map(|(_, r)| r.width()).sum();
        // Both children should split full width (within 1px tolerance).
        assert!((total_w - area.width()).abs() < 1.0,
                "rects don't span full width: total {} vs area {}", total_w, area.width());
    }

    #[test]
    fn close_pane_serde_round_trip() {
        let mut l = PaneLayout::from_template(Layout::Three, &[0, 1, 2]);
        let closed = l.close_pane(1);
        assert_eq!(closed, Some(1));
        // Serialize → deserialize via the Clone impl (which round-trips JSON).
        let cloned = l.clone();
        assert_eq!(cloned.pane_count(), 2);
        let cloned_slots: Vec<usize> = cloned.iter_slots().map(|(_, s)| s).collect();
        let orig_slots:   Vec<usize> = l.iter_slots().map(|(_, s)| s).collect();
        assert_eq!(cloned_slots, orig_slots);
    }

    #[test]
    fn template_switch_after_splits_works() {
        // Start at Two, split each leaf to make 4, then switch to "Two" again.
        let mut l = PaneLayout::from_template(Layout::Two, &[0, 1]);
        l.split_pane(0, Axis::Vertical, 2);
        l.split_pane(1, Axis::Vertical, 3);
        assert_eq!(l.pane_count(), 4);
        l.replace_with_template(Layout::Two, &[0, 1]);
        assert_eq!(l.pane_count(), 2);
    }

    #[test]
    fn partial_eq_via_serde() {
        let a = PaneLayout::from_template(Layout::Four, &[0, 1, 2, 3]);
        let b = PaneLayout::from_template(Layout::Four, &[0, 1, 2, 3]);
        assert_eq!(a, b);
        let c = PaneLayout::from_template(Layout::Two, &[0, 1]);
        assert_ne!(a, c);
    }
}
