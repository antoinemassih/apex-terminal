//! Table — sortable, resizable, virtualized rows.
//!
//! Generic over row data. Caller supplies columns + a row-render
//! callback. Widget handles: header strip, click-to-sort, drag-to-resize
//! columns, scroll virtualization, alternating row tint, hover state.
//!
//! API:
//! ```ignore
//!   #[derive(Clone)]
//!   struct OrderRow { sym: String, qty: u32, price: f32 }
//!
//!   let cols = [
//!       Column::new("Symbol").min_width(80.0).sortable(true),
//!       Column::new("Qty").width(60.0).align(ColAlign::Right),
//!       Column::new("Price").width(80.0).align(ColAlign::Right).sortable(true),
//!   ];
//!
//!   let resp = Table::new(&cols, &orders, &mut state)
//!     .row_height(22.0)
//!     .alternate_rows(true)
//!     .row_render(|ui, theme, row, col_idx, col_rect| {
//!         match col_idx {
//!             0 => { Label::new(&row.sym).number().show(ui, theme); }
//!             1 => { Label::new(&format!("{}", row.qty)).show(ui, theme); }
//!             2 => { Label::new(&format!("{:.2}", row.price)).show(ui, theme); }
//!             _ => unreachable!(),
//!         };
//!     })
//!     .show(ui, theme);
//! ```
//!
//! Sort lifecycle: header clicks cycle `None → Asc → Desc → None`. The
//! widget only flips state and reports `sort_changed = true`. The caller
//! is responsible for reordering `rows` based on `state.sort_col` /
//! `state.sort_dir` before the next frame.
//!
//! Column resize: drag the 4-px hit zone on the right edge of any column
//! to update `state.col_widths[i]`. The vector persists across frames; if
//! it is empty the widget seeds it from the column specs on the first
//! frame.
//!
//! Virtualization: body is wrapped in `egui::ScrollArea::vertical()` and
//! uses `show_rows` so only visible rows allocate. Constant cost per
//! frame regardless of row count.

#![allow(clippy::too_many_arguments)]

use egui::{CursorIcon, FontFamily, FontId, Id, Pos2, Rect, Response, Sense, Stroke, Ui, Vec2};

use super::motion;
use super::theme::ComponentTheme;
use super::tooltip::Tooltip;

use crate::chart_renderer::ui::style::{
    alpha_muted, alpha_tint, color_alpha, font_sm, font_xs, gap_2xs, gap_xs, stroke_thin,
    ALPHA_GHOST,
};

// ── Public types ────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ColAlign {
    #[default]
    Left,
    Center,
    Right,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SortDir {
    #[default]
    None,
    Asc,
    Desc,
}

impl SortDir {
    pub fn next(self) -> Self {
        match self {
            SortDir::None => SortDir::Asc,
            SortDir::Asc => SortDir::Desc,
            SortDir::Desc => SortDir::None,
        }
    }
    fn glyph(self) -> Option<&'static str> {
        match self {
            SortDir::Asc => Some("▲"),
            SortDir::Desc => Some("▼"),
            SortDir::None => None,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum ColWidth {
    Fixed(f32),
    Min(f32),
    Flex(f32),
}

#[derive(Clone)]
pub struct Column<'a> {
    pub label: &'a str,
    pub width: ColWidth,
    pub align: ColAlign,
    pub sortable: bool,
    pub icon: Option<&'static str>,
    pub tooltip: Option<&'a str>,
}

impl<'a> Column<'a> {
    pub fn new(label: &'a str) -> Self {
        Self {
            label,
            width: ColWidth::Min(60.0),
            align: ColAlign::Left,
            sortable: false,
            icon: None,
            tooltip: None,
        }
    }
    pub fn width(mut self, w: f32) -> Self {
        self.width = ColWidth::Fixed(w);
        self
    }
    pub fn min_width(mut self, w: f32) -> Self {
        self.width = ColWidth::Min(w);
        self
    }
    pub fn flex(mut self, ratio: f32) -> Self {
        self.width = ColWidth::Flex(ratio);
        self
    }
    pub fn align(mut self, a: ColAlign) -> Self {
        self.align = a;
        self
    }
    pub fn sortable(mut self, v: bool) -> Self {
        self.sortable = v;
        self
    }
    pub fn icon(mut self, icon: &'static str) -> Self {
        self.icon = Some(icon);
        self
    }
    pub fn tooltip(mut self, text: &'a str) -> Self {
        self.tooltip = Some(text);
        self
    }
}

#[derive(Clone, Default)]
pub struct TableState {
    pub sort_col: Option<usize>,
    pub sort_dir: SortDir,
    pub col_widths: Vec<f32>,
    pub selected_row: Option<usize>,
    pub hovered_row: Option<usize>,
}

pub struct TableResponse {
    pub response: Response,
    pub sort_changed: bool,
    pub column_resized: Option<usize>,
    pub row_clicked: Option<usize>,
}

// ── Builder ─────────────────────────────────────────────────────────────

type RowRenderFn<'a, T> = Box<dyn Fn(&mut Ui, &dyn ComponentTheme, &T, usize, Rect) + 'a>;
type RowClickFn<'a, T> = Box<dyn FnMut(usize, &T) + 'a>;

#[must_use = "Table must be finalized with `.show(ui, theme)` to render"]
pub struct Table<'a, T: Clone> {
    columns: &'a [Column<'a>],
    rows: &'a [T],
    state: &'a mut TableState,
    row_height: f32,
    header_height: f32,
    alternate_rows: bool,
    hover_row: bool,
    selectable_rows: bool,
    resizable: bool,
    show_header: bool,
    empty_state: Option<String>,
    row_render: Option<RowRenderFn<'a, T>>,
    row_click: Option<RowClickFn<'a, T>>,
}

impl<'a, T: Clone> Table<'a, T> {
    pub fn new(columns: &'a [Column<'a>], rows: &'a [T], state: &'a mut TableState) -> Self {
        Self {
            columns,
            rows,
            state,
            row_height: 22.0,
            header_height: 28.0,
            alternate_rows: false,
            hover_row: true,
            selectable_rows: false,
            resizable: true,
            show_header: true,
            empty_state: None,
            row_render: None,
            row_click: None,
        }
    }

    pub fn row_height(mut self, h: f32) -> Self {
        self.row_height = h;
        self
    }
    pub fn header_height(mut self, h: f32) -> Self {
        self.header_height = h;
        self
    }
    pub fn alternate_rows(mut self, v: bool) -> Self {
        self.alternate_rows = v;
        self
    }
    pub fn striped(self, v: bool) -> Self {
        self.alternate_rows(v)
    }
    pub fn hover_row(mut self, v: bool) -> Self {
        self.hover_row = v;
        self
    }
    pub fn selectable_rows(mut self, v: bool) -> Self {
        self.selectable_rows = v;
        self
    }
    pub fn resizable(mut self, v: bool) -> Self {
        self.resizable = v;
        self
    }
    pub fn show_header(mut self, v: bool) -> Self {
        self.show_header = v;
        self
    }
    pub fn empty_state(mut self, text: impl Into<String>) -> Self {
        self.empty_state = Some(text.into());
        self
    }
    pub fn row_render(
        mut self,
        f: impl Fn(&mut Ui, &dyn ComponentTheme, &T, usize, Rect) + 'a,
    ) -> Self {
        self.row_render = Some(Box::new(f));
        self
    }
    pub fn row_click(mut self, f: impl FnMut(usize, &T) + 'a) -> Self {
        self.row_click = Some(Box::new(f));
        self
    }

    pub fn show(mut self, ui: &mut Ui, theme: &dyn ComponentTheme) -> TableResponse {
        let avail_w = ui.available_width();
        let avail_h = ui.available_height();
        let origin = ui.cursor().min;
        let outer = Rect::from_min_size(origin, Vec2::new(avail_w, avail_h));

        // Resolve column widths (seed state on first frame, then trust it).
        let col_widths = resolve_widths(self.columns, &self.state.col_widths, avail_w);
        if self.state.col_widths.len() != self.columns.len() {
            self.state.col_widths = col_widths.clone();
        } else {
            // Keep stored widths but clamp pathological values.
            for (i, w) in self.state.col_widths.iter_mut().enumerate() {
                if !w.is_finite() || *w < 16.0 {
                    *w = col_widths[i].max(16.0);
                }
            }
        }
        let widths: Vec<f32> = self.state.col_widths.clone();
        let col_xs: Vec<f32> = {
            let mut xs = Vec::with_capacity(widths.len());
            let mut x = outer.left();
            for w in &widths {
                xs.push(x);
                x += *w;
            }
            xs
        };

        let mut sort_changed = false;
        let mut column_resized: Option<usize> = None;
        let mut row_clicked: Option<usize> = None;

        // ── Header ──────────────────────────────────────────────────────
        let header_h = if self.show_header { self.header_height } else { 0.0 };
        if self.show_header {
            let header_rect = Rect::from_min_size(outer.min, Vec2::new(avail_w, header_h));
            ui.painter()
                .rect_filled(header_rect, 0.0, color_alpha(theme.surface(), 200));

            // Bottom border.
            ui.painter().line_segment(
                [
                    Pos2::new(header_rect.left(), header_rect.bottom()),
                    Pos2::new(header_rect.right(), header_rect.bottom()),
                ],
                Stroke::new(stroke_thin(), theme.border()),
            );

            for (i, col) in self.columns.iter().enumerate() {
                let cx = col_xs[i];
                let cw = widths[i];
                let cell = Rect::from_min_size(
                    Pos2::new(cx, header_rect.min.y),
                    Vec2::new(cw, header_h),
                );

                let sense = if col.sortable {
                    Sense::click()
                } else {
                    Sense::hover()
                };
                let resp = ui.interact(cell, ui.id().with(("apex_table_hdr", i)), sense);

                let is_sorted = self.state.sort_col == Some(i)
                    && self.state.sort_dir != SortDir::None;
                let label_color = if is_sorted {
                    theme.accent()
                } else if resp.hovered() && col.sortable {
                    theme.text()
                } else {
                    theme.dim()
                };

                let pad_x = gap_xs();
                let glyph = if is_sorted {
                    self.state.sort_dir.glyph()
                } else if col.sortable && resp.hovered() {
                    Some("▾")
                } else {
                    None
                };
                let glyph_color = if is_sorted {
                    theme.accent()
                } else {
                    color_alpha(theme.dim(), 140)
                };

                let font = FontId::new(font_xs(), FontFamily::Proportional);
                let glyph_w = if glyph.is_some() { 12.0 } else { 0.0 };

                match col.align {
                    ColAlign::Left => {
                        let mut tx = cell.left() + pad_x;
                        if let Some(icon) = col.icon {
                            ui.painter().text(
                                Pos2::new(tx, cell.center().y),
                                egui::Align2::LEFT_CENTER,
                                icon,
                                font.clone(),
                                label_color,
                            );
                            tx += 14.0;
                        }
                        let label_pos = Pos2::new(tx, cell.center().y);
                        let label_rect = ui.painter().text(
                            label_pos,
                            egui::Align2::LEFT_CENTER,
                            col.label,
                            font.clone(),
                            label_color,
                        );
                        if let Some(g) = glyph {
                            ui.painter().text(
                                Pos2::new(label_rect.right() + 3.0, cell.center().y),
                                egui::Align2::LEFT_CENTER,
                                g,
                                font.clone(),
                                glyph_color,
                            );
                        }
                    }
                    ColAlign::Center => {
                        ui.painter().text(
                            cell.center(),
                            egui::Align2::CENTER_CENTER,
                            col.label,
                            font.clone(),
                            label_color,
                        );
                    }
                    ColAlign::Right => {
                        let right = cell.right() - gap_2xs();
                        let mut x = right;
                        if let Some(g) = glyph {
                            ui.painter().text(
                                Pos2::new(x, cell.center().y),
                                egui::Align2::RIGHT_CENTER,
                                g,
                                font.clone(),
                                glyph_color,
                            );
                            x -= glyph_w;
                        }
                        ui.painter().text(
                            Pos2::new(x - 2.0, cell.center().y),
                            egui::Align2::RIGHT_CENTER,
                            col.label,
                            font.clone(),
                            label_color,
                        );
                    }
                }

                // Click → cycle sort.
                if col.sortable && resp.clicked() {
                    if self.state.sort_col == Some(i) {
                        self.state.sort_dir = self.state.sort_dir.next();
                        if self.state.sort_dir == SortDir::None {
                            self.state.sort_col = None;
                        }
                    } else {
                        self.state.sort_col = Some(i);
                        self.state.sort_dir = SortDir::Asc;
                    }
                    sort_changed = true;
                }

                // Tooltip.
                if let Some(tip) = col.tooltip {
                    if resp.hovered() {
                        Tooltip::new(tip).show(ui, &resp, theme);
                    }
                }

                // Resize handle on the right edge.
                if self.resizable && i < self.columns.len() {
                    let handle_rect = Rect::from_min_max(
                        Pos2::new(cell.right() - 2.0, header_rect.top()),
                        Pos2::new(cell.right() + 2.0, outer.bottom()),
                    );
                    let resize_id = ui.id().with(("apex_table_resize", i));
                    let h_resp = ui.interact(handle_rect, resize_id, Sense::click_and_drag());
                    if h_resp.hovered() || h_resp.dragged() {
                        ui.ctx().set_cursor_icon(CursorIcon::ResizeHorizontal);
                        // Subtle hover indicator.
                        let t = motion::ease_bool(
                            ui.ctx(),
                            resize_id.with("hover"),
                            h_resp.hovered() || h_resp.dragged(),
                            motion::FAST,
                        );
                        let line_color = motion::lerp_color(
                            color_alpha(theme.border(), alpha_muted()),
                            theme.accent(),
                            t,
                        );
                        ui.painter().line_segment(
                            [
                                Pos2::new(cell.right(), header_rect.top() + 4.0),
                                Pos2::new(cell.right(), header_rect.bottom() - 4.0),
                            ],
                            Stroke::new(stroke_thin(), line_color),
                        );
                    }
                    if h_resp.dragged() {
                        let dx = h_resp.drag_delta().x;
                        if dx.abs() > 0.0 {
                            let new_w = (self.state.col_widths[i] + dx).max(24.0);
                            self.state.col_widths[i] = new_w;
                            column_resized = Some(i);
                        }
                    }
                }
            }
        }

        // Allocate the header strip space so following content sits below it.
        if self.show_header {
            let _ = ui.allocate_exact_size(Vec2::new(avail_w, header_h), Sense::hover());
        }

        // ── Body ────────────────────────────────────────────────────────
        let body_top = outer.top() + header_h;
        let body_rect = Rect::from_min_max(
            Pos2::new(outer.left(), body_top),
            Pos2::new(outer.right(), outer.bottom()),
        );

        if self.rows.is_empty() {
            if let Some(text) = &self.empty_state {
                let font = FontId::new(font_sm(), FontFamily::Proportional);
                ui.painter().text(
                    body_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    text,
                    font,
                    theme.dim(),
                );
            }
            let response = ui.allocate_rect(outer, Sense::hover());
            return TableResponse {
                response,
                sort_changed,
                column_resized,
                row_clicked,
            };
        }

        // Reset hovered_row each frame; will be re-set below if any row is hovered.
        let prev_hovered = self.state.hovered_row.take();
        let _ = prev_hovered;

        let row_h = self.row_height;
        let total_w: f32 = widths.iter().sum();

        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(body_rect));
        let row_count = self.rows.len();
        let rows_slice = self.rows;
        let render = self.row_render.take();
        let mut click_cb = self.row_click.take();
        let alt = self.alternate_rows;
        let hover_row = self.hover_row;
        let selectable = self.selectable_rows;
        let cols_len = self.columns.len();
        let state = &mut *self.state;

        egui::ScrollArea::vertical()
            .id_salt("apex_table_body")
            .auto_shrink([false; 2])
            .show_rows(&mut child, row_h, row_count, |ui, range| {
                ui.set_min_width(total_w);
                for ri in range {
                    let row = &rows_slice[ri];
                    let y = body_rect.top() + (ri as f32) * row_h;
                    let row_rect =
                        Rect::from_min_size(Pos2::new(body_rect.left(), y), Vec2::new(total_w, row_h));

                    let row_id = ui.id().with(("apex_table_row", ri));
                    let sense = if selectable {
                        Sense::click()
                    } else {
                        Sense::hover()
                    };
                    let row_resp = ui.interact(row_rect, row_id, sense);

                    if row_resp.hovered() {
                        state.hovered_row = Some(ri);
                    }
                    let is_sel = selectable && state.selected_row == Some(ri);

                    // Background paint order: alternate → hover → selected.
                    if alt && ri % 2 == 1 && !is_sel {
                        ui.painter().rect_filled(
                            row_rect,
                            0.0,
                            color_alpha(theme.surface(), 60),
                        );
                    }

                    if hover_row && !is_sel {
                        let t = motion::ease_bool(
                            ui.ctx(),
                            row_id.with("hover"),
                            row_resp.hovered(),
                            motion::FAST,
                        );
                        if t > 0.0 {
                            let bg = color_alpha(theme.text(), 18);
                            // Multiply alpha by t for fade.
                            let bg =
                                egui::Color32::from_rgba_unmultiplied(bg.r(), bg.g(), bg.b(), ((bg.a() as f32) * t) as u8);
                            ui.painter().rect_filled(row_rect, 0.0, bg);
                        }
                    }

                    if is_sel {
                        ui.painter().rect_filled(
                            row_rect,
                            0.0,
                            color_alpha(theme.accent(), ALPHA_GHOST),
                        );
                    }

                    // Per-cell render.
                    if let Some(render) = &render {
                        for ci in 0..cols_len {
                            let cx = col_xs[ci];
                            let cw = widths[ci];
                            let cell_rect =
                                Rect::from_min_size(Pos2::new(cx, y), Vec2::new(cw, row_h));
                            let mut cell_ui =
                                ui.new_child(egui::UiBuilder::new().max_rect(cell_rect.shrink2(
                                    Vec2::new(gap_xs(), gap_2xs()),
                                )));
                            render(&mut cell_ui, theme, row, ci, cell_rect);
                        }
                    }

                    if selectable && row_resp.clicked() {
                        state.selected_row = Some(ri);
                        row_clicked = Some(ri);
                        if let Some(cb) = click_cb.as_mut() {
                            cb(ri, row);
                        }
                    }
                }
                let _ = alpha_tint();
                let _ = Id::NULL;
            });

        let response = ui.allocate_rect(outer, Sense::hover());
        TableResponse {
            response,
            sort_changed,
            column_resized,
            row_clicked,
        }
    }
}

// ── Width resolution ────────────────────────────────────────────────────

fn resolve_widths(columns: &[Column<'_>], stored: &[f32], avail: f32) -> Vec<f32> {
    if stored.len() == columns.len() && stored.iter().all(|w| w.is_finite() && *w >= 16.0) {
        return stored.to_vec();
    }
    let n = columns.len();
    if n == 0 {
        return Vec::new();
    }
    let mut widths = vec![0.0_f32; n];
    let mut fixed_total = 0.0;
    let mut min_total = 0.0;
    let mut flex_total = 0.0;
    for (i, c) in columns.iter().enumerate() {
        match c.width {
            ColWidth::Fixed(w) => {
                widths[i] = w;
                fixed_total += w;
            }
            ColWidth::Min(w) => {
                widths[i] = w;
                min_total += w;
            }
            ColWidth::Flex(_) => {}
        }
        if let ColWidth::Flex(r) = c.width {
            flex_total += r.max(0.0);
        }
    }
    let used = fixed_total + min_total;
    let remaining = (avail - used).max(0.0);

    if flex_total > 0.0 {
        // Distribute remaining among Flex.
        for (i, c) in columns.iter().enumerate() {
            if let ColWidth::Flex(r) = c.width {
                widths[i] = remaining * (r.max(0.0) / flex_total);
            }
        }
    } else if remaining > 0.0 {
        // No flex columns — distribute slack across Min columns proportionally.
        let min_count = columns
            .iter()
            .filter(|c| matches!(c.width, ColWidth::Min(_)))
            .count();
        if min_count > 0 {
            let extra = remaining / (min_count as f32);
            for (i, c) in columns.iter().enumerate() {
                if matches!(c.width, ColWidth::Min(_)) {
                    widths[i] += extra;
                }
            }
        }
    }
    widths
}
