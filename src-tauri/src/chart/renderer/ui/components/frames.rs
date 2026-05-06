//! Frames — card, dialog, themed popup, and order card with accent stripe.

use super::super::style::*;
use egui::{self, Color32, Rect, Stroke, Ui, Vec2};

// ─── Frames ───────────────────────────────────────────────────────────────────

/// Card frame — surface with style-aware corners. Hairline border under Meridien;
/// soft border + drop shadow under Relay.
pub fn card_frame<R>(
    ui: &mut Ui,
    theme_bg: Color32,
    theme_border: Color32,
    add_contents: impl FnOnce(&mut Ui) -> R,
) -> R {
    let st = current();
    let mut frame = egui::Frame::NONE
        .fill(theme_bg)
        .corner_radius(r_md_cr())
        .inner_margin(egui::Margin::same(gap_lg() as i8));

    if st.hairline_borders {
        frame = frame.stroke(Stroke::new(
            st.stroke_std,
            color_alpha(theme_border, alpha_strong()),
        ));
    } else {
        frame = frame.stroke(Stroke::new(
            st.stroke_thin,
            color_alpha(theme_border, alpha_muted()),
        ));
    }

    if st.shadows_enabled {
        frame = frame.shadow(egui::epaint::Shadow {
            offset: [0, shadow_offset() as i8],
            blur: shadow_spread() as u8,
            spread: 0,
            color: Color32::from_black_alpha(shadow_alpha()),
        });
    }

    let mut out: Option<R> = None;
    frame.show(ui, |ui| {
        out = Some(add_contents(ui));
    });
    out.expect("card_frame contents")
}

/// Dialog frame — modal popups. Square + hairline under Meridien;
/// rounded + soft shadow under Relay.
pub fn dialog_frame<R>(
    ui: &mut Ui,
    theme_bg: Color32,
    theme_border: Color32,
    add_contents: impl FnOnce(&mut Ui) -> R,
) -> R {
    let st = current();
    let mut frame = egui::Frame::popup(&ui.ctx().style())
        .fill(theme_bg)
        .corner_radius(r_lg_cr())
        .inner_margin(egui::Margin::same(gap_xl() as i8));

    if st.hairline_borders {
        frame = frame.stroke(Stroke::new(st.stroke_std, theme_border));
    } else {
        frame = frame.stroke(Stroke::new(
            st.stroke_thin,
            color_alpha(theme_border, alpha_strong()),
        ));
    }

    if st.shadows_enabled {
        frame = frame.shadow(egui::epaint::Shadow {
            offset: [0, 8],
            blur: 28,
            spread: 2,
            color: Color32::from_black_alpha(80),
        });
    } else {
        // Meridien: explicitly clear any default popup shadow.
        frame = frame.shadow(egui::epaint::Shadow::NONE);
    }

    let mut out: Option<R> = None;
    frame.show(ui, |ui| {
        out = Some(add_contents(ui));
    });
    out.expect("dialog_frame contents")
}

// ─── Themed popup frame ───────────────────────────────────────────────────────

/// Pre-themed `egui::Frame` for use inside `egui::Window::frame(...)` and
/// similar contexts where the caller cannot pass a closure.
/// Replaces hand-rolled `Frame::popup(...).fill(...).stroke(...).corner_radius(...)`
/// boilerplate. Honors `hairline_borders` and `shadows_enabled`.
pub fn themed_popup_frame(
    ctx: &egui::Context,
    theme_bg: Color32,
    theme_border: Color32,
) -> egui::Frame {
    let st = current();
    // Under Meridien, popup bg is slightly LIGHTER than the canvas — picks the
    // popup off the surrounding chrome with the soft drop-shadow.
    let pop_bg = if st.hairline_borders {
        theme_bg.gamma_multiply(1.10)
    } else {
        theme_bg
    };
    let mut frame = egui::Frame::popup(&ctx.style())
        .fill(pop_bg)
        .corner_radius(r_lg_cr())
        .inner_margin(egui::Margin::same(gap_lg() as i8));

    if st.hairline_borders {
        frame = frame.stroke(Stroke::new(st.stroke_std, theme_border));
    } else {
        frame = frame.stroke(Stroke::new(
            st.stroke_thin,
            color_alpha(theme_border, alpha_strong()),
        ));
    }

    if st.shadows_enabled {
        // Soft, diffused drop-shadow tuned to match the Meridien close-up
        // reference — low offset, generous blur, near-zero spread, faint alpha.
        frame = frame.shadow(egui::epaint::Shadow {
            offset: [0, 8],
            blur: 24,
            spread: 1,
            color: Color32::from_black_alpha(40),
        });
    } else {
        frame = frame.shadow(egui::epaint::Shadow::NONE);
    }

    frame
}

// ─── Cards ────────────────────────────────────────────────────────────────────

/// Accent card — card with a left accent stripe and explicit border color param.
/// Returns generic R from `add_contents`. Distinct from style::order_card (which
/// takes no border param and returns bool).
pub fn accent_card<R>(
    ui: &mut Ui,
    accent: Color32,
    bg: Color32,
    border: Color32,
    add_contents: impl FnOnce(&mut Ui) -> R,
) -> R {
    let st = current();
    let mut frame = egui::Frame::NONE
        .fill(bg)
        .corner_radius(r_md_cr())
        .inner_margin(egui::Margin {
            left: gap_md() as i8 + 3,
            right: gap_lg() as i8,
            top: gap_md() as i8,
            bottom: gap_md() as i8,
        });
    if st.hairline_borders {
        frame = frame.stroke(Stroke::new(st.stroke_std, border));
    } else {
        frame = frame.stroke(Stroke::new(st.stroke_thin, color_alpha(border, alpha_muted())));
    }

    let mut out: Option<R> = None;
    let resp = frame.show(ui, |ui| {
        // Paint the left accent stripe inside the frame.
        let max = ui.max_rect();
        ui.painter().rect_filled(
            Rect::from_min_size(max.min, Vec2::new(2.5, max.height())),
            r_xs(),
            accent,
        );
        out = Some(add_contents(ui));
    });
    let _ = resp;
    out.expect("accent_card contents")
}
