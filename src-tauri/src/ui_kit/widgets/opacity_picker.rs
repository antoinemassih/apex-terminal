//! OpacityPicker — segmented opacity-level picker.
//!
//! Renders horizontal segments at given opacity steps; the user clicks to
//! pick. Used in drawing tools, indicator settings, anywhere a multi-step
//! opacity choice is needed.
//!
//! ```ignore
//! let mut opacity = 0.5_f32;
//! if OpacityPicker::new(&mut opacity).show(ui, theme).changed() {
//!     // user picked a new level
//! }
//! ```

use egui::{Response, Sense, Ui};
use super::theme::ComponentTheme;
use crate::ui_kit::sx::{palette_ct, Tone};
use crate::ui_kit::tokens as st;

/// Six discrete opacity levels used across the app. Re-exported so panels can
/// share the same constant instead of defining their own.
pub const OPACITY_LEVELS: [f32; 6] = [0.15, 0.30, 0.50, 0.70, 0.85, 1.0];

/// Find the closest level index for an opacity value.
fn closest_level_idx(levels: &[f32], op: f32) -> usize {
    let mut best = 0;
    let mut best_d = f32::MAX;
    for (i, &lv) in levels.iter().enumerate() {
        let d = (lv - op).abs();
        if d < best_d {
            best_d = d;
            best = i;
        }
    }
    best
}

/// Compact segmented opacity picker.
///
/// Allocates a row of small rectangular segments; filled segments use the
/// theme accent at the corresponding opacity, unfilled segments use a dim
/// ghost. Clicking a segment sets `*value` to the corresponding level.
///
/// Returns an [`egui::Response`] — check `.changed()` to react to picks.
#[must_use = "OpacityPicker does nothing until `.show(ui, theme)` is called"]
pub struct OpacityPicker<'a> {
    value: &'a mut f32,
    levels: &'a [f32],
    seg_w: f32,
    seg_h: f32,
    gap: f32,
}

impl<'a> OpacityPicker<'a> {
    pub fn new(value: &'a mut f32) -> Self {
        Self {
            value,
            levels: &OPACITY_LEVELS,
            seg_w: 7.0,
            seg_h: 10.0,
            gap: 1.0,
        }
    }

    /// Override the opacity levels shown (default: `OPACITY_LEVELS`).
    pub fn levels(mut self, l: &'a [f32]) -> Self {
        self.levels = l;
        self
    }

    /// Override segment width (default: 7.0).
    pub fn seg_width(mut self, w: f32) -> Self {
        self.seg_w = w;
        self
    }

    /// Override segment height (default: 10.0).
    pub fn seg_height(mut self, h: f32) -> Self {
        self.seg_h = h;
        self
    }

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
        let pal = palette_ct(theme);
        let accent = pal.base(Tone::Accent);
        let dim = pal.base(Tone::Dim);
        let cur_idx = closest_level_idx(self.levels, *self.value);
        let total_w = (self.seg_w + self.gap) * self.levels.len() as f32;

        let (rect, mut resp) = ui.allocate_exact_size(
            egui::vec2(total_w, self.seg_h + 2.0),
            Sense::click(),
        );
        let painter = ui.painter_at(rect);
        let mut clicked_idx: Option<usize> = None;

        for i in 0..self.levels.len() {
            let x = rect.min.x + i as f32 * (self.seg_w + self.gap);
            let seg_rect = egui::Rect::from_min_size(
                egui::pos2(x, rect.min.y + 1.0),
                egui::vec2(self.seg_w, self.seg_h),
            );
            let filled = i <= cur_idx;
            let col = if filled {
                let a = self.levels[i];
                crate::ui_kit::style::color_alpha(accent, (a * 220.0) as u8)
            } else {
                crate::ui_kit::style::color_alpha(dim, crate::ui_kit::style::alpha_subtle())
            };
            painter.rect_filled(seg_rect, st::radius_xs(), col);

            if resp.clicked() {
                if let Some(pos) = resp.interact_pointer_pos() {
                    if pos.x >= seg_rect.left() && pos.x <= seg_rect.right() + self.gap {
                        clicked_idx = Some(i);
                    }
                }
            }
        }

        resp = resp.on_hover_text_at_pointer(format!(
            "Opacity {}%",
            (self.levels[cur_idx] * 100.0) as i32
        ));

        if let Some(i) = clicked_idx {
            *self.value = self.levels[i];
            resp.mark_changed();
        }

        resp
    }
}

impl<'a> egui::Widget for OpacityPicker<'a> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let theme = super::theme::active_theme(ui.ctx());
        self.show(ui, &theme)
    }
}
