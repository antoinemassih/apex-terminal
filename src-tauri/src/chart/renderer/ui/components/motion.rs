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
use std::cell::RefCell;
use std::collections::HashMap;

use crate::foundation::frame_profiler;

// ── In-flight animation tracking ───────────────────────────────────────
//
// egui's `animate_*_with_time` walks the cached value toward the target
// over `duration`. We detect "still animating" by comparing the returned
// value to its target — when |raw - target| > EPSILON the transition is
// in flight. On every (idle → flying) transition we bump
// frame_profiler::animation_started; on every (flying → idle) transition
// we call animation_finished. Each (Id) flips at most once per direction
// so the global counter stays balanced.
//
// State is per-thread (motion is only called from the render thread) and
// capped at MAX_TRACKED entries. Stale Ids (widget unmounted mid-animation)
// are reaped after STALE_FRAME_THRESHOLD ticks of no observation; if the
// reaped entry was still flagged in-flight we emit a synthetic
// animation_finished so the global counter doesn't leak.

/// Threshold below which a value is considered to have settled on its
/// target. egui's interpolator typically lands well within 1e-3.
const ANIM_EPSILON: f32 = 0.001;
/// Approximate tick threshold (≈ frames) after which an unobserved entry
/// is treated as gone. We tick on every ease_* call, so this is a soft
/// "60 calls without seeing this id" limit, not literal frames.
const STALE_TICK_THRESHOLD: u64 = 60;
/// Soft cap on the tracking map. When exceeded we sweep stale entries.
const MAX_TRACKED: usize = 1024;

#[derive(Clone, Copy)]
struct AnimEntry {
    /// Whether this id was last seen in flight (and thus has an
    /// outstanding animation_started call to balance).
    in_flight: bool,
    /// Monotonic tick of the last observation. Used for eviction.
    last_seen_tick: u64,
}

thread_local! {
    static ANIM_STATE: RefCell<HashMap<Id, AnimEntry>> = RefCell::new(HashMap::new());
    static TICK: RefCell<u64> = const { RefCell::new(0) };
}

/// Bump the per-thread tick and run an opportunistic eviction sweep when
/// the map gets large. Stale entries that were still flagged in-flight
/// emit a balancing animation_finished so the global counter stays sane.
fn next_tick() -> u64 {
    TICK.with(|t| {
        let mut t = t.borrow_mut();
        *t = t.wrapping_add(1);
        *t
    })
}

fn observe(id: Id, in_flight_now: bool, tick: u64) {
    ANIM_STATE.with(|s| {
        let mut map = s.borrow_mut();
        let was_in_flight = map.get(&id).map(|e| e.in_flight).unwrap_or(false);

        if in_flight_now && !was_in_flight {
            frame_profiler::animation_started();
        } else if !in_flight_now && was_in_flight {
            frame_profiler::animation_finished();
        }

        if in_flight_now {
            map.insert(id, AnimEntry { in_flight: true, last_seen_tick: tick });
        } else {
            // Settled: drop the entry to keep the map small. The
            // animation_finished (if any) was already dispatched above.
            map.remove(&id);
        }

        // Opportunistic sweep: only when the map is oversized, to keep
        // the hot path cheap.
        if map.len() > MAX_TRACKED {
            map.retain(|_, e| {
                let stale = tick.wrapping_sub(e.last_seen_tick) > STALE_TICK_THRESHOLD;
                if stale && e.in_flight {
                    // Implicit finish — widget vanished mid-animation.
                    frame_profiler::animation_finished();
                }
                !stale
            });
        }
    });
}

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
    let target = if value { 1.0 } else { 0.0 };
    let in_flight = (raw - target).abs() > ANIM_EPSILON;
    observe(id, in_flight, next_tick());
    ease_in_out_cubic(raw)
}

// ── Value tracker ──────────────────────────────────────────────────────
pub fn ease_value(ctx: &Context, id: Id, target: f32, duration: f32) -> f32 {
    let raw = ctx.animate_value_with_time(id, target, duration);
    let in_flight = (raw - target).abs() > ANIM_EPSILON;
    observe(id, in_flight, next_tick());
    raw
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
