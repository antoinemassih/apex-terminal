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

// ── Typed enums (S3/S11 blockers — defined here in S1) ───────────────────────

/// How the active pane is visually indicated. Replaces `Chrome.pane_active_indicator: u8`.
///
/// S3 will update `TokenSnapshot` and `Chrome` to use this typed form.
/// Until then, `Chrome.pane_active_indicator` remains `u8` for binary compatibility;
/// use `PaneActiveIndicator::from_u8` / `as_u8` at conversion boundaries.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum PaneActiveIndicator {
    /// No visual indicator — pane focus is implicit.
    None,
    /// Thin accent stripe along the top edge of the active pane header.
    #[default]
    TopStripe,
    /// Active pane header is filled with a lightened/darkened surface colour.
    HeaderFill,
    /// Both top stripe and header fill.
    Both,
}

impl PaneActiveIndicator {
    /// Convert from the legacy `u8` index used in `StyleSettings.pane_active_indicator`.
    pub fn from_u8(v: u8) -> Self {
        match v {
            0 => Self::None,
            1 => Self::TopStripe,
            2 => Self::HeaderFill,
            3 => Self::Both,
            _ => Self::HeaderFill,
        }
    }

    /// Convert back to the legacy `u8` index.
    pub fn as_u8(self) -> u8 {
        match self {
            Self::None       => 0,
            Self::TopStripe  => 1,
            Self::HeaderFill => 2,
            Self::Both       => 3,
        }
    }
}

/// How the side-panel header strip is rendered. Replaces `Chrome.panel_header_treatment: u8`.
///
/// S3 will update `TokenSnapshot` and `Chrome` to use this typed form.
/// Until then, `Chrome.panel_header_treatment` remains `u8` for binary compatibility;
/// use `PanelHeaderTreatment::from_u8` / `as_u8` at conversion boundaries.
///
/// RECIPE-CANDIDATE(S4)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum PanelHeaderTreatment {
    /// Underline-style tab strip — accent hairline under the active tab.
    #[default]
    Line,
    /// Segmented control — pill background on the active segment.
    Segmented,
    /// Filled tab — solid fill on the active tab.
    Filled,
    /// Card-style — the active tab is a slightly elevated card.
    Card,
    /// Pane-style — header strip matches the pane header aesthetic.
    Pane,
}

impl PanelHeaderTreatment {
    /// Convert from the legacy `u8` index used in `StyleSettings.panel_header_treatment`.
    pub fn from_u8(v: u8) -> Self {
        match v {
            0 => Self::Line,
            1 => Self::Segmented,
            2 => Self::Filled,
            3 => Self::Card,
            4 => Self::Pane,
            _ => Self::Line,
        }
    }

    /// Convert back to the legacy `u8` index.
    pub fn as_u8(self) -> u8 {
        match self {
            Self::Line      => 0,
            Self::Segmented => 1,
            Self::Filled    => 2,
            Self::Card      => 3,
            Self::Pane      => 4,
        }
    }
}

// ── Typography ───────────────────────────────────────────────────────────────

/// Font size scale (pixels / points as `f32`) and font-family identifiers.
///
/// Matches the token names exposed by `style.rs` (`font_xs`, `font_sm`, …).
/// Font-family fields are plain `String` names resolved by the font loader at
/// startup — Stream S7 depends on `family_ui`, `family_mono`, `family_display`
/// existing here.
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

    /// Section / eyebrow label font size (`StyleSettings.font_section_label`).
    /// Distinct from `size_xs` (which backs `font_caption`).
    #[serde(default = "Typography::default_section_label")]
    pub size_section_label: f32,
    /// Letter-spacing (px) for general tracked-out labels (`label_letter_spacing_px`).
    #[serde(default)]
    pub label_tracking: f32,
    /// Letter-spacing (px) for toolbar nav button text (`nav_letter_spacing_px`).
    #[serde(default)]
    pub nav_tracking: f32,
    /// Letter-spacing (px) specifically for section/eyebrow headers (`section_header_tracking`).
    #[serde(default)]
    pub section_tracking: f32,

    // ── Font family identifiers (S7 blocker) ─────────────────────────────────
    /// UI / proportional family name — used for all body text, labels, and
    /// headings that are not explicitly mono or display.
    /// Default: `"Inter"` (matches the current compiled-in egui font loader).
    #[serde(default = "Typography::default_family_ui")]
    pub family_ui: String,
    /// Monospace family name — used for prices, code, and timestamps.
    /// Default: `"JetBrains Mono"` (matches the current compiled-in egui mono font).
    #[serde(default = "Typography::default_family_mono")]
    pub family_mono: String,
    /// Display / hero family name — used for `size_xl` hero numerics and big
    /// headings when `Treatments.serif_headlines` is true.
    /// Default: `"Inter"` (fallback to UI family when no separate display font is loaded).
    #[serde(default = "Typography::default_family_display")]
    pub family_display: String,

    // ── M1: authorable UI type ladder (backs `font_2xs()..font_xl()`) ────────
    #[serde(default = "Typography::default_ui_2xs")]
    pub ui_2xs: f32,   //  9
    #[serde(default = "Typography::default_ui_xs")]
    pub ui_xs:  f32,   // 10
    #[serde(default = "Typography::default_ui_sm")]
    pub ui_sm:  f32,   // 12
    #[serde(default = "Typography::default_ui_md")]
    pub ui_md:  f32,   // 14
    #[serde(default = "Typography::default_ui_lg")]
    pub ui_lg:  f32,   // 16
    // Display scale + the three in-between UI rungs. All seven were hardcoded
    // literals in `ui_kit::style` (`font_display_lg()` = 42.0 and friends), so
    // a style could author its whole UI type ladder and still not touch its
    // display type — the sizes that set a screen's voice most.
    #[serde(default = "Typography::default_display_sm")]
    pub display_sm: f32,  // 28
    #[serde(default = "Typography::default_display_md")]
    pub display_md: f32,  // 32
    #[serde(default = "Typography::default_display_lg")]
    pub display_lg: f32,  // 42
    #[serde(default = "Typography::default_display_xl")]
    pub display_xl: f32,  // 56
    #[serde(default = "Typography::default_ui_4xs")]
    pub ui_4xs: f32,      //  6
    #[serde(default = "Typography::default_ui_xs_plus")]
    pub ui_xs_plus: f32,  // 10
    #[serde(default = "Typography::default_ui_md_plus")]
    pub ui_md_plus: f32,  // 14
    #[serde(default = "Typography::default_ui_xl")]
    pub ui_xl:  f32,   // 22
}

impl Typography {
    fn default_display_sm() -> f32 { 28.0 }
    fn default_display_md() -> f32 { 32.0 }
    fn default_display_lg() -> f32 { 42.0 }
    fn default_display_xl() -> f32 { 56.0 }
    fn default_ui_4xs()     -> f32 {  6.0 }
    fn default_ui_xs_plus() -> f32 { 10.0 }
    fn default_ui_md_plus() -> f32 { 14.0 }
    fn default_ui_2xs() -> f32 {  9.0 }
    fn default_ui_xs()  -> f32 { 10.0 }
    fn default_ui_sm()  -> f32 { 12.0 }
    fn default_ui_md()  -> f32 { 14.0 }
    fn default_ui_lg()  -> f32 { 16.0 }
    fn default_ui_xl()  -> f32 { 22.0 }
}

impl Typography {
    fn default_section_label() -> f32 { 9.0 }
    fn default_family_ui()      -> String { "Inter".to_owned() }
    fn default_family_mono()    -> String { "JetBrains Mono".to_owned() }
    fn default_family_display() -> String { "Inter".to_owned() }
}

impl Default for Typography {
    fn default() -> Self {
        // P2.2: aligned to TokenSnapshot DEFAULT (the values the live frame
        // renders with via `frame_tokens()`). Was 10/11/13/15/18; corrected
        // to match `font_xs()` … `font_xl()` in ui_kit/style.rs.
        Self {
            size_xs:  9.0,
            size_sm: 11.0,
            size_md: 13.0,
            size_lg: 16.0,
            size_xl: 22.0,
            mono_sm: 11.0,
            mono_md: 13.0,
            mono_lg: 16.0,
            size_section_label: 9.0,
            label_tracking: 0.0,
            nav_tracking:   0.0,
            section_tracking: 0.0,
            family_ui:      "Inter".to_owned(),
            family_mono:    "JetBrains Mono".to_owned(),
            family_display: "Inter".to_owned(),
            display_sm: Self::default_display_sm(),
            display_md: Self::default_display_md(),
            display_lg: Self::default_display_lg(),
            display_xl: Self::default_display_xl(),
            ui_4xs:     Self::default_ui_4xs(),
            ui_xs_plus: Self::default_ui_xs_plus(),
            ui_md_plus: Self::default_ui_md_plus(),
            ui_2xs: Self::default_ui_2xs(),
            ui_xs:  Self::default_ui_xs(),
            ui_sm:  Self::default_ui_sm(),
            ui_md:  Self::default_ui_md(),
            ui_lg:  Self::default_ui_lg(),
            ui_xl:  Self::default_ui_xl(),
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

    /// Primary CTA button horizontal padding (`cta_padding_x`).
    #[serde(default = "Spacing::default_cta_padding_x")]
    pub cta_padding_x: f32,
    /// Standard button height (`button_height_px`).
    // The 2px base of the gap ladder. `gap_2xs()` applied the spacing override
    // to a hardcoded 2.0, so the rung scaled but a style could not re-pitch it.
    #[serde(default = "Spacing::default_gap_2xs")]
    pub gap_2xs: f32,           //  2
    #[serde(default = "Spacing::default_button_height")]
    pub button_height: f32,
    /// Standard button horizontal padding (`button_padding_x`).
    #[serde(default = "Spacing::default_button_padding_x")]
    pub button_padding_x: f32,
    /// Tab strip height (`tab_height`).
    #[serde(default = "Spacing::default_tab_height")]
    pub tab_height: f32,

    // ── M1: the authorable GAP LADDER (backs `gap_*()` / TokenSnapshot) ──────
    //
    // These are the fields `begin_frame()` sources the per-frame gap tokens
    // from (precedence: hot-reload override ▸ design-mode dt ▸ these). They
    // are DISTINCT from `xs..xxl` above, which predate the P2.2 type-scale
    // lift, carry stale authored values in the builtin systems, and feed the
    // adapter's card-padding mapping — repurposing them would have changed
    // card padding for every style. Defaults equal the previous hard literals
    // in `begin_frame`, so a style that doesn't author these renders
    // byte-identically to before the wire-up. Theme packs may now author the
    // whitespace axis (e.g. Meridien's airier 6/12/18/24/32 ladder).
    #[serde(default = "Spacing::default_gap_xs")]
    pub gap_xs:  f32,   //  4
    #[serde(default = "Spacing::default_gap_sm")]
    pub gap_sm:  f32,   //  8
    #[serde(default = "Spacing::default_gap_md")]
    pub gap_md:  f32,   // 12
    #[serde(default = "Spacing::default_gap_lg")]
    pub gap_lg:  f32,   // 16
    #[serde(default = "Spacing::default_gap_xl")]
    pub gap_xl:  f32,   // 20
    #[serde(default = "Spacing::default_gap_2xl")]
    pub gap_2xl: f32,   // 24
    #[serde(default = "Spacing::default_gap_3xl")]
    pub gap_3xl: f32,   // 32
}

impl Spacing {
    fn default_gap_2xs()           -> f32 {  2.0 }
    fn default_cta_padding_x()   -> f32 { 12.0 }
    fn default_button_height()   -> f32 { 24.0 }
    fn default_button_padding_x()-> f32 { 10.0 }
    fn default_tab_height()      -> f32 { 28.0 }
    fn default_gap_xs()  -> f32 {  4.0 }
    fn default_gap_sm()  -> f32 {  8.0 }
    fn default_gap_md()  -> f32 { 12.0 }
    fn default_gap_lg()  -> f32 { 16.0 }
    fn default_gap_xl()  -> f32 { 20.0 }
    fn default_gap_2xl() -> f32 { 24.0 }
    fn default_gap_3xl() -> f32 { 32.0 }
}

impl Default for Spacing {
    fn default() -> Self {
        // P2.2: aligned to TokenSnapshot DEFAULT. Was 2/4/6/8/12/16/24;
        // corrected to gap_xs..gap_2xl in ui_kit/style.rs (4/8/12/16/20/24).
        // The xs_mid 6.0 already matched and stays.
        Self {
            gap_2xs:           Self::default_gap_2xs(),
            xs:         4.0,
            sm:         8.0,
            xs_mid:     6.0,
            md:        12.0,
            lg:        16.0,
            xl:        20.0,
            xxl:       24.0,
            gmd:        8.0,
            cta_height: 28.0,
            cta_padding_x:   12.0,
            button_height:   24.0,
            button_padding_x:10.0,
            tab_height:      28.0,
            gap_xs:   Self::default_gap_xs(),
            gap_sm:   Self::default_gap_sm(),
            gap_md:   Self::default_gap_md(),
            gap_lg:   Self::default_gap_lg(),
            gap_xl:   Self::default_gap_xl(),
            gap_2xl:  Self::default_gap_2xl(),
            gap_3xl:  Self::default_gap_3xl(),
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
    /// Full pill / circular (conceptual, used by helpers needing "max round").
    pub full: f32,
    /// Pill radius as the runtime `r_pill` value (px, 0–99). Distinct from `full`:
    /// Meridien uses 0 (sharp pill), Aperture/Octave use 99 (rounded pill).
    #[serde(default = "Radii::default_pill")]
    pub pill: f32,
    /// Chip/badge corner radius (`r_chip`). 0 = use `sm`.
    #[serde(default)]
    pub chip: f32,
}

impl Radii {
    fn default_pill() -> f32 { 99.0 }
}

impl Default for Radii {
    fn default() -> Self {
        // P2.2: aligned to TokenSnapshot DEFAULT (radius_lg corrected 8.0 → 12.0).
        Self { none: 0.0, xs: 2.0, sm: 4.0, md: 6.0, lg: 12.0, full: 9999.0, pill: 99.0, chip: 0.0 }
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
    /// Whisper (25) and hint (30) — the two rungs between `ghost` (15) and
    /// `subtle` (40). Both were hardcoded in `ui_kit::style`.
    #[serde(default = "Alphas::default_whisper")]
    pub whisper: u8,
    #[serde(default = "Alphas::default_hint")]
    pub hint: u8,
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
    /// Dense (160) and near-solid (180) — the top of the ladder steps by 20
    /// (`active` 100 → `heavy` 120 → `scrim` 140) and then jumped 60 straight to
    /// `solid`. These two continue that rhythm rather than inventing a new one.
    /// `dense` is borders and fills at full presence; `near_solid` is secondary
    /// label text and disabled accents — read as text, so not quite opaque.
    #[serde(default = "Alphas::default_dense")]
    pub dense: u8,     // 160
    #[serde(default = "Alphas::default_near_solid")]
    pub near_solid: u8, // 180
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

impl Alphas {
    fn default_whisper() -> u8 { 25 }
    fn default_hint()    -> u8 { 30 }
    fn default_dense()      -> u8 { 160 }
    fn default_near_solid() -> u8 { 180 }
}

impl Default for Alphas {
    fn default() -> Self {
        Self {
            // u8 tiers — match DEFAULT_TOKEN_SNAPSHOT in style.rs
            whisper:   Self::default_whisper(),
            hint:      Self::default_hint(),
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
            dense:      Self::default_dense(),
            near_solid: Self::default_near_solid(),
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

// ── Icons ────────────────────────────────────────────────────────────────────

/// Icon glyph sizes.
///
/// These were four hardcoded literals in `ui_kit::style` — `icon_xs()` = 14.0
/// and friends — so no theme could change icon scale at all. Icon size is a
/// primary axis of a UI's density and character; a design system that cannot
/// move it is not controlling the look, only the colours.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Icons {
    #[serde(default = "Icons::default_xs")]
    pub xs: f32, // 14
    #[serde(default = "Icons::default_sm")]
    pub sm: f32, // 16
    #[serde(default = "Icons::default_md")]
    pub md: f32, // 18
    #[serde(default = "Icons::default_lg")]
    pub lg: f32, // 20
}

impl Icons {
    fn default_xs() -> f32 { 14.0 }
    fn default_sm() -> f32 { 16.0 }
    fn default_md() -> f32 { 18.0 }
    fn default_lg() -> f32 { 20.0 }
}

impl Default for Icons {
    fn default() -> Self {
        Self { xs: Self::default_xs(), sm: Self::default_sm(),
               md: Self::default_md(), lg: Self::default_lg() }
    }
}

// ── Line heights ─────────────────────────────────────────────────────────────

/// Line-height (leading) multipliers.
///
/// Also six hardcoded literals before this. Leading is the single biggest lever
/// on whether a dense trading UI reads as tight and technical or open and
/// editorial — exactly the difference between the Meridien and Aperture
/// targets — and it was the one axis a theme could not touch.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LineHeights {
    #[serde(default = "LineHeights::default_tight")]
    pub tight: f32,   // 1.20
    #[serde(default = "LineHeights::default_heading")]
    pub heading: f32, // 1.25
    #[serde(default = "LineHeights::default_dense")]
    pub dense: f32,   // 1.30
    #[serde(default = "LineHeights::default_compact")]
    pub compact: f32, // 1.35
    #[serde(default = "LineHeights::default_normal")]
    pub normal: f32,  // 1.40
    #[serde(default = "LineHeights::default_loose")]
    pub loose: f32,   // 1.50
}

impl LineHeights {
    fn default_tight()   -> f32 { 1.20 }
    fn default_heading() -> f32 { 1.25 }
    fn default_dense()   -> f32 { 1.30 }
    fn default_compact() -> f32 { 1.35 }
    fn default_normal()  -> f32 { 1.40 }
    fn default_loose()   -> f32 { 1.50 }
}

impl Default for LineHeights {
    fn default() -> Self {
        Self { tight: Self::default_tight(), heading: Self::default_heading(),
               dense: Self::default_dense(), compact: Self::default_compact(),
               normal: Self::default_normal(), loose: Self::default_loose() }
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

    // ── M4.5: the STRUCTURAL ladder (authorable proportions) ────────────────
    //
    // The layout audit: "gap_*() scales with the SpacingScale override; row
    // heights, header heights, splitter width and the rail Width presets do
    // NOT. Themes can retune the gutters but not the PROPORTIONS — which is
    // the stated design-system vision."
    //
    // These back `ui_kit::style::row_height_*()` and the chrome dimensions
    // that were hard literals. Defaults equal those literals exactly, so an
    // unauthored style renders byte-identically; Meridien can now author its
    // 32px rows and Mariner its 22px ones as DATA.
    #[serde(default = "Density::default_row_dense")]
    pub row_dense: f32,      // 18
    #[serde(default = "Density::default_row_compact")]
    pub row_compact: f32,    // 20
    #[serde(default = "Density::default_row_default")]
    pub row_default: f32,    // 22
    #[serde(default = "Density::default_row_spacious")]
    pub row_spacious: f32,   // 24
    #[serde(default = "Density::default_row_tall")]
    pub row_tall: f32,       // 30
    /// Splitter / drag-handle thickness (was a 6.0 literal in pane_grid).
    #[serde(default = "Density::default_splitter")]
    pub splitter_width: f32, // 6
    /// Side-rail width presets (were the 240/300/400 `Width` enum literals).
    #[serde(default = "Density::default_rail_narrow")]
    pub rail_narrow: f32,    // 240
    #[serde(default = "Density::default_rail_medium")]
    pub rail_medium: f32,    // 300
    #[serde(default = "Density::default_rail_wide")]
    pub rail_wide: f32,      // 400

    /// Control-height ladder — what `Size::{Xs,Sm,Md,Lg,Xl}::height()` returns.
    ///
    /// `Size` already read live tokens for `font_size()`, `padding_x()` and
    /// `padding()`; only `height()` returned frozen literals (18/22/28/34/40).
    /// One struct, two philosophies — so a theme could change a control's type
    /// and padding but never its height, which is the axis the eye actually
    /// reads as "consistent".
    ///
    /// Defaults are exactly the former literals, so an unauthored style is
    /// byte-identical. Same additive shape as the M1 `Typography.ui_*` ladder.
    #[serde(default = "Density::default_control_xs")]
    pub control_xs: f32,     // 18
    #[serde(default = "Density::default_control_sm")]
    pub control_sm: f32,     // 22
    #[serde(default = "Density::default_control_md")]
    pub control_md: f32,     // 28
    #[serde(default = "Density::default_control_lg")]
    pub control_lg: f32,     // 34
    #[serde(default = "Density::default_control_xl")]
    pub control_xl: f32,     // 40
}

impl Density {
    fn default_row_dense()    -> f32 {  18.0 }
    fn default_row_compact()  -> f32 {  20.0 }
    fn default_row_default()  -> f32 {  22.0 }
    fn default_row_spacious() -> f32 {  24.0 }
    fn default_row_tall()     -> f32 {  30.0 }
    fn default_splitter()     -> f32 {   8.0 }
    fn default_rail_narrow()  -> f32 { 240.0 }
    fn default_rail_medium()  -> f32 { 300.0 }
    fn default_rail_wide()    -> f32 { 400.0 }
    fn default_control_xs()   -> f32 {  18.0 }
    fn default_control_sm()   -> f32 {  22.0 }
    fn default_control_md()   -> f32 {  28.0 }
    fn default_control_lg()   -> f32 {  34.0 }
    fn default_control_xl()   -> f32 {  40.0 }
}

impl Default for Density {
    fn default() -> Self {
        Self {
            factor: 1.0,
            row_height_dense: 22.0,
            row_height_comfortable: 32.0,
            // M4.5: structural ladder — defaults equal the former hard
            // literals in `ui_kit::style`, so unauthored styles are
            // byte-identical.
            row_dense:      Self::default_row_dense(),
            row_compact:    Self::default_row_compact(),
            row_default:    Self::default_row_default(),
            row_spacious:   Self::default_row_spacious(),
            row_tall:       Self::default_row_tall(),
            splitter_width: Self::default_splitter(),
            rail_narrow:    Self::default_rail_narrow(),
            rail_medium:    Self::default_rail_medium(),
            rail_wide:      Self::default_rail_wide(),
            control_xs:     Self::default_control_xs(),
            control_sm:     Self::default_control_sm(),
            control_md:     Self::default_control_md(),
            control_lg:     Self::default_control_lg(),
            control_xl:     Self::default_control_xl(),
        }
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
// ── M1 Change D: signature tokens (numerals + card recipe) ──────────────────

/// Which font family a text role draws from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum FontRole {
    #[default]
    Ui,
    Mono,
    Display,
}

/// Display-numeral treatment (hero prices, big metrics). The tell that this
/// must be authorable: Aperture's hero numerals are SANS (`Inter Tight 500 @
/// -0.04em`) while every other theme is mono — "big numbers are mono"
/// hardcoded would make Aperture permanently wrong.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct NumeralTier {
    pub family:   FontRole,
    /// Letter-spacing in px (negative = tight; Aperture ≈ -0.5).
    pub tracking: f32,
    /// Weight expressed as a strength hint (400/500/600/700). NOTE: egui
    /// selects weight by FAMILY registration, not a variable axis — until a
    /// per-weight family is registered this is advisory (strong=true when
    /// >= 600). Recorded per the M1.3-D spike decision.
    pub weight:   u16,
}


// ── M1 Change E: multi-layer shadow stacks (additive; legacy specs stay) ────
//
// The DS card treatments are STACKS — e.g. Alto's four-layer "Zed warm-dark
// bevel": inset warm highlight + inset shadow + contact line + ambient drop.
// Even the light themes need TWO outer layers (Lucid's paper drop), and the
// single `ShadowSpec` cannot express either. `*_layers` is additive with
// `#[serde(default)]` (empty = use the legacy single spec), so no schema
// bump and no pack migration — v1 packs load unchanged.

/// Semantic tint for a shadow layer. NEVER a literal colour at a call site —
/// resolution happens against the active palette at paint time.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum ShadowTint {
    /// Modulates the palette's `shadow` colour.
    Shadow,
    /// Modulates the palette's authored `bevel_highlight` tint
    /// (WARM on Alto, COOL on Mariner; WHITE when unauthored).
    Highlight,
    /// Explicit RGBA (rare; prefer the semantic variants).
    Custom([u8; 4]),
}

/// One layer of a shadow stack. `inset` layers with `blur == 0` (every DS
/// inset in the six systems) render as 1px edge strokes clipped to the rect
/// — cheap, no blur pass.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ShadowLayer {
    pub inset:    bool,
    pub offset_x: f32,
    pub offset_y: f32,
    pub blur:     f32,
    pub spread:   f32,
    pub tint:     ShadowTint,
    /// 0-255 over the resolved tint.
    pub alpha:    u8,
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

    // M1 Change E: authored stacks. Empty = derive from the legacy spec
    // above (byte-identical). A DS authors e.g. Alto's 4-layer card bevel.
    #[serde(default)]
    pub card_layers: Vec<ShadowLayer>,
    #[serde(default)]
    pub modal_layers: Vec<ShadowLayer>,
}

impl Default for Shadows {
    fn default() -> Self {
        Self {
            card:     ShadowSpec { blur: 8.0,  spread: 0.0, offset_x: 0.0, offset_y: 2.0, alpha: 0.3 },
            modal:    ShadowSpec { blur: 24.0, spread: 0.0, offset_x: 0.0, offset_y: 8.0, alpha: 0.5 },
            tooltip:  ShadowSpec { blur: 6.0,  spread: 0.0, offset_x: 0.0, offset_y: 2.0, alpha: 0.4 },
            dropdown: ShadowSpec { blur: 12.0, spread: 0.0, offset_x: 0.0, offset_y: 4.0, alpha: 0.4 },
            card_layers: Vec::new(),
            modal_layers: Vec::new(),
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

    /// When `true`, panel section headers are prefixed with a zero-padded
    /// ordinal in the accent colour — `01 WATCHLIST`, `02 ORDER BOOK`, … —
    /// numbered in visual order down the frame.
    ///
    /// This is a *treatment*, not a string transform: the numeral is accent-
    /// coloured while the title is not, so it cannot ride on
    /// `uppercase_section_labels`'s `style_label_case` path. It occupies the
    /// header's leading (icon) slot, which is already accent + mono + measured.
    ///
    /// **Defaults to `false`; four of the six documented styles opt in.**
    ///
    /// Settled against the BESPOKE apps, which is the only evidence that can
    /// answer this:
    ///
    /// | style | numbers | where |
    /// |---|---|---|
    /// | meridien, lucid, alto, mariner | yes | `num="01".."08"` in the app |
    /// | aperture | no | `SectionH` is `title \| sub \| right` — no numeral |
    /// | cadence | no | `num` appears only in its design-system doc |
    ///
    /// I got the scope wrong twice before this. First `true` for Meridien
    /// alone, from its render. Then `true` for all six, from
    /// `faithful/<style>/normalized.html` — which carries the numeral for
    /// every theme, but is a token-driven harness that renders the SAME markup
    /// for all of them to isolate colour and shape. Its own HANDOFF.md says so:
    /// *"visual sanity-check only (token-driven; NOT a pixel clone of the
    /// bespoke app)"*. It cannot answer a composition question, and I used it
    /// for one before reading that line.
    ///
    /// Default `false` rather than `true` because 4-of-6 is not a mandate, and
    /// the styles with no bespoke reference (octave, relay, glass) should not
    /// inherit a decoration by coin-flip.
    pub numbered_section_labels: bool,

    /// When `true`, the segmented control idle state uses a filled background.
    /// When `false`, idle segments are transparent with an outline.
    pub segmented_filled_idle: bool,

    /// Input focus ring style.
    pub focus_ring: FocusRingStyle,

    /// Surface bevel treatment applied to button faces, panel headers, chips
    /// and inline tabs. `None` = flat fill (editorial / light themes);
    /// `Raised` = top highlight + bottom shadow (Zed faces — Alto/Mariner);
    /// `Inset` = sunken well (inputs / TF pills). The highlight/shadow *tint*
    /// is derived from palette luminance at paint time (light tint on dark
    /// themes, dark tint on light), so it works for any colour scheme.
    pub surface_bevel: BevelStyle,
    /// Alpha (0-255) of the bevel top inner-highlight line. 0 = no highlight.
    pub bevel_highlight_alpha: u8,
    /// Alpha (0-255) of the bevel bottom inner-shadow line. 0 = no shadow.
    pub bevel_shadow_alpha: u8,

    // ── Per-style list row shape ──────────────────────────────────────────────
    /// Horizontal inset (px) each side of a watchlist/list row. 0 = flush rows;
    /// 6 = Aperture pill rows; 4 = Glass soft rows. Palette-independent.
    pub wl_row_side_margin: f32,
    /// Corner radius for list rows (px). 0 = square; 99 = full pill.
    pub wl_row_corner_radius: u8,
    /// Alpha (0-255) of a per-row hairline bottom divider. 0 = no divider.
    pub wl_row_divider_alpha: u8,

    // ── Per-style typography behaviour ────────────────────────────────────────
    /// When `true`, section/eyebrow headers use the monospace family
    /// (Alto/Mariner/Relay IBM Plex Mono; others proportional).
    pub section_header_mono: bool,
    /// When `true`, symbol text in list rows uses the monospace family.
    pub wl_symbol_mono: bool,
    /// Default tab treatment index for ui-kit Tabs widgets.
    /// 0=Line, 1=Segmented, 2=Filled, 3=Card, 4=Pane.
    pub panel_tab_treatment: u8,

    // ── Active pane header fill ───────────────────────────────────────────────
    /// When `true`, the active pane header fills with the accent colour
    /// (Aperture signature — orange bar). All text inside flips to contrast.
    pub pane_active_fill_accent: bool,

    // ── Editorial / chrome behavioural flags (migrated from StyleSettings) ─────
    /// Use a serif family for hero numerics / display headings (`serif_headlines`).
    #[serde(default)] pub serif_headlines: bool,
    /// Active-state button treatment index (`button_treatment`):
    /// 0=SoftPill, 1=OutlineAccent, 2=UnderlineActive, 3=RaisedActive, 4=BlackFillActive.
    #[serde(default)] pub button_treatment: u8,
    /// Invert palette on active elements (fill=text, text=bg) — `invert_active_fill`.
    #[serde(default)] pub invert_active_fill: bool,
    /// Paint full-height vertical dividers between toolbar button clusters.
    #[serde(default)] pub vertical_group_dividers: bool,
    /// Show the active-tab accent underline in tab bars.
    #[serde(default = "Treatments::default_true")] pub show_active_tab_underline: bool,
    /// Paint a distinct recessed fill behind inactive pane headers.
    #[serde(default = "Treatments::default_true")] pub inactive_header_fill: bool,
    /// Drop icon glyphs from right-side toolbar nav buttons (label-only).
    #[serde(default)] pub nav_buttons_label_only: bool,
    /// Render toolbar nav button labels in ALL CAPS.
    #[serde(default)] pub nav_buttons_uppercase_labels: bool,
    /// Draw the tab underline directly under active tab text (not header bottom).
    #[serde(default)] pub tab_underline_under_text: bool,
    /// Show a floating card shadow even when `shadows_enabled` is false.
    #[serde(default)] pub card_floating_shadow: bool,
    /// Master toggle for drop shadows (cannot be derived from blur > 0).
    #[serde(default = "Treatments::default_true")] pub shadows_enabled: bool,
    /// Honour the "reduce motion" preference — false snaps all animation instant.
    #[serde(default = "Treatments::default_true")] pub animations_enabled: bool,
}

impl Treatments {
    fn default_true() -> bool { true }
}

/// Surface bevel mode (dimension-only; tint derives from palette luminance).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum BevelStyle {
    /// Flat fill — no inset highlight/shadow (editorial, light, minimal themes).
    #[default]
    None,
    /// Raised face — light inner highlight on top edge, dark inner shadow on
    /// bottom edge (the Zed "raised button face" look — Alto / Mariner).
    Raised,
    /// Sunken well — inverted bevel (dark top, light bottom) for inputs / pills.
    Inset,
}

/// How focus rings are drawn (dimension-only; ring colour comes from `ColorScheme.accent`).
///
/// `Copy` because it now travels in `TokenSnapshot`, which is copied per frame
/// — a three-variant fieldless enum has nothing to clone.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
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
            numbered_section_labels: false,
            segmented_filled_idle:    false,
            focus_ring:               FocusRingStyle::Outline,
            surface_bevel:            BevelStyle::None,
            bevel_highlight_alpha:    0,
            bevel_shadow_alpha:       0,
            wl_row_side_margin:       0.0,
            wl_row_corner_radius:     0,
            wl_row_divider_alpha:     0,
            section_header_mono:      false,
            wl_symbol_mono:           false,
            panel_tab_treatment:      0,
            pane_active_fill_accent:  false,
            serif_headlines:          false,
            button_treatment:         0,
            invert_active_fill:       false,
            vertical_group_dividers:  false,
            show_active_tab_underline: true,
            inactive_header_fill:     true,
            nav_buttons_label_only:   false,
            nav_buttons_uppercase_labels: false,
            tab_underline_under_text: false,
            card_floating_shadow:     false,
            shadows_enabled:          true,
            animations_enabled:       true,
        }
    }
}

// ── Chrome (geometry + finish; migrated from StyleSettings) ───────────────────

/// How a toolbar button-group is enclosed. The concrete look (radius / fill /
/// border) lives as composed `Sx` at the render site, not as data threaded
/// through the style pipeline — so a new treatment is one new variant here plus
/// its `Sx` recipe, with no schema change.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum GroupEnclosure {
    /// Flat — buttons spaced + separated by hairline dividers (Meridien/Octave).
    #[default]
    None,
    /// Rounded box: subtle fill + hairline border (Aperture).
    Bordered,
    /// Frosted: fill-only, no hard border (Glass).
    Frosted,
    /// Sharp editorial outline: near-square, border-only (Lucid).
    Sharp,
}

/// Per-style chrome geometry and finish tokens that don't fit the semantic
/// sub-structs above (toolbar/pane-header heights, divider alphas, indicator
/// styles, focus-ring dimensions, drag handle, toast). All palette-independent
/// dimensions — colour comes from `ColorScheme` at render time.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Chrome {
    /// Multiplier on toolbar height (1.0 baseline, 1.4 Meridien tall).
    pub toolbar_height_scale: f32,
    /// Multiplier on pane-header height.
    pub header_height_scale: f32,
    /// Extra px added to the pane header when the user picks the COMPACT
    /// header size. Per-style, because how much a compact header can give up
    /// depends on what that style puts in it.
    ///
    /// AUDIT 2026-08 — this was a hardcoded index match, written twice:
    ///
    ///     match (style_id(wl), wl.pane_header_size) {
    ///         (1, Compact) => base + 2.0,
    ///         (2, Compact) => (base - 2.0).max(..),
    ///         _ => base,
    ///     }
    ///
    /// duplicated in `pane_header_h` and `pane_tabs_header_h`. `1` means
    /// Aperture and `2` means Octave only if you go and look, reordering the
    /// style list silently reassigns the tweaks, and a new style cannot express
    /// this at all without editing two functions in the renderer. Per-style
    /// behaviour belongs in the style.
    #[serde(default)]
    pub pane_header_compact_adjust: f32,
    /// Account strip panel height (px).
    pub account_strip_height: f32,
    /// Pane outline thickness (px).
    pub pane_border_width: f32,
    /// Gap between adjacent panes (px). >0 = tiled card layout.
    pub pane_gap: f32,
    /// Pane gap fill alpha (0-255). 0 = transparent gutters (show canvas bg).
    pub pane_gap_alpha: u8,
    /// Active pane indicator: 0=none, 1=top stripe, 2=header fill, 3=both.
    pub pane_active_indicator: u8,
    /// Active pane header fill multiplier (gamma over bg).
    pub active_header_fill_multiply: f32,
    /// Inactive pane header fill multiplier.
    pub inactive_header_fill_multiply: f32,
    /// Alpha of the hairline outer border around pane headers.
    pub header_outer_border_alpha: u8,
    /// Stroke width of the pane-header outer border (px).
    pub header_outer_border_width: f32,
    /// Alpha of the inter-section vertical dividers inside the pane header.
    pub header_divider_alpha: u8,
    /// Toolbar nav active-column tint alpha.
    pub nav_active_col_alpha: u8,
    /// Dialog / popup backdrop overlay alpha. 0 = no backdrop.
    pub dialog_backdrop_alpha: u8,
    /// Inactive tab text alpha multiplier (0.0-1.0).
    pub tab_inactive_alpha: f32,
    /// Inactive-tab hover background alpha.
    pub tab_hover_bg_alpha: u8,
    /// Active-tab underline thickness (px). 0 = no underline.
    pub tab_underline_thickness: f32,
    /// Section label top padding (px).
    pub section_label_padding_top: f32,
    /// Section label bottom padding (px).
    pub section_label_padding_bottom: f32,
    /// Drag handle (split divider) alpha multiplier (0.0-1.0).
    pub drag_handle_alpha: f32,
    /// Drag handle dot size multiplier.
    pub drag_handle_dot_scale: f32,
    /// Toast / status-bar background alpha.
    pub toast_bg_alpha: u8,
    /// Stripe/accent-banner fill alpha for order/alert cards.
    pub card_stripe_alpha: u8,
    /// Alpha for the floating card shadow when enabled.
    pub card_floating_shadow_alpha: u8,
    /// Saturation/brightness multiplier for accent on active elements.
    pub accent_emphasis: f32,
    /// Opacity multiplier for disabled widgets (0.0-1.0).
    pub disabled_opacity: f32,
    /// Focus ring stroke width (px).
    pub focus_ring_width: f32,
    /// Focus ring alpha (0-255).
    pub focus_ring_alpha: u8,
    /// Hover overlay alpha (0-255).
    pub hover_bg_alpha: u8,
    /// Active/pressed overlay alpha (0-255).
    pub active_bg_alpha: u8,

    // ── Shell region layout (floating-card chrome) ────────────────────────────
    /// Gap (px) between major shell regions (top-nav, tool layer, workspace,
    /// right rail). 0 = flush/contiguous chrome; 8 = Aperture floating cards.
    #[serde(default)]
    pub region_gap: f32,
    /// Corner radius (px) of each shell region card. 0 = square.
    #[serde(default = "Chrome::default_region_radius")]
    pub region_radius: f32,
    /// Border alpha (0-255) drawn around each shell region card.
    #[serde(default = "Chrome::default_region_border_alpha")]
    pub region_border_alpha: u8,

    // ── Nav cluster styling (top-nav segments) ────────────────────────────────
    /// Corner radius (px) of a nav cluster's background pill. 0 = square.
    #[serde(default = "Chrome::default_nav_cluster_radius")]
    pub nav_cluster_radius: f32,
    /// Fill alpha (0-255) of a nav cluster background over the toolbar surface.
    /// 0 = transparent clusters (default); >0 = visible grouped pills.
    #[serde(default)]
    pub nav_cluster_fill_alpha: u8,
    /// Horizontal inner padding (px) inside a nav cluster.
    #[serde(default = "Chrome::default_nav_cluster_padding")]
    pub nav_cluster_padding: f32,

    // ── Button group enclosure (toolbar button-section boxes) ─────────────────
    // A "button group" is a run of related toolbar buttons (sidebar toggles,
    // chart-tool dropdowns, actions). The *look* of the enclosure is now a named
    // recipe ([`GroupEnclosure`]) composed as an `Sx` at render time — adding a
    // new treatment is a new enum variant + its `Sx`, not four threaded numbers.
    #[serde(default)]
    pub button_group: GroupEnclosure,

    // ── Toolnav (second chrome row: tools + indicators + ticker) ──────────────
    /// Height (px) of the second toolbar row (the "toolnav"). 0 = single-row
    /// chrome (indicator dropdowns stay in the top nav). >0 = two-row chrome
    /// (indicators + ticker move to the toolnav). Aperture/Glass use ~30.
    #[serde(default)]
    pub toolnav_height: f32,

    // ── Footer (bottom dock: Orders / Positions / Account / Notifications) ─────
    /// Whether the bottom dock (footer) is open by *default* for this style.
    /// The user can always toggle it regardless (Ctrl+`), and that session
    /// override wins — exactly mirroring the toolnav's hybrid visibility.
    #[serde(default)]
    pub footer_default_open: bool,

    // ── Side panel anatomy (header / sections / footer card) ──────────────────
    /// Header toggle/tab treatment for side panels (WATCH/POS/ALERT etc).
    /// 0=Line, 1=Segmented, 2=Filled, 3=Card, 4=Pane — mirrors `panel_tab_treatment`
    /// but scoped to the panel header strip.
    #[serde(default)]
    pub panel_header_treatment: u8,
    /// Fill alpha (0-255) of a `PanelSection` body band over the panel surface.
    /// 0 = transparent/flat sections; >0 = visible grouped bands.
    #[serde(default)]
    pub panel_section_fill_alpha: u8,
    /// When true, a pinned panel footer renders as an elevated rounded *card*
    /// (the ApertureJune P&L block) instead of a flat band.
    #[serde(default)]
    pub panel_footer_card: bool,
    /// Corner radius (px) of the pinned footer card. 0 = square.
    #[serde(default = "Chrome::default_panel_footer_radius")]
    pub panel_footer_radius: f32,
}

impl Chrome {
    fn default_region_radius()       -> f32 { 12.0 }
    fn default_region_border_alpha() -> u8  { 40 }
    fn default_nav_cluster_radius()  -> f32 { 8.0 }
    fn default_nav_cluster_padding() -> f32 { 6.0 }
    fn default_panel_footer_radius() -> f32 { 10.0 }
}

impl Default for Chrome {
    fn default() -> Self {
        // Matches style_defaults(_) Meridien baseline where a value exists,
        // else the neutral 1.0/baseline.
        Self {
            toolbar_height_scale: 1.0,
            header_height_scale: 1.0,
            pane_header_compact_adjust: 0.0,
            account_strip_height: 26.0,
            pane_border_width: 1.0,
            pane_gap: 0.0,
            pane_gap_alpha: 0,
            pane_active_indicator: 2,
            active_header_fill_multiply: 0.7,
            inactive_header_fill_multiply: 1.08,
            header_outer_border_alpha: 38,
            header_outer_border_width: 0.5,
            header_divider_alpha: 50,
            nav_active_col_alpha: 0,
            dialog_backdrop_alpha: 0,
            tab_inactive_alpha: 0.55,
            tab_hover_bg_alpha: 18,
            tab_underline_thickness: 2.0,
            section_label_padding_top: 4.0,
            section_label_padding_bottom: 2.0,
            drag_handle_alpha: 0.6,
            drag_handle_dot_scale: 1.0,
            toast_bg_alpha: 220,
            card_stripe_alpha: 255,
            card_floating_shadow_alpha: 0,
            accent_emphasis: 1.0,
            disabled_opacity: 0.5,
            focus_ring_width: 1.5,
            focus_ring_alpha: 110,
            hover_bg_alpha: 18,
            active_bg_alpha: 30,
            region_gap: 0.0,
            region_radius: 12.0,
            region_border_alpha: 40,
            nav_cluster_radius: 8.0,
            nav_cluster_fill_alpha: 0,
            nav_cluster_padding: 6.0,
            // No enclosure by default — flat styles space buttons + draw dividers.
            button_group: GroupEnclosure::None,
            toolnav_height: 0.0,
            footer_default_open: false,
            panel_header_treatment: 0,
            panel_section_fill_alpha: 0,
            panel_footer_card: false,
            panel_footer_radius: 10.0,
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
    /// Chrome geometry + finish tokens (toolbar/pane heights, dividers, focus ring…).
    #[serde(default)]
    pub chrome:     Chrome,
    #[serde(default)]
    pub icons:      Icons,
    #[serde(default)]
    pub line_heights: LineHeights,

    /// M1 Change D: display-numeral treatment. `None` = classic mono.
    #[serde(default)]
    pub numerals: Option<NumeralTier>,

    /// DS-6.0: shell shape + the theme's DEFAULT layout archetype.
    ///
    /// Additive with `#[serde(default)]`, so no `CURRENT_SCHEMA_VERSION` bump
    /// and every existing `.apextheme` keeps loading — same additive route M1
    /// Change E took. A pack that says nothing about the shell gets
    /// `ShellSpec::default()`, which is today's shape exactly.
    #[serde(default)]
    pub shell: ShellSpec,
}

// ── DS-6.0: shell shape ──────────────────────────────────────────────────────
//
// Vocabulary adopted from `docs/migration/shell-profile.md` (Stream S6). That
// document remains the reference for what these variants MEAN; the decision to
// carry them here rather than in a parallel `ShellProfile` store is recorded in
// `docs/handoffs/frontend-ds-adoption/13-DS-6.0-DECISION.md` (D1).
//
// The point of absorbing them: layout selection resolves through the SAME path
// as colour and dimension, so there is exactly one mechanism. A separate
// ShellProfile store would have been a second one — logged as risk R5.

/// The primary navigation shape. See shell-profile.md §NavStyle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum NavStyle {
    /// Single horizontal pill bar across the top — today's only shape.
    #[default]
    TopPills,
    /// Horizontal tab strip, Bloomberg-style; taller than `TopPills`.
    TopTabs,
    /// Vertical icon strip on the left, replacing the top bar.
    SideRail,
    /// Minimal-height menu bar with dropdown items, no icon pills.
    MenuBar,
}

/// Where the bottom trading dock sits and what shape it takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum DockStyle {
    /// Full-width bottom panel — today's shape.
    #[default]
    BottomBar,
    /// Collapsed to its tab strip by default; click a tab to expand.
    BottomPill,
    /// No bottom dock; its tabs live in right-rail panels instead.
    Hidden,
}

/// Which side the persistent panel stack occupies.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum RailSide {
    /// Right side — today's only variant.
    #[default]
    Right,
    /// Left side.
    Left,
    /// No persistent rail; panels open as floating windows.
    Floating,
}

/// The layout archetype — how the CENTRAL content area is organised.
///
/// The four identified in `06-LAYOUT-ARCHETYPES`. This is the theme's default;
/// a workspace may override it (see [`ShellSpec`] docs for why the override
/// lives on the workspace and not here).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Archetype {
    /// D — Alto / Mariner: the classic trading shell. Today's layout, and the
    /// default because it is what every existing workspace already is.
    #[default]
    TradingShell,
    /// A — Aperture: 12-column x 92px tile mosaic (solved exactly by the M4.4
    /// `Grid`: 4-col hero = 436px, 2-row = 196px).
    Mosaic,
    /// B — Cadence: dense screens.
    DenseScreens,
    /// C — Lucid / Meridien: the editorial dashboard (300 / 1fr / 360 columns).
    Editorial,
}

impl Archetype {
    /// Stable name — what gets persisted and exported.
    ///
    /// Matches the `Debug` spelling so the DTCG export and this stay in step;
    /// [`Archetype::from_name`] is its exact inverse, asserted by
    /// `archetype_name_round_trips`.
    pub fn name(self) -> &'static str {
        match self {
            Archetype::TradingShell => "TradingShell",
            Archetype::Mosaic       => "Mosaic",
            Archetype::DenseScreens => "DenseScreens",
            Archetype::Editorial    => "Editorial",
        }
    }

    /// Parse a stored name. `None` for anything unrecognised — a workspace
    /// saved by a newer build must fall back to the theme's archetype rather
    /// than land on an arbitrary one.
    pub fn from_name(s: &str) -> Option<Self> {
        match s {
            "TradingShell" => Some(Archetype::TradingShell),
            "Mosaic"       => Some(Archetype::Mosaic),
            "DenseScreens" => Some(Archetype::DenseScreens),
            "Editorial"    => Some(Archetype::Editorial),
            _ => None,
        }
    }

    /// Every variant, for pickers.
    pub const ALL: [Archetype; 4] = [
        Archetype::TradingShell,
        Archetype::Mosaic,
        Archetype::DenseScreens,
        Archetype::Editorial,
    ];
}

/// Shell shape carried by a [`StyleSystem`].
///
/// DS-6.0 D1. `archetype` here is the theme's DEFAULT, not the final answer —
/// a workspace may override it, and resolution is deliberately the same
/// one-line shape as the colour and dimension axes:
///
/// ```ignore
/// workspace.archetype.unwrap_or(active_style.shell.archetype)
/// ```
///
/// The override lives on the WORKSPACE rather than on the theme on purpose: a
/// user choosing a layout is expressing a preference about that workspace, not
/// editing the design system. Themes are exportable as `.apextheme`, so storing
/// it here would let a personal preference travel inside a shared artifact.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ShellSpec {
    #[serde(default)]
    pub nav: NavStyle,
    #[serde(default)]
    pub dock: DockStyle,
    #[serde(default)]
    pub rail: RailSide,
    #[serde(default)]
    pub archetype: Archetype,
}

impl ShellSpec {
    /// Resolve the archetype actually in effect, given an optional
    /// per-workspace override. The whole of D1's precedence rule.
    #[inline]
    pub fn resolve_archetype(&self, workspace_override: Option<Archetype>) -> Archetype {
        workspace_override.unwrap_or(self.archetype)
    }
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
            // DS-6.0: the default style is today's shape — TopPills / BottomBar
            // / Right rail / TradingShell. Every field of ShellSpec defaults to
            // the variant that already exists, so this is a no-op at runtime.
            shell:      ShellSpec::default(),
            typography: Typography::default(),
            icons:      Icons::default(),
            line_heights: LineHeights::default(),
            spacing:    Spacing::default(),
            radii:      Radii::default(),
            strokes:    Strokes::default(),
            alphas:     Alphas::default(),
            elevation:  Elevation::default(),
            density:    Density::default(),
            shadows:    Shadows::default(),
            treatments: Treatments::default(),
            chrome:     Chrome::default(),
            numerals: None,
        }
    }

    /// "Meridien" style — sharp corners, hairline borders, solid active fills,
    /// uppercase section labels.  The first authentic style beyond the default.
    pub fn meridien() -> Self {
        Self {
            meta: Meta::new("meridien", "Meridien", true),
            // DS-6.0: match the BUILTIN Meridien's archetype. This is a second,
            // simplified construction of "Meridien" used by the import tests;
            // it round-trips against itself, so nothing would have failed if the
            // two disagreed — but then the committed Figma fixture would claim
            // Meridien is a trading shell, which is simply false.
            shell: ShellSpec { archetype: Archetype::Editorial, ..ShellSpec::default() },
            radii: Radii { none: 0.0, xs: 0.0, sm: 0.0, md: 0.0, lg: 0.0, full: 9999.0, ..Radii::default() },
            strokes: Strokes { hair: 0.3, thin: 0.5, medium: 0.8, std: 0.5, bold: 1.0, thick: 1.5, md: 1.0, heavy: 1.5 },
            treatments: Treatments {
                solid_active_fills:       true,
                hairline_borders:         true,
                uppercase_section_labels: true,
                segmented_filled_idle:    false,
                focus_ring:               FocusRingStyle::Outline,
                surface_bevel:            BevelStyle::None, // editorial — flat
                bevel_highlight_alpha:    0,
                bevel_shadow_alpha:       0,
                wl_row_divider_alpha:     30,
                section_header_mono:      false,
                ..Treatments::default()
            },
            ..Self::builtin_default()
        }
    }
}

#[cfg(test)]
mod shell_spec_tests {
    use super::*;

    /// DS-6.0 D1, stated as an assertion: the theme supplies the DEFAULT, the
    /// workspace overrides it, and there is exactly one precedence rule.
    #[test]
    fn workspace_override_beats_theme_default() {
        let editorial = ShellSpec { archetype: Archetype::Editorial, ..ShellSpec::default() };

        // No override → the theme's archetype.
        assert_eq!(editorial.resolve_archetype(None), Archetype::Editorial);
        // Override → the user's choice, whatever the theme says.
        assert_eq!(
            editorial.resolve_archetype(Some(Archetype::Mosaic)),
            Archetype::Mosaic,
        );
    }

    /// The default shell must be TODAY'S shape, or adding this field silently
    /// restyles every existing theme and every pack that omits it.
    #[test]
    fn default_shell_is_todays_shape() {
        let d = ShellSpec::default();
        assert_eq!(d.nav, NavStyle::TopPills);
        assert_eq!(d.dock, DockStyle::BottomBar);
        assert_eq!(d.rail, RailSide::Right);
        assert_eq!(d.archetype, Archetype::TradingShell);
        assert_eq!(StyleSystem::builtin_default().shell, d);
    }

    /// The archetype map from `06-LAYOUT-ARCHETYPES` §6, pinned so a builtin
    /// cannot quietly lose the layout it was designed around.
    ///
    /// This is the DS-6.0 analogue of the M5 relationship invariants: the
    /// values are individually legal, so only asserting the MAPPING catches a
    /// theme drifting away from its own design.
    #[test]
    fn builtins_keep_their_designed_archetype() {
        use crate::design_system::builtin_style_systems;
        let want = [
            ("meridien", Archetype::Editorial),
            ("aperture", Archetype::Mosaic),
            ("cadence",  Archetype::DenseScreens),
            ("lucid",    Archetype::Editorial),
            // alto / mariner ARE the trading shell; octave / relay / glass inherit it.
            ("alto",     Archetype::TradingShell),
            ("mariner",  Archetype::TradingShell),
        ];
        let systems = builtin_style_systems();
        for (id, arch) in want {
            let ss = systems
                .iter()
                .find(|s| s.meta.id == id)
                .unwrap_or_else(|| panic!("builtin style '{id}' missing"));
            assert_eq!(ss.shell.archetype, arch, "style '{id}' lost its designed archetype");
        }
    }
}

#[cfg(test)]
mod archetype_name_tests {
    use super::*;

    /// `name()` and `from_name()` must be exact inverses: the same string is
    /// what the DTCG export writes AND what a workspace persists, so a drift
    /// between them would silently reset one of the two to the default.
    #[test]
    fn archetype_name_round_trips() {
        for a in Archetype::ALL {
            assert_eq!(Archetype::from_name(a.name()), Some(a), "{a:?} did not round-trip");
            // The persisted spelling must also match the Debug spelling the
            // DTCG exporter emits via `format!("{:?}")`.
            assert_eq!(a.name(), format!("{a:?}"), "{a:?} name/Debug drift");
        }
    }

    /// An unknown name (a workspace written by a newer build) falls back
    /// rather than landing on an arbitrary variant.
    #[test]
    fn unknown_archetype_name_is_none() {
        assert_eq!(Archetype::from_name("Holographic"), None);
        assert_eq!(Archetype::from_name(""), None);
    }
}
