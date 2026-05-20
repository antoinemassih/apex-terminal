//! Builder + impl Widget primitives — status / feedback family.
//!
//! Adds builder wrappers for status dots, spinners, progress bars/rings,
//! skeleton placeholders, toasts, notification badges, connection
//! indicators, and trend arrows. **NEW additions only** — no migration of
//! existing call sites (Wave 5).
//!
//! Reuses `widgets::{text, frames, pills}` plus `style::*` primitives where
//! possible. Chart paint is sacred — `chart_widgets.rs` was read for visual
//! reference only.
//!
//! All builders implement `impl Widget` (or expose `.show(ui)` when they
//! return non-`Response` data) and follow the ambient design-token rules.

#![allow(dead_code, unused_imports)]

use egui::{Color32, Pos2, Rect, Response, RichText, Sense, Stroke, Ui, Vec2, Widget};
use super::super::style::*;

type Theme = crate::chart_renderer::gpu::Theme;

#[inline(always)]
fn ambient(ctx: &egui::Context) -> &'static Theme {
    crate::ui_kit::widgets::theme::active_theme(ctx)
}

// ─── Shared size enum ─────────────────────────────────────────────────────────

/// Tri-size knob shared by [`Spinner`] and other loading primitives.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum LoadSize { Sm, Md, Lg }

impl LoadSize {
    fn px(self) -> f32 { match self { LoadSize::Sm => 10.0, LoadSize::Md => 14.0, LoadSize::Lg => 20.0 } }
}

// ─── StatusDot ────────────────────────────────────────────────────────────────

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum DotVariant { Success, Danger, Warning, Neutral, Custom }

/// Small filled circle that conveys binary / categorical status. An optional
/// label is laid out to the right of the dot.
///
/// ```ignore
/// ui.add(StatusDot::new().success().label("Connected").theme(t));
/// ui.add(StatusDot::new().danger().pulsing().theme(t));
/// ```
#[must_use = "StatusDot must be added with `ui.add(...)` to render"]
pub struct StatusDot<'a> {
    label: Option<&'a str>,
    variant: DotVariant,
    color: Option<Color32>,
    label_color: Option<Color32>,
    pulsing: bool,
    radius: f32,
}

impl<'a> StatusDot<'a> {
    pub fn new() -> Self {
        Self {
            label: None,
            variant: DotVariant::Neutral,
            color: None,
            label_color: None,
            pulsing: false,
            radius: 3.5,
        }
    }
    pub fn label(mut self, s: &'a str) -> Self { self.label = Some(s); self }
    pub fn radius(mut self, r: f32) -> Self { self.radius = r; self }
    pub fn pulsing(mut self) -> Self { self.pulsing = true; self }
    pub fn color(mut self, c: Color32) -> Self { self.color = Some(c); self.variant = DotVariant::Custom; self }
    pub fn success(mut self) -> Self { self.variant = DotVariant::Success; self }
    pub fn danger(mut self)  -> Self { self.variant = DotVariant::Danger;  self }
    pub fn warning(mut self) -> Self { self.variant = DotVariant::Warning; self }
    pub fn neutral(mut self) -> Self { self.variant = DotVariant::Neutral; self }
    pub fn theme(mut self, t: &Theme) -> Self {
        self.label_color = Some(t.text);
        self.color = Some(match self.variant {
            DotVariant::Success => t.bull,
            DotVariant::Danger  => t.bear,
            DotVariant::Warning => t.warn,
            DotVariant::Neutral => t.dim,
            DotVariant::Custom  => self.color.unwrap_or(t.dim),
        });
        self
    }
}

impl<'a> Default for StatusDot<'a> {
    fn default() -> Self { Self::new() }
}

impl<'a> Widget for StatusDot<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        let amb = ambient(ui.ctx());
        let base_color = self.color.unwrap_or_else(|| match self.variant {
            DotVariant::Success => amb.bull,
            DotVariant::Danger  => amb.bear,
            DotVariant::Warning => amb.warn,
            DotVariant::Neutral => amb.dim,
            DotVariant::Custom  => amb.dim,
        });
        let label_color = self.label_color.unwrap_or(amb.text);
        let r = self.radius;
        let label_h = font_sm() + 2.0;
        let dot_box = Vec2::new(r * 2.0 + 2.0, label_h.max(r * 2.0 + 2.0));
        let label_w = self.label.map(|s| s.len() as f32 * font_sm() * 0.6).unwrap_or(0.0);
        let total = Vec2::new(dot_box.x + if self.label.is_some() { label_w + gap_sm() } else { 0.0 }, dot_box.y);
        let (rect, resp) = ui.allocate_exact_size(total, Sense::hover());
        let painter = ui.painter();

        // Pulsing alpha animation
        let mut color = base_color;
        if self.pulsing {
            let t = ui.ctx().input(|i| i.time);
            let phase = (t.sin() * 0.5 + 0.5) as f32;
            let a = (alpha_dim() as f32 + phase * (255.0 - alpha_dim() as f32)) as u8;
            color = color_alpha(base_color, a);
            ui.ctx().request_repaint();
            // Outer halo
            painter.circle_filled(
                Pos2::new(rect.left() + r + 1.0, rect.center().y),
                r + 2.0,
                color_alpha(base_color, (alpha_soft() as f32 * (1.0 - phase)) as u8),
            );
        }
        painter.circle_filled(Pos2::new(rect.left() + r + 1.0, rect.center().y), r, color);

        if let Some(s) = self.label {
            let text_pos = Pos2::new(rect.left() + dot_box.x + gap_sm(), rect.center().y);
            painter.text(
                text_pos,
                egui::Align2::LEFT_CENTER,
                s,
                egui::FontId::monospace(font_sm()),
                label_color,
            );
        }
        resp
    }
}

// ─── Spinner ──────────────────────────────────────────────────────────────────

/// Animated rotating arc loader.
///
/// ```ignore
/// ui.add(Spinner::new().md().theme(t));
/// ```
#[must_use = "Spinner must be added with `ui.add(...)` to render"]
pub struct Spinner {
    size: LoadSize,
    color: Option<Color32>,
    width: f32,
}

impl Spinner {
    pub fn new() -> Self {
        Self { size: LoadSize::Md, color: None, width: 1.5 }
    }
    pub fn size(mut self, s: LoadSize) -> Self { self.size = s; self }
    pub fn sm(mut self) -> Self { self.size = LoadSize::Sm; self }
    pub fn md(mut self) -> Self { self.size = LoadSize::Md; self }
    pub fn lg(mut self) -> Self { self.size = LoadSize::Lg; self }
    pub fn color(mut self, c: Color32) -> Self { self.color = Some(c); self }
    pub fn theme(mut self, t: &Theme) -> Self { self.color = Some(t.accent); self }
}

impl Default for Spinner {
    fn default() -> Self { Self::new() }
}

impl Widget for Spinner {
    fn ui(self, ui: &mut Ui) -> Response {
        let color = self.color.unwrap_or(ambient(ui.ctx()).accent);
        let d = self.size.px();
        let (rect, resp) = ui.allocate_exact_size(Vec2::splat(d + 2.0), Sense::hover());
        let center = rect.center();
        let radius = d * 0.5;
        let t = ui.ctx().input(|i| i.time) as f32;
        ui.ctx().request_repaint();

        // Arc: ~270° sweep rotating
        let segments = 24;
        let sweep = std::f32::consts::TAU * 0.75;
        let start = t * 4.0;
        let painter = ui.painter();
        for i in 0..segments {
            let a0 = start + sweep * (i as f32) / (segments as f32);
            let a1 = start + sweep * ((i + 1) as f32) / (segments as f32);
            let p0 = center + Vec2::new(a0.cos(), a0.sin()) * radius;
            let p1 = center + Vec2::new(a1.cos(), a1.sin()) * radius;
            // Fade older tail of the arc
            let frac = i as f32 / segments as f32;
            let alpha = (alpha_soft() as f32 + frac * (alpha_active() as f32 - alpha_soft() as f32)) as u8;
            painter.line_segment([p0, p1], Stroke::new(self.width, color_alpha(color, alpha)));
        }
        resp
    }
}

// ─── Skeleton ─────────────────────────────────────────────────────────────────

/// Shimmer placeholder rectangle for loading states.
///
/// ```ignore
/// ui.add(Skeleton::new().size(120.0, 14.0).rounding(4.0).theme(t));
/// ```
#[must_use = "Skeleton must be added with `ui.add(...)` to render"]
pub struct Skeleton {
    size: Vec2,
    rounding: f32,
    base: Option<Color32>,
    highlight: Option<Color32>,
}

impl Skeleton {
    pub fn new() -> Self {
        Self {
            size: Vec2::new(80.0, 12.0),
            rounding: 3.0,
            base: None,
            highlight: None,
        }
    }
    pub fn size(mut self, w: f32, h: f32) -> Self { self.size = Vec2::new(w, h); self }
    pub fn width(mut self, w: f32) -> Self { self.size.x = w; self }
    pub fn height(mut self, h: f32) -> Self { self.size.y = h; self }
    pub fn rounding(mut self, r: f32) -> Self { self.rounding = r; self }
    pub fn theme(mut self, t: &Theme) -> Self {
        self.base = Some(color_alpha(t.toolbar_border, alpha_heavy()));
        self.highlight = Some(color_alpha(t.dim, alpha_muted()));
        self
    }
}

impl Default for Skeleton {
    fn default() -> Self { Self::new() }
}

impl Widget for Skeleton {
    fn ui(self, ui: &mut Ui) -> Response {
        let amb = ambient(ui.ctx());
        let base = self.base.unwrap_or(color_alpha(amb.toolbar_border, alpha_heavy()));
        let highlight = self.highlight.unwrap_or(color_alpha(amb.dim, alpha_muted()));
        let (rect, resp) = ui.allocate_exact_size(self.size, Sense::hover());
        let cr = egui::CornerRadius::same(self.rounding as u8);
        let painter = ui.painter();
        painter.rect_filled(rect, cr, base);

        // Animated shimmer band moving left → right
        let t = ui.ctx().input(|i| i.time) as f32;
        ui.ctx().request_repaint();
        let phase = (t * 0.6).fract();
        let band_w = self.size.x * 0.35;
        let x0 = rect.left() - band_w + phase * (self.size.x + band_w * 2.0);
        let band = Rect::from_min_size(Pos2::new(x0, rect.top()), Vec2::new(band_w, self.size.y))
            .intersect(rect);
        if band.width() > 0.0 {
            painter.rect_filled(band, cr, color_alpha(highlight, alpha_subtle()));
        }
        resp
    }
}

// ─── Toast ────────────────────────────────────────────────────────────────────
//
// Toast / ToastVariant / ToastResponse have moved to
// `crate::ui_kit::widgets::toast`. Re-exported here for back-compat.
#[deprecated(note = "use crate::ui_kit::widgets::Toast")]
pub use crate::ui_kit::widgets::toast::Toast;
#[deprecated(note = "use crate::ui_kit::widgets::toast::ToastVariant")]
pub use crate::ui_kit::widgets::toast::ToastVariant;
#[deprecated(note = "use crate::ui_kit::widgets::toast::ToastResponse")]
pub use crate::ui_kit::widgets::toast::ToastResponse;

// (Toast struct/impl moved to ui_kit::widgets::toast — see re-exports above.)


