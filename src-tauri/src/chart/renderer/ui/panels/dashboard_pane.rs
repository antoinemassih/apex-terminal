//! Dashboard pane — auto-tiling grid of widgets.

use egui;
use super::super::style::*;
use super::super::super::gpu::*;
use super::super::widgets::layout::EmptyState;
use super::super::widgets::headers::PaneHeader;

const TILE_GAP: f32 = 6.0;
const HEADER_H: f32 = 28.0;

pub(crate) fn render(
    ui: &mut egui::Ui, _ctx: &egui::Context,
    panes: &mut [Chart], pane_idx: usize, _active_pane: &mut usize,
    _visible_count: usize, pane_rects: &[egui::Rect], theme_idx: usize,
    watchlist: &mut Watchlist,
) {
    let t = &THEMES[theme_idx];
    if pane_rects.is_empty() { return; }
    let rect = pane_rects[0];

    // Background
    ui.painter_at(rect).rect_filled(rect, 0.0, t.bg);
    // Activate pane on hover (don't allocate rect — that blocks widget clicks)
    if let Some(pos) = ui.ctx().pointer_hover_pos() {
        if rect.contains(pos) {
            *_active_pane = pane_idx;
        }
    }

    // Header (chrome widget) — matches heatmap_pane / portfolio_pane.
    let header_rect = egui::Rect::from_min_size(rect.min, egui::vec2(rect.width(), HEADER_H));
    {
        let mut header_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(header_rect)
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );
        header_ui.add(PaneHeader::new("Dashboard").theme(t));
    }
    let body_rect = egui::Rect::from_min_max(
        egui::pos2(rect.left(), rect.top() + HEADER_H),
        rect.max,
    );

    // Count visible widgets
    let widget_count = panes[pane_idx].chart_widgets.iter().filter(|w| w.visible).count();

    if widget_count == 0 {
        // Empty-state migrated to design-system widget. Render inside a child Ui
        // scoped to the pane rect so EmptyState's vertical_centered flow centers
        // correctly within the dashboard.
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(body_rect)
                .layout(egui::Layout::top_down(egui::Align::Center)),
        );
        EmptyState::new("\u{2637}", "No widgets", "Add widgets from the Widgets menu")
            .theme(t)
            .show(&mut child);
        return;
    }

    // Just use the existing floating widget renderer with the dashboard rect
    // This gives full widget interactivity (drag, collapse, mode toggle, etc.)
    let chart = &mut panes[pane_idx];

    // Force widgets into a grid layout by overriding their positions
    let content = egui::Rect::from_min_max(
        egui::pos2(body_rect.left() + TILE_GAP, body_rect.top() + TILE_GAP),
        egui::pos2(body_rect.right() - TILE_GAP, body_rect.bottom() - TILE_GAP));
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
    super::super::chart_widgets::draw_widgets(ui, chart, rect, t);
}
