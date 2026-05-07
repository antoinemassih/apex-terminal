//! Toast widget — re-homed from `chart::renderer::ui::components::status`.
//!
//! Stateless notification card. Caller owns the lifecycle and consults
//! `auto_dismiss_due` to drop the toast on its render list. The widget
//! adds a slide+fade entrance (and matching exit when the host stops
//! calling `show`) over `motion::FAST`.
//!
//! API surface (builder methods, `ToastVariant`, `ToastResponse`) is
//! unchanged from the legacy struct so callers compile via the
//! `components::status` re-export.

#![allow(dead_code)]

use egui::{Color32, Id, Rect, Response, RichText, Stroke, Ui, Vec2};

use super::theme::ComponentTheme;
use super::motion;

use crate::chart_renderer::ui::style::{
    alpha_dim, alpha_strong, color_alpha, font_md, font_sm, gap_lg, r_md_cr, stroke_thin,
};

type Theme = crate::chart_renderer::gpu::Theme;

fn ft() -> &'static Theme {
    &crate::chart_renderer::gpu::THEMES[0]
}

/// Toast variant — affects accent / icon color.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ToastVariant { Info, Success, Warning, Danger }

/// Returned from [`Toast::show`] — exposes the response plus an
/// `auto_dismiss_due` hint that callers can compare to `ctx.input(|i| i.time)`
/// to decide when to stop rendering. The widget itself is stateless.
pub struct ToastResponse {
    pub response: Response,
    /// Suggested timestamp (seconds, matching `egui::InputState::time`) at
    /// which the host should drop this toast. `None` = sticky.
    pub auto_dismiss_due: Option<f64>,
}

/// Temporary notification card with a title + optional body. Stateless —
/// the caller owns the lifecycle and consults `auto_dismiss_due` to decide
/// when to remove it from its render list.
#[must_use = "Toast must be shown with `.show(ui)` to render"]
pub struct Toast<'a> {
    title: &'a str,
    body: Option<&'a str>,
    variant: ToastVariant,
    accent: Option<Color32>,
    bg: Color32,
    border: Color32,
    text: Color32,
    auto_dismiss_secs: Option<f32>,
    width: f32,
    id: Option<&'a str>,
}

impl<'a> Toast<'a> {
    pub fn new(title: &'a str) -> Self {
        Self {
            title,
            body: None,
            variant: ToastVariant::Info,
            accent: None,
            bg: ft().toolbar_bg,
            border: ft().toolbar_border,
            text: ft().text,
            auto_dismiss_secs: None,
            width: 280.0,
            id: None,
        }
    }
    pub fn body(mut self, s: &'a str) -> Self { self.body = Some(s); self }
    pub fn variant(mut self, v: ToastVariant) -> Self { self.variant = v; self }
    pub fn info(mut self)    -> Self { self.variant = ToastVariant::Info;    self }
    pub fn success(mut self) -> Self { self.variant = ToastVariant::Success; self }
    pub fn warning(mut self) -> Self { self.variant = ToastVariant::Warning; self }
    pub fn danger(mut self)  -> Self { self.variant = ToastVariant::Danger;  self }
    pub fn auto_dismiss_secs(mut self, s: f32) -> Self { self.auto_dismiss_secs = Some(s); self }
    pub fn width(mut self, w: f32) -> Self { self.width = w; self }
    pub fn accent(mut self, c: Color32) -> Self { self.accent = Some(c); self }
    /// Stable id for the entrance animation. If unset, falls back to the title.
    pub fn id(mut self, id: &'a str) -> Self { self.id = Some(id); self }
    /// Theme — accepts any `ComponentTheme`.
    pub fn theme<T: ComponentTheme>(mut self, t: &T) -> Self {
        self.bg = t.surface();
        self.border = t.border();
        self.text = t.text();
        self.accent = Some(match self.variant {
            ToastVariant::Info    => t.accent(),
            ToastVariant::Success => t.bull(),
            ToastVariant::Warning => t.warn(),
            ToastVariant::Danger  => t.bear(),
        });
        self
    }

    pub fn show(self, ui: &mut Ui) -> ToastResponse {
        let accent = self.accent.unwrap_or_else(|| ft().accent);
        let due = self.auto_dismiss_secs
            .map(|s| ui.ctx().input(|i| i.time) + s as f64);

        // Slide + fade entrance over FAST. The widget is stateless so we
        // anchor the animation on title (or explicit id). When the host
        // stops calling `show`, egui prunes the memory entry naturally.
        let anim_id = Id::new(("apex_toast_anim", self.id.unwrap_or(self.title)));
        let appear_t = motion::ease_bool(ui.ctx(), anim_id, true, motion::FAST);
        let slide_offset = (1.0 - appear_t) * 12.0; // px slide from right

        // toast_bg_alpha controls how opaque the toast background is (semi-transparent = glassmorphic).
        let st_toast = crate::chart_renderer::ui::style::current();
        let toast_fill = color_alpha(self.bg, st_toast.toast_bg_alpha);
        let frame = egui::Frame::NONE
            .fill(toast_fill)
            .stroke(Stroke::new(stroke_thin(), color_alpha(self.border, alpha_strong())))
            .corner_radius(r_md_cr())
            .inner_margin(egui::Margin::same(gap_lg() as i8));

        // Apply slide-in by shifting the cursor right by `slide_offset`,
        // then dropping opacity for the fade-in.
        if slide_offset > 0.0 {
            ui.add_space(slide_offset);
        }
        ui.scope(|ui| {
            ui.set_opacity(appear_t);
        });
        let prev_opacity = ui.opacity();
        ui.set_opacity(prev_opacity * appear_t);

        let inner = frame.show(ui, |ui| {
            ui.set_width(self.width);
            ui.horizontal(|ui| {
                // Accent stripe / dot
                let painter = ui.painter();
                let cur = ui.cursor().min;
                painter.rect_filled(
                    Rect::from_min_size(cur, Vec2::new(3.0, font_md() + font_sm() + 6.0)),
                    egui::CornerRadius::same(2),
                    accent,
                );
                ui.add_space(8.0);
                ui.vertical(|ui| {
                    ui.label(RichText::new(self.title).monospace().size(font_md()).strong().color(self.text));
                    if let Some(b) = self.body {
                        ui.label(RichText::new(b).monospace().size(font_sm()).color(color_alpha(self.text, alpha_dim())));
                    }
                });
            });
        });

        ui.set_opacity(prev_opacity);

        ToastResponse { response: inner.response, auto_dismiss_due: due }
    }
}
