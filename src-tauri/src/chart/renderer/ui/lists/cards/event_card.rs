//! `EventCard` — calendar event row.
//!
//! Wave 4.5d: migrated onto `CardShell`. Public API unchanged.

#![allow(dead_code, unused_imports)]

use egui::{Color32, Margin, RichText, Ui};
use super::super::super::style::*;
use crate::chart::renderer::ui::foundation::shell::CardShell;

type Theme = crate::chart_renderer::gpu::Theme;

fn ft() -> &'static Theme { &crate::chart_renderer::gpu::THEMES[0] }

#[must_use = "EventCard must be rendered with `.show(ui)`"]
pub struct EventCard<'a> {
    title: &'a str,
    when:  Option<&'a str>,
    venue: Option<&'a str>,
    note:  Option<&'a str>,
    theme: Option<&'a Theme>,
}

impl<'a> EventCard<'a> {
    pub fn new(title: &'a str) -> Self {
        Self { title, when: None, venue: None, note: None, theme: None }
    }
    pub fn when(mut self, w: &'a str)  -> Self { self.when = Some(w); self }
    pub fn venue(mut self, v: &'a str) -> Self { self.venue = Some(v); self }
    pub fn note(mut self, n: &'a str)  -> Self { self.note = Some(n); self }
    pub fn theme(mut self, t: &'a Theme) -> Self { self.theme = Some(t); self }

    pub fn show(self, ui: &mut Ui) {
        let dim  = self.theme.map(|t| t.dim).unwrap_or_else(|| ft().dim);
        let text = self.theme.map(|t| t.text).unwrap_or_else(|| ft().text);
        let theme = self.theme;
        let title = self.title;
        let when = self.when;
        let venue = self.venue;
        let note = self.note;
        CardShell::new_themeless()
            .theme(theme)
            .padding(Margin::same(gap_lg() as i8))
            .body(move |ui| {
                ui.horizontal(|ui| {
                    if let Some(w) = when {
                        ui.label(RichText::new(w).monospace().size(font_sm()).color(dim));
                    }
                    ui.label(RichText::new(title).monospace().size(font_md()).strong().color(text));
                });
                if let Some(v) = venue {
                    ui.label(RichText::new(v).monospace().size(font_xs()).color(dim));
                }
                if let Some(n) = note {
                    ui.add_space(gap_xs());
                    ui.label(RichText::new(n).monospace().size(font_sm()).color(text));
                }
            })
            .show(ui);
    }
}
