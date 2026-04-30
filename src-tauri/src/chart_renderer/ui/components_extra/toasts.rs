//! Toasts, loading spinners, breadcrumbs, and small captions.

use super::super::style::*;
use egui::{self, Color32, Rect, Response, RichText, Stroke, Ui, Vec2};

// ─── Toast card ───────────────────────────────────────────────────────────────

/// Toast notification card — accent-stripe + monospace text.
pub fn toast_card(
    ui: &mut Ui,
    accent: Color32,
    bg: Color32,
    fg: Color32,
    text: &str,
) {
    let st = current();
    let mut frame = egui::Frame::NONE
        .fill(bg)
        .corner_radius(r_md_cr())
        .inner_margin(egui::Margin::same(gap_md() as i8));

    if st.hairline_borders {
        frame = frame.stroke(Stroke::new(st.stroke_std, color_alpha(accent, alpha_strong())));
    } else {
        frame = frame.stroke(Stroke::new(st.stroke_thin, color_alpha(accent, alpha_muted())));
    }
    if st.shadows_enabled {
        frame = frame.shadow(egui::epaint::Shadow {
            offset: [0, 4],
            blur: 16,
            spread: 1,
            color: Color32::from_black_alpha(60),
        });
    }

    frame.show(ui, |ui| {
        let max = ui.max_rect();
        ui.painter().rect_filled(
            Rect::from_min_size(max.min, Vec2::new(2.5, max.height())),
            r_xs(),
            accent,
        );
        ui.label(
            RichText::new(text)
                .monospace()
                .size(font_sm())
                .color(fg),
        );
    });
}

// ─── Loading dots ─────────────────────────────────────────────────────────────

/// Animated three-dot loading indicator.
pub fn loading_dots(ui: &mut Ui, color: Color32) {
    let now = ui.input(|i| i.time);
    let phase = (now * 4.0) as usize % 3;
    let dot = |i: usize| if i == phase { "\u{25CF}" } else { "\u{25CB}" };
    ui.horizontal(|ui| {
        for i in 0..3 {
            ui.label(RichText::new(dot(i)).size(font_md()).color(color));
        }
    });
    ui.ctx().request_repaint();
}

// ─── Breadcrumb ───────────────────────────────────────────────────────────────

/// Path breadcrumb — segments separated by " / ". Last segment styled accent.
pub fn breadcrumb(ui: &mut Ui, segments: &[&str], accent: Color32, dim: Color32) {
    ui.horizontal(|ui| {
        let prev = ui.spacing().item_spacing.x;
        ui.spacing_mut().item_spacing.x = gap_xs();
        let last = segments.len().saturating_sub(1);
        for (i, seg) in segments.iter().enumerate() {
            let is_last = i == last;
            let color = if is_last { accent } else { dim };
            ui.label(
                RichText::new(*seg)
                    .monospace()
                    .size(font_sm())
                    .color(color),
            );
            if !is_last {
                ui.label(
                    RichText::new("/")
                        .monospace()
                        .size(font_sm())
                        .color(color_alpha(dim, alpha_muted())),
                );
            }
        }
        ui.spacing_mut().item_spacing.x = prev;
    });
}

// ─── Caption ──────────────────────────────────────────────────────────────────

/// Secondary caption — dim, font_xs. For URLs, timestamps, hint text under
/// primary labels.
pub fn caption_label(ui: &mut Ui, text: &str, dim: Color32) -> Response {
    ui.label(
        RichText::new(text)
            .monospace()
            .size(font_xs())
            .color(color_alpha(dim, alpha_dim())),
    )
}
