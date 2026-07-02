pub mod shortcuts;
pub mod guard;
pub mod monitoring;
pub mod frame_profiler;
pub mod perf_log;
pub mod design_tokens;
#[cfg(feature = "design-mode")]
pub mod design_inspector;
pub mod types;

// Wave 3: the old `data_types` module became `types/mod.rs`, with new
// submodules for `Price`, `Timestamp`, `Symbol`. The flat re-export
// preserves every prior import path.
pub use types::*;

// Backwards-compatible alias so any `crate::foundation::data_types::X`
// path keeps resolving.
pub use types as data_types;
