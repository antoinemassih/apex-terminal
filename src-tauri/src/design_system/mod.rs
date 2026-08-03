//! # Design System — two-axis theme engine
//!
//! ## Two-axis model
//!
//! A "theme" is the *product* of two independently-selectable axes:
//!
//! | Axis | Struct | Holds |
//! |---|---|---|
//! | **Style** | [`StyleSystem`] | Typography, spacing, radii, strokes, alphas, elevation, density, shadow geometry, behavioural treatments. **No colour.** |
//! | **Palette** | [`ColorScheme`] | Background, surface, text, accent, bull, bear, warn, shadow tint, command-palette badges. **No dimensions.** |
//!
//! `N` styles × `M` palettes = `N·M` valid combinations, all switchable
//! at runtime without recompile.
//!
//! ## Live runtime wiring (current as of 2026-05, post-Phase 1)
//!
//! - **Palette axis** is fully live:
//!   - `LIVE_THEMES` in `chart_renderer::gpu` is populated at startup from
//!     [`builtin_color_schemes`] via [`color_scheme_to_theme`].
//!   - User-installed JSON files in `<themes_dir>/colorschemes/*.json` are
//!     loaded at startup by [`scan_theme_dir`] and hot-reloaded by the
//!     [`start_theme_watcher`] thread via `upsert_installed_themes()` —
//!     edits take effect on the next frame without restart.
//!   - Per-pane palette selection is driven by `Watchlist.theme_idx`.
//!   - The "+ cmd_palette" extension (P1.4) lives on `ColorScheme` so per-
//!     scheme overrides of the 11 command-palette badge colours are possible.
//!
//! - **Style axis is partially live:**
//!   - The `StyleSystem` slot under [`start_theme_watcher`] is hot-reloadable
//!     via `<themes_dir>/styles/*.json` (radii + strokes flow into the
//!     `THEME_OVERRIDE` slot consumed by `chart_renderer::ui::style::begin_frame`).
//!   - The runtime hot-path uses [`super::chart_renderer::ui::style::StyleSettings`]
//!     (semantic role names: `font_caption`, `font_body`, `font_hero`) rather
//!     than the tier-scale token names in [`style_system::Typography`].
//!     Reconciling the two naming schemes (semantic vs tier) is a Phase 5
//!     task tied to the `ComponentTheme` trait reshape — it touches
//!     1400+ call sites.
//!   - [`ThemeRegistry`] is now exposed as a process-wide singleton via
//!     [`live_registry()`] (P1.8). It owns the canonical {styles, schemes,
//!     active pair} state. The chart-app's per-pane `theme_idx` path into
//!     `gpu::LIVE_THEMES` remains the hot path for frame colour resolution.
//!   - [`ActiveTheme`] is the snapshot type for resolving the active pair.
//!
//! ## Sub-modules
//!
//! - [`color_scheme`] — `ColorScheme`, `Rgba`, `Meta`, `CMD_PALETTE_DEFAULT`.
//! - [`style_system`] — `StyleSystem` and its sub-structs.
//! - [`snapshot`] — `DesignSnapshot`: the flat `Copy` per-frame resolved form.
//! - [`adapter`] — `color_scheme_to_theme` (the runtime palette adapter).
//! - [`loader`] — DTCG JSON ↔ `StyleSystem` / `ColorScheme` round-trip.
//! - [`export`] — write built-in themes to disk as DTCG JSON.
//! - [`hot_reload`] — file watcher for live palette/style editing.
//! - [`registry`] — `ThemeRegistry`: scaffolding for global active-pair tracking
//!   (not wired to the per-pane runtime model).
//! - [`builtin`] — the 16 built-in `ColorScheme`s and 3 built-in `StyleSystem`s.
//! - [`builtin_recipes`] — per-style `RecipeSet`s (the component-level payload
//!   that makes a style switch restyle COMPONENTS, not just colours). Authored
//!   from the React port's `html[data-ds="…"] .ds-…` rules.
//! - [`baseline`] — golden snapshot used by equivalence tests.
//! - [`equivalence_tests`] — `#[cfg(test)]` parity proof against `gpu::THEMES`.
//!
//! ## What deletes when
//!
//! - `gpu::THEMES` is now `#[cfg(test)]` only — it survives as the equivalence
//!   tests' ground truth. Drop it when those tests get re-anchored on a
//!   golden snapshot rather than the legacy const.
//! - `gpu::CMD_PALETTE_DEFAULT` survives because the `#[cfg(test)]` THEMES
//!   const still references it; will go away with THEMES.

// P13: `adapter` (ColorScheme → gpu::Theme) moved into chart_renderer to sever
// design_system's dependency on chart_renderer types. Re-imported via the
// chart_renderer side at `chart_renderer::theme_adapter::color_scheme_to_theme`.
pub mod baseline;
pub mod builtin;
pub mod builtin_recipes;
pub mod color_scheme;
pub mod equivalence_tests;
pub mod export;
pub mod hot_reload;
pub mod import;
pub mod loader;
pub mod recipes;
pub mod style_system;
pub mod theme_pack;

// ── Convenient top-level re-exports ──────────────────────────────────────────

// `color_scheme_to_theme` lives in chart_renderer::theme_adapter (P13). The
// design_system crate intentionally does not re-export it — callers should
// import directly from chart_renderer, which keeps the dependency arrow
// pointing the right way (chart_renderer → design_system, never reverse).
pub use baseline::{baseline_color_scheme, baseline_style_system};
pub use builtin::{builtin_color_schemes, builtin_style_systems};
pub use builtin_recipes::builtin_recipes;
pub use color_scheme::{ColorScheme, Meta, Rgba};
pub use export::{export_builtin_themes, scan_theme_dir};
pub use hot_reload::{active_override, start_theme_watcher, themes_dir};
pub use recipes::{RecipeSet, SxCache};
pub use style_system::StyleSystem;
