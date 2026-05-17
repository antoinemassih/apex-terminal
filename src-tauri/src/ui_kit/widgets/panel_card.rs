//! PanelCard — content card primitive for panel bodies.
//!
//! Replaces ad-hoc `ui.group()`, `order_card`, hand-rolled shadow card, and
//! scanner builder card patterns with a single layered primitive that uses
//! the L2 surface (one step up from `toolbar_bg`) for its background.
//!
//! ```ignore
//! PanelCard::new()
//!     .tone(Tone::Default)
//!     .stripe(true)
//!     .padding(gap_md())
//!     .show(ui, t, |ui, t| { /* card body */ });
//! ```
//!
//! Visual spec (locked):
//! - Background: L2 surface — `color_layer_up(t, 1)` when available (Agent J),
//!   otherwise an inlined approximation: lift `t.toolbar_bg` by ~4% luminance.
//!   This is deliberately a tiny step so a card sits ON the panel, not
//!   floating above it.
//! - **No border by default.** Layering instead of strokes.
//! - Corner radius: `radius_md()`.
//! - Shadow: only when `tone != Default` (toned cards get `shadow_card_themed`
//!   so they read with a touch of elevation).
//! - Optional 2px LEFT accent stripe in `tone.color(t)` when `.stripe(true)`.
//! - Body padding: `gap_md` default; override via `.padding(...)`.
//!
//! When to use:
//! - Grouping related fields/rows that should read as one unit (a setup, an
//!   order in flight, a closed trade summary).
//!
//! When NOT to use:
//! - Repeating list items — use `PanelListRow`.
//! - Single-line label/value pairs — use `PanelKeyValueRow`.
//! - Full panel chrome — use `PanelSection`.
//!
//! Sister widgets: `PanelSection`, `PanelListRow`, `TradeCard`.

use egui::{Color32, CornerRadius, Frame, Margin, Pos2, Rect, Stroke, Ui};

use super::panel_section::Tone;
use crate::chart::renderer::ui::style::{
    gap_md, radius_md, shadow_card_themed,
};
use crate::chart_renderer::gpu::Theme;

#[must_use = "PanelCard must be rendered with `.show(...)`"]
pub struct PanelCard {
    tone: Tone,
    stripe: bool,
    padding: f32,
}

impl Default for PanelCard {
    fn default() -> Self {
        Self::new()
    }
}

impl PanelCard {
    pub fn new() -> Self {
        Self {
            tone: Tone::Default,
            stripe: false,
            padding: gap_md(),
        }
    }

    pub fn tone(mut self, t: Tone) -> Self {
        self.tone = t;
        self
    }

    pub fn stripe(mut self, on: bool) -> Self {
        self.stripe = on;
        self
    }

    pub fn padding(mut self, px: f32) -> Self {
        self.padding = px;
        self
    }

    pub fn show<R>(
        self,
        ui: &mut Ui,
        t: &Theme,
        body: impl FnOnce(&mut Ui, &Theme) -> R,
    ) -> R {
        let bg = card_surface(t);
        let radius = CornerRadius::same(radius_md() as u8);
        let pad = self.padding as i8;
        let mut frame = Frame::NONE
            .fill(bg)
            .corner_radius(radius)
            .inner_margin(Margin {
                left: pad,
                right: pad,
                top: pad,
                bottom: pad,
            });
        if self.tone != Tone::Default {
            frame = frame.shadow(shadow_card_themed(t));
        }

        let resp = frame.show(ui, |ui| body(ui, t));

        // Optional left accent stripe — painted on top after the frame.
        if self.stripe {
            let r = resp.response.rect;
            let stripe_rect = Rect::from_min_max(
                Pos2::new(r.left(), r.top()),
                Pos2::new(r.left() + 2.0, r.bottom()),
            );
            ui.painter().rect_filled(
                stripe_rect,
                CornerRadius {
                    nw: radius.nw,
                    sw: radius.sw,
                    ne: 0,
                    se: 0,
                },
                self.tone.color(t),
            );
            // Suppress an unused-Stroke warning from imports while keeping
            // the import explicit for future border-on-tone variants.
            let _ = Stroke::NONE;
        }

        resp.inner
    }
}

/// L2 card surface. TODO: replace with `color_layer_up(t, 1)` once Agent J
/// merges that helper into `style.rs`. Until then, inline a small uniform
/// lift of `t.toolbar_bg` toward `t.text` (~4%) — works for both dark and
/// light themes because we move toward the theme's foreground, not toward
/// hardcoded white/black.
fn card_surface(t: &Theme) -> Color32 {
    // TODO: replace with color_layer_up(t, 1) once Agent J merges
    let bg = t.toolbar_bg;
    let fg = t.text;
    blend(bg, fg, 0.04)
}

fn blend(a: Color32, b: Color32, w: f32) -> Color32 {
    let w = w.clamp(0.0, 1.0);
    let lerp = |x: u8, y: u8| -> u8 {
        let xf = x as f32;
        let yf = y as f32;
        (xf + (yf - xf) * w).round().clamp(0.0, 255.0) as u8
    };
    Color32::from_rgba_unmultiplied(
        lerp(a.r(), b.r()),
        lerp(a.g(), b.g()),
        lerp(a.b(), b.b()),
        a.a().max(b.a()),
    )
}
