//! SectionHeader — collapsible section header used in both the stocks and
//! options areas of `watchlist_panel.rs`.
//!
//! The exact same pattern (chevron + title + optional item count + right-side
//! delete button) appeared twice. This widget extracts the shared paint code.
//!
//! # Example
//! ```ignore
//! let resp = SectionHeader::new(&sec_title)
//!     .collapsed(sec_collapsed)
//!     .item_count(sec_item_count)
//!     .theme(t)
//!     .show(ui);
//! if resp.chevron_clicked { toggle_collapse = Some(si); }
//! if resp.delete_clicked  { remove_section  = Some(si); }
//! ```

#![allow(dead_code)]

use egui::{Response, Ui};
use crate::chart_renderer::ui::style::*;
use crate::chart_renderer::gpu::Theme;
use crate::chart_renderer::ui::widgets::text::MonospaceCode;
use crate::ui_kit::icons::Icon;
use crate::ui_kit::widgets::{Button, tokens::{Variant, Size}};

/// Return value from [`SectionHeader::show`].
pub struct SectionHeaderResponse {
    /// The full horizontal row response (use for context menus).
    pub response: Response,
    pub chevron_clicked: bool,
    pub delete_clicked: bool,
}

#[must_use = "SectionHeader must be shown with `.show(ui)` to render"]
pub struct SectionHeader<'a> {
    title: &'a str,
    collapsed: bool,
    item_count: usize,
    show_delete_when_empty: bool,
    dim: egui::Color32,
}

impl<'a> SectionHeader<'a> {
    pub fn new(title: &'a str) -> Self {
        Self {
            title,
            collapsed: false,
            item_count: 0,
            show_delete_when_empty: true,
            dim: crate::chart_renderer::gpu::THEMES[0].dim,
        }
    }

    pub fn collapsed(mut self, v: bool) -> Self { self.collapsed = v; self }
    pub fn item_count(mut self, n: usize) -> Self { self.item_count = n; self }
    pub fn show_delete_when_empty(mut self, v: bool) -> Self { self.show_delete_when_empty = v; self }

    pub fn theme(mut self, t: &Theme) -> Self {
        self.dim = t.dim;
        self
    }

    pub fn show(self, ui: &mut Ui) -> SectionHeaderResponse {
        let mut chevron_clicked = false;
        let mut delete_clicked  = false;

        let inner = ui.horizontal(|ui| {
            ui.set_min_height(20.0);

            let chevron = if self.collapsed { Icon::CARET_RIGHT } else { Icon::CARET_DOWN };
            if ui.add(
                Button::icon(chevron)
                    .variant(Variant::Ghost)
                    .size(Size::Sm)
                    .glyph_color(self.dim.gamma_multiply(0.6))
                    .frameless(true),
            ).clicked() {
                chevron_clicked = true;
            }

            ui.add(
                MonospaceCode::new(self.title)
                    .size_px(font_sm_tight())
                    .strong(true)
                    .color(self.dim)
                    .gamma(0.6),
            );

            if self.collapsed {
                ui.add(
                    MonospaceCode::new(&format!("({})", self.item_count))
                        .size_px(8.0)
                        .color(self.dim)
                        .gamma(0.3),
                );
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if self.show_delete_when_empty && self.item_count == 0 {
                    if ui.add(
                        Button::icon(Icon::X)
                            .variant(Variant::Ghost)
                            .size(Size::Sm)
                            .glyph_color(self.dim.gamma_multiply(0.3))
                            .frameless(true),
                    ).clicked() {
                        delete_clicked = true;
                    }
                }
            });
        });

        SectionHeaderResponse {
            response: inner.response,
            chevron_clicked,
            delete_clicked,
        }
    }
}
