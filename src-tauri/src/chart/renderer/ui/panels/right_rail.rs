//! `right_rail` — the single container that auto-arranges ALL open right-side
//! panels into columns. Each panel is Full / ½ / ⅓; the rail re-packs them for
//! efficiency every frame.
//!
//! ## The unification
//!
//! There is exactly ONE rail component and ONE dispatch path:
//!
//! * **The component** is [`SidePanelShell`] in rail-slot mode — it renders any
//!   panel's real header + body into a manager-assigned rect. Styling, the
//!   size-cycle control, the header underline/shadow: all identical for every
//!   panel. (`side_panel_shell.rs`.)
//! * **The dispatch** is the [`PANELS`] registry below: a flat table of
//!   `(id, is_open, render)`. The render loop iterates it — there is no
//!   per-panel `match`. Each panel exposes one [`RailPanelDef`] (`RAIL`) and one
//!   uniform entry point `rail_render(&mut RailCtx, RailSlot) -> bool`.
//!
//! A panel's `rail_render` only *parameterizes* the shared component (its title,
//! tabs, footer, body) — it never forks the styling. Adding a panel to the rail
//! is one line in [`PANELS`].
//!
//! The varying per-panel data (`panes` / `symbol` / `account_data`) is bundled
//! into [`RailCtx`] so every panel has the SAME signature.
//!
//! Excluded by design: `SplitSectionPanel`-wrapped panels (Signals / Feed /
//! Analysis), Chart Library (`&mut Vec` arg), and the floating `Modal` panels
//! (Spread, Order Health) — draggable popups, not dockable side panels.

use std::cell::RefCell;
use crate::chart_renderer::ui::style::tint;
use crate::ui_kit::sx::Tone;
use super::rail_layout::{RailHeight, compute_rail_rects, pack_columns};
use super::side_panel_shell::{RailSlot, take_size_cycle};
use super::super::style::{region_gap, color_alpha, alpha_solid, alpha_heavy};
use crate::chart_renderer::trading::{AccountSummary, Position, IbOrder};

type Theme = crate::chart_renderer::gpu::Theme;
type Watchlist = crate::chart_renderer::gpu::Watchlist;
type Chart = crate::chart_renderer::gpu::Chart;
type Layout = crate::chart_renderer::gpu::Layout;
pub(crate) type AccountData = Option<(AccountSummary, Vec<Position>, Vec<IbOrder>)>;

/// Everything a rail panel needs, bundled so every panel's render entry has the
/// SAME signature. Per-panel data differs (some need `panes`, some `symbol`,
/// orders needs `account_data`) — they just read the fields they care about.
pub(crate) struct RailCtx<'a> {
    pub ctx: &'a egui::Context,
    pub t: &'a Theme,
    pub watchlist: &'a mut Watchlist,
    pub panes: &'a mut [Chart],
    pub active_pane: usize,
    pub account_data: &'a AccountData,
    /// Active pane's symbol, pre-derived so panels needn't reborrow `panes`.
    pub symbol: String,
    /// Current workspace layout (needed by the Chart Library panel to save).
    pub layout: Layout,
}

/// One panel's registration with the rail. `render` parameterizes the shared
/// [`SidePanelShell`] component for this panel (title / tabs / footer / body)
/// and renders it into the slot; the size-cycle click is reported separately
/// through per-id egui memory (see [`take_size_cycle`]).
pub(crate) struct RailPanelDef {
    pub id: &'static str,
    pub is_open: fn(&Watchlist) -> bool,
    pub render: fn(&mut RailCtx, RailSlot),
}

/// The rail's panel registry — fixed left→right / top→bottom precedence for
/// packing. Adding a dockable panel to the rail = one line here.
pub(crate) static PANELS: &[RailPanelDef] = &[
    super::watchlist_panel::RAIL,
    super::orders_panel::RAIL,
    super::indicators_panel::RAIL,
    super::playbook_panel::RAIL,
    // Scanner moved into the Watchlist as the "SCAN" tab — no standalone rail entry.
    super::tape_panel::RAIL,
    super::journal_panel::RAIL,
    super::rrg_panel::RAIL,
    super::script_panel::RAIL,
    super::order_ledger_panel::RAIL,
    super::provenance_pane::RAIL,
    super::signals_panel::RAIL,
    super::analysis_panel::RAIL,
    super::auto_chart_panel::RAIL,
    super::feed_panel::RAIL,
    super::chart_library_panel::RAIL,
];

// Per-panel height (session-only; persistence is a follow-up). Keyed by id.
thread_local! {
    static HEIGHTS: RefCell<std::collections::HashMap<&'static str, RailHeight>> =
        RefCell::new(std::collections::HashMap::new());
}
// Rail width is pinned (`exact_width`) and driven by our own left-edge drag grip
// — egui's `SidePanel` resize state didn't persist reliably for the host. The
// width itself is persisted in `Watchlist::rail_col_width` (SidebarState), so it
// survives restarts; these are the interaction bounds + handle geometry.
const RAIL_COL_MIN: f32 = 240.0;
const RAIL_COL_MAX: f32 = 820.0;
const RAIL_GRIP_W: f32 = 8.0;

fn height_of(id: &'static str) -> RailHeight {
    HEIGHTS.with(|h| *h.borrow().get(id).unwrap_or(&RailHeight::Full))
}
fn cycle_height(id: &'static str) {
    HEIGHTS.with(|h| {
        let mut m = h.borrow_mut();
        let cur = *m.get(id).unwrap_or(&RailHeight::Full);
        m.insert(id, cur.next());
    });
}

// ── Duplicate instances (session-only) ───────────────────────────────────────
// A spawned instance = a second copy of a tabbed panel showing its OWN tab,
// tiled as an equal peer. Each tabbed panel takes an `instance_tab: Option<&mut
// u8>` override; the rail stores the tab here and dispatches by id. Binary: at
// most one duplicate per panel kind.
#[derive(Clone, Copy)]
struct Spawn { id: &'static str, tab: u8, height: RailHeight }
thread_local! {
    static SPAWNS: RefCell<Vec<Spawn>> = const { RefCell::new(Vec::new()) };
}

/// The tabbed panels that support duplicate instances → their static id.
fn spawnable_id(id: &str) -> Option<&'static str> {
    match id {
        "watchlist" => Some("watchlist"),
        "orders" => Some("orders"),
        "signals" => Some("signals"),
        "analysis" => Some("analysis"),
        "feed" => Some("feed"),
        "chart_library" => Some("chart_library"),
        _ => None,
    }
}

/// Request a duplicate instance (from a panel's right-click-tab menu). The new
/// instance carries `tab`; the base panel is set to Half so the two stack.
pub(crate) fn request_spawn(id: &str, tab: u8) {
    let Some(sid) = spawnable_id(id) else { return; };
    let added = SPAWNS.with(|s| {
        let mut v = s.borrow_mut();
        if v.iter().any(|sp| sp.id == sid) { false } // one duplicate per panel kind
        else { v.push(Spawn { id: sid, tab, height: RailHeight::Half }); true }
    });
    if added {
        HEIGHTS.with(|h| { h.borrow_mut().insert(sid, RailHeight::Half); });
    }
}

/// Render the auto-arranged right rail. Returns true when it owned ≥1 panel.
pub(crate) fn render(
    ctx: &egui::Context,
    watchlist: &mut Watchlist,
    panes: &mut [Chart],
    active_pane: usize,
    account_data: &AccountData,
    layout: Layout,
    t: &Theme,
) -> bool {
    // Which registered panels are open, in registry order.
    let open: Vec<&'static RailPanelDef> =
        PANELS.iter().filter(|p| (p.is_open)(watchlist)).collect();
    let nbase = open.len();
    let spawn_heights: Vec<RailHeight> =
        SPAWNS.with(|s| s.borrow().iter().map(|sp| sp.height).collect());
    if nbase == 0 && spawn_heights.is_empty() { return false; }

    // Instances = base panels (0..nbase) then duplicate spawns (nbase..).
    let mut heights: Vec<RailHeight> = open.iter().map(|p| height_of(p.id)).collect();
    heights.extend(spawn_heights.iter().copied());
    let symbol = panes.get(active_pane).map(|c| c.symbol.clone()).unwrap_or_default();
    let rgap = region_gap();
    let gap = if rgap > 0.0 { rgap } else { 1.0 };

    // Width is pinned + persisted in `Watchlist::rail_col_width`. Every column
    // shares the width; total grows with column count.
    let ncols = pack_columns(&heights).len().max(1) as f32;
    let col_w = watchlist.rail_col_width.clamp(RAIL_COL_MIN, RAIL_COL_MAX);
    let total_w = col_w * ncols + gap * (ncols - 1.0);

    // ── Resize handle ── a top-layer (Foreground) overlay straddling the rail's
    // left seam, so the GPU chart underneath can't intercept the drag (the
    // failure mode of an in-panel grip at the edge).
    let avail = ctx.available_rect();
    let seam_x = avail.right() - total_w;
    let handle = egui::Rect::from_min_size(
        egui::pos2(seam_x - RAIL_GRIP_W * 0.5, avail.top()),
        egui::vec2(RAIL_GRIP_W, avail.height()),
    );
    let hresp = egui::Area::new(egui::Id::new("rail_resize_handle"))
        .order(egui::Order::Foreground)
        .fixed_pos(handle.min)
        .show(ctx, |ui| {
            let r = ui.interact(handle, ui.id().with("grip"), egui::Sense::drag());
            let active = r.hovered() || r.dragged();
            if active { ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal); }
            let hx = handle.center().x;
            ui.painter().line_segment(
                [egui::pos2(hx, handle.top()), egui::pos2(hx, handle.bottom())],
                egui::Stroke::new(2.0, tint(t, Tone::Accent, if active { alpha_solid() } else { alpha_heavy() })),
            );
            r
        }).inner;
    if hresp.dragged() {
        // Dragging left (negative dx) widens; shared across columns.
        let nw = (col_w - hresp.drag_delta().x / ncols).clamp(RAIL_COL_MIN, RAIL_COL_MAX);
        watchlist.rail_col_width = nw;
    }
    if hresp.drag_stopped() {
        let w = watchlist.rail_col_width;
        watchlist.update_sidebar_state(|s| s.rail_col_width = w);
    }

    let mut cx = RailCtx { ctx, t, watchlist, panes, active_pane, account_data, symbol, layout };
    let mut cycled: Option<&'static str> = None;   // base panel split toggled
    let mut spawn_cycle: Option<usize> = None;      // duplicate split toggled
    let mut spawn_remove: Option<usize> = None;     // duplicate closed

    egui::SidePanel::right(egui::Id::new("right_rail_host"))
        .exact_width(total_w)
        .frame(egui::Frame::NONE.fill(t.bg))
        .show(ctx, |ui| {
            let area = ui.max_rect();
            let layer = ui.layer_id();
            let (rects, _) = compute_rail_rects(&heights, area.min, col_w, gap, area.height());
            // Visible border (border_variant @ alpha_solid — the same color the
            // inter-tab dividers use, so it reads clearly on the panel surface).
            let border = egui::Stroke::new(1.0, color_alpha(t.border_variant, alpha_solid()));
            for r in &rects {
                let slot_rect = r.rect.intersect(area);
                if !slot_rect.is_finite() || slot_rect.height() < 24.0 { continue; }
                let slot = RailSlot { rect: slot_rect, layer, height: heights[r.panel] };
                if r.panel < nbase {
                    let def = open[r.panel];
                    (def.render)(&mut cx, slot);
                    if take_size_cycle(cx.ctx) { cycled = Some(def.id); }
                } else {
                    // Duplicate instance — render the panel (by id) with its own
                    // tab from the spawn store; closing it removes the duplicate.
                    let k = r.panel - nbase;
                    let close = SPAWNS.with(|s| {
                        let mut spawns = s.borrow_mut();
                        let sp = &mut spawns[k];
                        match sp.id {
                            "watchlist" => super::watchlist_panel::draw(
                                cx.ctx, cx.watchlist, cx.panes, cx.active_pane, cx.t, Some(slot), Some(&mut sp.tab)),
                            "orders" => super::orders_panel::draw(
                                cx.ctx, cx.watchlist, cx.panes, cx.active_pane, cx.t, cx.account_data, Some(slot), Some(&mut sp.tab)),
                            "signals" => super::signals_panel::draw(
                                cx.ctx, cx.watchlist, cx.panes, cx.active_pane, cx.t, Some(slot), Some(&mut sp.tab)),
                            "analysis" => super::analysis_panel::draw(
                                cx.ctx, cx.watchlist, cx.panes, cx.active_pane, cx.t, Some(slot), Some(&mut sp.tab)),
                            "feed" => super::feed_panel::draw(
                                cx.ctx, cx.watchlist, cx.panes, cx.active_pane, cx.t, Some(slot), Some(&mut sp.tab)),
                            "chart_library" => super::chart_library_panel::draw(
                                cx.ctx, cx.watchlist, cx.panes, cx.layout, cx.t, Some(slot), Some(&mut sp.tab)),
                            _ => false,
                        }
                    });
                    if take_size_cycle(cx.ctx) { spawn_cycle = Some(k); }
                    if close { spawn_remove = Some(k); }
                }
                // Left / right / bottom borders, painted on top of the body. The
                // bottom border doubles as the divider between stacked split halves.
                ui.painter().line_segment([slot_rect.left_top(), slot_rect.left_bottom()], border);
                ui.painter().line_segment([slot_rect.right_top(), slot_rect.right_bottom()], border);
                ui.painter().line_segment([slot_rect.left_bottom(), slot_rect.right_bottom()], border);
            }
        });

    if let Some(id) = cycled { cycle_height(id); }
    if let Some(k) = spawn_cycle {
        SPAWNS.with(|s| { if let Some(sp) = s.borrow_mut().get_mut(k) { sp.height = sp.height.next(); } });
    }
    if let Some(k) = spawn_remove {
        SPAWNS.with(|s| { let mut v = s.borrow_mut(); if k < v.len() { v.remove(k); } });
    }
    true
}
