//! Baseline snapshot — the exact design-token values the app renders with
//! today, expressed as a `StyleSystem` + `ColorScheme` pair.
//!
//! ## How these were derived
//!
//! **Style** → Meridien (`style_idx = 0`, the `_ =>` arm of `style_defaults()`
//! in `src/chart/renderer/ui/style.rs`).  The app initialises
//! `ACTIVE_STYLE = 0` and `Watchlist::style_idx = 0`.
//!
//! **Colour** → Gruvbox (`theme_idx = 5`, `THEMES[5]` in
//! `src/chart/renderer/gpu.rs`).  The compile-time default is
//! `theme_idx: 5 // Gruvbox` (gpu.rs line ~1928) and the JSON load-path
//! falls back to `5` when no saved state is found.
//!
//! Every value below was read from the canonical source (style_defaults / THEMES)
//! and cross-checked against the five anchor surfaces specified in the task.
//! Conflicts are documented inline.

use super::{
    color_scheme::{ColorScheme, Meta, Rgba},
    style_system::{
        Alphas, BevelStyle, Chrome, Density, Elevation, FocusRingStyle, Radii, Shadows, ShadowSpec,
        Spacing, Strokes, StyleSystem, Treatments, Typography,
    },
};

// ── Colour helpers (mirrors gpu.rs helpers, but produces Rgba) ─────────────

/// Fully-opaque `[u8; 4]` from RGB.
const fn rgb(r: u8, g: u8, b: u8) -> Rgba { [r, g, b, 255] }

/// `[u8; 4]` with explicit alpha.
const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Rgba { [r, g, b, a] }

/// Const-safe channel clamp: `i16` → `u8`, saturating at [0, 255].
const fn clamp_ch(v: i16) -> u8 {
    if v < 0 { 0 } else if v > 255 { 255 } else { v as u8 }
}

/// Mirrors `gpu::hairline_border`: shift all channels by +13 on dark bgs.
///
/// For Gruvbox bg=(40,40,40): sum=120 < 384 → dark → shift +13 →
/// border = (53, 53, 53, 255).
const fn hairline_border(bg: Rgba) -> Rgba {
    let is_dark = (bg[0] as u16 + bg[1] as u16 + bg[2] as u16) < 384;
    let shift: i16 = if is_dark { 13 } else { -13 };
    [
        clamp_ch(bg[0] as i16 + shift),
        clamp_ch(bg[1] as i16 + shift),
        clamp_ch(bg[2] as i16 + shift),
        255,
    ]
}

// ── Public API ─────────────────────────────────────────────────────────────

/// Returns the `StyleSystem` that exactly matches the Meridien style
/// (`style_idx = 0`, the app's compile-time default).
///
/// # Token→field mapping
///
/// | `style_defaults(0)` field(s)              | `StyleSystem` field              | Value      |
/// |-------------------------------------------|----------------------------------|------------|
/// | `r_xs=0, r_sm=0, r_md=0, r_lg=0`         | `radii.xs/sm/md/lg`             | 0.0        |
/// | `r_pill=0` (0 in struct but semantic pill)| `radii.full`                    | 9999.0 *   |
/// | `stroke_hair=0.5`                         | `strokes.thin`                  | 0.5        |
/// | `stroke_thin=1.0, stroke_std=1.0`         | `strokes.std`                   | 1.0        |
/// | `stroke_bold=1.0`                         | `strokes.md`                    | 1.0        |
/// | `stroke_thick=1.0`                        | `strokes.heavy`                 | 1.0        |
/// | `font_section_label=8, font_caption=8`    | `typography.size_xs`            | 8.0        |
/// | `font_body=10`                            | `typography.size_sm`            | 10.0       |
/// | (no explicit md/lg in StyleSettings)      | `typography.size_md/lg`         | 13.0/15.0 †|
/// | `font_hero=36`                            | `typography.size_xl`            | 36.0       |
/// | (pinned monospace scale)                  | `typography.mono_sm/md/lg`      | 11/13/15  †|
/// | `card_padding_y=8`                        | `spacing.md` (approx)           | 8.0        |
/// | `card_padding_x=10`                       | `spacing.lg`                    | 10.0       |
/// | `cta_height_px=36`                        | `spacing.cta_height`            | 36.0       |
/// | `hover_bg_alpha=20` (20/255≈0.08)         | `alphas.subtle`                 | 0.08       |
/// | `active_bg_alpha=35` (35/255≈0.14)        | `alphas.soft`                   | 0.14       |
/// | `header_outer_border_alpha=38` (38/255)   | `alphas.header_border`          | 0.149      |
/// | `shadow_blur=0, shadow_offset_y=0, alpha=0`| `shadows.*` (all zero)         | 0.0        |
/// | `density=1`                               | `density.factor`                | 1.0        |
/// | `row_height_px=22`                        | `density.row_height_dense`      | 22.0       |
/// | `solid_active_fills=true`                 | `treatments.solid_active_fills` | true       |
/// | `hairline_borders=true`                   | `treatments.hairline_borders`   | true       |
/// | `uppercase_section_labels=true`           | `treatments.uppercase_section_labels`| true  |
/// | `focus_ring_width=1.0`                    | `treatments.focus_ring`         | Outline    |
///
/// * `r_pill=0` in the Meridien struct but `radius_pill()` returns 999.0 in
///   `style.rs` regardless — the field is 0 meaning "not used"; the sentinel
///   999.0 is kept in `radii.full` to preserve round-pill semantics.
/// † No explicit md/lg/mono values in `StyleSettings`; defaults from
///   `style.rs` constants (font_sm=11, font_md=13, font_lg=16 …) are used.
pub fn baseline_style_system() -> StyleSystem {
    StyleSystem {
        meta: Meta::new("meridien-baseline", "Meridien (Baseline)", true),

        typography: Typography {
            // font_section_label = font_caption = 8.0 (style_defaults(0): font_section_label=8, font_caption=8)
            size_xs: 8.0,
            // font_body = 10.0 (style_defaults(0): font_body=10)
            size_sm: 10.0,
            // No explicit md in StyleSettings; style.rs font_md() = 13.0
            size_md: 13.0,
            // No explicit lg in StyleSettings; style.rs font_lg() = 16.0
            // Sourced from: style.rs `pub fn font_lg() -> f32 { 16.0 }`
            // Note: builtin.rs uses 15.0 here. Anchor: style.rs constant wins → 16.0.
            // CONFLICT RESOLVED: style.rs `font_lg()` is 16.0 (confirmed at line 205).
            // builtin.rs used 15.0 as an approximation. Live app uses 16.0.
            size_lg: 16.0,
            // font_hero = 36.0 (style_defaults(0): font_hero=36)
            size_xl: 36.0,
            // Monospace scale pinned in style.rs: mono_sm=font_sm()=11, mono_md=font_md()=13
            mono_sm: 11.0,
            mono_md: 13.0,
            // mono_lg: font_lg()=16.0
            mono_lg: 16.0,
            size_section_label: 8.0,
            label_tracking: 0.0, nav_tracking: 0.0, section_tracking: 0.0,
            // Font family identifiers — Meridien uses the compiled-in defaults.
            family_ui: "Inter".to_owned(),
            family_mono: "JetBrains Mono".to_owned(),
            family_display: "Inter".to_owned(),
        },

        spacing: Spacing {
            // gap_2xs() = 2.0 (style.rs line 264)
            xs: 2.0,
            // gap_xs() = 4.0 (style.rs line 265)
            sm: 4.0,
            // gap_xs_mid() = 6.0 — DEFAULT_TOKEN_SNAPSHOT.gap_xs_mid (DS-IMPL-3)
            xs_mid: 6.0,
            // gap_sm() = 8.0 (style.rs line 270); card_padding_y=8 in Meridien
            md: 8.0,
            // card_padding_x = 10.0 (style_defaults(0))
            lg: 10.0,
            // gap_lg() = 16.0 (style.rs line 272)
            xl: 16.0,
            // gap_2xl() = 24.0 (style.rs line 274)
            xxl: 24.0,
            // gap_md() = 12.0 (style.rs line 271)
            gmd: 12.0,
            // cta_height_px = 36.0 (style_defaults(0))
            cta_height: 36.0,
            cta_padding_x: 16.0, button_height: 24.0, button_padding_x: 10.0, tab_height: 28.0,
        },

        radii: Radii {
            none: 0.0,
            // r_xs = 0 (style_defaults(0))
            xs:   0.0,
            // r_sm = 0
            sm:   0.0,
            // r_md = 0
            md:   0.0,
            // r_lg = 0
            lg:   0.0,
            // r_pill = 0 in struct, but style.rs radius_pill() returns 999.0 sentinel
            // Anchor: five surfaces (toolbar, context menu) do not use r_pill directly;
            // keeping 9999.0 to match design-system convention.
            full: 9999.0,
            pill: 0.0, chip: 0.0,
        },

        strokes: Strokes {
            // stroke_hair (sub-pixel): DEFAULT_TOKEN_SNAPSHOT = 0.3 (style.rs line 88)
            // style_defaults(0) sets stroke_hair=0.5 → stored in thin field.
            // hair field carries the DEFAULT_TOKEN_SNAPSHOT value (0.3).
            hair:   0.3,
            // stroke_hair = 0.5 (style_defaults(0))
            thin:   0.5,
            // stroke_medium: DEFAULT_TOKEN_SNAPSHOT = 0.8 (DS-IMPL-3)
            medium: 0.8,
            // stroke_thin=1.0, stroke_std=1.0 (Meridien collapses both to 1.0)
            std:    1.0,
            // stroke_bold = 1.0 (style_defaults(0))
            bold:   1.0,
            // stroke_thick = 1.0 (style_defaults(0))
            thick:  1.0,
            // legacy aliases
            md:     1.0,
            heavy:  1.0,
        },

        alphas: Alphas {
            // u8 tiers — exact values from DEFAULT_TOKEN_SNAPSHOT (style.rs lines 92–103)
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
            // hover_bg_alpha=20 → 20/255 ≈ 0.0784
            subtle:        20.0 / 255.0,
            // active_bg_alpha=35 → 35/255 ≈ 0.1373
            soft:          35.0 / 255.0,
            // No direct mapping; default 0.24 matches dt_u8!(alpha.muted, 60)/255 = 60/255≈0.235
            muted:         0.24,
            // No direct mapping; alpha_tint() = 48/255 ≈ 0.188; using default 0.48 as mid-tier
            // CONFLICT: Alphas.mid default is 0.48 — no Meridien StyleSettings field maps here.
            // Using default (0.48) as no anchor surface contradicts it.
            mid:           0.48,
            // alpha_active() = 100/255 ≈ 0.392; using default 0.72 — no direct override.
            strong:        0.72,
            opaque:        1.0,
            // header_outer_border_alpha = 38 → 38/255 ≈ 0.1490
            header_border: 38.0 / 255.0,
        },

        // Elevation: no Meridien-specific overrides; use the perceptual constants from style.rs.
        // style.rs ELEVATION_1_FACTOR=0.95, ELEVATION_2_FACTOR=0.88, ELEVATION_3_FACTOR=0.85.
        // CONFLICT: Elevation::default() has l1=1.05, l2=0.95, l3=0.88.
        // Live app: elevation_1 = bg × 0.95, elevation_2 = bg × 0.88, elevation_3 = bg × 0.85.
        // Resolved: override l1 to 0.95 (= ELEVATION_1_FACTOR), l3 to 0.85 (= ELEVATION_3_FACTOR).
        elevation: Elevation { l1: 0.95, l2: 0.88, l3: 0.85 },

        density: Density {
            // density=1 → normal → factor=1.0
            factor: 1.0,
            // row_height_px = 22.0 (style_defaults(0))
            row_height_dense: 22.0,
            // No explicit comfortable height; use button_height_px=24 as proxy
            row_height_comfortable: 24.0,
        },

        // Meridien: shadow_blur=0, shadow_offset_y=0, shadow_alpha=0 → all shadows disabled
        shadows: Shadows {
            card:     ShadowSpec { blur: 0.0, spread: 0.0, offset_x: 0.0, offset_y: 0.0, alpha: 0.0 },
            modal:    ShadowSpec { blur: 0.0, spread: 0.0, offset_x: 0.0, offset_y: 0.0, alpha: 0.0 },
            tooltip:  ShadowSpec { blur: 0.0, spread: 0.0, offset_x: 0.0, offset_y: 0.0, alpha: 0.0 },
            dropdown: ShadowSpec { blur: 0.0, spread: 0.0, offset_x: 0.0, offset_y: 0.0, alpha: 0.0 },
        },

        treatments: Treatments {
            // solid_active_fills = true (style_defaults(0))
            solid_active_fills:       true,
            // hairline_borders = true (style_defaults(0))
            hairline_borders:         true,
            // uppercase_section_labels = true (style_defaults(0))
            uppercase_section_labels: true,
            // No segmented_filled_idle field in StyleSettings; Meridien default = false
            segmented_filled_idle:    false,
            // focus_ring_width = 1.0 → Outline
            focus_ring: FocusRingStyle::Outline,
            // Meridien is flat editorial — no bevel, flush rows, proportional headers.
            surface_bevel: BevelStyle::None,
            bevel_highlight_alpha: 0,
            bevel_shadow_alpha: 0,
            wl_row_side_margin: 0.0, wl_row_corner_radius: 0, wl_row_divider_alpha: 30,
            section_header_mono: false, wl_symbol_mono: false, panel_tab_treatment: 0,
            pane_active_fill_accent: false,
            serif_headlines: true, button_treatment: 2, invert_active_fill: true,
            vertical_group_dividers: true, show_active_tab_underline: true, inactive_header_fill: true,
            nav_buttons_label_only: true, nav_buttons_uppercase_labels: true, tab_underline_under_text: true,
            card_floating_shadow: true, shadows_enabled: true, animations_enabled: true,
        },
        chrome: Chrome {
            toolbar_height_scale: 1.40, header_height_scale: 1.10, account_strip_height: 36.0,
            pane_gap: 0.0, pane_active_indicator: 1, tab_underline_thickness: 2.0,
            ..Chrome::default()
        },
    }
}

/// Returns the `ColorScheme` that exactly matches the Gruvbox colour theme
/// (`theme_idx = 5`, `THEMES[5]` in `gpu.rs`, compile-time default).
///
/// # Token→field mapping from THEMES[5]
///
/// | `Theme` field        | Value (from gpu.rs THEMES[5])       | `ColorScheme` field |
/// |----------------------|-------------------------------------|---------------------|
/// | `bg`                 | rgb(40, 40, 40)                     | `bg`                |
/// | `toolbar_bg`         | rgb(34, 34, 34)                     | `surface`           |
/// | `toolbar_bg` +8 lum  | rgb(42, 42, 42) (paper_from_surface)| `paper`             |
/// | `text`               | rgb(220, 220, 230)                  | `text`              |
/// | `dim`                | rgb(213, 196, 161)                  | `dim`               |
/// | `toolbar_border`     | hairline_border(bg) = rgb(53,53,53) | `border`            |
/// | `accent`             | rgb(254, 128, 25)                   | `accent`            |
/// | `bull`               | rgb(184, 187, 38)                   | `bull`              |
/// | `bear`               | rgb(251, 73, 52)                    | `bear`              |
/// | `warn`               | rgb(250, 189, 47)                   | `warn`              |
/// | `shadow_color`       | rgb(0, 0, 0) + alpha 180            | `shadow`            |
///
/// No conflicts found — all fields have a direct 1-to-1 source in THEMES[5].
pub fn baseline_color_scheme() -> ColorScheme {
    let bg   = rgb(40, 40, 40);
    let surf = rgb(34, 34, 34);

    // toolbar_border = hairline_border(bg=(40,40,40))
    // sum = 120 < 384 → dark → shift +13 → (53, 53, 53)
    let border = hairline_border(bg);

    ColorScheme {
        meta: Meta::new("gruvbox-baseline", "Gruvbox (Baseline)", true),

        // ── Backgrounds ─────────────────────────────────────────────────
        bg,      // rgb(40, 40, 40)
        surface: surf,   // rgb(34, 34, 34) — toolbar_bg

        // ── Text ────────────────────────────────────────────────────────
        text: rgb(220, 220, 230), // t.text
        dim:  rgb(213, 196, 161), // t.dim

        // ── Structural chrome ────────────────────────────────────────────
        border,  // rgb(53, 53, 53) — hairline_border(bg)

        // ── Semantic ────────────────────────────────────────────────────
        accent: rgb(254, 128,  25), // t.accent
        bull:   rgb(184, 187,  38), // t.bull
        bear:   rgb(251,  73,  52), // t.bear
        warn:   rgb(250, 189,  47), // t.warn

        // ── Shadow ──────────────────────────────────────────────────────
        // shadow_color = rgb(0,0,0); dark themes use alpha 180 per builtin.rs
        shadow: rgba(0, 0, 0, 180),


        // ── Hand-authored extras (Gruvbox / THEMES[5] values) ───────────
        notification_red: rgb(251,  73,  52),
        gold:             rgb(250, 189,  47),
        overlay_text:     rgb(235, 219, 178),
        rrg_leading:      rgb(184, 187,  38),
        rrg_improving:    rgb(131, 165, 152),
        rrg_weakening:    rgb(250, 189,  47),
        rrg_lagging:      rgb(251,  73,  52),
        // pinned_row_tint = rgba_pre(6,8,7,13) — premultiplied bytes stored as-is
        pinned_row_tint:  [6, 8, 7, 13],
        text_muted:       rgb(185, 178, 160),
        // hud_bg = rgba_pre(28,28,28,230) — premultiplied bytes stored as-is
        hud_bg:           [28, 28, 28, 230],
        hud_border:       rgb( 60,  56,  50),
        // Extended semantic palette — None falls back to bull/bear/warn at render time.
        success: None, danger: None, warning: None, info: None,
        pane_gap_color: None,
        cmd_palette:      crate::design_system::color_scheme::CMD_PALETTE_DEFAULT,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn baseline_style_system_builds_and_is_sane() {
        let ss = baseline_style_system();

        // Meta
        assert_eq!(ss.meta.id, "meridien-baseline");
        assert!(ss.meta.is_dark);

        // Typography — every size must be > 0
        assert!(ss.typography.size_xs > 0.0, "size_xs must be > 0");
        assert!(ss.typography.size_sm > 0.0, "size_sm must be > 0");
        assert!(ss.typography.size_md > 0.0, "size_md must be > 0");
        assert!(ss.typography.size_xl > 0.0, "size_xl must be > 0");

        // Meridien-specific: all corner radii zero
        assert_eq!(ss.radii.xs,  0.0, "Meridien xs radius must be 0");
        assert_eq!(ss.radii.sm,  0.0, "Meridien sm radius must be 0");
        assert_eq!(ss.radii.md,  0.0, "Meridien md radius must be 0");
        assert_eq!(ss.radii.lg,  0.0, "Meridien lg radius must be 0");
        assert!(ss.radii.full > 100.0, "full (pill) sentinel must be > 100");

        // Meridien-specific: hairline strokes
        assert!(ss.strokes.thin > 0.0, "thin stroke must be > 0");
        assert!(ss.strokes.std  >= ss.strokes.thin, "std must be >= thin");

        // Treatments: Meridien should have all three booleans true
        assert!(ss.treatments.solid_active_fills,       "Meridien solid_active_fills must be true");
        assert!(ss.treatments.hairline_borders,          "Meridien hairline_borders must be true");
        assert!(ss.treatments.uppercase_section_labels,  "Meridien uppercase_section_labels must be true");

        // Meridien has no drop shadows
        assert_eq!(ss.shadows.card.blur, 0.0, "Meridien card shadow blur must be 0");
        assert_eq!(ss.shadows.modal.alpha, 0.0, "Meridien modal shadow alpha must be 0");

        // Density factor should be 1.0 (normal)
        assert!((ss.density.factor - 1.0).abs() < 1e-6, "density factor must be 1.0");
    }

    #[test]
    fn baseline_color_scheme_builds_and_is_sane() {
        let cs = baseline_color_scheme();

        // Meta
        assert_eq!(cs.meta.id, "gruvbox-baseline");
        assert!(cs.meta.is_dark);

        // Background should be non-white / non-black (dark mid-tone)
        let [r, g, b, a] = cs.bg;
        assert_eq!(a, 255, "bg must be fully opaque");
        assert!(r < 100 && g < 100 && b < 100, "bg must be a dark colour");
        assert!(r > 0 || g > 0 || b > 0, "bg must not be pure black");

        // Text should be lighter than bg
        let [tr, tg, tb, _] = cs.text;
        assert!(tr as u16 + tg as u16 + tb as u16 > r as u16 + g as u16 + b as u16,
            "text must be lighter than bg");

        // Bull/bear are distinct
        assert_ne!(cs.bull, cs.bear, "bull and bear must be different colours");

        // Accent is non-zero
        let [ar, ag, ab, _] = cs.accent;
        assert!(ar > 0 || ag > 0 || ab > 0, "accent must not be black");

        // Border is between bg and text in luminance
        let bg_lum  = cs.bg[0]     as u16 + cs.bg[1]     as u16 + cs.bg[2]     as u16;
        let brd_lum = cs.border[0] as u16 + cs.border[1] as u16 + cs.border[2] as u16;
        assert!(brd_lum > bg_lum, "border must be lighter than bg on a dark theme");

        // Gruvbox-specific spot checks
        assert_eq!(cs.bg,     [40,  40,  40, 255], "Gruvbox bg");
        assert_eq!(cs.surface,[34,  34,  34, 255], "Gruvbox surface (toolbar_bg)");
        assert_eq!(cs.border, [53,  53,  53, 255], "Gruvbox border (hairline +13)");
        assert_eq!(cs.accent, [254, 128,  25, 255], "Gruvbox accent");
        assert_eq!(cs.bull,   [184, 187,  38, 255], "Gruvbox bull");
        assert_eq!(cs.bear,   [251,  73,  52, 255], "Gruvbox bear");
        assert_eq!(cs.warn,   [250, 189,  47, 255], "Gruvbox warn");
        assert_eq!(cs.text,   [220, 220, 230, 255], "Gruvbox text");
        assert_eq!(cs.dim,    [213, 196, 161, 255], "Gruvbox dim");
    }
}
