//! Builder + impl Widget primitives — buttons family.
//! See ui/widgets/mod.rs for the rationale.

#![allow(dead_code, unused_imports)]

use egui::{Color32, Response, RichText, Stroke, Ui, Widget};
use super::super::style::*;

#[inline(always)]
fn hit(r: &egui::Rect, family: &'static str, category: &'static str) {
    crate::design_tokens::register_hit(
        [r.min.x, r.min.y, r.width(), r.height()], family, category);
}

// ─── IconBtn ──────────────────────────────────────────────────────────────────

/// Builder for an icon-only button. Replaces `style::icon_btn(ui, glyph, color, size)`.
///
/// ```ignore
/// ui.add(IconBtn::new("✕").medium().color(theme.dim));
/// ```
#[must_use = "IconBtn must be added with `ui.add(...)` to render"]
pub struct IconBtn<'a> {
    glyph: &'a str,
    color: Option<Color32>,
    size: f32,
}

impl<'a> IconBtn<'a> {
    pub fn new(glyph: &'a str) -> Self {
        Self { glyph, color: None, size: 14.0 }
    }
    pub fn color(mut self, c: Color32) -> Self { self.color = Some(c); self }
    pub fn size(mut self, s: f32) -> Self { self.size = s; self }
    pub fn small(mut self) -> Self { self.size = 11.0; self }
    pub fn medium(mut self) -> Self { self.size = 14.0; self }
    pub fn large(mut self) -> Self { self.size = 18.0; self }
    /// Pull dim color from a theme.
    pub fn theme(self, t: &super::super::super::gpu::Theme) -> Self {
        self.color(t.dim)
    }
    /// Explicit palette — accent, bear, dim. Uses `dim` as default color.
    pub fn palette(self, _accent: Color32, _bear: Color32, dim: Color32) -> Self {
        self.color(dim)
    }
}

impl<'a> Widget for IconBtn<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        let color = self.color.unwrap_or_else(|| color_alpha(Color32::from_rgb(120, 120, 130), alpha_dim()));
        let size = self.size;
        let side = (size + 8.0).max(22.0);
        let prev_pad = ui.spacing().button_padding;
        ui.spacing_mut().button_padding = egui::vec2(0.0, 0.0);
        let resp = ui.add(
            egui::Button::new(RichText::new(self.glyph).size(size).color(color))
                .frame(false)
                .min_size(egui::vec2(side, side))
        );
        ui.spacing_mut().button_padding = prev_pad;
        hit(&resp.rect, "ICON_BTN", "Icon Buttons");
        if resp.hovered() && !crate::design_tokens::is_inspect_mode() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            ui.painter().rect_filled(resp.rect, radius_sm(), color_alpha(color, alpha_ghost()));
            ui.painter().rect_stroke(resp.rect, radius_sm(),
                egui::Stroke::new(stroke_thin(), color_alpha(color, alpha_muted())), egui::StrokeKind::Inside);
        }
        resp
    }
}

// ─── TradeBtn ─────────────────────────────────────────────────────────────────

/// Builder for a trade button (BUY/SELL). Replaces `style::trade_btn(ui, label, color, width)`.
///
/// ```ignore
/// if ui.add(TradeBtn::new("BUY").color(theme.bull).width(80.0)).clicked() { ... }
/// ```
#[must_use = "TradeBtn must be added with `ui.add(...)` to render"]
pub struct TradeBtn<'a> {
    label: &'a str,
    color: Color32,
    width: f32,
    height: Option<f32>,
}

impl<'a> TradeBtn<'a> {
    pub fn new(label: &'a str) -> Self {
        Self { label, color: Color32::from_rgb(80, 180, 100), width: 0.0, height: None }
    }
    pub fn color(mut self, c: Color32) -> Self { self.color = c; self }
    pub fn width(mut self, w: f32) -> Self { self.width = w; self }
    /// Override the default trade-button height. Used by the DOM panel where
    /// the action area is sized in absolute pixels (e.g. 30px).
    pub fn height(mut self, h: f32) -> Self { self.height = Some(h); self }
    pub fn theme(self, t: &super::super::super::gpu::Theme) -> Self {
        self.color(t.accent)
    }
    pub fn palette(self, accent: Color32, _bear: Color32, _dim: Color32) -> Self {
        self.color(accent)
    }
}

impl<'a> Widget for TradeBtn<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        let color = self.color;
        let label = self.label;
        let s = current();
        let bright = crate::dt_f32!(button.trade_brightness, 0.55);
        let dim_bg = Color32::from_rgb(
            (color.r() as f32 * bright) as u8,
            (color.g() as f32 * bright) as u8,
            (color.b() as f32 * bright) as u8);

        let (bg, fg, stroke_w, border, cr) = match s.button_treatment {
            ButtonTreatment::SoftPill => (dim_bg, Color32::WHITE, 0.0_f32, Color32::TRANSPARENT, r_sm_cr()),
            ButtonTreatment::OutlineAccent => (color, contrast_fg(color), 1.5_f32, color, r_md_cr()),
            ButtonTreatment::UnderlineActive | ButtonTreatment::RaisedActive | ButtonTreatment::BlackFillActive => (Color32::TRANSPARENT, color, 0.0_f32, Color32::TRANSPARENT, r_xs()),
        };

        let h = self.height.unwrap_or_else(btn_trade_height);
        let resp = ui.add(egui::Button::new(RichText::new(label).monospace().size(font_md()).strong().color(fg))
            .fill(bg).stroke(Stroke::new(stroke_w, border))
            .min_size(egui::vec2(self.width, h)).corner_radius(cr));
        hit(&resp.rect, "TRADE_BTN", "Buttons");
        if resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            match s.button_treatment {
                ButtonTreatment::SoftPill => {
                    let hb = crate::dt_f32!(button.trade_hover_brightness, 0.7);
                    let hover_bg = Color32::from_rgb(
                        (color.r() as f32 * hb).min(255.0) as u8,
                        (color.g() as f32 * hb).min(255.0) as u8,
                        (color.b() as f32 * hb).min(255.0) as u8);
                    ui.painter().rect_filled(resp.rect, radius_md(), hover_bg);
                    ui.painter().text(resp.rect.center(), egui::Align2::CENTER_CENTER,
                        label, egui::FontId::monospace(font_lg()), Color32::WHITE);
                }
                ButtonTreatment::OutlineAccent => {
                    ui.painter().rect_filled(resp.rect, current().r_md, color);
                    ui.painter().text(resp.rect.center(), egui::Align2::CENTER_CENTER,
                        label, egui::FontId::monospace(font_lg()), contrast_fg(color));
                }
                ButtonTreatment::UnderlineActive | ButtonTreatment::RaisedActive | ButtonTreatment::BlackFillActive => {
                    ui.painter().rect_filled(resp.rect, current().r_xs, color_alpha(color, alpha_ghost()));
                }
            }
        }
        if matches!(s.button_treatment, ButtonTreatment::UnderlineActive) {
            let r = resp.rect;
            ui.painter().line_segment(
                [egui::pos2(r.left(), r.bottom() + 0.5),
                 egui::pos2(r.right(), r.bottom() + 0.5)],
                Stroke::new(1.0, color));
        }
        resp
    }
}

// ─── SimpleBtn ────────────────────────────────────────────────────────────────

/// Builder for a simple form button. Replaces `style::simple_btn(ui, label, color, min_width)`.
/// Default `min_width` = 0.
///
/// ```ignore
/// if ui.add(SimpleBtn::new("Cancel").color(theme.dim)).clicked() { ... }
/// ```
#[must_use = "SimpleBtn must be added with `ui.add(...)` to render"]
pub struct SimpleBtn<'a> {
    label: &'a str,
    color: Color32,
    min_width: f32,
    height: Option<f32>,
}

impl<'a> SimpleBtn<'a> {
    pub fn new(label: &'a str) -> Self {
        Self { label, color: Color32::from_rgb(120, 120, 130), min_width: 0.0, height: None }
    }
    pub fn color(mut self, c: Color32) -> Self { self.color = c; self }
    pub fn min_width(mut self, w: f32) -> Self { self.min_width = w; self }
    /// Override the default small-button height for pixel-pinned layouts
    /// (e.g. the DOM panel control row).
    pub fn height(mut self, h: f32) -> Self { self.height = Some(h); self }
    pub fn theme(self, t: &super::super::super::gpu::Theme) -> Self {
        self.color(t.dim)
    }
    pub fn palette(self, accent: Color32, _bear: Color32, _dim: Color32) -> Self {
        self.color(accent)
    }
}

impl<'a> Widget for SimpleBtn<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        let color = self.color;
        let s = current();
        let (fill, fg, stroke_w, stroke_col, cr) = match s.button_treatment {
            ButtonTreatment::SoftPill => (
                color_alpha(color, alpha_faint()), color, stroke_thin(), color_alpha(color, alpha_muted()), r_sm_cr()
            ),
            ButtonTreatment::OutlineAccent => (
                Color32::TRANSPARENT, color, 1.5_f32, color_alpha(color, alpha_strong()), r_md_cr()
            ),
            ButtonTreatment::UnderlineActive | ButtonTreatment::RaisedActive | ButtonTreatment::BlackFillActive => (
                Color32::TRANSPARENT, color, 0.0_f32, Color32::TRANSPARENT, r_xs()
            ),
        };
        let h = self.height.unwrap_or_else(btn_small_height);
        let resp = ui.add(egui::Button::new(RichText::new(self.label).monospace().size(font_sm()).color(fg))
            .fill(fill)
            .stroke(Stroke::new(stroke_w, stroke_col))
            .corner_radius(cr)
            .min_size(egui::vec2(self.min_width, h)));
        hit(&resp.rect, "SIMPLE_BTN", "Buttons");
        if resp.hovered() && !crate::design_tokens::is_inspect_mode() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            match s.button_treatment {
                ButtonTreatment::OutlineAccent => {
                    ui.painter().rect_filled(resp.rect, current().r_md, color_alpha(color, alpha_soft()));
                    ui.painter().rect_stroke(resp.rect, current().r_md,
                        Stroke::new(1.5, color), egui::StrokeKind::Inside);
                }
                ButtonTreatment::UnderlineActive | ButtonTreatment::RaisedActive | ButtonTreatment::BlackFillActive => {
                    ui.painter().rect_filled(resp.rect, current().r_xs, color_alpha(color, alpha_ghost()));
                }
                _ => {}
            }
        }
        if matches!(s.button_treatment, ButtonTreatment::UnderlineActive) {
            let r = resp.rect;
            ui.painter().line_segment(
                [egui::pos2(r.left(), r.bottom() + 0.5),
                 egui::pos2(r.right(), r.bottom() + 0.5)],
                Stroke::new(1.0, color));
        }
        resp
    }
}

// ─── SmallActionBtn ───────────────────────────────────────────────────────────

/// Builder for an inline header action button. Replaces `style::small_action_btn(ui, label, color)`.
///
/// ```ignore
/// if ui.add(SmallActionBtn::new("Clear All").color(theme.dim)).clicked() { ... }
/// ```
#[must_use = "SmallActionBtn must be added with `ui.add(...)` to render"]
pub struct SmallActionBtn<'a> {
    label: &'a str,
    color: Color32,
}

impl<'a> SmallActionBtn<'a> {
    pub fn new(label: &'a str) -> Self {
        Self { label, color: Color32::from_rgb(120, 120, 130) }
    }
    pub fn color(mut self, c: Color32) -> Self { self.color = c; self }
    pub fn theme(self, t: &super::super::super::gpu::Theme) -> Self {
        self.color(t.dim)
    }
    pub fn palette(self, accent: Color32, _bear: Color32, _dim: Color32) -> Self {
        self.color(accent)
    }
}

impl<'a> Widget for SmallActionBtn<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        let color = self.color;
        let s = current();
        let (fill, fg, stroke_w, stroke_col, cr) = match s.button_treatment {
            ButtonTreatment::SoftPill => (
                color_alpha(color, alpha_soft()), color, stroke_thin(), color_alpha(color, alpha_dim()), r_sm_cr()
            ),
            ButtonTreatment::OutlineAccent => (
                Color32::TRANSPARENT, color, 1.5_f32, color_alpha(color, alpha_strong()), r_md_cr()
            ),
            ButtonTreatment::UnderlineActive | ButtonTreatment::RaisedActive | ButtonTreatment::BlackFillActive => (
                Color32::TRANSPARENT, color, 0.0_f32, Color32::TRANSPARENT, r_xs()
            ),
        };
        let resp = ui.add(egui::Button::new(RichText::new(self.label).monospace().size(font_sm()).strong().color(fg))
            .fill(fill)
            .corner_radius(cr)
            .stroke(Stroke::new(stroke_w, stroke_col))
            .min_size(egui::vec2(0.0, btn_compact_height())));
        hit(&resp.rect, "SMALL_BTN", "Buttons");
        if resp.hovered() && !crate::design_tokens::is_inspect_mode() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            match s.button_treatment {
                ButtonTreatment::OutlineAccent => {
                    ui.painter().rect_filled(resp.rect, current().r_md, color_alpha(color, alpha_soft()));
                    ui.painter().rect_stroke(resp.rect, current().r_md,
                        Stroke::new(1.5, color), egui::StrokeKind::Inside);
                }
                ButtonTreatment::UnderlineActive | ButtonTreatment::RaisedActive | ButtonTreatment::BlackFillActive => {
                    ui.painter().rect_filled(resp.rect, current().r_xs, color_alpha(color, alpha_ghost()));
                }
                _ => {}
            }
        }
        if matches!(s.button_treatment, ButtonTreatment::UnderlineActive) {
            let r = resp.rect;
            ui.painter().line_segment(
                [egui::pos2(r.left(), r.bottom() + 0.5),
                 egui::pos2(r.right(), r.bottom() + 0.5)],
                Stroke::new(1.0, color));
        }
        resp
    }
}

// ─── ChromeBtn ────────────────────────────────────────────────────────────────

/// Chrome button — for bespoke `egui::Button` chrome that doesn't fit
/// IconBtn/TradeBtn/SimpleBtn/ActionBtn. The label is supplied as a
/// pre-styled `RichText` so callers retain full control of
/// font/size/strong/monospace. Bypasses ButtonTreatment dispatch — every
/// visual is explicit. Useful for: Connect/Disconnect, Add Bot, Send,
/// Above/Below alert pills, Paper/Live frameless toggle, etc.
#[must_use = "ChromeBtn must be added with `ui.add(...)` to render"]
pub struct ChromeBtn {
    text: RichText,
    fill: Option<Color32>,
    stroke: Option<egui::Stroke>,
    corner_radius: Option<egui::CornerRadius>,
    frameless: bool,
    min_size: Option<egui::Vec2>,
    padding: Option<egui::Margin>,
}

impl ChromeBtn {
    pub fn new(text: RichText) -> Self {
        Self {
            text,
            fill: None,
            stroke: None,
            corner_radius: None,
            frameless: false,
            min_size: None,
            padding: None,
        }
    }
    pub fn fill(mut self, c: Color32) -> Self { self.fill = Some(c); self }
    pub fn stroke(mut self, s: egui::Stroke) -> Self { self.stroke = Some(s); self }
    pub fn corner_radius(mut self, r: impl Into<egui::CornerRadius>) -> Self { self.corner_radius = Some(r.into()); self }
    pub fn frameless(mut self, f: bool) -> Self { self.frameless = f; self }
    pub fn min_size(mut self, s: egui::Vec2) -> Self { self.min_size = Some(s); self }
    pub fn padding(mut self, m: egui::Margin) -> Self { self.padding = Some(m); self }
}

impl Widget for ChromeBtn {
    fn ui(self, ui: &mut Ui) -> Response {
        let mut btn = egui::Button::new(self.text);
        if let Some(c) = self.fill { btn = btn.fill(c); }
        if let Some(s) = self.stroke { btn = btn.stroke(s); }
        if let Some(r) = self.corner_radius { btn = btn.corner_radius(r); }
        if self.frameless { btn = btn.frame(false); }
        if let Some(s) = self.min_size { btn = btn.min_size(s); }
        // padding: egui::Button has no direct margin setter; field stored for callers' reference only
        let _ = self.padding;
        let resp = ui.add(btn);
        hit(&resp.rect, "CHROME_BTN", "Buttons");
        if resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        resp
    }
}

// ─── ActionBtn ────────────────────────────────────────────────────────────────

/// Builder for a small tinted action button. Replaces `style::action_btn(ui, label, color, enabled)`.
/// Distinct from `ActionButton` (the big full-width button in components_extra).
///
/// ```ignore
/// if ui.add(ActionBtn::new("Place").color(theme.accent).enabled(order.valid)).clicked() { ... }
/// ```
#[must_use = "ActionBtn must be added with `ui.add(...)` to render"]
pub struct ActionBtn<'a> {
    label: &'a str,
    color: Color32,
    enabled: bool,
}

impl<'a> ActionBtn<'a> {
    pub fn new(label: &'a str) -> Self {
        Self { label, color: Color32::from_rgb(120, 140, 220), enabled: true }
    }
    pub fn color(mut self, c: Color32) -> Self { self.color = c; self }
    pub fn enabled(mut self, e: bool) -> Self { self.enabled = e; self }
    pub fn theme(self, t: &super::super::super::gpu::Theme) -> Self {
        self.color(t.accent)
    }
    pub fn palette(self, accent: Color32, _bear: Color32, _dim: Color32) -> Self {
        self.color(accent)
    }
}

impl<'a> Widget for ActionBtn<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        let color = self.color;
        let enabled = self.enabled;
        let s = current();
        let (bg, fg, border, stroke_w, cr) = match s.button_treatment {
            ButtonTreatment::SoftPill => {
                let (bg, fg, border) = if enabled {
                    (color_alpha(color, alpha_muted()), color, color_alpha(color, alpha_active()))
                } else {
                    (color_alpha(color, alpha_faint()), color_alpha(color, alpha_active()), color_alpha(color, alpha_line()))
                };
                (bg, fg, border, 0.5_f32, r_sm_cr())
            }
            ButtonTreatment::OutlineAccent => {
                let (bg, fg, border) = if enabled {
                    (color, contrast_fg(color), color)
                } else {
                    (Color32::TRANSPARENT, color_alpha(color, alpha_muted()), color_alpha(color, alpha_muted()))
                };
                (bg, fg, border, 1.5_f32, r_md_cr())
            }
            ButtonTreatment::UnderlineActive | ButtonTreatment::RaisedActive | ButtonTreatment::BlackFillActive => {
                let fg = if enabled { color } else { color_alpha(color, alpha_muted()) };
                (Color32::TRANSPARENT, fg, Color32::TRANSPARENT, 0.0_f32, r_xs())
            }
        };
        let resp = ui.add_enabled(enabled,
            egui::Button::new(RichText::new(self.label).monospace().size(font_sm()).strong().color(fg))
                .fill(bg).stroke(Stroke::new(stroke_w, border))
                .corner_radius(cr).min_size(egui::vec2(0.0, btn_simple_height())));
        hit(&resp.rect, "ACTION_BTN", "Buttons");
        if resp.hovered() && !crate::design_tokens::is_inspect_mode() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
        if matches!(s.button_treatment, ButtonTreatment::UnderlineActive) && enabled {
            let r = resp.rect;
            ui.painter().line_segment(
                [egui::pos2(r.left(), r.bottom() + 0.5),
                 egui::pos2(r.right(), r.bottom() + 0.5)],
                Stroke::new(1.0, color));
        }
        resp
    }
}
