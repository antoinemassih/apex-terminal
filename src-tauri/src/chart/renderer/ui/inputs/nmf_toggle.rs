//! NmfToggle — Near / Mid / Far toggle button group used in the options chain.
//!
//! The exact same 3-button inline pattern appeared for both the 0DTE and
//! far-DTE chains. This widget extracts it.
//!
//! # Example
//! ```ignore
//! NmfToggle::new(&mut watchlist.chain_0_nmf).theme(t).show(ui);
//! ```

#![allow(dead_code)]

use egui::Ui;
use crate::chart_renderer::ui::style::*;
use crate::chart_renderer::gpu::Theme;
use crate::ui_kit::widgets::Button;
use crate::ui_kit::widgets::tokens::{Variant, Size};

#[inline(always)]
fn ft() -> &'static crate::chart_renderer::gpu::Theme { &crate::chart_renderer::gpu::THEMES[0] }

pub struct NmfToggle<'a> {
    value: &'a mut u8,
    accent: egui::Color32,
    dim:    egui::Color32,
}

impl<'a> NmfToggle<'a> {
    pub fn new(value: &'a mut u8) -> Self {
        Self {
            value,
            accent: ft().accent,
            dim:    ft().dim,
        }
    }

    pub fn theme(mut self, t: &Theme) -> Self {
        self.accent = t.accent;
        self.dim    = t.dim;
        self
    }

    pub fn show(self, ui: &mut Ui) {
        for (lvl, label) in [(0u8, "N"), (1u8, "M"), (2u8, "F")] {
            let active = *self.value == lvl;
            let col = if active { self.accent } else { self.dim.gamma_multiply(0.4) };
            if ui.add(
                Button::new(label).variant(Variant::Chrome).size(Size::Xs).fg(col)
                    .fill(if active {
                        color_alpha(self.accent, alpha_subtle())
                    } else {
                        egui::Color32::TRANSPARENT
                    })
                    .min_size(egui::vec2(14.0, 14.0))
                    .corner_radius(crate::chart_renderer::ui::style::current().r_sm as f32),
            ).clicked() {
                *self.value = lvl;
            }
        }
    }
}
