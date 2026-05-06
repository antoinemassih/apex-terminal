//! Hotkey Editor UI component.

use egui;
use super::super::style::*;
use super::super::super::gpu::*;
use super::super::widgets::buttons::{SimpleBtn, ChromeBtn};
use super::super::widgets::text::{BodyLabel, SectionLabel};
use crate::ui_kit::icons::Icon;
const fn rgb(r: u8, g: u8, b: u8) -> egui::Color32 { egui::Color32::from_rgb(r, g, b) }

pub(crate) fn draw(ctx: &egui::Context, watchlist: &mut Watchlist, panes: &mut [Chart], ap: usize, t: &Theme) {
// ── Hotkey editor: key capture (runs before dialog rendering) ──────────
if let Some(edit_id) = watchlist.hotkey_editing_id {
    let input = ctx.input(|i| {
        let ctrl = i.modifiers.command;
        let shift = i.modifiers.shift;
        let alt = i.modifiers.alt;
        let keys = [
            (egui::Key::A, "A"), (egui::Key::B, "B"), (egui::Key::C, "C"), (egui::Key::D, "D"),
            (egui::Key::E, "E"), (egui::Key::F, "F"), (egui::Key::G, "G"), (egui::Key::H, "H"),
            (egui::Key::I, "I"), (egui::Key::J, "J"), (egui::Key::K, "K"), (egui::Key::L, "L"),
            (egui::Key::M, "M"), (egui::Key::N, "N"), (egui::Key::O, "O"), (egui::Key::P, "P"),
            (egui::Key::Q, "Q"), (egui::Key::R, "R"), (egui::Key::S, "S"), (egui::Key::T, "T"),
            (egui::Key::U, "U"), (egui::Key::V, "V"), (egui::Key::W, "W"), (egui::Key::X, "X"),
            (egui::Key::Y, "Y"), (egui::Key::Z, "Z"),
            (egui::Key::F1, "F1"), (egui::Key::F2, "F2"), (egui::Key::F3, "F3"), (egui::Key::F4, "F4"),
            (egui::Key::F5, "F5"), (egui::Key::F6, "F6"), (egui::Key::F7, "F7"), (egui::Key::F8, "F8"),
            (egui::Key::Delete, "Del"), (egui::Key::Backspace, "Bksp"),
        ];
        for (key, name) in keys {
            if i.key_pressed(key) {
                let mut display = String::new();
                if ctrl { display.push_str("Ctrl+"); }
                if shift { display.push_str("Shift+"); }
                if alt { display.push_str("Alt+"); }
                display.push_str(name);
                return Some((key, ctrl, shift, alt, display));
            }
        }
        if i.key_pressed(egui::Key::Escape) { return Some((egui::Key::Escape, false, false, false, String::new())); }
        None
    });
    if let Some((key, ctrl, shift, alt, display)) = input {
        if key == egui::Key::Escape {
            watchlist.hotkey_editing_id = None;
        } else {
            if let Some(hk) = watchlist.hotkeys.iter_mut().find(|h| h.id == edit_id) {
                hk.key = key; hk.ctrl = ctrl; hk.shift = shift; hk.alt = alt; hk.key_name = display;
            }
            watchlist.hotkey_editing_id = None;
        }
    }
}

// ── Hotkey editor dialog ────────────────────────────────────────────────
if watchlist.hotkey_editor_open {
    use super::super::widgets::modal::{Modal, Anchor, HeaderStyle, FrameKind};
    let screen = ctx.screen_rect();
    let resp = Modal::new("KEYBOARD SHORTCUTS")
        .id("hotkey_editor")
        .ctx(ctx)
        .theme(t)
        .size(egui::vec2(540.0, 0.0))
        .anchor(Anchor::Window { pos: Some(egui::pos2(screen.center().x - 270.0, 40.0)) })
        .header_style(HeaderStyle::Dialog)
        .frame_kind(FrameKind::DialogWindow)
        .separator(false)
        .show(|ui| {
            ui.add_space(gap_md());
            draw_content(ui, watchlist, t);
        });
    if resp.closed { watchlist.hotkey_editor_open = false; }
}


}

/// Draw the hotkey list content into `ui` (used by settings panel Shortcuts tab).
pub(crate) fn draw_content(ui: &mut egui::Ui, watchlist: &mut Watchlist, t: &Theme) {
    let mut current_category = String::new();
    let editing_id = watchlist.hotkey_editing_id;
    {
        let hotkeys_snapshot: Vec<(u32, String, String, String, bool)> = watchlist.hotkeys.iter()
            .map(|h| (h.id, h.name.clone(), h.category.clone(), h.key_name.clone(), editing_id == Some(h.id)))
            .collect();
        for (hk_id, hk_name, hk_cat, hk_key_name, is_editing) in &hotkeys_snapshot {
            if *hk_cat != current_category {
                if !current_category.is_empty() { ui.add_space(gap_md()); }
                current_category = hk_cat.clone();
                ui.add_space(gap_xs());
                ui.add(SectionLabel::new(hk_cat).tiny().size_px(9.0).color(t.dim));
                ui.add_space(gap_xs());
            }
            ui.horizontal(|ui| {
                ui.add_space(gap_lg());
                ui.add(BodyLabel::new(hk_name.as_str()).size(font_sm()).monospace(true).color(egui::Color32::from_white_alpha(180)));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if *is_editing {
                        ui.add(BodyLabel::new("Press a key...").size(font_sm()).monospace(true).color(t.accent));
                    } else {
                        if ui.add(ChromeBtn::new(egui::RichText::new("Edit").monospace().size(font_xs()).color(t.dim)).frameless(true)).clicked() {
                            watchlist.hotkey_editing_id = Some(*hk_id);
                        }
                    }
                    let key_bg = if *is_editing { color_alpha(t.accent, alpha_tint()) } else { color_alpha(t.toolbar_border, alpha_tint()) };
                    let key_fg = if *is_editing { t.accent } else { egui::Color32::from_white_alpha(140) };
                    ui.add(ChromeBtn::new(egui::RichText::new(hk_key_name.as_str()).monospace().size(font_sm()).color(key_fg))
                        .fill(key_bg).corner_radius(r_sm_cr()).min_size(egui::vec2(80.0, 18.0)));
                });
            });
        }
    }
    ui.add_space(gap_lg());
    ui.horizontal(|ui| {
        ui.add_space(gap_lg());
        if ui.add(SimpleBtn::new("Reset Defaults").color(t.dim)).clicked() {
            watchlist.hotkeys = default_hotkeys();
        }
    });
}
