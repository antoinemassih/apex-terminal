//! `ColorScheme` — the palette axis of the two-axis theme system.
//!
//! A `ColorScheme` holds **only colour** — background, surface, text, accent,
//! semantic colours (bull/bear/warn), and a shadow tint.  It has no knowledge
//! of typography, spacing, or any other dimension-axis concern.
//!
//! # Wire format
//! Serialises to / deserialises from W3C DTCG token JSON via the
//! [`super::loader`] module.  Direct `serde` impls use the flat struct form
//! (not DTCG-wrapped) for internal persistence.

use serde::{Deserialize, Serialize};

// ── Rgba ────────────────────────────────────────────────────────────────────

/// A 4-channel colour value: `[red, green, blue, alpha]`, each 0–255.
///
/// Chosen as `[u8; 4]` (not an egui type) so the schema crate has no egui
/// dependency and DTCG JSON round-trips without loss.
pub type Rgba = [u8; 4];

/// Convenience constructors.
pub mod rgba {
    use super::Rgba;

    /// Fully opaque RGB colour.
    #[inline]
    pub const fn rgb(r: u8, g: u8, b: u8) -> Rgba {
        [r, g, b, 255]
    }

    /// RGBA colour.
    #[inline]
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Rgba {
        [r, g, b, a]
    }

    /// Parse `#rrggbb` or `#rrggbbaa` hex strings.
    /// Returns `None` on malformed input.
    pub fn from_hex(s: &str) -> Option<Rgba> {
        let s = s.trim_start_matches('#');
        match s.len() {
            6 => {
                let r = u8::from_str_radix(&s[0..2], 16).ok()?;
                let g = u8::from_str_radix(&s[2..4], 16).ok()?;
                let b = u8::from_str_radix(&s[4..6], 16).ok()?;
                Some([r, g, b, 255])
            }
            8 => {
                let r = u8::from_str_radix(&s[0..2], 16).ok()?;
                let g = u8::from_str_radix(&s[2..4], 16).ok()?;
                let b = u8::from_str_radix(&s[4..6], 16).ok()?;
                let a = u8::from_str_radix(&s[6..8], 16).ok()?;
                Some([r, g, b, a])
            }
            _ => None,
        }
    }

    /// Format as `#rrggbbaa`.
    pub fn to_hex(c: Rgba) -> String {
        format!("#{:02x}{:02x}{:02x}{:02x}", c[0], c[1], c[2], c[3])
    }
}

// ── Meta ────────────────────────────────────────────────────────────────────

/// Identity and display metadata shared by both axes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Meta {
    /// Stable machine identifier (e.g. `"dracula"`, `"meridien"`).
    pub id: String,
    /// Human-readable display name.
    pub name: String,
    /// `true` for dark palettes; `false` for light ones.
    pub is_dark: bool,
}

impl Meta {
    pub fn new(id: impl Into<String>, name: impl Into<String>, is_dark: bool) -> Self {
        Self { id: id.into(), name: name.into(), is_dark }
    }
}

// ── Command-palette default ─────────────────────────────────────────────────

/// Default 11-colour command-palette category badge palette.
///
/// Slot order: `[symbol, widget, overlay, theme, timeframe, layout, play,
/// alert, ai, dynamic, calc]`. All slots opaque (alpha 255).
///
/// This is the seed used by every built-in `ColorScheme`. Per-theme overrides
/// are supported by setting a different array on the `cmd_palette` field; the
/// adapter reads from the scheme, never from this const.
pub const CMD_PALETTE_DEFAULT: [Rgba; 11] = [
    rgba::rgb(120, 180, 255), // symbol
    rgba::rgb(180, 140, 240), // widget
    rgba::rgb(160, 200, 140), // overlay
    rgba::rgb(240, 180, 140), // theme
    rgba::rgb(140, 220, 200), // timeframe
    rgba::rgb(220, 200, 120), // layout
    rgba::rgb(240, 140, 180), // play
    rgba::rgb(240, 120, 120), // alert
    rgba::rgb(255, 120, 200), // ai
    rgba::rgb(255, 180,  80), // dynamic
    rgba::rgb(140, 240, 200), // calc
];

// ── ColorScheme ─────────────────────────────────────────────────────────────

/// Axis 2 — the palette. Pure colour. No dimension values.
///
/// Corresponds to the `gpu.rs::Theme` palette columns; maps 1-to-1 with the
/// DTCG `colorscheme.*.json` file kind (§6 of the spec).
///
/// **Rule:** never add non-colour fields here. Typography, spacing, alpha
/// values etc. belong in [`super::style_system::StyleSystem`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ColorScheme {
    /// Identity and dark/light flag.
    pub meta: Meta,

    // ── Background layers ──────────────────────────────────────────────────
    /// Deepest background (window / canvas floor).
    pub bg: Rgba,
    /// Elevated surface (panels, cards, toolbars).
    pub surface: Rgba,

    // ── Text ───────────────────────────────────────────────────────────────
    /// Primary text / foreground.
    pub text: Rgba,
    /// Muted / secondary text, disabled labels, placeholders.
    pub dim: Rgba,

    // ── Structural chrome ──────────────────────────────────────────────────
    /// Borders, dividers, hairlines.
    pub border: Rgba,

    // ── Semantic ───────────────────────────────────────────────────────────
    /// Brand / interactive accent (buttons, focus rings, links).
    pub accent: Rgba,
    /// Upward / positive price movement.
    pub bull: Rgba,
    /// Downward / negative price movement.
    pub bear: Rgba,
    /// Warning / caution state.
    pub warn: Rgba,

    // ── Extended semantic palette (PALETTE-DEPTH decision) ─────────────────
    // Independent semantic slots distinct from the trading-specific bull/bear.
    // Defaults to bull/bear/warn so existing themes are visually unchanged.
    // New themes may set these independently for richer UI states.

    /// General success / positive state (not price-specific).
    /// Defaults to `bull` at render time when `None`.
    #[serde(default)]
    pub success: Option<Rgba>,
    /// General danger / error / destructive action state.
    /// Defaults to `bear` at render time when `None`.
    #[serde(default)]
    pub danger: Option<Rgba>,
    /// General warning / caution state (semantic alias over `warn`).
    /// Defaults to `warn` at render time when `None`.
    #[serde(default)]
    pub warning: Option<Rgba>,
    /// Informational / neutral highlight (help text, info badges, links).
    /// No direct legacy equivalent; defaults to a generic muted blue at render time.
    #[serde(default)]
    pub info: Option<Rgba>,

    // ── Axis-violation fix: pane gap color ────────────────────────────────
    // Previously `StyleSettings.pane_gap_color: Option<Color32>` lived on the
    // dimension axis — a color field in a dimension struct. Moved here where it
    // belongs. None = renderers derive gap color from bg/border at paint time.
    /// Override colour for the gutter between adjacent panes.
    /// `None` = derive from `bg`/`border` at paint time (most themes leave this `None`).
    #[serde(default)]
    pub pane_gap_color: Option<Rgba>,

    // ── M1 Change A: the AUTHORED SURFACE RAMP (design-brief 4.1) ────────────
    //
    // Design systems hand-author a 4-step background ramp and 4-step ink ramp;
    // previously we derived them via the achromatic `elevate()`, which cannot
    // produce Aperture's warm #141311 from #000000 and cannot express Lucid's
    // non-monotonic paper ramp (panel LIGHTER than canvas, surface darker).
    // `None` keeps today's derived behaviour byte-for-byte; authored values
    // win at the ComponentTheme boundary.

    /// Panel surface — one step off the canvas (DS `--ds-bg-panel`).
    #[serde(default)]
    pub bg_panel: Option<Rgba>,
    /// Elevated surface — popovers, raised cards (DS `--ds-bg-elevated`).
    #[serde(default)]
    pub bg_elevated: Option<Rgba>,
    /// Hover wash carrying HUE, not just alpha (DS `--ds-bg-hover`) —
    /// Aperture's hover is accent-tinted; Cadence's is neutral white.
    #[serde(default)]
    pub bg_hover: Option<Rgba>,
    /// Fourth ink step below `text_muted` (DS `--ds-fg-xmuted`).
    #[serde(default)]
    pub fg_xmuted: Option<Rgba>,
    /// Secondary accent — gradients, sub-emphasis (DS `--ds-accent-sub`).
    #[serde(default)]
    pub accent_sub: Option<Rgba>,
    /// Authored bull/bear washes — row tints, depth bars
    /// (DS `--ds-bull-alpha` / `--ds-bear-alpha`).
    #[serde(default)]
    pub bull_alpha: Option<Rgba>,
    #[serde(default)]
    pub bear_alpha: Option<Rgba>,
    /// Dimmer hairline distinct from `border` (DS `--ds-border-dim`).
    #[serde(default)]
    pub border_dim: Option<Rgba>,

    // ── M1 Change C: AUTHORED BEVEL TINTS ────────────────────────────────────
    // `Treatments.surface_bevel` geometry exists; its tint was luminance-
    // derived (achromatic), which made Alto's WARM rgba(255,238,210) highlight
    // vs Mariner's COOL rgba(190,215,245) inexpressible — the single palette-
    // level difference between those sibling themes. Colour belongs on the
    // palette axis; intensity (`bevel_*_alpha`) stays on Treatments.
    /// Bevel top-highlight tint. `None` → luminance-derived (today's look).
    #[serde(default)]
    pub bevel_highlight: Option<Rgba>,
    /// Bevel bottom-shadow tint. `None` → luminance-derived.
    #[serde(default)]
    pub bevel_shadow: Option<Rgba>,

    // ── Shadow ─────────────────────────────────────────────────────────────
    /// Shadow tint colour (used by elevation helpers). Typically near-black
    /// for dark themes, near-neutral for light themes.
    pub shadow: Rgba,

    // ── Hand-authored extras (per-theme, not derivable) ────────────────────
    // These fields carry values that differ per theme and cannot be computed
    // from the base fields. Required to make `color_scheme_to_theme` lossless.

    /// Alert / notification badge colour (legacy: `t.notification_red`).
    pub notification_red: Rgba,
    /// Gold accent; typically a warm yellow used for highlights (legacy: `t.gold`).
    pub gold: Rgba,
    /// Overlay / HUD foreground text colour (legacy: `t.overlay_text`).
    pub overlay_text: Rgba,
    /// RRG leading quadrant colour (strong bull).
    pub rrg_leading: Rgba,
    /// RRG improving quadrant colour (trending up).
    pub rrg_improving: Rgba,
    /// RRG weakening quadrant colour (warning, trending down).
    pub rrg_weakening: Rgba,
    /// RRG lagging quadrant colour (strong bear).
    pub rrg_lagging: Rgba,
    /// Subtle tint applied behind pinned rows (premultiplied alpha).
    pub pinned_row_tint: Rgba,
    /// Muted text variant (secondary body copy).
    pub text_muted: Rgba,
    /// HUD / floating overlay background colour (premultiplied alpha).
    pub hud_bg: Rgba,
    /// HUD / floating overlay border colour.
    pub hud_border: Rgba,

    /// 11-colour command-palette category badges. Defaults to
    /// [`CMD_PALETTE_DEFAULT`] for all built-in schemes; per-theme overrides
    /// supported by setting a different array here.
    #[serde(default = "default_cmd_palette")]
    pub cmd_palette: [Rgba; 11],
}

#[inline]
fn default_cmd_palette() -> [Rgba; 11] { CMD_PALETTE_DEFAULT }

impl ColorScheme {
    /// The built-in dark default — a neutral dark theme suitable as a
    /// fallback when no user-provided scheme is active.
    pub const fn default_dark() -> Self {
        ColorScheme {
            meta: Meta { id: String::new(), name: String::new(), is_dark: true },
            bg:      rgba::rgb(18,  18,  18),
            surface: rgba::rgb(28,  28,  28),
            text:    rgba::rgb(220, 220, 220),
            dim:     rgba::rgb(134, 134, 134),
            border:  rgba::rgb(55,  55,  55),
            accent:  rgba::rgb(99,  102, 241),
            bull:    rgba::rgb(52,  211, 153),
            bear:    rgba::rgb(248, 113, 113),
            warn:    rgba::rgb(251, 191,  36),
            // Extended semantic palette: None → fallback to bull/bear/warn at render time.
            success: None,
            danger:  None,
            warning: None,
            info:    None,
            pane_gap_color: None,
            shadow:  rgba::rgba(0, 0, 0, 180),
            // extras: sensible generic defaults
            notification_red: rgba::rgb(231,  76,  60),
            gold:             rgba::rgb(255, 193,  37),
            overlay_text:     rgba::rgb(240, 240, 240),
            rrg_leading:      rgba::rgb( 52, 211, 153),
            rrg_improving:    rgba::rgb( 99, 102, 241),
            rrg_weakening:    rgba::rgb(251, 191,  36),
            rrg_lagging:      rgba::rgb(248, 113, 113),
            pinned_row_tint:  rgba::rgba(  0,   0,   0, 12),
            text_muted:       rgba::rgb(170, 170, 180),
            hud_bg:           rgba::rgba(  0,   0,   0, 230),
            hud_border:       rgba::rgb( 50,  50,  60),
            cmd_palette:      CMD_PALETTE_DEFAULT,
            bg_panel: None, bg_elevated: None, bg_hover: None, fg_xmuted: None,
            accent_sub: None, bull_alpha: None, bear_alpha: None, border_dim: None,
            bevel_highlight: None, bevel_shadow: None,
        }
    }

    // ── Resolved semantic palette accessors ────────────────────────────────
    // These fall back to the legacy bull/bear/warn fields when the extended
    // semantic slots are unset, preserving the existing visual for all themes
    // that were defined before the PALETTE-DEPTH decision.

    /// Resolved success colour: `success` if set, otherwise `bull`.
    #[inline]
    pub fn resolved_success(&self) -> Rgba { self.success.unwrap_or(self.bull) }

    /// Resolved danger colour: `danger` if set, otherwise `bear`.
    #[inline]
    pub fn resolved_danger(&self) -> Rgba { self.danger.unwrap_or(self.bear) }

    /// Resolved warning colour: `warning` if set, otherwise `warn`.
    #[inline]
    pub fn resolved_warning(&self) -> Rgba { self.warning.unwrap_or(self.warn) }

    /// Resolved info colour: `info` if set, otherwise a neutral blue
    /// appropriate for the dark/light context.
    #[inline]
    pub fn resolved_info(&self) -> Rgba {
        self.info.unwrap_or_else(|| {
            if self.meta.is_dark {
                rgba::rgb(100, 160, 220) // muted blue for dark themes
            } else {
                rgba::rgb(30, 100, 180)  // deeper blue for light themes
            }
        })
    }
}

// `const` can't initialise non-`Copy` types with heap allocation (Vec), so
// provide an `fn` builder that returns an owned value for registry init.
/// Built-in dark default `ColorScheme` (heap-allocated, registry-ready).
pub fn builtin_dark() -> ColorScheme {
    ColorScheme {
        meta: Meta::new("apex-dark", "Apex Dark", true),
        bg:      rgba::rgb(18,  18,  18),
        surface: rgba::rgb(28,  28,  28),
        text:    rgba::rgb(220, 220, 220),
        dim:     rgba::rgb(134, 134, 134),
        border:  rgba::rgb(55,  55,  55),
        accent:  rgba::rgb(99,  102, 241),
        bull:    rgba::rgb(52,  211, 153),
        bear:    rgba::rgb(248, 113, 113),
        warn:    rgba::rgb(251, 191,  36),
        // Extended semantic palette: None = fallback to bull/bear/warn at render time.
        success: None,
        danger:  None,
        warning: None,
        info:    None,
        pane_gap_color: None,
        shadow:  rgba::rgba(0, 0, 0, 180),
        notification_red: rgba::rgb(231,  76,  60),
        gold:             rgba::rgb(255, 193,  37),
        overlay_text:     rgba::rgb(240, 240, 240),
        rrg_leading:      rgba::rgb( 52, 211, 153),
        rrg_improving:    rgba::rgb( 99, 102, 241),
        rrg_weakening:    rgba::rgb(251, 191,  36),
        rrg_lagging:      rgba::rgb(248, 113, 113),
        pinned_row_tint:  rgba::rgba(  0,   0,   0, 12),
        text_muted:       rgba::rgb(170, 170, 180),
        hud_bg:           rgba::rgba(  0,   0,   0, 230),
        hud_border:       rgba::rgb( 50,  50,  60),
        cmd_palette:      CMD_PALETTE_DEFAULT,
        bg_panel: None, bg_elevated: None, bg_hover: None, fg_xmuted: None,
            accent_sub: None, bull_alpha: None, bear_alpha: None, border_dim: None,
            bevel_highlight: None, bevel_shadow: None,
    }
}

/// Built-in light default `ColorScheme`.
pub fn builtin_light() -> ColorScheme {
    ColorScheme {
        meta: Meta::new("apex-light", "Apex Light", false),
        bg:      rgba::rgb(248, 248, 248),
        surface: rgba::rgb(255, 255, 255),
        text:    rgba::rgb(20,  20,  20),
        dim:     rgba::rgb(112, 112, 112),
        border:  rgba::rgb(200, 200, 200),
        accent:  rgba::rgb(79,  70,  229),
        bull:    rgba::rgb(22, 163,  74),
        bear:    rgba::rgb(220,  38,  38),
        warn:    rgba::rgb(202, 138,   4),
        // Extended semantic palette: None = fallback to bull/bear/warn at render time.
        success: None,
        danger:  None,
        warning: None,
        info:    None,
        pane_gap_color: None,
        shadow:  rgba::rgba(0, 0, 0, 80),
        notification_red: rgba::rgb(220,  38,  38),
        gold:             rgba::rgb(202, 138,   4),
        overlay_text:     rgba::rgb(20,  20,  20),
        rrg_leading:      rgba::rgb(22, 163,  74),
        rrg_improving:    rgba::rgb(79,  70, 229),
        rrg_weakening:    rgba::rgb(202, 138,   4),
        rrg_lagging:      rgba::rgb(220,  38,  38),
        pinned_row_tint:  rgba::rgba(  0,   0,   0, 12),
        text_muted:       rgba::rgb(100, 100, 110),
        hud_bg:           rgba::rgba( 20,  20,  20, 220),
        hud_border:       rgba::rgb( 80,  80,  88),
        cmd_palette:      CMD_PALETTE_DEFAULT,
        bg_panel: None, bg_elevated: None, bg_hover: None, fg_xmuted: None,
            accent_sub: None, bull_alpha: None, bear_alpha: None, border_dim: None,
            bevel_highlight: None, bevel_shadow: None,
    }
}
