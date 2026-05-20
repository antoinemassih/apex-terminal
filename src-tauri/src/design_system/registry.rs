//! `ThemeRegistry` — the runtime owner of all loaded styles and colour schemes.
//!
//! Spec §4 piece 2, §12.
//!
//! ## Model
//! - Two independent lists: `Vec<Arc<StyleSystem>>` and `Vec<Arc<ColorScheme>>`.
//! - An active *pair* tracked by list index (`active_style_idx`,
//!   `active_colors_idx`).
//! - `set_style(id)` / `set_colors(id)` scan by `meta.id` and swap the active
//!   `Arc`; the change is picked up by the next frame's `begin_frame`.
//! - The registry is **never empty**: the built-in styles/palettes are
//!   inserted at construction so there is always a valid fallback.
//!
//! ## Thread safety
//! `ThemeRegistry` is designed for single-threaded use (egui's render thread).
//! Wrap in `Arc<Mutex<_>>` or `RwLock` if cross-thread access is needed.

use std::sync::Arc;

use super::{
    color_scheme::{builtin_dark, builtin_light, ColorScheme},
    style_system::StyleSystem,
};

// ── ThemeRegistry ─────────────────────────────────────────────────────────────

/// Owns all loaded `StyleSystem`s and `ColorScheme`s; tracks the active pair.
#[derive(Clone, Debug)]
pub struct ThemeRegistry {
    styles: Vec<Arc<StyleSystem>>,
    colors: Vec<Arc<ColorScheme>>,
    active_style_idx:  usize,
    active_colors_idx: usize,
}

impl ThemeRegistry {
    /// Create a registry pre-populated with the built-in styles and palettes.
    ///
    /// The registry is **never empty**: built-ins guarantee at least one valid
    /// entry in each list and a sane active pair on the first frame.
    pub fn with_builtins() -> Self {
        let styles: Vec<Arc<StyleSystem>> = vec![
            Arc::new(StyleSystem::builtin_default()),
            Arc::new(StyleSystem::meridien()),
        ];
        let colors: Vec<Arc<ColorScheme>> = vec![
            Arc::new(builtin_dark()),
            Arc::new(builtin_light()),
        ];
        Self { styles, colors, active_style_idx: 0, active_colors_idx: 0 }
    }

    // ── Style axis ────────────────────────────────────────────────────────────

    /// Return the active `StyleSystem`.
    pub fn active_style(&self) -> Arc<StyleSystem> {
        self.styles[self.active_style_idx].clone()
    }

    /// Set the active style by `meta.id`.  No-op if the id is not found.
    ///
    /// The new style is picked up on the next call to `active()`.
    pub fn set_style(&mut self, id: &str) -> bool {
        if let Some(idx) = self.styles.iter().position(|s| s.meta.id == id) {
            self.active_style_idx = idx;
            true
        } else {
            false
        }
    }

    /// Register a new `StyleSystem`.  If a style with the same `meta.id` is
    /// already present, it is replaced in-place.
    pub fn register_style(&mut self, style: StyleSystem) {
        let id = style.meta.id.clone();
        if let Some(idx) = self.styles.iter().position(|s| s.meta.id == id) {
            self.styles[idx] = Arc::new(style);
        } else {
            self.styles.push(Arc::new(style));
        }
    }

    /// All registered style ids (stable order: built-ins first).
    pub fn style_ids(&self) -> Vec<&str> {
        self.styles.iter().map(|s| s.meta.id.as_str()).collect()
    }

    // ── Color axis ────────────────────────────────────────────────────────────

    /// Return the active `ColorScheme`.
    pub fn active_colors(&self) -> Arc<ColorScheme> {
        self.colors[self.active_colors_idx].clone()
    }

    /// Set the active color scheme by `meta.id`.  No-op if not found.
    pub fn set_colors(&mut self, id: &str) -> bool {
        if let Some(idx) = self.colors.iter().position(|c| c.meta.id == id) {
            self.active_colors_idx = idx;
            true
        } else {
            false
        }
    }

    /// Register a new `ColorScheme`.  Replaces in-place if same id already present.
    pub fn register_colors(&mut self, colors: ColorScheme) {
        let id = colors.meta.id.clone();
        if let Some(idx) = self.colors.iter().position(|c| c.meta.id == id) {
            self.colors[idx] = Arc::new(colors);
        } else {
            self.colors.push(Arc::new(colors));
        }
    }

    /// All registered color scheme ids (stable order: built-ins first).
    pub fn color_ids(&self) -> Vec<&str> {
        self.colors.iter().map(|c| c.meta.id.as_str()).collect()
    }

    // ── Active pair ───────────────────────────────────────────────────────────

    /// Return the active `(StyleSystem, ColorScheme)` pair.
    ///
    /// These are cheap `Arc` clones — the underlying data is shared.
    pub fn active(&self) -> (Arc<StyleSystem>, Arc<ColorScheme>) {
        (self.active_style(), self.active_colors())
    }
}

impl Default for ThemeRegistry {
    fn default() -> Self {
        Self::with_builtins()
    }
}

// ── ActiveTheme ───────────────────────────────────────────────────────────────

/// The resolved active theme pair — a snapshot of the registry's current
/// selection.  Passed by reference to the render loop / widget layer.
///
/// Spec §3.1.
#[derive(Clone, Debug)]
pub struct ActiveTheme {
    pub style:  Arc<StyleSystem>,
    pub colors: Arc<ColorScheme>,
}

impl ActiveTheme {
    /// Resolve the registry's active pair into an `ActiveTheme`.
    pub fn from_registry(reg: &ThemeRegistry) -> Self {
        let (style, colors) = reg.active();
        Self { style, colors }
    }

    /// Snapshot the resolved token values for one frame.
    ///
    /// See [`super::snapshot::snapshot`] for the full resolver.
    pub fn snapshot(&self) -> super::snapshot::DesignSnapshot {
        super::snapshot::snapshot(&self.style, &self.colors)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::design_system::color_scheme::builtin_light;
    use crate::design_system::style_system::StyleSystem;

    #[test]
    fn registry_is_not_empty() {
        let reg = ThemeRegistry::with_builtins();
        assert!(!reg.style_ids().is_empty(), "registry must have at least one style");
        assert!(!reg.color_ids().is_empty(), "registry must have at least one color scheme");
    }

    #[test]
    fn set_style_swaps_active() {
        let mut reg = ThemeRegistry::with_builtins();
        // Default active is "apex-default"
        assert_eq!(reg.active_style().meta.id, "apex-default");

        // Swap to "meridien"
        let found = reg.set_style("meridien");
        assert!(found, "meridien must be registered as built-in");
        assert_eq!(reg.active_style().meta.id, "meridien");
    }

    #[test]
    fn set_colors_swaps_active() {
        let mut reg = ThemeRegistry::with_builtins();
        assert_eq!(reg.active_colors().meta.id, "apex-dark");

        let found = reg.set_colors("apex-light");
        assert!(found, "apex-light must be registered as built-in");
        assert_eq!(reg.active_colors().meta.id, "apex-light");
    }

    #[test]
    fn set_unknown_id_returns_false() {
        let mut reg = ThemeRegistry::with_builtins();
        let found = reg.set_style("non-existent-style");
        assert!(!found);
        // Active should not have changed
        assert_eq!(reg.active_style().meta.id, "apex-default");
    }

    #[test]
    fn register_replaces_existing() {
        let mut reg = ThemeRegistry::with_builtins();
        let mut custom = StyleSystem::builtin_default();
        custom.typography.size_md = 99.0;  // sentinel value
        reg.register_style(custom);

        reg.set_style("apex-default");
        assert_eq!(reg.active_style().typography.size_md, 99.0);
    }

    #[test]
    fn active_theme_snapshot() {
        let reg = ThemeRegistry::with_builtins();
        let theme = ActiveTheme::from_registry(&reg);
        let snap = theme.snapshot();
        // Should match the default style's size_sm
        assert_eq!(snap.size_sm, reg.active_style().typography.size_sm);
        // Should match the active palette's accent
        assert_eq!(snap.accent, reg.active_colors().accent);
    }

    #[test]
    fn active_pair_is_independent() {
        let mut reg = ThemeRegistry::with_builtins();
        // Mix Meridien style with light palette
        reg.set_style("meridien");
        reg.set_colors("apex-light");

        let (style, colors) = reg.active();
        assert_eq!(style.meta.id,  "meridien");
        assert_eq!(colors.meta.id, "apex-light");
        // Meridien is marked is_dark; light palette is not — they are independent axes
        assert!(style.meta.is_dark);
        assert!(!colors.meta.is_dark);
    }
}
