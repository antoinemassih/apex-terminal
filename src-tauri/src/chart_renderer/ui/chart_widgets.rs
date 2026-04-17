//! Chart Widgets — floating info cards rendered on the chart canvas.
//! Each widget is a draggable, collapsible card showing a specific data visualization.
//! Think Apple Watch complications but for trading charts.

use egui;
use super::style::*;
use super::super::gpu::*;
use crate::chart_renderer::ChartWidgetKind;

/// Render all visible widgets for a chart pane.
/// Called from render_chart_pane after all chart content is drawn.
pub(crate) fn draw_widgets(
    ui: &mut egui::Ui,
    chart: &mut Chart,
    rect: egui::Rect,  // the chart body rect (inside pane header, inside axes)
    t: &Theme,
) {
    let painter = ui.painter_at(rect);

    for wi in 0..chart.chart_widgets.len() {
        let w = &chart.chart_widgets[wi];
        if !w.visible { continue; }

        // Compute absolute position from fractional coordinates
        let abs_x = rect.left() + w.x * rect.width();
        let abs_y = rect.top() + w.y * rect.height();
        let card_w = w.w;
        let card_h = if w.collapsed { 24.0 } else { w.h };

        let card_rect = egui::Rect::from_min_size(egui::pos2(abs_x, abs_y), egui::vec2(card_w, card_h));

        // Skip if fully outside visible area
        if !rect.intersects(card_rect) { continue; }

        // ── Card background with glassmorphism effect ──
        let bg = egui::Color32::from_rgba_unmultiplied(
            t.toolbar_bg.r(), t.toolbar_bg.g(), t.toolbar_bg.b(), 210);
        painter.rect_filled(card_rect, RADIUS_LG, bg);
        painter.rect_stroke(card_rect, RADIUS_LG,
            egui::Stroke::new(STROKE_THIN, color_alpha(t.toolbar_border, ALPHA_STRONG)),
            egui::StrokeKind::Outside);

        // ── Title bar (always visible, even when collapsed) ──
        let title_h = 22.0;
        let title_rect = egui::Rect::from_min_size(card_rect.min, egui::vec2(card_w, title_h));

        // Icon + name
        let kind = w.kind;
        painter.text(
            egui::pos2(title_rect.left() + 8.0, title_rect.center().y),
            egui::Align2::LEFT_CENTER,
            kind.icon(),
            egui::FontId::proportional(FONT_SM),
            t.accent);
        painter.text(
            egui::pos2(title_rect.left() + 22.0, title_rect.center().y),
            egui::Align2::LEFT_CENTER,
            kind.label(),
            egui::FontId::monospace(FONT_XS),
            TEXT_PRIMARY);

        // Collapse/expand dot
        let dot_x = title_rect.right() - 10.0;
        let dot_y = title_rect.center().y;
        painter.circle_filled(egui::pos2(dot_x, dot_y), 3.0,
            if w.collapsed { t.dim.gamma_multiply(0.5) } else { t.accent });

        // ── Widget body (placeholder content per kind) ──
        if !w.collapsed {
            let body_rect = egui::Rect::from_min_size(
                egui::pos2(card_rect.left(), card_rect.top() + title_h),
                egui::vec2(card_w, card_h - title_h));
            let cx = body_rect.center().x;
            let cy = body_rect.center().y;

            match kind {
                ChartWidgetKind::TrendStrength => {
                    // Gauge arc placeholder
                    let score = chart.trend_health_score;
                    let color = if score > 70.0 { t.bull } else if score > 40.0 { t.accent } else { t.bear };
                    let label = if score > 0.0 { format!("{:.0}", score) } else { "—".into() };
                    painter.text(egui::pos2(cx, cy - 4.0), egui::Align2::CENTER_CENTER,
                        &label, egui::FontId::monospace(FONT_XL), color);
                    painter.text(egui::pos2(cx, cy + 14.0), egui::Align2::CENTER_CENTER,
                        "TREND", egui::FontId::monospace(FONT_XS), t.dim.gamma_multiply(0.5));
                }
                ChartWidgetKind::Momentum => {
                    painter.text(egui::pos2(cx, cy), egui::Align2::CENTER_CENTER,
                        "MOM", egui::FontId::monospace(FONT_LG), t.accent);
                    painter.text(egui::pos2(cx, cy + 16.0), egui::Align2::CENTER_CENTER,
                        "placeholder", egui::FontId::monospace(FONT_XS), t.dim.gamma_multiply(0.4));
                }
                ChartWidgetKind::Volatility => {
                    painter.text(egui::pos2(cx, cy), egui::Align2::CENTER_CENTER,
                        "VOL", egui::FontId::monospace(FONT_LG), t.accent);
                    painter.text(egui::pos2(cx, cy + 16.0), egui::Align2::CENTER_CENTER,
                        "placeholder", egui::FontId::monospace(FONT_XS), t.dim.gamma_multiply(0.4));
                }
                ChartWidgetKind::SessionTimer => {
                    painter.text(egui::pos2(cx, cy - 2.0), egui::Align2::CENTER_CENTER,
                        "00:00:00", egui::FontId::monospace(FONT_LG), TEXT_PRIMARY);
                    painter.text(egui::pos2(cx, cy + 14.0), egui::Align2::CENTER_CENTER,
                        "to close", egui::FontId::monospace(FONT_XS), t.dim.gamma_multiply(0.5));
                }
                ChartWidgetKind::VolumeProfile => {
                    // Mini horizontal bars placeholder
                    let bar_x = body_rect.left() + 8.0;
                    let bar_w = body_rect.width() - 16.0;
                    for i in 0..8 {
                        let y = body_rect.top() + 6.0 + i as f32 * 18.0;
                        let pct = 0.3 + (i as f32 * 0.1).sin().abs() * 0.7;
                        let color = if i == 4 { t.accent } else { color_alpha(t.dim, ALPHA_MUTED) };
                        painter.rect_filled(
                            egui::Rect::from_min_size(egui::pos2(bar_x, y), egui::vec2(bar_w * pct, 12.0)),
                            2.0, color);
                    }
                }
                ChartWidgetKind::KeyLevels => {
                    let levels = ["R2  458.50", "R1  455.20", "PP  452.00", "S1  448.80", "S2  445.50"];
                    for (i, level) in levels.iter().enumerate() {
                        let y = body_rect.top() + 8.0 + i as f32 * 18.0;
                        let col = if i < 2 { t.bear } else if i == 2 { t.accent } else { t.bull };
                        painter.text(egui::pos2(body_rect.left() + 10.0, y + 6.0),
                            egui::Align2::LEFT_CENTER, *level, egui::FontId::monospace(FONT_SM), col);
                    }
                }
                ChartWidgetKind::OptionGreeks => {
                    let greeks = [("Delta", "0.45"), ("Gamma", "0.03"), ("Theta", "-0.12"), ("Vega", "0.08")];
                    for (i, (name, val)) in greeks.iter().enumerate() {
                        let y = body_rect.top() + 8.0 + i as f32 * 16.0;
                        painter.text(egui::pos2(body_rect.left() + 10.0, y + 5.0),
                            egui::Align2::LEFT_CENTER, *name, egui::FontId::monospace(FONT_XS), t.dim);
                        painter.text(egui::pos2(body_rect.right() - 10.0, y + 5.0),
                            egui::Align2::RIGHT_CENTER, *val, egui::FontId::monospace(FONT_SM), TEXT_PRIMARY);
                    }
                }
                ChartWidgetKind::RiskReward => {
                    painter.text(egui::pos2(cx, cy - 4.0), egui::Align2::CENTER_CENTER,
                        "2.8 : 1", egui::FontId::monospace(FONT_XL), t.bull);
                    painter.text(egui::pos2(cx, cy + 14.0), egui::Align2::CENTER_CENTER,
                        "R : R", egui::FontId::monospace(FONT_XS), t.dim.gamma_multiply(0.5));
                }
                ChartWidgetKind::MarketBreadth => {
                    let rows = [("Adv/Dec", "1842 / 1156"), ("New Hi", "48"), ("New Lo", "12"), ("VIX", "18.5")];
                    for (i, (label, val)) in rows.iter().enumerate() {
                        let y = body_rect.top() + 8.0 + i as f32 * 18.0;
                        painter.text(egui::pos2(body_rect.left() + 10.0, y + 5.0),
                            egui::Align2::LEFT_CENTER, *label, egui::FontId::monospace(FONT_XS), t.dim);
                        painter.text(egui::pos2(body_rect.right() - 10.0, y + 5.0),
                            egui::Align2::RIGHT_CENTER, *val, egui::FontId::monospace(FONT_SM), TEXT_PRIMARY);
                    }
                }
                ChartWidgetKind::Custom => {
                    painter.text(egui::pos2(cx, cy), egui::Align2::CENTER_CENTER,
                        "Custom Widget", egui::FontId::monospace(FONT_SM), t.dim.gamma_multiply(0.5));
                }
            }
        }

        // ── Interaction: drag to move, click title to collapse ──
        let drag_resp = ui.interact(title_rect,
            egui::Id::new(("widget_drag", wi)), egui::Sense::click_and_drag());

        if drag_resp.dragged_by(egui::PointerButton::Primary) {
            if let Some(delta) = Some(drag_resp.drag_delta()) {
                let new_x = chart.chart_widgets[wi].x + delta.x / rect.width();
                let new_y = chart.chart_widgets[wi].y + delta.y / rect.height();
                chart.chart_widgets[wi].x = new_x.clamp(0.0, 0.95);
                chart.chart_widgets[wi].y = new_y.clamp(0.0, 0.95);
            }
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        } else if drag_resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
        }

        if drag_resp.clicked() {
            chart.chart_widgets[wi].collapsed = !chart.chart_widgets[wi].collapsed;
        }
    }
}
