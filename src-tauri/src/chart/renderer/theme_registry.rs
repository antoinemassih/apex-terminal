//! Live theme registry (WS-E E2 extraction from gpu.rs).
//!
//! Owns the process-wide LIVE_THEMES store: built-in schemes adapted from
//! design_system at init, plus user/installed schemes appended/upserted at
//! runtime. Extracted verbatim to shrink gpu.rs; re-exported there so every
//! `gpu::get_theme()` / `gpu::append_installed_themes()` call site is unchanged.
//! `Theme` lives in the sibling gpu module; the ColorScheme->Theme adapter and
//! design_system are reached by absolute path.

use super::gpu::Theme;
use std::sync::{OnceLock, RwLock};

static LIVE_THEMES: OnceLock<RwLock<Vec<Theme>>> = OnceLock::new();

pub(crate) fn live_themes() -> &'static RwLock<Vec<Theme>> {
    LIVE_THEMES.get_or_init(|| {
        // Phase B flip — the live theme list is now sourced from the
        // design_system registry: each built-in ColorScheme is adapted to a
        // Theme via color_scheme_to_theme(). The design_system equivalence
        // test proves this is field-exact vs THEMES[] for all 16 themes, so
        // the result is byte-identical to `THEMES.to_vec()` — but the colour
        // axis now genuinely flows through design_system, making it the
        // runtime source of truth (not dead scaffolding).
        let themes: Vec<Theme> = crate::design_system::builtin_color_schemes()
            .iter()
            .map(crate::chart_renderer::theme_adapter::color_scheme_to_theme)
            .collect();
        debug_assert!(
            themes.len() >= 16,
            "design_system must provide at least 16 built-in colour schemes (got {})",
            themes.len(),
        );
        RwLock::new(themes)
    })
}

pub(crate) fn get_theme(idx: usize) -> Theme {
    // Wave 8 fix: recover from a poisoned lock rather than cascading the panic.
    let themes = live_themes().read().unwrap_or_else(|e| e.into_inner());
    // Fall back to index 0 when idx is stale/out-of-range (e.g. after
    // a user-theme was uninstalled or a workspace was created on a
    // machine with more themes).
    themes
        .get(idx)
        .or_else(|| themes.get(0))
        .expect("live_themes is never empty")
        .clone()
}

pub(crate) fn set_theme(idx: usize, theme: Theme) {
    live_themes().write().unwrap_or_else(|e| e.into_inner())[idx] = theme;
}

pub(crate) fn get_all_themes() -> Vec<Theme> {
    live_themes().read().unwrap_or_else(|e| e.into_inner()).clone()
}

pub(crate) fn live_theme_count() -> usize {
    live_themes().read().unwrap_or_else(|e| e.into_inner()).len()
}

pub fn append_installed_themes(schemes: Vec<crate::design_system::ColorScheme>) {
    let mut guard = live_themes().write().unwrap_or_else(|e| e.into_inner());
    for scheme in schemes {
        let candidate = crate::chart_renderer::theme_adapter::color_scheme_to_theme(&scheme);
        let already_present = guard.iter().any(|t| t.name == candidate.name);
        if !already_present {
            guard.push(candidate);
        }
    }
}

/// Hot-reload friendly variant: upsert installed themes into LIVE_THEMES.
/// If a theme with the same name already exists, it's REPLACED in place
/// (preserving its index so the active-theme picker doesn't jump). New
/// names are appended. Used by `design_system::hot_reload` when a
/// colorscheme JSON file changes on disk.
pub fn upsert_installed_themes(schemes: Vec<crate::design_system::ColorScheme>) {
    let mut guard = live_themes().write().unwrap_or_else(|e| e.into_inner());
    for scheme in schemes {
        let candidate = crate::chart_renderer::theme_adapter::color_scheme_to_theme(&scheme);
        if let Some(slot) = guard.iter_mut().find(|t| t.name == candidate.name) {
            *slot = candidate;
        } else {
            guard.push(candidate);
        }
    }
}
