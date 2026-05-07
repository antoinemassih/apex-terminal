//! Link — text-only button styled like a hyperlink.
//!
//! Reuses Button with Variant::Link, but wraps in a friendlier API
//! and adds optional external indicator (↗ icon).
//!
//! API:
//!   if ui.add(Link::new("Open ApexData docs").external(true)).clicked() {
//!       open_url(...);
//!   }

use egui::{Response, Ui, Widget};

use super::button::Button;
use super::theme::ComponentTheme;
use super::tokens::{Size, Variant};

#[must_use = "Link does nothing until `.show(ui, theme)` or `ui.add(link)` is called"]
pub struct Link<'a> {
    text: &'a str,
    external: bool,
    size: Size,
}

impl<'a> Link<'a> {
    pub fn new(text: &'a str) -> Self {
        Self { text, external: false, size: Size::Sm }
    }

    pub fn external(mut self, v: bool) -> Self { self.external = v; self }
    pub fn size(mut self, s: Size) -> Self { self.size = s; self }

    pub fn show(self, ui: &mut Ui, theme: &dyn ComponentTheme) -> Response {
        let mut btn = Button::new(self.text).variant(Variant::Link).size(self.size);
        if self.external {
            btn = btn.trailing_icon("\u{2197}"); // ↗
        }
        btn.show(ui, theme)
    }
}

impl<'a> Widget for Link<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        let theme = &crate::chart_renderer::gpu::THEMES[0];
        self.show(ui, theme)
    }
}
