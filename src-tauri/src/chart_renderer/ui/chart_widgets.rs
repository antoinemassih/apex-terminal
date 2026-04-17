//! Chart Widgets — floating info cards rendered on the chart canvas.
//! Premium infographic-style gauges and data visualizations.
//! Think Bloomberg terminal meets Apple Watch complications.

use egui::{self, Color32, Stroke};
use super::style::*;
use super::super::gpu::*;
use crate::chart_renderer::ChartWidgetKind;
use std::f32::consts::PI;

/// Render all visible widgets for a chart pane.
pub(crate) fn draw_widgets(
    ui: &mut egui::Ui,
    chart: &mut Chart,
    rect: egui::Rect,
    t: &Theme,
) {
    let painter = ui.painter_at(rect);

    // Compute live data from bars once
    let wd = WidgetData::from_chart(chart);

    for wi in 0..chart.chart_widgets.len() {
        let w = &chart.chart_widgets[wi];
        if !w.visible { continue; }

        let abs_x = rect.left() + w.x * rect.width();
        let abs_y = rect.top() + w.y * rect.height();
        let card_w = w.w;
        let card_h = if w.collapsed { 26.0 } else { w.h };
        let card_rect = egui::Rect::from_min_size(egui::pos2(abs_x, abs_y), egui::vec2(card_w, card_h));

        if !rect.intersects(card_rect) { continue; }

        // ── Drop shadow ──
        let sh1 = card_rect.translate(egui::vec2(0.0, 3.0));
        painter.rect_filled(sh1.expand(2.0), RADIUS_LG + 2.0,
            Color32::from_rgba_unmultiplied(0, 0, 0, 30));
        let sh2 = card_rect.translate(egui::vec2(0.0, 1.5));
        painter.rect_filled(sh2.expand(1.0), RADIUS_LG + 1.0,
            Color32::from_rgba_unmultiplied(0, 0, 0, 18));

        // ── Card background — dark glass ──
        let bg = Color32::from_rgba_unmultiplied(
            t.toolbar_bg.r().saturating_add(4),
            t.toolbar_bg.g().saturating_add(4),
            t.toolbar_bg.b().saturating_add(6), 230);
        painter.rect_filled(card_rect, RADIUS_LG, bg);

        // ── Top bevel highlight ──
        let bevel = egui::Rect::from_min_max(card_rect.min,
            egui::pos2(card_rect.right(), card_rect.top() + 1.0));
        painter.rect_filled(bevel,
            egui::CornerRadius { nw: RADIUS_LG as u8, ne: RADIUS_LG as u8, sw: 0, se: 0 },
            Color32::from_rgba_unmultiplied(255, 255, 255, 10));

        // ── Border ──
        painter.rect_stroke(card_rect, RADIUS_LG,
            Stroke::new(STROKE_THIN, color_alpha(t.toolbar_border, ALPHA_LINE)),
            egui::StrokeKind::Outside);

        // ── Title bar ──
        let title_h = 24.0;
        let title_rect = egui::Rect::from_min_size(card_rect.min, egui::vec2(card_w, title_h));
        let kind = w.kind;

        painter.text(
            egui::pos2(title_rect.left() + 10.0, title_rect.center().y),
            egui::Align2::LEFT_CENTER, kind.icon(),
            egui::FontId::proportional(FONT_MD), t.accent);
        painter.text(
            egui::pos2(title_rect.left() + 24.0, title_rect.center().y),
            egui::Align2::LEFT_CENTER, kind.label(),
            egui::FontId::monospace(FONT_SM), TEXT_PRIMARY);

        // Collapse chevron
        let chev = if w.collapsed { "\u{25B6}" } else { "\u{25BC}" }; // ▶ ▼
        painter.text(egui::pos2(title_rect.right() - 12.0, title_rect.center().y),
            egui::Align2::CENTER_CENTER, chev,
            egui::FontId::proportional(6.0), t.dim.gamma_multiply(0.4));

        // ── Widget body ──
        if !w.collapsed {
            // Separator under title
            painter.line_segment(
                [egui::pos2(card_rect.left() + 8.0, card_rect.top() + title_h),
                 egui::pos2(card_rect.right() - 8.0, card_rect.top() + title_h)],
                Stroke::new(STROKE_HAIR, color_alpha(t.toolbar_border, ALPHA_MUTED)));

            let body = egui::Rect::from_min_size(
                egui::pos2(card_rect.left(), card_rect.top() + title_h + 2.0),
                egui::vec2(card_w, card_h - title_h - 2.0));

            match kind {
                ChartWidgetKind::TrendStrength => draw_trend_gauge(&painter, body, &wd, t),
                ChartWidgetKind::Momentum      => draw_momentum_gauge(&painter, body, &wd, t),
                ChartWidgetKind::Volatility    => draw_volatility_widget(&painter, body, &wd, t),
                ChartWidgetKind::VolumeProfile => draw_volume_profile(&painter, body, &wd, t),
                ChartWidgetKind::SessionTimer  => draw_session_timer(&painter, body, t),
                ChartWidgetKind::KeyLevels     => draw_key_levels(&painter, body, &wd, t),
                ChartWidgetKind::OptionGreeks  => draw_option_greeks(&painter, body, t),
                ChartWidgetKind::RiskReward    => draw_risk_reward(&painter, body, &wd, t),
                ChartWidgetKind::MarketBreadth => draw_market_breadth(&painter, body, t),
                ChartWidgetKind::Custom        => draw_custom(&painter, body, t),
            }
        }

        // ── Interaction ──
        let drag_resp = ui.interact(title_rect,
            egui::Id::new(("widget_drag", wi)), egui::Sense::click_and_drag());

        if drag_resp.dragged_by(egui::PointerButton::Primary) {
            let d = drag_resp.drag_delta();
            let nx = chart.chart_widgets[wi].x + d.x / rect.width();
            let ny = chart.chart_widgets[wi].y + d.y / rect.height();
            chart.chart_widgets[wi].x = nx.clamp(0.0, 0.95);
            chart.chart_widgets[wi].y = ny.clamp(0.0, 0.95);
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        } else if drag_resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
        }

        if drag_resp.clicked() {
            chart.chart_widgets[wi].collapsed = !chart.chart_widgets[wi].collapsed;
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Live data extraction
// ═══════════════════════════════════════════════════════════════════════════════

struct WidgetData {
    trend_score: f32,
    trend_dir: i8,
    trend_regime: String,
    rsi: f32,          // 0-100
    momentum: f32,     // -100..100
    atr: f32,
    atr_pct: f32,      // atr as % of price
    vol_ratio: f32,    // current vol / avg vol
    last_close: f32,
    prev_close: f32,
    day_change_pct: f32,
    vol_bars: [f32; 12], // normalized volume distribution
    price_levels: [(f32, &'static str); 5], // pivot points
}

impl WidgetData {
    fn from_chart(chart: &Chart) -> Self {
        let bars = &chart.bars;
        let n = bars.len();
        let last_close = if n > 0 { bars[n - 1].close } else { 0.0 };
        let prev_close = if n > 1 { bars[n - 2].close } else { last_close };
        let day_change_pct = if prev_close > 0.0 { (last_close - prev_close) / prev_close * 100.0 } else { 0.0 };

        // Compute RSI from last 14 bars
        let rsi = compute_rsi(bars, 14);

        // Compute momentum as rate of change over 10 bars
        let momentum = if n > 10 && bars[n - 11].close > 0.0 {
            (last_close - bars[n - 11].close) / bars[n - 11].close * 100.0
        } else { 0.0 };

        // ATR from last 14 bars
        let atr = compute_atr(bars, 14);
        let atr_pct = if last_close > 0.0 { atr / last_close * 100.0 } else { 0.0 };

        // Volume ratio — last bar vs 20-bar avg
        let vol_ratio = if n > 20 {
            let avg: f32 = bars[n-21..n-1].iter().map(|b| b.volume).sum::<f32>() / 20.0;
            if avg > 0.0 { bars[n - 1].volume / avg } else { 1.0 }
        } else { 1.0 };

        // Volume distribution (last 12 bars, normalized to 0-1)
        let mut vol_bars = [0.0f32; 12];
        if n >= 12 {
            let start = n - 12;
            let max_v = bars[start..n].iter().map(|b| b.volume).fold(0.0f32, f32::max).max(1.0);
            for i in 0..12 {
                vol_bars[i] = bars[start + i].volume / max_v;
            }
        }

        // Pivot points from high/low/close
        let (h, l) = if n > 0 {
            let recent = &bars[n.saturating_sub(20)..n];
            let hi = recent.iter().map(|b| b.high).fold(f32::NEG_INFINITY, f32::max);
            let lo = recent.iter().map(|b| b.low).fold(f32::INFINITY, f32::min);
            (hi, lo)
        } else { (100.0, 90.0) };
        let pp = (h + l + last_close) / 3.0;
        let r1 = 2.0 * pp - l;
        let s1 = 2.0 * pp - h;
        let r2 = pp + (h - l);
        let s2 = pp - (h - l);

        WidgetData {
            trend_score: chart.trend_health_score,
            trend_dir: chart.trend_health_direction,
            trend_regime: chart.trend_health_regime.clone(),
            rsi, momentum, atr, atr_pct, vol_ratio,
            last_close, prev_close, day_change_pct, vol_bars,
            price_levels: [(r2, "R2"), (r1, "R1"), (pp, "PP"), (s1, "S1"), (s2, "S2")],
        }
    }
}

fn compute_rsi(bars: &[crate::chart_renderer::types::Bar], period: usize) -> f32 {
    let n = bars.len();
    if n < period + 1 { return 50.0; }
    let mut gain_sum = 0.0f32;
    let mut loss_sum = 0.0f32;
    for i in (n - period)..n {
        let diff = bars[i].close - bars[i - 1].close;
        if diff > 0.0 { gain_sum += diff; } else { loss_sum += -diff; }
    }
    let avg_gain = gain_sum / period as f32;
    let avg_loss = loss_sum / period as f32;
    if avg_loss < 0.0001 { return 100.0; }
    let rs = avg_gain / avg_loss;
    100.0 - (100.0 / (1.0 + rs))
}

fn compute_atr(bars: &[crate::chart_renderer::types::Bar], period: usize) -> f32 {
    let n = bars.len();
    if n < period + 1 { return 0.0; }
    let mut sum = 0.0f32;
    for i in (n - period)..n {
        let tr = (bars[i].high - bars[i].low)
            .max((bars[i].high - bars[i - 1].close).abs())
            .max((bars[i].low - bars[i - 1].close).abs());
        sum += tr;
    }
    sum / period as f32
}

// ═══════════════════════════════════════════════════════════════════════════════
// Shared painting helpers
// ═══════════════════════════════════════════════════════════════════════════════

/// Draw an arc from angle `start` to `end` (radians, 0 = right, counter-clockwise).
fn draw_arc(p: &egui::Painter, center: egui::Pos2, radius: f32, start: f32, end: f32,
            stroke: Stroke, segments: usize) {
    if segments < 2 { return; }
    let step = (end - start) / segments as f32;
    let points: Vec<egui::Pos2> = (0..=segments)
        .map(|i| {
            let a = start + step * i as f32;
            egui::pos2(center.x + radius * a.cos(), center.y - radius * a.sin())
        })
        .collect();
    for pair in points.windows(2) {
        p.line_segment([pair[0], pair[1]], stroke);
    }
}

/// Blend two colors by t (0.0 = a, 1.0 = b).
fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let inv = 1.0 - t;
    Color32::from_rgb(
        (a.r() as f32 * inv + b.r() as f32 * t) as u8,
        (a.g() as f32 * inv + b.g() as f32 * t) as u8,
        (a.b() as f32 * inv + b.b() as f32 * t) as u8,
    )
}

/// Hero number — the big featured value in a widget.
fn hero_number(p: &egui::Painter, pos: egui::Pos2, text: &str, color: Color32) {
    // Glow behind the number
    p.text(pos + egui::vec2(0.0, 0.5), egui::Align2::CENTER_CENTER,
        text, egui::FontId::monospace(22.0),
        Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 25));
    // The number itself
    p.text(pos, egui::Align2::CENTER_CENTER,
        text, egui::FontId::monospace(22.0), color);
}

/// Sub-label under hero number.
fn sub_label(p: &egui::Painter, pos: egui::Pos2, text: &str, color: Color32) {
    p.text(pos, egui::Align2::CENTER_CENTER,
        text, egui::FontId::monospace(FONT_XS),
        Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 140));
}

// ═══════════════════════════════════════════════════════════════════════════════
// Widget renderers
// ═══════════════════════════════════════════════════════════════════════════════

fn draw_trend_gauge(p: &egui::Painter, body: egui::Rect, wd: &WidgetData, t: &Theme) {
    let cx = body.center().x;
    let score = if wd.trend_score > 0.0 { wd.trend_score } else { 72.0 }; // demo fallback

    // Color gradient: bear → amber → bull
    let color = if score > 66.0 {
        lerp_color(Color32::from_rgb(255, 191, 0), t.bull, (score - 66.0) / 34.0)
    } else if score > 33.0 {
        lerp_color(t.bear, Color32::from_rgb(255, 191, 0), (score - 33.0) / 33.0)
    } else {
        t.bear
    };

    // Arc gauge — 180° sweep from left to right
    let gauge_cy = body.top() + 38.0;
    let r = 28.0;

    // Background track
    draw_arc(p, egui::pos2(cx, gauge_cy), r, 0.0, PI, Stroke::new(3.0,
        color_alpha(t.toolbar_border, ALPHA_MUTED)), 40);

    // Filled arc proportional to score
    let sweep = (score / 100.0) * PI;
    draw_arc(p, egui::pos2(cx, gauge_cy), r, PI - sweep, PI, Stroke::new(3.5, color), 30);

    // Tick marks at 0, 25, 50, 75, 100
    for pct in [0.0, 0.25, 0.5, 0.75, 1.0] {
        let a = PI - pct * PI;
        let inner = r - 5.0;
        let outer = r + 2.0;
        let p1 = egui::pos2(cx + inner * a.cos(), gauge_cy - inner * a.sin());
        let p2 = egui::pos2(cx + outer * a.cos(), gauge_cy - outer * a.sin());
        p.line_segment([p1, p2], Stroke::new(STROKE_THIN, color_alpha(t.dim, ALPHA_DIM)));
    }

    // Needle
    let needle_a = PI - (score / 100.0) * PI;
    let needle_end = egui::pos2(cx + (r - 8.0) * needle_a.cos(), gauge_cy - (r - 8.0) * needle_a.sin());
    p.line_segment([egui::pos2(cx, gauge_cy), needle_end],
        Stroke::new(1.5, Color32::WHITE));
    p.circle_filled(egui::pos2(cx, gauge_cy), 3.0, Color32::WHITE);

    // Hero score
    hero_number(p, egui::pos2(cx, gauge_cy + 14.0), &format!("{:.0}", score), color);

    // Regime label
    let regime = if wd.trend_regime.is_empty() {
        if score > 66.0 { "STRONG" } else if score > 33.0 { "MIXED" } else { "WEAK" }
    } else { &wd.trend_regime };
    sub_label(p, egui::pos2(cx, gauge_cy + 32.0), regime, color);

    // Direction arrow
    let dir_icon = match wd.trend_dir { d if d > 0 => "\u{25B2}", d if d < 0 => "\u{25BC}", _ => "\u{25C6}" }; // ▲ ▼ ◆
    let dir_col = match wd.trend_dir { d if d > 0 => t.bull, d if d < 0 => t.bear, _ => t.dim };
    p.text(egui::pos2(cx + 30.0, gauge_cy + 14.0), egui::Align2::LEFT_CENTER,
        dir_icon, egui::FontId::proportional(FONT_SM), dir_col);
}

fn draw_momentum_gauge(p: &egui::Painter, body: egui::Rect, wd: &WidgetData, t: &Theme) {
    let cx = body.center().x;
    let rsi = wd.rsi;
    let mom = wd.momentum;

    // RSI zone coloring
    let rsi_color = if rsi > 70.0 { t.bull }
        else if rsi < 30.0 { t.bear }
        else { Color32::from_rgb(255, 191, 0) };

    // RSI arc gauge — same style as trend
    let gauge_cy = body.top() + 36.0;
    let r = 26.0;

    // Track
    draw_arc(p, egui::pos2(cx, gauge_cy), r, 0.0, PI,
        Stroke::new(2.5, color_alpha(t.toolbar_border, ALPHA_MUTED)), 40);

    // Zone fills: red zone (0-30), yellow zone (30-70), green zone (70-100)
    draw_arc(p, egui::pos2(cx, gauge_cy), r, PI * 0.7, PI,
        Stroke::new(2.5, color_alpha(t.bear, ALPHA_MUTED)), 10);
    draw_arc(p, egui::pos2(cx, gauge_cy), r, 0.0, PI * 0.3,
        Stroke::new(2.5, color_alpha(t.bull, ALPHA_MUTED)), 10);

    // Filled arc
    let sweep = (rsi / 100.0) * PI;
    draw_arc(p, egui::pos2(cx, gauge_cy), r, PI - sweep, PI,
        Stroke::new(3.0, rsi_color), 30);

    // Needle
    let a = PI - (rsi / 100.0) * PI;
    let ne = egui::pos2(cx + (r - 7.0) * a.cos(), gauge_cy - (r - 7.0) * a.sin());
    p.line_segment([egui::pos2(cx, gauge_cy), ne], Stroke::new(1.5, Color32::WHITE));
    p.circle_filled(egui::pos2(cx, gauge_cy), 2.5, Color32::WHITE);

    // Hero RSI value
    hero_number(p, egui::pos2(cx, gauge_cy + 12.0), &format!("{:.0}", rsi), rsi_color);

    // Zone label
    let zone = if rsi > 70.0 { "OVERBOUGHT" } else if rsi < 30.0 { "OVERSOLD" } else { "NEUTRAL" };
    sub_label(p, egui::pos2(cx, gauge_cy + 30.0), zone, rsi_color);

    // Momentum ROC — small indicator bottom-right
    let mom_col = if mom > 0.0 { t.bull } else { t.bear };
    let mom_sign = if mom > 0.0 { "+" } else { "" };
    p.text(egui::pos2(body.right() - 8.0, body.bottom() - 8.0), egui::Align2::RIGHT_CENTER,
        &format!("{}{:.1}%", mom_sign, mom), egui::FontId::monospace(FONT_XS), mom_col);
    p.text(egui::pos2(body.left() + 8.0, body.bottom() - 8.0), egui::Align2::LEFT_CENTER,
        "ROC", egui::FontId::monospace(7.0), t.dim.gamma_multiply(0.4));
}

fn draw_volatility_widget(p: &egui::Painter, body: egui::Rect, wd: &WidgetData, t: &Theme) {
    let cx = body.center().x;

    // ATR value — big hero
    let atr_str = if wd.atr > 1.0 { format!("{:.2}", wd.atr) } else { format!("{:.4}", wd.atr) };
    hero_number(p, egui::pos2(cx, body.top() + 18.0), &atr_str, t.accent);
    sub_label(p, egui::pos2(cx, body.top() + 36.0), "ATR (14)", t.dim);

    // ATR as % of price — horizontal bar
    let bar_y = body.top() + 50.0;
    let bar_x = body.left() + 12.0;
    let bar_w = body.width() - 24.0;
    let bar_h = 6.0;

    // Track
    p.rect_filled(egui::Rect::from_min_size(egui::pos2(bar_x, bar_y), egui::vec2(bar_w, bar_h)),
        3.0, color_alpha(t.toolbar_border, ALPHA_MUTED));

    // Fill — clamp to 0-5% range for visualization
    let pct = (wd.atr_pct / 5.0).clamp(0.0, 1.0);
    let vol_color = if wd.atr_pct > 3.0 { t.bear }
        else if wd.atr_pct > 1.5 { Color32::from_rgb(255, 191, 0) }
        else { t.bull };
    p.rect_filled(egui::Rect::from_min_size(egui::pos2(bar_x, bar_y), egui::vec2(bar_w * pct, bar_h)),
        3.0, vol_color);

    // % label
    p.text(egui::pos2(cx, bar_y + 14.0), egui::Align2::CENTER_CENTER,
        &format!("{:.2}% of price", wd.atr_pct), egui::FontId::monospace(FONT_XS), vol_color);

    // Volume ratio spark
    let vr_y = body.bottom() - 14.0;
    let vr_col = if wd.vol_ratio > 1.5 { t.bull } else if wd.vol_ratio > 0.7 { t.dim } else { t.bear };
    p.text(egui::pos2(body.left() + 12.0, vr_y), egui::Align2::LEFT_CENTER,
        "RVOL", egui::FontId::monospace(7.0), t.dim.gamma_multiply(0.4));
    p.text(egui::pos2(body.right() - 12.0, vr_y), egui::Align2::RIGHT_CENTER,
        &format!("{:.1}x", wd.vol_ratio), egui::FontId::monospace(FONT_SM), vr_col);
}

fn draw_volume_profile(p: &egui::Painter, body: egui::Rect, wd: &WidgetData, t: &Theme) {
    let bar_x = body.left() + 10.0;
    let max_w = body.width() - 20.0;
    let n = wd.vol_bars.len();
    let total_h = body.height() - 12.0;
    let bar_h = (total_h / n as f32).min(12.0);
    let gap = ((total_h - bar_h * n as f32) / (n as f32 - 1.0).max(1.0)).max(1.0);

    // Find max for the POC (point of control) highlight
    let max_idx = wd.vol_bars.iter()
        .enumerate()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(i, _)| i).unwrap_or(0);

    for i in 0..n {
        let y = body.top() + 6.0 + i as f32 * (bar_h + gap);
        let w = max_w * wd.vol_bars[i].max(0.03); // min visible width
        let is_poc = i == max_idx;

        // Bar color gradient — POC gets accent, others get a blue-to-purple gradient
        let color = if is_poc {
            t.accent
        } else {
            let t_val = i as f32 / n as f32;
            lerp_color(
                Color32::from_rgb(80, 120, 200),  // top: blue
                Color32::from_rgb(140, 80, 180),   // bottom: purple
                t_val,
            )
        };

        let alpha = if is_poc { ALPHA_STRONG } else { ALPHA_DIM };
        let bar_rect = egui::Rect::from_min_size(egui::pos2(bar_x, y), egui::vec2(w, bar_h));
        p.rect_filled(bar_rect, 2.0, color_alpha(color, alpha));

        // Glow on POC
        if is_poc {
            p.rect_filled(bar_rect.expand(1.0), 3.0,
                Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 20));
            p.text(egui::pos2(bar_x + w + 4.0, y + bar_h / 2.0),
                egui::Align2::LEFT_CENTER, "POC",
                egui::FontId::monospace(7.0), t.accent);
        }
    }
}

fn draw_session_timer(p: &egui::Painter, body: egui::Rect, t: &Theme) {
    let cx = body.center().x;

    // Get current time (UTC) and compute time to 4:00 PM ET (20:00 UTC, approximate)
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs();
    let day_secs = (now % 86400) as i64;
    let close_utc = 20 * 3600; // 4 PM ET ≈ 20:00 UTC (no DST adjust)
    let remaining = if day_secs < close_utc { close_utc - day_secs } else { 86400 - day_secs + close_utc };

    let h = remaining / 3600;
    let m = (remaining % 3600) / 60;
    let s = remaining % 60;

    // Circular progress ring
    let ring_cy = body.top() + 22.0;
    let ring_r = 16.0;
    let total_session = 6.5 * 3600.0; // 6.5 hours
    let elapsed_frac = 1.0 - (remaining as f32 / total_session).clamp(0.0, 1.0);

    // Background ring
    draw_arc(p, egui::pos2(cx, ring_cy), ring_r, 0.0, 2.0 * PI,
        Stroke::new(2.0, color_alpha(t.toolbar_border, ALPHA_MUTED)), 60);

    // Progress ring
    let progress_color = if elapsed_frac > 0.9 { t.bear }
        else if elapsed_frac > 0.7 { Color32::from_rgb(255, 191, 0) }
        else { t.accent };
    let sweep = elapsed_frac * 2.0 * PI;
    draw_arc(p, egui::pos2(cx, ring_cy), ring_r,
        PI / 2.0, PI / 2.0 - sweep,
        Stroke::new(2.5, progress_color), 40);

    // Time display
    let time_str = format!("{:02}:{:02}:{:02}", h, m, s);
    hero_number(p, egui::pos2(cx, body.top() + 48.0), &time_str, TEXT_PRIMARY);
    sub_label(p, egui::pos2(cx, body.top() + 66.0), "TO CLOSE", t.dim);
}

fn draw_key_levels(p: &egui::Painter, body: egui::Rect, wd: &WidgetData, t: &Theme) {
    let left = body.left() + 10.0;
    let right = body.right() - 10.0;
    let row_h = (body.height() - 8.0) / 5.0;

    for (i, (price, label)) in wd.price_levels.iter().enumerate() {
        let y = body.top() + 4.0 + i as f32 * row_h + row_h / 2.0;
        let is_pp = *label == "PP";
        let is_r = label.starts_with('R');

        let level_color = if is_pp { t.accent }
            else if is_r { Color32::from_rgb(220, 80, 80) }
            else { Color32::from_rgb(80, 180, 120) };

        // Label badge
        let badge_w = 24.0;
        let badge_rect = egui::Rect::from_min_size(
            egui::pos2(left, y - 8.0), egui::vec2(badge_w, 16.0));
        p.rect_filled(badge_rect, 3.0, color_alpha(level_color, ALPHA_TINT));
        p.text(badge_rect.center(), egui::Align2::CENTER_CENTER,
            *label, egui::FontId::monospace(FONT_XS), level_color);

        // Dashed line
        let line_x_start = left + badge_w + 6.0;
        let line_x_end = right - 50.0;
        let dash_len = 4.0;
        let gap_len = 3.0;
        let mut x = line_x_start;
        while x < line_x_end {
            let end = (x + dash_len).min(line_x_end);
            p.line_segment(
                [egui::pos2(x, y), egui::pos2(end, y)],
                Stroke::new(STROKE_HAIR, color_alpha(level_color, ALPHA_MUTED)));
            x += dash_len + gap_len;
        }

        // Price value
        let font_size = if is_pp { FONT_LG } else { FONT_SM };
        p.text(egui::pos2(right, y), egui::Align2::RIGHT_CENTER,
            &format!("{:.2}", price), egui::FontId::monospace(font_size), level_color);

        // Distance from current price
        if wd.last_close > 0.0 {
            let dist = (price - wd.last_close) / wd.last_close * 100.0;
            let dist_col = if dist.abs() < 0.5 { t.accent } else { t.dim.gamma_multiply(0.4) };
            p.text(egui::pos2(right, y + 9.0), egui::Align2::RIGHT_CENTER,
                &format!("{:+.1}%", dist), egui::FontId::monospace(7.0), dist_col);
        }
    }
}

fn draw_option_greeks(p: &egui::Painter, body: egui::Rect, t: &Theme) {
    let greeks: [(&str, f32, Color32); 4] = [
        ("\u{0394} Delta", 0.45, Color32::from_rgb(100, 200, 255)),   // light blue
        ("\u{0393} Gamma", 0.032, Color32::from_rgb(180, 130, 255)),  // purple
        ("\u{0398} Theta", -0.12, Color32::from_rgb(255, 140, 100)),  // coral
        ("\u{03BD} Vega",  0.085, Color32::from_rgb(100, 230, 180)),  // mint
    ];

    let row_h = (body.height() - 8.0) / 4.0;
    let left = body.left() + 10.0;
    let right = body.right() - 10.0;
    let bar_max_w = body.width() * 0.35;

    for (i, (name, val, color)) in greeks.iter().enumerate() {
        let y = body.top() + 4.0 + i as f32 * row_h + row_h / 2.0;

        // Greek name
        p.text(egui::pos2(left, y), egui::Align2::LEFT_CENTER,
            *name, egui::FontId::monospace(FONT_SM), *color);

        // Value
        let val_str = if val.abs() < 0.01 { format!("{:.3}", val) } else { format!("{:.2}", val) };
        p.text(egui::pos2(right, y), egui::Align2::RIGHT_CENTER,
            &val_str, egui::FontId::monospace(FONT_LG), TEXT_PRIMARY);

        // Mini bar visualization
        let bar_x = left + 64.0;
        let bar_w = (val.abs() * bar_max_w * 2.0).min(bar_max_w);
        let bar_rect = egui::Rect::from_min_size(
            egui::pos2(bar_x, y - 3.0), egui::vec2(bar_w, 6.0));
        p.rect_filled(bar_rect, 2.0, color_alpha(*color, ALPHA_DIM));
    }
}

fn draw_risk_reward(p: &egui::Painter, body: egui::Rect, wd: &WidgetData, t: &Theme) {
    let cx = body.center().x;

    // Simulated R:R (would come from active play/position)
    let risk = 1.0f32;
    let reward = 2.8f32;

    let total = risk + reward;
    let bar_w = body.width() - 24.0;
    let bar_x = body.left() + 12.0;
    let bar_y = body.top() + 12.0;
    let bar_h = 10.0;

    // Risk portion (red)
    let risk_w = bar_w * (risk / total);
    p.rect_filled(egui::Rect::from_min_size(egui::pos2(bar_x, bar_y), egui::vec2(risk_w, bar_h)),
        egui::CornerRadius { nw: 4, sw: 4, ne: 0, se: 0 }, color_alpha(t.bear, ALPHA_STRONG));

    // Reward portion (green)
    p.rect_filled(egui::Rect::from_min_size(egui::pos2(bar_x + risk_w, bar_y), egui::vec2(bar_w - risk_w, bar_h)),
        egui::CornerRadius { nw: 0, sw: 0, ne: 4, se: 4 }, color_alpha(t.bull, ALPHA_STRONG));

    // Entry marker
    p.circle_filled(egui::pos2(bar_x + risk_w, bar_y + bar_h / 2.0), 4.0, Color32::WHITE);

    // R:R hero number
    let rr_str = format!("{:.1} : 1", reward);
    let rr_col = if reward >= 2.0 { t.bull } else if reward >= 1.0 { Color32::from_rgb(255, 191, 0) } else { t.bear };
    hero_number(p, egui::pos2(cx, body.top() + 40.0), &rr_str, rr_col);

    // Labels
    p.text(egui::pos2(bar_x, bar_y + bar_h + 6.0), egui::Align2::LEFT_TOP,
        "RISK", egui::FontId::monospace(7.0), t.bear.gamma_multiply(0.7));
    p.text(egui::pos2(bar_x + bar_w, bar_y + bar_h + 6.0), egui::Align2::RIGHT_TOP,
        "REWARD", egui::FontId::monospace(7.0), t.bull.gamma_multiply(0.7));
    sub_label(p, egui::pos2(cx, body.top() + 58.0), "RISK / REWARD", t.dim);
}

fn draw_market_breadth(p: &egui::Painter, body: egui::Rect, t: &Theme) {
    // Demo data — would come from real market data feed
    let metrics: [(&str, &str, Color32, f32); 4] = [
        ("ADV / DEC", "1,842 / 1,156", t.bull, 0.614),     // advance ratio
        ("NEW HI", "48", Color32::from_rgb(100, 200, 255), 0.4),
        ("NEW LO", "12", Color32::from_rgb(255, 140, 100), 0.1),
        ("VIX", "18.5", Color32::from_rgb(255, 191, 0), 0.37),
    ];

    let row_h = (body.height() - 8.0) / 4.0;
    let left = body.left() + 10.0;
    let right = body.right() - 10.0;

    for (i, (label, value, color, bar_pct)) in metrics.iter().enumerate() {
        let y = body.top() + 4.0 + i as f32 * row_h;

        // Label
        p.text(egui::pos2(left, y + 5.0), egui::Align2::LEFT_TOP,
            *label, egui::FontId::monospace(7.0),
            Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 120));

        // Value
        p.text(egui::pos2(right, y + 5.0), egui::Align2::RIGHT_TOP,
            *value, egui::FontId::monospace(FONT_SM), *color);

        // Mini bar underneath
        let bar_y = y + 16.0;
        let bar_w = body.width() - 20.0;
        p.rect_filled(egui::Rect::from_min_size(egui::pos2(left, bar_y), egui::vec2(bar_w, 3.0)),
            1.0, color_alpha(t.toolbar_border, ALPHA_FAINT));
        p.rect_filled(egui::Rect::from_min_size(egui::pos2(left, bar_y), egui::vec2(bar_w * bar_pct, 3.0)),
            1.0, color_alpha(*color, ALPHA_DIM));
    }
}

fn draw_custom(p: &egui::Painter, body: egui::Rect, t: &Theme) {
    let cx = body.center().x;
    let cy = body.center().y;
    p.text(egui::pos2(cx, cy - 6.0), egui::Align2::CENTER_CENTER,
        "\u{2699}", egui::FontId::proportional(20.0), t.dim.gamma_multiply(0.2));
    p.text(egui::pos2(cx, cy + 14.0), egui::Align2::CENTER_CENTER,
        "Drag to configure", egui::FontId::monospace(FONT_XS), t.dim.gamma_multiply(0.3));
}
