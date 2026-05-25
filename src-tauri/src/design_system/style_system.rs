//! `StyleSystem` — the dimension axis of the two-axis theme system.
//!
//! A `StyleSystem` carries **no colour**. It defines the design language:
//! typography scale, spacing rhythm, corner radii, stroke weights, alpha
//! values, elevation gamma factors, layout density, shadow geometry, and
//! the non-colour behavioural booleans (`Treatments`).
//!
//! Colour and dimension never meet until the Resolver (§4) joins them at
//! render time.

use serde::{Deserialize, Serialize};
use super::color_scheme::Meta;

// ── Typography ───────────────────────────────────────────────────────────────

/// Font size scale (pixels / points as `f32`).
///
/// Matches the token names exposed by `style.rs` (`font_xs`, `font_sm`, …).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Typography {
    /// Extra-small label / annotation text.
    pub size_xs: f32,
    /// Small body / table cell text.
    pub size_sm: f32,
    /// Medium body text — the primary reading size.
    pub size_md: f32,
    /// Large heading.
    pub size_lg: f32,
    /// Extra-large display / hero heading.
    pub size_xl: f32,

    /// Monospace small (code, numbers, timestamps).
    pub mono_sm: f32,
    /// Monospace medium.
    pub mono_md: f32,
    /// Monospace large (e.g. price display).
    pub mono_lg: f32,
}

impl Default for Typography {
    fn default() -> Self {
        Self {
            size_xs: 10.0,
            size_sm: 11.0,
            size_md: 13.0,
            size_lg: 15.0,
            size_xl: 18.0,
            mono_sm: 11.0,
            mono_md: 13.0,
            mono_lg: 15.0,
        }
    }
}

// ── Spacing ──────────────────────────────────────────────────────────────────

/// Layout spacing scale (pixels as `f32`).
///
/// Uses a multiplicative rhythm: xs → sm → md → lg → xl → 2xl.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Spacing {
    pub xs:  f32,   //  2 px
    pub sm:  f32,   //  4 px
    /// Micro-gap tier between `xs` (4 px) and `sm` (8 px).
    /// Backs the `gap_xs_mid()` / `spacing.xs_mid` token (DS-IMPL-3).
    pub xs_mid: f32, //  6 px
    pub md:  f32,   //  8 px   (was `gap_md`)
    pub lg:  f32,   // 12 px
    pub xl:  f32,   // 16 px
    pub xxl: f32,   // 24 px

    /// Named alias matching the existing `gap_md()` token.
    pub gmd: f32,
    /// Standard button / control height.
    pub cta_height: f32,
}

impl Default for Spacing {
    fn default() -> Self {
        Self {
            xs: 2.0,
            sm: 4.0,
            xs_mid: 6.0,
            md: 8.0,
            lg: 12.0,
            xl: 16.0,
            xxl: 24.0,
            gmd: 8.0,
            cta_height: 28.0,
        }
    }
}

// ── Radii ────────────────────────────────────────────────────────────────────

/// Corner radius scale (pixels as `f32`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Radii {
    /// No rounding (sharp corners — the Meridien aesthetic).
    pub none: f32,
    pub xs:   f32,
    pub sm:   f32,
    pub md:   f32,
    pub lg:   f32,
    /// Full pill / circular.
    pub full: f32,
}

impl Default for Radii {
    fn default() -> Self {
        Self { none: 0.0, xs: 2.0, sm: 4.0, md: 6.0, lg: 8.0, full: 9999.0 }
    }
}

// ── Strokes ──────────────────────────────────────────────────────────────────

/// Stroke / border width scale (pixels as `f32`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Strokes {
    /// Sub-pixel hairline (e.g. 0.3–0.5 px — lightest separator).
    /// Backs the `stroke_hair()` / `stroke.hair` token.
    pub hair: f32,
    /// Sub-pixel thin border (0.5 px).
    pub thin: f32,
    /// Mid-weight border tier between `thin` (0.5 px) and `std` (1.0 px).
    /// Backs the `stroke_medium()` / `stroke.medium` token (DS-IMPL-3).
    pub medium: f32,
    /// Standard 1 px border.
    pub std: f32,
    /// Bold 1.5 px emphasis stroke. Backs `stroke_bold()`.
    pub bold: f32,
    /// Thick 2 px stroke. Backs `stroke_thick()`.
    pub thick: f32,
    /// Medium 1.5 px emphasis stroke (legacy alias — prefer `bold`).
    pub md: f32,
    /// Heavy 2 px stroke (focus rings, active indicators; legacy alias — prefer `thick`).
    pub heavy: f32,
}

impl Default for Strokes {
    fn default() -> Self {
        Self {
            hair:   0.3,
            thin:   0.5,
            medium: 0.8,
            std:    1.0,
            bold:   1.5,
            thick:  2.0,
            md:     1.5,
            heavy:  2.0,
        }
    }
}

// ── Alphas ───────────────────────────────────────────────────────────────────

/// Alpha / opacity scale (u8, 0–255) for the dimension axis.
///
/// Fields whose names match `alpha_*()` token functions in `style.rs` are
/// backed by the `dt_u8!` path and carried here so they can be style-overridden.
/// The remaining fields are dimension-axis multipliers (0.0–1.0) used by the
/// resolver for composite operations (fills, borders, etc.).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Alphas {
    // ── u8 tiers (0-255) — mirror TokenSnapshot alpha fields ─────────────────
    /// Near-invisible overlay (hover shimmer). Backs `alpha_faint()` = 10.
    pub faint:   u8,   // 10
    /// Ghost — barely-visible. Backs `alpha_ghost()` = 15.
    pub ghost:   u8,   // 15
    /// Soft muted overlay (disabled states). Backs `alpha_soft()` = 20.
    pub soft_u8: u8,   // 20
    /// Subtle — low-emphasis overlay. Backs `alpha_subtle()` = 40.
    pub subtle_u8: u8, // 40
    /// Tint — icon/chip accent tint. Backs `alpha_tint()` = 48.
    pub tint:    u8,   // 48
    /// Muted — primary dimming value. Backs `alpha_muted()` = 60.
    pub muted_u8: u8,  // 60
    /// Dim — border/line dimming. Backs `alpha_dim()` = 60.
    pub dim:     u8,   // 60
    /// Line — structural line alpha. Backs `alpha_line()` = 80.
    pub line:    u8,   // 80
    /// Strong — selected row fill. Backs `alpha_strong()` = 80.
    pub strong_u8: u8, // 80
    /// Active — interactive element alpha. Backs `alpha_active()` = 100.
    pub active:  u8,   // 100
    /// Heavy — near-opaque overlay. Backs `alpha_heavy()` = 120.
    pub heavy_u8: u8,  // 120
    /// Scrim — modal-backdrop / cmd-palette dimming. Backs `alpha_scrim()` = 140.
    pub scrim:   u8,   // 140
    /// Solid — high-opacity element. Backs `alpha_solid()` = 200.
    pub solid:   u8,   // 200

    // ── f32 multipliers (0.0–1.0) — resolver composites ──────────────────────
    /// Near-invisible overlay (hover shimmer, track backgrounds).
    pub subtle:   f32,  // 0.04
    /// Soft muted overlay (disabled states, secondary text tint).
    pub soft:     f32,  // 0.12
    /// Muted — the primary dimming value (`alpha_muted()`).
    pub muted:    f32,  // 0.24
    /// Mid-opacity (ghost fills, inactive tab backgrounds).
    pub mid:      f32,  // 0.48
    /// Strong (selected row fill, active chip background).
    pub strong:   f32,  // 0.72
    /// Opaque override (override opaque where transparency is undesirable).
    pub opaque:   f32,  // 1.0
    /// Header outer border alpha (migrated from `StyleSettings.header_outer_border_alpha`).
    pub header_border: f32, // 0.18
}

impl Default for Alphas {
    fn default() -> Self {
        Self {
            // u8 tiers — match DEFAULT_TOKEN_SNAPSHOT in style.rs
            faint:     10,
            ghost:     15,
            soft_u8:   20,
            subtle_u8: 40,
            tint:      48,
            muted_u8:  60,
            dim:       60,
            line:      80,
            strong_u8: 80,
            active:   100,
            heavy_u8: 120,
            scrim:    140,
            solid:    200,
            // f32 multipliers
            subtle:        0.04,
            soft:          0.12,
            muted:         0.24,
            mid:           0.48,
            strong:        0.72,
            opaque:        1.0,
            header_border: 0.18,
        }
    }
}

// ── Elevation ────────────────────────────────────────────────────────────────

/// Gamma / luminance factors for elevation-derived surface colours.
///
/// An *elevation* is a background derived from the base palette by applying
/// a gamma multiply.  Three levels replace the six hardcoded `0.95/0.88/0.85`
/// sites in the current `style.rs`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Elevation {
    /// Level 1 — slightly raised (toolbar, column header). Factor > 1 in dark
    /// themes (brightens), < 1 in light (darkens).
    pub l1: f32,   // 1.05
    /// Level 2 — moderately raised (card, popover).
    pub l2: f32,   // 0.95
    /// Level 3 — prominently raised (modal, tooltip).
    pub l3: f32,   // 0.88
}

impl Default for Elevation {
    fn default() -> Self {
        Self { l1: 1.05, l2: 0.95, l3: 0.88 }
    }
}

// ── Density ──────────────────────────────────────────────────────────────────

/// Layout density control — scales certain spacing / height tokens.
///
/// `factor` multiplies spacing/height tokens: 1.0 = standard, 0.8 = compact,
/// 1.2 = comfortable.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Density {
    /// Scale factor applied to row heights and vertical gaps. 1.0 = standard.
    pub factor: f32,
    /// Row height in compact list views (e.g. watchlist, scanner).
    pub row_height_dense: f32,
    /// Row height in comfortable list views.
    pub row_height_comfortable: f32,
}

impl Default for Density {
    fn default() -> Self {
        Self { factor: 1.0, row_height_dense: 22.0, row_height_comfortable: 32.0 }
    }
}

// ── Shadows ──────────────────────────────────────────────────────────────────

/// A single shadow layer's geometry. No colour — colour comes from
/// `ColorScheme.shadow` combined with `Alphas` at the resolver.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ShadowSpec {
    pub blur:    f32,
    pub spread:  f32,
    pub offset_x: f32,
    pub offset_y: f32,
    /// Alpha multiplier applied on top of `ColorScheme.shadow`.
    pub alpha: f32,
}

/// Named shadow roles — geometry only, no colour.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Shadows {
    /// Subtle card lift.
    pub card:    ShadowSpec,
    /// Floating panel / dropdown.
    pub modal:   ShadowSpec,
    /// Tooltip shadow.
    pub tooltip: ShadowSpec,
    /// Dropdown menu shadow.
    pub dropdown: ShadowSpec,
}

impl Default for Shadows {
    fn default() -> Self {
        Self {
            card:     ShadowSpec { blur: 8.0,  spread: 0.0, offset_x: 0.0, offset_y: 2.0, alpha: 0.3 },
            modal:    ShadowSpec { blur: 24.0, spread: 0.0, offset_x: 0.0, offset_y: 8.0, alpha: 0.5 },
            tooltip:  ShadowSpec { blur: 6.0,  spread: 0.0, offset_x: 0.0, offset_y: 2.0, alpha: 0.4 },
            dropdown: ShadowSpec { blur: 12.0, spread: 0.0, offset_x: 0.0, offset_y: 4.0, alpha: 0.4 },
        }
    }
}

// ── Treatments ───────────────────────────────────────────────────────────────

/// Non-colour behavioural booleans and named-role enums.
///
/// These absorb the `StyleSettings` fields that were not colours but described
/// *how* the style behaves (§3.2 of the spec).  A `StyleSystem` that sets
/// `solid_active_fills = true` means "active elements invert the palette
/// (text on bg)"; the resolver applies that using `ColorScheme` colours —
/// so it works for any palette.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Treatments {
    /// When `true`, active/selected controls use a solid fill (typically
    /// `colors.text` bg + `colors.bg` text — palette inversion).
    /// When `false`, active state is indicated by accent colour tint only.
    /// Replaces `StyleSettings.active_fill_color / active_text_color`.
    pub solid_active_fills: bool,

    /// When `true`, borders are drawn at `Strokes.thin` (0.5 px hairline).
    /// When `false`, borders use `Strokes.std` (1 px).
    pub hairline_borders: bool,

    /// When `true`, section / group labels are rendered in uppercase.
    pub uppercase_section_labels: bool,

    /// When `true`, the segmented control idle state uses a filled background.
    /// When `false`, idle segments are transparent with an outline.
    pub segmented_filled_idle: bool,

    /// Input focus ring style.
    pub focus_ring: FocusRingStyle,
}

/// How focus rings are drawn (dimension-only; ring colour comes from `ColorScheme.accent`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum FocusRingStyle {
    /// No visible ring — rely on fill/colour change only.
    None,
    /// Hairline outline at accent colour.
    Outline,
    /// Glow / drop shadow at accent colour.
    Glow,
}

impl Default for Treatments {
    fn default() -> Self {
        Self {
            solid_active_fills:       false,
            hairline_borders:         false,
            uppercase_section_labels: false,
            segmented_filled_idle:    false,
            focus_ring:               FocusRingStyle::Outline,
        }
    }
}

// ── StyleSystem ──────────────────────────────────────────────────────────────

/// Axis 1 — the design language. Pure dimension. No colour.
///
/// Corresponds to `style.rs` token functions; maps 1-to-1 with the DTCG
/// `style.*.json` file kind (§6 of the spec).
///
/// **Rule:** never add colour fields here.  Alpha *values* (multipliers) are
/// dimension-axis; the colour those alphas modify comes from `ColorScheme`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StyleSystem {
    /// Identity and dark/light preference flag.
    pub meta:       Meta,
    /// Font size scale.
    pub typography: Typography,
    /// Layout spacing scale.
    pub spacing:    Spacing,
    /// Corner radius scale.
    pub radii:      Radii,
    /// Border / stroke weight scale.
    pub strokes:    Strokes,
    /// Opacity / alpha multipliers.
    pub alphas:     Alphas,
    /// Surface elevation gamma factors.
    pub elevation:  Elevation,
    /// Layout density control.
    pub density:    Density,
    /// Shadow geometry (no colour).
    pub shadows:    Shadows,
    /// Non-colour behavioural flags and enums.
    pub treatments: Treatments,
}

impl Default for StyleSystem {
    fn default() -> Self {
        Self::builtin_default()
    }
}

impl StyleSystem {
    /// The built-in "Apex Default" style — matches the current compiled-in
    /// token values so the registry is always non-empty at startup.
    pub fn builtin_default() -> Self {
        Self {
            meta:       Meta::new("apex-default", "Apex Default", true),
            typography: Typography::default(),
            spacing:    Spacing::default(),
            radii:      Radii::default(),
            strokes:    Strokes::default(),
            alphas:     Alphas::default(),
            elevation:  Elevation::default(),
            density:    Density::default(),
            shadows:    Shadows::default(),
            treatments: Treatments::default(),
        }
    }

    /// "Meridien" style — sharp corners, hairline borders, solid active fills,
    /// uppercase section labels.  The first authentic style beyond the default.
    pub fn meridien() -> Self {
        Self {
            meta: Meta::new("meridien", "Meridien", true),
            radii: Radii { none: 0.0, xs: 0.0, sm: 0.0, md: 0.0, lg: 0.0, full: 9999.0 },
            strokes: Strokes { hair: 0.3, thin: 0.5, medium: 0.8, std: 0.5, bold: 1.0, thick: 1.5, md: 1.0, heavy: 1.5 },
            treatments: Treatments {
                solid_active_fills:       true,
                hairline_borders:         true,
                uppercase_section_labels: true,
                segmented_filled_idle:    false,
                focus_ring:               FocusRingStyle::Outline,
            },
            ..Self::builtin_default()
        }
    }
}
