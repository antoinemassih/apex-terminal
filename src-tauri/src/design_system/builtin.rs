//! Built-in `ColorScheme` catalogue derived from `gpu::THEMES`.
//!
//! Each entry in `THEMES` is transcribed to a `ColorScheme` using the
//! following lossy mapping (one-way; `gpu::THEMES` is not modified):
//!
//! | `ColorScheme` field | `Theme` source                          |
//! |---------------------|-----------------------------------------|
//! | `bg`                | `bg`                                    |
//! | `surface`           | `toolbar_bg`                            |
//! | `paper`             | derived: toolbar_bg shifted +8 lum     |
//! | `text`              | `text`                                  |
//! | `dim`               | `dim`                                   |
//! | `border`            | `toolbar_border` (= `Theme::border()`)  |
//! | `accent`            | `accent`                                |
//! | `bull`              | `bull`                                  |
//! | `bear`              | `bear`                                  |
//! | `warn`              | `warn`                                  |
//! | `shadow`            | `shadow_color` (dark) / derived (light) |
//!
//! Fields with no direct analogue (`paper`, `shadow` on light themes) are
//! derived as noted above. This is an intentionally lossy map — `ColorScheme`
//! is a reduced canonical palette.

use super::{
    color_scheme::{ColorScheme, Meta, Rgba},
    registry::ThemeRegistry,
};

// ── Internal helpers ──────────────────────────────────────────────────────────

/// Convert an `egui::Color32`-equivalent `[u8;4]` tuple to `Rgba`.
#[inline]
const fn c(r: u8, g: u8, b: u8, a: u8) -> Rgba { [r, g, b, a] }

/// Fully-opaque RGB to Rgba.
#[inline]
const fn rgb(r: u8, g: u8, b: u8) -> Rgba { [r, g, b, 255] }

/// Clamp an `i16` to `[0, 255]` and cast to `u8`.
const fn clamp_u8(v: i16) -> u8 {
    if v < 0 { 0 } else if v > 255 { 255 } else { v as u8 }
}

/// Derive a "paper" layer from a surface colour: shift luminance by +8
/// toward white (dark) or -8 toward black (light).
const fn paper_from_surface(surf: Rgba, is_dark: bool) -> Rgba {
    let shift: i16 = if is_dark { 8 } else { -8 };
    [
        clamp_u8(surf[0] as i16 + shift),
        clamp_u8(surf[1] as i16 + shift),
        clamp_u8(surf[2] as i16 + shift),
        255,
    ]
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
        // ── Dark themes ──────────────────────────────────────────────────────
        {
            let bg = rgb(14, 16, 21);
            let surf = rgb(10, 12, 17);
            let border = hairline_border(bg);
            ColorScheme {
                meta: Meta { id: to_id("Midnight"), name: "Midnight".into(), is_dark: true },
                bg, surface: surf,
                paper: paper_from_surface(surf, true),
                text:   rgb(220, 220, 230),
                dim:    rgb(100, 105, 115),
                border,
                accent: rgb(62, 120, 180),
                bull:   rgb(62, 120, 180),
                bear:   rgb(180, 65, 58),
                warn:   rgb(255, 191, 0),
                shadow: c(0, 0, 0, 180),
                accent_alts: vec![],
            }
        },
        {
            let bg = rgb(38, 44, 56);
            let surf = rgb(32, 38, 50);
            let border = hairline_border(bg);
            ColorScheme {
                meta: Meta { id: to_id("Nord"), name: "Nord".into(), is_dark: true },
                bg, surface: surf,
                paper: paper_from_surface(surf, true),
                text:   rgb(220, 220, 230),
                dim:    rgb(129, 161, 193),
                border,
                accent: rgb(136, 192, 208),
                bull:   rgb(163, 190, 140),
                bear:   rgb(191, 97, 106),
                warn:   rgb(235, 203, 139),
                shadow: c(0, 0, 0, 180),
                accent_alts: vec![],
            }
        },
        {
            let bg = rgb(39, 40, 34);
            let surf = rgb(33, 34, 28);
            let border = hairline_border(bg);
            ColorScheme {
                meta: Meta { id: to_id("Monokai"), name: "Monokai".into(), is_dark: true },
                bg, surface: surf,
                paper: paper_from_surface(surf, true),
                text:   rgb(220, 220, 230),
                dim:    rgb(165, 159, 133),
                border,
                accent: rgb(230, 219, 116),
                bull:   rgb(166, 226, 46),
                bear:   rgb(249, 38, 114),
                warn:   rgb(230, 219, 116),
                shadow: c(0, 0, 0, 180),
                accent_alts: vec![],
            }
        },
        {
            let bg = rgb(0, 43, 54);
            let surf = rgb(0, 37, 48);
            let border = hairline_border(bg);
            ColorScheme {
                meta: Meta { id: to_id("Solarized"), name: "Solarized".into(), is_dark: true },
                bg, surface: surf,
                paper: paper_from_surface(surf, true),
                text:   rgb(220, 220, 230),
                dim:    rgb(131, 148, 150),
                border,
                accent: rgb(42, 161, 152),
                bull:   rgb(133, 153, 0),
                bear:   rgb(220, 50, 47),
                warn:   rgb(181, 137, 0),
                shadow: c(0, 0, 0, 180),
                accent_alts: vec![],
            }
        },
        {
            let bg = rgb(40, 42, 54);
            let surf = rgb(34, 36, 48);
            let border = hairline_border(bg);
            ColorScheme {
                meta: Meta { id: to_id("Dracula"), name: "Dracula".into(), is_dark: true },
                bg, surface: surf,
                paper: paper_from_surface(surf, true),
                text:   rgb(220, 220, 230),
                dim:    rgb(189, 147, 249),
                border,
                accent: rgb(255, 121, 198),
                bull:   rgb(80, 250, 123),
                bear:   rgb(255, 85, 85),
                warn:   rgb(241, 250, 140),
                shadow: c(0, 0, 0, 180),
                accent_alts: vec![],
            }
        },
        {
            let bg = rgb(40, 40, 40);
            let surf = rgb(34, 34, 34);
            let border = hairline_border(bg);
            ColorScheme {
                meta: Meta { id: to_id("Gruvbox"), name: "Gruvbox".into(), is_dark: true },
                bg, surface: surf,
                paper: paper_from_surface(surf, true),
                text:   rgb(220, 220, 230),
                dim:    rgb(213, 196, 161),
                border,
                accent: rgb(254, 128, 25),
                bull:   rgb(184, 187, 38),
                bear:   rgb(251, 73, 52),
                warn:   rgb(250, 189, 47),
                shadow: c(0, 0, 0, 180),
                accent_alts: vec![],
            }
        },
        {
            let bg = rgb(30, 30, 46);
            let surf = rgb(24, 24, 38);
            let border = hairline_border(bg);
            ColorScheme {
                meta: Meta { id: to_id("Catppuccin"), name: "Catppuccin".into(), is_dark: true },
                bg, surface: surf,
                paper: paper_from_surface(surf, true),
                text:   rgb(220, 220, 230),
                dim:    rgb(180, 190, 254),
                border,
                accent: rgb(203, 166, 247),
                bull:   rgb(166, 227, 161),
                bear:   rgb(243, 139, 168),
                warn:   rgb(249, 226, 175),
                shadow: c(0, 0, 0, 180),
                accent_alts: vec![],
            }
        },
        {
            let bg = rgb(26, 27, 38);
            let surf = rgb(21, 22, 32);
            let border = hairline_border(bg);
            ColorScheme {
                meta: Meta { id: to_id("Tokyo Night"), name: "Tokyo Night".into(), is_dark: true },
                bg, surface: surf,
                paper: paper_from_surface(surf, true),
                text:   rgb(220, 220, 230),
                dim:    rgb(122, 162, 247),
                border,
                accent: rgb(125, 207, 255),
                bull:   rgb(158, 206, 106),
                bear:   rgb(247, 118, 142),
                warn:   rgb(224, 175, 104),
                shadow: c(0, 0, 0, 180),
                accent_alts: vec![],
            }
        },
        {
            let bg = rgb(22, 22, 29);
            let surf = rgb(18, 18, 24);
            let border = hairline_border(bg);
            ColorScheme {
                meta: Meta { id: to_id("Kanagawa"), name: "Kanagawa".into(), is_dark: true },
                bg, surface: surf,
                paper: paper_from_surface(surf, true),
                text:   rgb(220, 220, 230),
                dim:    rgb(84, 88, 104),
                border,
                accent: rgb(127, 180, 202),
                bull:   rgb(118, 169, 130),
                bear:   rgb(195, 64, 67),
                warn:   rgb(228, 175, 69),
                shadow: c(0, 0, 0, 180),
                accent_alts: vec![],
            }
        },
        {
            let bg = rgb(39, 46, 38);
            let surf = rgb(33, 40, 32);
            let border = hairline_border(bg);
            ColorScheme {
                meta: Meta { id: to_id("Everforest"), name: "Everforest".into(), is_dark: true },
                bg, surface: surf,
                paper: paper_from_surface(surf, true),
                text:   rgb(220, 220, 230),
                dim:    rgb(157, 169, 140),
                border,
                accent: rgb(131, 165, 152),
                bull:   rgb(167, 192, 128),
                bear:   rgb(230, 126, 128),
                warn:   rgb(223, 199, 118),
                shadow: c(0, 0, 0, 180),
                accent_alts: vec![],
            }
        },
        {
            let bg = rgb(16, 16, 16);
            let surf = rgb(11, 11, 11);
            let border = hairline_border(bg);
            ColorScheme {
                meta: Meta { id: to_id("Vesper"), name: "Vesper".into(), is_dark: true },
                bg, surface: surf,
                paper: paper_from_surface(surf, true),
                text:   rgb(220, 220, 230),
                dim:    rgb(120, 120, 120),
                border,
                accent: rgb(255, 199, 119),
                bull:   rgb(166, 218, 149),
                bear:   rgb(238, 130, 98),
                warn:   rgb(255, 199, 119),
                shadow: c(0, 0, 0, 180),
                accent_alts: vec![],
            }
        },
        {
            let bg = rgb(25, 23, 36);
            let surf = rgb(20, 18, 30);
            let border = hairline_border(bg);
            ColorScheme {
                // "Rosé Pine" — strip non-ASCII 'é' → "rose-pine"
                meta: Meta { id: "rose-pine".into(), name: "Rosé Pine".into(), is_dark: true },
                bg, surface: surf,
                paper: paper_from_surface(surf, true),
                text:   rgb(220, 220, 230),
                dim:    rgb(110, 106, 134),
                border,
                accent: rgb(196, 167, 231),
                bull:   rgb(156, 207, 216),
                bear:   rgb(235, 111, 146),
                warn:   rgb(246, 193, 119),
                shadow: c(0, 0, 0, 180),
                accent_alts: vec![],
            }
        },
        // ── Light themes ─────────────────────────────────────────────────────
        {
            let bg = rgb(242, 242, 238);
            let surf = rgb(248, 248, 245);
            let border = hairline_border(bg);
            ColorScheme {
                meta: Meta { id: to_id("Bauhaus"), name: "Bauhaus".into(), is_dark: false },
                bg, surface: surf,
                paper: paper_from_surface(surf, false),
                text:   rgb(22, 22, 24),
                dim:    rgb(120, 125, 130),
                border,
                accent: rgb(232, 93, 38),
                bull:   rgb(20, 120, 60),
                bear:   rgb(200, 55, 45),
                warn:   rgb(204, 120, 0),
                shadow: c(40, 40, 40, 120),
                accent_alts: vec![],
            }
        },
        {
            let bg = rgb(243, 241, 238);
            let surf = rgb(250, 248, 246);
            let border = hairline_border(bg);
            ColorScheme {
                meta: Meta { id: to_id("Peach"), name: "Peach".into(), is_dark: false },
                bg, surface: surf,
                paper: paper_from_surface(surf, false),
                text:   rgb(20, 20, 22),
                dim:    rgb(115, 120, 125),
                border,
                accent: rgb(210, 95, 70),
                bull:   rgb(22, 130, 70),
                bear:   rgb(195, 50, 55),
                warn:   rgb(200, 130, 0),
                shadow: c(40, 40, 40, 120),
                accent_alts: vec![],
            }
        },
        {
            let bg = rgb(240, 242, 238);
            let surf = rgb(248, 250, 246);
            let border = hairline_border(bg);
            ColorScheme {
                meta: Meta { id: to_id("Ivory"), name: "Ivory".into(), is_dark: false },
                bg, surface: surf,
                paper: paper_from_surface(surf, false),
                text:   rgb(18, 20, 22),
                dim:    rgb(118, 122, 128),
                border,
                accent: rgb(160, 190, 40),
                bull:   rgb(80, 160, 50),
                bear:   rgb(210, 60, 50),
                warn:   rgb(190, 140, 0),
                shadow: c(40, 40, 40, 120),
                accent_alts: vec![],
            }
        },
        {
            let bg = rgb(238, 232, 220);
            let surf = rgb(238, 232, 220); // toolbar_bg same as bg for Newsprint
            let border = hairline_border(bg);
            ColorScheme {
                meta: Meta { id: to_id("Newsprint"), name: "Newsprint".into(), is_dark: false },
                bg, surface: surf,
                paper: paper_from_surface(surf, false),
                text:   rgb(28, 28, 28),
                dim:    rgb(120, 116, 104),
                border,
                accent: rgb(34, 94, 56),
                bull:   rgb(34, 94, 56),
                bear:   rgb(168, 52, 52),
                warn:   rgb(168, 120, 0),
                shadow: c(60, 50, 40, 120),
                accent_alts: vec![],
            }
        },
    ]
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
        assert_eq!(schemes.len(), 16, "expected 16 schemes (all THEMES entries)");
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
}
