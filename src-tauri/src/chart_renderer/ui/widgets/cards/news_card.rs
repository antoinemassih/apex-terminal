//! `NewsCard` — headline + source + timestamp + body excerpt.
//!
//! Wave 4.5d: migrated onto `CardShell`. Public API unchanged.

#![allow(dead_code, unused_imports)]

use egui::{Color32, Margin, RichText, Ui};
use super::super::super::style::*;
use super::super::foundation::shell::CardShell;

type Theme = crate::chart_renderer::gpu::Theme;

fn ft() -> &'static Theme { &crate::chart_renderer::gpu::THEMES[0] }

#[must_use = "NewsCard must be rendered with `.show(ui)`"]
pub struct NewsCard<'a> {
    headline:  &'a str,
    source:    Option<&'a str>,
    timestamp: Option<&'a str>,
    excerpt:   Option<&'a str>,
    theme:     Option<&'a Theme>,
}

impl<'a> NewsCard<'a> {
    pub fn new(headline: &'a str) -> Self {
        Self { headline, source: None, timestamp: None, excerpt: None, theme: None }
    }
    pub fn source(mut self, s: &'a str)    -> Self { self.source = Some(s); self }
    pub fn timestamp(mut self, t: &'a str) -> Self { self.timestamp = Some(t); self }
    pub fn excerpt(mut self, e: &'a str)   -> Self { self.excerpt = Some(e); self }
    pub fn theme(mut self, t: &'a Theme)   -> Self { self.theme = Some(t); self }

    pub fn show(self, ui: &mut Ui) {
        let dim = self.theme.map(|t| t.dim).unwrap_or_else(|| ft().dim);
        let text = self.theme.map(|t| t.text).unwrap_or_else(|| ft().text);
        let theme = self.theme;
        let headline = self.headline;
        let source = self.source;
        let ts = self.timestamp;
        let excerpt = self.excerpt;
        CardShell::new_themeless()
            .theme(theme)
            .padding(Margin::same(gap_lg() as i8))
            .body(move |ui| {
                ui.label(RichText::new(headline)
                    .monospace().size(font_md()).strong().color(text));
                if source.is_some() || ts.is_some() {
                    ui.horizontal(|ui| {
                        if let Some(s) = source {
                            ui.label(RichText::new(s).monospace().size(font_xs()).color(dim));
                        }
                        if source.is_some() && ts.is_some() {
                            ui.label(RichText::new("·").monospace().size(font_xs()).color(dim));
                        }
                        if let Some(t) = ts {
                            ui.label(RichText::new(t).monospace().size(font_xs()).color(dim));
                        }
                    });
                }
                if let Some(e) = excerpt {
                    ui.add_space(gap_xs());
                    ui.label(RichText::new(e).monospace().size(font_sm()).color(text));
                }
            })
            .show(ui);
    }
}
