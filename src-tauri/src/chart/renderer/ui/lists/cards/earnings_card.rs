//! `EarningsCard` — earnings calendar event.
//!
//! Wave 4.5d: migrated onto `CardShell`. Public API unchanged.

#![allow(dead_code, unused_imports)]

use egui::{Color32, Margin, RichText, Ui};
use super::super::super::style::*;
use crate::chart::renderer::ui::foundation::shell::CardShell;
use crate::chart::renderer::ui::foundation::text_style::TextStyle;

type Theme = crate::chart_renderer::gpu::Theme;

fn ft() -> &'static Theme { &crate::chart_renderer::gpu::THEMES[0] }

#[must_use = "EarningsCard must be rendered with `.show(ui)`"]
pub struct EarningsCard<'a> {
    symbol:    &'a str,
    when:      Option<&'a str>,   // BMO / AMC / time
    est_eps:   Option<f64>,
    act_eps:   Option<f64>,
    surprise:  Option<f32>,        // percent
    theme:     Option<&'a Theme>,
}

impl<'a> EarningsCard<'a> {
    pub fn new(symbol: &'a str) -> Self {
        Self { symbol, when: None, est_eps: None, act_eps: None, surprise: None, theme: None }
    }
    pub fn when(mut self, w: &'a str)    -> Self { self.when = Some(w); self }
    pub fn estimate(mut self, v: f64)    -> Self { self.est_eps = Some(v); self }
    pub fn actual(mut self, v: f64)      -> Self { self.act_eps = Some(v); self }
    pub fn surprise(mut self, pct: f32)  -> Self { self.surprise = Some(pct); self }
    pub fn theme(mut self, t: &'a Theme) -> Self { self.theme = Some(t); self }

    pub fn show(self, ui: &mut Ui) {
        let bull = self.theme.map(|t| t.bull).unwrap_or_else(|| ft().bull);
        let bear = self.theme.map(|t| t.bear).unwrap_or_else(|| ft().bear);
        let dim  = self.theme.map(|t| t.dim).unwrap_or_else(|| ft().dim);
        let text = self.theme.map(|t| t.text).unwrap_or_else(|| ft().text);
        let theme = self.theme;
        let when = self.when;
        let est = self.est_eps;
        let act = self.act_eps;
        let sur = self.surprise;
        CardShell::new_themeless()
            .theme(theme)
            .title(self.symbol)
            .title_style(TextStyle::Numeric)
            .padding(Margin::same(gap_lg() as i8))
            .body(move |ui| {
                ui.add_space(gap_xs());
                if let Some(w) = when {
                    ui.label(RichText::new(w).monospace().size(font_xs()).color(dim));
                }
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Est").monospace().size(font_sm()).color(dim));
                    let s = est.map(|v| format!("{:.2}", v)).unwrap_or_else(|| "—".into());
                    ui.label(RichText::new(s).monospace().size(font_sm()).color(text));
                    ui.label(RichText::new("Act").monospace().size(font_sm()).color(dim));
                    let s = act.map(|v| format!("{:.2}", v)).unwrap_or_else(|| "—".into());
                    ui.label(RichText::new(s).monospace().size(font_sm()).strong().color(text));
                });
                if let Some(p) = sur {
                    let c = if p >= 0.0 { bull } else { bear };
                    ui.label(RichText::new(format!("{:+.1}% surprise", p))
                        .monospace().size(font_sm()).color(c));
                }
            })
            .show(ui);
    }
}
