//! HeatmapGrid — symbol heatmap row/cell renderer.
//!
//! Renders a horizontal grid of cells where each cell shows a symbol + change%,
//! tinted by direction (bull/bear) at intensity proportional to magnitude.
//! Includes hover highlight, active-symbol border, a proportional bar fill,
//! and a left-edge accent strip.
//!
//! ```ignore
//! HeatmapGrid::new(&cells)
//!     .num_cols(3)
//!     .active_symbol(Some("AAPL"))
//!     .show(ui, theme);
//! ```

use egui::{Color32, Response, Ui};
use super::theme::ComponentTheme;
use crate::ui_kit::tokens as st;

// ── Data ──────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct HeatmapCell {
    pub symbol: String,
    pub change_pct: f32,
}

// ── Widget ────────────────────────────────────────────────────────────────────

#[must_use = "HeatmapGrid does nothing until `.show(ui, theme)` is called"]
pub struct HeatmapGrid<'a> {
    cells: &'a [HeatmapCell],
    num_cols: usize,
    active_symbol: Option<&'a str>,
    on_click: Option<Box<dyn FnMut(&str) + 'a>>,
}

impl<'a> HeatmapGrid<'a> {
    pub fn new(cells: &'a [HeatmapCell]) -> Self {
        Self {
            cells,
            num_cols: 3,
            active_symbol: None,
            on_click: None,
        }
    }

    pub fn num_cols(mut self, n: usize) -> Self { self.num_cols = n; self }
    pub fn active_symbol(mut self, s: Option<&'a str>) -> Self { self.active_symbol = s; self }
    pub fn on_click<F: FnMut(&str) + 'a>(mut self, f: F) -> Self { self.on_click = Some(Box::new(f)); self }

    /// Render the grid. Mirrors the exact pixel geometry from heat_panel.rs:147-220.
    pub fn show(mut self, ui: &mut Ui, theme: &dyn ComponentTheme) -> Response {
        let items = self.cells;
        let num_cols = self.num_cols.max(1);

        let avail_w = ui.available_width();
        let gap = 3.0_f32;
        let col_w = (avail_w - gap * (num_cols - 1) as f32) / num_cols as f32;
        let cell_h = if num_cols == 1 { 26.0_f32 } else { 28.0_f32 };
        let font_sz = if num_cols >= 3 { 10.0_f32 } else { 12.0_f32 };
        let max_pct = items.iter().map(|i| i.change_pct.abs()).fold(1.0_f32, f32::max);
        let rows = (items.len() + num_cols - 1) / num_cols;
        let total_h = rows as f32 * cell_h;

        let (rect, resp) = ui.allocate_exact_size(egui::vec2(avail_w, total_h), egui::Sense::click());
        let painter = ui.painter();

        // Hover detection — find which cell the mouse is over
        let hover_idx: Option<usize> = ui.input(|i| i.pointer.hover_pos()).and_then(|pos| {
            if !rect.contains(pos) { return None; }
            let col = ((pos.x - rect.left()) / (col_w + gap)).floor() as usize;
            let row = ((pos.y - rect.top()) / cell_h).floor() as usize;
            let idx = row * num_cols + col;
            if idx < items.len() { Some(idx) } else { None }
        });
        if hover_idx.is_some() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }

        // Click detection
        let mut clicked_symbol: Option<String> = None;
        if resp.clicked() {
            if let Some(pos) = resp.interact_pointer_pos() {
                let col = ((pos.x - rect.left()) / (col_w + gap)).floor() as usize;
                let row = ((pos.y - rect.top()) / cell_h).floor() as usize;
                let idx = row * num_cols + col;
                if let Some(item) = items.get(idx) {
                    clicked_symbol = Some(item.symbol.clone());
                }
            }
        }
        if let (Some(sym), Some(cb)) = (clicked_symbol.as_deref(), self.on_click.as_mut()) {
            cb(sym);
        }

        for (i, item) in items.iter().enumerate() {
            let col = i % num_cols;
            let row = i / num_cols;
            let cx = rect.left() + col as f32 * (col_w + gap);
            let cy = rect.top() + row as f32 * cell_h;
            let intensity = (item.change_pct.abs() / 5.0).min(1.0);
            let is_up = item.change_pct >= 0.0;
            let is_active = self.active_symbol.map_or(false, |s| s == item.symbol);
            let is_hovered = hover_idx == Some(i);

            // Hover highlight
            if is_hovered {
                painter.rect_filled(
                    egui::Rect::from_min_size(egui::pos2(cx, cy), egui::vec2(col_w, cell_h)),
                    2.0, st::color_alpha(theme.text(), 12));
            }
            // Active symbol border
            if is_active {
                painter.rect_stroke(
                    egui::Rect::from_min_size(egui::pos2(cx, cy + 1.0), egui::vec2(col_w, cell_h - 2.0)),
                    2.0,
                    egui::Stroke::new(st::stroke_bold(), theme.accent()),
                    egui::StrokeKind::Outside);
            }
            // Background bar (proportional to peer magnitude)
            let bar_frac = if max_pct > 0.0 { item.change_pct.abs() / max_pct } else { 0.0 };
            let bar_w = bar_frac * col_w * 0.6;
            let bar_col = if is_up {
                st::color_alpha(theme.bull(), (25.0 + intensity * 55.0) as u8)
            } else {
                st::color_alpha(theme.bear(), (25.0 + intensity * 55.0) as u8)
            };
            painter.rect_filled(
                egui::Rect::from_min_size(egui::pos2(cx, cy + 1.0), egui::vec2(bar_w, cell_h - 2.0)),
                2.0, bar_col);
            // Left-edge accent strip
            let edge_a = (120.0 + intensity * 135.0) as u8;
            let edge_col = if is_up { st::color_alpha(theme.bull(), edge_a) } else { st::color_alpha(theme.bear(), edge_a) };
            painter.rect_filled(
                egui::Rect::from_min_size(egui::pos2(cx, cy + 1.0), egui::vec2(3.0, cell_h - 2.0)),
                0.0, edge_col);
            // Symbol text
            let sym_col: Color32 = if is_active {
                theme.text()
            } else if is_hovered {
                st::color_alpha(theme.text(), 230)
            } else {
                st::color_alpha(theme.text(), 190)
            };
            painter.text(
                egui::pos2(cx + 7.0, cy + cell_h / 2.0),
                egui::Align2::LEFT_CENTER,
                &item.symbol,
                egui::FontId::monospace(font_sz),
                sym_col);
            // Change% text
            let chg_col = if is_up { theme.bull() } else { theme.bear() };
            painter.text(
                egui::pos2(cx + col_w - 3.0, cy + cell_h / 2.0),
                egui::Align2::RIGHT_CENTER,
                &format!("{:+.1}%", item.change_pct),
                egui::FontId::monospace(font_sz),
                chg_col);
        }

        resp
    }
}
