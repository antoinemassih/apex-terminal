//! Built-in `ColorScheme` catalogue derived from `gpu::THEMES`.
//!
//! Each entry in `THEMES` is transcribed to a `ColorScheme`. The base fields
//! map 1-to-1 from `Theme`; derived fields are recomputed using the same
//! helpers as `gpu.rs`; and the 11 hand-authored extra fields are transcribed
//! verbatim from the `THEMES` literals so that `color_scheme_to_theme` is
//! provably lossless.
//!
//! | `ColorScheme` field  | `Theme` source / derivation                        |
//! |----------------------|----------------------------------------------------|
//! | `bg`                 | `bg`                                               |
//! | `surface`            | `toolbar_bg`                                       |
//! | `paper`              | derived: toolbar_bg shifted ±8 lum                 |
//! | `text`               | `text`                                             |
//! | `dim`                | `dim`                                              |
//! | `border`             | `toolbar_border` (= `hairline_border(bg)`)         |
//! | `accent`             | `accent`                                           |
//! | `bull`               | `bull`                                             |
//! | `bear`               | `bear`                                             |
//! | `warn`               | `warn`                                             |
//! | `shadow`             | `shadow_color` RGB + alpha 180/120 (dark/light)    |
//! | `notification_red`   | `notification_red` (hand-authored)                 |
//! | `gold`               | `gold` (hand-authored)                             |
//! | `overlay_text`       | `overlay_text` (hand-authored)                     |
//! | `rrg_leading`        | `rrg_leading` (hand-authored)                      |
//! | `rrg_improving`      | `rrg_improving` (hand-authored)                    |
//! | `rrg_weakening`      | `rrg_weakening` (hand-authored)                    |
//! | `rrg_lagging`        | `rrg_lagging` (hand-authored)                      |
//! | `pinned_row_tint`    | `pinned_row_tint` (premultiplied, hand-authored)   |
//! | `text_muted`         | `text_muted` (hand-authored)                       |
//! | `hud_bg`             | `hud_bg` (premultiplied, hand-authored)            |
//! | `hud_border`         | `hud_border` (hand-authored)                       |

use super::{
    color_scheme::{ColorScheme, Meta, Rgba, CMD_PALETTE_DEFAULT},
    registry::ThemeRegistry,
    style_system::{
        Alphas, BevelStyle, Chrome, Density, Elevation, FocusRingStyle, GroupEnclosure, Radii, Shadows, ShadowSpec,
        Spacing, Strokes, StyleSystem, Treatments, Typography,
    },
};

// ── Internal helpers ──────────────────────────────────────────────────────────

/// Convert an `egui::Color32`-equivalent `[u8;4]` tuple to `Rgba`.
#[inline]
const fn c(r: u8, g: u8, b: u8, a: u8) -> Rgba { [r, g, b, a] }

/// Fully-opaque RGB to Rgba.
#[inline]
const fn rgb(r: u8, g: u8, b: u8) -> Rgba { [r, g, b, 255] }

/// Premultiplied RGBA — mirrors `gpu::rgba_pre(r, g, b, a)`.
///
/// `gpu.rs` stores `pinned_row_tint` and `hud_bg` as premultiplied Color32
/// values (via `rgba_pre`).  We store the same premultiplied bytes here so the
/// adapter can emit them verbatim without re-premultiplying.
#[inline]
const fn pre_rgba(r: u8, g: u8, b: u8, a: u8) -> Rgba { [r, g, b, a] }

/// Clamp an `i16` to `[0, 255]` and cast to `u8`.
const fn clamp_u8(v: i16) -> u8 {
    if v < 0 { 0 } else if v > 255 { 255 } else { v as u8 }
}

/// Kebab-case id from a theme name: lowercase, spaces → hyphens, strip accents.
fn to_id(name: &str) -> String {
    name.chars()
        .map(|ch| match ch {
            'A'..='Z' => (ch as u8 + 32) as char,
            ' ' | '_' => '-',
            // Strip non-ASCII (e.g. 'é' in "Rosé Pine")
            c if c.is_ascii() => c,
            _ => '-',
        })
        .collect::<String>()
        // Collapse consecutive hyphens that can arise from multi-byte chars
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

// ── hairline_border helper (mirrors gpu.rs logic, runtime edition) ────────────

/// Shift luminance by ~5% (13/255) toward white for dark bgs, toward black
/// for light bgs. Matches `gpu::hairline_border`.
fn hairline_border(bg: Rgba) -> Rgba {
    let is_dark = (bg[0] as u16 + bg[1] as u16 + bg[2] as u16) < 384;
    let shift: i16 = if is_dark { 13 } else { -13 };
    let clamp = |v: i16| -> u8 {
        if v < 0 { 0 } else if v > 255 { 255 } else { v as u8 }
    };
    [
        clamp(bg[0] as i16 + shift),
        clamp(bg[1] as i16 + shift),
        clamp(bg[2] as i16 + shift),
        255,
    ]
}

// ── Public API ─────────────────────────────────────────────────────────────────

/// Returns all built-in `ColorScheme`s derived from `gpu::THEMES`, one per
/// GPU theme entry, in the same order as `THEMES`.
///
/// This is a runtime builder (not `const`) because `ColorScheme` contains
/// `Vec` which is not `const`-constructible.  Call once at startup and store
/// the result.
pub fn builtin_color_schemes() -> Vec<ColorScheme> {
    // Border colours are derived the same way gpu.rs does via hairline_border.
    // Each entry is manually transcribed from the THEMES const array in gpu.rs.
    // Mapping: bg→bg, toolbar_bg→surface, text→text, dim→dim,
    //   toolbar_border→border (hairline_border(bg)),
    //   accent→accent, bull→bull, bear→bear, warn→warn,
    //   shadow_color→shadow (dark themes: alpha 180; light themes: alpha 120).

    vec![
        // ── [0] Midnight ─────────────────────────────────────────────────────
        {
            let bg = rgb(14, 16, 21);
            let surf = rgb(10, 12, 17);
            let border = hairline_border(bg);
            ColorScheme {
                meta: Meta { id: to_id("Midnight"), name: "Midnight".into(), is_dark: true },
                bg, surface: surf,
                text:   rgb(220, 220, 230),
                dim:    rgb(100, 105, 115),
                border,
                accent: rgb(62, 120, 180),
                bull:   rgb(62, 120, 180),
                bear:   rgb(180, 65, 58),
                warn:   rgb(255, 191, 0),
                shadow: c(0, 0, 0, 180),
                notification_red: rgb(231,  76,  60),
                gold:             rgb(255, 193,  37),
                overlay_text:     rgb(240, 240, 250),
                rrg_leading:      rgb( 56, 203, 137),
                rrg_improving:    rgb( 74, 158, 255),
                rrg_weakening:    rgb(230, 200,  50),
                rrg_lagging:      rgb(224,  82,  82),
                pinned_row_tint:  pre_rgba(3, 5, 9, 12),
                text_muted:       rgb(180, 180, 195),
                hud_bg:           pre_rgba(12, 12, 18, 230),
                hud_border:       rgb( 50,  52,  64),
            // Extended semantic palette — None falls back to bull/bear/warn at render time.
            success: None, danger: None, warning: None, info: None,
            pane_gap_color: None,
            cmd_palette:      CMD_PALETTE_DEFAULT,
            }
        },
        // ── [1] Nord ─────────────────────────────────────────────────────────
        {
            let bg = rgb(38, 44, 56);
            let surf = rgb(32, 38, 50);
            let border = hairline_border(bg);
            ColorScheme {
                meta: Meta { id: to_id("Nord"), name: "Nord".into(), is_dark: true },
                bg, surface: surf,
                text:   rgb(220, 220, 230),
                dim:    rgb(129, 161, 193),
                border,
                accent: rgb(136, 192, 208),
                bull:   rgb(163, 190, 140),
                bear:   rgb(191,  97, 106),
                warn:   rgb(235, 203, 139),
                shadow: c(0, 0, 0, 180),
                notification_red: rgb(191,  97, 106),
                gold:             rgb(235, 203, 139),
                overlay_text:     rgb(236, 239, 244),
                rrg_leading:      rgb(163, 190, 140),
                rrg_improving:    rgb(136, 192, 208),
                rrg_weakening:    rgb(235, 203, 139),
                rrg_lagging:      rgb(191,  97, 106),
                pinned_row_tint:  pre_rgba(5, 7, 9, 14),
                text_muted:       rgb(175, 180, 190),
                hud_bg:           pre_rgba(30, 34, 46, 230),
                hud_border:       rgb( 60,  66,  80),
            // Extended semantic palette — None falls back to bull/bear/warn at render time.
            success: None, danger: None, warning: None, info: None,
            pane_gap_color: None,
            cmd_palette:      CMD_PALETTE_DEFAULT,
            }
        },
        // ── [2] Monokai ───────────────────────────────────────────────────────
        {
            let bg = rgb(39, 40, 34);
            let surf = rgb(33, 34, 28);
            let border = hairline_border(bg);
            ColorScheme {
                meta: Meta { id: to_id("Monokai"), name: "Monokai".into(), is_dark: true },
                bg, surface: surf,
                text:   rgb(220, 220, 230),
                dim:    rgb(165, 159, 133),
                border,
                accent: rgb(230, 219, 116),
                bull:   rgb(166, 226,  46),
                bear:   rgb(249,  38, 114),
                warn:   rgb(230, 219, 116),
                shadow: c(0, 0, 0, 180),
                notification_red: rgb(249,  38, 114),
                gold:             rgb(255, 193,  37),
                overlay_text:     rgb(248, 248, 240),
                rrg_leading:      rgb(166, 226,  46),
                rrg_improving:    rgb(102, 217, 239),
                rrg_weakening:    rgb(230, 219, 116),
                rrg_lagging:      rgb(249,  38, 114),
                pinned_row_tint:  pre_rgba(4, 10, 11, 12),
                text_muted:       rgb(180, 178, 160),
                hud_bg:           pre_rgba(30, 30, 24, 230),
                hud_border:       rgb( 55,  54,  44),
            // Extended semantic palette — None falls back to bull/bear/warn at render time.
            success: None, danger: None, warning: None, info: None,
            pane_gap_color: None,
            cmd_palette:      CMD_PALETTE_DEFAULT,
            }
        },
        // ── [3] Solarized ─────────────────────────────────────────────────────
        {
            let bg = rgb(0, 43, 54);
            let surf = rgb(0, 37, 48);
            let border = hairline_border(bg);
            ColorScheme {
                meta: Meta { id: to_id("Solarized"), name: "Solarized".into(), is_dark: true },
                bg, surface: surf,
                text:   rgb(220, 220, 230),
                dim:    rgb(131, 148, 150),
                border,
                accent: rgb( 42, 161, 152),
                bull:   rgb(133, 153,   0),
                bear:   rgb(220,  50,  47),
                warn:   rgb(181, 137,   0),
                shadow: c(0, 0, 0, 180),
                notification_red: rgb(220,  50,  47),
                gold:             rgb(181, 137,   0),
                overlay_text:     rgb(253, 246, 227),
                rrg_leading:      rgb(133, 153,   0),
                rrg_improving:    rgb( 38, 139, 210),
                rrg_weakening:    rgb(181, 137,   0),
                rrg_lagging:      rgb(220,  50,  47),
                pinned_row_tint:  pre_rgba(1, 6, 9, 12),
                text_muted:       rgb(156, 172, 175),
                hud_bg:           pre_rgba(0, 28, 36, 230),
                hud_border:       rgb(  7,  54,  66),
            // Extended semantic palette — None falls back to bull/bear/warn at render time.
            success: None, danger: None, warning: None, info: None,
            pane_gap_color: None,
            cmd_palette:      CMD_PALETTE_DEFAULT,
            }
        },
        // ── [4] Dracula ───────────────────────────────────────────────────────
        {
            let bg = rgb(40, 42, 54);
            let surf = rgb(34, 36, 48);
            let border = hairline_border(bg);
            ColorScheme {
                meta: Meta { id: to_id("Dracula"), name: "Dracula".into(), is_dark: true },
                bg, surface: surf,
                text:   rgb(220, 220, 230),
                dim:    rgb(189, 147, 249),
                border,
                accent: rgb(255, 121, 198),
                bull:   rgb( 80, 250, 123),
                bear:   rgb(255,  85,  85),
                warn:   rgb(241, 250, 140),
                shadow: c(0, 0, 0, 180),
                notification_red: rgb(255,  85,  85),
                gold:             rgb(241, 250, 140),
                overlay_text:     rgb(248, 248, 242),
                rrg_leading:      rgb( 80, 250, 123),
                rrg_improving:    rgb(139, 233, 253),
                rrg_weakening:    rgb(241, 250, 140),
                rrg_lagging:      rgb(255,  85,  85),
                pinned_row_tint:  pre_rgba(6, 10, 11, 12),
                text_muted:       rgb(190, 185, 215),
                hud_bg:           pre_rgba(30, 32, 44, 230),
                hud_border:       rgb( 55,  58,  75),
            // Extended semantic palette — None falls back to bull/bear/warn at render time.
            success: None, danger: None, warning: None, info: None,
            pane_gap_color: None,
            cmd_palette:      CMD_PALETTE_DEFAULT,
            }
        },
        // ── [5] Gruvbox ───────────────────────────────────────────────────────
        {
            let bg = rgb(40, 40, 40);
            let surf = rgb(34, 34, 34);
            let border = hairline_border(bg);
            ColorScheme {
                meta: Meta { id: to_id("Gruvbox"), name: "Gruvbox".into(), is_dark: true },
                bg, surface: surf,
                text:   rgb(220, 220, 230),
                dim:    rgb(213, 196, 161),
                border,
                accent: rgb(254, 128,  25),
                bull:   rgb(184, 187,  38),
                bear:   rgb(251,  73,  52),
                warn:   rgb(250, 189,  47),
                shadow: c(0, 0, 0, 180),
                notification_red: rgb(251,  73,  52),
                gold:             rgb(250, 189,  47),
                overlay_text:     rgb(235, 219, 178),
                rrg_leading:      rgb(184, 187,  38),
                rrg_improving:    rgb(131, 165, 152),
                rrg_weakening:    rgb(250, 189,  47),
                rrg_lagging:      rgb(251,  73,  52),
                pinned_row_tint:  pre_rgba(6, 8, 7, 13),
                text_muted:       rgb(185, 178, 160),
                hud_bg:           pre_rgba(28, 28, 28, 230),
                hud_border:       rgb( 60,  56,  50),
            // Extended semantic palette — None falls back to bull/bear/warn at render time.
            success: None, danger: None, warning: None, info: None,
            pane_gap_color: None,
            cmd_palette:      CMD_PALETTE_DEFAULT,
            }
        },
        // ── [6] Catppuccin ────────────────────────────────────────────────────
        {
            let bg = rgb(30, 30, 46);
            let surf = rgb(24, 24, 38);
            let border = hairline_border(bg);
            ColorScheme {
                meta: Meta { id: to_id("Catppuccin"), name: "Catppuccin".into(), is_dark: true },
                bg, surface: surf,
                text:   rgb(220, 220, 230),
                dim:    rgb(180, 190, 254),
                border,
                accent: rgb(203, 166, 247),
                bull:   rgb(166, 227, 161),
                bear:   rgb(243, 139, 168),
                warn:   rgb(249, 226, 175),
                shadow: c(0, 0, 0, 180),
                notification_red: rgb(243, 139, 168),
                gold:             rgb(249, 226, 175),
                overlay_text:     rgb(205, 214, 244),
                rrg_leading:      rgb(166, 227, 161),
                rrg_improving:    rgb(137, 220, 235),
                rrg_weakening:    rgb(249, 226, 175),
                rrg_lagging:      rgb(243, 139, 168),
                pinned_row_tint:  pre_rgba(6, 8, 11, 12),
                text_muted:       rgb(182, 186, 220),
                hud_bg:           pre_rgba(20, 20, 36, 230),
                hud_border:       rgb( 49,  50,  68),
            // Extended semantic palette — None falls back to bull/bear/warn at render time.
            success: None, danger: None, warning: None, info: None,
            pane_gap_color: None,
            cmd_palette:      CMD_PALETTE_DEFAULT,
            }
        },
        // ── [7] Tokyo Night ───────────────────────────────────────────────────
        {
            let bg = rgb(26, 27, 38);
            let surf = rgb(21, 22, 32);
            let border = hairline_border(bg);
            ColorScheme {
                meta: Meta { id: to_id("Tokyo Night"), name: "Tokyo Night".into(), is_dark: true },
                bg, surface: surf,
                text:   rgb(220, 220, 230),
                dim:    rgb(122, 162, 247),
                border,
                accent: rgb(125, 207, 255),
                bull:   rgb(158, 206, 106),
                bear:   rgb(247, 118, 142),
                warn:   rgb(224, 175, 104),
                shadow: c(0, 0, 0, 180),
                notification_red: rgb(247, 118, 142),
                gold:             rgb(224, 175, 104),
                overlay_text:     rgb(192, 202, 245),
                rrg_leading:      rgb(158, 206, 106),
                rrg_improving:    rgb(125, 207, 255),
                rrg_weakening:    rgb(224, 175, 104),
                rrg_lagging:      rgb(247, 118, 142),
                pinned_row_tint:  pre_rgba(5, 9, 12, 12),
                text_muted:       rgb(172, 178, 220),
                hud_bg:           pre_rgba(18, 18, 28, 230),
                hud_border:       rgb( 40,  44,  62),
            // Extended semantic palette — None falls back to bull/bear/warn at render time.
            success: None, danger: None, warning: None, info: None,
            pane_gap_color: None,
            cmd_palette:      CMD_PALETTE_DEFAULT,
            }
        },
        // ── [8] Kanagawa ─────────────────────────────────────────────────────
        {
            let bg = rgb(22, 22, 29);
            let surf = rgb(18, 18, 24);
            let border = hairline_border(bg);
            ColorScheme {
                meta: Meta { id: to_id("Kanagawa"), name: "Kanagawa".into(), is_dark: true },
                bg, surface: surf,
                text:   rgb(220, 220, 230),
                dim:    rgb( 84,  88, 104),
                border,
                accent: rgb(127, 180, 202),
                bull:   rgb(118, 169, 130),
                bear:   rgb(195,  64,  67),
                warn:   rgb(228, 175,  69),
                shadow: c(0, 0, 0, 180),
                notification_red: rgb(195,  64,  67),
                gold:             rgb(228, 175,  69),
                overlay_text:     rgb(220, 215, 186),
                rrg_leading:      rgb(118, 169, 130),
                rrg_improving:    rgb(127, 180, 202),
                rrg_weakening:    rgb(228, 175,  69),
                rrg_lagging:      rgb(195,  64,  67),
                pinned_row_tint:  pre_rgba(5, 8, 9, 12),
                text_muted:       rgb(155, 158, 175),
                hud_bg:           pre_rgba(14, 14, 20, 230),
                hud_border:       rgb( 36,  36,  50),
            // Extended semantic palette — None falls back to bull/bear/warn at render time.
            success: None, danger: None, warning: None, info: None,
            pane_gap_color: None,
            cmd_palette:      CMD_PALETTE_DEFAULT,
            }
        },
        // ── [9] Everforest ────────────────────────────────────────────────────
        {
            let bg = rgb(39, 46, 38);
            let surf = rgb(33, 40, 32);
            let border = hairline_border(bg);
            ColorScheme {
                meta: Meta { id: to_id("Everforest"), name: "Everforest".into(), is_dark: true },
                bg, surface: surf,
                text:   rgb(220, 220, 230),
                dim:    rgb(157, 169, 140),
                border,
                accent: rgb(131, 165, 152),
                bull:   rgb(167, 192, 128),
                bear:   rgb(230, 126, 128),
                warn:   rgb(223, 199, 118),
                shadow: c(0, 0, 0, 180),
                notification_red: rgb(230, 126, 128),
                gold:             rgb(223, 199, 118),
                overlay_text:     rgb(211, 198, 170),
                rrg_leading:      rgb(167, 192, 128),
                rrg_improving:    rgb(131, 165, 152),
                rrg_weakening:    rgb(223, 199, 118),
                rrg_lagging:      rgb(230, 126, 128),
                pinned_row_tint:  pre_rgba(6, 8, 7, 13),
                text_muted:       rgb(175, 178, 162),
                hud_bg:           pre_rgba(28, 34, 28, 230),
                hud_border:       rgb( 52,  60,  50),
            // Extended semantic palette — None falls back to bull/bear/warn at render time.
            success: None, danger: None, warning: None, info: None,
            pane_gap_color: None,
            cmd_palette:      CMD_PALETTE_DEFAULT,
            }
        },
        // ── [10] Vesper ───────────────────────────────────────────────────────
        {
            let bg = rgb(16, 16, 16);
            let surf = rgb(11, 11, 11);
            let border = hairline_border(bg);
            ColorScheme {
                meta: Meta { id: to_id("Vesper"), name: "Vesper".into(), is_dark: true },
                bg, surface: surf,
                text:   rgb(220, 220, 230),
                dim:    rgb(120, 120, 120),
                border,
                accent: rgb(255, 199, 119),
                bull:   rgb(166, 218, 149),
                bear:   rgb(238, 130,  98),
                warn:   rgb(255, 199, 119),
                shadow: c(0, 0, 0, 180),
                notification_red: rgb(238, 130,  98),
                gold:             rgb(255, 193,  37),
                overlay_text:     rgb(230, 230, 230),
                rrg_leading:      rgb(166, 218, 149),
                rrg_improving:    rgb( 74, 158, 255),
                rrg_weakening:    rgb(255, 199, 119),
                rrg_lagging:      rgb(238, 130,  98),
                pinned_row_tint:  pre_rgba(3, 6, 11, 11),
                text_muted:       rgb(170, 170, 180),
                hud_bg:           pre_rgba(10, 10, 10, 230),
                hud_border:       rgb( 42,  42,  42),
            // Extended semantic palette — None falls back to bull/bear/warn at render time.
            success: None, danger: None, warning: None, info: None,
            pane_gap_color: None,
            cmd_palette:      CMD_PALETTE_DEFAULT,
            }
        },
        // ── [11] Rosé Pine ────────────────────────────────────────────────────
        {
            let bg = rgb(25, 23, 36);
            let surf = rgb(20, 18, 30);
            let border = hairline_border(bg);
            ColorScheme {
                // "Rosé Pine" — strip non-ASCII 'é' → "rose-pine"
                meta: Meta { id: "rose-pine".into(), name: "Rosé Pine".into(), is_dark: true },
                bg, surface: surf,
                text:   rgb(220, 220, 230),
                dim:    rgb(110, 106, 134),
                border,
                accent: rgb(196, 167, 231),
                bull:   rgb(156, 207, 216),
                bear:   rgb(235, 111, 146),
                warn:   rgb(246, 193, 119),
                shadow: c(0, 0, 0, 180),
                notification_red: rgb(235, 111, 146),
                gold:             rgb(246, 193, 119),
                overlay_text:     rgb(224, 222, 244),
                rrg_leading:      rgb(156, 207, 216),
                rrg_improving:    rgb(196, 167, 231),
                rrg_weakening:    rgb(246, 193, 119),
                rrg_lagging:      rgb(235, 111, 146),
                pinned_row_tint:  pre_rgba(7, 9, 10, 12),
                text_muted:       rgb(167, 162, 187),
                hud_bg:           pre_rgba(18, 16, 28, 230),
                hud_border:       rgb( 44,  40,  58),
            // Extended semantic palette — None falls back to bull/bear/warn at render time.
            success: None, danger: None, warning: None, info: None,
            pane_gap_color: None,
            cmd_palette:      CMD_PALETTE_DEFAULT,
            }
        },
        // ── [12] Bauhaus (light) ──────────────────────────────────────────────
        {
            let bg = rgb(242, 242, 238);
            let surf = rgb(248, 248, 245);
            let border = hairline_border(bg);
            ColorScheme {
                meta: Meta { id: to_id("Bauhaus"), name: "Bauhaus".into(), is_dark: false },
                bg, surface: surf,
                text:   rgb( 22,  22,  24),
                dim:    rgb(120, 125, 130),
                border,
                accent: rgb(232,  93,  38),
                bull:   rgb( 20, 120,  60),
                bear:   rgb(200,  55,  45),
                warn:   rgb(204, 120,   0),
                shadow: c(40, 40, 40, 120),
                notification_red: rgb(200,  55,  45),
                gold:             rgb(204, 153,   0),
                overlay_text:     rgb( 20,  20,  22),
                rrg_leading:      rgb( 20, 120,  60),
                rrg_improving:    rgb( 30, 100, 180),
                rrg_weakening:    rgb(180, 140,   0),
                rrg_lagging:      rgb(200,  55,  45),
                pinned_row_tint:  pre_rgba(1, 5, 9, 14),
                text_muted:       rgb(100, 102, 110),
                hud_bg:           pre_rgba(20, 20, 20, 220),
                hud_border:       rgb( 80,  82,  88),
            // Extended semantic palette — None falls back to bull/bear/warn at render time.
            success: None, danger: None, warning: None, info: None,
            pane_gap_color: None,
            cmd_palette:      CMD_PALETTE_DEFAULT,
            }
        },
        // ── [13] Peach (light) ────────────────────────────────────────────────
        {
            let bg = rgb(243, 241, 238);
            let surf = rgb(250, 248, 246);
            let border = hairline_border(bg);
            ColorScheme {
                meta: Meta { id: to_id("Peach"), name: "Peach".into(), is_dark: false },
                bg, surface: surf,
                text:   rgb( 20,  20,  22),
                dim:    rgb(115, 120, 125),
                border,
                accent: rgb(210,  95,  70),
                bull:   rgb( 22, 130,  70),
                bear:   rgb(195,  50,  55),
                warn:   rgb(200, 130,   0),
                shadow: c(40, 40, 40, 120),
                notification_red: rgb(195,  50,  55),
                gold:             rgb(200, 150,   0),
                overlay_text:     rgb( 20,  20,  22),
                rrg_leading:      rgb( 22, 130,  70),
                rrg_improving:    rgb( 30, 100, 180),
                rrg_weakening:    rgb(180, 140,   0),
                rrg_lagging:      rgb(195,  50,  55),
                pinned_row_tint:  pre_rgba(1, 5, 9, 14),
                text_muted:       rgb( 98, 100, 108),
                hud_bg:           pre_rgba(20, 20, 20, 220),
                hud_border:       rgb( 82,  80,  78),
            // Extended semantic palette — None falls back to bull/bear/warn at render time.
            success: None, danger: None, warning: None, info: None,
            pane_gap_color: None,
            cmd_palette:      CMD_PALETTE_DEFAULT,
            }
        },
        // ── [14] Ivory (light) ────────────────────────────────────────────────
        {
            let bg = rgb(240, 242, 238);
            let surf = rgb(248, 250, 246);
            let border = hairline_border(bg);
            ColorScheme {
                meta: Meta { id: to_id("Ivory"), name: "Ivory".into(), is_dark: false },
                bg, surface: surf,
                text:   rgb( 18,  20,  22),
                dim:    rgb(118, 122, 128),
                border,
                accent: rgb(160, 190,  40),
                bull:   rgb( 80, 160,  50),
                bear:   rgb(210,  60,  50),
                warn:   rgb(190, 140,   0),
                shadow: c(40, 40, 40, 120),
                notification_red: rgb(210,  60,  50),
                gold:             rgb(190, 150,   0),
                overlay_text:     rgb( 18,  20,  22),
                rrg_leading:      rgb( 80, 160,  50),
                rrg_improving:    rgb( 30, 100, 180),
                rrg_weakening:    rgb(180, 140,   0),
                rrg_lagging:      rgb(210,  60,  50),
                pinned_row_tint:  pre_rgba(1, 5, 9, 14),
                text_muted:       rgb(100, 102, 108),
                hud_bg:           pre_rgba(18, 20, 18, 220),
                hud_border:       rgb( 80,  82,  80),
            // Extended semantic palette — None falls back to bull/bear/warn at render time.
            success: None, danger: None, warning: None, info: None,
            pane_gap_color: None,
            cmd_palette:      CMD_PALETTE_DEFAULT,
            }
        },
        // ── [15] Newsprint (light) ────────────────────────────────────────────
        {
            let bg = rgb(238, 232, 220);
            let surf = rgb(238, 232, 220); // toolbar_bg same as bg for Newsprint
            let border = hairline_border(bg);
            ColorScheme {
                meta: Meta { id: to_id("Newsprint"), name: "Newsprint".into(), is_dark: false },
                bg, surface: surf,
                text:   rgb( 28,  28,  28),
                dim:    rgb(120, 116, 104),
                border,
                accent: rgb( 34,  94,  56),
                bull:   rgb( 34,  94,  56),
                bear:   rgb(168,  52,  52),
                warn:   rgb(168, 120,   0),
                shadow: c(60, 50, 40, 120),
                notification_red: rgb(168,  52,  52),
                gold:             rgb(168, 130,   0),
                overlay_text:     rgb( 28,  28,  28),
                rrg_leading:      rgb( 34,  94,  56),
                rrg_improving:    rgb( 30,  90, 160),
                rrg_weakening:    rgb(160, 120,   0),
                rrg_lagging:      rgb(168,  52,  52),
                pinned_row_tint:  pre_rgba(1, 4, 8, 13),
                text_muted:       rgb(105, 100,  90),
                hud_bg:           pre_rgba(28, 24, 18, 220),
                hud_border:       rgb( 90,  82,  68),
            // Extended semantic palette — None falls back to bull/bear/warn at render time.
            success: None, danger: None, warning: None, info: None,
            pane_gap_color: None,
            cmd_palette:      CMD_PALETTE_DEFAULT,
            }
        },

        // ══ React ApexTerminalThemes palette ports ════════════════════════════
        // These 6 color schemes are transcribed directly from the React mockup's
        // `global.css` `[data-ds="<theme>"]` token blocks — the styling source of
        // truth tuned to ~90% source fidelity.  Pair with the matching style preset
        // for the full intended look (e.g. scheme "Aperture" + style "Aperture").

        // ── [16] Aperture — warm-tinted black canvas, orange accent ───────────
        {
            let bg   = rgb(  0,   0,   0); // pure black canvas
            let surf = rgb( 20,  19,  17); // #141311 bg-panel
            ColorScheme {
                meta: Meta { id: "aperture".into(), name: "Aperture".into(), is_dark: true },
                bg, surface: surf,
                text:   rgb(244, 236, 224), // #f4ece0 warm cream
                dim:    rgb(182, 173, 157), // #b6ad9d
                border: c(255, 255, 255, 31), // rgba(255,255,255,0.12)
                accent: rgb(239,  91,  59), // #ef5b3b orange
                bull:   rgb( 78, 192, 122), // #4ec07a
                bear:   rgb(216,  80,  62), // #d8503e
                warn:   rgb(255, 138,  76), // #ff8a4c (accent-sub)
                shadow: c(0, 0, 0, 180),
                notification_red: rgb(216, 80, 62),
                gold:             rgb(239, 91, 59),
                overlay_text:     rgb(244, 236, 224),
                rrg_leading:      rgb( 78, 192, 122),
                rrg_improving:    rgb(239, 150,  90),
                rrg_weakening:    rgb(255, 200,  80),
                rrg_lagging:      rgb(216,  80,  62),
                pinned_row_tint:  pre_rgba(239, 91, 59, 14),
                text_muted:       rgb(118, 112,  95), // #76705f
                hud_bg:           pre_rgba( 20, 19, 17, 230),
                hud_border:       rgb( 50,  47,  40),
                // Extended semantic palette — None falls back to bull/bear/warn at render time.
            success: None, danger: None, warning: None, info: None,
            pane_gap_color: None,
            cmd_palette:      CMD_PALETTE_DEFAULT,
            }
        },

        // ── [17] Cadence — Spotify-style pure black, vivid green accent ───────
        {
            let bg   = rgb(  0,   0,   0); // pure black
            let surf = rgb( 18,  18,  18); // #121212
            ColorScheme {
                meta: Meta { id: "cadence".into(), name: "Cadence".into(), is_dark: true },
                bg, surface: surf,
                text:   rgb(255, 255, 255),
                dim:    rgb(179, 179, 179), // #b3b3b3 text-secondary
                border: c(255, 255, 255, 18), // rgba(255,255,255,0.07)
                accent: rgb( 30, 215,  96), // #1ed760 Spotify green
                bull:   rgb( 30, 215,  96), // same as accent (pos)
                bear:   rgb(241,  94, 108), // #f15e6c
                warn:   rgb( 22, 156,  70), // #169c46 accent-sub
                shadow: c(0, 0, 0, 200),
                notification_red: rgb(241, 94, 108),
                gold:             rgb(255, 200,  50),
                overlay_text:     rgb(255, 255, 255),
                rrg_leading:      rgb( 30, 215,  96),
                rrg_improving:    rgb( 80, 200, 160),
                rrg_weakening:    rgb(200, 200,  50),
                rrg_lagging:      rgb(241,  94, 108),
                pinned_row_tint:  pre_rgba(30, 215, 96, 12),
                text_muted:       rgb(122, 122, 122), // #7a7a7a
                hud_bg:           pre_rgba(18, 18, 18, 230),
                hud_border:       c(255, 255, 255, 25),
                // Extended semantic palette — None falls back to bull/bear/warn at render time.
            success: None, danger: None, warning: None, info: None,
            pane_gap_color: None,
            cmd_palette:      CMD_PALETTE_DEFAULT,
            }
        },

        // ── [18] Alto — Zed-inspired warm dark, amber accent ─────────────────
        {
            let bg   = rgb( 21,  18,  14); // #15120e
            let surf = rgb( 28,  24,  20); // #1c1814 bg-panel
            ColorScheme {
                meta: Meta { id: "alto".into(), name: "Alto".into(), is_dark: true },
                bg, surface: surf,
                text:   rgb(239, 231, 216), // #efe7d8
                dim:    rgb(156, 147, 133), // #9c9385
                border: rgb( 61,  52,  43), // #3d342b
                accent: rgb(217, 152,  88), // #d99858 amber
                bull:   rgb(111, 191, 115), // #6fbf73
                bear:   rgb(226,  93,  93), // #e25d5d
                warn:   rgb(184, 120,  56), // #b87838 accent-sub
                shadow: c(0, 0, 0, 160),
                notification_red: rgb(226, 93, 93),
                gold:             rgb(217, 152, 88),
                overlay_text:     rgb(239, 231, 216),
                rrg_leading:      rgb(111, 191, 115),
                rrg_improving:    rgb(217, 180, 100),
                rrg_weakening:    rgb(200, 170,  60),
                rrg_lagging:      rgb(226,  93,  93),
                pinned_row_tint:  pre_rgba(217, 152, 88, 14),
                text_muted:       rgb(107,  99,  88), // #6b6358
                hud_bg:           pre_rgba(21, 18, 14, 230),
                hud_border:       rgb( 61,  52,  43),
                // Extended semantic palette — None falls back to bull/bear/warn at render time.
            success: None, danger: None, warning: None, info: None,
            pane_gap_color: None,
            cmd_palette:      CMD_PALETTE_DEFAULT,
            }
        },

        // ── [19] Mariner — Alto warm-dark, steel-blue accent ─────────────────
        {
            let bg   = rgb( 21,  18,  14);
            let surf = rgb( 28,  24,  20);
            ColorScheme {
                meta: Meta { id: "mariner".into(), name: "Mariner".into(), is_dark: true },
                bg, surface: surf,
                text:   rgb(239, 231, 216),
                dim:    rgb(156, 147, 133),
                border: rgb( 61,  52,  43),
                accent: rgb(110, 160, 200), // #6ea0c8 steel blue
                bull:   rgb(111, 191, 115),
                bear:   rgb(226,  93,  93),
                warn:   rgb( 79, 126, 160), // #4f7ea0 accent-sub
                shadow: c(0, 0, 0, 160),
                notification_red: rgb(226, 93, 93),
                gold:             rgb(200, 170, 80),
                overlay_text:     rgb(239, 231, 216),
                rrg_leading:      rgb(111, 191, 115),
                rrg_improving:    rgb(110, 160, 200),
                rrg_weakening:    rgb(200, 180,  60),
                rrg_lagging:      rgb(226,  93,  93),
                pinned_row_tint:  pre_rgba(110, 160, 200, 14),
                text_muted:       rgb(107,  99,  88),
                hud_bg:           pre_rgba(21, 18, 14, 230),
                hud_border:       rgb( 61,  52,  43),
                // Extended semantic palette — None falls back to bull/bear/warn at render time.
            success: None, danger: None, warning: None, info: None,
            pane_gap_color: None,
            cmd_palette:      CMD_PALETTE_DEFAULT,
            }
        },

        // ── [20] Lucid — editorial light, cream paper, terracotta accent ──────
        {
            let bg   = rgb(241, 237, 228); // #f1ede4 cream paper
            let surf = rgb(247, 243, 234); // #f7f3ea bg-panel (lighter)
            ColorScheme {
                meta: Meta { id: "lucid".into(), name: "Lucid".into(), is_dark: false },
                bg, surface: surf,
                text:   rgb( 20,  20,  15), // #14140f near-black ink
                dim:    rgb(138, 134, 117), // #8a8675
                border: rgb(191, 181, 154), // #bfb59a
                accent: rgb(214,  85,  43), // #d6552b terracotta
                bull:   rgb( 31, 111,  59), // #1f6f3b deep green
                bear:   rgb(196,  58,  31), // #c43a1f venetian red
                warn:   rgb(196,  58,  31), // same as bear (editorial)
                shadow: c(120, 110, 90, 100),
                notification_red: rgb(196, 58, 31),
                gold:             rgb(170, 120,  20),
                overlay_text:     rgb( 20,  20,  15),
                rrg_leading:      rgb( 31, 111,  59),
                rrg_improving:    rgb(100, 160,  80),
                rrg_weakening:    rgb(180, 130,  20),
                rrg_lagging:      rgb(196,  58,  31),
                pinned_row_tint:  pre_rgba(214, 85, 43, 12),
                text_muted:       rgb(179, 173, 156), // #b3ad9c
                hud_bg:           pre_rgba(241, 237, 228, 230),
                hud_border:       rgb(191, 181, 154),
                // Extended semantic palette — None falls back to bull/bear/warn at render time.
            success: None, danger: None, warning: None, info: None,
            pane_gap_color: None,
            cmd_palette:      CMD_PALETTE_DEFAULT,
            }
        },
    ]
}

/// Returns the 3 built-in `StyleSystem`s transcribed from `style_defaults(0/1/2)`.
///
/// # Mapping  (`StyleSettings` → `StyleSystem` sub-struct)
///
/// | StyleSettings field(s)                            | StyleSystem target          |
/// |---------------------------------------------------|-----------------------------|
/// | `r_xs / r_sm / r_md / r_lg / r_pill`             | `Radii.xs/sm/md/lg/full`    |
/// | `stroke_hair / stroke_thin / stroke_std / stroke_bold / stroke_thick` | `Strokes.thin/std/md/heavy` |
/// | `font_section_label / font_body / font_caption / font_hero` | `Typography` (section_label→xs, caption→xs, body→sm, hero→xl; md/lg/mono defaulted) |
/// | `card_padding_y / card_padding_x / row_height_px / button_height_px / button_padding_x / pane_gap` | `Spacing` |
/// | `hover_bg_alpha / active_bg_alpha`                | `Alphas.subtle/soft/muted/mid/strong` (hover→subtle, active→soft; others defaulted) |
/// | `header_outer_border_alpha`                       | `Alphas.header_border`      |
/// | `shadow_blur / shadow_offset_y / shadow_alpha`    | `Shadows.card` (modal/tooltip/dropdown use scaled defaults) |
/// | `density`                                         | `Density.factor` (0=compact→0.8, 1=normal→1.0, 2=roomy→1.2) |
/// | `solid_active_fills / hairline_borders / uppercase_section_labels` | `Treatments` booleans |
/// | `focus_ring_width` ≤ 1.0 → `FocusRingStyle::Outline`; > 1.5 → `Glow`           | `Treatments.focus_ring` |
///
/// Fields with no clean source (`Elevation`, `Spacing.gmd`, mono sizes) use
/// `Default::default()`.  Fields exclusive to `StyleSettings` that are pure
/// color-layer concerns (`active_fill_color`, `toolbar_bg`, …) are dropped.
pub fn builtin_style_systems() -> Vec<StyleSystem> {
    // ── Meridien (id=0, the `_ =>` arm in style_defaults) ────────────────────
    // Sharp corners, hairline borders, solid active fills, uppercase labels,
    // no drop shadows, standard density.
    let meridien = StyleSystem {
        meta: Meta::new("meridien", "Meridien", true),
        typography: Typography {
            size_xs: 8.0,   // font_section_label / font_caption
            size_sm: 10.0,  // font_body
            size_md: 13.0,  // default — no explicit md in StyleSettings
            size_lg: 15.0,  // default
            size_xl: 36.0,  // font_hero
            mono_sm: 11.0,
            mono_md: 13.0,
            mono_lg: 15.0,
            size_section_label: 8.0, // font_section_label = 8
            label_tracking: 0.0, nav_tracking: 0.0, section_tracking: 0.0,
            // Font family identifiers — Meridien uses the compiled-in defaults.
            family_ui: "Inter".to_owned(),
            family_mono: "JetBrains Mono".to_owned(),
            family_display: "Inter".to_owned(),
        },
        spacing: Spacing {
            xs: 2.0,
            sm: 4.0,
            xs_mid: 6.0,      // gap_xs_mid default
            md: 8.0,          // card_padding_y / 1 (approx)
            lg: 10.0,         // card_padding_x
            xl: 16.0,
            xxl: 24.0,
            gmd: 8.0,
            cta_height: 36.0, // cta_height_px
            cta_padding_x: 16.0, button_height: 24.0, button_padding_x: 10.0, tab_height: 28.0,
        },
        // radii + strokes aligned to the LIVE default style (style_defaults(0)).
        // The Phase B source-swap deliberately defined Meridien-the-default as
        // the graduated dt_f32! scale to preserve the existing look — so the
        // design_system Meridien matches it (equivalence test: field-exact).
        radii: Radii {
            none: 0.0,
            xs: 2.0,
            sm: 4.0,
            md: 6.0,
            lg: 12.0,
            full: 9999.0, // pill = fully round
            pill: 0.0,    // r_pill = 0 (Meridien sharp pill)
            chip: 0.0,    // r_chip = 0
        },
        strokes: Strokes {
            hair:   0.3,
            thin:   0.5,
            medium: 0.8,
            std:    1.0,
            bold:   1.5,
            thick:  2.0,
            md:     1.5,  // legacy alias for bold
            heavy:  2.0,  // legacy alias for thick
        },
        alphas: Alphas {
            // u8 tiers — defaults from DEFAULT_TOKEN_SNAPSHOT
            faint: 10, ghost: 15, soft_u8: 20, subtle_u8: 40,
            tint: 48, muted_u8: 60, dim: 60, line: 80,
            strong_u8: 80, active: 100, heavy_u8: 120, scrim: 140, solid: 200,
            // f32 multipliers
            subtle:        0.08,  // hover_bg_alpha = 20/255 ≈ 0.08
            soft:          0.14,  // active_bg_alpha = 35/255 ≈ 0.14
            muted:         0.24,
            mid:           0.48,
            strong:        0.72,
            opaque:        1.0,
            header_border: 38.0 / 255.0, // header_outer_border_alpha = 38
        },
        elevation: Elevation::default(),
        density: Density {
            factor: 1.0,               // density=1 → normal
            row_height_dense: 22.0,    // row_height_px = 22
            row_height_comfortable: 28.0,
        },
        shadows: Shadows {
            // shadow_blur=0, shadow_offset_y=0, shadow_alpha=0 → no shadows
            card:     ShadowSpec { blur: 0.0, spread: 0.0, offset_x: 0.0, offset_y: 0.0, alpha: 0.0 },
            modal:    ShadowSpec { blur: 0.0, spread: 0.0, offset_x: 0.0, offset_y: 0.0, alpha: 0.0 },
            tooltip:  ShadowSpec { blur: 0.0, spread: 0.0, offset_x: 0.0, offset_y: 0.0, alpha: 0.0 },
            dropdown: ShadowSpec { blur: 0.0, spread: 0.0, offset_x: 0.0, offset_y: 0.0, alpha: 0.0 },
        },
        treatments: Treatments {
            solid_active_fills:       true,  // solid_active_fills = true
            hairline_borders:         true,  // hairline_borders = true
            uppercase_section_labels: true,  // uppercase_section_labels = true
            segmented_filled_idle:    false,
            focus_ring: FocusRingStyle::Outline, // focus_ring_width = 1.0
            surface_bevel: BevelStyle::None, bevel_highlight_alpha: 0, bevel_shadow_alpha: 0,
            wl_row_side_margin: 0.0, wl_row_corner_radius: 0, wl_row_divider_alpha: 0,
            section_header_mono: false, wl_symbol_mono: false, panel_tab_treatment: 0,
            pane_active_fill_accent: false,
            // Editorial chrome behaviour (from style_defaults(0)).
            serif_headlines: true, button_treatment: 2 /*UnderlineActive*/, invert_active_fill: true,
            vertical_group_dividers: true, show_active_tab_underline: true, inactive_header_fill: true,
            nav_buttons_label_only: true, nav_buttons_uppercase_labels: true, tab_underline_under_text: true,
            card_floating_shadow: true, shadows_enabled: true, animations_enabled: true,
        },
        chrome: Chrome {
            toolbar_height_scale: 1.40, header_height_scale: 1.10, account_strip_height: 36.0,
            pane_border_width: 1.0, pane_gap: 0.0, pane_gap_alpha: 0, pane_active_indicator: 1,
            active_header_fill_multiply: 0.7, inactive_header_fill_multiply: 1.08,
            header_outer_border_alpha: 38, header_outer_border_width: 0.5, header_divider_alpha: 50,
            nav_active_col_alpha: 18, dialog_backdrop_alpha: 0,
            tab_inactive_alpha: 0.6, tab_hover_bg_alpha: 12, tab_underline_thickness: 2.0,
            section_label_padding_top: 4.0, section_label_padding_bottom: 2.0,
            drag_handle_alpha: 0.5, drag_handle_dot_scale: 1.0,
            toast_bg_alpha: 230, card_stripe_alpha: 255, card_floating_shadow_alpha: 25,
            accent_emphasis: 1.0, disabled_opacity: 0.4,
            focus_ring_width: 1.0, focus_ring_alpha: 120, hover_bg_alpha: 20, active_bg_alpha: 35,
            // Meridien: flush contiguous chrome (no floating cards), single row.
            region_gap: 0.0, region_radius: 0.0, region_border_alpha: 40,
            nav_cluster_radius: 0.0, nav_cluster_fill_alpha: 0, nav_cluster_padding: 6.0,
            // Flat: button groups are spaced + divider-separated, no enclosure box.
            button_group: GroupEnclosure::None,
            toolnav_height: 0.0,
            footer_default_open: false,
            panel_header_treatment: 0, panel_section_fill_alpha: 0,
            panel_footer_card: false, panel_footer_radius: 0.0,
        },
    };

    // ── Aperture (id=1) ───────────────────────────────────────────────────────
    // Soft-pill corners, full drop shadows, roomy density.
    let aperture = StyleSystem {
        meta: Meta::new("aperture", "Aperture", true),
        typography: Typography {
            size_xs: 9.0,   // font_caption = 9
            size_sm: 11.0,  // font_body = 11
            size_md: 13.0,
            size_lg: 15.0,
            size_xl: 22.0,  // font_hero = 22
            mono_sm: 11.0,
            mono_md: 13.0,
            mono_lg: 15.0,
            size_section_label: 10.0, // font_section_label = 10
            label_tracking: 0.8, nav_tracking: 0.0, section_tracking: 0.8,
            // Font family identifiers — Aperture uses the compiled-in defaults.
            family_ui: "Inter".to_owned(),
            family_mono: "JetBrains Mono".to_owned(),
            family_display: "Inter".to_owned(),
        },
        spacing: Spacing {
            xs: 2.0,
            sm: 4.0,
            xs_mid: 6.0,      // gap_xs_mid default
            md: 12.0,         // card_padding_y = 12
            lg: 14.0,         // card_padding_x = 14
            xl: 16.0,
            xxl: 24.0,
            gmd: 8.0,
            cta_height: 40.0, // cta_height_px = 40
            cta_padding_x: 12.0, button_height: 28.0, button_padding_x: 14.0, tab_height: 32.0,
        },
        radii: Radii {
            none: 0.0,
            // React fidelity: Aperture's signature big-radius scale (8/10/14/20).
            // Must stay in lockstep with style_defaults(1).r_* (adapter equivalence test).
            xs: 8.0,      // r_xs = 8
            sm: 10.0,     // r_sm = 10
            md: 14.0,     // r_md = 14
            lg: 20.0,     // r_lg = 20
            full: 9999.0, // r_pill = 99
            pill: 99.0, chip: 0.0,
        },
        strokes: Strokes {
            hair:   0.3,  // sub-pixel hairline
            thin:   0.5,  // stroke_hair = 0.5
            medium: 0.8,  // mid-weight default
            std:    1.0,  // stroke_thin = 1.0
            bold:   1.5,  // stroke_bold = 1.5
            thick:  2.0,  // stroke_thick = 2.0
            md:     1.5,  // legacy alias
            heavy:  2.0,  // legacy alias
        },
        alphas: Alphas {
            // u8 tiers — defaults from DEFAULT_TOKEN_SNAPSHOT
            faint: 10, ghost: 15, soft_u8: 20, subtle_u8: 40,
            tint: 48, muted_u8: 60, dim: 60, line: 80,
            strong_u8: 80, active: 100, heavy_u8: 120, scrim: 140, solid: 200,
            // f32 multipliers
            subtle:        0.06,  // hover_bg_alpha = 15/255 ≈ 0.06
            soft:          0.10,  // active_bg_alpha = 25/255 ≈ 0.10
            muted:         0.24,
            mid:           0.48,
            strong:        0.72,
            opaque:        1.0,
            header_border: 38.0 / 255.0, // header_outer_border_alpha = 38
        },
        elevation: Elevation::default(),
        density: Density {
            factor: 1.2,               // density=2 → roomy
            row_height_dense: 26.0,    // row_height_px = 26
            row_height_comfortable: 34.0,
        },
        shadows: Shadows {
            // shadow_blur=24, shadow_offset_y=8, shadow_alpha=40 (≈0.157)
            card:     ShadowSpec { blur: 24.0, spread: 0.0, offset_x: 0.0, offset_y: 8.0, alpha: 40.0 / 255.0 },
            modal:    ShadowSpec { blur: 36.0, spread: 0.0, offset_x: 0.0, offset_y: 12.0, alpha: 40.0 / 255.0 },
            tooltip:  ShadowSpec { blur: 12.0, spread: 0.0, offset_x: 0.0, offset_y: 4.0, alpha: 40.0 / 255.0 },
            dropdown: ShadowSpec { blur: 24.0, spread: 0.0, offset_x: 0.0, offset_y: 8.0, alpha: 40.0 / 255.0 },
        },
        treatments: Treatments {
            solid_active_fills:       false, // solid_active_fills = false
            hairline_borders:         false, // hairline_borders = false
            uppercase_section_labels: false, // uppercase_section_labels = false
            segmented_filled_idle:    true,
            focus_ring: FocusRingStyle::Glow, // focus_ring_width = 2.0 → Glow
            surface_bevel: BevelStyle::None, bevel_highlight_alpha: 0, bevel_shadow_alpha: 0,
            // Aperture: pill rows, filled tabs, mono symbols, orange active header.
            wl_row_side_margin: 6.0, wl_row_corner_radius: 8, wl_row_divider_alpha: 0,
            section_header_mono: false, wl_symbol_mono: true, panel_tab_treatment: 2,
            pane_active_fill_accent: true,
            serif_headlines: false, button_treatment: 4 /*BlackFillActive*/, invert_active_fill: false,
            vertical_group_dividers: false, show_active_tab_underline: true, inactive_header_fill: true,
            nav_buttons_label_only: false, nav_buttons_uppercase_labels: false, tab_underline_under_text: false,
            card_floating_shadow: false, shadows_enabled: true, animations_enabled: true,
        },
        chrome: Chrome {
            toolbar_height_scale: 1.0, header_height_scale: 1.0, account_strip_height: 26.0,
            pane_border_width: 1.0, pane_gap: 8.0, pane_gap_alpha: 0, pane_active_indicator: 2,
            active_header_fill_multiply: 0.7, inactive_header_fill_multiply: 1.08,
            header_outer_border_alpha: 38, header_outer_border_width: 0.5, header_divider_alpha: 50,
            nav_active_col_alpha: 0, dialog_backdrop_alpha: 0,
            tab_inactive_alpha: 0.55, tab_hover_bg_alpha: 18, tab_underline_thickness: 0.0,
            section_label_padding_top: 6.0, section_label_padding_bottom: 2.0,
            drag_handle_alpha: 0.7, drag_handle_dot_scale: 1.0,
            toast_bg_alpha: 200, card_stripe_alpha: 255, card_floating_shadow_alpha: 0,
            accent_emphasis: 1.1, disabled_opacity: 0.5,
            focus_ring_width: 2.0, focus_ring_alpha: 90, hover_bg_alpha: 15, active_bg_alpha: 25,
            // Aperture signature: 8px floating-card chrome, rounded regions + nav pills, 2-row.
            region_gap: 8.0, region_radius: 12.0, region_border_alpha: 40,
            nav_cluster_radius: 99.0, nav_cluster_fill_alpha: 0, nav_cluster_padding: 8.0,
            // Aperture: button groups render as enclosed rounded-rect sections
            // (subtle fill + hairline border); internal dividers are suppressed.
            button_group: GroupEnclosure::Bordered,
            toolnav_height: 30.0,
            // Aperture ships the footer open by default (toggle anytime with Ctrl+`).
            footer_default_open: true,
            // Aperture side panels: filled header toggle + pinned P&L card.
            panel_header_treatment: 2, panel_section_fill_alpha: 0,
            panel_footer_card: true, panel_footer_radius: 10.0,
        },
    };

    // ── Octave (id=2) ─────────────────────────────────────────────────────────
    // Minimal corners, hairline borders, solid active fills, compact density.
    let octave = StyleSystem {
        meta: Meta::new("octave", "Octave", true),
        typography: Typography {
            size_xs: 8.0,   // font_section_label=8, font_caption=8
            size_sm: 10.0,  // font_body = 10
            size_md: 13.0,
            size_lg: 15.0,
            size_xl: 22.0,  // font_hero = 22
            mono_sm: 11.0,
            mono_md: 13.0,
            mono_lg: 15.0,
            size_section_label: 8.0,
            label_tracking: 0.0, nav_tracking: 0.0, section_tracking: 0.0,
            // Font family identifiers — Octave uses the compiled-in defaults.
            family_ui: "Inter".to_owned(),
            family_mono: "JetBrains Mono".to_owned(),
            family_display: "Inter".to_owned(),
        },
        spacing: Spacing {
            xs: 2.0,
            sm: 4.0,
            xs_mid: 6.0,      // gap_xs_mid default
            md: 6.0,          // card_padding_y = 6
            lg: 8.0,          // card_padding_x = 8
            xl: 16.0,
            xxl: 24.0,
            gmd: 8.0,
            cta_height: 32.0, // cta_height_px = 32
            cta_padding_x: 12.0, button_height: 22.0, button_padding_x: 8.0, tab_height: 26.0,
        },
        radii: Radii {
            none: 0.0,
            xs: 1.0,      // r_xs = 1
            sm: 2.0,      // r_sm = 2
            md: 3.0,      // r_md = 3
            lg: 4.0,      // r_lg = 4
            full: 9999.0, // r_pill = 99
            pill: 99.0, chip: 0.0,
        },
        strokes: Strokes {
            hair:   0.3,  // sub-pixel hairline
            thin:   0.4,  // stroke_hair = 0.4
            medium: 0.7,  // scaled mid-weight for compact style
            std:    0.6,  // stroke_thin = 0.6
            bold:   1.0,  // stroke_bold = 1.0
            thick:  1.4,  // stroke_thick = 1.4
            md:     1.0,  // legacy alias
            heavy:  1.4,  // legacy alias
        },
        alphas: Alphas {
            // u8 tiers — defaults from DEFAULT_TOKEN_SNAPSHOT
            faint: 10, ghost: 15, soft_u8: 20, subtle_u8: 40,
            tint: 48, muted_u8: 60, dim: 60, line: 80,
            strong_u8: 80, active: 100, heavy_u8: 120, scrim: 140, solid: 200,
            // f32 multipliers
            subtle:        0.07,  // hover_bg_alpha = 18/255 ≈ 0.07
            soft:          0.12,  // active_bg_alpha = 30/255 ≈ 0.12
            muted:         0.24,
            mid:           0.48,
            strong:        0.72,
            opaque:        1.0,
            header_border: 38.0 / 255.0, // header_outer_border_alpha = 38
        },
        elevation: Elevation::default(),
        density: Density {
            factor: 0.8,               // density=0 → compact
            row_height_dense: 20.0,    // row_height_px = 20
            row_height_comfortable: 28.0,
        },
        shadows: Shadows {
            // shadow_blur=8, shadow_offset_y=4, shadow_alpha=20 (≈0.078)
            card:     ShadowSpec { blur: 8.0,  spread: 0.0, offset_x: 0.0, offset_y: 4.0, alpha: 20.0 / 255.0 },
            modal:    ShadowSpec { blur: 16.0, spread: 0.0, offset_x: 0.0, offset_y: 6.0, alpha: 20.0 / 255.0 },
            tooltip:  ShadowSpec { blur: 6.0,  spread: 0.0, offset_x: 0.0, offset_y: 2.0, alpha: 20.0 / 255.0 },
            dropdown: ShadowSpec { blur: 8.0,  spread: 0.0, offset_x: 0.0, offset_y: 4.0, alpha: 20.0 / 255.0 },
        },
        treatments: Treatments {
            solid_active_fills:       true,  // solid_active_fills = true
            hairline_borders:         true,  // hairline_borders = true
            uppercase_section_labels: true,  // uppercase_section_labels = true
            segmented_filled_idle:    false,
            focus_ring: FocusRingStyle::Outline, // focus_ring_width = 1.5
            surface_bevel: BevelStyle::None, bevel_highlight_alpha: 0, bevel_shadow_alpha: 0,
            wl_row_side_margin: 0.0, wl_row_corner_radius: 0, wl_row_divider_alpha: 0,
            section_header_mono: false, wl_symbol_mono: false, panel_tab_treatment: 0,
            pane_active_fill_accent: false,
            serif_headlines: false, button_treatment: 3 /*RaisedActive*/, invert_active_fill: false,
            vertical_group_dividers: false, show_active_tab_underline: true, inactive_header_fill: true,
            nav_buttons_label_only: false, nav_buttons_uppercase_labels: false, tab_underline_under_text: false,
            card_floating_shadow: false, shadows_enabled: false, animations_enabled: true,
        },
        chrome: Chrome {
            toolbar_height_scale: 1.0, header_height_scale: 1.0, account_strip_height: 26.0,
            pane_border_width: 1.0, pane_gap: 2.0, pane_gap_alpha: 15, pane_active_indicator: 3,
            active_header_fill_multiply: 0.7, inactive_header_fill_multiply: 1.08,
            header_outer_border_alpha: 38, header_outer_border_width: 0.5, header_divider_alpha: 50,
            nav_active_col_alpha: 25, dialog_backdrop_alpha: 0,
            tab_inactive_alpha: 0.5, tab_hover_bg_alpha: 20, tab_underline_thickness: 1.0,
            section_label_padding_top: 3.0, section_label_padding_bottom: 1.0,
            drag_handle_alpha: 0.6, drag_handle_dot_scale: 0.85,
            toast_bg_alpha: 220, card_stripe_alpha: 255, card_floating_shadow_alpha: 0,
            accent_emphasis: 0.95, disabled_opacity: 0.45,
            focus_ring_width: 1.5, focus_ring_alpha: 110, hover_bg_alpha: 18, active_bg_alpha: 30,
            // Octave: dense flush chrome, single row.
            region_gap: 0.0, region_radius: 0.0, region_border_alpha: 40,
            nav_cluster_radius: 2.0, nav_cluster_fill_alpha: 0, nav_cluster_padding: 4.0,
            // Flat dense: minimal-radius groups, no enclosure box (dividers do the work).
            button_group: GroupEnclosure::None,
            toolnav_height: 0.0,
            footer_default_open: true, // Octave is a dense desk style — ops footer on by default.
            panel_header_treatment: 0, panel_section_fill_alpha: 0,
            panel_footer_card: false, panel_footer_radius: 0.0,
        },
    };

    // ── Cadence (id=3) — Spotify-dark: pill primaries, elevated cards ────────
    let cadence = StyleSystem {
        meta: Meta::new("cadence", "Cadence", true),
        typography: Typography {
            size_section_label: 11.0, size_sm: 13.0,
            label_tracking: 0.6, section_tracking: 0.6,
            ..Typography::default()
        },
        radii: Radii { none: 0.0, xs: 4.0, sm: 6.0, md: 10.0, lg: 14.0, full: 9999.0, pill: 99.0, chip: 99.0 },
        density: Density { factor: 1.0, row_height_dense: 26.0, row_height_comfortable: 34.0 },
        shadows: Shadows {
            card:     ShadowSpec { blur: 8.0,  spread: 0.0, offset_x: 0.0, offset_y: 2.0, alpha: 90.0/255.0 },
            modal:    ShadowSpec { blur: 16.0, spread: 0.0, offset_x: 0.0, offset_y: 6.0, alpha: 90.0/255.0 },
            tooltip:  ShadowSpec { blur: 6.0,  spread: 0.0, offset_x: 0.0, offset_y: 2.0, alpha: 90.0/255.0 },
            dropdown: ShadowSpec { blur: 8.0,  spread: 0.0, offset_x: 0.0, offset_y: 4.0, alpha: 90.0/255.0 },
        },
        treatments: Treatments {
            hairline_borders: true, uppercase_section_labels: true, segmented_filled_idle: true,
            surface_bevel: BevelStyle::Raised, bevel_highlight_alpha: 24, bevel_shadow_alpha: 60,
            panel_tab_treatment: 2, wl_row_divider_alpha: 0,
            ..Treatments::default()
        },
        chrome: Chrome {
            pane_gap: 0.0, pane_gap_alpha: 0, pane_active_indicator: 2,
            tab_underline_thickness: 3.0, accent_emphasis: 1.1,
            ..Chrome::default()
        },
        ..StyleSystem::builtin_default()
    };

    // ── Alto (id=4) — Zed warm-dark: raised button faces, amber bevel ────────
    let alto = StyleSystem {
        meta: Meta::new("alto", "Alto", true),
        radii: Radii { none: 0.0, xs: 2.0, sm: 4.0, md: 6.0, lg: 8.0, full: 9999.0, pill: 99.0, chip: 0.0 },
        density: Density { factor: 1.0, row_height_dense: 24.0, row_height_comfortable: 30.0 },
        shadows: Shadows {
            card: ShadowSpec { blur: 8.0, spread: 0.0, offset_x: 0.0, offset_y: 2.0, alpha: 80.0/255.0 },
            ..Shadows::default()
        },
        treatments: Treatments {
            hairline_borders: false, uppercase_section_labels: true,
            surface_bevel: BevelStyle::Raised, bevel_highlight_alpha: 16, bevel_shadow_alpha: 90,
            wl_row_divider_alpha: 22, section_header_mono: true, wl_symbol_mono: true,
            panel_tab_treatment: 0,
            ..Treatments::default()
        },
        chrome: Chrome {
            pane_gap: 0.0, pane_active_indicator: 2, tab_underline_thickness: 2.0,
            ..Chrome::default()
        },
        ..StyleSystem::builtin_default()
    };

    // ── Mariner (id=5) — Alto's nautical sibling, steel-blue, denser ─────────
    let mariner = StyleSystem {
        meta: Meta::new("mariner", "Mariner", true),
        radii: alto.radii.clone(),
        density: Density { factor: 0.85, row_height_dense: 22.0, row_height_comfortable: 28.0 },
        shadows: alto.shadows.clone(),
        treatments: Treatments {
            hairline_borders: false, uppercase_section_labels: true,
            surface_bevel: BevelStyle::Raised, bevel_highlight_alpha: 20, bevel_shadow_alpha: 97,
            wl_row_divider_alpha: 28, section_header_mono: true, wl_symbol_mono: true,
            panel_tab_treatment: 0,
            ..Treatments::default()
        },
        chrome: Chrome {
            // Nautical instrument: denser chrome + steel-blue top stripe on active pane.
            toolbar_height_scale: 0.9, header_height_scale: 0.95,
            pane_gap: 0.0, pane_active_indicator: 1, tab_underline_thickness: 2.0,
            accent_emphasis: 1.05,
            footer_default_open: true, // dense instrument style — ops footer on by default.
            ..Chrome::default()
        },
        ..StyleSystem::builtin_default()
    };

    // ── Lucid (id=6) — editorial LIGHT: flat, restrained radii, serif hero ────
    let lucid = StyleSystem {
        meta: Meta::new("lucid", "Lucid", false),
        typography: Typography { size_xl: 28.0, label_tracking: 0.4, section_tracking: 0.4, ..Typography::default() },
        radii: Radii { none: 0.0, xs: 2.0, sm: 3.0, md: 5.0, lg: 8.0, full: 9999.0, pill: 99.0, chip: 0.0 },
        density: Density { factor: 1.0, row_height_dense: 26.0, row_height_comfortable: 34.0 },
        shadows: Shadows {
            card: ShadowSpec { blur: 0.0, spread: 0.0, offset_x: 0.0, offset_y: 0.0, alpha: 0.0 },
            ..Shadows::default()
        },
        treatments: Treatments {
            solid_active_fills: true, hairline_borders: false, uppercase_section_labels: true,
            serif_headlines: true, invert_active_fill: true,
            surface_bevel: BevelStyle::None, bevel_highlight_alpha: 0, bevel_shadow_alpha: 0,
            wl_row_divider_alpha: 12, panel_tab_treatment: 0, shadows_enabled: false,
            ..Treatments::default()
        },
        chrome: Chrome {
            pane_gap: 0.0, pane_active_indicator: 1, tab_underline_thickness: 0.0,
            // Lucid: sharp editorial outline — thin border, no fill, near-square corners.
            button_group: GroupEnclosure::Sharp,
            ..Chrome::default()
        },
        ..StyleSystem::builtin_default()
    };

    // ── Relay (id=7) — dark Bloomberg/editorial: sharp, serif hero, monospace ─
    let relay = StyleSystem {
        meta: Meta::new("relay", "Relay", true),
        typography: Typography {
            size_xl: 48.0, size_section_label: 9.0,
            label_tracking: 1.2, nav_tracking: 1.5, section_tracking: 1.2,
            ..Typography::default()
        },
        radii: Radii { none: 0.0, xs: 0.0, sm: 0.0, md: 2.0, lg: 4.0, full: 0.0, pill: 0.0, chip: 0.0 },
        density: Density { factor: 1.0, row_height_dense: 24.0, row_height_comfortable: 30.0 },
        shadows: Shadows {
            card: ShadowSpec { blur: 12.0, spread: 0.0, offset_x: 0.0, offset_y: 4.0, alpha: 80.0/255.0 },
            ..Shadows::default()
        },
        treatments: Treatments {
            solid_active_fills: true, hairline_borders: true, uppercase_section_labels: true,
            serif_headlines: true, invert_active_fill: true, vertical_group_dividers: true,
            nav_buttons_label_only: true, nav_buttons_uppercase_labels: true, tab_underline_under_text: true,
            card_floating_shadow: true,
            surface_bevel: BevelStyle::None,
            wl_row_divider_alpha: 30, section_header_mono: true, wl_symbol_mono: true,
            panel_tab_treatment: 0,
            ..Treatments::default()
        },
        chrome: Chrome {
            toolbar_height_scale: 1.20, header_height_scale: 1.05,
            pane_active_indicator: 1, tab_underline_thickness: 2.0,
            card_floating_shadow_alpha: 40, nav_active_col_alpha: 18,
            footer_default_open: true, // Bloomberg-terminal style — ops footer on by default.
            ..Chrome::default()
        },
        ..StyleSystem::builtin_default()
    };

    // ── Glass (id=8) — modern translucent: very large radii, soft shadows ──────
    let glass = StyleSystem {
        meta: Meta::new("glass", "Glass", true),
        typography: Typography { size_xl: 28.0, size_sm: 13.0, ..Typography::default() },
        radii: Radii { none: 0.0, xs: 6.0, sm: 10.0, md: 16.0, lg: 24.0, full: 9999.0, pill: 99.0, chip: 99.0 },
        density: Density { factor: 1.2, row_height_dense: 30.0, row_height_comfortable: 38.0 },
        shadows: Shadows {
            card:     ShadowSpec { blur: 32.0, spread: 0.0, offset_x: 0.0, offset_y: 8.0, alpha: 30.0/255.0 },
            modal:    ShadowSpec { blur: 48.0, spread: 0.0, offset_x: 0.0, offset_y: 12.0, alpha: 30.0/255.0 },
            tooltip:  ShadowSpec { blur: 16.0, spread: 0.0, offset_x: 0.0, offset_y: 4.0, alpha: 30.0/255.0 },
            dropdown: ShadowSpec { blur: 24.0, spread: 0.0, offset_x: 0.0, offset_y: 8.0, alpha: 30.0/255.0 },
        },
        treatments: Treatments {
            segmented_filled_idle: true, surface_bevel: BevelStyle::None,
            wl_row_side_margin: 4.0, wl_row_corner_radius: 10, panel_tab_treatment: 2,
            ..Treatments::default()
        },
        chrome: Chrome {
            toolbar_height_scale: 1.05, pane_gap: 0.0, pane_gap_alpha: 0,
            pane_active_indicator: 2, tab_underline_thickness: 0.0, accent_emphasis: 1.2,
            // Glass: airy floating cards, generous radius, 2-row.
            region_gap: 8.0, region_radius: 16.0, region_border_alpha: 30,
            nav_cluster_radius: 99.0, nav_cluster_fill_alpha: 0, nav_cluster_padding: 10.0,
            // Glass: frosted fill-only group (no hard border) — distinct from Aperture's box.
            button_group: GroupEnclosure::Frosted,
            toolnav_height: 32.0,
            panel_header_treatment: 2, panel_footer_card: true, panel_footer_radius: 16.0,
            ..Chrome::default()
        },
        ..StyleSystem::builtin_default()
    };

    vec![meridien, aperture, octave, cadence, alto, mariner, lucid, relay, glass]
}

/// Builds a `ThemeRegistry` pre-populated with all built-in `ColorScheme`s
/// (derived from `gpu::THEMES`) and the default `StyleSystem`(s).
///
/// The first color scheme (Midnight) becomes the active default; the first
/// registered `StyleSystem` (apex-default) becomes the active style.
pub fn builtin_registry() -> ThemeRegistry {
    let mut reg = ThemeRegistry::with_builtins();
    for scheme in builtin_color_schemes() {
        reg.register_colors(scheme);
    }
    for style in builtin_style_systems() {
        reg.register_style(style);
    }
    reg
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_schemes_are_non_empty() {
        let schemes = builtin_color_schemes();
        assert!(
            !schemes.is_empty(),
            "builtin_color_schemes() must return at least one scheme"
        );
    }

    #[test]
    fn every_scheme_has_non_empty_id() {
        for scheme in builtin_color_schemes() {
            assert!(
                !scheme.meta.id.is_empty(),
                "scheme '{}' has an empty id",
                scheme.meta.name
            );
        }
    }

    #[test]
    fn scheme_count_matches_themes() {
        // gpu::THEMES has 17 entries (8 dark originals + 4 more dark + 4 light + 1 = 17 actually 17)
        // We transcribed all 17: Midnight, Nord, Monokai, Solarized, Dracula, Gruvbox,
        // Catppuccin, Tokyo Night, Kanagawa, Everforest, Vesper, Rosé Pine,
        // Bauhaus, Peach, Ivory, Newsprint.
        let schemes = builtin_color_schemes();
        // 16 THEMES-backed schemes + 5 React palette ports (Aperture/Cadence/Alto/Mariner/Lucid).
        assert_eq!(schemes.len(), 21, "expected 21 schemes (16 THEMES + 5 React ports)");
    }

    #[test]
    fn dark_light_classification() {
        let schemes = builtin_color_schemes();
        let dark_names = ["Midnight", "Nord", "Monokai", "Solarized", "Dracula",
                          "Gruvbox", "Catppuccin", "Tokyo Night", "Kanagawa",
                          "Everforest", "Vesper", "Rosé Pine"];
        let light_names = ["Bauhaus", "Peach", "Ivory", "Newsprint"];

        for s in &schemes {
            if dark_names.contains(&s.meta.name.as_str()) {
                assert!(s.meta.is_dark, "{} should be dark", s.meta.name);
            } else if light_names.contains(&s.meta.name.as_str()) {
                assert!(!s.meta.is_dark, "{} should be light", s.meta.name);
            }
        }
    }

    #[test]
    fn builtin_registry_has_gpu_themes() {
        let reg = builtin_registry();
        let ids = reg.color_ids();
        // Spot-check a few
        assert!(ids.contains(&"midnight"),     "Midnight must be in registry");
        assert!(ids.contains(&"dracula"),      "Dracula must be in registry");
        assert!(ids.contains(&"rose-pine"),    "Rose Pine must be in registry");
        assert!(ids.contains(&"bauhaus"),      "Bauhaus must be in registry");
        assert!(ids.contains(&"tokyo-night"),  "Tokyo Night must be in registry");
    }

    #[test]
    fn builtin_style_systems_has_three_entries() {
        let styles = builtin_style_systems();
        // 3 canonical (Meridien/Aperture/Octave) + 6 React ports (Cadence/Alto/
        // Mariner/Lucid/Relay/Glass) = 9 authoritative style systems.
        assert_eq!(styles.len(), 9, "builtin_style_systems() must return 9 entries");
        for s in &styles {
            assert!(!s.meta.id.is_empty(), "style '{}' has an empty id", s.meta.name);
        }
    }

    #[test]
    fn builtin_style_systems_ids_are_correct() {
        let styles = builtin_style_systems();
        let ids: Vec<&str> = styles.iter().map(|s| s.meta.id.as_str()).collect();
        assert!(ids.contains(&"meridien"), "meridien must be present");
        assert!(ids.contains(&"aperture"), "aperture must be present");
        assert!(ids.contains(&"octave"),   "octave must be present");
    }

    #[test]
    fn builtin_registry_has_style_systems() {
        let reg = builtin_registry();
        let ids = reg.style_ids();
        assert!(ids.contains(&"meridien"), "meridien must be in registry");
        assert!(ids.contains(&"aperture"), "aperture must be in registry");
        assert!(ids.contains(&"octave"),   "octave must be in registry");
    }
}
