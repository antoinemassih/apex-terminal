//! TableHeader — column-header strip for streaming-data panels.
//!
//! Renders a single row of labelled, fixed-or-weighted column headers above a
//! [`PanelListRow`] column-mode list. The column spec mirrors the `Column`
//! slice API so the header and the data rows share the same width math and
//! never drift out of alignment.
//!
//! ```ignore
//! TableHeader::new()
//!     .column("Time",   80.0, false)
//!     .column("Price",  90.0, true)
//!     .column("Size",   60.0, true)
//!     .show(ui, t);
//! ```
//!
//! Each column declaration matches one `Column` in the companion rows:
//! - `label` — header label text (uppercase recommended but not enforced).
//! - `width` — fixed pixel width reserved for the column (same as
//!   [`Column::min_width`] in `PanelListRow` columns mode).
//! - `right_align` — when `true`, the label is right-aligned inside the cell,
//!   matching numeric/price columns that typically use `ColAlign::Right`.
//!
//! Visual spec:
//! - Height: 18 px (denser than a data row to read as a heading, not a peer).
//! - Typography: `font_xs` proportional in `t.dim` (muted to recede behind
//!   data rows).
//! - Background: `color_alpha(t.toolbar_border, alpha_ghost())` — the same
//!   hairline-dark strip used by section sub-headers.
//! - Bottom hairline: `stroke_thin()` at `color_alpha(t.toolbar_border, 60)`.
//! - Columns are laid out with the same gap/padding math as
//!   `PanelListRow::paint_columns` so alignment is pixel-exact.
//!
//! When to use:
//! - Directly above a [`PanelListRow`] list that uses `.columns(&[...])`.
//! - Streaming panels: tape T&S, scanner, journal.
//!
//! When NOT to use:
//! - `ui_kit::Table` already provides its own header — don't double-up.
//! - Header-less lists (watchlist rows, alert rows) — use `PanelSection`'s
//!   title instead.
//!
//! Sister widgets: [`PanelListRow`], [`PanelSection`], `ui_kit::Table`.

use egui::{Color32, FontId, Pos2, Rect, Sense, Ui, Vec2};

use crate::chart::renderer::ui::style::{
    alpha_ghost, color_alpha, font_xs, gap_md, gap_xs, stroke_thin,
};
use crate::ui_kit::widgets::theme::Theme;

/// One column definition. Mirrors the sizing fields of
/// `panel_list_row::Column` so callers can declare both header and data rows
/// from the same set of constants.
#[derive(Clone, Debug)]
struct ColSpec {
    label: &'static str,
    width: f32,
    right_align: bool,
}

/// Height of a `TableHeader` row in pixels.
const HEADER_H: f32 = 18.0;

#[must_use = "TableHeader must be rendered with `.show(...)`"]
pub struct TableHeader {
    cols: Vec<ColSpec>,
}

impl TableHeader {
    pub fn new() -> Self {
        Self { cols: Vec::new() }
    }

    /// Add a column to the header strip.
    ///
    /// - `label` — display text for the column.
    /// - `width` — fixed pixel width (should match the corresponding
    ///   `Column::min_width` in the companion `PanelListRow` rows).
    /// - `right_align` — align label to the right of its cell (numeric cols).
    pub fn column(mut self, label: &'static str, width: f32, right_align: bool) -> Self {
        self.cols.push(ColSpec { label, width, right_align });
        self
    }

    pub fn show(self, ui: &mut Ui, t: &Theme) {
        if self.cols.is_empty() {
            return;
        }
        let avail_w = ui.available_width();
        let (rect, _) = ui.allocate_exact_size(Vec2::new(avail_w, HEADER_H), Sense::hover());

        if !ui.is_rect_visible(rect) {
            return;
        }

        let painter = ui.painter_at(rect);

        // Strip background — same tint as PanelSubSection / section sub-headers.
        painter.rect_filled(rect, 0.0, color_alpha(t.toolbar_border, alpha_ghost()));

        // Bottom hairline rule.
        let y = rect.bottom() - 0.5;
        painter.line_segment(
            [Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
            egui::Stroke::new(stroke_thin(), color_alpha(t.toolbar_border, 60)),
        );

        // Column layout: same padding math as PanelListRow::paint_columns.
        let inner_left = rect.left() + gap_md();
        let inner_right = rect.right() - gap_md();
        let content_w = (inner_right - inner_left).max(0.0);
        let n = self.cols.len();
        let gap = gap_xs();
        let total_gaps = gap * (n as f32 - 1.0).max(0.0);

        // Distribute remaining flex width equally among columns that have no
        // explicit min_width reservation — here every column has a fixed width,
        // so flex = leftover after fixed reservations.
        let fixed_total: f32 = self.cols.iter().map(|c| c.width).sum();
        let flex_w = (content_w - total_gaps - fixed_total).max(0.0);
        // Spread leftover evenly — avoids the last column disappearing off-screen.
        let per_col_flex = if n > 0 { flex_w / n as f32 } else { 0.0 };

        let cy = rect.center().y;
        let mut x = inner_left;
        let col_color = color_alpha(t.dim, 180); // muted but readable

        for (i, c) in self.cols.iter().enumerate() {
            let cell_w = c.width + per_col_flex;
            let cell_rect = Rect::from_min_max(
                Pos2::new(x, rect.top()),
                Pos2::new(x + cell_w, rect.bottom()),
            );

            let font = FontId::proportional(font_xs());
            let galley = ui.fonts(|f| {
                f.layout_no_wrap(c.label.to_uppercase(), font, col_color)
            });
            let tw = galley.size().x;
            let th = galley.size().y;
            let tx = if c.right_align {
                cell_rect.right() - tw
            } else {
                cell_rect.left()
            };

            let cell_painter = painter.with_clip_rect(cell_rect);
            cell_painter.galley(Pos2::new(tx, cy - th * 0.5), galley, col_color);

            x += cell_w;
            if i + 1 < n {
                x += gap;
            }
        }
    }
}

impl Default for TableHeader {
    fn default() -> Self { Self::new() }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Builder round-trip: columns are stored in declaration order.
    #[test]
    fn builder_stores_columns_in_order() {
        let th = TableHeader::new()
            .column("TIME",  80.0, false)
            .column("PRICE", 90.0, true)
            .column("SIZE",  60.0, true);
        assert_eq!(th.cols.len(), 3);
        assert_eq!(th.cols[0].label, "TIME");
        assert!(!th.cols[0].right_align);
        assert_eq!(th.cols[1].label, "PRICE");
        assert!(th.cols[1].right_align);
        assert_eq!(th.cols[2].label, "SIZE");
        assert_eq!(th.cols[2].width, 60.0);
    }

    /// Empty header: no columns stored, show() is a no-op.
    #[test]
    fn empty_header_is_valid() {
        let th = TableHeader::new();
        assert!(th.cols.is_empty());
    }

    /// Width field is stored verbatim.
    #[test]
    fn column_width_preserved() {
        let th = TableHeader::new().column("LABEL", 123.4, false);
        assert!((th.cols[0].width - 123.4).abs() < f32::EPSILON);
    }
}
