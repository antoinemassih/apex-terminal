//! Re-exports the motion primitives from chart/renderer/ui/components/motion.rs
//! so widgets can stay decoupled from chart_renderer paths. Migrate the
//! original module here in a future pass.
//!
//! In addition to the re-exports, this module hosts ui_kit-only motion
//! tokens that don't yet have a home in the legacy module:
//!   - `CURSOR_BLINK_PERIOD` / `cursor_visibility` for custom-paint text
//!     editors (Phase 2 — `ui_kit::widgets::Input` still uses egui's
//!     built-in blink and is not migrated by this task).
//!   - Scroll/momentum tuning constants consumed by
//!     `ui_kit::widgets::scroll_area::ThemedScrollArea`.

pub use crate::chart::renderer::ui::components::motion::*;

use egui::{Context, Id};

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
    let smoothed = ease_value(ctx, id, target, 0.06);
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
pub const SCROLL_EASE: f32 = 0.20; // 200ms cubic ease-out
