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

use egui::{CornerRadius, Frame, Margin, Pos2, Rect, Stroke, Ui};

use super::panel_section::Tone;
use crate::ui_kit::tokens::gap_md;
use crate::ui_kit::widgets::theme::{ComponentTheme, get_ambient_recipes};
use crate::ui_kit::sx::{Sx, StyleState};

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
        let float = t.cards_float();
        // Rounder cards on tiled styles (radius_lg — the signature bigger radius),
        // sharp on flush styles (Meridien/Mariner). Never hardcoded.
        let rr = if float { crate::ui_kit::style::radius_lg() as u8 } else { 0 };
        let radius = CornerRadius::same(rr);
        let pad = self.padding as i8;
        // Refined per-style edge: a subtle text-alpha hairline. Slightly stronger
        // than before so the card reads as a distinct raised surface.
        let border = crate::ui_kit::style::color_alpha(t.text(), crate::ui_kit::style::alpha_soft());
        // Outer margin so cards breathe from the panel edges and from each other
        // (stacked cards) instead of sitting flush against the borders.
        let om = crate::ui_kit::style::gap_sm() as i8;
        // ── Card chrome: RECIPE first, CardRecipe as fallback ────────────────
        //
        // Two mechanisms described a card and disagreed. `StyleSystem.card`
        // (M1 Change D) was consumed here; the `"card"` RECIPE key was authored
        // by all six styles and consumed by nothing. Decision: the recipe wins
        // — it is the designed per-theme mechanism, the same one Button, Tag
        // and Tabs resolve through, and it is authored everywhere rather than
        // on four of six styles.
        //
        // The legacy `StyleSystem.card` (CardRecipe) that used to be consulted
        // here is GONE. It was reachable only for a style that authored it and
        // NOT a `card` recipe; every builtin authors the recipe, so the branch
        // was dead on arrival. Keeping a second card mechanism alive "just in
        // case" is how the duplication started.
        //
        // `border_width: None` / a recipe with no border still means NO STROKE
        // AT ALL, not a zero-width one — Aperture's borderless lifted tiles.
        let recipes = get_ambient_recipes(ui.ctx());
        let default_card_sx = Sx::new();
        // Floating styles (Aperture / Cadence / Glass — `cards_float()`) get
        // `card.floating` when a style authors it, falling back to `card`.
        // That variant was authored and resolved by nobody, so a style could
        // describe its LIFTED card and only its flat one would ever paint.
        let card_key = if float { "card.floating" } else { "card" };
        let card_sx = recipes.resolve(card_key, default_card_sx, t);
        // A style that floats but authors only the base key still themes.
        let card_sx = if float && card_sx.resolved(StyleState::Normal).radius.is_none() {
            recipes.resolve("card", Sx::new(), t)
        } else {
            card_sx
        };
        let card_delta = card_sx.resolved(StyleState::Normal);
        let recipe_border = card_delta.border_spec();
        let recipe_authored =
            card_delta.radius.is_some() || card_delta.pad_x().is_some() || recipe_border.is_some();

        let (radius, pad, border_stroke) = if recipe_authored {
            let r = card_delta.radius.unwrap_or(rr as f32);
            let p = card_delta.pad_x().unwrap_or(self.padding);
            // An authored recipe with NO border means no stroke at all — that is
            // how Aperture ships `--ds-card-border: none`.
            let pal = crate::ui_kit::sx::palette_ct(t);
            let stroke = match recipe_border {
                Some(b) if b.width > 0.0 => Stroke::new(
                    b.width,
                    card_delta.resolved_border_color(&pal).unwrap_or(border),
                ),
                _ => Stroke::NONE,
            };
            (
                egui::CornerRadius::same(r.clamp(0.0, 255.0).round() as u8),
                p as i8,
                stroke,
            )
        } else {
            (radius, pad, Stroke::new(crate::ui_kit::style::stroke_thin(), border))
        };
        let mut frame = Frame::NONE
            .fill(bg)
            .corner_radius(radius)
            .stroke(border_stroke)
            .outer_margin(Margin { left: om, right: om, top: om / 2, bottom: om / 2 })
            .inner_margin(Margin {
                left: pad,
                right: pad,
                top: pad,
                bottom: pad,
            });
        // Slight soft drop shadow on FLOATING styles (Aperture/Cadence/Glass) or
        // when toned. Flat styles rely on border + fill only.
        //
        // M1 Change E: when the active style AUTHORS a card shadow stack
        // (e.g. Alto's 4-layer Zed bevel, Lucid's 2-layer paper drop), the
        // stack replaces the legacy single Frame shadow. OUTER layers paint
        // before the frame (under the fill); INSET layers paint after (inside
        // the rect) — matching CSS box-shadow semantics.
        let stack = crate::ui_kit::style::card_shadow_layers();
        let use_stack = (float || self.tone != Tone::Default) && !stack.is_empty();
        if (float || self.tone != Tone::Default) && !use_stack {
            frame = frame.shadow(t.shadow_card());
        }

        let resp = if use_stack {
            // Outer layers need the rect before the frame paints; egui gives
            // us the rect only after. Paint the whole stack AFTER instead:
            // outer layers under-draw slightly over neighbors (acceptable for
            // soft ambient drops), inset hairlines land exactly.
            let r = frame.show(ui, |ui| body(ui, t));
            let outer: Vec<_> = stack.iter().copied().filter(|l| !l.inset).collect();
            let inset: Vec<_> = stack.iter().copied().filter(|l| l.inset).collect();
            let rect = r.response.rect;
            let painter = ui.painter();
            // ambient drops first (behind-ish), then inset hairlines on top
            crate::ui_kit::style::paint_shadow_stack(painter, rect, radius, &outer, t.shadow_color());
            crate::ui_kit::style::paint_shadow_stack(painter, rect, radius, &inset, t.shadow_color());
            r
        } else {
            frame.show(ui, |ui| body(ui, t))
        };

        // Texture / elevation cue: a 1px top-highlight bevel just inside the top
        // edge (lighter than the surface) so the card catches a hint of light —
        // reads as raised, not flat. Floating styles only; inset past the corners.
        if float {
            let r = resp.response.rect;
            let inset = (rr as f32).max(2.0);
            ui.painter().line_segment(
                [egui::pos2(r.left() + inset, r.top() + 0.5),
                 egui::pos2(r.right() - inset, r.top() + 0.5)],
                Stroke::new(crate::ui_kit::style::stroke_std(), crate::ui_kit::style::color_alpha(t.text(), 12)),
            );
        }

        // Optional left accent stripe — painted on top after the frame.
        if self.stripe {
            let r = resp.response.rect;
            let stripe_rect = Rect::from_min_max(
                Pos2::new(r.left(), r.top()),
                Pos2::new(r.left() + crate::ui_kit::style::gap_2xs(), r.bottom()),
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

