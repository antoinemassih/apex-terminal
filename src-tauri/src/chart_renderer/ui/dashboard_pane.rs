//! Dashboard pane — masonry grid of widgets without a chart.

use egui;
use super::style::*;
use super::super::gpu::*;

pub(crate) fn render(
    ui: &mut egui::Ui, ctx: &egui::Context,
    panes: &mut [Chart], pane_idx: usize, active_pane: &mut usize,
    visible_count: usize, pane_rects: &[egui::Rect], theme_idx: usize,
    watchlist: &mut Watchlist,
) {
    let t = &THEMES[theme_idx];
    let rect_idx = 0;
    if rect_idx >= pane_rects.len() { return; }
    let rect = pane_rects[rect_idx];

    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, t.bg);

    // Header
    painter.text(egui::pos2(rect.left() + 16.0, rect.top() + 14.0), egui::Align2::LEFT_CENTER,
        "DASHBOARD", egui::FontId::monospace(FONT_SM), t.dim);

    // Render all visible widgets in a masonry grid layout within this pane
    let chart = &mut panes[pane_idx];
    if chart.chart_widgets.is_empty() {
        // Empty state
        painter.text(rect.center(), egui::Align2::CENTER_CENTER,
            "Add widgets from the Widgets menu", egui::FontId::monospace(FONT_SM),
            t.dim.gamma_multiply(0.4));
        return;
    }

    // Use the existing widget renderer but with the pane rect as the canvas
    let widget_rect = egui::Rect::from_min_max(
        egui::pos2(rect.left(), rect.top() + 28.0),
        rect.max);
    super::chart_widgets::draw_widgets(ui, chart, widget_rect, t);
}
