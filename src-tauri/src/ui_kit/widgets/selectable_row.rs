//! SelectableRow — a clickable row for menus and dropdowns.
//!
//! Renders as a left-aligned label with optional leading icon, a hover tint,
//! an accent background and accent text when selected, and a dimmed look
//! when disabled. Replaces ad-hoc `ui.selectable_label(...)` callsites in
//! menus so visuals (font size, padding, hover/selected/disabled states)
//! are consistent across the app.
//!
//! API:
//!   ui.add(SelectableRow::new("Triangulator", false));
//!   ui.add(SelectableRow::new("Auto Target", true).disabled(true));
//!   ui.add(SelectableRow::new("RSI", on).leading_icon(Icon::CHART_LINE));
//!
//! Behavior:
//! - Row height scales with `Size` (defaults to ~24px / `gap_2xl()`).
//! - Idle: transparent background, `theme.text()` label.
//! - Selected: `color_alpha(theme.accent(), alpha_soft())` background,
//!   `theme.accent()` label.
//! - Hover: `color_alpha(theme.text(), alpha_faint())` background tint.
//! - Disabled: `st::color_dim(theme.text())` label, hover-only sense.
//! - Optional leading icon: `theme.dim()` color, `font_sm()` size,
//!   `gap_xs()` from text.
//!
//! Returns a normal `Response` so callers use `.clicked()` exactly like
//! `ui.selectable_label(...)`.

use egui::{CornerRadius, FontId, Pos2, Response, Sense, Ui, Vec2, Widget};

use super::theme::ComponentTheme;
use crate::ui_kit::sx::{palette_ct, Tone};
use super::tokens::Size;
use crate::ui_kit::tokens as st;

#[must_use = "SelectableRow does nothing until `.show(ui, theme)` or `ui.add(row)` is called"]
pub struct SelectableRow<'a> {
    label: &'a str,
    selected: bool,
    disabled: bool,
    leading_icon: Option<&'a str>,
    size: Size,
}

impl<'a> SelectableRow<'a> {
    pub fn new(label: &'a str, selected: bool) -> Self {
        Self {
            label,
            selected,
            disabled: false,
            leading_icon: None,
            size: Size::Sm,
        }
    }

    pub fn disabled(mut self, v: bool) -> Self { self.disabled = v; self }
    pub fn leading_icon(mut self, icon: &'a str) -> Self { self.leading_icon = Some(icon); self }
    pub fn size(mut self, s: Size) -> Self { self.size = s; self }

    pub fn show(self, ui: &mut Ui, theme: &dyn ComponentTheme) -> Response {
        let SelectableRow { label, selected, disabled, leading_icon, size } = self;

        // Sizing. Row height ~gap_2xl() (24px) at default Sm; scales modestly.
        let pad_x = st::gap_sm();
        let pad_y = st::gap_2xs();
        let icon_gap = st::gap_xs();

        let font_size = match size {
            Size::Xs => st::font_xs(),
            Size::Sm => st::font_sm(),
            Size::Md => st::font_sm(),
            Size::Lg => st::font_md(),
            Size::Xl => st::font_md(),
        };
        let row_h = match size {
            Size::Xs => 18.0,
            Size::Sm => st::gap_2xl(),     // 24
            Size::Md => st::gap_2xl(),     // 24
            Size::Lg => 28.0,
            Size::Xl => 28.0,
        };

        // Resolve colors.
        let text_color = if disabled {
            st::color_dim(palette_ct(theme).base(Tone::Text))
        } else if selected {
            palette_ct(theme).base(Tone::Accent)
        } else {
            palette_ct(theme).base(Tone::Text)
        };
        let icon_color = if disabled { st::color_half(palette_ct(theme).base(Tone::Dim)) } else { palette_ct(theme).base(Tone::Dim) };

        // Measure label.
        let label_font = FontId::monospace(font_size);
        let label_galley = ui.fonts(|f| {
            f.layout_no_wrap(label.to_string(), label_font.clone(), text_color)
        });
        let label_w = label_galley.rect.width();
        let label_h = label_galley.rect.height();

        // Optional leading icon measurement.
        let icon_font = FontId::proportional(font_size);
        let (icon_w, icon_h) = if let Some(ic) = leading_icon {
            let g = ui.fonts(|f| f.layout_no_wrap(ic.to_string(), icon_font.clone(), icon_color));
            (g.rect.width(), g.rect.height())
        } else {
            (0.0, 0.0)
        };

        // Allocate full available width so rows align in a vertical menu.
        let mut content_w = label_w;
        if leading_icon.is_some() { content_w += icon_w + icon_gap; }
        let min_w = content_w + pad_x * 2.0;
        let avail_w = ui.available_width().max(min_w);
        let h = row_h.max(label_h.max(icon_h) + pad_y * 2.0);

        let sense = if disabled { Sense::hover() } else { Sense::click() };
        let (rect, response) = ui.allocate_exact_size(Vec2::new(avail_w, h), sense);

        if !ui.is_rect_visible(rect) {
            return response;
        }

        let painter = ui.painter_at(rect);
        let cr = CornerRadius::same(st::radius_sm() as u8);

        // Background fill.
        if selected {
            let bg = st::color_alpha(palette_ct(theme).base(Tone::Accent), st::alpha_soft());
            painter.rect_filled(rect, cr, bg);
        } else if response.hovered() && !disabled {
            let bg = st::color_alpha(palette_ct(theme).base(Tone::Text), st::alpha_faint());
            painter.rect_filled(rect, cr, bg);
        }

        // Cursor affordance.
        if response.hovered() && !disabled {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }

        // Focus ring for keyboard navigation — egui's Sense::click() already
        // fires clicked() on Enter/Space when the widget has keyboard focus.
        if !disabled {
            st::cursor::focus_ring(ui, &response, palette_ct(theme).base(Tone::Accent));
        }

        // Layout: leading icon, then label.
        let cy = rect.center().y;
        let mut x = rect.left() + pad_x;

        if let Some(ic) = leading_icon {
            painter.text(
                Pos2::new(x, cy),
                egui::Align2::LEFT_CENTER,
                ic,
                icon_font,
                icon_color,
            );
            x += icon_w + icon_gap;
        }

        painter.galley(
            Pos2::new(x, cy - label_h * 0.5),
            label_galley,
            text_color,
        );

        response
    }
}

impl<'a> Widget for SelectableRow<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        let theme = super::theme::active_theme(ui.ctx());
        self.show(ui, &theme)
    }
}

