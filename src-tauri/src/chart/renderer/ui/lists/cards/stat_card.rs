//! `StatCard` — label + big value + delta + sparkline area.
//!
//! Wave 4.5d: migrated onto `CardShell`. Public API unchanged.

#![allow(dead_code, unused_imports)]

use egui::{Color32, Margin, RichText, Sense, Stroke, Ui, Vec2};
use super::super::super::style::*;
use crate::chart::renderer::ui::foundation::shell::CardShell;

type Theme = crate::chart_renderer::gpu::Theme;

fn ft() -> &'static Theme { &crate::chart_renderer::gpu::THEMES[0] }

#[must_use = "StatCard must be rendered with `.show(ui)`"]
pub struct StatCard<'a> {
    label: &'a str,
    value: String,
    delta: Option<f32>,
    spark: Option<&'a [f32]>,
    theme: Option<&'a Theme>,
}

impl<'a> StatCard<'a> {
    pub fn new(label: &'a str, value: impl Into<String>) -> Self {
        Self { label, value: value.into(), delta: None, spark: None, theme: None }
    }
    pub fn delta(mut self, d: f32) -> Self { self.delta = Some(d); self }
    pub fn spark(mut self, s: &'a [f32]) -> Self { self.spark = Some(s); self }
    pub fn theme(mut self, t: &'a Theme) -> Self { self.theme = Some(t); self }

    pub fn show(self, ui: &mut Ui) {
        let bull = self.theme.map(|t| t.bull).unwrap_or_else(|| ft().bull);
        let bear = self.theme.map(|t| t.bear).unwrap_or_else(|| ft().bear);
        let dim  = self.theme.map(|t| t.dim).unwrap_or_else(|| ft().dim);
        let text = self.theme.map(|t| t.text).unwrap_or_else(|| ft().text);
        let theme = self.theme;
        let label = self.label;
        let value = self.value;
        let delta = self.delta;
        let spark = self.spark;
        CardShell::new_themeless()
            .theme(theme)
            .padding(Margin::same(gap_lg() as i8))
            .body(move |ui| {
                ui.label(RichText::new(label).monospace().size(font_xs()).color(dim));
                ui.label(RichText::new(&value).monospace().size(font_xl()).strong().color(text));
                let line_color = if let Some(d) = delta {
                    let c = if d >= 0.0 { bull } else { bear };
                    ui.label(RichText::new(format!("{:+.2}", d))
                        .monospace().size(font_sm()).color(c));
                    c
                } else {
                    text
                };
                ui.add_space(gap_xs());
                let h = font_lg() + 4.0;
                let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), h), Sense::hover());
                if let Some(samples) = spark {
                    if samples.len() >= 2 {
                        let (mn, mx) = samples.iter().fold((f32::INFINITY, f32::NEG_INFINITY),
                            |(a, b), v| (a.min(*v), b.max(*v)));
                        let span = (mx - mn).max(1e-6);
                        let pts: Vec<egui::Pos2> = samples.iter().enumerate().map(|(i, v)| {
                            let x = rect.left() + (i as f32 / (samples.len() - 1) as f32) * rect.width();
                            let y = rect.bottom() - ((v - mn) / span) * rect.height();
                            egui::pos2(x, y)
                        }).collect();
                        ui.painter().add(egui::Shape::line(pts, Stroke::new(stroke_std(), line_color)));
                    }
                }
            })
            .show(ui);
    }
}
