//! Header glyph buttons + tab bar with close affordance.

use super::super::style::*;
use egui::{self, Color32, Response, RichText, Stroke, Ui, Vec2};
use crate::ui_kit::widgets::Button;
use crate::ui_kit::widgets::tokens::{Variant as KitVariant, Size as KitSize};

// ─── Header action button ─────────────────────────────────────────────────────

/// Tiny transparent ghost glyph button for panel headers (+, ×, ⚙).
/// Frameless, dim color, hover changes cursor. Used in compact header rows
/// where a full `icon_btn` is too prominent.
pub fn header_action_btn(ui: &mut Ui, glyph: &str, dim: Color32) -> Response {
    let theme = crate::ui_kit::widgets::theme::active_theme(ui.ctx());
    Button::icon(glyph).variant(KitVariant::Ghost).size(KitSize::Xs)
        .glyph_color(dim).min_size(Vec2::new(14.0, 14.0))
        .show(ui, theme)
}

/// Smaller, dimmer variant of `style::close_button` for secondary close
/// affordances inside split sections / nested headers.
pub fn secondary_close_btn(ui: &mut Ui, _dim: Color32) -> bool {
    Button::close()
        .show(ui, &crate::ui_kit::widgets::theme::active_theme(ui.ctx()))
        .on_hover_text("Close")
        .clicked()
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

                let theme = crate::ui_kit::widgets::theme::active_theme(ui.ctx());
                if is_active && !st.hairline_borders {
                    let resp = Button::toggle(s.as_str(), true).size(KitSize::Sm)
                        .min_size(Vec2::new(0.0, 18.0)).show(ui, theme);
                    if resp.clicked() { action = TabAction::Selected(i); }
                } else {
                    let resp = Button::new(s.as_str()).variant(KitVariant::Ghost).size(KitSize::Sm)
                        .fg(fg).frameless(!is_active).min_size(Vec2::new(0.0, 18.0))
                        .show(ui, theme);
                    if resp.clicked() { action = TabAction::Selected(i); }
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
