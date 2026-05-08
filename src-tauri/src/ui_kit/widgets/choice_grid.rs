//! ChoiceGrid — grid picker with shared hairlines.
//!
//! Zed pattern: VS Code / JetBrains / Sublime / Atom / Emacs / Cursor
//! arranged 3×2. Each cell is icon + label. Selected cell highlights.
//! Adjacent cells share hairline borders (dividers belong to one cell,
//! not double-painted).
//!
//! API:
//!   let mut idx = 0;
//!   ChoiceGrid::new(&mut idx, &items)
//!     .columns(3)
//!     .show(ui, theme);

use egui::{Color32, CornerRadius, FontId, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Ui, Vec2};

use super::label::Label;
use super::theme::ComponentTheme;
use super::tokens::Size;

const DEFAULT_CELL: Vec2 = Vec2::new(96.0, 56.0);

#[derive(Clone, Copy)]
pub struct ChoiceItem<'a> {
    pub label: &'a str,
    pub icon: &'static str,
}

impl<'a> ChoiceItem<'a> {
    pub fn new(label: &'a str, icon: &'static str) -> Self {
        Self { label, icon }
    }
}

#[must_use = "ChoiceGrid does nothing until `.show(ui, theme)` is called"]
pub struct ChoiceGrid<'a> {
    selected: &'a mut usize,
    items: &'a [ChoiceItem<'a>],
    columns: usize,
    cell_size: Vec2,
}

impl<'a> ChoiceGrid<'a> {
    pub fn new(selected: &'a mut usize, items: &'a [ChoiceItem<'a>]) -> Self {
        Self {
            selected,
            items,
            columns: 3,
            cell_size: DEFAULT_CELL,
        }
    }

    pub fn columns(mut self, n: usize) -> Self {
        self.columns = n.max(1);
        self
    }

    pub fn cell_size(mut self, size: Vec2) -> Self {
        self.cell_size = size;
        self
    }

    pub fn show(self, ui: &mut Ui, theme: &dyn ComponentTheme) -> Response {
        let Self { selected, items, columns, cell_size } = self;

        let n = items.len();
        let rows = if n == 0 { 0 } else { (n + columns - 1) / columns };
        let total = Vec2::new(
            cell_size.x * columns as f32,
            cell_size.y * rows as f32,
        );

        let (rect, mut response) = ui.allocate_exact_size(total, Sense::hover());
        if n == 0 || !ui.is_rect_visible(rect) {
            return response;
        }

        let painter = ui.painter_at(rect);
        let divider = theme.border_variant();
        let divider_stroke = Stroke::new(1.0, divider);

        // Per-cell pass: detect hover/click, paint cell background + inner
        // dividers (right + bottom edges, except boundary cells).
        for (i, item) in items.iter().enumerate() {
            let r = i / columns;
            let c = i % columns;
            let is_last_col = c == columns - 1 || i == n - 1;
            let is_last_row = r == rows - 1;
            let cell_rect = Rect::from_min_size(
                Pos2::new(rect.left() + c as f32 * cell_size.x, rect.top() + r as f32 * cell_size.y),
                cell_size,
            );

            let id = ui.id().with(("choice_grid_cell", i));
            let cell_resp = ui.interact(cell_rect, id, Sense::click());

            // Background: selected wins over hover.
            let is_selected = *selected == i;
            if is_selected {
                painter.rect_filled(cell_rect, CornerRadius::ZERO, theme.element_selected());
            } else if cell_resp.hovered() {
                painter.rect_filled(cell_rect, CornerRadius::ZERO, theme.element_hover());
            }

            // Icon centered in the upper portion of the cell.
            let icon_size = 18.0;
            let label_h = crate::chart_renderer::ui::style::font_xs() + 2.0;
            let label_gap = 4.0;
            let content_h = icon_size + label_gap + label_h;
            let content_top = cell_rect.center().y - content_h * 0.5;
            let icon_center = Pos2::new(cell_rect.center().x, content_top + icon_size * 0.5);
            painter.text(
                icon_center,
                egui::Align2::CENTER_CENTER,
                item.icon,
                FontId::proportional(icon_size),
                if is_selected { theme.icon_accent() } else { theme.icon() },
            );

            // Label below the icon.
            let label_rect = Rect::from_min_size(
                Pos2::new(cell_rect.left(), content_top + icon_size + label_gap),
                Vec2::new(cell_rect.width(), label_h),
            );
            let mut child = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(label_rect)
                    .layout(egui::Layout::centered_and_justified(egui::Direction::LeftToRight)),
            );
            Label::new(item.label).size(Size::Xs).muted().truncate(true).show(&mut child, theme);

            // Inner dividers — each cell paints its right + bottom edge,
            // but only when not on the trailing column / row.
            if !is_last_col {
                painter.line_segment(
                    [Pos2::new(cell_rect.right(), cell_rect.top()), Pos2::new(cell_rect.right(), cell_rect.bottom())],
                    divider_stroke,
                );
            }
            if !is_last_row {
                painter.line_segment(
                    [Pos2::new(cell_rect.left(), cell_rect.bottom()), Pos2::new(cell_rect.right(), cell_rect.bottom())],
                    divider_stroke,
                );
            }

            // Selected cell: 1px accent border on top of dividers.
            if is_selected {
                painter.rect_stroke(
                    cell_rect,
                    CornerRadius::ZERO,
                    Stroke::new(1.0, theme.accent()),
                    StrokeKind::Inside,
                );
            }

            if cell_resp.clicked() {
                *selected = i;
                response.mark_changed();
            }
            if cell_resp.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            response = response.union(cell_resp);
        }

        // Outer grid border.
        ui.painter().rect_stroke(
            rect,
            CornerRadius::ZERO,
            Stroke::new(1.0, divider),
            StrokeKind::Inside,
        );

        response
    }
}
