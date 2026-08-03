//! Dashboard pane — auto-tiling grid of widgets.

use egui;
use super::super::style as st;
use super::super::super::gpu::*;
use super::super::components::layout::EmptyState;
use crate::ui_kit::widgets::Header;

const TILE_GAP: f32 = 6.0;
/// Must match `HeaderVariant::Panel.height()` = 28.0.
const HEADER_H: f32 = 28.0;

pub(crate) fn render(
    ui: &mut egui::Ui, _ctx: &egui::Context,
    panes: &mut [Chart], pane_idx: usize, _active_pane: &mut usize,
    _visible_count: usize, pane_rects: &[egui::Rect], theme_idx: usize,
    watchlist: &mut Watchlist,
) {
    let t_owned = crate::chart_renderer::gpu::get_theme(theme_idx); let t = &t_owned;
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

    // Header — canonical ui_kit::Header::panel replaces legacy PaneHeader.
    // Both are 28px; behaviour and density are identical.
    let header_rect = egui::Rect::from_min_size(rect.min, egui::vec2(rect.width(), HEADER_H));
    {
        let mut header_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(header_rect)
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );
        Header::panel("Dashboard").show(&mut header_ui, t);
    }
    let mut header_bottom = rect.top() + HEADER_H;

    // ── Wave 10 breadth widget (additive strip) ────────────────────────────
    // Cheap numeric strip directly under the header. Hidden when the breadth
    // projector hasn't returned anything yet (avoids a blank reservation).
    const BREADTH_WIDGET_H: f32 = 26.0;
    if let Some(b) = crate::apex_data::live_state::get_breadth("us") {
        let strip_rect = egui::Rect::from_min_max(
            egui::pos2(rect.left() + 6.0, header_bottom + 2.0),
            egui::pos2(rect.right() - 6.0, header_bottom + 2.0 + BREADTH_WIDGET_H),
        );
        let p = ui.painter_at(strip_rect);
        p.rect_filled(strip_rect, st::radius_sm(), t.toolbar_bg);
        let bull_pct = if b.advancers + b.decliners > 0 {
            b.advancers as f32 / (b.advancers + b.decliners) as f32
        } else { 0.5 };
        // Left half: adv/dec totals
        let s1 = format!("Adv {}  Dec {}", b.advancers, b.decliners);
        p.text(
            egui::pos2(strip_rect.left() + 8.0, strip_rect.center().y),
            egui::Align2::LEFT_CENTER, s1,
            st::mono_xs_plus(),
            if bull_pct >= 0.5 { t.bull } else { t.bear },
        );
        // Middle: NH/NL
        let s2 = format!("NH {} / NL {}", b.new_highs, b.new_lows);
        p.text(
            strip_rect.center(),
            egui::Align2::CENTER_CENTER, s2,
            st::mono_xs_plus(), t.text,
        );
        // Right: % above SMA200
        let s3 = format!("{:.0}% > SMA200", b.pct_above_sma200);
        p.text(
            egui::pos2(strip_rect.right() - 8.0, strip_rect.center().y),
            egui::Align2::RIGHT_CENTER, s3,
            st::mono_xs_plus(), t.dim,
        );
        header_bottom += BREADTH_WIDGET_H + 4.0;
    }

    let body_rect = egui::Rect::from_min_max(
        egui::pos2(rect.left(), header_bottom),
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

    // ── DS-6.1: the layout is the ARCHETYPE's, not one hardcoded grid ──
    //
    // This used to pick a column count from the widget count and hand every
    // widget the same box. That is one layout and it is nobody's design:
    // Aperture's mosaic (12 cols x 92px, typed spans) and the Lucid/Meridien
    // editorial grid (300 / 1fr / 360) are both explicitly non-uniform.
    //
    // Per DS-6.0 D1 the archetype is the theme's default with a per-workspace
    // override, and per D3 this is a workspace VIEW — so nothing here reaches
    // into the root shell or sacred core.rs.
    let archetype = {
        let ss = crate::chart_renderer::ui::style::active_style_system();
        ss.shell.resolve_archetype(watchlist.workspace_archetype())
    };
    let kinds: Vec<crate::chart_renderer::ChartWidgetKind> = chart
        .chart_widgets.iter().filter(|w| w.visible).map(|w| w.kind).collect();
    let tiles = super::dashboard_layout::solve(archetype, body_rect, &kinds, TILE_GAP);

    // Set each visible widget's position and size from the solved grid.
    for (w, tile) in chart.chart_widgets.iter_mut().filter(|w| w.visible).zip(tiles) {
        // Convert pixel position to fractional position (as draw_widgets expects)
        w.x = if rect.width() > 0.0 { (tile.left() - rect.left()) / rect.width() } else { 0.0 };
        w.y = if rect.height() > 0.0 { (tile.top() - rect.top()) / rect.height() } else { 0.0 };
        w.w = tile.width();
        w.h = tile.height();
        w.display = crate::chart_renderer::WidgetDisplayMode::Card;
        w.dock = crate::chart_renderer::WidgetDock::Float;
        w.collapsed = false;
        w.anim_x = tile.left();
        w.anim_y = tile.top();
        w.anim_init = true;
    }

    // Render using the full widget system (which handles interaction, hover, buttons)
    super::super::chart_widgets::draw_widgets(ui, chart, rect, t);
}
