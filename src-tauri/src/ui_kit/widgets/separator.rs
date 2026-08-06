//! Separator — horizontal or vertical hairline. Replaces hairlines.rs
//! call sites with a token-aligned widget.
//!
//! API:
//!   ui.add(Separator::horizontal());
//!   ui.add(Separator::horizontal().with_label("Active orders"));
//!   ui.add(Separator::vertical());

use egui::{FontId, Pos2, Response, Sense, Stroke, Ui, Vec2, Widget};

use super::theme::ComponentTheme;
use crate::ui_kit::tokens as st;
use crate::ui_kit::sx::{palette_ct, Tone};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Orientation { Horizontal, Vertical }

#[must_use = "Separator does nothing until `.show(ui, theme)` or `ui.add(sep)` is called"]
pub struct Separator<'a> {
    orientation: Orientation,
    label: Option<String>,
    faint: bool,
    spacing: Option<f32>,
    _lt: std::marker::PhantomData<&'a ()>,
}

impl<'a> Separator<'a> {
    pub fn horizontal() -> Self {
        Self {
            orientation: Orientation::Horizontal,
            label: None,
            faint: false,
            spacing: None,
            _lt: std::marker::PhantomData,
        }
    }

    pub fn vertical() -> Self {
        Self {
            orientation: Orientation::Vertical,
            label: None,
            faint: false,
            spacing: None,
            _lt: std::marker::PhantomData,
        }
    }

    pub fn with_label(mut self, text: impl Into<String>) -> Self {
        self.label = Some(text.into());
        self
    }

    pub fn faint(mut self) -> Self { self.faint = true; self }
    pub fn spacing(mut self, px: f32) -> Self { self.spacing = Some(px); self }

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
        // Hairline color from the unified Sx Border tone (S500 == theme.border()).
        let pal = palette_ct(theme);
        let mut color = pal.base(Tone::Border);
        if self.faint {
            color = st::color_alpha(color, 80);
        }

        let pad = self.spacing.unwrap_or_else(st::gap_xs);

        match self.orientation {
            Orientation::Horizontal => {
                let avail_w = ui.available_width();
                let h = 1.0 + pad * 2.0;
                let desired = Vec2::new(avail_w, h);
                let (rect, response) = ui.allocate_exact_size(desired, Sense::hover());

                if ui.is_rect_visible(rect) {
                    let y = rect.center().y;
                    let painter = ui.painter_at(rect);

                    if let Some(label) = self.label.as_ref() {
                        let font_size = st::font_xs();
                        let dim = pal.base(Tone::Dim);
                        let galley = ui.fonts(|f| f.layout_no_wrap(label.clone(), FontId::proportional(font_size), dim));
                        let lw = galley.rect.width();
                        let gap = st::gap_sm();
                        let total = lw + gap * 2.0;
                        let left_end = rect.left() + ((avail_w - total) * 0.5).max(0.0);
                        let right_start = left_end + total;
                        painter.line_segment(
                            [Pos2::new(rect.left(), y), Pos2::new(left_end, y)],
                            Stroke::new(st::stroke_std(), color),
                        );
                        painter.text(
                            Pos2::new(left_end + gap, y),
                            egui::Align2::LEFT_CENTER,
                            label,
                            FontId::proportional(font_size),
                            dim,
                        );
                        painter.line_segment(
                            [Pos2::new(right_start, y), Pos2::new(rect.right(), y)],
                            Stroke::new(st::stroke_std(), color),
                        );
                    } else {
                        painter.line_segment(
                            [Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
                            Stroke::new(st::stroke_std(), color),
                        );
                    }
                }
                response
            }
            Orientation::Vertical => {
                let avail_h = ui.available_height();
                let h = if avail_h.is_finite() && avail_h > 0.0 { avail_h } else { 16.0 };
                let w = 1.0 + pad * 2.0;
                let desired = Vec2::new(w, h);
                let (rect, response) = ui.allocate_exact_size(desired, Sense::hover());
                if ui.is_rect_visible(rect) {
                    let x = rect.center().x;
                    ui.painter_at(rect).line_segment(
                        [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
                        Stroke::new(st::stroke_std(), color),
                    );
                }
                response
            }
        }
    }
}

impl<'a> Widget for Separator<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        let theme = super::theme::active_theme(ui.ctx());
        self.show(ui, &theme)
    }
}
