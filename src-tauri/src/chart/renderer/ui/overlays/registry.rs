//! `OverlayWidget` — the canonical per-kind interface for on-chart overlays.
//!
//! Every overlay kind answers two questions: what is its collapsed one-line
//! badge (`mini`), and how does its full body paint (`body`). Historically these
//! lived as two parallel 40-arm `match` statements (`mini_summary`,
//! `draw_widget_body`) buried in the 3k-line `chart_widgets` module, alongside
//! the per-kind `icon()` / `label()` matches on the enum itself.
//!
//! This trait gives that dispatch a single named contract and call seam —
//! `kind.mini(..)`, `kind.body(..)` — so callers depend on the *interface*, not
//! on free functions. The `impl OverlayWidget for ChartWidgetKind` lives in
//! `chart_widgets`, next to the private `draw_*` body fns it dispatches to (so no
//! visibility plumbing is needed). The two inner `match`es remain the impl's
//! detail for now; the intended follow-up is to move each kind's `mini`/`body`
//! logic into per-kind code behind this same stable contract, retiring the
//! god-matches without touching any call site.
//!
//! `icon()` / `label()` deliberately stay as inherent methods on the enum in its
//! definition module (`chart::renderer::mod`) — that is their correct home as
//! pure enum metadata; hoisting them into the UI layer would worsen, not improve,
//! separation. They are reachable wherever a `ChartWidgetKind` is in scope.

use egui::{self, Color32};
use crate::chart_renderer::gpu::Theme;
use crate::chart_renderer::ChartWidgetKind;
use super::super::chart_widgets::{WidgetData, WidgetBtnAction};

/// The render contract every on-chart overlay kind implements.
pub(crate) trait OverlayWidget {
    /// Collapsed one-line badge for HUD / minimised display: `(label, value, colour)`.
    fn mini(self, wd: &WidgetData, t: &Theme) -> (&'static str, String, Color32);

    /// Paint the full widget body into `body`. Push any interactive button
    /// hit-rects into `btns` for the caller's click routing; `hover` is the
    /// pointer position when over the card (for in-body hover affordances).
    fn body(
        self, p: &egui::Painter, body: egui::Rect, wd: &WidgetData, t: &Theme,
        hover: Option<egui::Pos2>, btns: &mut Vec<(egui::Rect, WidgetBtnAction)>,
    );
}
