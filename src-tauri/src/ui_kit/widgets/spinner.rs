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
        Progress::circular_indeterminate().size(self.size).show(ui, theme)
    }
}

impl Default for Spinner {
    fn default() -> Self { Self::new() }
}

impl Widget for Spinner {
    fn ui(self, ui: &mut Ui) -> Response {
        let theme = &crate::chart_renderer::gpu::THEMES[0];
        self.show(ui, theme)
    }
}
