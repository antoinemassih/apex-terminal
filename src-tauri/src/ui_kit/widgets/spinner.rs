//! Spinner — small rotating indicator. Just an alias for circular
//! indeterminate progress with sensible defaults.
//!
//! API:
//!   ui.add(Spinner::new());
//!   ui.add(Spinner::new().size(Size::Sm));

use egui::{Response, Ui, Widget};

use super::progress::Progress;
use super::theme::ComponentTheme;
use super::tokens::Size;

#[must_use = "Spinner does nothing until `.show(ui, theme)` or `ui.add(spinner)` is called"]
pub struct Spinner {
    size: Size,
}

impl Spinner {
    pub fn new() -> Self { Self { size: Size::Sm } }
    pub fn size(mut self, s: Size) -> Self { self.size = s; self }

    pub fn show(self, ui: &mut Ui, theme: &dyn ComponentTheme) -> Response {
        // Build the ctx from the UI so it carries the AMBIENT RecipeSet.
        // `StyleCtx::from_theme` would hand this widget an empty set — see
        // `ctx.rs` for why that shim must never be used inside a `show`.
        let sctx = super::ctx::StyleCtx::from_ui(theme, ui);
        self.show_ctx(ui, &sctx)
    }

    /// [`StyleCtx`](super::ctx::StyleCtx) entry point.
    ///
    /// Callers that need per-call-site token overrides or an explicit
    /// `RecipeSet` construct a `StyleCtx` and call this directly; `show`
    /// delegates here with the ambient one.
    pub fn show_ctx(self, ui: &mut Ui, sctx: &super::ctx::StyleCtx<'_>) -> Response {
        let theme = sctx.theme();
        Progress::circular_indeterminate().size(self.size).show(ui, theme)
    }
}

impl Default for Spinner {
    fn default() -> Self { Self::new() }
}

impl Widget for Spinner {
    fn ui(self, ui: &mut Ui) -> Response {
        let theme = super::theme::active_theme(ui.ctx());
        self.show(ui, &theme)
    }
}
