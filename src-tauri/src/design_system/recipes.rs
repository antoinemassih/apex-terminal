//! `RecipeSet` — a keyed map of [`RecipeSpec`]s, the serialisable per-component
//! style override layer.
//!
//! # Purpose
//!
//! A `RecipeSet` is the runtime owner of all theme-author style overrides. It
//! maps namespaced string keys (e.g. `"button.primary"`, `"row.list.selected"`)
//! to [`RecipeSpec`]s. It is loaded from a theme's `recipes.json` / `recipes.toml`
//! at startup and hot-reloaded when the file changes.
//!
//! # Resolution chain
//!
//! ```text
//! RecipeSet.get(key)   →   RecipeSpec.apply_over(widget_default_Sx, theme)
//!                      →   resolved Sx used for painting
//!
//! Missing key:         →   widget_default_Sx used as-is (no change)
//! ```
//!
//! # Caching
//!
//! `RecipeSpec → Sx` conversion is designed to be done **once per frame per key**
//! (not per-widget-instance). `RecipeSet` exposes a `resolve_cached` method that
//! takes a mutable `HashMap<RecipeKey, Sx>` (the frame cache, owned by the caller
//! or stored on a per-theme state object). On the first call for a key in a frame,
//! the spec is converted and stored; subsequent calls (e.g. two buttons with the
//! same key) are a map lookup.
//!
//! Typical wiring (pseudocode):
//!
//! ```ignore
//! // Once per frame, before rendering:
//! frame_cache.clear();
//!
//! // Per widget, at render time:
//! let sx = recipe_set.resolve_cached(
//!     "button.primary",
//!     || button_default_sx(),  // widget built-in default
//!     theme,
//!     &mut frame_cache,
//! );
//! sx.show_ct(ui, theme, sense, body);
//! ```
//!
//! # Key registry
//!
//! See `docs/migration/recipe-keys.md` for the canonical append-only key list.
//! Keys follow the convention `<component>.<variant>.<state>` (nested dots),
//! where `<variant>` and `<state>` are optional.

use std::collections::HashMap;
use serde::{Deserialize, Serialize};

use crate::ui_kit::sx::recipe_spec::{RecipeKey, RecipeSpec};
use crate::ui_kit::sx::style::Sx;
use crate::ui_kit::widgets::theme::ComponentTheme;

// ── RecipeSet ─────────────────────────────────────────────────────────────────

/// A keyed map of [`RecipeSpec`]s. Serialisable so it can be loaded from
/// `recipes.json` in a theme directory and hot-reloaded at runtime.
///
/// Keys are namespaced strings (e.g. `"button.primary"`, `"row.list"`).
/// The map is **append-only** — keys are never renamed or removed to preserve
/// backward compatibility with persisted recipe files.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RecipeSet {
    #[serde(flatten)]
    specs: HashMap<RecipeKey, RecipeSpec>,
}

impl RecipeSet {
    /// Create an empty set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert or replace a recipe for the given key.
    pub fn insert(&mut self, key: impl Into<RecipeKey>, spec: RecipeSpec) {
        self.specs.insert(key.into(), spec);
    }

    /// Look up a recipe by key. Returns `None` when the key is not registered,
    /// which signals the widget to use its built-in default `Sx` unchanged.
    pub fn get(&self, key: &str) -> Option<&RecipeSpec> {
        self.specs.get(key)
    }

    /// Resolve `key` to an `Sx` by merging the recipe (if any) over `default_sx`.
    ///
    /// - **Key found:** returns `spec.apply_over(default_sx, theme)`.
    /// - **Key absent:** returns `default_sx` unchanged.
    ///
    /// This is the single-call resolution entry point for widgets. It allocates
    /// nothing beyond what `palette_ct` (thread-local cache) needs on first use.
    pub fn resolve(
        &self,
        key: &str,
        default_sx: Sx,
        theme: &dyn ComponentTheme,
    ) -> Sx {
        match self.specs.get(key) {
            Some(spec) => spec.apply_over(default_sx, theme),
            None => default_sx,
        }
    }

    /// Resolve `key` with a per-frame resolved-`Sx` cache (`frame_cache`).
    ///
    /// `default_fn` is called (lazily) only on cache miss — it produces the
    /// widget's built-in default [`Sx`]. The resolved `Sx` is stored in
    /// `frame_cache` so that N widgets sharing the same key incur only one
    /// `RecipeSpec → Sx` conversion per frame.
    ///
    /// **Caller contract:** clear `frame_cache` once per frame before rendering
    /// (typically in the `begin_frame` hook or the render root). The cache is
    /// intentionally NOT stored inside `RecipeSet` so the set itself remains
    /// `Clone + Send + Sync` (no interior mutability).
    ///
    /// ```ignore
    /// // In begin_frame or render root:
    /// let mut sx_cache: HashMap<String, Sx> = HashMap::new();
    ///
    /// // In each widget's render:
    /// let sx = recipe_set.resolve_cached(
    ///     "button.primary",
    ///     || Sx::new().rounded_md().bg(Tone::Accent),
    ///     theme,
    ///     &mut sx_cache,
    /// );
    /// ```
    pub fn resolve_cached(
        &self,
        key: &str,
        default_fn: impl FnOnce() -> Sx,
        theme: &dyn ComponentTheme,
        frame_cache: &mut HashMap<String, Sx>,
    ) -> Sx {
        if let Some(&sx) = frame_cache.get(key) {
            return sx;
        }
        let sx = self.resolve(key, default_fn(), theme);
        frame_cache.insert(key.to_string(), sx);
        sx
    }

    /// Number of registered recipes.
    pub fn len(&self) -> usize {
        self.specs.len()
    }

    /// True when no recipes are registered (the empty/default case).
    pub fn is_empty(&self) -> bool {
        self.specs.is_empty()
    }

    /// Merge another `RecipeSet` on top of this one. Keys in `other` replace
    /// matching keys in `self`; keys absent from `other` are kept. Used when
    /// layering a per-pane recipe override on top of a global set.
    pub fn merge_from(&mut self, other: &RecipeSet) {
        for (key, spec) in &other.specs {
            self.specs.insert(key.clone(), spec.clone());
        }
    }
}

// ── SxCache type alias ────────────────────────────────────────────────────────

/// Convenience type alias for the per-frame resolved-`Sx` cache.
/// Declared here so callers can `use design_system::recipes::SxCache` instead
/// of spelling out the full type.
pub type SxCache = HashMap<String, Sx>;

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui_kit::sx::color::Tone;
    use crate::ui_kit::sx::recipe_spec::{
        ColorSpec, PadTier, RadiusTier, RecipeDelta, TextSizeTier, TextSpec, ToneRef, ShadeRef,
    };
    use crate::ui_kit::sx::style::StyleState;

    // Minimal ComponentTheme for unit tests.
    struct MockTheme;

    impl ComponentTheme for MockTheme {
        fn accent(&self)           -> egui::Color32 { egui::Color32::from_rgb(99, 102, 241) }
        fn bull(&self)             -> egui::Color32 { egui::Color32::from_rgb(52, 211, 153) }
        fn bear(&self)             -> egui::Color32 { egui::Color32::from_rgb(248, 113, 113) }
        fn warn(&self)             -> egui::Color32 { egui::Color32::from_rgb(251, 191, 36) }
        fn text(&self)             -> egui::Color32 { egui::Color32::from_rgb(220, 220, 220) }
        fn dim(&self)              -> egui::Color32 { egui::Color32::from_rgb(120, 120, 120) }
        fn border(&self)           -> egui::Color32 { egui::Color32::from_rgb(55, 55, 55) }
        fn border_variant(&self)   -> egui::Color32 { egui::Color32::from_rgb(70, 70, 70) }
        fn surface(&self)          -> egui::Color32 { egui::Color32::from_rgb(28, 28, 28) }
        fn bg(&self)               -> egui::Color32 { egui::Color32::from_rgb(18, 18, 18) }
        fn element_hover(&self)    -> egui::Color32 { egui::Color32::from_rgba_unmultiplied(255, 255, 255, 12) }
        fn element_active(&self)   -> egui::Color32 { egui::Color32::from_rgba_unmultiplied(255, 255, 255, 20) }
        fn element_selected(&self) -> egui::Color32 { egui::Color32::from_rgba_unmultiplied(99, 102, 241, 40) }
        fn element_disabled(&self) -> egui::Color32 { egui::Color32::from_rgba_unmultiplied(255, 255, 255, 6) }
        fn ghost_hover(&self)      -> egui::Color32 { egui::Color32::from_rgba_unmultiplied(255, 255, 255, 8) }
        fn ghost_active(&self)     -> egui::Color32 { egui::Color32::from_rgba_unmultiplied(255, 255, 255, 14) }
        fn icon(&self)             -> egui::Color32 { egui::Color32::from_rgb(180, 180, 180) }
        fn icon_muted(&self)       -> egui::Color32 { egui::Color32::from_rgb(100, 100, 100) }
        fn icon_disabled(&self)    -> egui::Color32 { egui::Color32::from_rgb(60, 60, 60) }
        fn icon_accent(&self)      -> egui::Color32 { egui::Color32::from_rgb(99, 102, 241) }
    }

    fn make_button_primary_recipe() -> RecipeSpec {
        RecipeSpec {
            base: RecipeDelta {
                radius: Some(RadiusTier::Pill),
                fill: Some(ColorSpec::Tone { tone: ToneRef::Accent, shade: Some(ShadeRef::S500) }),
                px: Some(PadTier::Lg),
                py: Some(PadTier::Sm),
                text: Some(TextSpec { tone: ToneRef::Bg, shade: Some(ShadeRef::S50) }),
                text_size: Some(TextSizeTier::Sm),
                ..Default::default()
            },
            hover: Some(RecipeDelta {
                fill: Some(ColorSpec::Tone { tone: ToneRef::Accent, shade: Some(ShadeRef::S400) }),
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    fn make_row_list_recipe() -> RecipeSpec {
        RecipeSpec {
            base: RecipeDelta {
                radius: Some(RadiusTier::None),
                border: None, // explicitly no border
                py: Some(PadTier::Xs),
                ..Default::default()
            },
            selected: Some(RecipeDelta {
                fill: Some(ColorSpec::Alpha { tone: ToneRef::Accent, alpha: 28 }),
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    #[test]
    fn missing_key_returns_default_unchanged() {
        let set = RecipeSet::new();
        let t = MockTheme;
        let default_sx = Sx::new().rounded_md().bg(Tone::Surface);

        let result = set.resolve("button.primary", default_sx, &t);
        // Should equal the default. We compare the resolved deltas for Normal state.
        let default_delta = default_sx.resolved(StyleState::Normal);
        let result_delta  = result.resolved(StyleState::Normal);

        assert_eq!(default_delta.radius, result_delta.radius,
            "missing key must not change radius");
        assert!(result_delta.fill.is_some(),
            "missing key must preserve fill from default");
    }

    #[test]
    fn button_primary_recipe_overrides_radius_and_fill() {
        let mut set = RecipeSet::new();
        set.insert("button.primary", make_button_primary_recipe());
        let t = MockTheme;

        // Widget default: small radius, accent fill at S600.
        let default_sx = Sx::new().rounded_sm().bg_shade(Tone::Accent, crate::ui_kit::sx::color::Shade::S600);

        let result = set.resolve("button.primary", default_sx, &t);
        let delta = result.resolved(StyleState::Normal);

        // Recipe overrides radius to Pill (999.0).
        assert_eq!(delta.radius, Some(999.0),
            "recipe should set radius to pill (999)");

        // Recipe overrides fill (Accent S500 base).
        assert!(delta.fill.is_some(), "recipe should set fill");
    }

    #[test]
    fn button_primary_recipe_hover_differs_from_normal() {
        let mut set = RecipeSet::new();
        set.insert("button.primary", make_button_primary_recipe());
        let t = MockTheme;

        let default_sx = Sx::new().rounded_sm();
        let result = set.resolve("button.primary", default_sx, &t);

        let normal = result.resolved(StyleState::Normal);
        let hovered = result.resolved(StyleState::Hover);

        // Both have fill; the hover fill should differ (S400 vs S500).
        assert!(normal.fill.is_some() && hovered.fill.is_some(),
            "both normal and hover should have fill");

        // Resolve the actual Color32 values to compare.
        let pal = crate::ui_kit::sx::color::palette_ct(&t);
        let normal_color = match normal.fill.unwrap() {
            crate::ui_kit::sx::style::Fill::Solid(c) => c,
            crate::ui_kit::sx::style::Fill::Shade(tone, shade) => pal.shade(tone, shade),
            crate::ui_kit::sx::style::Fill::Alpha(tone, a) => {
                let b = pal.base(tone);
                egui::Color32::from_rgba_unmultiplied(b.r(), b.g(), b.b(), a)
            }
        };
        let hover_color = match hovered.fill.unwrap() {
            crate::ui_kit::sx::style::Fill::Solid(c) => c,
            crate::ui_kit::sx::style::Fill::Shade(tone, shade) => pal.shade(tone, shade),
            crate::ui_kit::sx::style::Fill::Alpha(tone, a) => {
                let b = pal.base(tone);
                egui::Color32::from_rgba_unmultiplied(b.r(), b.g(), b.b(), a)
            }
        };

        assert_ne!(normal_color, hover_color,
            "hover fill (S400) should differ from normal fill (S500)");
    }

    #[test]
    fn row_list_recipe_flush_and_square() {
        let mut set = RecipeSet::new();
        set.insert("row.list", make_row_list_recipe());
        let t = MockTheme;

        // Widget default: has some radius and a hairline border.
        let default_sx = Sx::new()
            .rounded_sm()
            .border_thin(Tone::Border);

        let result = set.resolve("row.list", default_sx, &t);
        let delta = result.resolved(StyleState::Normal);

        // Recipe overrides radius to None (0.0 = square).
        assert_eq!(delta.radius, Some(0.0), "row.list recipe should set radius=0 (square)");
    }

    #[test]
    fn row_list_selected_state_has_tinted_fill() {
        let mut set = RecipeSet::new();
        set.insert("row.list", make_row_list_recipe());
        let t = MockTheme;

        let default_sx = Sx::new().rounded_sm();
        let result = set.resolve("row.list", default_sx, &t);

        // selected maps to Active in Sx.
        let normal  = result.resolved(StyleState::Normal);
        let active  = result.resolved(StyleState::Active);

        // Normal has no fill (default_sx has none, recipe base has none).
        assert!(normal.fill.is_none(), "row.list normal should have no fill");
        // Active/selected has Accent alpha tint.
        assert!(active.fill.is_some(), "row.list selected should have tinted fill");
    }

    #[test]
    fn frame_cache_prevents_double_conversion() {
        let mut set = RecipeSet::new();
        set.insert("button.primary", make_button_primary_recipe());
        let t = MockTheme;
        let mut cache: SxCache = HashMap::new();

        let default_sx = Sx::new().rounded_sm();

        // First call: cache miss, inserts.
        let sx1 = set.resolve_cached("button.primary", || default_sx, &t, &mut cache);
        assert_eq!(cache.len(), 1);

        // Second call: cache hit (default_fn NOT called because closure is consumed,
        // but the resolved Sx should be identical).
        let sx2 = set.resolve_cached("button.primary", || {
            panic!("default_fn must not be called on cache hit")
        }, &t, &mut cache);

        // Both should produce the same resolved normal delta.
        let d1 = sx1.resolved(StyleState::Normal);
        let d2 = sx2.resolved(StyleState::Normal);
        assert_eq!(d1.radius, d2.radius, "cached sx should match first resolution");
    }

    #[test]
    fn recipe_set_json_round_trip() {
        let mut set = RecipeSet::new();
        set.insert("button.primary", make_button_primary_recipe());
        set.insert("row.list", make_row_list_recipe());

        let json = serde_json::to_string_pretty(&set).expect("serialise");
        let back: RecipeSet = serde_json::from_str(&json).expect("deserialise");

        assert_eq!(back.len(), 2);
        assert!(back.get("button.primary").is_some());
        assert!(back.get("row.list").is_some());
        assert!(back.get("nav.cluster").is_none());
    }

    #[test]
    fn merge_from_overlays_keys() {
        let mut base = RecipeSet::new();
        base.insert("button.primary", make_button_primary_recipe());
        base.insert("row.list", make_row_list_recipe());

        let mut overlay = RecipeSet::new();
        overlay.insert("row.list", RecipeSpec {
            base: RecipeDelta {
                // Override: rounded instead of square.
                radius: Some(RadiusTier::Sm),
                ..Default::default()
            },
            ..Default::default()
        });
        overlay.insert("nav.cluster", RecipeSpec::default());

        base.merge_from(&overlay);

        assert_eq!(base.len(), 3, "merge should add nav.cluster");
        let t = MockTheme;
        let row_result = base.resolve("row.list", Sx::new(), &t);
        let delta = row_result.resolved(StyleState::Normal);
        // After overlay, radius should be Sm (not None from original row.list recipe).
        assert!(delta.radius.is_some());
        assert!(delta.radius.unwrap() > 0.0, "overlay should have changed radius from 0 to Sm");
    }
}
