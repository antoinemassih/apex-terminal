//! PanelError — standardised error state for panel sections.
//!
//! Completes the empty/loading/error trio alongside [`PanelEmpty`] and
//! [`PanelLoading`]. Renders a bear-tinted icon above a message line, with an
//! optional retry button below. Matches the vertical cadence of its siblings.
//!
//! ```ignore
//! PanelError::new("Failed to fetch data")
//!     .hint("Check your connection and try again")
//!     .retry_action(|ui, t| {
//!         if Button::new("Retry").variant(Variant::Ghost).size(Size::Sm).show(ui, t).clicked() {
//!             refetch();
//!         }
//!     })
//!     .show(ui, t);
//! ```
//!
//! Visual spec (mirrors `PanelEmpty`):
//! - Vertical layout, center-aligned, 64 px minimum height.
//! - Icon: `font_xl` (22 px) in `bear` color — uses `Icon::WARNING` (`⚠`).
//! - Message: `mono_sm` in `t.dim`.
//! - Hint (optional, second line): `mono_xs` in `color_muted(t.dim)`.
//! - Retry slot (optional): anything the caller paints. Centred below the
//!   hint/message with `gap_sm` separation.
//!
//! When to use:
//! - Inside a `PanelSection` body when a data fetch fails.
//! - Inside a `PanelCard` when the card cannot hydrate.
//!
//! When NOT to use:
//! - Non-fatal warnings — use [`Alert`] with `AlertVariant::Warning`.
//! - Empty data that is expected — use [`PanelEmpty`].
//! - Loading states — use [`PanelLoading`].
//!
//! Sister widgets: [`PanelEmpty`], [`PanelLoading`], [`Alert`].

use egui::{Align, FontId, Layout, RichText, Ui};

use crate::ui_kit::tokens::{color_muted, font_sm, font_xl, font_xs, gap_md, gap_sm, gap_xs};
use crate::ui_kit::widgets::theme::Theme;
use crate::ui_kit::icons::Icon;

#[must_use = "PanelError must be rendered with `.show(...)`"]
pub struct PanelError<'a> {
    message: &'a str,
    hint: Option<&'a str>,
    retry_action: Option<Box<dyn FnOnce(&mut Ui, &Theme) + 'a>>,
    min_height: f32,
}

impl<'a> PanelError<'a> {
    pub fn new(message: &'a str) -> Self {
        Self {
            message,
            hint: None,
            retry_action: None,
            min_height: 64.0,
        }
    }

    /// Optional second-line hint text below the message.
    pub fn hint(mut self, h: &'a str) -> Self {
        self.hint = Some(h);
        self
    }

    /// Optional retry slot. The closure is called below the message (and hint,
    /// if set). Caller is responsible for painting the button and wiring the
    /// click. Receives `(&mut Ui, &Theme)` so it can use `ui_kit::Button`.
    pub fn retry_action(mut self, f: impl FnOnce(&mut Ui, &Theme) + 'a) -> Self {
        self.retry_action = Some(Box::new(f));
        self
    }

    /// Override the minimum vertical height (default 64 px).
    pub fn min_height(mut self, h: f32) -> Self {
        self.min_height = h;
        self
    }

    pub fn show(self, ui: &mut Ui, t: &Theme) {
        let avail_w = ui.available_width();
        ui.allocate_ui_with_layout(
            egui::Vec2::new(avail_w, self.min_height),
            Layout::top_down(Align::Center),
            |ui| {
                ui.add_space(gap_md());

                // Bear-tinted warning icon (Shield Warning from Phosphor Bold set).
                ui.label(
                    RichText::new(Icon::SHIELD_WARNING)
                        .font(FontId::proportional(font_xl()))
                        .color(t.bear),
                );
                ui.add_space(gap_xs());

                // Error message.
                ui.label(
                    RichText::new(self.message)
                        .monospace()
                        .size(font_sm())
                        .color(t.dim),
                );

                // Optional hint.
                if let Some(h) = self.hint {
                    ui.add_space(gap_xs());
                    ui.label(
                        RichText::new(h)
                            .monospace()
                            .size(font_xs())
                            .color(color_muted(t.dim)),
                    );
                }

                // Optional retry slot.
                if let Some(action) = self.retry_action {
                    ui.add_space(gap_sm());
                    action(ui, t);
                }

                ui.add_space(gap_md());
            },
        );
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Builder stores message and optional hint.
    #[test]
    fn builder_message_and_hint() {
        let e = PanelError::new("Connection failed")
            .hint("Check your network")
            .min_height(80.0);
        assert_eq!(e.message, "Connection failed");
        assert_eq!(e.hint, Some("Check your network"));
        assert!((e.min_height - 80.0).abs() < f32::EPSILON);
        // retry_action not set
        assert!(e.retry_action.is_none());
    }

    /// No hint by default.
    #[test]
    fn no_hint_by_default() {
        let e = PanelError::new("Error");
        assert!(e.hint.is_none());
    }

    /// retry_action can be set via closure.
    #[test]
    fn retry_action_is_set() {
        let mut called = false;
        {
            let e = PanelError::new("Err").retry_action(|_ui, _t| {
                // would call a refetch; just mark for test
            });
            assert!(e.retry_action.is_some());
        }
        // Closure not called here (no egui context in unit tests) — just
        // verify the Some() path was taken.
        let _ = called; // suppress unused warning
    }

    /// Default min_height is 64.
    #[test]
    fn default_min_height() {
        let e = PanelError::new("Oops");
        assert!((e.min_height - 64.0).abs() < f32::EPSILON);
    }
}
