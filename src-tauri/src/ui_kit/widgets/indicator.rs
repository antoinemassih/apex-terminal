//! Indicator — small status dot for state visualization.
//!
//! Zed pattern: indicator dots show LSP states, git diff states,
//! connection state. Drop in any row alongside text labels. For Apex:
//! ApexData connection, market state (open/closed/pre/post), alert
//! active, signals feed health.
//!
//! API:
//!   ui.add(Indicator::dot().tone(IndicatorTone::Bull));
//!   ui.add(Indicator::pulsing().tone(IndicatorTone::Warn));
//!   ui.add(Indicator::dot().custom_color(theme.bull()));

use egui::{Color32, Response, Sense, Stroke, Ui, Vec2, Widget};

use super::motion;
use super::theme::ComponentTheme;
use crate::chart::renderer::ui::style as st;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum IndicatorTone {
    #[default]
    Neutral,
    Accent,
    Bull,
    Bear,
    Warn,
    Custom,
}

impl IndicatorTone {
    fn resolve(self, theme: &dyn ComponentTheme, custom: Option<Color32>) -> Color32 {
        match self {
            IndicatorTone::Neutral => theme.dim(),
            IndicatorTone::Accent => theme.accent(),
            IndicatorTone::Bull => theme.bull(),
            IndicatorTone::Bear => theme.bear(),
            IndicatorTone::Warn => theme.warn(),
            IndicatorTone::Custom => custom.unwrap_or_else(|| theme.dim()),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IndicatorStyle {
    /// Solid filled circle (default 6px).
    Dot,
    /// Hollow circle, 1.5px stroke; for less-prominent states.
    Ring,
    /// Dot with a faint outer ring that pulses 0..=1 over 1500ms.
    Pulsing,
}

#[must_use = "Indicator does nothing until `.show(ui, theme)` or `ui.add(indicator)` is called"]
pub struct Indicator {
    style: IndicatorStyle,
    tone: IndicatorTone,
    custom: Option<Color32>,
    size_px: f32,
}

impl Indicator {
    pub fn dot() -> Self {
        Self {
            style: IndicatorStyle::Dot,
            tone: IndicatorTone::default(),
            custom: None,
            size_px: 6.0,
        }
    }

    pub fn ring() -> Self {
        Self { style: IndicatorStyle::Ring, ..Self::dot() }
    }

    pub fn pulsing() -> Self {
        Self { style: IndicatorStyle::Pulsing, ..Self::dot() }
    }

    pub fn tone(mut self, t: IndicatorTone) -> Self {
        self.tone = t;
        self
    }

    /// Escape hatch for arbitrary colors. Sets tone to Custom and stores
    /// the color; takes precedence over the tone palette mapping.
    pub fn custom_color(mut self, c: Color32) -> Self {
        self.tone = IndicatorTone::Custom;
        self.custom = Some(c);
        self
    }

    pub fn size_px(mut self, px: f32) -> Self {
        self.size_px = px;
        self
    }

    pub fn show(self, ui: &mut Ui, theme: &dyn ComponentTheme) -> Response {
        let color = self.tone.resolve(theme, self.custom);
        // Reserve enough space for the pulsing outer ring (2x dot).
        let alloc = match self.style {
            IndicatorStyle::Pulsing => self.size_px * 2.0,
            _ => self.size_px,
        };
        let (rect, response) = ui.allocate_exact_size(Vec2::splat(alloc), Sense::hover());

        if !ui.is_rect_visible(rect) {
            return response;
        }

        let painter = ui.painter_at(rect);
        let center = rect.center();
        let radius = self.size_px * 0.5;

        match self.style {
            IndicatorStyle::Dot => {
                painter.circle_filled(center, radius, color);
            }
            IndicatorStyle::Ring => {
                painter.circle_stroke(center, radius, Stroke::new(st::stroke_bold(), color));
            }
            IndicatorStyle::Pulsing => {
                // Solid inner dot.
                painter.circle_filled(center, radius, color);

                // Outer ring pulse: cycles 0..=1 over 1500ms via a saw
                // target driven by wall-clock time. ease_value smooths the
                // jump back to 0 — visually a soft fade-in / quick reset.
                let id = response.id.with("pulse");
                let t_now = ui.input(|i| i.time) as f32;
                const PERIOD: f32 = 1.5;
                let phase = (t_now.rem_euclid(PERIOD)) / PERIOD; // 0..1 saw
                let eased = motion::ease_value(ui.ctx(), id, phase, 0.06);

                // Ring grows from 1.0x to 2.0x dot radius and fades alpha
                // from ~140 to 0 across the cycle.
                let outer_r = radius * (1.0 + eased);
                let alpha = ((1.0 - eased) * 140.0).clamp(0.0, 255.0) as u8;
                let ring_color = Color32::from_rgba_unmultiplied(
                    color.r(),
                    color.g(),
                    color.b(),
                    alpha,
                );
                painter.circle_stroke(center, outer_r, Stroke::new(st::stroke_std(), ring_color));

                // Keep animating while visible.
                ui.ctx().request_repaint();
            }
        }

        response
    }
}

impl Widget for Indicator {
    fn ui(self, ui: &mut Ui) -> Response {
        let theme = super::theme::active_theme(ui.ctx());
        self.show(ui, theme)
    }
}
