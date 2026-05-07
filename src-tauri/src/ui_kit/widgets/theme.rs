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
    fn warn(&self) -> Color32;

    // Surface tokens (background fills).
    fn bg(&self) -> Color32;
    fn surface(&self) -> Color32; // raised surface, e.g. toolbar_bg
}

impl ComponentTheme for crate::chart_renderer::gpu::Theme {
    fn accent(&self) -> Color32 { self.accent }
    fn bull(&self) -> Color32 { self.bull }
    fn bear(&self) -> Color32 { self.bear }
    fn text(&self) -> Color32 { self.text }
    fn dim(&self) -> Color32 { self.dim }
    fn border(&self) -> Color32 { self.toolbar_border }
    fn warn(&self) -> Color32 { self.warn }
    fn bg(&self) -> Color32 { self.bg }
    fn surface(&self) -> Color32 { self.toolbar_bg }
}

// Blanket impl so callers can pass `&T` where T: ComponentTheme through
// `&dyn ComponentTheme` interchangeably without explicit coercion in
// generic contexts.
impl<T: ComponentTheme + ?Sized> ComponentTheme for &T {
    fn accent(&self) -> Color32 { (**self).accent() }
    fn bull(&self) -> Color32 { (**self).bull() }
    fn bear(&self) -> Color32 { (**self).bear() }
    fn text(&self) -> Color32 { (**self).text() }
    fn dim(&self) -> Color32 { (**self).dim() }
    fn border(&self) -> Color32 { (**self).border() }
    fn warn(&self) -> Color32 { (**self).warn() }
    fn bg(&self) -> Color32 { (**self).bg() }
    fn surface(&self) -> Color32 { (**self).surface() }
}
