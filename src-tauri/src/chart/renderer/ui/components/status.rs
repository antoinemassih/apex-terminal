//! Builder + impl Widget primitives — status / feedback family.
//!
//! Contains [`StatusDot`] and Toast re-exports.
//! `Spinner` and `Skeleton` have been migrated to `ui_kit::widgets`.
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

/// Tri-size knob for loading primitives.
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


