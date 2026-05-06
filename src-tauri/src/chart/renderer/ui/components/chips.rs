//! Chips: keybind hint, filter (deprecated), display, removable, notification badge.

use super::super::style::*;
use egui::{self, Color32, Response, RichText, Stroke, Ui, Vec2};

// ─── Keyboard shortcut chip ───────────────────────────────────────────────────

/// Keyboard shortcut hint chip — small pill with hint text (Cmd+K, Esc).
pub fn keybind_chip(ui: &mut Ui, hint: &str, fg: Color32, bg_border: Color32) -> Response {
    let st = current();
    let cr = r_xs();
    let stroke = if st.hairline_borders {
        Stroke::new(st.stroke_std, color_alpha(bg_border, alpha_strong()))
    } else {
        Stroke::new(st.stroke_thin, color_alpha(bg_border, alpha_muted()))
    };
    ui.add(
        egui::Button::new(
            RichText::new(hint).monospace().size(font_xs()).color(fg),
        )
        .fill(Color32::TRANSPARENT)
        .stroke(stroke)
        .corner_radius(cr)
        .min_size(Vec2::new(0.0, 14.0)),
    )
}

// ─── Filter chip ──────────────────────────────────────────────────────────────

/// Filter chip — togglable inline tag.
/// Filter chip toggle.
///
/// **Deprecated**: use [`super::super::components::pill_button`] for new code.
#[deprecated(since = "0.10.0", note = "Use `pill_button(ui, text, active, accent, dim)` — see docs/DESIGN_SYSTEM.md")]
pub fn filter_chip(
    ui: &mut Ui,
    text: &str,
    active: bool,
    accent: Color32,
    fg_inactive: Color32,
) -> Response {
    let st = current();
    let cr = r_pill();

    let (bg, fg, stroke) = if active {
        if st.solid_active_fills {
            (accent, contrast_fg_local(accent), Stroke::NONE)
        } else {
            (
                color_alpha(accent, alpha_tint()),
                accent,
                Stroke::new(st.stroke_thin, color_alpha(accent, alpha_strong())),
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
                .size(font_xs())
                .color(fg),
        )
        .fill(bg)
        .stroke(stroke)
        .corner_radius(cr)
        .min_size(Vec2::new(0.0, 16.0)),
    );
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

// ─── Notification badge ───────────────────────────────────────────────────────

/// Small filled pill with a count. Used to indicate unread items.
pub fn notification_badge(ui: &mut Ui, count: u32, accent: Color32, fg: Color32) -> Response {
    let cr = r_pill();
    let text = if count > 99 { "99+".to_string() } else { count.to_string() };
    ui.add(
        egui::Button::new(
            RichText::new(text)
                .monospace()
                .size(font_xs())
                .strong()
                .color(fg),
        )
        .fill(accent)
        .stroke(Stroke::NONE)
        .corner_radius(cr)
        .min_size(Vec2::new(14.0, 14.0)),
    )
}

// ─── Display + removable chips ────────────────────────────────────────────────

/// Display chip — non-interactive status indicator. Uses the same shape and
/// sizing as `pill_button`; no click behavior. Pass a single semantic color
/// (e.g. session_col, paper_orange, live_green); the chip tints its bg with
/// `alpha_tint()` and uses the color for the border + text.
pub fn display_chip(
    ui: &mut Ui,
    label: &str,
    color: Color32,
) -> Response {
    let prev_pad = ui.spacing().button_padding;
    ui.spacing_mut().button_padding = egui::vec2(gap_md(), 0.0);
    let resp = ui.add(
        egui::Button::new(
            RichText::new(label)
                .monospace()
                .size(font_xs())
                .strong()
                .color(color),
        )
        .fill(color_alpha(color, alpha_tint()))
        .stroke(Stroke::new(stroke_thin(), color_alpha(color, alpha_dim())))
        .corner_radius(r_pill())
        .min_size(egui::vec2(0.0, 14.0))
        .sense(egui::Sense::hover()),
    );
    ui.spacing_mut().button_padding = prev_pad;
    resp
}

/// Removable chip — text + ✕ in a single pill. Returns
/// `(label_resp, x_clicked)` so the caller can act on either.
/// Visual signature matches `pill_button`.
pub fn removable_chip(
    ui: &mut Ui,
    text: &str,
    accent: Color32,
    dim: Color32,
) -> (Response, bool) {
    let mut x_clicked = false;
    let resp = ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        let prev_pad = ui.spacing().button_padding;
        ui.spacing_mut().button_padding = egui::vec2(gap_md(), 0.0);
        // Body label (looks like a pill, no hover affordance)
        let body = ui.add(
            egui::Button::new(
                RichText::new(text)
                    .monospace()
                    .size(font_sm())
                    .color(dim),
            )
            .fill(color_alpha(accent, alpha_faint()))
            .stroke(Stroke::new(stroke_thin(), color_alpha(dim, alpha_dim())))
            .corner_radius(egui::CornerRadius { nw: 99, sw: 99, ne: 0, se: 0 })
            .min_size(egui::vec2(0.0, 18.0)),
        );
        // ✕ remove button (paired)
        let x = ui.add(
            egui::Button::new(
                RichText::new("\u{00D7}")
                    .monospace()
                    .size(font_sm())
                    .color(dim),
            )
            .fill(color_alpha(accent, alpha_faint()))
            .stroke(Stroke::new(stroke_thin(), color_alpha(dim, alpha_dim())))
            .corner_radius(egui::CornerRadius { nw: 0, sw: 0, ne: 99, se: 99 })
            .min_size(egui::vec2(18.0, 18.0)),
        );
        ui.spacing_mut().button_padding = prev_pad;
        if x.clicked() { x_clicked = true; }
        if x.hovered() && !crate::design_tokens::is_inspect_mode() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        body
    }).inner;
    (resp, x_clicked)
}

// ─── Local utility ────────────────────────────────────────────────────────────

#[inline]
fn contrast_fg_local(bg: Color32) -> Color32 {
    let r = bg.r() as f32 * 0.299;
    let g = bg.g() as f32 * 0.587;
    let b = bg.b() as f32 * 0.114;
    if r + g + b > 140.0 { Color32::from_rgb(20, 20, 20) } else { Color32::from_rgb(245, 245, 245) }
}
