//! NumberStepper — design-system replacement for `egui::DragValue`.
//!
//! Pattern: a numeric input that supports drag-to-change, click-to-edit, and
//! optional ±buttons. Used for indicator periods, settings sliders, etc.
//!
//! ```ignore
//! NumberStepper::new(&mut value)
//!     .range(1..=100)
//!     .step(1.0)
//!     .suffix("px")
//!     .show(ui, theme);
//! ```
//!
//! Why this exists: 30+ sites use `egui::DragValue` directly, which doesn't
//! honor our spacing/font/color tokens. This wrapper provides one canonical
//! treatment that matches `Input` styling.
//!
//! Caveat: this is currently a thin wrapper around DragValue. Future work may
//! paint it from scratch with motion-driven hover and proper focus rings.

use egui::{Response, Stroke, Ui};
use std::ops::RangeInclusive;

use super::theme::ComponentTheme;
use super::tokens::Size;
use crate::ui_kit::tokens as st;
use crate::ui_kit::sx::{palette_ct, Tone};

#[must_use = "NumberStepper does nothing until `.show(ui, theme)` is called"]
pub struct NumberStepper<'a, T: egui::emath::Numeric> {
    value: &'a mut T,
    range: Option<RangeInclusive<T>>,
    step: f64,
    prefix: &'a str,
    suffix: &'a str,
    size: Size,
    width: Option<f32>,
    disabled: bool,
    /// If set, renders the value with this many decimals (passes through to
    /// `DragValue::fixed_decimals`). Use `0` for integer rendering.
    fixed_decimals: Option<usize>,
}

impl<'a, T: egui::emath::Numeric> NumberStepper<'a, T> {
    pub fn new(value: &'a mut T) -> Self {
        Self {
            value,
            range: None,
            step: 1.0,
            prefix: "",
            suffix: "",
            size: Size::Sm,
            width: None,
            disabled: false,
            fixed_decimals: None,
        }
    }

    pub fn range(mut self, r: RangeInclusive<T>) -> Self { self.range = Some(r); self }
    pub fn step(mut self, s: f64) -> Self { self.step = s; self }
    pub fn prefix(mut self, p: &'a str) -> Self { self.prefix = p; self }
    pub fn suffix(mut self, s: &'a str) -> Self { self.suffix = s; self }
    pub fn size(mut self, s: Size) -> Self { self.size = s; self }
    pub fn width(mut self, w: f32) -> Self { self.width = Some(w); self }
    pub fn disabled(mut self, d: bool) -> Self { self.disabled = d; self }
    /// Render the value with a fixed number of decimals (0 = integer).
    pub fn decimals(mut self, n: usize) -> Self { self.fixed_decimals = Some(n); self }
    /// Convenience: render as an integer (alias for `.decimals(0)`).
    pub fn integer(mut self) -> Self { self.fixed_decimals = Some(0); self }

    #[track_caller]
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
        let bug_loc = std::panic::Location::caller();
        let font = match self.size {
            Size::Xs => st::font_xs(),
            Size::Sm => st::font_sm(),
            Size::Md => st::font_md(),
            Size::Lg => st::font_lg(),
            Size::Xl => st::font_lg(),
        };
        let mut dv = egui::DragValue::new(self.value).speed(self.step);
        if let Some(r) = self.range {
            dv = dv.range(r);
        }
        if !self.prefix.is_empty() { dv = dv.prefix(self.prefix); }
        if !self.suffix.is_empty() { dv = dv.suffix(self.suffix); }
        if let Some(n) = self.fixed_decimals {
            dv = dv.fixed_decimals(n);
        }
        if let Some(w) = self.width {
            dv = dv.max_decimals(18).min_decimals(0); // width honoured via custom_formatter workaround below
            let _ = w; // DragValue doesn't expose a direct width setter; reserved for future scratch paint
        }
        let disabled = self.disabled;
        let resp = ui.scope(|ui| {
            let font_id = crate::ui_kit::style::mono_at(font);
            ui.style_mut().override_font_id = Some(font_id);

            // Style inactive/hover/active states to match Input appearance.
            let pal = palette_ct(theme);
            let v = &mut ui.style_mut().visuals;
            let cr = egui::CornerRadius::same(st::radius_sm() as u8);
            v.widgets.inactive.bg_fill    = pal.base(Tone::Surface);
            v.widgets.inactive.bg_stroke  = Stroke::new(st::stroke_thin(), pal.base(Tone::Border));
            v.widgets.inactive.corner_radius   = cr;
            v.widgets.hovered.bg_fill     = pal.base(Tone::Surface);
            v.widgets.hovered.bg_stroke   = Stroke::new(st::stroke_std(), pal.base(Tone::Dim));
            v.widgets.hovered.corner_radius    = cr;
            v.widgets.active.bg_fill      = pal.base(Tone::Surface);
            v.widgets.active.bg_stroke    = Stroke::new(st::stroke_std(), st::color_alpha(pal.base(Tone::Accent), st::alpha_active()));
            v.widgets.active.corner_radius     = cr;

            ui.add_enabled(!disabled, dv)
        });
        crate::ui_kit::inspect::mark(bug_loc, "stepper", resp.inner.rect);
        // U1-4: accent focus ring on keyboard focus (DragValue is focusable). Its
        // own doc promised this; it was relying on egui-default focus visuals.
        crate::ui_kit::tokens::cursor::focus_ring(
            ui, &resp.inner,
            crate::ui_kit::sx::palette_ct(theme).base(crate::ui_kit::sx::Tone::Accent),
        );
        resp.inner
    }
}

impl<'a, T: egui::emath::Numeric> egui::Widget for NumberStepper<'a, T> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let theme = super::theme::active_theme(ui.ctx());
        self.show(ui, &theme)
    }
}
