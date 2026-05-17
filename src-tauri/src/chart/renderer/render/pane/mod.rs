//! Chart pane rendering.
//!
//! This module is being split incrementally from the original
//! `pane.rs` monolith (~12,500 lines). See `docs/PANE_RS_SPLIT_PLAN.md`
//! at the repo root for the wave-by-wave plan.
//!
//! **Wave 0 (current state):** The entire original file lives in
//! `core.rs`. This `mod.rs` exists only to give the split a destination
//! directory and to re-export the public entry points so call sites
//! (`render::pane::render_toolbar`, `render::pane::draw_chart`) keep
//! working unchanged.
//!
//! **Future waves** will move:
//! - free helpers → `pane/drawing_helpers.rs`, `pane/deferred.rs`
//! - pure paint sub-systems → `pane/paint.rs`, `pane/geometry.rs`
//! - indicator overlays → `pane/indicators/`
//! - drawing renderer + hit-test → `pane/drawings/`
//! - order chrome → `pane/orders/`
//! - options overlays → `pane/options/`
//!
//! Each wave preserves bit-identical visual behavior. Do not mix wave
//! cuts with feature work — sequencing matters.

mod core;

// Re-export the public entry points so existing call sites (`use
// crate::chart::renderer::render::pane::{render_toolbar, draw_chart}`)
// keep compiling. As sub-systems get extracted in later waves, their
// re-exports get added here too.
pub(crate) use core::{render_toolbar, draw_chart};
