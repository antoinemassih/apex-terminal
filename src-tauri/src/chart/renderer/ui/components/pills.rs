//! Pills, chips, status badges, and the canonical `pill_button`.

use super::super::style::*;
use egui::{self, Color32, Response, RichText, Stroke, Ui, Vec2};

// ─── Pills / chips ────────────────────────────────────────────────────────────

/// Status pill — small accent-colored chip with text. Square under Meridien.
pub fn status_pill(ui: &mut Ui, text: &str, fill: Color32, fg: Color32) -> Response {
    let st = current();
    let cr = r_pill();
    let stroke = if st.hairline_borders {
        Stroke::new(st.stroke_hair, color_alpha(fill, alpha_dim()))
    } else {
        Stroke::NONE
    };
    ui.add(
        egui::Button::new(
            RichText::new(text)
                .monospace()
                .size(font_xs())
                .strong()
                .color(fg),
        )
        .fill(fill)
        .stroke(stroke)
        .corner_radius(cr)
        .min_size(Vec2::new(0.0, 14.0)),
    )
}

/// Pill button — interactive, style-aware corner radius.
/// Under Meridien (`solid_active_fills`), active fills solid with `fill`.
/// Under Relay, active is a low-alpha tint.
///
/// **Deprecated**: use [`pill_button`] for new code (simpler signature, single source of truth).
#[deprecated(since = "0.10.0", note = "Use `pill_button(ui, text, active, accent, dim)` — see docs/DESIGN_SYSTEM.md")]
pub fn pill_btn(
    ui: &mut Ui,
    text: &str,
    active: bool,
    fill: Color32,
    fg_active: Color32,
    fg_inactive: Color32,
) -> Response {
    let st = current();
    let cr = r_pill();

    let (bg, fg, stroke) = if active {
        if st.solid_active_fills {
            (fill, fg_active, Stroke::new(st.stroke_std, fill))
        } else {
            (
                color_alpha(fill, alpha_tint()),
                fg_active,
                Stroke::new(st.stroke_thin, color_alpha(fill, alpha_strong())),
            )
        }
    } else {
        (
            Color32::TRANSPARENT,
            fg_inactive,
            Stroke::new(st.stroke_thin, color_alpha(fg_inactive, alpha_muted())),
        )
    };

    let resp = ui.add(
        egui::Button::new(
            RichText::new(text)
                .monospace()
                .size(font_sm())
                .strong()
                .color(fg),
        )
        .fill(bg)
        .stroke(stroke)
        .corner_radius(cr)
        .min_size(Vec2::new(0.0, 18.0)),
    );
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

/// Status badge — small filled pill for things like DRAFT, ACTIVE, FILLED.
pub fn status_badge(ui: &mut Ui, text: &str, bg: Color32, fg: Color32) -> Response {
    let st = current();
    let cr = r_pill();
    let stroke = if st.hairline_borders {
        Stroke::new(st.stroke_hair, color_alpha(bg, alpha_strong()))
    } else {
        Stroke::NONE
    };
    ui.add(
        egui::Button::new(
            RichText::new(text)
                .monospace()
                .size(font_xs())
                .strong()
                .color(fg),
        )
        .fill(bg)
        .stroke(stroke)
        .corner_radius(cr)
        .min_size(Vec2::new(0.0, 12.0)),
    )
}

// ═══════════════════════════════════════════════════════════════════════════════
// Design-system: PillButton (canonical)
// Added by design-system rollout. See docs/DESIGN_SYSTEM.md.
// ═══════════════════════════════════════════════════════════════════════════════

/// Canonical pill toggle button. Replaces deprecated `pill_btn` and `filter_chip`.
///
/// - **Active**: accent-tinted fill, accent text, accent border.
/// - **Inactive**: transparent fill, dim text, dim border.
///
/// Uses `font_sm()`, `gap_md()` x-padding, `r_pill()` for pill shape.
pub fn pill_button(
    ui: &mut Ui,
    text: &str,
    active: bool,
    accent: Color32,
    dim: Color32,
) -> Response {
    let pill_r = r_pill();
    let (bg, fg, border) = if active {
        (
            color_alpha(accent, alpha_muted()),
            accent,
            color_alpha(accent, alpha_active()),
        )
    } else {
        (
            Color32::TRANSPARENT,
            dim,
            color_alpha(dim, alpha_dim()),
        )
    };

    let prev_pad = ui.spacing().button_padding;
    ui.spacing_mut().button_padding = egui::vec2(gap_md(), prev_pad.y);
    let resp = ui.add(
        egui::Button::new(
            RichText::new(text)
                .monospace()
                .size(font_sm())
                .color(fg),
        )
        .fill(bg)
        .stroke(Stroke::new(stroke_thin(), border))
        .corner_radius(pill_r)
        .min_size(egui::vec2(0.0, 18.0)),
    );
    ui.spacing_mut().button_padding = prev_pad;

    if resp.hovered() && !crate::design_tokens::is_inspect_mode() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

// ─── Brand colors (read from palette tokens) ──────────────────────────────────

/// Discord brand color — reads from `palette.discord` so brand surfaces stay
/// in sync with the design system. Falls back to the canonical Discord blurple
/// when design-mode is off.
#[inline]
pub fn discord_brand_color() -> Color32 {
    Color32::from_rgb(88, 101, 242)
}
