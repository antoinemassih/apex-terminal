//! `Pane` trait — the unified contract every pane-type component implements.
//!
//! Panes are full-area renderers (unlike side panels which are egui windows).
//! Each pane receives a `PaneContext` containing the common denominator of
//! shared state, then renders into a `&mut egui::Ui`.
//!
//! # What goes in PaneContext
//!
//! Fields present in ≥2 of the 4 migrated panes:
//!  - `theme`        — all 4 use it
//!  - `panes`        — portfolio + plays + dom use it
//!  - `pane_idx`     — portfolio + spreadsheet use it (active_pane tracking)
//!  - `active_pane`  — portfolio + spreadsheet write it
//!  - `pane_rects`   — portfolio + spreadsheet use it
//!  - `watchlist`    — plays + spreadsheet (watchlist is `_watchlist` in
//!                     spreadsheet but kept for future use; portfolio reads plays)
//!
//! Fields excluded from PaneContext (single-pane only):
//!  - `account_data` — only portfolio_pane; passed via `PortfolioPaneAdapter`
//!  - DOM state refs — only dom_panel; 10+ args, all DOM-specific
//!
//! # Migration status
//!
//! Wrapped: portfolio_pane, spreadsheet_pane, plays_panel, dom_panel.
//! portfolio_pane call site in gpu.rs migrated to use `Pane` trait.

use egui;
use crate::chart_renderer::gpu::{Chart, Theme, Watchlist};
use crate::chart_renderer::trading::{AccountSummary, Position, IbOrder};

// ── Context ──────────────────────────────────────────────────────────────────

/// Shared context passed to every pane's `render` call.
///
/// This is the common denominator across all pane-type renderers.
/// Pane-specific state that only one pane needs is stored in its adapter struct,
/// not here — keeping this context lean and stable.
pub struct PaneContext<'a> {
    pub theme: &'a Theme,
    /// Slice of all panes — use `&mut panes[..]` to coerce from `Vec<Chart>`.
    pub panes: &'a mut [Chart],
    pub pane_idx: usize,
    pub active_pane: &'a mut usize,
    pub pane_rects: &'a [egui::Rect],
    pub watchlist: &'a mut Watchlist,
}

// ── Trait ─────────────────────────────────────────────────────────────────────

/// Unified contract for every pane-type renderer.
pub trait Pane {
    /// Renders this pane into the supplied `ui` / `ctx`.
    ///
    /// The pane receives its full area via `cx.pane_rects[0]` (or however it
    /// slices the rect array internally). The pane is responsible for painting
    /// its background and updating `*cx.active_pane` on hover.
    fn render(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, cx: &mut PaneContext<'_>);
}

// ── Concrete adapters ─────────────────────────────────────────────────────────

/// Adapter for [`portfolio_pane`].
///
/// `account_data` is portfolio-only state; it lives in the adapter, not in
/// `PaneContext`, because no other pane needs it.
pub struct PortfolioPaneAdapter<'d> {
    pub account_data: &'d Option<(AccountSummary, Vec<Position>, Vec<IbOrder>)>,
    pub theme_idx: usize,
}

impl<'d> Pane for PortfolioPaneAdapter<'d> {
    fn render(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, cx: &mut PaneContext<'_>) {
        super::panels::portfolio_pane::render(
            ui, ctx,
            cx.panes, cx.pane_idx, cx.active_pane,
            1, cx.pane_rects,
            self.theme_idx, cx.watchlist,
            self.account_data,
        );
    }
}

/// Adapter for [`spreadsheet_pane`].
pub struct SpreadsheetPaneAdapter {
    pub theme_idx: usize,
}

impl Pane for SpreadsheetPaneAdapter {
    fn render(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, cx: &mut PaneContext<'_>) {
        super::panels::spreadsheet_pane::render(
            ui, ctx,
            cx.panes, cx.pane_idx, cx.active_pane,
            1, cx.pane_rects,
            self.theme_idx, cx.watchlist,
        );
    }
}

/// Adapter for [`plays_panel`].
///
/// `plays_panel::draw_content` renders into an *existing* `Ui` (not a new
/// egui window), so the adapter simply forwards the call.  `pane_rects` is
/// not used — the parent Ui already constrains the area.
pub struct PlaysPaneAdapter;

impl Pane for PlaysPaneAdapter {
    fn render(&mut self, ui: &mut egui::Ui, _ctx: &egui::Context, cx: &mut PaneContext<'_>) {
        // draw_content takes `ap` = active pane index (for chart interaction).
        let ap = *cx.active_pane;
        super::panels::plays_panel::draw_content(
            ui,
            cx.watchlist,
            cx.panes,
            ap,
            cx.theme,
        );
    }
}

/// Adapter for [`dom_panel`].
///
/// The DOM panel has ~13 bespoke parameters (price ladder, drag state, order
/// refs, etc.) that are DOM-specific and shared with no other pane. They are
/// stored in this adapter rather than polluting `PaneContext`.
pub struct DomPaneAdapter<'d> {
    pub dom_rect: egui::Rect,
    pub current_price: f32,
    pub levels: &'d [super::panels::dom_panel::DomLevel],
    pub tick_size: f32,
    pub center_price: &'d mut f32,
    pub dom_width: &'d mut f32,
    pub orders: &'d [crate::chart_renderer::trading::OrderLevel],
    pub dom_selected_price: &'d mut Option<f32>,
    pub dom_order_type: &'d mut super::panels::dom_panel::DomOrderType,
    pub order_qty: &'d mut u32,
    pub new_order: &'d mut Option<(crate::chart_renderer::trading::OrderSide, f32, u32)>,
    pub cancel_all: &'d mut bool,
    pub cancel_order_id: &'d mut Option<u32>,
    pub move_order: &'d mut Option<(u32, f32)>,
    pub dom_armed: &'d mut bool,
    pub dom_col_mode: &'d mut u8,
    pub dom_dragging: &'d mut Option<(u32, f32)>,
}

impl<'d> Pane for DomPaneAdapter<'d> {
    fn render(&mut self, ui: &mut egui::Ui, _ctx: &egui::Context, cx: &mut PaneContext<'_>) {
        super::panels::dom_panel::draw(
            ui, self.dom_rect, self.current_price, self.levels,
            self.tick_size, self.center_price, self.dom_width,
            self.orders, self.dom_selected_price, self.dom_order_type,
            self.order_qty, self.new_order, self.cancel_all,
            self.cancel_order_id, self.move_order, self.dom_armed,
            self.dom_col_mode, self.dom_dragging, cx.theme,
        );
    }
}
