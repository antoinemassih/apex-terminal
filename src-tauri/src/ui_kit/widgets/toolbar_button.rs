//! ToolBarButton — small framed button for per-pane top-left tool strips.
//!
//! Same visual treatment as the chart-pane HEADER buttons (ORDER / DOM / OPTIONS /
//! OV in the right cluster): subtle bg fill, hairline border, hover/active fades,
//! and an accent-coloured foreground when toggled on. Thin wrapper around the
//! existing `Button` widget so call sites stay tight:
//!
//! ```ignore
//! if ToolBarButton::icon(Icon::MAGNET).active(chart.magnet).show(ui, t).clicked() {
//!     // toggle …
//! }
//! ```
//!
//! Variants:
//! - `new(label)` — text-only button (e.g. `"5M"`, `"STD"`, `"1D"`).
//! - `icon(glyph)` — Phosphor PUA icon-only button.
//!
//! When to use a different widget:
//! - For a dropdown trigger (menu opens on click), use `Button::menu(label)` /
//!   `KitButton::menu(...)` — those route through `ui.menu_button` and need
//!   different rendering.

use egui::{Response, Ui};
use super::Button;
use super::tokens::Size;
use super::theme::ComponentTheme;
use crate::ui_kit::sx::{palette_ct, Tone};

#[must_use = "ToolBarButton must be shown with `.show(ui, theme)` to render"]
pub struct ToolBarButton<'a> {
    inner: Button<'a>,
    active: bool,
}

impl<'a> ToolBarButton<'a> {
    /// Text-label toolbar button.
    pub fn new(label: &'a str) -> Self {
        Self {
            inner: Button::new(label).size(Size::Sm).status(true),
            active: false,
        }
    }

    /// Icon-only toolbar button. `glyph` is a Phosphor PUA string from `Icon::`.
    pub fn icon(glyph: &'a str) -> Self {
        Self {
            inner: Button::icon(glyph).size(Size::Sm).status(true),
            active: false,
        }
    }

    /// Mark the button as active (toggled on). Foreground tints to the theme
    /// accent when active — matches `toolbar_btn`'s legacy behaviour.
    pub fn active(mut self, v: bool) -> Self {
        self.inner = self.inner.active(v);
        self.active = v;
        self
    }

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
        let mut btn = self.inner;
        if self.active {
            btn = btn.fg(palette_ct(theme).base(Tone::Accent));
        }
        let resp = btn.show(ui, theme);
        crate::ui_kit::cursor::clickable(ui, &resp);
        resp
    }
}

impl<'a> egui::Widget for ToolBarButton<'a> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let theme = super::theme::active_theme(ui.ctx());
        self.show(ui, &theme)
    }
}
