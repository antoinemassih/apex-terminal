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

/// Horizontal progress / ratio bar (track + fill) at `rect`, `frac` 0..1.
#[allow(dead_code)] // kit primitive — ready for the next batch of bar widgets
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
