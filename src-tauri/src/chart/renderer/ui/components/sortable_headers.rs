//! Sortable column headers and N-column metric rows.

use super::super::style::*;
use egui::{self, Color32, Response, RichText, Ui, Vec2};

// ─── Sortable column header ───────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortDirection { None, Asc, Desc }

/// Sortable column header with sort indicator. Returns the click Response.
pub fn sortable_col_header(
    ui: &mut Ui,
    label: &str,
    width: f32,
    sort: SortDirection,
    color: Color32,
    right_align: bool,
) -> Response {
    let layout = if right_align {
        egui::Layout::right_to_left(egui::Align::Center)
    } else {
        egui::Layout::left_to_right(egui::Align::Center)
    };
    let s = style_label_case(label);
    let arrow = match sort {
        SortDirection::Asc  => " \u{25B2}",
        SortDirection::Desc => " \u{25BC}",
        SortDirection::None => "",
    };
    let text = format!("{}{}", s, arrow);
    let mut resp_out: Option<Response> = None;
    ui.allocate_ui_with_layout(Vec2::new(width, 14.0), layout, |ui| {
        let resp = ui.add(
            egui::Button::new(
                RichText::new(text).monospace().size(font_xs()).color(color),
            )
            .frame(false),
        );
        if resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        resp_out = Some(resp);
    });
    resp_out.expect("sortable_col_header response")
}

// ─── Metric grid row ──────────────────────────────────────────────────────────

/// N-column metric row — each cell is a (label, value, value_color) triple.
/// Labels rendered above values in `font_xs` dim; values in `font_md` bold.
/// Common in dashboards / journal stats.
pub fn metric_grid_row(
    ui: &mut Ui,
    cells: &[(&str, &str, Color32)],
    label_color: Color32,
) {
    if cells.is_empty() { return; }
    let avail = ui.available_width();
    let cell_w = (avail / cells.len() as f32).max(60.0);
    ui.horizontal(|ui| {
        for (label, value, value_color) in cells {
            ui.allocate_ui(Vec2::new(cell_w, 0.0), |ui| {
                ui.vertical(|ui| {
                    let s = style_label_case(label);
                    ui.label(
                        RichText::new(s)
                            .monospace()
                            .size(font_xs())
                            .color(label_color),
                    );
                    let value_text = {
                        let mut t = RichText::new(*value)
                            .size(font_md())
                            .strong()
                            .color(*value_color);
                        if current().serif_headlines {
                            t = t.family(egui::FontFamily::Name("serif".into()));
                        } else {
                            t = t.monospace();
                        }
                        t
                    };
                    ui.label(value_text);
                });
            });
        }
    });
}
