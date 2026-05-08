//! ThemePreviewCard — interactive theme preview tile.
//!
//! Renders a tiny mock editor (background bar + colored line strokes
//! representing code) inside a clickable card. Selected state swaps
//! the always-reserved 2px border from transparent to accent.
//! Zero layout shift on selection — pure color swap.
//!
//! API:
//!   ThemePreviewCard::new("Gruvbox", &gruvbox_theme)
//!     .selected(active_theme_idx == 2)
//!     .show(ui, theme);

use egui::{Color32, CornerRadius, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Ui, Vec2};

use super::label::Label;
use super::theme::ComponentTheme;
use super::tokens::Size;

const DEFAULT_SIZE: Vec2 = Vec2::new(132.0, 80.0);
const BORDER_WIDTH: f32 = 2.0;
const CARD_RADIUS: f32 = 6.0;

#[must_use = "ThemePreviewCard does nothing until `.show(ui, theme)` is called"]
pub struct ThemePreviewCard<'a> {
    label: String,
    preview_theme: &'a dyn ComponentTheme,
    selected: bool,
    size: Vec2,
}

impl<'a> ThemePreviewCard<'a> {
    pub fn new(label: impl Into<String>, preview_theme: &'a dyn ComponentTheme) -> Self {
        Self {
            label: label.into(),
            preview_theme,
            selected: false,
            size: DEFAULT_SIZE,
        }
    }

    pub fn selected(mut self, v: bool) -> Self {
        self.selected = v;
        self
    }

    pub fn size(mut self, size: Vec2) -> Self {
        self.size = size;
        self
    }

    pub fn show(self, ui: &mut Ui, theme: &dyn ComponentTheme) -> Response {
        let Self { label, preview_theme, selected, size } = self;

        // Allocate full vertical space (card + gap + label) so the layout
        // is stable whether or not the label wraps.
        let label_h = crate::chart_renderer::ui::style::font_xs() + 2.0;
        let gap = crate::chart_renderer::ui::style::gap_2xs();
        let total = Vec2::new(size.x, size.y + gap + label_h);

        let (rect, response) = ui.allocate_exact_size(total, Sense::click());
        if !ui.is_rect_visible(rect) {
            return response;
        }

        let card_rect = Rect::from_min_size(rect.min, size);
        let painter = ui.painter_at(card_rect);

        // Card background.
        painter.rect_filled(card_rect, CornerRadius::same(CARD_RADIUS as u8), preview_theme.bg());

        // Hover overlay (subtle, on top of bg, before mock editor).
        if response.hovered() {
            painter.rect_filled(
                card_rect,
                CornerRadius::same(CARD_RADIUS as u8),
                theme.element_hover(),
            );
        }

        // Mock editor: 5 horizontal "code" lines with varying widths and
        // mock indentation. 2px tall, ~3px gap.
        // Inset from card edges.
        let pad_x = 10.0;
        let pad_y = 12.0;
        let inner_left = card_rect.left() + pad_x;
        let inner_right = card_rect.right() - pad_x;
        let inner_top = card_rect.top() + pad_y;
        let inner_w = inner_right - inner_left;

        // (indent multiplier, width fraction, use_accent)
        let lines: [(f32, f32, bool); 5] = [
            (0.00, 0.55, true),   // "fn name() {"
            (0.18, 0.42, false),
            (0.18, 0.62, false),
            (0.36, 0.30, true),
            (0.00, 0.18, false),  // "}"
        ];
        let line_h = 2.0;
        let line_gap = 4.0;
        let mut y = inner_top;
        for (indent, frac, accent) in lines {
            let x0 = inner_left + indent * inner_w;
            let x1 = (x0 + frac * inner_w).min(inner_right);
            let color = if accent { preview_theme.accent() } else { preview_theme.text() };
            // Slightly muted so the preview doesn't scream.
            let color = Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 200);
            let lr = Rect::from_min_max(Pos2::new(x0, y), Pos2::new(x1, y + line_h));
            painter.rect_filled(lr, CornerRadius::same(1), color);
            y += line_h + line_gap;
            if y + line_h > card_rect.bottom() - pad_y {
                break;
            }
        }

        // Border — ALWAYS 2px, painted last so it overlays the card.
        // Color swaps on selection; width never changes (zero layout shift).
        let border_color = if selected { theme.accent() } else { Color32::TRANSPARENT };
        ui.painter().rect_stroke(
            card_rect,
            CornerRadius::same(CARD_RADIUS as u8),
            Stroke::new(BORDER_WIDTH, border_color),
            StrokeKind::Inside,
        );

        // Label below the card.
        let label_rect = Rect::from_min_size(
            Pos2::new(rect.left(), card_rect.bottom() + gap),
            Vec2::new(rect.width(), label_h),
        );
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(label_rect)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );
        Label::new(label).size(Size::Xs).muted().truncate(true).show(&mut child, theme);

        if response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }

        response
    }
}
