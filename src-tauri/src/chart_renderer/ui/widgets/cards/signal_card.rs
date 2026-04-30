//! `SignalCard` — signal name + score + sparkline placeholder.
//!
//! Wave 4.5d: migrated onto `CardShell`. Public API unchanged.

#![allow(dead_code, unused_imports)]

use egui::{Color32, Margin, RichText, Sense, Stroke, Ui, Vec2};
use super::super::super::style::*;
use super::super::foundation::shell::CardShell;
use super::super::foundation::text_style::TextStyle;

type Theme = crate::chart_renderer::gpu::Theme;

#[must_use = "SignalCard must be rendered with `.show(ui)`"]
pub struct SignalCard<'a> {
    name:   &'a str,
    score:  f32,
    spark:  Option<&'a [f32]>,
    theme:  Option<&'a Theme>,
}

impl<'a> SignalCard<'a> {
    pub fn new(name: &'a str, score: f32) -> Self {
        Self { name, score, spark: None, theme: None }
    }
    pub fn spark(mut self, samples: &'a [f32]) -> Self { self.spark = Some(samples); self }
    pub fn theme(mut self, t: &'a Theme) -> Self { self.theme = Some(t); self }

    pub fn show(self, ui: &mut Ui) {
        let score = self.score;
        let spark = self.spark;
        let theme = self.theme;
        CardShell::new_themeless()
            .theme(theme)
            .title(self.name)
            .title_style(TextStyle::Numeric)
            .padding(Margin::same(gap_lg() as i8))
            .body(move |ui| {
                ui.add_space(gap_xs());
                let color = theme
                    .map(|t| if score >= 0.0 { t.bull } else { t.bear })
                    .unwrap_or(Color32::from_rgb(180, 180, 200));
                ui.label(RichText::new(format!("{:+.2}", score))
                    .monospace().size(font_xl()).strong().color(color));
                let h = font_md() + 2.0;
                let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), h), Sense::hover());
                let dim = theme.map(|t| t.dim).unwrap_or(Color32::DARK_GRAY);
                ui.painter().rect_stroke(
                    rect,
                    egui::CornerRadius::same(radius_sm() as u8),
                    Stroke::new(stroke_thin(), color_alpha(dim, alpha_muted())),
                    egui::StrokeKind::Inside,
                );
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
                        ui.painter().add(egui::Shape::line(pts, Stroke::new(stroke_std(), color)));
                    }
                }
            })
            .show(ui);
    }
}
