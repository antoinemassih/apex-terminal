//! Kbd — keycap visualization for keyboard shortcuts. Used inline in
//! ContextMenu rows, tooltips, command palette.
//!
//! API:
//!   ui.add(Kbd::new("Ctrl+K"));
//!   ui.add(Kbd::sequence(&["Cmd", "Shift", "P"]));

use egui::{FontId, Response, Sense, Ui, Vec2, Widget};

use crate::ui_kit::layout::{Align as FlexAlign, Flex, Item};

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
        // Build the ctx from the UI so it carries the AMBIENT RecipeSet.
        // `StyleCtx::from_theme` would hand this widget an empty set — see
        // `ctx.rs` for why that shim must never be used inside a `show`.
        let sctx = super::ctx::StyleCtx::from_ui(theme, ui);
        self.show_ctx(ui, &sctx)
    }

    /// [`StyleCtx`](super::ctx::StyleCtx) entry point.
    ///
    /// Callers that need per-call-site token overrides or an explicit
    /// `RecipeSet` construct a `StyleCtx` and call this directly; `show`
    /// delegates here with the ambient one.
    pub fn show_ctx(self, ui: &mut Ui, sctx: &super::ctx::StyleCtx<'_>) -> Response {
        let theme = sctx.theme();
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
        // `kbd` key — the keycap box. The key was already REGISTERED and
        // authored but nothing here resolved it, so the styles' keycap
        // declarations were dead data.
        let cap_sx = super::theme::resolve_sx(ui.ctx(), theme, "kbd",
            Sx::new()
                .rounded_sm()
                .bg_alpha(Tone::Surface, 200)
                .border(Tone::Border, st::stroke_std()),
        );
        let text_col = pal.base(Tone::Text);
        let dim = pal.base(Tone::Dim);

        // Pre-measure each cap and each "+" joiner.
        //
        // M4.3: the chord used to be a cursor walk with THREE different
        // advances (`x += w`, `x += plus_gap`, `x += pw + plus_gap`). It is
        // really just an alternating strip — cap, joiner, cap, … — on a
        // `plus_gap` gutter, which is one flex row.
        let plus_w = ui
            .fonts(|f| f.layout_no_wrap("+".to_string(), crate::ui_kit::style::mono_at(font_size), dim))
            .rect
            .width();

        let mut cap_widths: Vec<f32> = Vec::with_capacity(self.keys.len());
        let mut total_w: f32 = 0.0;
        for (i, k) in self.keys.iter().enumerate() {
            let g = ui.fonts(|f| f.layout_no_wrap(k.clone(), crate::ui_kit::style::mono_at(font_size), text_col));
            let w = (g.rect.width() + pad_x * 2.0).max(cap_h);
            cap_widths.push(w);
            total_w += w;
            if i + 1 < self.keys.len() {
                total_w += plus_gap * 2.0 + plus_w;
            }
        }

        let desired = Vec2::new(total_w.max(cap_h), cap_h);
        let (rect, response) = ui.allocate_exact_size(desired, Sense::hover());

        if ui.is_rect_visible(rect) {
            let painter = ui.painter_at(rect);

            // `cap · + · cap · …`, `plus_gap` between every pair. The caps
            // stretch to the strip height (they used to be built explicitly
            // from `rect.top()` + `cap_h`); the joiners are centred glyphs.
            let mut f = Flex::row().gap(plus_gap).align(FlexAlign::Center);
            for (i, w) in cap_widths.iter().enumerate() {
                f = f.item(Item::fixed(*w).cross(cap_h));
                if i + 1 < cap_widths.len() {
                    f = f.item(Item::fixed(plus_w));
                }
            }
            let off = rect.min.to_vec2();
            let slots: Vec<_> = f.solve(rect.size()).into_iter().map(|r| r.translate(off)).collect();

            for (i, k) in self.keys.iter().enumerate() {
                let cap_rect = slots[i * 2];
                cap_sx.paint_box_at(&painter, cap_rect, theme);
                painter.text(
                    cap_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    k,
                    crate::ui_kit::style::mono_at(font_size),
                    text_col,
                );
                if i + 1 < self.keys.len() {
                    painter.text(
                        slots[i * 2 + 1].center(),
                        egui::Align2::CENTER_CENTER,
                        "+",
                        crate::ui_kit::style::mono_at(font_size),
                        dim,
                    );
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
