//! Right-click chart interaction menu.
//!
//! The menu body is rendered inline in `gpu.rs` inside `resp.context_menu(|ui| { ... })`.
//! This module provides the output struct and shared helpers used by the menu.

use crate::chart_renderer::gpu::{Theme, Chart};
use crate::chart_renderer::gpu::Watchlist;
use crate::ui_kit::icons::Icon;

/// Post-render output from the chart interaction menu.
///
/// Currently the menu mutations happen inline in gpu.rs via `&mut Chart`/`&mut Watchlist`.
/// Future refactor: move the body here and return this struct.
pub struct ChartInteractionMenuOutput {
    _phantom: (),
}

/// Show the chart right-click context menu body.
///
/// Call this inside `resp.context_menu(|ui| { ... })` in gpu.rs.
/// `click_price` is the chart-space price at the right-click position.
/// `chart_n` is the total number of bars currently loaded.
///
/// This is a forward declaration; the full body remains in gpu.rs for now.
pub fn show_chart_interaction_menu(
    _ui: &mut egui::Ui,
    _t: &Theme,
    _chart: &mut Chart,
    _watchlist: &mut Watchlist,
    _click_price: f32,
    _chart_n: usize,
) -> ChartInteractionMenuOutput {
    ChartInteractionMenuOutput { _phantom: () }
}
