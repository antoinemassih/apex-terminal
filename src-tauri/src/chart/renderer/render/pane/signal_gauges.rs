//! Signal gauges (trend health, exit gauge, precursor badge) and the VIX expiry card.
//!
//! Extracted from `core.rs` — post-paint overlays with no hot-path state.
//! These are painted on every frame but only when the relevant scores are
//! non-zero, and they share no state with the candle/indicator pipeline.

#![allow(unused_imports)]

use crate::chart_renderer::gpu::*;
use crate::chart_renderer::ui::style::{
    color_alpha, mono_2xs, mono_xs, mono_sm,
    COLOR_AMBER,
};

/// Render signal gauges (trend health, exit, precursor pills) and the VIX card.
///
/// Parameters from `render_chart_pane` scope:
/// - `rect`    — full pane rect (gauges anchor to `rect.right()` / `rect.bottom()`)
/// - `painter` — pane painter
/// - `t`       — resolved `Theme`
/// - `ctx`     — egui context (for `ctx.input(|i| i.time)` pulse animations)
/// - `chart`   — active `Chart` (read: scores, VIX fields, flags)
pub(super) fn render_signal_gauges(
    rect: egui::Rect,
    painter: &egui::Painter,
    t: &Theme,
    ctx: &egui::Context,
    chart: &Chart,
) {
    // ── Signal gauges — compact pill design, top-right ─────────────────
    {
        let gauge_x = rect.right() - 100.0;
        let mut gauge_y = rect.top() + 6.0;
        let pill_h = 18.0;
        let pill_w = 90.0;
        let pill_r = pill_h / 2.0; // fully rounded ends
        let bg = egui::Color32::from_rgba_unmultiplied(18, 18, 24, 210);

        // Helper: draw one gauge pill
        let draw_pill = |painter: &egui::Painter, y: f32, label: &str, score: f32, color: egui::Color32| {
            // Dark pill background
            painter.rect_filled(
                egui::Rect::from_min_size(egui::pos2(gauge_x, y), egui::vec2(pill_w, pill_h)),
                pill_r, bg,
            );
            // Thin fill bar inside (2px from edges)
            let bar_y = y + pill_h - 4.0;
            let bar_w = (pill_w - 8.0) * (score / 100.0).min(1.0);
            painter.rect_filled(
                egui::Rect::from_min_size(egui::pos2(gauge_x + 4.0, bar_y), egui::vec2(bar_w, 2.0)),
                1.0, color_alpha(color, 200),
            );
            // Left: colored dot + label
            painter.circle_filled(egui::pos2(gauge_x + 10.0, y + pill_h / 2.0), 3.0, color);
            painter.text(
                egui::pos2(gauge_x + 17.0, y + pill_h / 2.0), egui::Align2::LEFT_CENTER,
                label, mono_2xs(),
                t.dim,
            );
            // Right: score
            painter.text(
                egui::pos2(gauge_x + pill_w - 6.0, y + pill_h / 2.0), egui::Align2::RIGHT_CENTER,
                format!("{:.0}", score), mono_xs(), color,
            );
        };

        // ── Trend Health ─────────────────────────────────────────────────
        if chart.show_trend_health && chart.trend_health_score > 0.0 {
            let th = chart.trend_health_score;
            let th_color = if th > 70.0 { t.bull }
                else if th > 40.0 { COLOR_AMBER }
                else { t.bear };
            let dir = match chart.trend_health_direction { 1 => "TH ▲", -1 => "TH ▼", _ => "TH ─" };
            draw_pill(&painter, gauge_y, dir, th, th_color);
            gauge_y += pill_h + 2.0;
        }

        // ── Exit Gauge ───────────────────────────────────────────────────
        if chart.show_exit_gauge && chart.exit_gauge_score > 0.0 {
            let eg = chart.exit_gauge_score;
            let eg_color = if eg > 80.0 { t.bear }
                else if eg > 60.0 { COLOR_AMBER }
                else if eg > 40.0 { COLOR_AMBER }
                else { t.bull };
            let label = if eg > 80.0 { "EXIT" } else if eg > 60.0 { "CLOSE" } else if eg > 40.0 { "TIGHT" } else { "HOLD" };
            draw_pill(&painter, gauge_y, label, eg, eg_color);

            // Subtle glow when critical
            if eg > 80.0 {
                let pulse = ((ctx.input(|i| i.time) * 3.0).sin() * 0.3 + 0.7) as f32;
                painter.rect_stroke(
                    egui::Rect::from_min_size(egui::pos2(gauge_x - 1.0, gauge_y - 1.0), egui::vec2(pill_w + 2.0, pill_h + 2.0)),
                    pill_r, egui::Stroke::new(1.0, color_alpha(t.bear, (pulse * 120.0) as u8)), egui::StrokeKind::Outside,
                );
                ctx.request_repaint();
            }
            gauge_y += pill_h + 2.0;
        }

        // ── Precursor Badge ──────────────────────────────────────────────
        if chart.show_precursor && chart.precursor_active && chart.precursor_score > 30.0 {
            let pr_color = match chart.precursor_direction {
                d if d > 0 => t.bull,
                d if d < 0 => t.bear,
                _ => COLOR_AMBER,
            };
            let dir = if chart.precursor_direction > 0 { "PRE ▲" } else if chart.precursor_direction < 0 { "PRE ▼" } else { "PRE ?" };
            draw_pill(&painter, gauge_y, dir, chart.precursor_score, pr_color);

            // Subtle pulse
            let pulse = ((ctx.input(|i| i.time) * 2.5).sin() * 0.3 + 0.7) as f32;
            painter.rect_stroke(
                egui::Rect::from_min_size(egui::pos2(gauge_x - 1.0, gauge_y - 1.0), egui::vec2(pill_w + 2.0, pill_h + 2.0)),
                pill_r, egui::Stroke::new(1.0, color_alpha(pr_color, (pulse * 80.0) as u8)), egui::StrokeKind::Outside,
            );
            ctx.request_repaint();
        }
    }

    // ── VIX Expiry Alert Card (bottom-right when active) ────────────────
    if chart.show_vix_alert && chart.vix_expiry_active && chart.vix_expiry_days <= 5 {
        let card_w = 240.0;
        let card_h = 120.0;
        let card_x = rect.right() - card_w - 8.0;
        let card_y = rect.bottom() - card_h - 24.0;
        let card_rect = egui::Rect::from_min_size(egui::pos2(card_x, card_y), egui::vec2(card_w, card_h));

        // Card background
        let bg = color_alpha(t.toolbar_bg, 230);
        painter.rect_filled(card_rect, 6.0, bg);

        // Top accent — amber warning stripe
        let accent = COLOR_AMBER;
        painter.rect_filled(
            egui::Rect::from_min_size(egui::pos2(card_x, card_y), egui::vec2(card_w, 3.0)),
            egui::Rounding { nw: 6, ne: 6, sw: 0, se: 0 }, accent,
        );

        let text_x = card_x + 10.0;
        let dim = t.dim;
        let bright = t.text;

        // Title
        painter.text(egui::pos2(text_x, card_y + 12.0), egui::Align2::LEFT_CENTER,
            format!("VIX EXPIRY — {} days ({})", chart.vix_expiry_days, chart.vix_expiry_date),
            egui::FontId::monospace(9.5), accent);

        // VIX spot vs future
        let y = card_y + 28.0;
        painter.text(egui::pos2(text_x, y), egui::Align2::LEFT_CENTER,
            format!("VIX spot:      {:.1}", chart.vix_spot),
            mono_xs(), t.bear);
        painter.text(egui::pos2(text_x, y + 13.0), egui::Align2::LEFT_CENTER,
            format!("Expiring fut:  {:.1}  ← settlement target", chart.vix_expiring_future),
            mono_xs(), t.bull);
        painter.text(egui::pos2(text_x, y + 26.0), egui::Align2::LEFT_CENTER,
            format!("Realized vol:  {:.1}%", chart.vix_realized_vol),
            mono_xs(), dim);
        painter.text(egui::pos2(text_x, y + 39.0), egui::Align2::LEFT_CENTER,
            format!("Gap:           {:.1}%  {}", chart.vix_gap_pct,
                if chart.vix_gap_pct > 25.0 { "EXTREME" } else if chart.vix_gap_pct > 15.0 { "ELEVATED" } else { "" }),
            mono_xs(), if chart.vix_gap_pct > 25.0 { accent } else { bright });

        // Signal line
        let signal_text = if chart.vix_gap_pct > 20.0 {
            "SIGNAL: Mean reversion HIGH → bullish SPY"
        } else if chart.vix_gap_pct > 10.0 {
            "SIGNAL: Moderate convergence pressure"
        } else {
            "SIGNAL: VIX near fair value"
        };
        let signal_color = if chart.vix_gap_pct > 20.0 { t.bull } else { dim };
        painter.text(egui::pos2(text_x, y + 56.0), egui::Align2::LEFT_CENTER,
            signal_text, mono_2xs(), signal_color);

        // Convergence pressure bar
        let bar_y = y + 70.0;
        let bar_w = card_w - 20.0;
        painter.rect_filled(
            egui::Rect::from_min_size(egui::pos2(text_x, bar_y), egui::vec2(bar_w, 8.0)),
            4.0, color_alpha(t.toolbar_bg, 180),
        );
        let fill = bar_w * (chart.vix_convergence_score / 100.0).min(1.0);
        let bar_color = if chart.vix_convergence_score > 70.0 { t.bull }
            else if chart.vix_convergence_score > 40.0 { accent }
            else { dim };
        painter.rect_filled(
            egui::Rect::from_min_size(egui::pos2(text_x, bar_y), egui::vec2(fill, 8.0)),
            4.0, color_alpha(bar_color, 200),
        );
        painter.text(egui::pos2(text_x + bar_w + 4.0, bar_y + 4.0), egui::Align2::LEFT_CENTER,
            format!("{:.0}", chart.vix_convergence_score), mono_2xs(), bar_color);

        // Subtle border
        painter.rect_stroke(card_rect, 6.0,
            egui::Stroke::new(1.0, color_alpha(accent, 40)), egui::StrokeKind::Outside);
    }
}
