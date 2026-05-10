//! FilterPill — interactive pill toggle used in the watchlist filter strip.
//!
//! Wraps the bespoke `ChromeBtn`-as-pill pattern that appeared 2+ times in
//! `watchlist_panel.rs` (stock preset loop + custom filter area).
//!
//! # Example
//! ```ignore
//! if ui.add(FilterPill::new("All").active(preset == "All").theme(t)).clicked() {
//!     preset = "All".into();
//! }
//! ```

#![allow(dead_code)]

use egui::{Response, Widget};
use crate::chart_renderer::gpu::Theme;
use crate::ui_kit::widgets::Button;
use crate::ui_kit::widgets::tokens::{Variant, Size};

#[inline(always)]
fn ft() -> &'static crate::chart_renderer::gpu::Theme { &crate::chart_renderer::gpu::THEMES[0] }

#[must_use = "FilterPill must be added with `ui.add(...)` to render"]
pub struct FilterPill<'a> {
    label: &'a str,
    active: bool,
    accent: egui::Color32,
    dim:    egui::Color32,
}

impl<'a> FilterPill<'a> {
    pub fn new(label: &'a str) -> Self {
        Self {
            label,
            active: false,
            accent: ft().accent,
            dim:    ft().dim,
        }
    }

    pub fn active(mut self, v: bool) -> Self { self.active = v; self }

    pub fn theme(mut self, t: &Theme) -> Self {
        self.accent = t.accent;
        self.dim    = t.dim;
        self
    }
}

impl<'a> Widget for FilterPill<'a> {
    fn ui(self, ui: &mut egui::Ui) -> Response {
        // Variant::Chip handles the active/inactive fg+fill+corner_radius dance.
        ui.add(
            Button::new(self.label)
                .variant(Variant::Chip)
                .size(Size::Xs)
                .active(self.active),
        )
    }
}
