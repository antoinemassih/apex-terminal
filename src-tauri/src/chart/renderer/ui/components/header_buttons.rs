//! Header glyph buttons + tab bar with close affordance.

use super::super::style::*;
use egui::{self, Color32, Response, RichText, Stroke, Ui, Vec2};

// ─── Header action button ─────────────────────────────────────────────────────

/// Tiny transparent ghost glyph button for panel headers (+, ×, ⚙).
/// Frameless, dim color, hover changes cursor. Used in compact header rows
/// where a full `icon_btn` is too prominent.
pub fn header_action_btn(ui: &mut Ui, glyph: &str, dim: Color32) -> Response {
    let resp = ui.add(
        egui::Button::new(
            RichText::new(glyph)
                .monospace()
                .size(font_md())
                .color(dim),
        )
        .frame(false)
        .min_size(Vec2::new(14.0, 14.0)),
    );
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

/// Smaller, dimmer variant of `style::close_button` for secondary close
/// affordances inside split sections / nested headers.
pub fn secondary_close_btn(ui: &mut Ui, dim: Color32) -> bool {
    let resp = ui.add(
        egui::Button::new(
            RichText::new("\u{00D7}")
                .monospace()
                .size(font_sm())
                .color(color_alpha(dim, alpha_dim())),
        )
        .frame(false)
        .min_size(Vec2::new(14.0, 14.0)),
    );
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp.clicked()
}

// ─── Tab bar with close ───────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TabAction {
    None,
    Selected(usize),
    Closed(usize),
}

/// Tab strip with a small × on each tab. Returns the action triggered.
/// Active tab visual: pill bg under Relay, hairline bottom-rule under Meridien.
pub fn tab_bar_with_close(
    ui: &mut Ui,
    tabs: &[&str],
    active: usize,
    accent: Color32,
    dim: Color32,
) -> TabAction {
    let st = current();
    let mut action = TabAction::None;

    ui.horizontal(|ui| {
        let prev_x = ui.spacing().item_spacing.x;
        ui.spacing_mut().item_spacing.x = gap_xs();

        for (i, label) in tabs.iter().enumerate() {
            let is_active = i == active;
            let fg = if is_active { accent } else { dim };
            let s = style_label_case(label);

            // Per-tab cluster (label + ×)
            ui.horizontal(|ui| {
                let prev_inner = ui.spacing().item_spacing.x;
                ui.spacing_mut().item_spacing.x = 1.0;

                if is_active && !st.hairline_borders {
                    let resp = ui.add(
                        egui::Button::new(
                            RichText::new(s).monospace().size(font_sm()).strong().color(fg),
                        )
                        .fill(color_alpha(accent, alpha_tint()))
                        .stroke(Stroke::NONE)
                        .corner_radius(r_pill())
                        .min_size(Vec2::new(0.0, 18.0)),
                    );
                    if resp.clicked() { action = TabAction::Selected(i); }
                    if resp.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                } else {
                    let resp = ui.add(
                        egui::Button::new(
                            RichText::new(s).monospace().size(font_sm()).strong().color(fg),
                        )
                        .frame(false)
                        .min_size(Vec2::new(0.0, 18.0)),
                    );
                    if resp.clicked() { action = TabAction::Selected(i); }
                    if resp.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    if is_active && st.hairline_borders {
                        let r = resp.rect;
                        ui.painter().line_segment(
                            [
                                egui::pos2(r.left(), r.bottom() + 0.5),
                                egui::pos2(r.right(), r.bottom() + 0.5),
                            ],
                            Stroke::new(st.stroke_std, accent),
                        );
                    }
                }

                if secondary_close_btn(ui, dim) {
                    action = TabAction::Closed(i);
                }

                ui.spacing_mut().item_spacing.x = prev_inner;
            });
        }
        ui.spacing_mut().item_spacing.x = prev_x;
    });

    action
}
