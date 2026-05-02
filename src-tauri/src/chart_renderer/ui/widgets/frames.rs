//! Builder primitives — frames family.
//!
//! Each builder mirrors an existing free-function helper (in `style.rs` /
//! `components.rs` / `components_extra.rs`) but exposes a chained-setter API
//! so call sites can write:
//!
//! ```ignore
//! let frame = PanelFrame::new(bg, border).build();
//! let frame = PanelFrame::new(bg, border).theme(t).build();
//! ```
//!
//! **The legacy free-functions are NOT modified or removed.** This file only
//! adds builder wrappers whose `.build()` bodies are byte-for-byte copies of
//! the original helper bodies.

#![allow(dead_code, unused_imports)]

use egui::{Color32, Stroke};
use super::super::style::*;

// Shorthand for the Theme type used across the codebase.
type Theme = crate::chart_renderer::gpu::Theme;

// ─── PanelFrame ───────────────────────────────────────────────────────────────

/// Builder for `panel_frame(toolbar_bg, toolbar_border) -> egui::Frame`.
///
/// ```ignore
/// let f = PanelFrame::new(theme.toolbar_bg, theme.toolbar_border).build();
/// let f = PanelFrame::new(Color32::TRANSPARENT, Color32::TRANSPARENT)
///             .theme(t).build();
/// ```
pub struct PanelFrame {
    bg: Color32,
    border: Color32,
}

impl PanelFrame {
    pub fn new(toolbar_bg: Color32, toolbar_border: Color32) -> Self {
        Self { bg: toolbar_bg, border: toolbar_border }
    }

    /// Pull bg + border from a theme.
    pub fn theme(self, t: &Theme) -> Self {
        Self { bg: t.toolbar_bg, border: t.toolbar_border }
    }

    /// Override with explicit colors (alternative to `.theme()`).
    pub fn colors(self, bg: Color32, border: Color32) -> Self {
        Self { bg, border }
    }

    /// Build the `egui::Frame`. Body mirrors `style::panel_frame` byte-for-byte.
    pub fn build(self) -> egui::Frame {
        let s = current();
        let border = if s.hairline_borders {
            self.border
        } else {
            color_alpha(self.border, alpha_heavy())
        };
        egui::Frame::NONE
            .fill(self.bg)
            .inner_margin(egui::Margin {
                left:   gap_xl() as i8,
                right:  gap_xl() as i8,
                top:    gap_xl() as i8,
                bottom: gap_lg() as i8,
            })
            .corner_radius(r_md_cr())
            .stroke(Stroke::new(s.stroke_std, border))
    }
}

// ─── CardFrame ────────────────────────────────────────────────────────────────

/// Builder for the card frame used by `components::card_frame`.
///
/// Note: the original `card_frame` is a show-closure helper, not a plain
/// `-> egui::Frame` function. This builder extracts the frame construction
/// so callers can obtain the `egui::Frame` value directly.
///
/// ```ignore
/// CardFrame::new().theme(t).build().show(ui, |ui| { ... });
/// ```
pub struct CardFrame {
    bg: Color32,
    border: Color32,
}

impl CardFrame {
    pub fn new() -> Self {
        Self { bg: Color32::TRANSPARENT, border: Color32::TRANSPARENT }
    }

    pub fn theme(self, t: &Theme) -> Self {
        Self { bg: t.toolbar_bg, border: t.toolbar_border }
    }

    pub fn colors(self, bg: Color32, border: Color32) -> Self {
        Self { bg, border }
    }

    /// Build the `egui::Frame`. Body mirrors the frame construction inside
    /// `components::card_frame` byte-for-byte.
    pub fn build(self) -> egui::Frame {
        let st = current();
        // card_padding_y / card_padding_x knobs let the user tune card insets per style.
        let pad_y = st.card_padding_y as i8;
        let pad_x = st.card_padding_x as i8;
        let mut frame = egui::Frame::NONE
            .fill(self.bg)
            .corner_radius(r_md_cr())
            .inner_margin(egui::Margin { left: pad_x, right: pad_x, top: pad_y, bottom: pad_y });

        if st.hairline_borders {
            frame = frame.stroke(Stroke::new(
                st.stroke_std,
                color_alpha(self.border, alpha_strong()),
            ));
        } else {
            frame = frame.stroke(Stroke::new(
                st.stroke_thin,
                color_alpha(self.border, alpha_muted()),
            ));
        }

        if st.shadows_enabled {
            // shadow_blur / shadow_offset_y / shadow_alpha knobs override global tokens.
            frame = frame.shadow(egui::epaint::Shadow {
                offset: [0, st.shadow_offset_y as i8],
                blur:   st.shadow_blur as u8,
                spread: 0,
                color:  Color32::from_black_alpha(st.shadow_alpha),
            });
        }

        frame
    }
}

impl Default for CardFrame {
    fn default() -> Self { Self::new() }
}

// ─── DialogFrame ──────────────────────────────────────────────────────────────

/// Builder for the dialog / modal frame used by `components::dialog_frame`.
///
/// Like `CardFrame`, the original is a show-closure helper — this exposes the
/// raw `egui::Frame` for callers that need it directly.
///
/// ```ignore
/// DialogFrame::new().theme(t).build(ctx).show(ui, |ui| { ... });
/// ```
pub struct DialogFrame<'a> {
    bg: Color32,
    border: Color32,
    ctx: Option<&'a egui::Context>,
}

impl<'a> DialogFrame<'a> {
    pub fn new() -> Self {
        Self { bg: Color32::TRANSPARENT, border: Color32::TRANSPARENT, ctx: None }
    }

    pub fn theme(self, t: &Theme) -> Self {
        Self { bg: t.toolbar_bg, border: t.toolbar_border, ..self }
    }

    pub fn colors(self, bg: Color32, border: Color32) -> Self {
        Self { bg, border, ..self }
    }

    /// Provide a context so `.build()` can call `egui::Frame::popup`.
    /// Required when using `dialog_frame`-style popup base.
    pub fn ctx(self, ctx: &'a egui::Context) -> Self {
        Self { ctx: Some(ctx), ..self }
    }

    /// Build the `egui::Frame`. Body mirrors `components::dialog_frame`
    /// byte-for-byte.  Panics if no context was provided (needed for
    /// `egui::Frame::popup`).
    pub fn build(self) -> egui::Frame {
        let st = current();
        let ctx = self.ctx.expect("DialogFrame::build requires a Context — call .ctx(ctx) first");
        let mut frame = egui::Frame::popup(&ctx.style())
            .fill(self.bg)
            .corner_radius(r_lg_cr())
            .inner_margin(egui::Margin::same(gap_xl() as i8));

        if st.hairline_borders {
            frame = frame.stroke(Stroke::new(st.stroke_std, self.border));
        } else {
            frame = frame.stroke(Stroke::new(
                st.stroke_thin,
                color_alpha(self.border, alpha_strong()),
            ));
        }

        if st.shadows_enabled {
            // Dialogs use a stronger shadow than cards; offset_y and alpha still scale with style.
            let alpha = (st.shadow_alpha as u16).saturating_add(40).min(255) as u8;
            let blur = (st.shadow_blur * 1.2).min(64.0) as u8;
            frame = frame.shadow(egui::epaint::Shadow {
                offset: [0, st.shadow_offset_y as i8],
                blur,
                spread: 2,
                color:  Color32::from_black_alpha(alpha),
            });
        } else {
            frame = frame.shadow(egui::epaint::Shadow::NONE);
        }

        frame
    }
}

impl<'a> Default for DialogFrame<'a> {
    fn default() -> Self { Self::new() }
}

// ─── PopupFrame ───────────────────────────────────────────────────────────────

/// Builder for `components::themed_popup_frame(ctx, bg, border) -> egui::Frame`.
///
/// ```ignore
/// PopupFrame::new().theme(t).ctx(ctx).build()
/// ```
pub struct PopupFrame<'a> {
    bg: Color32,
    border: Color32,
    ctx: Option<&'a egui::Context>,
}

impl<'a> PopupFrame<'a> {
    pub fn new() -> Self {
        Self { bg: Color32::TRANSPARENT, border: Color32::TRANSPARENT, ctx: None }
    }

    pub fn theme(self, t: &Theme) -> Self {
        Self { bg: t.toolbar_bg, border: t.toolbar_border, ..self }
    }

    pub fn colors(self, bg: Color32, border: Color32) -> Self {
        Self { bg, border, ..self }
    }

    pub fn ctx(self, ctx: &'a egui::Context) -> Self {
        Self { ctx: Some(ctx), ..self }
    }

    /// Build the `egui::Frame`. Body mirrors `components::themed_popup_frame`
    /// byte-for-byte.
    pub fn build(self) -> egui::Frame {
        let st = current();
        let ctx = self.ctx.expect("PopupFrame::build requires a Context — call .ctx(ctx) first");

        let pop_bg = if st.hairline_borders {
            self.bg.gamma_multiply(1.10)
        } else {
            self.bg
        };

        let mut frame = egui::Frame::popup(&ctx.style())
            .fill(pop_bg)
            .corner_radius(r_lg_cr())
            .inner_margin(egui::Margin::same(gap_lg() as i8));

        if st.hairline_borders {
            frame = frame.stroke(Stroke::new(st.stroke_std, self.border));
        } else {
            frame = frame.stroke(Stroke::new(
                st.stroke_thin,
                color_alpha(self.border, alpha_strong()),
            ));
        }

        if st.shadows_enabled {
            frame = frame.shadow(egui::epaint::Shadow {
                offset: [0, st.shadow_offset_y as i8],
                blur:   st.shadow_blur as u8,
                spread: 1,
                color:  Color32::from_black_alpha(st.shadow_alpha),
            });
        } else {
            frame = frame.shadow(egui::epaint::Shadow::NONE);
        }

        frame
    }
}

impl<'a> Default for PopupFrame<'a> {
    fn default() -> Self { Self::new() }
}

// ─── SidePanelFrame ───────────────────────────────────────────────────────────

/// Builder for `components_extra::themed_side_panel_frame(ctx, bg, border) -> egui::Frame`.
///
/// ```ignore
/// SidePanelFrame::new().theme(t).ctx(ctx).build()
/// ```
pub struct SidePanelFrame<'a> {
    bg: Color32,
    border: Color32,
    /// Kept for API symmetry with `themed_side_panel_frame`; currently unused
    /// in the build body (original ignores it too — `_ctx`).
    _ctx: Option<&'a egui::Context>,
}

impl<'a> SidePanelFrame<'a> {
    pub fn new() -> Self {
        Self { bg: Color32::TRANSPARENT, border: Color32::TRANSPARENT, _ctx: None }
    }

    pub fn theme(self, t: &Theme) -> Self {
        Self { bg: t.toolbar_bg, border: t.toolbar_border, _ctx: self._ctx }
    }

    pub fn colors(self, bg: Color32, border: Color32) -> Self {
        Self { bg, border, _ctx: self._ctx }
    }

    pub fn ctx(self, ctx: &'a egui::Context) -> Self {
        Self { _ctx: Some(ctx), ..self }
    }

    /// Build the `egui::Frame`. Body mirrors
    /// `components_extra::themed_side_panel_frame` byte-for-byte.
    pub fn build(self) -> egui::Frame {
        let st = current();
        let stroke = if st.hairline_borders {
            Stroke::new(st.stroke_std, self.border)
        } else {
            Stroke::new(st.stroke_thin, color_alpha(self.border, alpha_strong()))
        };
        egui::Frame::NONE
            .fill(self.bg)
            .stroke(stroke)
            .corner_radius(r_md_cr())
            .inner_margin(egui::Margin::ZERO)
    }
}

impl<'a> Default for SidePanelFrame<'a> {
    fn default() -> Self { Self::new() }
}

// ─── TooltipFrame ─────────────────────────────────────────────────────────────

/// Builder for `style::tooltip_frame(toolbar_bg, toolbar_border) -> egui::Frame`.
///
/// ```ignore
/// TooltipFrame::new().theme(t).build()
/// ```
pub struct TooltipFrame {
    bg: Color32,
    border: Color32,
}

// ─── CompactPanelFrame ────────────────────────────────────────────────────────

/// Builder for `style::panel_frame_compact(toolbar_bg, toolbar_border) -> egui::Frame`.
///
/// Tighter margins than `PanelFrame` for narrow info-dense panels (scanner, tape).
///
/// ```ignore
/// let f = CompactPanelFrame::new(theme.toolbar_bg, theme.toolbar_border).build();
/// let f = CompactPanelFrame::new(Color32::TRANSPARENT, Color32::TRANSPARENT)
///             .theme(t).build();
/// ```
pub struct CompactPanelFrame {
    bg: Color32,
    border: Color32,
}

impl CompactPanelFrame {
    pub fn new(toolbar_bg: Color32, toolbar_border: Color32) -> Self {
        Self { bg: toolbar_bg, border: toolbar_border }
    }

    pub fn theme(self, t: &Theme) -> Self {
        Self { bg: t.toolbar_bg, border: t.toolbar_border }
    }

    pub fn colors(self, bg: Color32, border: Color32) -> Self {
        Self { bg, border }
    }

    /// Build the `egui::Frame`. Body mirrors `style::panel_frame_compact`
    /// byte-for-byte.
    pub fn build(self) -> egui::Frame {
        let s = current();
        let border = if s.hairline_borders {
            self.border
        } else {
            color_alpha(self.border, alpha_heavy())
        };
        egui::Frame::NONE
            .fill(self.bg)
            .inner_margin(egui::Margin {
                left:   gap_lg() as i8,
                right:  gap_lg() as i8,
                top:    gap_lg() as i8,
                bottom: gap_md() as i8,
            })
            .corner_radius(r_sm_cr())
            .stroke(Stroke::new(s.stroke_std, border))
    }
}

// ─── DialogSeparator ──────────────────────────────────────────────────────────

/// Builder for `style::dialog_separator(ui, margin, color)`.
///
/// Inset horizontal separator with margins on both sides.
///
/// ```ignore
/// DialogSeparator::new(theme.toolbar_border).indent(8.0).show(ui);
/// ```
pub struct DialogSeparator {
    indent: f32,
    color: Color32,
}

impl DialogSeparator {
    pub fn new(color: Color32) -> Self {
        Self { indent: 0.0, color }
    }
    pub fn indent(mut self, m: f32) -> Self { self.indent = m; self }
    pub fn color(mut self, c: Color32) -> Self { self.color = c; self }

    /// Body mirrors `style::dialog_separator` byte-for-byte.
    pub fn show(self, ui: &mut egui::Ui) {
        let s = current();
        let rect = ui.available_rect_before_wrap();
        let w = if s.hairline_borders { s.stroke_std } else { stroke_thin() };
        ui.painter().line_segment(
            [egui::pos2(rect.left() + self.indent, ui.cursor().min.y),
             egui::pos2(rect.right() - self.indent, ui.cursor().min.y)],
            Stroke::new(w, self.color));
        ui.add_space(crate::dt_f32!(separator.after_space, 1.0));
    }
}

impl TooltipFrame {
    pub fn new() -> Self {
        Self { bg: Color32::TRANSPARENT, border: Color32::TRANSPARENT }
    }

    pub fn theme(self, t: &Theme) -> Self {
        Self { bg: t.toolbar_bg, border: t.toolbar_border }
    }

    pub fn colors(self, bg: Color32, border: Color32) -> Self {
        Self { bg, border }
    }

    /// Build the `egui::Frame`. Body mirrors `style::tooltip_frame`
    /// byte-for-byte.
    pub fn build(self) -> egui::Frame {
        let s = current();
        let border = if s.hairline_borders {
            self.border
        } else {
            color_alpha(self.border, alpha_strong())
        };
        let stroke_w = if s.hairline_borders { s.stroke_std } else { stroke_thin() };
        let cr = if s.hairline_borders {
            egui::CornerRadius::ZERO
        } else {
            egui::CornerRadius::same(
                crate::dt_f32!(tooltip.corner_radius, 8.0) as u8,
            )
        };
        egui::Frame::NONE
            .fill(self.bg)
            .stroke(Stroke::new(stroke_w, border))
            .inner_margin(crate::dt_f32!(tooltip.padding, 8.0))
            .corner_radius(cr)
    }
}

impl Default for TooltipFrame {
    fn default() -> Self { Self::new() }
}
