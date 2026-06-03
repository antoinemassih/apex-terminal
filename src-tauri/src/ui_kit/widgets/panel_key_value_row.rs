//! PanelKeyValueRow — two-column label/value row for metric displays.
//!
//! ```ignore
//! PanelKeyValueRow::new("Buying Power", "$12,500.00")
//!     .tone(Tone::Default)
//!     .meta("USD")
//!     .show(ui, t);
//! ```
//!
//! Visual spec (locked):
//! - Two columns: label LEFT, value (and optional meta) RIGHT.
//! - Label: `mono_xs` in `color_muted(t.dim())`.
//! - Value: `mono_sm` in `t.text` by default, or `tone.color(t)` when set.
//! - Meta: optional very-muted `mono_xs` after the value (units, qualifier).
//! - Row height: `gap_lg()` (16px).
//!
//! When to use:
//! - Inside a `PanelSection` body for label/value metric stacks ("Buying Power",
//!   "Realized P&L", "Win Rate", etc.).
//! - Inside a `PanelCard` for the body of a summary card.
//!
//! When NOT to use:
//! - Clickable list items — use `PanelListRow`.
//! - Rows with bar/progress visualization — use `MetricRow`.
//! - Form fields (label + input control) — use `FormRow`.
//!
//! Sister widgets: `MetricRow`, `PanelListRow`, `PanelSection`.

use egui::{Align, FontId, Layout, Pos2, Sense, Ui, Vec2};

use super::panel_section::Tone;
use crate::ui_kit::tokens::{
    color_alpha, color_muted, font_sm, font_xs, gap_lg, gap_xs,
};
use crate::ui_kit::widgets::theme::ComponentTheme;
use crate::ui_kit::sx::{palette_ct, Tone as SxTone};

#[must_use = "PanelKeyValueRow must be rendered with `.show(...)`"]
pub struct PanelKeyValueRow<'a> {
    label: &'a str,
    value: String,
    tone: Tone,
    meta: Option<String>,
}

impl<'a> PanelKeyValueRow<'a> {
    pub fn new(label: &'a str, value: impl Into<String>) -> Self {
        Self {
            label,
            value: value.into(),
            tone: Tone::Default,
            meta: None,
        }
    }

    pub fn tone(mut self, tone: Tone) -> Self {
        self.tone = tone;
        self
    }

    pub fn meta(mut self, m: impl Into<String>) -> Self {
        self.meta = Some(m.into());
        self
    }

    pub fn show<T: ComponentTheme>(self, ui: &mut Ui, t: &T) {
        let h = gap_lg();
        let avail_w = ui.available_width();
        let (rect, _resp) = ui.allocate_exact_size(Vec2::new(avail_w, h), Sense::hover());

        let painter = ui.painter_at(rect);

        // Label — left.
        let label_color = color_muted(t.dim());
        let label_font = FontId::monospace(font_xs());
        painter.text(
            Pos2::new(rect.left(), rect.center().y),
            egui::Align2::LEFT_CENTER,
            self.label,
            label_font,
            label_color,
        );

        // Value (+ meta) — right.
        let value_color = match self.tone {
            Tone::Default => t.text(),
            other => other.color(t),
        };
        let value_font = FontId::monospace(font_sm());

        // Lay out meta first (further right), then value to its left.
        let mut x_right = rect.right();
        if let Some(m) = &self.meta {
            let meta_color = color_alpha(t.dim(), 140);
            let meta_font = FontId::monospace(font_xs());
            let g = ui.fonts(|f| {
                f.layout_no_wrap(m.clone(), meta_font.clone(), meta_color)
            });
            painter.text(
                Pos2::new(x_right, rect.center().y),
                egui::Align2::RIGHT_CENTER,
                m,
                meta_font,
                meta_color,
            );
            x_right -= g.size().x + gap_xs();
        }

        painter.text(
            Pos2::new(x_right, rect.center().y),
            egui::Align2::RIGHT_CENTER,
            &self.value,
            value_font,
            value_color,
        );

        // Silence imports we keep available for future-state ports.
        let _ = (Align::Center, Layout::default());
    }
}
