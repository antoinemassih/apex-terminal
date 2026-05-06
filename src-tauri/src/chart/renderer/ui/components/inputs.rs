//! Inputs: search box, numeric steppers, toggle rows, toggle switch,
//! text + numeric input fields, radio button rows.

use super::super::style::*;
use egui::{self, Color32, Response, RichText, Sense, Stroke, Ui, Vec2};

// ─── Search input ─────────────────────────────────────────────────────────────

/// Search input — text edit framed with a magnifier glyph.
pub fn search_input(
    ui: &mut Ui,
    buffer: &mut String,
    placeholder: &str,
    accent: Color32,
    dim: Color32,
    border: Color32,
) -> egui::Response {
    let st = current();
    let avail = ui.available_width();
    let frame = egui::Frame::NONE
        .fill(Color32::TRANSPARENT)
        .corner_radius(r_sm_cr())
        .stroke(if st.hairline_borders {
            Stroke::new(st.stroke_std, color_alpha(border, alpha_strong()))
        } else {
            Stroke::new(st.stroke_thin, color_alpha(border, alpha_muted()))
        })
        .inner_margin(egui::Margin {
            left: gap_md() as i8,
            right: gap_md() as i8,
            top: gap_xs() as i8,
            bottom: gap_xs() as i8,
        });
    let mut resp_out: Option<egui::Response> = None;
    frame.show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new("\u{1F50D}").size(font_sm()).color(dim));
            let edit = egui::TextEdit::singleline(buffer)
                .desired_width(avail - 36.0)
                .hint_text(RichText::new(placeholder).color(color_alpha(dim, alpha_muted())))
                .text_color(accent)
                .frame(false);
            resp_out = Some(ui.add(edit));
        });
    });
    resp_out.expect("search_input response")
}

// ─── Numeric stepper ──────────────────────────────────────────────────────────

/// Compact stepper — 14x14 buttons with dim value (no accent). Returns delta.
/// For tight inline layouts where `numeric_stepper`'s 18x18 is too big.
pub fn compact_stepper(
    ui: &mut Ui,
    value: &str,
    dim: Color32,
    border: Color32,
) -> i32 {
    let mut delta = 0;
    ui.horizontal(|ui| {
        let prev = ui.spacing().item_spacing.x;
        ui.spacing_mut().item_spacing.x = gap_xs();

        let st = current();
        let cr = egui::CornerRadius::same(st.r_xs as u8);
        let stroke = if st.hairline_borders {
            Stroke::new(st.stroke_std, color_alpha(border, alpha_strong()))
        } else {
            Stroke::new(st.stroke_thin, color_alpha(border, alpha_muted()))
        };

        let mut mk_btn = |ui: &mut Ui, sym: &str| -> Response {
            ui.add(
                egui::Button::new(
                    RichText::new(sym).monospace().size(font_xs()).color(dim),
                )
                .fill(Color32::TRANSPARENT)
                .stroke(stroke)
                .corner_radius(cr)
                .min_size(Vec2::new(14.0, 14.0)),
            )
        };

        if mk_btn(ui, "-").clicked() { delta = -1; }
        ui.label(
            RichText::new(value)
                .monospace()
                .size(font_xs())
                .color(dim),
        );
        if mk_btn(ui, "+").clicked() { delta = 1; }

        ui.spacing_mut().item_spacing.x = prev;
    });
    delta
}

/// Numeric stepper [-]value[+]. Returns delta clicks (-1, 0, +1).
pub fn numeric_stepper(
    ui: &mut Ui,
    value: &str,
    accent: Color32,
    dim: Color32,
    border: Color32,
) -> i32 {
    let mut delta = 0;
    ui.horizontal(|ui| {
        let prev = ui.spacing().item_spacing.x;
        ui.spacing_mut().item_spacing.x = gap_xs();

        let st = current();
        let cr = egui::CornerRadius::same(st.r_xs as u8);
        let stroke = if st.hairline_borders {
            Stroke::new(st.stroke_std, color_alpha(border, alpha_strong()))
        } else {
            Stroke::new(st.stroke_thin, color_alpha(border, alpha_muted()))
        };

        let mut mk_btn = |ui: &mut Ui, sym: &str| -> Response {
            ui.add(
                egui::Button::new(
                    RichText::new(sym).monospace().size(font_sm()).strong().color(dim),
                )
                .fill(Color32::TRANSPARENT)
                .stroke(stroke)
                .corner_radius(cr)
                .min_size(Vec2::new(18.0, 18.0)),
            )
        };

        if mk_btn(ui, "-").clicked() { delta = -1; }
        ui.label(
            RichText::new(value)
                .monospace()
                .size(font_sm())
                .strong()
                .color(accent),
        );
        if mk_btn(ui, "+").clicked() { delta = 1; }

        ui.spacing_mut().item_spacing.x = prev;
    });
    delta
}

// ─── Toggle row ───────────────────────────────────────────────────────────────

/// Settings-style row: label on left, checkbox on right. Uppercased under Meridien.
pub fn toggle_row(
    ui: &mut Ui,
    label: &str,
    state: &mut bool,
    label_color: Color32,
) -> Response {
    let mut resp = ui.allocate_response(Vec2::ZERO, Sense::hover());
    ui.horizontal(|ui| {
        let s = style_label_case(label);
        ui.label(
            RichText::new(s)
                .monospace()
                .size(font_sm())
                .color(label_color),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            resp = ui.checkbox(state, "");
        });
    });
    resp
}

// ─── ToggleSwitch ────────────────────────────────────────────────────────────

pub fn toggle_switch(ui: &mut Ui, state: &mut bool, accent: Color32, dim: Color32) -> Response {
    let track_w: f32 = 32.0;
    let track_h: f32 = 16.0;
    let thumb_d: f32 = 12.0;
    let pad: f32 = 2.0;
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(track_w, track_h), egui::Sense::click());
    if resp.clicked() {
        *state = !*state;
    }
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    let anim_id = resp.id;
    let t = ui.ctx().animate_bool_with_time(anim_id, *state, 0.15);
    if ui.is_rect_visible(rect) {
        let p = ui.painter();
        let cr = track_h / 2.0;
        let track_color = if *state { color_alpha(accent, alpha_active()) } else { color_alpha(dim, alpha_dim()) };
        p.rect_filled(rect, cr, track_color);
        if !*state {
            p.rect_stroke(rect, cr, Stroke::new(stroke_std(), color_alpha(dim, alpha_muted())), egui::StrokeKind::Inside);
        }
        let thumb_travel = track_w - thumb_d - pad * 2.0;
        let thumb_cx = rect.left() + pad + thumb_d / 2.0 + thumb_travel * t;
        let thumb_cy = rect.center().y;
        p.circle_filled(egui::pos2(thumb_cx, thumb_cy), thumb_d / 2.0, Color32::WHITE);
    }
    resp
}

// ─── TextInput ───────────────────────────────────────────────────────────────

pub fn text_input_field(
    ui: &mut Ui,
    buffer: &mut String,
    placeholder: &str,
    accent: Color32,
    _dim: Color32,
    border: Color32,
) -> Response {
    let id = ui.next_auto_id();
    let focused = ui.memory(|m| m.has_focus(id));
    let border_color = if focused { color_alpha(accent, alpha_active()) } else { color_alpha(border, alpha_line()) };
    let frame = egui::Frame::NONE
        .stroke(Stroke::new(stroke_std(), border_color))
        .inner_margin(gap_sm())
        .corner_radius(radius_sm());
    let mut resp_opt: Option<Response> = None;
    frame.show(ui, |ui| {
        let te = egui::TextEdit::singleline(buffer)
            .id(id)
            .hint_text(placeholder)
            .font(egui::FontSelection::FontId(egui::FontId::monospace(font_sm())))
            .frame(false)
            .desired_width(ui.available_width());
        resp_opt = Some(ui.add(te));
    });
    resp_opt.unwrap_or_else(|| ui.label(""))
}

// ─── NumericInput ────────────────────────────────────────────────────────────

pub fn numeric_input_field(
    ui: &mut Ui,
    value: &mut f32,
    placeholder: &str,
    accent: Color32,
    dim: Color32,
    border: Color32,
) -> Response {
    let buf_id = ui.next_auto_id();
    let mut buf = ui.memory_mut(|m| {
        m.data.get_temp_mut_or_insert_with::<String>(buf_id, || value.to_string()).clone()
    });
    let resp = text_input_field(ui, &mut buf, placeholder, accent, dim, border);
    ui.memory_mut(|m| {
        *m.data.get_temp_mut_or_insert_with::<String>(buf_id, || value.to_string()) = buf.clone();
    });
    if resp.lost_focus() {
        if let Ok(parsed) = buf.trim().parse::<f32>() {
            *value = parsed;
        }
        ui.memory_mut(|m| {
            *m.data.get_temp_mut_or_insert_with::<String>(buf_id, || value.to_string()) =
                value.to_string();
        });
    }
    resp
}

// ─── RadioButtonRow ──────────────────────────────────────────────────────────

pub fn radio_button_row<T: PartialEq + Clone>(
    ui: &mut Ui,
    current_val: &mut T,
    options: &[(T, &str)],
    accent: Color32,
    dim: Color32,
) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = gap_md();
        for (value, label) in options {
            let active = *current_val == *value;
            let fg = if active { accent } else { color_alpha(dim, alpha_muted()) };
            let bg = if active { color_alpha(accent, alpha_subtle()) } else { Color32::TRANSPARENT };
            let border = if active { color_alpha(accent, alpha_dim()) } else { color_alpha(dim, alpha_line()) };
            let resp = ui.add(
                egui::Button::new(RichText::new(*label).monospace().size(font_sm()).strong().color(fg))
                    .fill(bg)
                    .stroke(Stroke::new(stroke_std(), border))
                    .corner_radius(radius_sm())
                    .min_size(egui::vec2(0.0, 20.0)),
            );
            if resp.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            if resp.clicked() && !active {
                *current_val = value.clone();
                changed = true;
            }
        }
    });
    changed
}
