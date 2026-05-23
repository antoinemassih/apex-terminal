//! PanelLoading — single consistent loading affordance for panel content.
//!
//! Replaces the scattered Spinner / Progress / Skeleton / plain "Loading..."
//! text patterns that each panel currently picks at random.
//!
//! ```ignore
//! PanelLoading::new().show(ui, t);
//! PanelLoading::new().reason("Fetching chain").show(ui, t);
//! ```
//!
//! Visual spec (locked):
//! - A small row: animated `Spinner` (from `ui_kit::Spinner`) + optional
//!   `reason` text in `mono_xs` muted.
//! - Centered horizontally when there is no `reason`.
//! - Left-aligned when a `reason` is set (so multiple stacked loaders read as
//!   a list).
//! - Small — never dominates the panel. Designed to sit inside a section body.
//!
//! When to use:
//! - Inside a `PanelSection` body while data is being fetched.
//! - Inside a `PanelCard` while the card is hydrating.
//!
//! When NOT to use:
//! - Full-screen loading — use a dedicated splash.
//! - Progress with a known % — use `Progress` directly.
//! - Skeleton placeholders for known-shape content — use `Skeleton`.
//!
//! Sister widgets: `PanelEmpty`, `Spinner`, `Progress`, `Skeleton`.

use egui::{Align, Layout, RichText, Ui};

use super::spinner::Spinner;
use super::theme::active_theme;
use super::tokens::Size;
use crate::ui_kit::tokens::{color_muted, font_xs, gap_sm};
use crate::ui_kit::widgets::theme::Theme;

#[must_use = "PanelLoading must be rendered with `.show(...)`"]
#[derive(Default)]
pub struct PanelLoading<'a> {
    reason: Option<&'a str>,
}

impl<'a> PanelLoading<'a> {
    pub fn new() -> Self {
        Self { reason: None }
    }

    pub fn reason(mut self, r: &'a str) -> Self {
        self.reason = Some(r);
        self
    }

    pub fn show(self, ui: &mut Ui, t: &Theme) {
        // Use the active component theme for Spinner — Spinner is a
        // ui_kit-native widget that wants `&dyn ComponentTheme`.
        let comp_theme = active_theme(ui.ctx());
        let layout = if self.reason.is_some() {
            Layout::left_to_right(Align::Center)
        } else {
            Layout::left_to_right(Align::Center).with_main_align(Align::Center)
        };
        ui.with_layout(layout, |ui| {
            ui.spacing_mut().item_spacing.x = gap_sm();
            Spinner::new().size(Size::Sm).show(ui, &comp_theme);
            if let Some(r) = self.reason {
                ui.label(
                    RichText::new(r)
                        .monospace()
                        .size(font_xs())
                        .color(color_muted(t.dim)),
                );
            }
        });
    }
}
