//! Painter-based chrome helpers for chart-overlay widgets.
//!
//! Unlike `components.rs` / `style.rs` which assume `egui::Ui` flow layout,
//! these helpers paint directly to an `egui::Painter` at absolute coordinates.
//! They are used by `chart_widgets.rs` where every widget renders as a
//! free-floating overlay on the chart canvas.
//!
//! Each helper respects the active `UiStyle` (`current()`) so corner radii
//! and stroke weights flow with the active theme (Relay vs Meridien).

use egui::{Color32, FontId, Painter, Rect, Stroke, StrokeKind};
use super::super::style::*;

/// A small painter-based button: filled bg, stroked border, centered glyph/label.
///
/// Used for the floating header's `ctx` and `toggle` chips, and for body action
/// buttons like "Close All" and the per-row close-X. The visual ramps from a
/// dimmed resting state to an accent-tinted hovered state.
///
/// Parameters:
/// - `rect`        — button rect (already laid out by caller).
/// - `text`        — glyph or short label rendered centered.
/// - `font`        — font for the label (proportional for icons, monospace for labels).
/// - `accent`      — color used for the hovered fill / border / text ramp.
/// - `dim_text`    — text color when not hovered (caller picks `dim` vs `bear` etc).
/// - `border_dim`  — border color when not hovered.
/// - `hovered`     — whether the pointer is over the rect.
/// - `rest_alpha`  — bg alpha when not hovered (e.g. 25 for chip, 40 for danger-soft).
/// - `hot_alpha`   — bg alpha when hovered (e.g. 50 for chip, 80–100 for danger-strong).
pub fn paint_painter_btn(
    p: &Painter,
    rect: Rect,
    text: &str,
    font: FontId,
    accent: Color32,
    dim_text: Color32,
    border_dim: Color32,
    hovered: bool,
    rest_alpha: u8,
    hot_alpha: u8,
) {
    let s = current();
    let cr = r_md_cr();
    let bg = color_alpha(accent, if hovered { hot_alpha } else { rest_alpha });
    let border = if hovered { accent } else { border_dim };
    let stroke_w = if hovered { s.stroke_std } else { s.stroke_thin };
    let fg = if hovered { accent } else { dim_text };

    p.rect_filled(rect, cr, bg);
    p.rect_stroke(rect, cr, Stroke::new(stroke_w, border), StrokeKind::Outside);
    p.text(rect.center(), egui::Align2::CENTER_CENTER, text, font, fg);
}
