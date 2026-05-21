//! # Design System — two-axis theme engine (Phase B1)
//!
//! This module implements the **B1 phase** of the theme system refactor as
//! specified in `docs/THEME_SYSTEM_SPEC.md`.  It is entirely additive — no
//! existing call site, struct, or function is modified by this phase.
//!
//! ## Two-axis model
//!
//! A "theme" is the *product* of two independently-selectable axes:
//!
//! | Axis | Struct | Holds |
//! |---|---|---|
//! | **Style** | [`StyleSystem`] | Typography, spacing, radii, strokes, alphas, elevation, density, shadow geometry, behavioural treatments. **No colour.** |
//! | **Palette** | [`ColorScheme`] | Background, surface, text, accent, bull, bear, warn, shadow tint. **No dimensions.** |
//!
//! `N` styles × `M` colour schemes = `N·M` valid combinations, all switchable
//! at runtime without recompile.
//!
//! ## Sub-modules
//!
//! - [`color_scheme`] — `ColorScheme`, `Rgba`, `Meta`.
//! - [`style_system`] — `StyleSystem` and its sub-structs.
//! - [`snapshot`] — `DesignSnapshot`: the flat `Copy` per-frame resolved form.
//! - [`loader`] — DTCG JSON ↔ `StyleSystem` / `ColorScheme` round-trip.
//! - [`registry`] — `ThemeRegistry`: runtime owner + active-pair tracker.
//!
//! ## Usage (Phase B2+)
//!
//! ```rust,ignore
//! // Once per frame, before any widget rendering:
//! pub fn begin_frame(active: &ActiveTheme) {
//!     FRAME.with(|c| c.set(active.snapshot()));
//! }
//!
//! // Token reads in style.rs (signature unchanged):
//! pub fn font_sm() -> f32 { FRAME.with(|c| c.get().size_sm) }
//! ```

pub mod adapter;
pub mod baseline;
pub mod builtin;
pub mod color_scheme;
pub mod equivalence_tests;
pub mod export;
pub mod hot_reload;
pub mod loader;
pub mod registry;
pub mod snapshot;
pub mod style_system;

// ── Convenient top-level re-exports ──────────────────────────────────────────

pub use adapter::color_scheme_to_theme;
pub use baseline::{baseline_color_scheme, baseline_style_system};
pub use builtin::{builtin_color_schemes, builtin_registry, builtin_style_systems};
pub use color_scheme::{ColorScheme, Meta, Rgba};
pub use export::{export_builtin_themes, scan_theme_dir};
pub use hot_reload::{active_override, start_theme_watcher};
pub use registry::{ActiveTheme, ThemeRegistry};
pub use snapshot::{DesignSnapshot, DEFAULT_SNAPSHOT};
pub use style_system::StyleSystem;
