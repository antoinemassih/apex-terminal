//! SearchInput — magnifying-glass-prefixed text input with clear button.
//!
//! Convenience wrapper over `Input` for the very common search pattern.
//! Renders a leading search icon and a clear button when the field is
//! non-empty. Prefer this over constructing `Input` manually whenever the
//! intent is "filter / search".
//!
//! ```ignore
//! SearchInput::new(&mut query)
//!     .placeholder("Search symbols…")
//!     .full_width()
//!     .show(ui, theme);
//! ```

use egui::Ui;

use super::input::InputResponse;
use super::theme::ComponentTheme;
use super::tokens::Size;
use super::Input;
use crate::ui_kit::icons::Icon;

/// Builder for a search-styled text input. See module docs for usage.
#[must_use = "SearchInput does nothing until `.show(ui, theme)` is called"]
pub struct SearchInput<'a> {
    value: &'a mut String,
    placeholder: &'a str,
    full_width: bool,
    width: Option<f32>,
    size: Size,
    disabled: bool,
}

impl<'a> SearchInput<'a> {
    pub fn new(value: &'a mut String) -> Self {
        Self {
            value,
            placeholder: "Search…",
            full_width: false,
            width: None,
            // Lg (34px) — search bars are the most-touched input in the
            // app and deserve real breathing room. Callers that want a
            // compact form can opt in via .size(Size::Md) or Sm.
            size: Size::Lg,
            disabled: false,
        }
    }

    pub fn placeholder(mut self, p: &'a str) -> Self { self.placeholder = p; self }
    pub fn full_width(mut self) -> Self { self.full_width = true; self }
    pub fn width(mut self, w: f32) -> Self { self.width = Some(w); self }
    pub fn size(mut self, s: Size) -> Self { self.size = s; self }
    pub fn disabled(mut self, d: bool) -> Self { self.disabled = d; self }

    /// Show the widget. Returns the full [`InputResponse`] so callers can
    /// inspect `.clear_clicked` and `.submitted` in addition to `.response`.
    pub fn show(self, ui: &mut Ui, theme: &dyn ComponentTheme) -> InputResponse {
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
    pub fn show_ctx(self, ui: &mut Ui, sctx: &super::ctx::StyleCtx<'_>) -> InputResponse {
        let theme = sctx.theme();
        let mut input = Input::new(self.value)
            .placeholder(self.placeholder)
            .leading_icon(Icon::MAGNIFYING_GLASS)
            .clearable(true)
            .size(self.size)
            .disabled(self.disabled);
        if self.full_width {
            input = input.full_width();
        }
        if let Some(w) = self.width {
            input = input.min_width(w);
        }
        input.show(ui, theme)
    }
}
