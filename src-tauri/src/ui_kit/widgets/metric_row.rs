//! MetricRow — "Label: Value" row used across stat panels.
//!
//! Replaces inline `ui.horizontal { ui.label(label); ui.label(value) }`
//! patterns that appear 50+ times in panels (orders, journal, plays,
//! diagnostics). Provides consistent spacing, font choice, and optional
//! semantic tone for the value (bull / bear / warn / accent / muted).
//!
//! ```ignore
//! MetricRow::new("Realized P&L").value("$1,234.56").tone(Tone::Bull).show(ui, t);
//! MetricRow::new("Win rate").value("64%").bar(0.64).show(ui, t);
//! ```

use egui::{Response, RichText, Ui};

use super::theme::ComponentTheme;
use crate::ui_kit::tokens as st;

/// Semantic tint for the value side of a `MetricRow`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tone {
    #[default]
    Default,
    Muted,
    Accent,
    Bull,
    Bear,
    Warn,
}

#[must_use = "MetricRow does nothing until `.show(ui, theme)` is called"]
pub struct MetricRow<'a> {
    label: &'a str,
    value: String,
    tone: Tone,
    /// Optional 0..=1 progress shown as a hairline bar under the row.
    bar: Option<f32>,
    /// Optional override for the value font size.
    value_font: Option<f32>,
    /// Optional override for the label font size (default = font_xs).
    label_font: Option<f32>,
    monospace: bool,
}

impl<'a> MetricRow<'a> {
    pub fn new(label: &'a str) -> Self {
        Self {
            label,
            value: String::new(),
            tone: Tone::Default,
            bar: None,
            value_font: None,
            label_font: None,
            monospace: true,
        }
    }

    pub fn value(mut self, v: impl Into<String>) -> Self { self.value = v.into(); self }
    pub fn tone(mut self, tone: Tone) -> Self { self.tone = tone; self }
    pub fn bar(mut self, p: f32) -> Self { self.bar = Some(p.clamp(0.0, 1.0)); self }
    pub fn value_font(mut self, px: f32) -> Self { self.value_font = Some(px); self }
    pub fn label_font(mut self, px: f32) -> Self { self.label_font = Some(px); self }
    pub fn proportional(mut self) -> Self { self.monospace = false; self }

    pub fn show(self, ui: &mut Ui, theme: &dyn ComponentTheme) -> Response {
        let label_size = self.label_font.unwrap_or_else(st::font_xs);
        let value_size = self.value_font.unwrap_or_else(st::font_sm);
        let label_color = theme.dim();
        let value_color = match self.tone {
            Tone::Default => theme.text(),
            Tone::Muted   => st::color_muted(theme.dim()),
            Tone::Accent  => theme.accent(),
            Tone::Bull    => theme.bull(),
            Tone::Bear    => theme.bear(),
            Tone::Warn    => theme.warn(),
        };

        let resp = ui.horizontal(|ui| {
            let mut label_text = RichText::new(self.label).size(label_size).color(label_color);
            if self.monospace { label_text = label_text.monospace(); }
            ui.label(label_text);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut v = RichText::new(&self.value).size(value_size).color(value_color);
                if self.monospace { v = v.monospace(); }
                ui.label(v);
            });
        });

        if let Some(p) = self.bar {
            let r = resp.response.rect;
            let bar_y = r.bottom() + 1.0;
            let total_w = r.width();
            let painter = ui.painter();
            painter.line_segment(
                [egui::pos2(r.left(), bar_y), egui::pos2(r.left() + total_w, bar_y)],
                egui::Stroke::new(st::stroke_thin(), st::color_alpha(theme.border(), st::alpha_line())),
            );
            painter.line_segment(
                [egui::pos2(r.left(), bar_y), egui::pos2(r.left() + total_w * p, bar_y)],
                egui::Stroke::new(st::stroke_bold(), value_color),
            );
        }

        resp.response
    }
}
