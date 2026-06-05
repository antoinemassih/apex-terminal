//! ToggleGroup — row of standalone buttons, exactly one selected.
//!
//! Like SegmentedControl but each button renders as an individual
//! `Variant::Chip` pill rather than a connected container. Use when the
//! visual weight of a SegmentedControl would be too heavy (e.g., a density
//! preset N / M / F inline with other controls).
//!
//! ```ignore
//! ToggleGroup::new(&mut value, &[(0u8, "N"), (1u8, "M"), (2u8, "F")])
//!     .size(Size::Xs)
//!     .show(ui, theme);
//! ```

use egui::{Response, Ui};
use super::theme::ComponentTheme;
use crate::ui_kit::sx::{palette_ct, Tone};
use super::tokens::{Size, Variant};
use super::button::Button;

/// Row of `Variant::Chip` buttons — exactly one active at a time.
#[must_use = "ToggleGroup does nothing until `.show(ui, theme)` is called"]
pub struct ToggleGroup<'a, T: Copy + PartialEq + 'a> {
    selected: &'a mut T,
    items: &'a [(T, &'a str)],
    size: Size,
    disabled: bool,
}

impl<'a, T: Copy + PartialEq + 'a> ToggleGroup<'a, T> {
    /// Create from a `(value, label)` slice.
    pub fn new(selected: &'a mut T, items: &'a [(T, &'a str)]) -> Self {
        Self { selected, items, size: Size::Sm, disabled: false }
    }

    /// Override the size tier (default: `Size::Sm`).
    pub fn size(mut self, s: Size) -> Self { self.size = s; self }
    /// Disables all buttons in the group.
    pub fn disabled(mut self, d: bool) -> Self { self.disabled = d; self }

    /// Render horizontally and return a `Response` whose `.changed()` is
    /// `true` when the active selection changed.
    pub fn show(self, ui: &mut Ui, theme: &dyn ComponentTheme) -> Response {
        let mut changed = false;
        let mut outer: Option<Response> = None;

        ui.horizontal(|ui| {
            for (val, label) in self.items {
                let is_active = *self.selected == *val;
                let resp = Button::new(*label)
                    .variant(Variant::Chip)
                    .size(self.size)
                    .active(is_active)
                    .disabled(self.disabled)
                    .show(ui, theme);

                if resp.clicked() && !is_active && !self.disabled {
                    *self.selected = *val;
                    changed = true;
                }

                outer = Some(match outer.take() {
                    Some(acc) => acc.union(resp),
                    None => resp,
                });
            }
        }).inner;

        let mut resp = outer.unwrap_or_else(|| {
            ui.allocate_exact_size(egui::Vec2::ZERO, egui::Sense::hover()).1
        });

        if changed {
            resp.mark_changed();
        }
        resp
    }
}

impl<'a, T: Copy + PartialEq + 'a> egui::Widget for ToggleGroup<'a, T> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let theme = super::theme::active_theme(ui.ctx());
        self.show(ui, &theme)
    }
}
