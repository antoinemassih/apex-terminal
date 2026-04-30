//! Spreadsheet pane — editable string-cell grid (v1).
//!
//! v1 scope: editable string cells. No formulas, copy/paste, or persistence
//! beyond the in-memory `Chart` fields. Click to select, double-click to edit.

use egui;
use super::style::*;
use super::super::gpu::*;

const HEADER_H: f32 = 18.0;
const ROW_H: f32 = 22.0;
const GUTTER_W: f32 = 32.0;
const CELL_W: f32 = 96.0;
const TOOLBAR_H: f32 = 28.0;

fn col_label(mut idx: usize) -> String {
    let mut s = String::new();
    idx += 1;
    while idx > 0 {
        let r = (idx - 1) % 26;
        s.insert(0, (b'A' + r as u8) as char);
        idx = (idx - 1) / 26;
    }
    s
}

fn cell_ref(row: usize, col: usize) -> String {
    format!("{}{}", col_label(col), row + 1)
}

pub(crate) fn render(
    ui: &mut egui::Ui, _ctx: &egui::Context,
    panes: &mut [Chart], pane_idx: usize, active_pane: &mut usize,
    _visible_count: usize, pane_rects: &[egui::Rect], theme_idx: usize,
    _watchlist: &mut Watchlist,
) {
    let t = &THEMES[theme_idx];
    if pane_rects.is_empty() { return; }
    let rect = pane_rects[0];

    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, t.bg);

    if let Some(pos) = ui.ctx().pointer_hover_pos() {
        if rect.contains(pos) { *active_pane = pane_idx; }
    }

    let chart = &mut panes[pane_idx];

    // ── Top toolbar ──
    let toolbar_rect = egui::Rect::from_min_size(
        rect.min, egui::vec2(rect.width(), TOOLBAR_H));
    let mut toolbar_ui = ui.new_child(egui::UiBuilder::new()
        .max_rect(toolbar_rect.shrink2(egui::vec2(GAP_SM, GAP_XS))));
    toolbar_ui.horizontal_centered(|ui| {
        let btn = |ui: &mut egui::Ui, label: &str| -> bool {
            ui.add(egui::Button::new(egui::RichText::new(label)
                    .monospace().size(FONT_XS).color(t.dim))
                .fill(color_alpha(t.toolbar_border, ALPHA_TINT))
                .stroke(egui::Stroke::new(stroke_thin(),
                    color_alpha(t.toolbar_border, ALPHA_MUTED)))
                .corner_radius(RADIUS_SM)
                .min_size(egui::vec2(0.0, 18.0))).clicked()
        };
        if btn(ui, "+ Row") {
            let cols = chart.spreadsheet_cols.max(1);
            chart.spreadsheet_cells.push(vec![String::new(); cols]);
            chart.spreadsheet_rows = chart.spreadsheet_cells.len();
        }
        if btn(ui, "+ Col") {
            chart.spreadsheet_cols += 1;
            for row in chart.spreadsheet_cells.iter_mut() {
                row.push(String::new());
            }
        }
        if btn(ui, "Clear") {
            for row in chart.spreadsheet_cells.iter_mut() {
                for c in row.iter_mut() { c.clear(); }
            }
            chart.spreadsheet_editing = None;
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let label = match chart.spreadsheet_selected {
                Some((r, c)) => cell_ref(r, c),
                None => "—".into(),
            };
            ui.label(egui::RichText::new(label).monospace().size(FONT_XS).color(t.accent));
        });
    });

    // Empty state
    if chart.spreadsheet_rows == 0 || chart.spreadsheet_cols == 0 {
        let empty_rect = egui::Rect::from_min_max(
            egui::pos2(rect.left(), rect.top() + TOOLBAR_H),
            rect.max);
        let p = ui.painter_at(empty_rect);
        p.text(egui::pos2(empty_rect.center().x, empty_rect.center().y - 8.0),
            egui::Align2::CENTER_CENTER, "No cells",
            egui::FontId::monospace(FONT_LG), t.dim.gamma_multiply(0.6));
        p.text(egui::pos2(empty_rect.center().x, empty_rect.center().y + 8.0),
            egui::Align2::CENTER_CENTER, "Add a row to start",
            egui::FontId::monospace(FONT_SM), t.dim.gamma_multiply(0.4));
        // Action button
        let bw = 80.0; let bh = 20.0;
        let br = egui::Rect::from_center_size(
            egui::pos2(empty_rect.center().x, empty_rect.center().y + 28.0),
            egui::vec2(bw, bh));
        let resp = ui.interact(br, ui.id().with("ss_empty_addrow"), egui::Sense::click());
        let p2 = ui.painter_at(br);
        p2.rect_filled(br, RADIUS_SM, color_alpha(t.accent, ALPHA_TINT));
        p2.rect_stroke(br, RADIUS_SM,
            egui::Stroke::new(stroke_thin(), color_alpha(t.accent, ALPHA_LINE)),
            egui::epaint::StrokeKind::Middle);
        p2.text(br.center(), egui::Align2::CENTER_CENTER, "Add row",
            egui::FontId::monospace(FONT_XS), t.accent);
        if resp.clicked() {
            let cols = chart.spreadsheet_cols.max(1);
            chart.spreadsheet_cols = cols;
            chart.spreadsheet_cells.push(vec![String::new(); cols]);
            chart.spreadsheet_rows = chart.spreadsheet_cells.len();
        }
        return;
    }

    // ── Grid area ──
    let grid_top = rect.top() + TOOLBAR_H;
    let grid_rect = egui::Rect::from_min_max(
        egui::pos2(rect.left(), grid_top), rect.max);

    // Column header strip
    let header_rect = egui::Rect::from_min_size(
        egui::pos2(grid_rect.left(), grid_rect.top()),
        egui::vec2(grid_rect.width(), HEADER_H));
    let p = ui.painter_at(header_rect);
    p.rect_filled(header_rect, 0.0, color_alpha(t.toolbar_border, ALPHA_TINT));
    // Column labels
    for c in 0..chart.spreadsheet_cols {
        let x = grid_rect.left() + GUTTER_W + (c as f32) * CELL_W;
        let r = egui::Rect::from_min_size(
            egui::pos2(x, header_rect.top()), egui::vec2(CELL_W, HEADER_H));
        p.text(r.center(), egui::Align2::CENTER_CENTER, col_label(c),
            egui::FontId::monospace(FONT_XS), t.dim.gamma_multiply(0.7));
        // vertical separator
        p.line_segment([
            egui::pos2(x, header_rect.top()),
            egui::pos2(x, header_rect.bottom())],
            egui::Stroke::new(stroke_thin(),
                color_alpha(t.toolbar_border, ALPHA_MUTED)));
    }

    // Scrollable body for rows
    let body_top = header_rect.bottom();
    let body_rect = egui::Rect::from_min_max(
        egui::pos2(grid_rect.left(), body_top), grid_rect.max);

    let total_w = GUTTER_W + (chart.spreadsheet_cols as f32) * CELL_W;
    let total_h = (chart.spreadsheet_rows as f32) * ROW_H;

    let mut body_ui = ui.new_child(egui::UiBuilder::new().max_rect(body_rect));
    egui::ScrollArea::both()
        .id_salt(("ss_scroll", pane_idx))
        .auto_shrink([false, false])
        .show(&mut body_ui, |ui| {
            let (resp_rect, _) = ui.allocate_exact_size(
                egui::vec2(total_w, total_h), egui::Sense::hover());
            let p = ui.painter_at(resp_rect);
            let stroke_grid = egui::Stroke::new(stroke_thin(),
                color_alpha(t.toolbar_border, ALPHA_MUTED));

            // Row gutter background
            let gutter_rect = egui::Rect::from_min_size(resp_rect.min,
                egui::vec2(GUTTER_W, total_h));
            p.rect_filled(gutter_rect, 0.0, color_alpha(t.toolbar_border, ALPHA_TINT));

            // Draw rows
            let mut commit: Option<(usize, usize, String)> = None;
            let mut cancel_edit = false;
            let mut start_edit: Option<(usize, usize)> = None;
            let mut new_select: Option<(usize, usize)> = None;

            for r in 0..chart.spreadsheet_rows {
                let y = resp_rect.top() + (r as f32) * ROW_H;
                // Row number
                let num_rect = egui::Rect::from_min_size(
                    egui::pos2(resp_rect.left(), y), egui::vec2(GUTTER_W, ROW_H));
                p.text(num_rect.center(), egui::Align2::CENTER_CENTER,
                    format!("{}", r + 1),
                    egui::FontId::monospace(FONT_XS), t.dim.gamma_multiply(0.6));
                // Horizontal grid line
                p.line_segment([
                    egui::pos2(resp_rect.left(), y + ROW_H),
                    egui::pos2(resp_rect.left() + total_w, y + ROW_H)],
                    stroke_grid);

                for c in 0..chart.spreadsheet_cols {
                    let x = resp_rect.left() + GUTTER_W + (c as f32) * CELL_W;
                    let cell_rect = egui::Rect::from_min_size(
                        egui::pos2(x, y), egui::vec2(CELL_W, ROW_H));
                    // Vertical grid line
                    p.line_segment([
                        egui::pos2(x + CELL_W, y),
                        egui::pos2(x + CELL_W, y + ROW_H)],
                        stroke_grid);

                    let editing_here = matches!(chart.spreadsheet_editing,
                        Some((er, ec, _)) if er == r && ec == c);
                    let selected_here = chart.spreadsheet_selected == Some((r, c));

                    if editing_here {
                        // Render TextEdit inline
                        if let Some((_, _, buf)) = chart.spreadsheet_editing.as_mut() {
                            let mut child = ui.new_child(
                                egui::UiBuilder::new().max_rect(cell_rect.shrink(1.0)));
                            let resp = child.add(egui::TextEdit::singleline(buf)
                                .font(egui::FontId::monospace(FONT_SM))
                                .frame(false)
                                .margin(egui::vec2(2.0, 2.0))
                                .desired_width(CELL_W - 4.0));
                            // Focus ring
                            ui.painter_at(cell_rect).rect_stroke(cell_rect, 0.0,
                                egui::Stroke::new(1.0, t.accent),
                                egui::epaint::StrokeKind::Middle);
                            resp.request_focus();
                            let input = ui.input(|i| (
                                i.key_pressed(egui::Key::Enter),
                                i.key_pressed(egui::Key::Tab),
                                i.key_pressed(egui::Key::Escape),
                            ));
                            if input.0 || input.1 {
                                commit = Some((r, c, buf.clone()));
                            } else if input.2 {
                                cancel_edit = true;
                            } else if resp.lost_focus() {
                                commit = Some((r, c, buf.clone()));
                            }
                        }
                    } else {
                        // Draw value
                        let val = &chart.spreadsheet_cells[r][c];
                        if !val.is_empty() {
                            let pp = ui.painter_at(cell_rect);
                            pp.text(
                                egui::pos2(cell_rect.left() + 4.0, cell_rect.center().y),
                                egui::Align2::LEFT_CENTER, val,
                                egui::FontId::monospace(FONT_SM),
                                TEXT_PRIMARY);
                        }
                        if selected_here {
                            ui.painter_at(cell_rect).rect_stroke(cell_rect, 0.0,
                                egui::Stroke::new(1.0, t.accent),
                                egui::epaint::StrokeKind::Middle);
                        }
                        // Interact
                        let id = ui.id().with(("ss_cell", r, c));
                        let resp = ui.interact(cell_rect, id,
                            egui::Sense::click_and_drag());
                        if resp.clicked() {
                            new_select = Some((r, c));
                        }
                        if resp.double_clicked() {
                            start_edit = Some((r, c));
                        }
                    }
                }
            }

            // Apply state changes
            if let Some((r, c, val)) = commit {
                if r < chart.spreadsheet_cells.len()
                    && c < chart.spreadsheet_cells[r].len() {
                    chart.spreadsheet_cells[r][c] = val;
                }
                chart.spreadsheet_editing = None;
            } else if cancel_edit {
                chart.spreadsheet_editing = None;
            }
            if let Some((r, c)) = start_edit {
                let cur = chart.spreadsheet_cells[r][c].clone();
                chart.spreadsheet_editing = Some((r, c, cur));
                chart.spreadsheet_selected = Some((r, c));
            } else if let Some(sel) = new_select {
                chart.spreadsheet_selected = Some(sel);
            }
        });
}
