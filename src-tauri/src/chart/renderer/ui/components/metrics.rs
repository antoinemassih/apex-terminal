//! Metric / stat displays, empty-state.

use super::super::style::*;
use super::text::SectionLabel;
use egui::{self, Color32, RichText, Ui};

// ─── Metric / stat displays ───────────────────────────────────────────────────

/// Metric card — small label above a large colored value, with optional subtitle.
/// Common for portfolio P&L, scanner counts, journal stats.
pub fn metric_value_with_label(
    ui: &mut Ui,
    label: &str,
    value: &str,
    color: Color32,
    size: f32,
    subtitle: Option<&str>,
    label_color: Color32,
) {
    ui.vertical(|ui| {
        ui.add(SectionLabel::new(label).xs().color(label_color));
        let value_text = {
            let mut t = RichText::new(value).size(size).strong().color(color);
            if current().serif_headlines {
                t = t.family(egui::FontFamily::Name("serif".into()));
            } else {
                t = t.monospace();
            }
            t
        };
        ui.label(value_text);
        if let Some(sub) = subtitle {
            ui.label(
                RichText::new(sub)
                    .monospace()
                    .size(font_xs())
                    .color(label_color),
            );
        }
    });
}

/// Label/value row — monospace label on the left, right-aligned value.
/// Used for settings rows, stat dumps, key/value displays.
pub fn monospace_label_row(
    ui: &mut Ui,
    label: &str,
    value: &str,
    value_color: Color32,
    label_color: Color32,
) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(label)
                .monospace()
                .size(font_sm())
                .color(label_color),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                RichText::new(value)
                    .monospace()
                    .size(font_sm())
                    .color(value_color),
            );
        });
    });
}


/// Empty state — centered icon + title + subtitle for "No data" placeholders.
pub fn empty_state_panel(
    ui: &mut Ui,
    icon: &str,
    title: &str,
    subtitle: &str,
    dim: Color32,
) {
    ui.vertical_centered(|ui| {
        ui.add_space(gap_3xl());
        ui.label(RichText::new(icon).size(font_2xl() * 1.5).color(dim));
        ui.add_space(gap_md());
        ui.label(
            RichText::new(title)
                .monospace()
                .size(font_md())
                .strong()
                .color(dim),
        );
        ui.add_space(gap_xs());
        ui.label(
            RichText::new(subtitle)
                .monospace()
                .size(font_sm())
                .color(color_alpha(dim, alpha_muted())),
        );
    });
}

