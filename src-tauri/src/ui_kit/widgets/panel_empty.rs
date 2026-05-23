//! PanelEmpty — single consistent empty state for panel sections.
//!
//! Replaces the 5 different forms of "nothing here" copy currently in use
//! across panels (italic muted text, centered placeholder, grey box with
//! hint, ASCII glyph + line, etc.) with one builder.
//!
//! ```ignore
//! PanelEmpty::new("No alerts")
//!     .hint("Set one with Cmd+A")
//!     .glyph(Icon::BELL_SLASH)
//!     .show(ui, t);
//! ```
//!
//! Visual spec (locked):
//! - Vertical layout, center-aligned, ~60–80px minimum vertical space.
//! - Optional glyph: `font_xl` (22px) in `color_muted(t.dim())`, painted above.
//! - Title: `mono_sm` in `t.dim()`, italic-free.
//! - Hint (optional, second line): `mono_xs` in `color_muted(t.dim())`.
//!
//! When to use:
//! - Inside a `PanelSection` body when the underlying collection is empty.
//! - Inside a `PanelCard` body when the card has no content yet.
//!
//! When NOT to use:
//! - Errors — use `Alert` (with `AlertVariant::Error`) instead.
//! - Loading states — use `PanelLoading`.
//! - Full-panel "select something to see content" splashes — those deserve a
//!   richer hand-built state.
//!
//! Sister widgets: `PanelLoading`, `PanelSection`, `Alert`.

use egui::{Align, FontId, Layout, RichText, Ui};

use crate::ui_kit::tokens::{
    color_muted, font_sm, font_xl, font_xs, gap_md, gap_xs,
};
use crate::ui_kit::widgets::theme::ComponentTheme;

#[must_use = "PanelEmpty must be rendered with `.show(...)`"]
pub struct PanelEmpty<'a> {
    title: &'a str,
    hint: Option<&'a str>,
    glyph: Option<&'a str>,
    min_height: f32,
}

impl<'a> PanelEmpty<'a> {
    pub fn new(title: &'a str) -> Self {
        Self {
            title,
            hint: None,
            glyph: None,
            min_height: 64.0,
        }
    }

    pub fn hint(mut self, h: &'a str) -> Self {
        self.hint = Some(h);
        self
    }

    pub fn glyph(mut self, g: &'a str) -> Self {
        self.glyph = Some(g);
        self
    }

    pub fn min_height(mut self, h: f32) -> Self {
        self.min_height = h;
        self
    }

    pub fn show(self, ui: &mut Ui, t: &dyn ComponentTheme) {
        let avail_w = ui.available_width();
        let prev = ui.spacing().item_spacing;
        ui.allocate_ui_with_layout(
            egui::Vec2::new(avail_w, self.min_height),
            Layout::top_down(Align::Center),
            |ui| {
                ui.add_space(gap_md());
                if let Some(g) = self.glyph {
                    let col = color_muted(t.dim());
                    ui.label(
                        RichText::new(g)
                            .font(FontId::proportional(font_xl()))
                            .color(col),
                    );
                    ui.add_space(gap_xs());
                }
                ui.label(
                    RichText::new(self.title)
                        .monospace()
                        .size(font_sm())
                        .color(t.dim()),
                );
                if let Some(h) = self.hint {
                    ui.add_space(gap_xs());
                    ui.label(
                        RichText::new(h)
                            .monospace()
                            .size(font_xs())
                            .color(color_muted(t.dim())),
                    );
                }
                ui.add_space(gap_md());
            },
        );
        ui.spacing_mut().item_spacing = prev;
    }
}
