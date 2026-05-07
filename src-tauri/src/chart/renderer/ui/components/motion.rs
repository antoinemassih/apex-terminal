//! Motion / animation primitives. Wraps egui's animate_* with cubic-ease
//! and exposes a stable token API so durations and curves stay consistent
//! across the app.
//!
//! Use these instead of snap-to-state painting. Example:
//!
//! ```ignore
//! let hover_t = motion::ease_bool(ui.ctx(), id, response.hovered(), motion::FAST);
//! let bg = motion::lerp_color(idle_bg, hover_bg, hover_t);
//! painter.rect_filled(rect, 0.0, bg);
//! ```

use egui::{Color32, Context, Id};

// ── Duration tokens ────────────────────────────────────────────────────
pub const FAST: f32 = 0.12;     // 120ms — hover backgrounds, small chrome
pub const MED:  f32 = 0.18;     // 180ms — panel toggles, tab actives
pub const SLOW: f32 = 0.28;     // 280ms — modal entries, large state changes

// ── Easing ─────────────────────────────────────────────────────────────
/// Cubic ease in/out. Input + output in 0..=1.
#[inline]
pub fn ease_in_out_cubic(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    if t < 0.5 { 4.0 * t * t * t } else { 1.0 - (-2.0 * t + 2.0).powi(3) / 2.0 }
}

// ── Bool tracker ───────────────────────────────────────────────────────
/// Returns 0..=1 that smoothly tracks `value` over `duration` seconds,
/// with cubic ease applied. Stable across frames via egui memory keyed
/// on `id`.
pub fn ease_bool(ctx: &Context, id: Id, value: bool, duration: f32) -> f32 {
    let raw = ctx.animate_bool_with_time(id, value, duration);
    ease_in_out_cubic(raw)
}

// ── Value tracker ──────────────────────────────────────────────────────
pub fn ease_value(ctx: &Context, id: Id, target: f32, duration: f32) -> f32 {
    ctx.animate_value_with_time(id, target, duration)
}

// ── Lerp helpers ───────────────────────────────────────────────────────
#[inline]
pub fn lerp_f32(a: f32, b: f32, t: f32) -> f32 { a + (b - a) * t }

#[inline]
pub fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let lerp_u8 = |x: u8, y: u8| -> u8 {
        (x as f32 + (y as f32 - x as f32) * t).round().clamp(0.0, 255.0) as u8
    };
    Color32::from_rgba_premultiplied(
        lerp_u8(a.r(), b.r()),
        lerp_u8(a.g(), b.g()),
        lerp_u8(a.b(), b.b()),
        lerp_u8(a.a(), b.a()),
    )
}

/// Linearly fade alpha from 0 to `c`'s alpha based on t.
#[inline]
pub fn fade_in(c: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    Color32::from_rgba_premultiplied(c.r(), c.g(), c.b(),
        (c.a() as f32 * t).round() as u8)
}
