//! Hotkey Editor UI component.

use egui;
use super::style::*;
use super::super::gpu::*;
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
    let screen = ctx.screen_rect();
    dialog_window_themed(ctx, "hotkey_editor", egui::pos2(screen.center().x - 280.0, 40.0), 560.0, t.toolbar_bg, t.toolbar_border, None)
        .show(ctx, |ui| {
            if dialog_header(ui, "KEYBOARD SHORTCUTS", t.dim) { watchlist.hotkey_editor_open = false; }
            ui.add_space(8.0);
            let mut current_category = String::new();
            let editing_id = watchlist.hotkey_editing_id;
            {
                let hotkeys_snapshot: Vec<(u32, String, String, String, bool)> = watchlist.hotkeys.iter()
                    .map(|h| (h.id, h.name.clone(), h.category.clone(), h.key_name.clone(), editing_id == Some(h.id)))
                    .collect();
                for (hk_id, hk_name, hk_cat, hk_key_name, is_editing) in &hotkeys_snapshot {
                    if *hk_cat != current_category {
                        if !current_category.is_empty() { ui.add_space(6.0); }
                        current_category = hk_cat.clone();
                        ui.add_space(2.0);
                        ui.label(egui::RichText::new(hk_cat.to_uppercase()).monospace().size(9.0).color(t.dim));
                        ui.add_space(2.0);
                    }
                    ui.horizontal(|ui| {
                        ui.add_space(10.0);
                        ui.label(egui::RichText::new(hk_name.as_str()).monospace().size(10.0).color(egui::Color32::from_white_alpha(180)));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if *is_editing {
                                ui.label(egui::RichText::new("Press a key...").monospace().size(9.0).color(t.accent));
                            } else {
                                if ui.add(egui::Button::new(egui::RichText::new("Edit").monospace().size(8.0).color(t.dim)).frame(false)).clicked() {
                                    watchlist.hotkey_editing_id = Some(*hk_id);
                                }
                            }
                            let key_bg = if *is_editing { color_alpha(t.accent, ALPHA_TINT) } else { color_alpha(t.toolbar_border, ALPHA_TINT) };
                            let key_fg = if *is_editing { t.accent } else { egui::Color32::from_white_alpha(140) };
                            ui.add(egui::Button::new(egui::RichText::new(hk_key_name.as_str()).monospace().size(10.0).color(key_fg))
                                .fill(key_bg).corner_radius(3.0).min_size(egui::vec2(80.0, 18.0)));
                        });
                    });
                }
            }
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                ui.add_space(10.0);
                if ui.button(egui::RichText::new("Reset Defaults").monospace().size(10.0).color(t.dim)).clicked() {
                    watchlist.hotkeys = default_hotkeys();
                }
            });
        });
}


}
