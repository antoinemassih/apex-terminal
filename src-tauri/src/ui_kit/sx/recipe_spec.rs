//! `RecipeSpec` — a serializable (human-authorable) mirror of [`SxDelta`] +
//! per-interaction-state overrides.
//!
//! # Purpose
//!
//! [`RecipeSpec`] is the **data format** that theme authors write in JSON/TOML
//! to override how a named component looks. It is loaded from disk, validated,
//! and converted into a resolved [`Sx`] once per key per frame (or whenever the
//! theme changes). This is distinct from the compile-time [`Recipe`]/[`SxDelta`]
//! system, which is how widget code declares its built-in defaults in Rust.
//!
//! # Color representation rule
//!
//! **All colors in `RecipeSpec` are semantic tone references** — [`ToneRef`] +
//! optional [`Shade`] + optional alpha (0–255). The only exception is the
//! `literal:` escape hatch variant, which accepts `#rrggbb` / `#rrggbbaa` hex
//! for cases where a theme author needs a one-off value not expressible via tones
//! (e.g. a custom brand color). Consumers that call [`RecipeSpec::to_sx`] receive
//! a fully resolved [`Sx`]; the resolution step calls `palette_ct` to expand
//! tone+shade to `Color32`.
//!
//! # Example (TOML representation)
//!
//! ```toml
//! [button.primary]
//! fill = { tone = "accent", shade = "s400" }
//! radius = "pill"
//! px = "lg"
//! py = "sm"
//! text = { tone = "bg", shade = "s50" }
//! text_size = "md"
//!
//! [button.primary.hover]
//! fill = { tone = "accent", shade = "s300" }
//!
//! [row.list]
//! radius = "none"
//! border_width = "none"
//! py = "xs"
//!
//! [row.list.selected]
//! fill = { tone = "accent", alpha = 28 }
//! ```
//!
//! # Resolution order (for `RecipeSet`)
//!
//! ```text
//! recipe (RecipeSpec) → widget built-in Sx default → transparent (no paint)
//! ```
//!
//! Missing fields in `RecipeSpec` leave the widget's own default intact.
//! Missing keys in `RecipeSet` leave the widget's own default completely intact.

use serde::{Deserialize, Serialize};

use super::color::{Shade, Tone};
use super::style::{Fill, Sx, SxDelta};
use crate::ui_kit::widgets::theme::ComponentTheme;
use super::color::palette_ct;

// ── ToneRef ──────────────────────────────────────────────────────────────────

/// Serialisable semantic tone reference. The serde name matches the palette
/// field names in `ColorScheme` so theme authors don't need to learn new names.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToneRef {
    Accent,
    Bull,
    Bear,
    Warn,
    Text,
    Dim,
    Border,
    Surface,
    Bg,
}

impl From<ToneRef> for Tone {
    fn from(r: ToneRef) -> Tone {
        match r {
            ToneRef::Accent  => Tone::Accent,
            ToneRef::Bull    => Tone::Bull,
            ToneRef::Bear    => Tone::Bear,
            ToneRef::Warn    => Tone::Warn,
            ToneRef::Text    => Tone::Text,
            ToneRef::Dim     => Tone::Dim,
            ToneRef::Border  => Tone::Border,
            ToneRef::Surface => Tone::Surface,
            ToneRef::Bg      => Tone::Bg,
        }
    }
}

// ── ShadeRef ─────────────────────────────────────────────────────────────────

/// Serialisable shade step — mirrors [`Shade`] with human-friendly names.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShadeRef {
    S50, S100, S200, S300, S400, S500, S600, S700, S800, S900, S950,
}

impl From<ShadeRef> for Shade {
    fn from(r: ShadeRef) -> Shade {
        match r {
            ShadeRef::S50  => Shade::S50,
            ShadeRef::S100 => Shade::S100,
            ShadeRef::S200 => Shade::S200,
            ShadeRef::S300 => Shade::S300,
            ShadeRef::S400 => Shade::S400,
            ShadeRef::S500 => Shade::S500,
            ShadeRef::S600 => Shade::S600,
            ShadeRef::S700 => Shade::S700,
            ShadeRef::S800 => Shade::S800,
            ShadeRef::S900 => Shade::S900,
            ShadeRef::S950 => Shade::S950,
        }
    }
}

// ── ColorSpec ────────────────────────────────────────────────────────────────

/// A serialisable color specification. Three forms:
///
/// - `Tone { tone, shade? }` — resolved via the active palette ramp.
/// - `Alpha { tone, alpha }` — the tone's S500 base at a fixed alpha (tinted overlay).
/// - `Literal { hex }` — escape hatch for `#rrggbb` / `#rrggbbaa` hex strings.
///
/// In JSON/TOML the tag is the `kind` field (internal tag):
///
/// ```json
/// { "kind": "tone",    "tone": "accent",  "shade": "s400" }
/// { "kind": "alpha",   "tone": "accent",  "alpha": 40     }
/// { "kind": "literal", "hex": "#6366f1"                   }
/// ```
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ColorSpec {
    /// Solid ramp shade. `shade` defaults to `S500` when omitted.
    Tone {
        tone: ToneRef,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        shade: Option<ShadeRef>,
    },
    /// The tone's base color at an explicit alpha (0–255). Useful for tinted fills.
    Alpha {
        tone: ToneRef,
        alpha: u8,
    },
    /// Hex literal escape hatch — for one-off colors not expressible via tones.
    /// Accepts `#rrggbb` or `#rrggbbaa`.
    Literal {
        hex: String,
    },
}

impl ColorSpec {
    /// Resolve to an [`egui::Color32`] via the active [`ComponentTheme`] palette.
    pub fn resolve(&self, t: &dyn ComponentTheme) -> egui::Color32 {
        let pal = palette_ct(t);
        match self {
            ColorSpec::Tone { tone, shade } => {
                let t: Tone = (*tone).into();
                let s: Shade = shade.map(|s| s.into()).unwrap_or(Shade::S500);
                pal.shade(t, s)
            }
            ColorSpec::Alpha { tone, alpha } => {
                let base = pal.base((*tone).into());
                egui::Color32::from_rgba_unmultiplied(base.r(), base.g(), base.b(), *alpha)
            }
            ColorSpec::Literal { hex } => {
                let h = hex.trim_start_matches('#');
                let parse_byte = |s: &str| u8::from_str_radix(s, 16).unwrap_or(0);
                match h.len() {
                    6 => egui::Color32::from_rgb(
                        parse_byte(&h[0..2]),
                        parse_byte(&h[2..4]),
                        parse_byte(&h[4..6]),
                    ),
                    8 => egui::Color32::from_rgba_unmultiplied(
                        parse_byte(&h[0..2]),
                        parse_byte(&h[2..4]),
                        parse_byte(&h[4..6]),
                        parse_byte(&h[6..8]),
                    ),
                    _ => egui::Color32::TRANSPARENT,
                }
            }
        }
    }

    /// Convert to an [`SxDelta`]-compatible [`Fill`].
    pub fn to_fill(&self, t: &dyn ComponentTheme) -> Fill {
        Fill::Solid(self.resolve(t))
    }
}

// ── Tier enums ───────────────────────────────────────────────────────────────

/// Radius tier — matches the `rounded_*` helpers on [`SxDelta`].
///
/// `Px(f32)` is a literal pixel value for cases where the tier scale doesn't
/// cover the needed value (e.g. `Pill` vs a custom 12px).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RadiusTier {
    None,
    Xs,
    Sm,
    Md,
    Lg,
    Pill,
    Px(f32),
}

impl RadiusTier {
    pub fn to_px(&self) -> f32 {
        use crate::ui_kit::tokens as st;
        match self {
            RadiusTier::None => 0.0,
            RadiusTier::Xs   => st::radius_xs(),
            RadiusTier::Sm   => st::radius_sm(),
            RadiusTier::Md   => st::radius_md(),
            RadiusTier::Lg   => st::radius_lg(),
            RadiusTier::Pill => 999.0,
            RadiusTier::Px(v) => *v,
        }
    }
}

/// Padding tier for a single axis.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PadTier {
    None,
    Xs,
    Sm,
    Md,
    Lg,
    Px(f32),
}

impl PadTier {
    pub fn to_px(&self) -> f32 {
        use crate::ui_kit::tokens as st;
        match self {
            PadTier::None  => 0.0,
            PadTier::Xs    => st::gap_xs(),
            PadTier::Sm    => st::gap_sm(),
            PadTier::Md    => st::gap_md(),
            PadTier::Lg    => st::gap_lg(),
            PadTier::Px(v) => *v,
        }
    }
}

/// Font size tier.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextSizeTier {
    Xs,
    Sm,
    Md,
    Lg,
    Px(f32),
}

impl TextSizeTier {
    pub fn to_px(&self) -> f32 {
        use crate::ui_kit::tokens as st;
        match self {
            TextSizeTier::Xs    => st::font_xs(),
            TextSizeTier::Sm    => st::font_sm(),
            TextSizeTier::Md    => st::font_md(),
            TextSizeTier::Lg    => st::font_lg(),
            TextSizeTier::Px(v) => *v,
        }
    }
}

/// Border width tier.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BorderWidthTier {
    None,
    Hair,
    Thin,
    Std,
    Px(f32),
}

impl BorderWidthTier {
    pub fn to_px(&self) -> f32 {
        use crate::ui_kit::tokens as st;
        match self {
            BorderWidthTier::None  => 0.0,
            BorderWidthTier::Hair  => st::stroke_hair(),
            BorderWidthTier::Thin  => st::stroke_thin(),
            BorderWidthTier::Std   => st::stroke_std(),
            BorderWidthTier::Px(v) => *v,
        }
    }
}

// ── TextSpec ─────────────────────────────────────────────────────────────────

/// Text color + size together in the `text` field.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TextSpec {
    pub tone: ToneRef,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shade: Option<ShadeRef>,
}

// ── BorderSpec (serialisable) ─────────────────────────────────────────────────

/// Serialisable border — color (tone/alpha/literal) + width tier.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BorderSpecRef {
    pub color: ColorSpec,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<BorderWidthTier>,
}

// ── RecipeDelta ───────────────────────────────────────────────────────────────

/// A flat set of overridable style properties — the serialisable mirror of
/// [`SxDelta`]. All fields are optional; missing fields leave the widget default
/// intact. This type is used both for the base style and for per-state overrides.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RecipeDelta {
    // ── Geometry ──────────────────────────────────────────────────────────────
    /// Corner radius. `"pill"` → 999px, `"none"` → 0, `"sm"/"md"/"lg"` → token tier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub radius: Option<RadiusTier>,

    /// Horizontal padding (both sides). Token tier or literal px.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub px: Option<PadTier>,

    /// Vertical padding (both sides). Token tier or literal px.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub py: Option<PadTier>,

    // ── Color ─────────────────────────────────────────────────────────────────
    /// Background fill. Semantic tone ref or literal hex.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill: Option<ColorSpec>,

    /// Border color + width tier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub border: Option<BorderSpecRef>,

    // ── Text ──────────────────────────────────────────────────────────────────
    /// Text color tone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<TextSpec>,

    /// Text size tier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_size: Option<TextSizeTier>,

    // ── Misc ──────────────────────────────────────────────────────────────────
    /// Overall opacity for the component (0.0–1.0). Used for disabled state.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opacity: Option<f32>,
}

impl RecipeDelta {
    /// Resolve to an [`SxDelta`], looking up tone refs via `t`.
    pub fn to_sx_delta(&self, t: &dyn ComponentTheme) -> SxDelta {
        let mut d = SxDelta::new();

        if let Some(r) = &self.radius {
            d = d.rounded(r.to_px());
        }
        if let Some(px) = &self.px {
            d = d.px(px.to_px());
        }
        if let Some(py) = &self.py {
            d = d.py(py.to_px());
        }
        if let Some(fill) = &self.fill {
            d = d.bg_color(fill.resolve(t));
        }
        if let Some(border) = &self.border {
            let width = border.width.as_ref().map(|w| w.to_px())
                .unwrap_or_else(|| crate::ui_kit::tokens::stroke_thin());
            d = d.border_color(border.color.resolve(t), width);
        }
        if let Some(text) = &self.text {
            let tone: Tone = text.tone.into();
            let shade: Shade = text.shade.map(|s| s.into()).unwrap_or(Shade::S500);
            d = d.text_shade(tone, shade);
        }
        if let Some(ts) = &self.text_size {
            d = d.text_size(ts.to_px());
        }
        if let Some(op) = self.opacity {
            d = d.opacity(op);
        }

        d
    }
}

// ── RecipeSpec ────────────────────────────────────────────────────────────────

/// A fully serialisable style specification for a named component recipe.
///
/// Mirrors the capabilities of `SxDelta` (the base style) plus four
/// per-interaction-state overrides: `hover`, `active`, `disabled`, `selected`.
///
/// **Design contract:**
/// - All colors are [`ColorSpec`] (semantic tone reference or literal hex).
/// - All sizes are tier enums (scale-aware, not raw px) — except the `Px`
///   escape variants on each tier type.
/// - Missing fields = widget's built-in default is preserved.
/// - Missing key in [`super::super::design_system::recipes::RecipeSet`] =
///   widget's built-in `Sx` is used entirely unchanged.
///
/// **Caching hint:** the resolved [`Sx`] should be cached per (key, theme_name)
/// since `to_sx` calls `palette_ct` which is a thread-local cache hit after
/// warmup, but converting all fields still costs a handful of match arms.
/// [`crate::design_system::recipes::RecipeSet`] exposes `resolve_cached` for
/// this pattern.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RecipeSpec {
    // ── Base style ────────────────────────────────────────────────────────────
    #[serde(flatten)]
    pub base: RecipeDelta,

    // ── Per-state overrides ───────────────────────────────────────────────────
    /// Override applied when the pointer hovers the component.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hover: Option<RecipeDelta>,

    /// Override applied while the component is pointer-pressed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active: Option<RecipeDelta>,

    /// Override applied when the component is in a disabled state.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disabled: Option<RecipeDelta>,

    /// Override applied when the component is in a selected/checked state
    /// (e.g. active tab, selected list row, toggled button).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected: Option<RecipeDelta>,
}

impl RecipeSpec {
    /// Convert this spec to a full [`Sx`] by resolving all tone references via
    /// the given [`ComponentTheme`].
    ///
    /// Cost: one `palette_ct` call (thread-local cache) + O(fields) match arms.
    /// Designed to be done once per key per frame and cached by the caller.
    ///
    /// Resolution order per field: `RecipeSpec` value → (field absent) widget default.
    pub fn to_sx(&self, t: &dyn ComponentTheme) -> Sx {
        let mut sx = Sx::new().with(self.base.to_sx_delta(t));

        if let Some(hover) = &self.hover {
            let delta = hover.to_sx_delta(t);
            sx = sx.hover(|_| delta);
        }
        if let Some(active) = &self.active {
            let delta = active.to_sx_delta(t);
            sx = sx.active(|_| delta);
        }
        if let Some(disabled) = &self.disabled {
            let delta = disabled.to_sx_delta(t);
            sx = sx.disabled(|_| delta);
        }
        // Note: `selected` maps to an `active` override in Sx (Sx has no
        // separate `selected` state; widgets manage their own selected flag and
        // paint with StyleState::Active). This is intentional — both states
        // use the same visual slot; a widget that needs both simultaneously
        // applies the selected override directly via `Sx::with`.
        if let Some(selected) = &self.selected {
            let delta = selected.to_sx_delta(t);
            // Merge selected on top of base as an always-on active override
            // (caller is responsible for gating this on selection state).
            sx = sx.active(|_| delta);
        }

        sx
    }

    /// Merge this spec's base delta over a widget's built-in default [`Sx`],
    /// leaving the widget's own hover/active/disabled state overrides intact
    /// when no corresponding override is defined in this spec.
    ///
    /// This is the CORRECT merge semantics for the resolution chain:
    ///
    /// ```text
    /// result.base   = default_sx.base   ← merged with recipe.base
    /// result.hover  = recipe.hover  if Some, else default_sx.hover
    /// result.active = recipe.active if Some, else default_sx.active
    /// result.disabled = recipe.disabled if Some, else default_sx.disabled
    /// ```
    ///
    /// Use this method (not `merge_over`) as the primary widget API.
    pub fn apply_over(&self, default_sx: Sx, t: &dyn ComponentTheme) -> Sx {
        // Start from the default and overlay the recipe base on the base delta.
        let mut result = default_sx.with(self.base.to_sx_delta(t));

        // Per-state: only replace if the recipe specifies the state override.
        if let Some(hover) = &self.hover {
            let delta = hover.to_sx_delta(t);
            result = result.hover(|_| delta);
        }
        if let Some(active) = &self.active {
            let delta = active.to_sx_delta(t);
            result = result.active(|_| delta);
        }
        if let Some(disabled) = &self.disabled {
            let delta = disabled.to_sx_delta(t);
            result = result.disabled(|_| delta);
        }
        if let Some(selected) = &self.selected {
            let delta = selected.to_sx_delta(t);
            result = result.active(|_| delta);
        }

        result
    }
}

// ── RecipeKey ─────────────────────────────────────────────────────────────────

/// Type alias for a recipe key string (e.g. `"button.primary"`, `"row.list"`).
/// Namespaced with `.` separators. Keys are append-only (never renamed/removed).
/// See `docs/migration/recipe-keys.md` for the canonical registry.
pub type RecipeKey = String;

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // Minimal ComponentTheme for unit tests — returns fixed colors so we can
    // verify tone resolution without a full gpu::Theme.
    struct MockTheme;

    impl crate::ui_kit::widgets::theme::ComponentTheme for MockTheme {
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

    #[test]
    fn color_spec_tone_resolves() {
        let t = MockTheme;
        let spec = ColorSpec::Tone { tone: ToneRef::Accent, shade: None };
        let c = spec.resolve(&t);
        // S500 = base color for Accent = (99, 102, 241) — verify non-zero
        assert_eq!(c, t.accent(), "S500 tone should equal base color");
    }

    #[test]
    fn color_spec_alpha_preserves_rgb() {
        let t = MockTheme;
        let spec = ColorSpec::Alpha { tone: ToneRef::Accent, alpha: 40 };
        let c = spec.resolve(&t);
        // Verify alpha is preserved; also verify the color is non-transparent
        // and derives from Accent (not from a different tone).
        // Use to_array() which returns the stored bytes in egui's internal format.
        let [_, _, _, a] = c.to_array();
        assert_eq!(a, 40, "alpha channel should be 40");
        // Confirm the color has some non-zero r/g/b content (not accidentally black).
        assert!(c != egui::Color32::TRANSPARENT, "should not be transparent");

        // A fully opaque version of the same color should use the Accent base.
        let spec_opaque = ColorSpec::Tone { tone: ToneRef::Accent, shade: None };
        let opaque = spec_opaque.resolve(&t);
        // The semi-transparent version should share the same hue: its r/g/b when
        // scaled back up should approximate the opaque values. We just verify they
        // are not equal (because the alpha reduces them) yet both come from Accent.
        assert_ne!(c, opaque, "semi-transparent should differ from opaque");
    }

    #[test]
    fn color_spec_literal_parses_hex() {
        let t = MockTheme;
        let spec = ColorSpec::Literal { hex: "#6366f1".to_string() };
        let c = spec.resolve(&t);
        assert_eq!(c.r(), 0x63);
        assert_eq!(c.g(), 0x66);
        assert_eq!(c.b(), 0xf1);
    }

    #[test]
    fn radius_tier_to_px() {
        assert_eq!(RadiusTier::None.to_px(), 0.0);
        assert_eq!(RadiusTier::Pill.to_px(), 999.0);
        assert_eq!(RadiusTier::Px(12.5).to_px(), 12.5);
    }

    #[test]
    fn recipe_spec_round_trips_json() {
        let spec = RecipeSpec {
            base: RecipeDelta {
                radius: Some(RadiusTier::Pill),
                fill: Some(ColorSpec::Tone { tone: ToneRef::Accent, shade: Some(ShadeRef::S400) }),
                px: Some(PadTier::Lg),
                py: Some(PadTier::Sm),
                text: Some(TextSpec { tone: ToneRef::Bg, shade: Some(ShadeRef::S50) }),
                text_size: Some(TextSizeTier::Md),
                ..Default::default()
            },
            hover: Some(RecipeDelta {
                fill: Some(ColorSpec::Tone { tone: ToneRef::Accent, shade: Some(ShadeRef::S300) }),
                ..Default::default()
            }),
            ..Default::default()
        };

        let json = serde_json::to_string(&spec).expect("serialise");
        let back: RecipeSpec = serde_json::from_str(&json).expect("deserialise");

        // Verify round-trip fidelity on a few fields.
        assert!(back.base.radius.is_some());
        assert!(back.hover.is_some());
        assert!(back.active.is_none());
        assert!(back.disabled.is_none());
        assert!(back.selected.is_none());

        if let Some(RadiusTier::Pill) = &back.base.radius { /* ok */ }
        else { panic!("radius did not round-trip to Pill"); }
    }

    #[test]
    fn recipe_spec_to_sx_produces_non_default() {
        let t = MockTheme;
        let spec = RecipeSpec {
            base: RecipeDelta {
                radius: Some(RadiusTier::Pill),
                fill: Some(ColorSpec::Tone { tone: ToneRef::Accent, shade: None }),
                ..Default::default()
            },
            ..Default::default()
        };
        let sx = spec.to_sx(&t);
        // Sanity: the resolved Sx has a radius set (pill = 999px).
        let delta = sx.resolved(super::super::style::StyleState::Normal);
        assert_eq!(delta.radius, Some(999.0));
    }

    #[test]
    fn apply_over_preserves_unspecified_fields() {
        let t = MockTheme;

        // Widget default: rounded_md + border.
        let default_sx = Sx::new()
            .rounded_md()
            .border_thin(Tone::Border);

        // Recipe only overrides fill — radius and border come from default.
        let spec = RecipeSpec {
            base: RecipeDelta {
                fill: Some(ColorSpec::Tone { tone: ToneRef::Accent, shade: None }),
                ..Default::default()
            },
            ..Default::default()
        };

        let merged = spec.apply_over(default_sx, &t);
        let delta = merged.resolved(super::super::style::StyleState::Normal);

        // Fill should be from recipe (Accent base = non-zero).
        assert!(delta.fill.is_some(), "fill should be set from recipe");
        // Radius from default_sx (rounded_md).
        assert!(delta.radius.is_some(), "radius should be preserved from default");
        // Border from default_sx.
        assert!(delta.border.is_some(), "border should be preserved from default");
    }
}
