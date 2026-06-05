//! Kbd — keycap visualization for keyboard shortcuts. Used inline in
//! ContextMenu rows, tooltips, command palette.
//!
//! API:
//!   ui.add(Kbd::new("Ctrl+K"));
//!   ui.add(Kbd::sequence(&["Cmd", "Shift", "P"]));

use egui::{FontId, Pos2, Response, Sense, Ui, Vec2, Widget};

use super::theme::ComponentTheme;
use super::tokens::Size;
use crate::ui_kit::tokens as st;
use crate::ui_kit::sx::{palette_ct, Sx, Tone};

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
        let pal = palette_ct(theme);
        // DS#4: each keycap box is DECLARED once as an Sx and painted per key.
        let cap_sx = Sx::new()
            .rounded_sm()
            .bg_alpha(Tone::Surface, 200)
            .border(Tone::Border, st::stroke_std());
        let text_col = pal.base(Tone::Text);
        let dim = pal.base(Tone::Dim);

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
                cap_sx.paint_box_at(&painter, cap_rect, theme);
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
        let theme = super::theme::active_theme(ui.ctx());
        self.show(ui, &theme)
    }
}
