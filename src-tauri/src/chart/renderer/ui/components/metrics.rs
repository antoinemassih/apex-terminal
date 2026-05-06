//! Metric / stat displays, action/icon buttons, empty-state, insight stat bar.

use super::super::style::*;
use super::labels::section_label_xs;
use egui::{self, Color32, Rect, Response, RichText, Sense, Ui, Vec2};

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
        section_label_xs(ui, label, label_color);
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

/// Direction badge — ▲/▼ + price, colored bull/bear.
pub fn colored_direction_badge(
    ui: &mut Ui,
    above: bool,
    price: f32,
    bull_col: Color32,
    bear_col: Color32,
) -> Response {
    let (sym, col) = if above { ("\u{25B2}", bull_col) } else { ("\u{25BC}", bear_col) };
    ui.horizontal(|ui| {
        ui.label(RichText::new(sym).monospace().size(font_xs()).color(col));
        ui.label(
            RichText::new(format!("{:.2}", price))
                .monospace()
                .size(font_sm())
                .strong()
                .color(col),
        );
    })
    .response
}

// ─── Buttons ──────────────────────────────────────────────────────────────────

/// Small action button — minimal, text-only, monospace; frameless, returns Response.
/// Used in tight header rows. Distinct from style::small_action_btn (which returns bool).
pub fn text_action_btn(ui: &mut Ui, text: &str, color: Color32) -> Response {
    let resp = ui.add(
        egui::Button::new(
            RichText::new(text)
                .monospace()
                .size(font_xs())
                .strong()
                .color(color),
        )
        .frame(false)
        .min_size(Vec2::new(0.0, 14.0)),
    );
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

/// Icon-only button — frameless, smaller, hover changes cursor, returns Response.
/// Distinct from style::icon_btn (which has different sizing behavior).
pub fn inline_icon_btn(ui: &mut Ui, icon: &str, color: Color32, size: f32) -> Response {
    let resp = ui.add(
        egui::Button::new(RichText::new(icon).size(size).color(color))
            .frame(false)
            .min_size(Vec2::new(size + 2.0, size + 2.0)),
    );
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

// ─── Empty state ──────────────────────────────────────────────────────────────

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

// ─── Stat bar ─────────────────────────────────────────────────────────────────

/// Insight stat bar — label, filled progress bar, count + pct.
pub fn insight_stat_bar(
    ui: &mut Ui,
    label: &str,
    pct: f32,
    count: u32,
    bar_color: Color32,
    track_color: Color32,
    label_color: Color32,
) {
    ui.horizontal(|ui| {
        ui.allocate_ui(Vec2::new(80.0, 14.0), |ui| {
            ui.label(
                RichText::new(label)
                    .monospace()
                    .size(font_sm())
                    .color(label_color),
            );
        });

        // Bar
        let bar_w = ui.available_width() - 80.0;
        let (rect, _) = ui.allocate_exact_size(Vec2::new(bar_w.max(40.0), 6.0), Sense::hover());
        ui.painter().rect_filled(rect, r_xs(), track_color);
        let fill_w = rect.width() * pct.clamp(0.0, 1.0);
        let fill_rect = Rect::from_min_size(rect.min, Vec2::new(fill_w, rect.height()));
        ui.painter().rect_filled(fill_rect, r_xs(), bar_color);

        // Right-aligned count + pct
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                RichText::new(format!("{:>3.0}% · {}", pct * 100.0, count))
                    .monospace()
                    .size(font_xs())
                    .color(label_color),
            );
        });
    });
}

