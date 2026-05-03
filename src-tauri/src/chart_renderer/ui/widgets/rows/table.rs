//! Generic `Table<T>` — sortable headers, sticky-first-col, row striping,
//! optional select column. NEW — no call sites migrated.
//!
//! Composition: caller supplies `columns()` (label + width spec + optional
//! sort key) and a `row_render` closure that fills each cell. The table
//! handles header click → sort state, vertical scroll, striping, and an
//! optional first checkbox column.

#![allow(dead_code, unused_imports)]

use egui::{Color32, Response, Sense, Stroke, Ui};
use super::super::super::style::*;

type Theme = crate::chart_renderer::gpu::Theme;
fn ft() -> &'static Theme { &crate::chart_renderer::gpu::THEMES[0] }

#[derive(Clone, Copy, PartialEq)]
pub enum SortDir { None, Asc, Desc }

impl SortDir {
    pub fn next(self) -> Self {
        match self {
            SortDir::None => SortDir::Asc,
            SortDir::Asc => SortDir::Desc,
            SortDir::Desc => SortDir::None,
        }
    }
    pub fn glyph(self) -> &'static str {
        match self { SortDir::Asc => "▲", SortDir::Desc => "▼", SortDir::None => "" }
    }
}

#[derive(Clone, Copy)]
pub enum ColWidth {
    /// Fixed pixel width.
    Fixed(f32),
    /// Fraction of remaining (0..1).
    Flex(f32),
}

pub struct Column<'a> {
    pub label: &'a str,
    pub width: ColWidth,
    pub sortable: bool,
    pub right_align: bool,
}

impl<'a> Column<'a> {
    pub fn fixed(label: &'a str, w: f32) -> Self {
        Self { label, width: ColWidth::Fixed(w), sortable: false, right_align: false }
    }
    pub fn flex(label: &'a str, frac: f32) -> Self {
        Self { label, width: ColWidth::Flex(frac), sortable: false, right_align: false }
    }
    pub fn sortable(mut self, v: bool) -> Self { self.sortable = v; self }
    pub fn right_align(mut self, v: bool) -> Self { self.right_align = v; self }
}

#[derive(Default, Clone, Copy)]
pub struct TableState {
    pub sort_col: Option<usize>,
    pub sort_dir: SortDir,
    pub selected: Option<usize>,
}

impl Default for SortDir { fn default() -> Self { SortDir::None } }

#[must_use = "Table must be finalized with `.show(ui, ...)` to render"]
pub struct Table<'a, T> {
    columns: &'a [Column<'a>],
    rows: &'a [T],
    row_height: f32,
    striping: bool,
    sticky_first: bool,
    select_col: bool,
    state: &'a mut TableState,
    theme_handle: Option<&'a Theme>,
    theme_dim: Option<Color32>,
    theme_fg: Option<Color32>,
    theme_accent: Option<Color32>,
    theme_border: Option<Color32>,
    theme_bg: Option<Color32>,
}

impl<'a, T> Table<'a, T> {
    pub fn new(columns: &'a [Column<'a>], rows: &'a [T], state: &'a mut TableState) -> Self {
        Self {
            columns, rows, state,
            row_height: 22.0, striping: true, sticky_first: false, select_col: false,
            theme_handle: None,
            theme_dim: None, theme_fg: None, theme_accent: None,
            theme_border: None, theme_bg: None,
        }
    }
    pub fn row_height(mut self, h: f32) -> Self { self.row_height = h; self }
    pub fn striping(mut self, v: bool) -> Self { self.striping = v; self }
    pub fn sticky_first(mut self, v: bool) -> Self { self.sticky_first = v; self }
    pub fn select_col(mut self, v: bool) -> Self { self.select_col = v; self }
    pub fn theme(mut self, t: &'a Theme) -> Self {
        self.theme_handle = Some(t);
        self.theme_dim = Some(t.dim);
        self.theme_fg = Some(t.text);
        self.theme_accent = Some(t.accent);
        self.theme_border = Some(t.toolbar_border);
        self.theme_bg = Some(t.toolbar_bg);
        self
    }

    /// Render. `cell_render(ui, row_idx, col_idx, row_data, cell_rect)` fills
    /// each non-select cell. Returns the outer response of the table area.
    pub fn show<F: FnMut(&mut Ui, usize, usize, &T, egui::Rect)>(
        self, ui: &mut Ui, mut cell_render: F,
    ) -> Response {
        let avail_w = ui.available_width();
        let avail_h = ui.available_height();
        let rect = egui::Rect::from_min_size(
            egui::pos2(ui.cursor().min.x, ui.cursor().min.y),
            egui::vec2(avail_w, avail_h),
        );

        let t = ft();
        let dim = self.theme_dim.unwrap_or(t.dim);
        let fg = self.theme_fg.unwrap_or(t.text);
        let accent = self.theme_accent.unwrap_or(t.accent);
        let border = self.theme_border.unwrap_or(t.toolbar_border);
        let bg = self.theme_bg.unwrap_or(t.toolbar_bg);

        // Compute column x-positions.
        let select_w = if self.select_col { 22.0 } else { 0.0 };
        let mut col_xs: Vec<(f32, f32)> = Vec::with_capacity(self.columns.len());
        let total_inner = (avail_w - select_w).max(0.0);
        let fixed_sum: f32 = self.columns.iter()
            .filter_map(|c| if let ColWidth::Fixed(w) = c.width { Some(w) } else { None })
            .sum();
        let flex_sum: f32 = self.columns.iter()
            .filter_map(|c| if let ColWidth::Flex(f) = c.width { Some(f) } else { None })
            .sum::<f32>().max(1e-6);
        let remaining = (total_inner - fixed_sum).max(0.0);
        let mut x = rect.left() + select_w;
        for c in self.columns {
            let w = match c.width {
                ColWidth::Fixed(w) => w,
                ColWidth::Flex(f) => remaining * (f / flex_sum),
            };
            col_xs.push((x, w));
            x += w;
        }

        // Header.
        let header_h = self.row_height;
        let header_rect = egui::Rect::from_min_size(rect.min, egui::vec2(avail_w, header_h));
        ui.painter().rect_filled(header_rect, 0.0, color_alpha(border, alpha_subtle()));
        for (i, c) in self.columns.iter().enumerate() {
            let (cx, cw) = col_xs[i];
            let cell = egui::Rect::from_min_size(
                egui::pos2(cx, header_rect.min.y), egui::vec2(cw, header_h));
            let resp = ui.allocate_rect(cell, if c.sortable { Sense::click() } else { Sense::hover() });
            let active = self.state.sort_col == Some(i) && self.state.sort_dir != SortDir::None;
            let col_text = if active { accent } else { dim };
            let align = if c.right_align { egui::Align2::RIGHT_CENTER } else { egui::Align2::LEFT_CENTER };
            let pad = 6.0;
            let pos = if c.right_align {
                egui::pos2(cell.right() - pad, cell.center().y)
            } else {
                egui::pos2(cell.left() + pad, cell.center().y)
            };
            ui.painter().text(pos, align, c.label, egui::FontId::monospace(9.0), col_text);
            if active {
                let gx = if c.right_align { cell.right() - pad - (c.label.len() as f32) * 6.0 - 8.0 } else { cell.left() + pad + (c.label.len() as f32) * 6.0 + 4.0 };
                ui.painter().text(egui::pos2(gx, cell.center().y),
                    egui::Align2::CENTER_CENTER,
                    self.state.sort_dir.glyph(), egui::FontId::monospace(8.0), accent);
            }
            if c.sortable && resp.clicked() {
                if self.state.sort_col == Some(i) {
                    self.state.sort_dir = self.state.sort_dir.next();
                    if self.state.sort_dir == SortDir::None { self.state.sort_col = None; }
                } else {
                    self.state.sort_col = Some(i);
                    self.state.sort_dir = SortDir::Asc;
                }
            }
        }
        // Header underline.
        ui.painter().line_segment(
            [egui::pos2(rect.left(), header_rect.bottom()),
             egui::pos2(rect.right(), header_rect.bottom())],
            Stroke::new(stroke_thin(), color_alpha(border, alpha_dim())),
        );

        // Body — vertical scroll.
        let body_rect = egui::Rect::from_min_max(
            egui::pos2(rect.left(), header_rect.bottom()),
            rect.max,
        );
        // Resolve a theme handle for RowShell — fall back to first theme.
        let theme_for_shell: &Theme = match self.theme_handle {
            Some(t) => t,
            None => &crate::chart_renderer::gpu::THEMES[0],
        };
        let select_col = self.select_col;
        let row_height = self.row_height;
        let striping = self.striping;
        let columns_len = self.columns.len();
        // Stable copy of col_xs available to each row body.
        let col_xs_owned = col_xs.clone();
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(body_rect));
        let rows_slice = self.rows;
        let state = self.state;
        egui::ScrollArea::vertical()
            .id_salt("apex_table_body")
            .show(&mut child, |ui| {
                ui.set_min_width(avail_w);
                for (ri, row) in rows_slice.iter().enumerate() {
                    let is_sel = state.selected == Some(ri);
                    let cr = &mut cell_render;
                    let col_xs_ref = &col_xs_owned;
                    let row_resp = super::super::foundation::shell::RowShell::new(theme_for_shell, "")
                        .variant(super::super::foundation::variants::RowVariant::Default)
                        .size(super::super::foundation::tokens::Size::Md)
                        .state(super::super::foundation::interaction::InteractionState::default()
                            .selected(is_sel))
                        .painter_mode(true)
                        .painter_height(row_height)
                        .painter_body(|ui, row_rect| {
                            // Striping band for odd rows when not selected/hovered.
                            if striping && ri % 2 == 1 && !is_sel {
                                ui.painter().rect_filled(row_rect, 0.0,
                                    color_alpha(border, alpha_ghost()));
                            }
                            if select_col {
                                let cb = egui::Rect::from_min_size(
                                    egui::pos2(row_rect.left() + 4.0, row_rect.center().y - 7.0),
                                    egui::vec2(14.0, 14.0));
                                ui.painter().rect_stroke(cb, 2.0,
                                    Stroke::new(stroke_thin(), dim), egui::StrokeKind::Inside);
                                if is_sel {
                                    ui.painter().text(cb.center(), egui::Align2::CENTER_CENTER,
                                        "✓", egui::FontId::monospace(10.0), accent);
                                }
                            }
                            for ci in 0..columns_len {
                                let (cx, cw) = col_xs_ref[ci];
                                let cell_rect = egui::Rect::from_min_size(
                                    egui::pos2(cx, row_rect.min.y), egui::vec2(cw, row_height));
                                cr(ui, ri, ci, row, cell_rect);
                            }
                        })
                        .show(ui);
                    if row_resp.clicked() {
                        state.selected = Some(ri);
                    }
                }
            });

        // Sticky first column shadow (visual only — caller already painted cells).
        if self.sticky_first {
            if let Some(&(cx, cw)) = col_xs.first() {
                let sx = cx + cw;
                ui.painter().line_segment(
                    [egui::pos2(sx, header_rect.bottom()),
                     egui::pos2(sx, body_rect.bottom())],
                    Stroke::new(stroke_thin(), color_alpha(border, alpha_muted())),
                );
            }
        }
        let _ = (fg, bg);

        crate::design_tokens::register_hit(
            [rect.min.x, rect.min.y, rect.width(), rect.height()],
            "TABLE", "Rows");
        ui.allocate_rect(rect, Sense::hover())
    }
}
