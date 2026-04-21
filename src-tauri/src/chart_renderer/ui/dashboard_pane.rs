//! Dashboard pane — auto-tiling grid of widgets.
//! Widgets are arranged in a responsive masonry grid that fills the pane.
//! Each widget gets a fixed tile in the grid — no floating, no overlap.

use egui;
use super::style::*;
use super::super::gpu::*;
use crate::chart_renderer::ChartWidgetKind;

const TILE_GAP: f32 = 6.0;
const MIN_TILE_W: f32 = 150.0;

pub(crate) fn render(
    ui: &mut egui::Ui, _ctx: &egui::Context,
    panes: &mut [Chart], pane_idx: usize, _active_pane: &mut usize,
    _visible_count: usize, pane_rects: &[egui::Rect], theme_idx: usize,
    watchlist: &mut Watchlist,
) {
    let t = &THEMES[theme_idx];
    if pane_rects.is_empty() { return; }
    let rect = pane_rects[0];

    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, t.bg);

    let chart = &mut panes[pane_idx];
    let visible: Vec<usize> = chart.chart_widgets.iter().enumerate()
        .filter(|(_, w)| w.visible).map(|(i, _)| i).collect();

    if visible.is_empty() {
        // Empty state with helpful prompt
        painter.text(egui::pos2(rect.center().x, rect.center().y - 14.0), egui::Align2::CENTER_CENTER,
            "\u{2637}", egui::FontId::proportional(32.0), t.dim.gamma_multiply(0.15));
        painter.text(egui::pos2(rect.center().x, rect.center().y + 14.0), egui::Align2::CENTER_CENTER,
            "Add widgets from the Widgets menu", egui::FontId::monospace(FONT_SM),
            t.dim.gamma_multiply(0.4));
        painter.text(egui::pos2(rect.center().x, rect.center().y + 28.0), egui::Align2::CENTER_CENTER,
            "They'll auto-arrange in a tile grid", egui::FontId::monospace(7.0),
            t.dim.gamma_multiply(0.25));
        return;
    }

    // ── Compute grid layout ──
    let content = egui::Rect::from_min_max(
        egui::pos2(rect.left() + TILE_GAP, rect.top() + TILE_GAP),
        egui::pos2(rect.right() - TILE_GAP, rect.bottom() - TILE_GAP));
    let avail_w = content.width();

    // Determine number of columns based on available width and widget count
    let n = visible.len();
    let cols = if avail_w > 600.0 && n >= 6 { 4 }
        else if avail_w > 450.0 && n >= 4 { 3 }
        else if avail_w > 250.0 && n >= 2 { 2 }
        else { 1 };
    let tile_w = (avail_w - (cols - 1) as f32 * TILE_GAP) / cols as f32;
    let rows = (n + cols - 1) / cols;

    // Tile height: distribute evenly across available height, but cap
    let max_tile_h = (content.height() - (rows - 1) as f32 * TILE_GAP) / rows as f32;
    let tile_h = max_tile_h.clamp(60.0, 250.0);

    // ── Render tiles ──
    // Use the widget data cache
    let wd = chart.widget_cache.take().unwrap_or_else(|| {
        super::chart_widgets::WidgetDataCache::from_chart(chart)
    });

    let hover_pos = ui.ctx().pointer_hover_pos();

    for (idx, &wi) in visible.iter().enumerate() {
        let col = idx % cols;
        let row = idx / cols;
        let tx = content.left() + col as f32 * (tile_w + TILE_GAP);
        let ty = content.top() + row as f32 * (tile_h + TILE_GAP);

        if ty > content.bottom() { break; } // clip vertically

        let tile_rect = egui::Rect::from_min_size(egui::pos2(tx, ty), egui::vec2(tile_w, tile_h));
        let kind = chart.chart_widgets[wi].kind;

        // Tile background
        let tile_hovered = hover_pos.map(|p| tile_rect.contains(p)).unwrap_or(false);
        let bg = if tile_hovered {
            color_alpha(t.toolbar_border, ALPHA_SUBTLE)
        } else {
            color_alpha(t.toolbar_border, 12)
        };
        painter.rect_filled(tile_rect, RADIUS_LG, bg);
        painter.rect_stroke(tile_rect, RADIUS_LG,
            egui::Stroke::new(0.5, color_alpha(t.toolbar_border, if tile_hovered { ALPHA_LINE } else { ALPHA_FAINT })),
            egui::StrokeKind::Outside);

        // Tile header: icon + label
        let header_h = 20.0;
        painter.text(egui::pos2(tx + 8.0, ty + header_h * 0.5), egui::Align2::LEFT_CENTER,
            kind.icon(), egui::FontId::proportional(FONT_SM),
            t.accent.gamma_multiply(0.6));
        painter.text(egui::pos2(tx + 22.0, ty + header_h * 0.5), egui::Align2::LEFT_CENTER,
            kind.label(), egui::FontId::monospace(FONT_XS),
            t.dim);

        // Separator
        painter.line_segment(
            [egui::pos2(tx + 6.0, ty + header_h), egui::pos2(tx + tile_w - 6.0, ty + header_h)],
            egui::Stroke::new(0.3, color_alpha(t.toolbar_border, ALPHA_FAINT)));

        // Widget body area
        let body = egui::Rect::from_min_max(
            egui::pos2(tx + 2.0, ty + header_h + 2.0),
            egui::pos2(tx + tile_w - 2.0, ty + tile_h - 2.0));

        // Render widget content using existing renderers
        let mut btns = Vec::new();
        super::chart_widgets::draw_widget_body_pub(&painter, body, kind, &wd, t, hover_pos, &mut btns);
    }

    // Store cache back
    chart.widget_cache = Some(wd);
}
