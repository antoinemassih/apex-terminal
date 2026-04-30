//! `TradeCard` — entry / target / stop summary.
//!
//! Wave 4.5d: migrated onto `CardShell`. Public API unchanged.

#![allow(dead_code, unused_imports)]

use egui::{Color32, Margin, RichText, Ui};
use super::super::super::style::*;
use super::super::foundation::shell::CardShell;
use super::super::foundation::text_style::TextStyle;

type Theme = crate::chart_renderer::gpu::Theme;

#[must_use = "TradeCard must be rendered with `.show(ui)`"]
pub struct TradeCard<'a> {
    symbol: &'a str,
    entry:  Option<f64>,
    target: Option<f64>,
    stop:   Option<f64>,
    theme:  Option<&'a Theme>,
}

impl<'a> TradeCard<'a> {
    pub fn new(symbol: &'a str) -> Self {
        Self { symbol, entry: None, target: None, stop: None, theme: None }
    }
    pub fn entry(mut self, p: f64)  -> Self { self.entry = Some(p); self }
    pub fn target(mut self, p: f64) -> Self { self.target = Some(p); self }
    pub fn stop(mut self, p: f64)   -> Self { self.stop = Some(p); self }
    pub fn theme(mut self, t: &'a Theme) -> Self { self.theme = Some(t); self }

    pub fn show(self, ui: &mut Ui) {
        let bull = self.theme.map(|t| t.bull).unwrap_or(Color32::from_rgb(120, 200, 130));
        let bear = self.theme.map(|t| t.bear).unwrap_or(Color32::from_rgb(220, 100, 100));
        let dim  = self.theme.map(|t| t.dim).unwrap_or(Color32::from_rgb(140, 140, 150));
        let text = self.theme.map(|t| t.text).unwrap_or(Color32::from_rgb(210, 210, 220));
        let theme = self.theme;
        let entry = self.entry;
        let target = self.target;
        let stop = self.stop;
        CardShell::new_themeless()
            .theme(theme)
            .title(self.symbol)
            .title_style(TextStyle::Numeric)
            .padding(Margin::same(gap_lg() as i8))
            .body(move |ui| {
                ui.add_space(gap_xs());
                let row = |ui: &mut Ui, label: &str, value: Option<f64>, color: Color32| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(label).monospace().size(font_sm()).color(dim));
                        let s = value.map(|v| format!("{:.2}", v)).unwrap_or_else(|| "—".to_string());
                        ui.label(RichText::new(s).monospace().size(font_md()).strong().color(color));
                    });
                };
                row(ui, "Entry  ", entry,  text);
                row(ui, "Target ", target, bull);
                row(ui, "Stop   ", stop,   bear);
            })
            .show(ui);
    }
}
