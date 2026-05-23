//! Theme contract for ui_kit widgets.
//!
//! Widgets take `&dyn ComponentTheme` instead of `&chart_renderer::gpu::Theme`
//! so the kit could extract as a standalone crate. The trait exposes only
//! what widgets actually need — the 6-color palette + a few derived
//! getters. Add fields cautiously; every new field couples the kit to a
//! specific theme shape.

use egui::Color32;

// Re-export the trading-app's concrete Theme + a few god-object types and
// chart-app accessors so other ui_kit widgets don't have to reach into
// `chart_renderer::gpu`. Part of the UI extraction (see
// docs/UI_EXTRACTION.md): when ui_kit moves to its own crate, this is the
// single bridge point that gets rewritten.
#[allow(unused_imports)]
pub(crate) use crate::chart_renderer::gpu::{
    Theme, Watchlist, SplitSection,
    pane_tabs_header_h, live_theme_count, get_theme,
};

pub trait ComponentTheme {
    // Core 6-color palette (matches the discipline established in item 4).
    fn accent(&self) -> Color32;
    fn bull(&self) -> Color32;
    fn bear(&self) -> Color32;
    fn text(&self) -> Color32;
    fn dim(&self) -> Color32;
    fn border(&self) -> Color32;
    fn border_variant(&self) -> Color32;
    fn warn(&self) -> Color32;

    // Surface tokens (background fills).
    fn bg(&self) -> Color32;
    fn surface(&self) -> Color32; // raised surface, e.g. toolbar_bg

    /// L2 surface — one layer up from `surface()`. Used for inputs,
    /// cards, sub-section bodies. Direction-aware: lighter on dark
    /// themes, darker on light themes. Default impl uses the standard
    /// `color_layer_up(t, 1)` 7%-step heuristic.
    fn surface_raised(&self) -> Color32 {
        // Heuristic: detect dark vs light by `bg()` luminance, shift
        // surface() toward text() by ~7%. Matches `style::color_layer_up`.
        let base = self.surface();
        let target = self.text();
        let bg = self.bg();
        let is_dark = (bg.r() as i16 + bg.g() as i16 + bg.b() as i16) < 384;
        let _ = target;
        let shift: i16 = if is_dark { 18 } else { -18 };
        let clamp = |c: i16| -> u8 { c.clamp(0, 255) as u8 };
        Color32::from_rgb(
            clamp(base.r() as i16 + shift),
            clamp(base.g() as i16 + shift),
            clamp(base.b() as i16 + shift),
        )
    }

    // Element state alpha overlays (Zed-derived). Applied OVER an element's
    // idle background to signal hover/active/selected/disabled without
    // switching colors. Pre-computed in the theme preset.
    fn element_hover(&self) -> Color32;
    fn element_active(&self) -> Color32;
    fn element_selected(&self) -> Color32;
    fn element_disabled(&self) -> Color32;
    fn ghost_hover(&self) -> Color32;
    fn ghost_active(&self) -> Color32;

    // Icon color ramp — decoupled from text hierarchy.
    fn icon(&self) -> Color32;
    fn icon_muted(&self) -> Color32;
    fn icon_disabled(&self) -> Color32;
    fn icon_accent(&self) -> Color32;

    /// Shadow tint. Themes pick a near-black for dark palettes and a soft
    /// gray for light palettes so drops don't read as a hard black smudge
    /// on Bauhaus / Peach / Ivory / Newsprint. The default impl returns
    /// `Color32::BLACK` as a safe fallback; concrete themes override.
    fn shadow_color(&self) -> Color32 { Color32::BLACK }

    /// Semantic "success" colour — green-ish. Default impl delegates to
    /// `bull()` so trading themes keep their existing palette; non-trading
    /// themes (e.g. a doc app) override to provide a non-bull semantic.
    fn success(&self) -> Color32 { self.bull() }

    /// Semantic "danger" colour — red-ish. Default impl delegates to
    /// `bear()`. Used by form validation, error indicators, destructive
    /// actions — anywhere the meaning is "warning/wrong/destroy" rather
    /// than "the market went down".
    fn danger(&self) -> Color32 { self.bear() }

    // ── Semantic surface tokens (Phase 2c — portability) ─────────────────────
    // These previously lived in chart_renderer::ui::style as free functions
    // taking `&Theme` (concrete). Moved onto the trait as defaults so widgets
    // can compute them from `&dyn ComponentTheme` without a chart-app dep.

    /// Border colour for a framed surface. Defaults to `border()`. Override
    /// only if a theme wants a separate "raised-surface" border tone.
    fn surface_border(&self) -> Color32 { self.border() }

    /// Header band background — one elevation step over `bg()`.
    fn header_surface(&self) -> Color32 { self.bg().gamma_multiply(0.95) }

    /// Section-header band background — two elevation steps over `bg()`.
    fn section_header_surface(&self) -> Color32 { self.bg().gamma_multiply(0.88) }

    /// Panel body background — three elevation steps over `bg()`.
    fn panel_surface(&self) -> Color32 { self.bg().gamma_multiply(0.85) }

    /// Header divider/border colour. 38α over `text()`.
    fn header_border(&self) -> Color32 {
        let t = self.text();
        Color32::from_rgba_unmultiplied(t.r(), t.g(), t.b(), 38)
    }
}

// `impl ComponentTheme for crate::chart_renderer::gpu::Theme` is the
// chart-app's bridge to this trait and lives in `chart_renderer::theme_impl`
// (correct dependency direction: chart_renderer -> ui_kit, not the reverse).
// Re-exported here for back-compat with widgets that import
// `super::theme::active_theme`.
pub use crate::chart_renderer::theme_impl::active_theme;

/// Read the active theme index stashed in egui memory by the render loop.
/// Falls back to 0 (Midnight) if nothing was stashed. Portable — no
/// chart-app dependency.
pub fn active_theme_idx(ctx: &egui::Context) -> usize {
    ctx.data(|d| d.get_temp::<usize>(egui::Id::new("apex_active_theme_idx")))
       .unwrap_or(0)
}

// ── Ambient theme (UI extraction, item 2) ────────────────────────────────────
//
// Hosts (the chart app, or any other app embedding ui_kit) stash the active
// `Theme` in egui memory once per frame. ui_kit widgets that have no
// theme arg (e.g. `Widget` impls returning `Response`) read it back via
// `get_ambient_theme(ctx)` instead of reaching into the chart-app's live
// theme registry. This severs `active_theme()`'s hard dependency on
// `chart_renderer::gpu::get_theme(idx)` — the registry is now a fallback.

const AMBIENT_KEY: &str = "apex_ambient_theme";

/// Stash the current `Theme` in egui memory so ui_kit's parameter-less
/// widgets can find it. Call this once per frame from the host app's
/// render loop, before any UI is built. Cheap — one cloned `Theme`
/// insertion per frame.
pub fn set_ambient_theme(ctx: &egui::Context, theme: Theme) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::new(AMBIENT_KEY), theme));
}

/// Read the ambient `Theme` set by [`set_ambient_theme`]. Returns `None`
/// if the host hasn't set one this frame — callers should fall back to
/// whatever's appropriate (the chart-app version of `active_theme()`
/// falls back to the live theme registry by index).
pub fn get_ambient_theme(ctx: &egui::Context) -> Option<Theme> {
    ctx.data(|d| d.get_temp::<Theme>(egui::Id::new(AMBIENT_KEY)))
}

impl<T: ComponentTheme + ?Sized> ComponentTheme for &T {
    fn accent(&self) -> Color32 { (**self).accent() }
    fn bull(&self) -> Color32 { (**self).bull() }
    fn bear(&self) -> Color32 { (**self).bear() }
    fn text(&self) -> Color32 { (**self).text() }
    fn dim(&self) -> Color32 { (**self).dim() }
    fn border(&self) -> Color32 { (**self).border() }
    fn border_variant(&self) -> Color32 { (**self).border_variant() }
    fn warn(&self) -> Color32 { (**self).warn() }
    fn bg(&self) -> Color32 { (**self).bg() }
    fn surface(&self) -> Color32 { (**self).surface() }
    fn element_hover(&self) -> Color32 { (**self).element_hover() }
    fn element_active(&self) -> Color32 { (**self).element_active() }
    fn element_selected(&self) -> Color32 { (**self).element_selected() }
    fn element_disabled(&self) -> Color32 { (**self).element_disabled() }
    fn ghost_hover(&self) -> Color32 { (**self).ghost_hover() }
    fn ghost_active(&self) -> Color32 { (**self).ghost_active() }
    fn icon(&self) -> Color32 { (**self).icon() }
    fn icon_muted(&self) -> Color32 { (**self).icon_muted() }
    fn icon_disabled(&self) -> Color32 { (**self).icon_disabled() }
    fn icon_accent(&self) -> Color32 { (**self).icon_accent() }
    fn shadow_color(&self) -> Color32 { (**self).shadow_color() }
}
