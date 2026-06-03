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
use crate::ui_kit::tokens::{
    color_layer_up, gap_md, radius_md, shadow_card_themed,
};
use crate::ui_kit::widgets::theme::ComponentTheme;
use crate::ui_kit::sx::{palette_ct, Tone as SxTone};

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

    pub fn show<T: ComponentTheme, R>(
        self,
        ui: &mut Ui,
        t: &T,
        body: impl FnOnce(&mut Ui, &T) -> R,
    ) -> R {
        let bg = t.color_layer_up(1);
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
            frame = frame.shadow(t.shadow_card());
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

// `card_surface` removed — callers now use `t.color_layer_up(1)` directly
// via the trait method (portable).

