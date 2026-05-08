//! Disclosure — chevron + label row that toggles a collapsible body.
//!
//! Zed pattern: every collapsible section in their sidebar, settings,
//! and tree views uses this primitive. Single visual treatment ensures
//! consistency.
//!
//! API:
//!   let mut open = true;
//!   Disclosure::new("Symbol Lists", &mut open)
//!       .show(ui, theme, |ui| {
//!           // body content rendered when open
//!           Label::new("CORE").show(ui, theme);
//!           Label::new("OPENING").show(ui, theme);
//!       });
//!
//!   // With trailing content (count badge, action button):
//!   Disclosure::new("Active Orders", &mut open)
//!       .trailing(|ui, theme| {
//!           Badge::count(orders.len()).show(ui, theme);
//!       })
//!       .show(ui, theme, |ui| { ... });
//!
//! v1 intentionally snaps the body show/hide — no animation. Zed itself
//! often snaps these; we'll add easing later if it feels missing.

use egui::{CornerRadius, FontId, Sense, Ui, Vec2};

use super::theme::ComponentTheme;
use super::tokens::Size;
use super::super::icons::Icon;
use crate::chart_renderer::ui::style::{color_alpha, gap_2xs, gap_md, gap_xs};

type TrailingFn<'a> = Box<dyn FnOnce(&mut Ui, &dyn ComponentTheme) + 'a>;

#[must_use = "Disclosure does nothing until `.show()` is called"]
pub struct Disclosure<'a> {
    label: String,
    open: &'a mut bool,
    icon: Option<&'static str>,
    trailing: Option<TrailingFn<'a>>,
    size: Size,
    indent_px: Option<f32>,
}

impl<'a> Disclosure<'a> {
    pub fn new(label: impl Into<String>, open: &'a mut bool) -> Self {
        Self {
            label: label.into(),
            open,
            icon: None,
            trailing: None,
            size: Size::Sm,
            indent_px: None,
        }
    }

    pub fn icon(mut self, icon: &'static str) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn trailing(mut self, f: impl FnOnce(&mut Ui, &dyn ComponentTheme) + 'a) -> Self {
        self.trailing = Some(Box::new(f));
        self
    }

    pub fn size(mut self, s: Size) -> Self {
        self.size = s;
        self
    }

    pub fn indent(mut self, px: f32) -> Self {
        self.indent_px = Some(px);
        self
    }

    /// Returns Some(R) if the body was rendered (open), None if closed.
    pub fn show<R>(
        self,
        ui: &mut Ui,
        theme: &dyn ComponentTheme,
        add_body: impl FnOnce(&mut Ui) -> R,
    ) -> Option<R> {
        let Disclosure { label, open, icon, trailing, size, indent_px } = self;

        let row_h = size.height();
        let font_size = size.font_size();
        let avail_w = ui.available_width();

        // Allocate the full header row up front so hover covers the entire
        // row, not just the chevron + text. Mirrors Zed's behavior.
        let (rect, response) = ui.allocate_exact_size(
            Vec2::new(avail_w.max(row_h), row_h),
            Sense::click(),
        );

        if response.clicked() {
            *open = !*open;
        }

        // Subtle hover background. Zed uses ~12-18 alpha of text — keep it
        // muted. TODO: switch to theme.element_hover() once landed.
        if response.hovered() && ui.is_rect_visible(rect) {
            let painter = ui.painter_at(rect);
            let bg = color_alpha(theme.text(), 18);
            painter.rect_filled(rect, CornerRadius::same(3), bg);
        }

        // Paint header contents left-to-right.
        if ui.is_rect_visible(rect) {
            let painter = ui.painter_at(rect);
            let mut x = rect.left() + gap_2xs();
            let cy = rect.center().y;

            // Chevron — glyph swap (not rotation) per spec.
            let chevron_glyph = if *open { Icon::CARET_DOWN } else { Icon::CARET_RIGHT };
            let chevron_color = if response.hovered() { theme.text() } else { theme.dim() };
            let chevron_font = FontId::proportional(font_size);
            let chevron_galley = ui.fonts(|f| {
                f.layout_no_wrap(chevron_glyph.to_string(), chevron_font.clone(), chevron_color)
            });
            painter.galley(
                egui::pos2(x, cy - chevron_galley.rect.height() * 0.5),
                chevron_galley.clone(),
                chevron_color,
            );
            x += chevron_galley.rect.width() + gap_2xs();

            // Optional leading icon.
            if let Some(ic) = icon {
                let ic_font = FontId::proportional(font_size);
                let ic_galley = ui.fonts(|f| {
                    f.layout_no_wrap(ic.to_string(), ic_font, theme.dim())
                });
                painter.galley(
                    egui::pos2(x, cy - ic_galley.rect.height() * 0.5),
                    ic_galley.clone(),
                    theme.dim(),
                );
                x += ic_galley.rect.width() + gap_xs();
            }

            // Label.
            let label_font = FontId::proportional(font_size);
            let label_color = theme.text();
            let label_galley = ui.fonts(|f| {
                f.layout_no_wrap(label.clone(), label_font, label_color)
            });
            painter.galley(
                egui::pos2(x, cy - label_galley.rect.height() * 0.5),
                label_galley,
                label_color,
            );
        }

        // Trailing slot — draw via a child UI clipped to a strip on the
        // right edge of the row. This lets the caller render arbitrary
        // widgets (Badge, Button) without needing pixel-perfect layout.
        if let Some(trailing) = trailing {
            // Reserve ~80px on the right for the trailing content. Real
            // size is determined by the child UI but we need a clip rect.
            let trailing_w = 120.0_f32.min(rect.width() * 0.5);
            let trailing_rect = egui::Rect::from_min_max(
                egui::pos2(rect.right() - trailing_w - gap_xs(), rect.top()),
                egui::pos2(rect.right() - gap_xs(), rect.bottom()),
            );
            let mut child = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(trailing_rect)
                    .layout(egui::Layout::right_to_left(egui::Align::Center)),
            );
            trailing(&mut child, theme);
        }

        // Body — render only when open. v1: snap, no animation.
        if *open {
            let indent = indent_px.unwrap_or_else(gap_md);
            let mut result: Option<R> = None;
            ui.horizontal(|ui| {
                ui.add_space(indent);
                ui.vertical(|ui| {
                    result = Some(add_body(ui));
                });
            });
            result
        } else {
            None
        }
    }
}

