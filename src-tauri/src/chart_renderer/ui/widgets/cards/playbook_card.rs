//! `PlaybookCard` — rule name + body + tags.
//!
//! Visual reference: playbook_panel.rs, plays_panel.rs (item rows with a
//! rule heading, a description block, and a row of small tag pills).
//!
//! Wave 4.5d: migrated onto `CardShell`. Public API unchanged.

#![allow(dead_code, unused_imports)]

use egui::{Color32, Margin, RichText, Ui};
use super::super::super::style::*;
use super::super::foundation::shell::CardShell;
use super::super::foundation::text_style::TextStyle;
use super::super::pills::PillButton;

type Theme = crate::chart_renderer::gpu::Theme;

#[must_use = "PlaybookCard must be rendered with `.show(ui, |ui| {...})`"]
pub struct PlaybookCard<'a> {
    rule:  &'a str,
    body:  Option<&'a str>,
    tags:  Vec<&'a str>,
    theme: Option<&'a Theme>,
}

impl<'a> PlaybookCard<'a> {
    pub fn new(rule: &'a str) -> Self {
        Self { rule, body: None, tags: Vec::new(), theme: None }
    }
    pub fn body(mut self, b: &'a str) -> Self { self.body = Some(b); self }
    pub fn tag(mut self, t: &'a str)  -> Self { self.tags.push(t); self }
    pub fn tags(mut self, ts: impl IntoIterator<Item = &'a str>) -> Self {
        self.tags.extend(ts); self
    }
    pub fn theme(mut self, t: &'a Theme) -> Self { self.theme = Some(t); self }

    pub fn show(self, ui: &mut Ui) {
        let body_text = self.body;
        let tags = self.tags;
        let theme = self.theme;
        CardShell::new_themeless()
            .theme(theme)
            .title(self.rule)
            .title_style(TextStyle::Numeric)
            .padding(Margin::same(gap_lg() as i8))
            .body(move |ui| {
                if body_text.is_some() || !tags.is_empty() {
                    ui.add_space(gap_xs());
                }
                if let Some(b) = body_text {
                    ui.label(RichText::new(b).monospace().size(font_sm()));
                }
                if !tags.is_empty() {
                    ui.add_space(gap_xs());
                    ui.horizontal_wrapped(|ui| {
                        for tag in &tags {
                            let mut pill = PillButton::new(tag);
                            if let Some(t) = theme { pill = pill.theme(t); }
                            ui.add(pill);
                        }
                    });
                }
            })
            .show(ui);
    }
}
