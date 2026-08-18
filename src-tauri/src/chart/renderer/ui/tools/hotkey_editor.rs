//! Hotkey Editor UI component.

use egui;
use crate::ui_kit::sx::Tone;
use super::super::style::*;
use super::super::super::gpu::*;
use super::super::components::text::{BodyLabel, SectionLabel};
use crate::ui_kit::widgets::Button;
use crate::ui_kit::widgets::tokens::{Variant, Size};

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
// Always call show() (no `if open` gate) and drive it with `.open(flag)` so the
// overlay plays its fade-out; ToolOverlay stops rendering once fully hidden.
{
    let screen = ctx.screen_rect();
    // Migrated to ToolOverlay (2026-05-26) — shared header chrome.
    let portable_t = crate::chart_renderer::theme_impl::theme_to_portable(t);
    let resp = crate::ui_kit::widgets::ToolOverlay::new("KEYBOARD SHORTCUTS")
        .id("hotkey_editor")
        .width(540.0)
        .pos(egui::pos2(screen.center().x - 270.0, 40.0))
        .open(watchlist.hotkey_editor_open)
        .show(ctx, &portable_t, |ui| {
            draw_content(ui, watchlist, t);
        });
    if resp.closed { watchlist.update_sidebar_state(|s| s.hotkey_editor_open = false); }
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
                ui.add(BodyLabel::new(hk_name.as_str()).size(font_sm()).monospace(true).color(tint(t, Tone::Text, alpha_strong())));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Two controls, ONE state change. Both the "Edit" label and
                    // the key pill below start a rebind, so the assignment lives
                    // in one place rather than being written at each control —
                    // which also keeps this to a single direct-mutation site
                    // instead of adding a second bypass of the command bus.
                    let mut start_edit = false;
                    if *is_editing {
                        ui.add(BodyLabel::new("Press a key...").size(font_sm()).monospace(true).color(t.accent));
                    } else if Button::new("Edit").variant(Variant::TextOnly).size(Size::Xs).fg(t.dim).show(ui, t).clicked() {
                        start_edit = true;
                    }
                    let key_bg = if *is_editing { tint(t, Tone::Accent, alpha_tint()) } else { tint(t, Tone::Border, alpha_tint()) };
                    let key_fg = if *is_editing { t.accent } else { tint(t, Tone::Text, alpha_muted()) };
                    // The key pill STARTS A REBIND. It used to drop its
                    // `Response` on the floor: a `Chrome`-variant `Button` that
                    // hover-animates like every other control here, sitting in a
                    // rebinding workflow, doing nothing when clicked. The only
                    // working target was the small "Edit" text two lines above.
                    //
                    // Wired rather than de-affordanced. The alternative fix was
                    // `.sense(Sense::hover())` to stop it looking pressable, but
                    // the key you want to change is the obvious thing to click,
                    // the action already exists directly above, and there is
                    // only one thing a click here could reasonably mean. This is
                    // not the `trade_plan_panel` case, where wiring would have
                    // meant inventing which of several surfaces should open it.
                    if Button::new(hk_key_name.as_str()).variant(Variant::Chrome).size(Size::Sm).fg(key_fg)
                        .fill(key_bg).corner_radius(crate::ui_kit::style::radius_sm())
                        .min_size(egui::vec2(80.0, row_height_dense())).show(ui, t).clicked()
                    {
                        start_edit = true;
                    }
                    if start_edit {
                        watchlist.hotkey_editing_id = Some(*hk_id);
                    }
                });
            });
        }
    }
    ui.add_space(gap_lg());
    ui.horizontal(|ui| {
        ui.add_space(gap_lg());
        if Button::new("Reset Defaults").variant(Variant::Secondary).simple_treatment(true).fg(t.dim).show(ui, t).clicked() {
            watchlist.hotkeys = default_hotkeys();
        }
    });
}
