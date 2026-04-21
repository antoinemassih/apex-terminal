//! Dashboard pane — auto-tiling grid of widgets.

use egui;
use super::style::*;
use super::super::gpu::*;

const TILE_GAP: f32 = 6.0;

pub(crate) fn render(
    ui: &mut egui::Ui, _ctx: &egui::Context,
    panes: &mut [Chart], pane_idx: usize, _active_pane: &mut usize,
    _visible_count: usize, pane_rects: &[egui::Rect], theme_idx: usize,
    watchlist: &mut Watchlist,
) {
    let t = &THEMES[theme_idx];
    if pane_rects.is_empty() { return; }
    let rect = pane_rects[0];

    // Background + click to activate
    ui.painter_at(rect).rect_filled(rect, 0.0, t.bg);
    let body_resp = ui.allocate_rect(rect, egui::Sense::click());
    if body_resp.clicked() || body_resp.hovered() {
        *_active_pane = pane_idx;
    }

    // Count visible widgets
    let widget_count = panes[pane_idx].chart_widgets.iter().filter(|w| w.visible).count();

    if widget_count == 0 {
        let p = ui.painter_at(rect);
        p.text(egui::pos2(rect.center().x, rect.center().y - 14.0), egui::Align2::CENTER_CENTER,
            "\u{2637}", egui::FontId::proportional(32.0), t.dim.gamma_multiply(0.15));
        p.text(egui::pos2(rect.center().x, rect.center().y + 14.0), egui::Align2::CENTER_CENTER,
            "Add widgets from the Widgets menu", egui::FontId::monospace(FONT_SM),
            t.dim.gamma_multiply(0.4));
        return;
    }

    // Just use the existing floating widget renderer with the dashboard rect
    // This gives full widget interactivity (drag, collapse, mode toggle, etc.)
    let chart = &mut panes[pane_idx];

    // Force widgets into a grid layout by overriding their positions
    let content = egui::Rect::from_min_max(
        egui::pos2(rect.left() + TILE_GAP, rect.top() + TILE_GAP),
        egui::pos2(rect.right() - TILE_GAP, rect.bottom() - TILE_GAP));
    let avail_w = content.width();
    let n = widget_count;
    let cols = if avail_w > 600.0 && n >= 6 { 4 }
        else if avail_w > 450.0 && n >= 4 { 3 }
        else if avail_w > 250.0 && n >= 2 { 2 }
        else { 1 };
    let rows = (n + cols - 1) / cols;
    let tile_w = (avail_w - (cols - 1) as f32 * TILE_GAP) / cols as f32;
    let tile_h = ((content.height() - (rows - 1) as f32 * TILE_GAP) / rows as f32).clamp(60.0, 280.0);

    // Set each visible widget's position and size to match the grid
    let mut idx = 0;
    for w in chart.chart_widgets.iter_mut() {
        if !w.visible { continue; }
        let col = idx % cols;
        let row = idx / cols;
        // Convert pixel position to fractional position (as draw_widgets expects)
        let px = content.left() + col as f32 * (tile_w + TILE_GAP);
        let py = content.top() + row as f32 * (tile_h + TILE_GAP);
        w.x = (px - rect.left()) / rect.width();
        w.y = (py - rect.top()) / rect.height();
        w.w = tile_w;
        w.h = tile_h;
        w.display = crate::chart_renderer::WidgetDisplayMode::Card;
        w.dock = crate::chart_renderer::WidgetDock::Float;
        w.collapsed = false;
        w.anim_x = px;
        w.anim_y = py;
        w.anim_init = true;
        idx += 1;
    }

    // Render using the full widget system (which handles interaction, hover, buttons)
    super::chart_widgets::draw_widgets(ui, chart, rect, t);
}
