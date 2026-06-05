//! Overlay Kit — the shared, Sx-styled drawing primitives that on-chart overlay
//! widgets compose their bodies from, instead of each one hand-painting.
//!
//! Before this module these helpers were scattered through the 3,500-line
//! `chart_widgets.rs` and used inconsistently (16 `hero_number` calls, 17
//! `sub_label`, …). Centralising them here is step 1 of the overlay system:
//! a widget body should read as a few kit calls (`radial_gauge(...)`,
//! `progress_bar(...)`), with colours resolved from Sx tones so overlays match
//! the active theme automatically.
//!
//! All primitives are **painter-based** — they draw directly into the chart
//! canvas `Painter` at an absolute rect/pos, matching the overlay render path.

use egui::{self, Color32, Stroke};
use crate::ui_kit::sx::Tone;
use crate::chart_renderer::ui::style::*;
use crate::chart_renderer::gpu::Theme;

// ── Geometry ──────────────────────────────────────────────────────────────────

/// Stroke an arc from `start`→`end` radians around `center` (y-up convention).
pub(crate) fn draw_arc(
    p: &egui::Painter, center: egui::Pos2, radius: f32, start: f32, end: f32,
    stroke: Stroke, segments: usize,
) {
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

/// Linear RGB lerp `a`→`b` by `t` (0..1). Used for value-driven colour ramps.
#[allow(dead_code)] // kit primitive — callers migrate off chart_widgets' local copy
pub(crate) fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let inv = 1.0 - t;
    Color32::from_rgb(
        (a.r() as f32 * inv + b.r() as f32 * t) as u8,
        (a.g() as f32 * inv + b.g() as f32 * t) as u8,
        (a.b() as f32 * inv + b.b() as f32 * t) as u8,
    )
}

// ── Text ──────────────────────────────────────────────────────────────────────

/// Hero number — the focal large proportional value at the centre of a widget.
pub(crate) fn hero_number(p: &egui::Painter, pos: egui::Pos2, text: &str, color: Color32) {
    p.text(pos, egui::Align2::CENTER_CENTER, text,
        egui::FontId::proportional(font_display_md()), color);
}

/// Even larger hero for primary KPIs.
#[allow(dead_code)] // kit primitive — for the KPI-style overlays migrating next
pub(crate) fn hero_number_lg(p: &egui::Painter, pos: egui::Pos2, text: &str, color: Color32) {
    p.text(pos, egui::Align2::CENTER_CENTER, text,
        egui::FontId::proportional(font_display_lg()), color);
}

/// Small uppercase mono label — editorial caption under a hero value.
pub(crate) fn sub_label(p: &egui::Painter, pos: egui::Pos2, text: &str, color: Color32) {
    p.text(pos, egui::Align2::CENTER_CENTER, text, egui::FontId::monospace(FONT_XS),
        Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 170));
}

// ── Composite primitives (compose the above — what bodies should call) ────────

/// Donut ring gauge — thick value arc over a full-circle track.
pub(crate) fn donut_ring(
    p: &egui::Painter, center: egui::Pos2, radius: f32, thickness: f32,
    value: f32, max: f32, color: Color32, track_color: Color32,
) {
    let segs = 48;
    let tau = std::f32::consts::TAU;
    draw_arc(p, center, radius, 0.0, tau, egui::Stroke::new(thickness, track_color), segs);
    let frac = (value / max).clamp(0.0, 1.0);
    let start = -std::f32::consts::FRAC_PI_2;
    for i in 0..segs {
        let t0 = i as f32 / segs as f32;
        if t0 >= frac { break; }
        let t1 = ((i + 1) as f32 / segs as f32).min(frac);
        let a0 = start + t0 * tau;
        let a1 = start + t1 * tau;
        let p0 = egui::pos2(center.x + radius * a0.cos(), center.y + radius * a0.sin());
        let p1 = egui::pos2(center.x + radius * a1.cos(), center.y + radius * a1.sin());
        p.line_segment([p0, p1], egui::Stroke::new(thickness, color));
    }
}

/// **The dominant overlay shape**: a donut ring + centred hero number + an
/// uppercase caption below — one call for the trend / momentum / exit /
/// conviction-style gauges. `frac` is 0..1 (the ring fill); `color` is the
/// value tone; the track is the theme border tone.
pub(crate) fn radial_gauge(
    p: &egui::Painter, center: egui::Pos2, radius: f32,
    frac: f32, hero: &str, caption: &str, color: Color32, t: &Theme,
) {
    let track = tint(t, Tone::Border, alpha_muted());
    donut_ring(p, center, radius, 6.0, frac.clamp(0.0, 1.0), 1.0, color, track);
    hero_number(p, center, hero, color);
    sub_label(p, egui::pos2(center.x, center.y + radius + 12.0), caption, color);
}

/// Gauge variant with the value + caption **stacked inside** the ring (vs
/// [`radial_gauge`]'s caption below it) — a thicker 8px ring, `font_xl` value
/// and a tiny `FONT_2XS` caption. For the sentiment / liquidity-style gauges.
pub(crate) fn radial_gauge_stacked(
    p: &egui::Painter, center: egui::Pos2, radius: f32,
    frac: f32, value: &str, caption: &str, color: Color32, t: &Theme,
) {
    let track = tint(t, Tone::Border, alpha_muted());
    donut_ring(p, center, radius, 8.0, frac.clamp(0.0, 1.0), 1.0, color, track);
    p.text(egui::pos2(center.x, center.y - 4.0), egui::Align2::CENTER_CENTER,
        value, egui::FontId::proportional(font_xl()), color);
    p.text(egui::pos2(center.x, center.y + 14.0), egui::Align2::CENTER_CENTER,
        caption, egui::FontId::monospace(FONT_2XS), color);
}

/// Big value + caption (no ring), centred at `center` — the non-gauge "stat".
#[allow(dead_code)] // kit primitive — for the stat-style overlays migrating next
pub(crate) fn stat(p: &egui::Painter, center: egui::Pos2, value: &str, caption: &str, color: Color32) {
    hero_number(p, center, value, color);
    sub_label(p, egui::pos2(center.x, center.y + 22.0), caption, color);
}

/// A labelled metric row: dim `label` (left) + `value` (right) + a thin
/// progress bar below at `frac` 0..1. The breadth / greeks-style overlay row.
/// `rect.top()` is the text baseline; the bar sits ~11px under it.
pub(crate) fn metric_row(
    p: &egui::Painter, rect: egui::Rect, label: &str, value: &str,
    frac: f32, color: Color32, t: &Theme,
) {
    p.text(egui::pos2(rect.left(), rect.top()), egui::Align2::LEFT_TOP,
        label, egui::FontId::monospace(FONT_2XS),
        Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 120));
    p.text(egui::pos2(rect.right(), rect.top()), egui::Align2::RIGHT_TOP,
        value, egui::FontId::monospace(FONT_SM), color);
    let bar_y = rect.top() + 11.0;
    let track = egui::Rect::from_min_size(egui::pos2(rect.left(), bar_y), egui::vec2(rect.width(), 3.0));
    p.rect_filled(track, 1.0, tint(t, Tone::Border, alpha_faint()));
    let fw = rect.width() * frac.clamp(0.0, 1.0);
    if fw > 0.5 {
        let fill = egui::Rect::from_min_size(egui::pos2(rect.left(), bar_y), egui::vec2(fw, 3.0));
        p.rect_filled(fill, 1.0, color_alpha(color, alpha_dim()));
    }
}

// ── Card shell (the chrome every overlay widget shares) ───────────────────────

/// The shared overlay **card shell** — drop shadow, sentiment-tinted background,
/// a top bevel highlight and the border — painted *under* every widget body.
///
/// This centralises what used to be ~40 lines of inline magic-alpha painting in
/// `chart_widgets`, so all overlay cards share one shell and one set of Sx tones.
/// `sentiment` is the widget's data state in `-1..=1` (drives the subtle green↔red
/// background tint); `card` is the body rect (the header floats above it).
pub(crate) fn overlay_card_frame(
    p: &egui::Painter, card: egui::Rect, sentiment: f32, t: &Theme,
) {
    // Drop shadow — two stacked soft rects offset downward.
    p.rect_filled(card.translate(egui::vec2(0.0, 3.0)).expand(2.0),
        r_lg_cr(), color_alpha(t.shadow_color, 20));
    p.rect_filled(card.translate(egui::vec2(0.0, 1.5)).expand(1.0),
        r_lg_cr(), color_alpha(t.shadow_color, 10));

    // Sentiment-driven background tint — pastel on light themes, a faint shift of
    // the toolbar surface on dark themes so the card reads against the chart.
    let is_light = t.is_light();
    let (sr, sg, sb) = if is_light {
        match sentiment {
            s if s > 0.6  => (200, 235, 200),  // soft green
            s if s > 0.2  => (215, 235, 210),  // sage
            s if s > -0.2 => (238, 238, 234),  // warm neutral
            s if s > -0.6 => (240, 225, 195),  // warm amber
            _             => (240, 210, 205),  // soft rose
        }
    } else {
        match sentiment {
            s if s > 0.6  => (t.bull.r() / 3 + t.toolbar_bg.r() * 2 / 3, t.bull.g() / 3 + t.toolbar_bg.g() * 2 / 3, t.bull.b() / 3 + t.toolbar_bg.b() * 2 / 3),
            s if s > 0.2  => (t.toolbar_bg.r().saturating_add(8), t.toolbar_bg.g().saturating_add(12), t.toolbar_bg.b().saturating_add(6)),
            s if s > -0.2 => (t.toolbar_bg.r().saturating_add(5), t.toolbar_bg.g().saturating_add(5), t.toolbar_bg.b().saturating_add(5)),
            s if s > -0.6 => (t.toolbar_bg.r().saturating_add(15), t.toolbar_bg.g().saturating_add(10), t.toolbar_bg.b()),
            _             => (t.bear.r() / 4 + t.toolbar_bg.r() * 3 / 4, t.bear.g() / 4 + t.toolbar_bg.g() * 3 / 4, t.bear.b() / 4 + t.toolbar_bg.b() * 3 / 4),
        }
    };
    p.rect_filled(card, r_lg_cr(), Color32::from_rgb(sr, sg, sb));

    // Top bevel highlight — a 1px lighter line along the top edge.
    let r_lg_u8 = r_lg_cr().nw;
    p.rect_filled(
        egui::Rect::from_min_max(card.min, egui::pos2(card.right(), card.top() + 1.0)),
        egui::CornerRadius { nw: r_lg_u8, ne: r_lg_u8, sw: 0, se: 0 },
        Color32::from_rgba_unmultiplied(255, 255, 255, if is_light { 50 } else { 10 }));

    // Border.
    p.rect_stroke(card, r_lg_cr(),
        Stroke::new(stroke_std(), tint(t, Tone::Border, if is_light { 50 } else { 30 })),
        egui::StrokeKind::Outside);
}

/// Draws the floating **header bar** above a hovered card — the kind icon +
/// label, an optional lock glyph, and the context-menu (`⋯`) + mode-toggle
/// buttons — and returns the two button hit-rects `(ctx_btn, mode_btn)` for the
/// caller's interaction routing. Painter-only: the caller owns click handling
/// and cursor feedback (the buttons' hover *visuals* are driven by `ptr` here).
pub(crate) fn overlay_card_header(
    p: &egui::Painter, card: egui::Rect, card_w: f32,
    icon: &str, label: &str, locked: bool, mode_icon: &str,
    ptr: Option<egui::Pos2>, t: &Theme,
) -> (egui::Rect, egui::Rect) {
    let hdr_h = 26.0;
    let hdr = egui::Rect::from_min_size(
        egui::pos2(card.left(), card.top() - hdr_h - 2.0),
        egui::vec2(card_w, hdr_h));
    // Header background + a hairline divider along its bottom edge.
    let hdr_r = r_lg_cr().nw;
    p.rect_filled(hdr,
        egui::CornerRadius { nw: hdr_r, ne: hdr_r, sw: 0, se: 0 },
        Color32::from_rgba_unmultiplied(t.toolbar_bg.r(), t.toolbar_bg.g(), t.toolbar_bg.b(), 230));
    p.line_segment(
        [egui::pos2(hdr.left() + 4.0, hdr.bottom()), egui::pos2(hdr.right() - 4.0, hdr.bottom())],
        Stroke::new(stroke_thin(), tint(t, Tone::Border, alpha_muted())));
    // Icon + label (+ lock glyph).
    p.text(egui::pos2(hdr.left() + 8.0, hdr.center().y),
        egui::Align2::LEFT_CENTER, icon, egui::FontId::proportional(font_md()), t.accent);
    p.text(egui::pos2(hdr.left() + 24.0, hdr.center().y),
        egui::Align2::LEFT_CENTER, label, egui::FontId::monospace(font_xs()), t.text);
    if locked {
        p.text(egui::pos2(hdr.left() + 24.0 + label.len() as f32 * 7.0 + 6.0, hdr.center().y),
            egui::Align2::LEFT_CENTER, "\u{1F512}", egui::FontId::proportional(font_xs()), color_half(t.dim));
    }

    let btn_w = 32.0;
    let btn_h = 22.0;

    // One header button: hover-lit fill + stroke + glyph. Returns its rect.
    let mut header_btn = |center_x: f32, glyph: &str| -> egui::Rect {
        let r = egui::Rect::from_center_size(
            egui::pos2(center_x, hdr.center().y), egui::vec2(btn_w, btn_h));
        let hov = ptr.map(|q| r.contains(q)).unwrap_or(false);
        p.rect_filled(r, r_sm_cr(),
            if hov { tint(t, Tone::Accent, alpha_line()) } else { tint(t, Tone::Border, alpha_subtle()) });
        p.rect_stroke(r, r_sm_cr(),
            Stroke::new(stroke_thin(), if hov { t.accent } else { tint(t, Tone::Border, alpha_muted()) }),
            egui::StrokeKind::Outside);
        p.text(r.center(), egui::Align2::CENTER_CENTER,
            glyph, egui::FontId::proportional(font_lg()), if hov { t.accent } else { t.dim });
        r
    };

    let ctx_rect = header_btn(hdr.right() - btn_w - 20.0, "\u{22EF}");
    let tog_rect = header_btn(hdr.right() - 18.0, mode_icon);
    (ctx_rect, tog_rect)
}

/// Horizontal progress / ratio bar (track + fill) at `rect`, `frac` 0..1.
/// Pill-rounded (corner = height/2). Used by the volatility / ratio widgets.
pub(crate) fn progress_bar(
    p: &egui::Painter, rect: egui::Rect, frac: f32, color: Color32, t: &Theme,
) {
    let cr = egui::CornerRadius::same((rect.height() * 0.5) as u8);
    p.rect_filled(rect, cr, tint(t, Tone::Border, alpha_muted()));
    let w = (rect.width() * frac.clamp(0.0, 1.0)).max(0.0);
    if w > 0.5 {
        let fill = egui::Rect::from_min_size(rect.min, egui::vec2(w, rect.height()));
        p.rect_filled(fill, cr, color);
    }
}
