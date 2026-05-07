//! Kbd — keycap visualization for keyboard shortcuts. Used inline in
//! ContextMenu rows, tooltips, command palette.
//!
//! API:
//!   ui.add(Kbd::new("Ctrl+K"));
//!   ui.add(Kbd::sequence(&["Cmd", "Shift", "P"]));

use egui::{CornerRadius, FontId, Pos2, Response, Sense, Stroke, StrokeKind, Ui, Vec2, Widget};

use super::theme::ComponentTheme;
use super::tokens::Size;
use crate::chart::renderer::ui::style as st;

#[must_use = "Kbd does nothing until `.show(ui, theme)` or `ui.add(kbd)` is called"]
pub struct Kbd<'a> {
    keys: Vec<String>,
    size: Size,
    _lt: std::marker::PhantomData<&'a ()>,
}

impl<'a> Kbd<'a> {
    pub fn new(text: impl Into<String>) -> Self {
        let text: String = text.into();
        let keys: Vec<String> = text
            .split('+')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        Self { keys, size: Size::Sm, _lt: std::marker::PhantomData }
    }

    pub fn sequence(keys: &'a [&'a str]) -> Self {
        Self {
            keys: keys.iter().map(|s| s.to_string()).collect(),
            size: Size::Sm,
            _lt: std::marker::PhantomData,
        }
    }

    pub fn size(mut self, s: Size) -> Self { self.size = s; self }

    pub fn show(self, ui: &mut Ui, theme: &dyn ComponentTheme) -> Response {
        let font_size: f32 = match self.size {
            Size::Xs => 9.0,
            _ => 10.0, // intentionally small for keycaps
        };
        let pad_x: f32 = 4.0;
        let pad_y: f32 = 1.0;
        let cap_h: f32 = font_size + pad_y * 2.0 + 2.0;
        let plus_gap: f32 = 3.0;
        let bg = st::color_alpha(theme.surface(), 200);
        let border = theme.border();
        let text_col = theme.text();
        let dim = theme.dim();

        // Pre-measure each cap.
        let mut cap_widths: Vec<f32> = Vec::with_capacity(self.keys.len());
        let mut total_w: f32 = 0.0;
        for (i, k) in self.keys.iter().enumerate() {
            let g = ui.fonts(|f| f.layout_no_wrap(k.clone(), FontId::monospace(font_size), text_col));
            let w = (g.rect.width() + pad_x * 2.0).max(cap_h);
            cap_widths.push(w);
            total_w += w;
            if i + 1 < self.keys.len() {
                let plus_g = ui.fonts(|f| f.layout_no_wrap("+".to_string(), FontId::monospace(font_size), dim));
                total_w += plus_gap * 2.0 + plus_g.rect.width();
            }
        }

        let desired = Vec2::new(total_w.max(cap_h), cap_h);
        let (rect, response) = ui.allocate_exact_size(desired, Sense::hover());

        if ui.is_rect_visible(rect) {
            let painter = ui.painter_at(rect);
            let cy = rect.center().y;
            let mut x = rect.left();
            for (i, k) in self.keys.iter().enumerate() {
                let w = cap_widths[i];
                let cap_rect = egui::Rect::from_min_size(Pos2::new(x, rect.top()), Vec2::new(w, cap_h));
                let cr = CornerRadius::same(3);
                painter.rect_filled(cap_rect, cr, bg);
                painter.rect_stroke(cap_rect, cr, Stroke::new(1.0, border), StrokeKind::Inside);
                painter.text(
                    cap_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    k,
                    FontId::monospace(font_size),
                    text_col,
                );
                x += w;
                if i + 1 < self.keys.len() {
                    x += plus_gap;
                    let plus_g = ui.fonts(|f| f.layout_no_wrap("+".to_string(), FontId::monospace(font_size), dim));
                    let pw = plus_g.rect.width();
                    painter.text(
                        Pos2::new(x + pw * 0.5, cy),
                        egui::Align2::CENTER_CENTER,
                        "+",
                        FontId::monospace(font_size),
                        dim,
                    );
                    x += pw + plus_gap;
                }
            }
        }

        response
    }
}

impl<'a> Widget for Kbd<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        let theme = &crate::chart_renderer::gpu::THEMES[0];
        self.show(ui, theme)
    }
}
