//! `MetricCard` — label + big value + delta.
//!
//! Wave 4.5d: migrated onto `CardShell`. Public API unchanged.

#![allow(dead_code, unused_imports)]

use egui::{Color32, Margin, RichText, Ui};
use super::super::super::style::*;
use super::super::foundation::shell::CardShell;

type Theme = crate::chart_renderer::gpu::Theme;

#[must_use = "MetricCard must be rendered with `.show(ui)`"]
pub struct MetricCard<'a> {
    label: &'a str,
    value: String,
    delta: Option<f32>,
    theme: Option<&'a Theme>,
}

impl<'a> MetricCard<'a> {
    pub fn new(label: &'a str, value: impl Into<String>) -> Self {
        Self { label, value: value.into(), delta: None, theme: None }
    }
    pub fn delta(mut self, d: f32) -> Self { self.delta = Some(d); self }
    pub fn theme(mut self, t: &'a Theme) -> Self { self.theme = Some(t); self }

    pub fn show(self, ui: &mut Ui) {
        let bull = self.theme.map(|t| t.bull).unwrap_or(Color32::from_rgb(120, 200, 130));
        let bear = self.theme.map(|t| t.bear).unwrap_or(Color32::from_rgb(220, 100, 100));
        let dim  = self.theme.map(|t| t.dim).unwrap_or(Color32::from_rgb(140, 140, 150));
        let text = self.theme.map(|t| t.text).unwrap_or(Color32::from_rgb(210, 210, 220));
        let theme = self.theme;
        let label = self.label;
        let value = self.value;
        let delta = self.delta;
        CardShell::new_themeless()
            .theme(theme)
            .padding(Margin::same(gap_lg() as i8))
            .body(move |ui| {
                ui.label(RichText::new(label).monospace().size(font_xs()).color(dim));
                ui.label(RichText::new(&value).monospace().size(font_xl()).strong().color(text));
                if let Some(d) = delta {
                    let c = if d >= 0.0 { bull } else { bear };
                    ui.label(RichText::new(format!("{:+.2}", d))
                        .monospace().size(font_sm()).color(c));
                }
            })
            .show(ui);
    }
}
