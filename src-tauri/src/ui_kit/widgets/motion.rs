//! Motion / animation primitives. Wraps egui's animate_* with cubic-ease
//! and exposes a stable token API so durations and curves stay consistent.
//!
//! ### Foundation primitive, not a widget
//! This module exposes timing constants and easing helpers (`ease_bool`,
//! `cursor_visibility`, `INSTANT`, `DELAY_TOOLTIP`, …) consumed by widgets.
//! It has no builder, no `show(ui, theme)`, and renders nothing on its own.
//! The "Builder + show()" rule in `CLAUDE.md` does not apply.
//!
//! ### History
//! Before 2026-05 the foundation primitives lived in
//! `chart::renderer::ui::components::motion` and were re-exported here
//! via `pub use`. P5b inlined them into ui_kit (the audit's extraction
//! Step 4) so the kit no longer depends on chart_renderer for motion.
//! The chart_renderer module now re-exports from here for back-compat.
//!
//! ### Animation kill switch
//! `ease_bool` / `ease_value` short-circuit to the target value when the
//! user's `MotionSpeed` override is `Off` — wired through the P5 token
//! picker. Hosts that need a separate per-style toggle (StyleSettings
//! `animations_enabled`) can compose: `if !style_anim { return target; }`.
//!
//! ### Profiler hook
//! When the foundation lived in chart_renderer, ease_bool/value tracked
//! in-flight animation counts via `foundation::frame_profiler`. To stay
//! portable, ui_kit exposes a `set_animation_hooks(start_fn, finish_fn)`
//! registration point; the chart-app calls it on startup. Default = no-ops.

use egui::{Color32, Context, Id};
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::atomic::{AtomicPtr, Ordering};

// ── Profiler hook (registered by host) ─────────────────────────────────
//
// The chart-app's frame_profiler tracks in-flight animation counts. We
// expose two function-pointer slots a host can register on startup; if
// not registered, the calls are zero-cost no-ops. The chart-app wires
// these in its startup path (native_main.rs) to keep the metrics flowing
// without coupling ui_kit to `foundation::frame_profiler`.

fn noop_hook() {}
static ANIM_START: AtomicPtr<()> = AtomicPtr::new(noop_hook as *mut ());
static ANIM_FINISH: AtomicPtr<()> = AtomicPtr::new(noop_hook as *mut ());

/// Register host callbacks invoked on animation start/finish edges.
/// Pass plain `fn()` function pointers. Defaults are no-ops; callers
/// that don't register lose nothing.
pub fn set_animation_hooks(start: fn(), finish: fn()) {
    ANIM_START.store(start as *mut (), Ordering::Release);
    ANIM_FINISH.store(finish as *mut (), Ordering::Release);
}

#[inline]
fn fire_anim_start() {
    let p = ANIM_START.load(Ordering::Acquire);
    if !p.is_null() {
        // SAFETY: we only ever store fn() pointers via set_animation_hooks
        // or the noop_hook initializer above.
        let f: fn() = unsafe { std::mem::transmute(p) };
        f();
    }
}
#[inline]
fn fire_anim_finish() {
    let p = ANIM_FINISH.load(Ordering::Acquire);
    if !p.is_null() {
        let f: fn() = unsafe { std::mem::transmute(p) };
        f();
    }
}

// ── In-flight animation tracking ───────────────────────────────────────
//
// egui's `animate_*_with_time` walks the cached value toward the target
// over `duration`. We detect "still animating" by comparing the returned
// value to its target — when |raw - target| > EPSILON the transition is
// in flight. On every (idle → flying) transition we fire the registered
// animation-start hook; on every (flying → idle) transition the finish
// hook. State is per-thread (motion is only called from the render
// thread) and capped at MAX_TRACKED entries. Stale Ids (widget unmounted
// mid-animation) are reaped after STALE_TICK_THRESHOLD ticks of no
// observation; if the reaped entry was still flagged in-flight we emit
// a synthetic finish so the counter doesn't leak.

/// Threshold below which a value is considered to have settled.
const ANIM_EPSILON: f32 = 0.001;
const STALE_TICK_THRESHOLD: u64 = 60;
const MAX_TRACKED: usize = 1024;

#[derive(Clone, Copy)]
struct AnimEntry {
    in_flight: bool,
    last_seen_tick: u64,
}

thread_local! {
    static ANIM_STATE: RefCell<HashMap<Id, AnimEntry>> = RefCell::new(HashMap::new());
    static TICK: RefCell<u64> = const { RefCell::new(0) };
}

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
            fire_anim_start();
        } else if !in_flight_now && was_in_flight {
            fire_anim_finish();
        }
        if in_flight_now {
            map.insert(id, AnimEntry { in_flight: true, last_seen_tick: tick });
        } else {
            map.remove(&id);
        }
        if map.len() > MAX_TRACKED {
            map.retain(|_, e| {
                let stale = tick.wrapping_sub(e.last_seen_tick) > STALE_TICK_THRESHOLD;
                if stale && e.in_flight { fire_anim_finish(); }
                !stale
            });
        }
    });
}

// ── Duration tokens ────────────────────────────────────────────────────
pub const FAST: f32 = 0.12;
pub const MED:  f32 = 0.18;
pub const SLOW: f32 = 0.28;

// ── Easing ─────────────────────────────────────────────────────────────
/// Cubic ease in/out. Input + output in 0..=1.
#[inline]
pub fn ease_in_out_cubic(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    if t < 0.5 { 4.0 * t * t * t } else { 1.0 - (-2.0 * t + 2.0).powi(3) / 2.0 }
}

/// Returns `true` if the user has set `MotionSpeed::Off`. Short-circuits
/// every ease_* helper to snap behavior.
#[inline]
fn animations_disabled() -> bool {
    crate::ui_kit::style::motion_speed_override() == crate::ui_kit::style::MotionSpeed::Off
}

/// Returns `true` while the pointer is being dragged anywhere in the app
/// (chart pan, slider drag, tab reorder, etc.). During a drag the user
/// doesn't care about button hover / tab transition animations on widgets
/// the cursor sweeps past — and those animations are surprisingly expensive
/// in aggregate because each `animate_*_with_time` call schedules a frame-
/// budget repaint while the animation settles. Pausing them eliminates the
/// per-frame budget pressure that makes Fifo+lat=2 input lag perceptible.
///
/// (2026-05-26 perf fix — root cause of "drag feels delayed" was that
/// motion.rs grew from a 74-line stub at PREICED to a full 608-line impl
/// post-iced-wave; the egui-side `animate_*` calls under it push some frames
/// over the 16ms vsync budget, exposing the latent buffered input lag.)
#[inline]
fn pointer_is_dragging(ctx: &Context) -> bool {
    ctx.input(|i| i.pointer.is_decidedly_dragging())
}

// ── Bool tracker ───────────────────────────────────────────────────────
pub fn ease_bool(ctx: &Context, id: Id, value: bool, duration: f32) -> f32 {
    if animations_disabled() || pointer_is_dragging(ctx) {
        return if value { 1.0 } else { 0.0 };
    }
    let raw = ctx.animate_bool_with_time(id, value, duration);
    let target = if value { 1.0 } else { 0.0 };
    let in_flight = (raw - target).abs() > ANIM_EPSILON;
    observe(id, in_flight, next_tick());
    ease_in_out_cubic(raw)
}

// ── Value tracker ──────────────────────────────────────────────────────
pub fn ease_value(ctx: &Context, id: Id, target: f32, duration: f32) -> f32 {
    if animations_disabled() || pointer_is_dragging(ctx) {
        return target;
    }
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
        lerp_u8(a.r(), b.r()), lerp_u8(a.g(), b.g()),
        lerp_u8(a.b(), b.b()), lerp_u8(a.a(), b.a()),
    )
}

/// Linearly fade alpha from 0 to `c`'s alpha based on t.
#[inline]
pub fn fade_in(c: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    Color32::from_rgba_premultiplied(c.r(), c.g(), c.b(), (c.a() as f32 * t).round() as u8)
}

// ── Named easing functions (Robert Penner / CSS spec) ─────────────────
// All functions accept t in 0.0..=1.0 (normalized progress) and return
// the eased progress in approximately the same range (overshoot/undershoot
// is intentional for Back and Elastic families).

/// Linear — no easing.  f(t) = t
#[inline] pub fn ease_linear(t: f32) -> f32 { t.clamp(0.0, 1.0) }

// ── Quadratic ─────────────────────────────────────────────────────────
/// Quadratic ease-in.  f(t) = t²
#[inline] pub fn ease_in_quad(t: f32) -> f32 { let t = t.clamp(0.0,1.0); t * t }

/// Quadratic ease-out.  f(t) = t(2-t)
#[inline] pub fn ease_out_quad(t: f32) -> f32 { let t = t.clamp(0.0,1.0); t * (2.0 - t) }

/// Quadratic ease-in-out.
#[inline] pub fn ease_in_out_quad(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    if t < 0.5 { 2.0 * t * t } else { -1.0 + (4.0 - 2.0 * t) * t }
}

// ── Cubic ─────────────────────────────────────────────────────────────
/// Cubic ease-in.  f(t) = t³
#[inline] pub fn ease_in_cubic(t: f32) -> f32 { let t = t.clamp(0.0,1.0); t * t * t }

/// Cubic ease-out.  f(t) = (t-1)³ + 1
#[inline] pub fn ease_out_cubic(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0) - 1.0;
    t * t * t + 1.0
}

// ease_in_out_cubic is provided by the re-exported foundation module.

// ── Quartic ───────────────────────────────────────────────────────────
/// Quartic ease-in.  f(t) = t⁴
#[inline] pub fn ease_in_quart(t: f32) -> f32 { let t = t.clamp(0.0,1.0); t*t*t*t }

/// Quartic ease-out.
#[inline] pub fn ease_out_quart(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0) - 1.0;
    1.0 - t*t*t*t
}

/// Quartic ease-in-out.
#[inline] pub fn ease_in_out_quart(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    if t < 0.5 { 8.0*t*t*t*t } else { let t=t-1.0; 1.0 - 8.0*t*t*t*t }
}

// ── Quintic ───────────────────────────────────────────────────────────
/// Quintic ease-in.  f(t) = t⁵
#[inline] pub fn ease_in_quint(t: f32) -> f32 { let t = t.clamp(0.0,1.0); t*t*t*t*t }

/// Quintic ease-out.
#[inline] pub fn ease_out_quint(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0) - 1.0;
    1.0 + t*t*t*t*t
}

/// Quintic ease-in-out.
#[inline] pub fn ease_in_out_quint(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    if t < 0.5 { 16.0*t*t*t*t*t } else { let t=t-1.0; 1.0 + 16.0*t*t*t*t*t }
}

// ── Sine ──────────────────────────────────────────────────────────────
/// Sine ease-in.
#[inline] pub fn ease_in_sine(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (t * std::f32::consts::FRAC_PI_2).cos()
}

/// Sine ease-out.
#[inline] pub fn ease_out_sine(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    (t * std::f32::consts::FRAC_PI_2).sin()
}

/// Sine ease-in-out.
#[inline] pub fn ease_in_out_sine(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    -(( std::f32::consts::PI * t).cos() - 1.0) / 2.0
}

// ── Exponential ───────────────────────────────────────────────────────
/// Exponential ease-in.
#[inline] pub fn ease_in_expo(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    if t == 0.0 { 0.0 } else { (2.0_f32).powf(10.0 * t - 10.0) }
}

/// Exponential ease-out.
#[inline] pub fn ease_out_expo(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    if t == 1.0 { 1.0 } else { 1.0 - (2.0_f32).powf(-10.0 * t) }
}

/// Exponential ease-in-out.
#[inline] pub fn ease_in_out_expo(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    if t == 0.0 { return 0.0; }
    if t == 1.0 { return 1.0; }
    if t < 0.5 { (2.0_f32).powf(20.0 * t - 10.0) / 2.0 }
    else { (2.0 - (2.0_f32).powf(-20.0 * t + 10.0)) / 2.0 }
}

// ── Circular ──────────────────────────────────────────────────────────
/// Circular ease-in.
#[inline] pub fn ease_in_circ(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t * t).sqrt()
}

/// Circular ease-out.
#[inline] pub fn ease_out_circ(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0) - 1.0;
    (1.0 - t * t).sqrt()
}

/// Circular ease-in-out.
#[inline] pub fn ease_in_out_circ(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    if t < 0.5 {
        (1.0 - (1.0 - (2.0*t)*(2.0*t)).sqrt()) / 2.0
    } else {
        ((1.0 - (-2.0*t+2.0)*(-2.0*t+2.0)).sqrt() + 1.0) / 2.0
    }
}

// ── Back (overshoot) ──────────────────────────────────────────────────
const BACK_C1: f32 = 1.701_58;       // canonical c1
const BACK_C2: f32 = BACK_C1 * 1.525; // canonical c3 (ease-in-out)
const BACK_C3: f32 = BACK_C1 + 1.0;

/// Back ease-in (slight backward windup before forward motion).
#[inline] pub fn ease_in_back(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    BACK_C3*t*t*t - BACK_C1*t*t
}

/// Back ease-out (overshoots target then settles back).
/// Gives a satisfying "snap past and return" feel — ideal for toggles.
#[inline] pub fn ease_out_back(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0) - 1.0;
    1.0 + BACK_C3*t*t*t + BACK_C1*t*t
}

/// Back ease-in-out.
#[inline] pub fn ease_in_out_back(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    if t < 0.5 {
        ((2.0*t)*(2.0*t) * ((BACK_C2+1.0)*2.0*t - BACK_C2)) / 2.0
    } else {
        ((2.0*t-2.0)*(2.0*t-2.0) * ((BACK_C2+1.0)*(2.0*t-2.0) + BACK_C2) + 2.0) / 2.0
    }
}

// ── Elastic ───────────────────────────────────────────────────────────
const ELASTIC_C4: f32 = std::f32::consts::TAU / 3.0;
const ELASTIC_C5: f32 = std::f32::consts::TAU / 4.5;

/// Elastic ease-in (spring oscillation at start).
#[inline] pub fn ease_in_elastic(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    if t == 0.0 { return 0.0; }
    if t == 1.0 { return 1.0; }
    -(2.0_f32).powf(10.0*t - 10.0) * ((10.0*t - 10.75) * ELASTIC_C4).sin()
}

/// Elastic ease-out (spring oscillation settling at end).
#[inline] pub fn ease_out_elastic(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    if t == 0.0 { return 0.0; }
    if t == 1.0 { return 1.0; }
    (2.0_f32).powf(-10.0*t) * ((10.0*t - 0.75) * ELASTIC_C4).sin() + 1.0
}

/// Elastic ease-in-out.
#[inline] pub fn ease_in_out_elastic(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    if t == 0.0 { return 0.0; }
    if t == 1.0 { return 1.0; }
    if t < 0.5 {
        -((2.0_f32).powf(20.0*t - 10.0) * ((20.0*t - 11.125) * ELASTIC_C5).sin()) / 2.0
    } else {
        (2.0_f32).powf(-20.0*t + 10.0) * ((20.0*t - 11.125) * ELASTIC_C5).sin() / 2.0 + 1.0
    }
}

// ── Bounce ────────────────────────────────────────────────────────────
/// Bounce ease-out (bouncing ball landing).
#[inline] pub fn ease_out_bounce(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    const N: f32 = 7.5625;
    const D: f32 = 2.75;
    if t < 1.0/D {
        N * t * t
    } else if t < 2.0/D {
        let t = t - 1.5/D;
        N * t * t + 0.75
    } else if t < 2.5/D {
        let t = t - 2.25/D;
        N * t * t + 0.9375
    } else {
        let t = t - 2.625/D;
        N * t * t + 0.984_375
    }
}

/// Bounce ease-in.
#[inline] pub fn ease_in_bounce(t: f32) -> f32 { 1.0 - ease_out_bounce(1.0 - t) }

/// Bounce ease-in-out.
#[inline] pub fn ease_in_out_bounce(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    if t < 0.5 { (1.0 - ease_out_bounce(1.0 - 2.0*t)) / 2.0 }
    else { (1.0 + ease_out_bounce(2.0*t - 1.0)) / 2.0 }
}

// ── Curve enum ────────────────────────────────────────────────────────
/// Named easing curve — pass to animation helpers for named, consistent curves.
///
/// ```ignore
/// let t = motion::ease_bool(ctx, id, on, motion::FAST);
/// let eased = Curve::EaseOutBack.apply(t);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Curve {
    Linear,
    EaseIn,          // alias: EaseInCubic
    EaseOut,         // alias: EaseOutCubic
    EaseInOut,       // alias: EaseInOutCubic (foundation)
    EaseInQuad,
    EaseOutQuad,
    EaseInOutQuad,
    EaseInBack,
    EaseOutBack,
    EaseInOutBack,
    EaseInOutCirc,
    EaseOutCirc,
    EaseOutElastic,
    EaseInElastic,
    EaseOutBounce,
    Spring,          // approximated via ease_out_elastic with gentle params
}

impl Curve {
    /// Apply this curve to a normalized progress value `t` in 0.0..=1.0.
    #[inline]
    pub fn apply(self, t: f32) -> f32 {
        match self {
            Curve::Linear        => ease_linear(t),
            Curve::EaseIn        => ease_in_cubic(t),
            Curve::EaseOut       => ease_out_cubic(t),
            Curve::EaseInOut     => ease_in_out_cubic(t),
            Curve::EaseInQuad    => ease_in_quad(t),
            Curve::EaseOutQuad   => ease_out_quad(t),
            Curve::EaseInOutQuad => ease_in_out_quad(t),
            Curve::EaseInBack    => ease_in_back(t),
            Curve::EaseOutBack   => ease_out_back(t),
            Curve::EaseInOutBack => ease_in_out_back(t),
            Curve::EaseInOutCirc => ease_in_out_circ(t),
            Curve::EaseOutCirc   => ease_out_circ(t),
            Curve::EaseOutElastic => ease_out_elastic(t),
            Curve::EaseInElastic => ease_in_elastic(t),
            Curve::EaseOutBounce => ease_out_bounce(t),
            // Spring approximation: gentle elastic overshoot (τ/4.5 period,
            // lower amplitude than EaseOutElastic full params).
            Curve::Spring        => {
                let t = t.clamp(0.0, 1.0);
                if t == 0.0 { return 0.0; }
                if t == 1.0 { return 1.0; }
                (2.0_f32).powf(-8.0*t) * ((10.0*t - 0.75) * ELASTIC_C4).sin() + 1.0
            }
        }
    }
}

// ── Spring physics ─────────────────────────────────────────────────────
/// A damped-spring integrator driven by explicit physics parameters.
///
/// Unlike the `Curve` enum which maps a normalized 0..=1 input to an
/// eased output, `Spring` integrates velocity over time and must be
/// stored in widget state between frames.
///
/// ### Quick start
/// ```ignore
/// // In your widget state:
/// let mut spring = Spring::new().stiffness(300.0).damping(24.0).mass(1.0);
///
/// // Each frame, call step() with the elapsed dt and current target:
/// spring.step(ui.ctx().input(|i| i.unstable_dt), target_x);
/// let x = spring.value();
/// ```
#[derive(Debug, Clone, Copy)]
pub struct Spring {
    /// Current position.
    pub value:     f32,
    /// Current velocity (units/second).
    pub velocity:  f32,
    /// Spring stiffness k (N/m equivalent). Higher = faster snap. Default 200.
    pub stiffness: f32,
    /// Damping coefficient c. Higher = less oscillation. Default 20.
    pub damping:   f32,
    /// Mass m (kg equivalent). Higher = more inertia. Default 1.
    pub mass:      f32,
}

impl Default for Spring {
    fn default() -> Self {
        Self { value: 0.0, velocity: 0.0, stiffness: 200.0, damping: 20.0, mass: 1.0 }
    }
}

impl Spring {
    /// Create a new spring at rest at position 0.
    pub fn new() -> Self { Self::default() }

    /// Create a spring already at rest at `position`.
    pub fn at(position: f32) -> Self { Self { value: position, ..Self::default() } }

    /// Set stiffness (spring constant k). Default 200. Higher = snappier.
    #[must_use]
    pub fn stiffness(mut self, k: f32) -> Self { self.stiffness = k; self }

    /// Set damping coefficient c. Default 20. At critical damping:
    /// c = 2 * sqrt(k * m). Higher → overdamped (no oscillation).
    #[must_use]
    pub fn damping(mut self, c: f32) -> Self { self.damping = c; self }

    /// Set mass m. Default 1. Higher = more inertia / slower response.
    #[must_use]
    pub fn mass(mut self, m: f32) -> Self { self.mass = m.max(0.0001); self }

    /// Integrate one time step of `dt` seconds toward `target`.
    /// Uses semi-implicit (symplectic) Euler which is unconditionally
    /// stable for reasonable stiffness/dt combinations.
    pub fn step(&mut self, dt: f32, target: f32) {
        let dt = dt.clamp(0.0, 0.05); // cap to 50ms to avoid dt spikes on tab resume
        let force = -self.stiffness * (self.value - target)
                    - self.damping * self.velocity;
        let accel  = force / self.mass;
        self.velocity += accel * dt;
        self.value    += self.velocity * dt;
    }

    /// Returns true when the spring has settled (position and velocity
    /// are both within the given epsilon of the target).
    pub fn is_settled(&self, target: f32, epsilon: f32) -> bool {
        (self.value - target).abs() < epsilon && self.velocity.abs() < epsilon
    }

    /// Instantly teleport to `position` (e.g. on first mount).
    pub fn snap_to(&mut self, position: f32) {
        self.value    = position;
        self.velocity = 0.0;
    }
}

// (Context, Id already imported at top of file in P5b motion-inlining.)

// ── Delay / micro-timing tokens ────────────────────────────────────────
/// Instant transitions (~30ms — for cursor blink smoothing, micro-flashes).
pub const INSTANT: f32 = 0.03;
/// Tooltip-style delay before showing (400ms).
pub const DELAY_TOOLTIP: f32 = 0.4;
/// Hover-card delay (600ms — slower than tooltip because content is heavier).
pub const DELAY_HOVER_CARD: f32 = 0.6;
/// Scroll wheel ease duration.
pub const SCROLL_EASE_DURATION: f32 = 0.20;

// ── Cursor blink ───────────────────────────────────────────────────────
/// Standard cursor blink rate. 530ms on, 530ms off — matches Zed.
pub const CURSOR_BLINK_PERIOD: f32 = 1.06;

/// Returns 1.0 when the cursor should be visible, 0.0 when invisible.
/// Driven by wall-clock time. Pass `force_visible=true` while the cursor
/// should remain solid (e.g., during active typing for ~700ms after the
/// last keystroke).
///
/// Output is smoothed via a 60ms cubic ease so the blink is a soft fade
/// rather than a hard square wave. Caller stores the time of the last
/// keystroke and computes `force_visible = (now - last_keystroke) < 0.7`.
pub fn cursor_visibility(ctx: &Context, id: Id, force_visible: bool) -> f32 {
    let target = if force_visible {
        1.0
    } else {
        let t = ctx.input(|i| i.time) as f32;
        let phase = t.rem_euclid(CURSOR_BLINK_PERIOD);
        if phase < CURSOR_BLINK_PERIOD * 0.5 { 1.0 } else { 0.0 }
    };
    // Smooth the square-wave with a ~60ms cubic ease so the blink fades
    // instead of snapping. ease_value already applies cubic via the
    // egui animation memory.
    let smoothed = ease_value(ctx, id, target, INSTANT);
    smoothed.clamp(0.0, 1.0)
}

// ── Scroll / momentum tuning ───────────────────────────────────────────
/// Friction multiplier applied per frame to scroll velocity for momentum
/// scrolling. Lower = more drag, higher = more glide. egui's default
/// already exposes momentum on touch/trackpad; this token documents the
/// desired feel for ThemedScrollArea wrappers that tune it.
pub const SCROLL_FRICTION: f32 = 0.92;

/// Below this absolute velocity (px/frame) momentum is considered done
/// and the offset is snapped to a stable integer.
pub const SCROLL_STOP_THRESHOLD: f32 = 0.1;

/// Duration used when easing a scroll offset toward a target (e.g. wheel
/// click → smooth scroll). Matches `motion::FAST` philosophy but slightly
/// longer so the eye can track long jumps.
pub const SCROLL_EASE: f32 = SCROLL_EASE_DURATION; // 200ms cubic ease-out
