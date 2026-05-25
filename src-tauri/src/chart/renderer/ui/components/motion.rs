//! Motion primitives — re-exports from `ui_kit::widgets::motion`.
//!
//! As of P5b (extraction Step 4) the foundation motion primitives
//! (`FAST`, `MED`, `SLOW`, `ease_bool`, `ease_value`, `ease_in_out_cubic`,
//! `lerp_*`, `fade_in`, animation tracking) live in `ui_kit/widgets/motion.rs`.
//!
//! This shim keeps existing `crate::chart::renderer::ui::components::motion::*`
//! call sites working without forcing every panel/widget to update imports
//! in the same commit. Migrate when convenient — none of these re-exports
//! add overhead beyond what the underlying ui_kit fns already do.
//!
//! The chart-app wires its `foundation::frame_profiler` into the ui_kit
//! motion hooks via `motion::set_animation_hooks(...)` at startup
//! (see `native_main.rs`) so animation start/finish telemetry continues
//! to flow.

pub use crate::ui_kit::widgets::motion::*;
