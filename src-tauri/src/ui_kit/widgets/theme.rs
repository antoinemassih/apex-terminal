//! Theme contract for ui_kit widgets.
//!
//! Widgets take `&dyn ComponentTheme` instead of `&chart_renderer::gpu::Theme`
//! so the kit could extract as a standalone crate. The trait exposes only
//! what widgets actually need — the 6-color palette + a few derived
//! getters. Add fields cautiously; every new field couples the kit to a
//! specific theme shape.

use egui::Color32;

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
}

impl ComponentTheme for crate::chart_renderer::gpu::Theme {
    fn accent(&self) -> Color32 { self.accent }
    fn bull(&self) -> Color32 { self.bull }
    fn bear(&self) -> Color32 { self.bear }
    fn text(&self) -> Color32 { self.text }
    fn dim(&self) -> Color32 { self.dim }
    fn border(&self) -> Color32 { self.toolbar_border }
    fn border_variant(&self) -> Color32 { self.border_variant }
    fn warn(&self) -> Color32 { self.warn }
    fn bg(&self) -> Color32 { self.bg }
    fn surface(&self) -> Color32 { self.toolbar_bg }
    fn element_hover(&self) -> Color32 { self.element_hover }
    fn element_active(&self) -> Color32 { self.element_active }
    fn element_selected(&self) -> Color32 { self.element_selected }
    fn element_disabled(&self) -> Color32 { self.element_disabled }
    fn ghost_hover(&self) -> Color32 { self.ghost_hover }
    fn ghost_active(&self) -> Color32 { self.ghost_active }
    fn icon(&self) -> Color32 { self.icon }
    fn icon_muted(&self) -> Color32 { self.icon_muted }
    fn icon_disabled(&self) -> Color32 { self.icon_disabled }
    fn icon_accent(&self) -> Color32 { self.icon_accent }
}

// Blanket impl so callers can pass `&T` where T: ComponentTheme through
// `&dyn ComponentTheme` interchangeably without explicit coercion in
// generic contexts.
/// Read the active theme index stashed in egui memory by the render loop.
/// Falls back to 0 (Midnight) if nothing was stashed.
pub fn active_theme_idx(ctx: &egui::Context) -> usize {
    ctx.data(|d| d.get_temp::<usize>(egui::Id::new("apex_active_theme_idx")))
       .unwrap_or(0)
}

/// Convenience: returns &Theme for the active idx via THEMES array.
pub fn active_theme(ctx: &egui::Context) -> &'static crate::chart_renderer::gpu::Theme {
    let idx = active_theme_idx(ctx).min(crate::chart_renderer::gpu::THEMES.len() - 1);
    &crate::chart_renderer::gpu::THEMES[idx]
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
}
