//! `DialogHeader` — the canonical modal title bar with a close button.
//!
//! ## Why this lives here
//!
//! This is the ui_kit-owned home of what used to be
//! `chart_renderer::ui::components::headers_widget::DialogHeaderWithClose`.
//! `ui_kit::widgets::modal` needed that builder, which made the canonical
//! component layer depend on the legacy chart component layer
//! (`ui_kit -> chart_renderer::ui::components -> chart_renderer::ui::panels::kit`)
//! and blocked retiring either legacy system.
//!
//! The dependency is now inverted: the implementation lives here, expressed
//! purely over [`ComponentTheme`] (the portability bridge), and the legacy
//! `DialogHeaderWithClose` is a thin delegating wrapper around this type.
//!
//! ## Relationship to [`Header`](super::header::Header)
//!
//! `DialogHeader` is a *convenience builder*, not a second implementation: it
//! renders through `Header::dialog(title).closable(true)`. It exists because
//! the dialog case has a fixed shape (title + close, boolean result) and
//! callers want `-> bool` rather than a `HeaderResponse`, plus an ambient-theme
//! `show(ui)` for the many call sites that have no theme handle in scope.
//!
//! ```ignore
//! if DialogHeader::new("Settings").show(ui) { open = false; }
//! if DialogHeader::new("Settings").show_with(ui, theme) { open = false; }
//! ```

use egui::{Color32, Ui};

use super::theme::{active_theme, ComponentTheme};
use super::Header;

/// Builder for a dialog header bar with close button. `show` returns `true`
/// when the close button was clicked.
#[must_use = "DialogHeader must be rendered with `.show(ui)`"]
pub struct DialogHeader<'a> {
    title: &'a str,
    /// Legacy colour override knob. `None` = resolve from the ambient theme at
    /// render time. `Header::dialog` derives its own chrome from the theme, so
    /// this is resolved-but-unread — kept so the value can never be a
    /// construction-time literal that survives into a light palette.
    dim: Option<Color32>,
}

impl<'a> DialogHeader<'a> {
    pub fn new(title: &'a str) -> Self {
        Self { title, dim: None }
    }

    /// Explicit "dim" colour override (see the field note above).
    pub fn dim(mut self, c: Color32) -> Self {
        self.dim = Some(c);
        self
    }

    /// Pull the dim colour from a theme. Any `ComponentTheme` works — the
    /// chart-app's `gpu::Theme` and the standalone `PortableTheme` both do.
    pub fn theme(self, t: &dyn ComponentTheme) -> Self {
        self.dim(t.dim())
    }

    /// Render against an explicit theme. Returns `true` if close was clicked.
    pub fn show_with(self, ui: &mut Ui, theme: &dyn ComponentTheme) -> bool {
        let sctx = super::ctx::StyleCtx::from_ui(theme, ui);
        self.show_ctx(ui, &sctx)
    }

    /// [`StyleCtx`](super::ctx::StyleCtx) entry point — see `show_with`.
    ///
    /// Named `show_with` rather than `show` for historical reasons; the ctx
    /// entry point uses the canonical name so callers do not have to remember
    /// which widgets are irregular.
    pub fn show_ctx(self, ui: &mut Ui, sctx: &super::ctx::StyleCtx<'_>) -> bool {
        let theme = sctx.theme();
        // Resolved at render time; `Header::dialog` colors itself from `theme`.
        let _dim = self.dim.unwrap_or_else(|| theme.dim());
        Header::dialog(self.title)
            .closable(true)
            .show(ui, theme)
            .close_clicked
    }

    /// Render against the ambient theme stashed by the host this frame.
    /// Returns `true` if the close button was clicked.
    pub fn show(self, ui: &mut Ui) -> bool {
        let theme = active_theme(ui.ctx());
        self.show_with(ui, &theme)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui_kit::widgets::theme::PortableTheme;

    /// `.dim(c)` stores the override.
    #[test]
    fn dim_override_is_stored() {
        let h = DialogHeader::new("Settings").dim(Color32::from_rgb(1, 2, 3));
        assert_eq!(h.dim, Some(Color32::from_rgb(1, 2, 3)));
    }

    /// `.theme(t)` sources the dim colour from the theme — via the
    /// `ComponentTheme` trait, NOT a concrete chart `Theme`.
    #[test]
    fn theme_sources_dim_through_component_theme() {
        let t = PortableTheme::dark();
        let h = DialogHeader::new("Settings").theme(&t);
        assert_eq!(h.dim, Some(t.dim()));
    }

    /// Unset dim stays `None` so it resolves against the LIVE theme at render
    /// time (a construction-time literal would render off-palette on the light
    /// themes).
    #[test]
    fn dim_defaults_to_none_for_render_time_resolution() {
        let h = DialogHeader::new("Settings");
        assert!(h.dim.is_none(), "dim must resolve at render time, not construction");
    }
}
