//! Panel utilities: themed side-panel frame, hover row tint, category section,
//! label/widget rows, symbol rows, and list-item rows.

use super::super::style::*;
use egui::{self, Color32, Rect, Response, RichText, Sense, Stroke, Ui, Vec2};

// ─── Themed SidePanel frame ───────────────────────────────────────────────────

/// Pre-themed `egui::Frame` for `egui::SidePanel::frame(...)` calls. Parallel
/// to `themed_popup_frame` but tuned for docked side panels: thinner border,
/// no shadow regardless of style.
pub fn themed_side_panel_frame(
    _ctx: &egui::Context,
    theme_bg: Color32,
    theme_border: Color32,
) -> egui::Frame {
    let st = current();
    let stroke = if st.hairline_borders {
        Stroke::new(st.stroke_std, theme_border)
    } else {
        Stroke::new(st.stroke_thin, color_alpha(theme_border, alpha_strong()))
    };
    egui::Frame::NONE
        .fill(theme_bg)
        .stroke(stroke)
        .corner_radius(r_md_cr())
        .inner_margin(egui::Margin::ZERO)
}

// ─── Misc utility components ──────────────────────────────────────────────────

/// Paint a row-tint behind a pre-allocated rect (for hover/selected states
/// where the row was allocated via `interact()` / `allocate_rect`).
pub fn hover_row_tint(ui: &mut Ui, rect: Rect, color: Color32) {
    ui.painter().rect_filled(rect, r_xs(), color);
}

/// Category section — uppercase label + body + bottom rule. Common pattern in
/// settings, hotkey editor, trendline filter.
pub fn category_section<R>(
    ui: &mut Ui,
    label: &str,
    label_color: Color32,
    rule_color: Color32,
    body: impl FnOnce(&mut Ui) -> R,
) -> R {
    let st = current();
    let s = style_label_case(label);
    ui.label(
        RichText::new(s)
            .monospace()
            .size(font_xs())
            .strong()
            .color(label_color),
    );
    ui.add_space(gap_xs());
    let out = body(ui);
    ui.add_space(gap_sm());
    let stroke_w = if st.hairline_borders { st.stroke_std } else { st.stroke_thin };
    let stroke_alpha = if st.hairline_borders { alpha_strong() } else { alpha_muted() };
    let avail = ui.available_width();
    let y = ui.cursor().min.y;
    ui.painter().line_segment(
        [egui::pos2(ui.cursor().min.x, y), egui::pos2(ui.cursor().min.x + avail, y)],
        Stroke::new(stroke_w, color_alpha(rule_color, stroke_alpha)),
    );
    ui.add_space(gap_sm());
    out
}

/// Label + arbitrary right-side widget — like `monospace_label_row` but the
/// value slot is a closure so callers can put a status badge, button, or any
/// other widget there.
pub fn label_widget_row<R>(
    ui: &mut Ui,
    label: &str,
    label_color: Color32,
    value: impl FnOnce(&mut Ui) -> R,
) -> R {
    let mut out: Option<R> = None;
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(label)
                .monospace()
                .size(font_sm())
                .color(label_color),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            out = Some(value(ui));
        });
    });
    out.expect("label_widget_row value")
}

/// Symbol row — sym + name + optional tag. Used in pickers and watchlist.
pub fn symbol_row(
    ui: &mut Ui,
    sym: &str,
    name: &str,
    tag: Option<&str>,
    is_active: bool,
    accent: Color32,
    text: Color32,
    dim: Color32,
) -> Response {
    let row_h = 18.0;
    let avail = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(avail, row_h), Sense::click());
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        ui.painter().rect_filled(rect, r_xs(), color_alpha(dim, alpha_subtle()));
    }
    if is_active {
        ui.painter().rect_filled(rect, r_xs(), color_alpha(accent, alpha_tint()));
    }
    let pad = gap_md();
    let baseline = rect.min.y + 4.0;
    let painter = ui.painter().clone();
    painter.text(
        egui::pos2(rect.min.x + pad, baseline),
        egui::Align2::LEFT_TOP,
        sym,
        egui::FontId::monospace(font_sm()),
        if is_active { accent } else { text },
    );
    let sym_w = (sym.len() as f32) * (font_sm() * 0.6);
    painter.text(
        egui::pos2(rect.min.x + pad + sym_w + gap_md(), baseline),
        egui::Align2::LEFT_TOP,
        name,
        egui::FontId::monospace(font_xs()),
        color_alpha(dim, alpha_strong()),
    );
    if let Some(t) = tag {
        painter.text(
            egui::pos2(rect.max.x - pad, baseline),
            egui::Align2::RIGHT_TOP,
            t,
            egui::FontId::monospace(font_xs()),
            color_alpha(accent, alpha_strong()),
        );
    }
    resp
}

// ─── List item row ────────────────────────────────────────────────────────────

/// Frameless interactive list-item row — for symbol-search results, watchlist
/// rows etc. Provides hover background using the canonical hover-alpha token.
/// Caller paints the row's content via the closure.
pub fn list_item_row<R>(
    ui: &mut Ui,
    accent: Color32,
    content: impl FnOnce(&mut Ui) -> R,
) -> (Response, R) {
    let row_h = 22.0;
    let avail_w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(
        egui::vec2(avail_w, row_h),
        egui::Sense::click(),
    );
    if resp.hovered() && !crate::design_tokens::is_inspect_mode() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        ui.painter().rect_filled(
            rect,
            radius_sm(),
            color_alpha(accent, alpha_ghost()),
        );
    }
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    let inner = content(&mut child);
    (resp, inner)
}
