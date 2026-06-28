//! `ColorScheme → Theme` adapter.
//!
//! Converts a [`ColorScheme`] (the design-system palette representation) into
//! a [`gpu::Theme`] (the runtime egui colour struct used by all render code).
//!
//! ## Field classification
//!
//! | Category | Fields | Strategy |
//! |---|---|---|
//! | **Base** | `bg`, `toolbar_bg`, `text`, `dim`, `accent`, `bull`, `bear`, `warn`, `shadow_color` | Direct copy from `ColorScheme` |
//! | **Derived** | `toolbar_border`, `border_variant`, `element_hover/active`, `ghost_hover/active`, `element_selected`, `element_disabled`, `icon`, `icon_muted`, `icon_disabled`, `icon_accent` | Recomputed using the same `gpu` helpers |
//! | **Palette extension** | `cmd_palette` | Copied from the `ColorScheme` (themable since P1.4) |
//! | **Hand-authored extras** | `notification_red`, `gold`, `overlay_text`, `rrg_*`, `pinned_row_tint`, `text_muted`, `hud_bg`, `hud_border` | Direct copy from new `ColorScheme` extra fields |
//!
//! ## Derived-field formulas (identical to `gpu.rs`)
//!
//! ```text
//! toolbar_border   = hairline_border(bg)
//! border_variant   = hairline_border_variant(bg)
//! element_hover    = element_overlay(bg, 12)
//! element_active   = element_overlay(bg, 24)
//! element_selected = alpha(accent, 24)
//! element_disabled = alpha(dim, 80)
//! ghost_hover      = element_overlay(bg, 6)
//! ghost_active     = element_overlay(bg, 12)
//! icon             = text
//! icon_muted       = alpha(text, 178)
//! icon_disabled    = alpha(text, 102)
//! icon_accent      = accent
//! cmd_palette      = cs.cmd_palette  (per-scheme; defaults to CMD_PALETTE_DEFAULT)
//! ```
//!
//! The adapter is the prerequisite for making `design_system` the runtime
//! colour source; wiring it into `get_theme()` is a separate step (not in
//! scope here).

use crate::design_system::color_scheme::ColorScheme;
use super::gpu::{hairline_border, hairline_border_variant, Theme};

/// Convert a `ColorScheme` into a `gpu::Theme`.
///
/// Base fields map directly; derived fields are recomputed using the same
/// helper functions that the `THEMES` const array uses; hand-authored extra
/// fields are carried over from the new `ColorScheme` extra fields verbatim.
///
/// The output is byte-identical to the corresponding `THEMES[i]` entry when
/// `cs` was produced by `builtin_color_schemes()[i]`.
pub fn color_scheme_to_theme(cs: &ColorScheme) -> Theme {
    // ── Base colour conversions ───────────────────────────────────────────────
    let bg        = c32(cs.bg);
    let toolbar_bg = c32(cs.surface);
    let text      = c32(cs.text);
    let dim       = c32(cs.dim);
    let accent    = c32(cs.accent);
    let bull      = c32(cs.bull);
    let bear      = c32(cs.bear);
    let warn      = c32(cs.warn);
    // shadow_color in gpu::Theme is stored as fully-opaque RGB (no alpha).
    // The alpha carried in ColorScheme.shadow is the design-system encoding;
    // gpu::Theme.shadow_color uses only the RGB bytes.
    let shadow_color = egui::Color32::from_rgb(cs.shadow[0], cs.shadow[1], cs.shadow[2]);

    // ── Derived fields (same formulas as gpu.rs THEMES const) ────────────────
    let toolbar_border   = hairline_border(bg);
    let border_variant   = hairline_border_variant(bg);
    // P12: 10 Zed-style overlay derivations (element_*, ghost_*, icon_*) were
    // formerly stored on Theme. They now live in theme_impl.rs at the
    // ComponentTheme trait boundary so palette overrides flow through.
    // cmd_palette: read from the ColorScheme (palette axis owns it now).
    // Convert the 11 Rgba slots to fully-opaque Color32.
    let cmd_palette: [egui::Color32; 11] = [
        c32(cs.cmd_palette[0]),  c32(cs.cmd_palette[1]),  c32(cs.cmd_palette[2]),
        c32(cs.cmd_palette[3]),  c32(cs.cmd_palette[4]),  c32(cs.cmd_palette[5]),
        c32(cs.cmd_palette[6]),  c32(cs.cmd_palette[7]),  c32(cs.cmd_palette[8]),
        c32(cs.cmd_palette[9]),  c32(cs.cmd_palette[10]),
    ];

    // ── Hand-authored extras ──────────────────────────────────────────────────
    let notification_red = c32(cs.notification_red);
    let gold             = c32(cs.gold);
    let overlay_text     = c32(cs.overlay_text);
    let rrg_leading      = c32(cs.rrg_leading);
    let rrg_improving    = c32(cs.rrg_improving);
    let rrg_weakening    = c32(cs.rrg_weakening);
    let rrg_lagging      = c32(cs.rrg_lagging);
    // pinned_row_tint and hud_bg are stored as premultiplied Color32 in gpu::Theme.
    // ColorScheme stores the same premultiplied bytes in pre_rgba fields.
    let pinned_row_tint = egui::Color32::from_rgba_premultiplied(
        cs.pinned_row_tint[0], cs.pinned_row_tint[1],
        cs.pinned_row_tint[2], cs.pinned_row_tint[3],
    );
    let text_muted = c32(cs.text_muted);
    let hud_bg = egui::Color32::from_rgba_premultiplied(
        cs.hud_bg[0], cs.hud_bg[1], cs.hud_bg[2], cs.hud_bg[3],
    );
    let hud_border = c32(cs.hud_border);

    // ── Assemble Theme ────────────────────────────────────────────────────────
    Theme {
        name: Box::leak(cs.meta.name.clone().into_boxed_str()),
        bg,
        toolbar_bg,
        accent,
        bull,
        bear,
        text,
        dim,
        toolbar_border,
        border_variant,
        warn,
        cmd_palette,
        gold,
        notification_red,
        shadow_color,
        overlay_text,
        rrg_leading,
        rrg_improving,
        rrg_weakening,
        rrg_lagging,
        pinned_row_tint,
        text_muted,
        hud_bg,
        hud_border,
    }
}

// ── Helper ────────────────────────────────────────────────────────────────────

/// Convert an opaque `Rgba` (`[u8;4]`) to `egui::Color32` (fully opaque RGB).
#[inline]
fn c32(rgba: crate::design_system::color_scheme::Rgba) -> egui::Color32 {
    egui::Color32::from_rgb(rgba[0], rgba[1], rgba[2])
}
