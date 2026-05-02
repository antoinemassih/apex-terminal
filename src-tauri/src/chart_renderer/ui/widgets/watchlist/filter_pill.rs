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

use egui::{Response, RichText, Widget};
use crate::chart_renderer::ui::style::*;
use crate::chart_renderer::gpu::Theme;
use super::super::buttons::ChromeBtn;

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
            accent: egui::Color32::from_rgb(120, 140, 220),
            dim:    egui::Color32::from_rgb(120, 120, 130),
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
        let col = if self.active { self.accent } else { self.dim };
        let bg  = if self.active {
            color_alpha(self.accent, alpha_subtle())
        } else {
            egui::Color32::TRANSPARENT
        };
        ui.add(
            ChromeBtn::new(RichText::new(self.label).monospace().size(8.0).color(col))
                .fill(bg)
                .corner_radius(r_md_cr())
                .min_size(egui::vec2(0.0, 16.0)),
        )
    }
}
