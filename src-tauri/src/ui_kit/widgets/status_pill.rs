//! StatusPill — small rounded-rect status indicator.
//!
//! A compact read-only badge that communicates connection/readiness states
//! in diagnostics panels, connection bars, or panel footers. Unlike [`Tag`]
//! (which is for categories and can be closable), `StatusPill` has no
//! interactive affordance and always renders in one of the semantic tones.
//!
//! ```ignore
//! StatusPill::new("Connected").tone(PanelTone::Bull).show(ui, t);
//! StatusPill::new("Reconnecting").tone(PanelTone::Warn).size(Size::Xs).show(ui, t);
//! StatusPill::new("Disconnected").tone(PanelTone::Bear).show(ui, t);
//! ```
//!
//! Visual spec:
//! - Fill: `color_alpha(tone_color, alpha_hint())` (soft tint, not a saturated
//!   block).
//! - Text: full `tone_color` for contrast against the tinted fill.
//! - Border: `color_alpha(tone_color, alpha_muted())` hairline via `stroke_thin()`.
//! - Corner radius: `radius_sm()` (Xs) / `radius_md()` (Sm) — or `radius_pill()`
//!   when the pill is very short to produce a natural capsule shape.
//! - Size::Xs: height 14 px, `font_xs` text, `gap_2xs` horizontal padding.
//! - Size::Sm: height 18 px, `font_xs` text, `gap_xs` horizontal padding.
//! - No interactive sense (Sense::hover only) — use [`Tag`] if you need a
//!   click target.
//!
//! When to use:
//! - Connection / readiness states ("Connected", "Reconnecting", "Degraded").
//! - Pipeline health indicators in diagnostics panels.
//! - Short lifecycle state labels in panel footers (< 12 chars).
//!
//! When NOT to use:
//! - User-applied labels / categories — use [`Tag`].
//! - Notification counts — use [`Badge`].
//! - Action buttons — use [`Button`] with `Variant::Chip`.
//! - Long multi-word statuses — use [`Alert`].
//!
//! Sister widgets: [`Tag`], [`Badge`], [`Indicator`], [`Alert`].

use egui::{Color32, CornerRadius, FontId, Pos2, Sense, Stroke, StrokeKind, Ui, Vec2};

use crate::ui_kit::tokens::{
    alpha_hint, alpha_muted, color_alpha, font_xs, gap_2xs, gap_xs, radius_md, radius_sm,
    stroke_thin,
};
use crate::ui_kit::widgets::theme::Theme;
use crate::ui_kit::widgets::panel_section::Tone as PanelTone;
use crate::ui_kit::widgets::tokens::Size;

#[must_use = "StatusPill must be rendered with `.show(...)`"]
pub struct StatusPill<'a> {
    label: &'a str,
    tone: PanelTone,
    size: Size,
    dot: bool,
}

impl<'a> StatusPill<'a> {
    pub fn new(label: &'a str) -> Self {
        Self {
            label,
            tone: PanelTone::Default,
            size: Size::Sm,
            dot: false,
        }
    }

    /// Semantic tone — resolves to a theme color at paint time.
    pub fn tone(mut self, t: PanelTone) -> Self {
        self.tone = t;
        self
    }

    /// `Size::Xs` (14 px) or `Size::Sm` (18 px, default). Larger sizes are
    /// clamped to `Sm`; StatusPill is intentionally compact.
    pub fn size(mut self, s: Size) -> Self {
        self.size = match s {
            Size::Xs => Size::Xs,
            _ => Size::Sm,
        };
        self
    }

    /// Prepend a filled dot indicator before the label for quick scanability.
    pub fn dot(mut self, v: bool) -> Self {
        self.dot = v;
        self
    }

    pub fn show(self, ui: &mut Ui, t: &Theme) {
        let tone_color = self.tone.color(t);

        let (h, pad_x, cr) = match self.size {
            Size::Xs => (14.0_f32, gap_2xs(), CornerRadius::same(radius_sm() as u8)),
            _ => (18.0_f32, gap_xs(), CornerRadius::same(radius_md() as u8)),
        };

        let font_size = font_xs();
        let font = FontId::proportional(font_size);

        let dot_size: f32 = if self.dot { 5.0 } else { 0.0 };
        let dot_gap: f32 = if self.dot { gap_2xs() } else { 0.0 };

        let galley = ui.fonts(|f| {
            f.layout_no_wrap(self.label.to_string(), font.clone(), tone_color)
        });
        let label_w = galley.size().x;

        let content_w = dot_size + dot_gap + label_w;
        let total_w = content_w + pad_x * 2.0;

        let desired = Vec2::new(total_w, h);
        let (rect, _) = ui.allocate_exact_size(desired, Sense::hover());

        if !ui.is_rect_visible(rect) {
            return;
        }

        let painter = ui.painter_at(rect);

        // Fill: soft tint.
        painter.rect_filled(rect, cr, color_alpha(tone_color, alpha_hint()));
        // Border: hairline.
        painter.rect_stroke(
            rect,
            cr,
            Stroke::new(stroke_thin(), color_alpha(tone_color, alpha_muted())),
            StrokeKind::Inside,
        );

        let cy = rect.center().y;
        let mut x = rect.left() + pad_x;

        // Optional status dot.
        if self.dot {
            let dot_center = Pos2::new(x + dot_size * 0.5, cy);
            painter.circle_filled(dot_center, dot_size * 0.5, tone_color);
            x += dot_size + dot_gap;
        }

        // Label text.
        let th = galley.size().y;
        painter.galley(Pos2::new(x, cy - th * 0.5), galley, tone_color);
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Builder round-trip: label, tone, size, dot are stored.
    #[test]
    fn builder_fields() {
        let p = StatusPill::new("Connected")
            .tone(PanelTone::Bull)
            .size(Size::Xs)
            .dot(true);
        assert_eq!(p.label, "Connected");
        assert_eq!(p.tone, PanelTone::Bull);
        assert_eq!(p.size, Size::Xs);
        assert!(p.dot);
    }

    /// Size clamping: Md is normalised down to Sm.
    #[test]
    fn size_clamp() {
        let p = StatusPill::new("OK").size(Size::Md);
        assert_eq!(p.size, Size::Sm);
        let p2 = StatusPill::new("OK").size(Size::Lg);
        assert_eq!(p2.size, Size::Sm);
    }

    /// Default tone is Default (dim).
    #[test]
    fn default_tone() {
        let p = StatusPill::new("Idle");
        assert_eq!(p.tone, PanelTone::Default);
    }

    /// Dot off by default.
    #[test]
    fn dot_default_off() {
        let p = StatusPill::new("Ready");
        assert!(!p.dot);
    }
}
