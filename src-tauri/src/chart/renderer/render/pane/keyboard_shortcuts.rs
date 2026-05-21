//! Keyboard shortcut registration and handling for the chart pane.
//!
//! Extracted from `core.rs` — interaction-gated (key presses, not per-frame
//! render path), zero per-frame render cost.

#![allow(unused_imports)]

use crate::chart_renderer::gpu::*;
use crate::chart_renderer::trading::*;
use crate::chart_renderer::gpu::{
    drawing_kind_short, drawing_to_db, drawing_persist_key, shift_drawing_time, new_uuid,
};

/// Register keyboard shortcuts (once) and handle per-frame key events.
///
/// Parameters mirror every local captured from `render_chart_pane`:
/// - `ui`        — the pane's `Ui` (used for `ui.input()` checks)
/// - `chart`     — the active `Chart` (mutated by undo/redo/duplicate/delete)
/// - `watchlist` — mutable `Watchlist` (screenshot entries)
pub(super) fn handle_keyboard_shortcuts(
    ui: &mut egui::Ui,
    chart: &mut Chart,
    watchlist: &mut Watchlist,
) {
    {
        use std::sync::Once;
        static SHORTCUTS_REGISTERED: Once = Once::new();
        SHORTCUTS_REGISTERED.call_once(|| {
            use crate::foundation::shortcuts::{register, shortcut, shortcut_cmd, shortcut_cmd_shift, ShortcutEntry};
            register(ShortcutEntry {
                shortcut: shortcut_cmd(egui::Key::Z),
                action: "drawing.undo",
                description: "Undo last drawing action",
                category: "Chart",
            });
            register(ShortcutEntry {
                shortcut: shortcut_cmd_shift(egui::Key::Z),
                action: "drawing.redo",
                description: "Redo last drawing action",
                category: "Chart",
            });
            register(ShortcutEntry {
                shortcut: shortcut_cmd(egui::Key::D),
                action: "drawing.duplicate",
                description: "Duplicate selected drawing",
                category: "Chart",
            });
            register(ShortcutEntry {
                shortcut: shortcut_cmd_shift(egui::Key::S),
                action: "chart.screenshot",
                description: "Save chart screenshot",
                category: "Chart",
            });
            register(ShortcutEntry {
                shortcut: shortcut(egui::Key::M),
                action: "chart.magnet_toggle",
                description: "Toggle magnet snap mode",
                category: "Chart",
            });
            register(ShortcutEntry {
                shortcut: shortcut_cmd(egui::Key::B),
                action: "trading.buy_market",
                description: "Buy market at last price",
                category: "Trading",
            });
            register(ShortcutEntry {
                shortcut: shortcut_cmd_shift(egui::Key::B),
                action: "trading.sell_market",
                description: "Sell market at last price",
                category: "Trading",
            });
            register(ShortcutEntry {
                shortcut: shortcut_cmd_shift(egui::Key::Q),
                action: "trading.cancel_all",
                description: "Cancel all orders",
                category: "Trading",
            });
            register(ShortcutEntry {
                shortcut: shortcut_cmd_shift(egui::Key::F),
                action: "trading.flatten",
                description: "Flatten all positions",
                category: "Trading",
            });
            register(ShortcutEntry {
                shortcut: shortcut_cmd_shift(egui::Key::K),
                action: "trading.kill_switch",
                description: "Kill switch — cancel all orders and flatten all positions",
                category: "Trading",
            });
            register(ShortcutEntry {
                shortcut: shortcut_cmd_shift(egui::Key::H),
                action: "trading.halt",
                description: "Halt trading",
                category: "Trading",
            });
            register(ShortcutEntry {
                shortcut: shortcut_cmd_shift(egui::Key::R),
                action: "trading.resume",
                description: "Resume trading",
                category: "Trading",
            });
        });
    }
    if ui.input(|i| i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace)) {
        if !chart.selected_ids.is_empty() {
            for id in &chart.selected_ids {
                if let Some(d) = chart.drawings.iter().find(|d| d.id == *id) {
                    if chart.undo_stack.len() >= 50 { chart.undo_stack.remove(0); }
                    chart.undo_stack.push(DrawingAction::Remove(d.clone()));
                }
                crate::drawing_db::remove(id);
            }
            let ids = chart.selected_ids.clone();
            chart.drawings.retain(|d| !ids.contains(&d.id));
            chart.redo_stack.clear();
            chart.selected_ids.clear();
            chart.selected_id = None;
        } else if let Some(id) = chart.selected_id.take() {
            if let Some(d) = chart.drawings.iter().find(|d| d.id == id) {
                if chart.undo_stack.len() >= 50 { chart.undo_stack.remove(0); }
                chart.undo_stack.push(DrawingAction::Remove(d.clone()));
            }
            crate::drawing_db::remove(&id);
            chart.drawings.retain(|d| d.id != id);
            chart.redo_stack.clear();
        }
    }
    // Ctrl+Z: Undo
    if ui.input(|i| i.modifiers.command && i.key_pressed(egui::Key::Z) && !i.modifiers.shift) {
        if let Some(action) = chart.undo_stack.pop() {
            let toast_desc = match &action {
                DrawingAction::Add(d) => format!("Undone: Added {}", drawing_kind_short(&d.kind)),
                DrawingAction::Remove(d) => format!("Undone: Removed {}", drawing_kind_short(&d.kind)),
                DrawingAction::Modify(_, d) => format!("Undone: Modified {}", drawing_kind_short(&d.kind)),
            };
            let redo_action = match &action {
                DrawingAction::Add(d) => {
                    chart.drawings.retain(|x| x.id != d.id);
                    crate::drawing_db::remove(&d.id);
                    DrawingAction::Remove(d.clone())
                }
                DrawingAction::Remove(d) => {
                    crate::drawing_db::save(&drawing_to_db(d, &drawing_persist_key(chart), &chart.timeframe));
                    chart.drawings.push(d.clone());
                    DrawingAction::Add(d.clone())
                }
                DrawingAction::Modify(id, old) => {
                    let current = chart.drawings.iter().find(|d| d.id == *id).cloned();
                    let pkey = drawing_persist_key(chart);
                    let tf = chart.timeframe.clone();
                    if let Some(d) = chart.drawings.iter_mut().find(|d| d.id == *id) {
                        *d = old.clone();
                        crate::drawing_db::save(&drawing_to_db(d, &pkey, &tf));
                    }
                    DrawingAction::Modify(id.clone(), current.unwrap_or_else(|| old.clone()))
                }
            };
            if chart.redo_stack.len() >= 50 { chart.redo_stack.remove(0); }
            chart.redo_stack.push(redo_action);
            PENDING_TOASTS.with(|ts| ts.borrow_mut().push((toast_desc, 0.0, true)));
        }
    }
    // Ctrl+Shift+Z or Ctrl+Y: Redo
    if ui.input(|i| (i.modifiers.command && i.modifiers.shift && i.key_pressed(egui::Key::Z)) || (i.modifiers.command && i.key_pressed(egui::Key::Y))) {
        if let Some(action) = chart.redo_stack.pop() {
            let toast_desc = match &action {
                DrawingAction::Add(d) => format!("Redone: Added {}", drawing_kind_short(&d.kind)),
                DrawingAction::Remove(d) => format!("Redone: Removed {}", drawing_kind_short(&d.kind)),
                DrawingAction::Modify(_, d) => format!("Redone: Modified {}", drawing_kind_short(&d.kind)),
            };
            let undo_action = match &action {
                DrawingAction::Add(d) => {
                    crate::drawing_db::save(&drawing_to_db(d, &drawing_persist_key(chart), &chart.timeframe));
                    chart.drawings.push(d.clone());
                    DrawingAction::Remove(d.clone())
                }
                DrawingAction::Remove(d) => {
                    chart.drawings.retain(|x| x.id != d.id);
                    crate::drawing_db::remove(&d.id);
                    DrawingAction::Add(d.clone())
                }
                DrawingAction::Modify(id, restored) => {
                    let current = chart.drawings.iter().find(|d| d.id == *id).cloned();
                    let pkey = drawing_persist_key(chart);
                    let tf = chart.timeframe.clone();
                    if let Some(d) = chart.drawings.iter_mut().find(|d| d.id == *id) {
                        *d = restored.clone();
                        crate::drawing_db::save(&drawing_to_db(d, &pkey, &tf));
                    }
                    DrawingAction::Modify(id.clone(), current.unwrap_or_else(|| restored.clone()))
                }
            };
            if chart.undo_stack.len() >= 50 { chart.undo_stack.remove(0); }
            chart.undo_stack.push(undo_action);
            PENDING_TOASTS.with(|ts| ts.borrow_mut().push((toast_desc, 0.0, true)));
        }
    }
    // Ctrl+D: Duplicate selected drawing
    if ui.input(|i| i.modifiers.command && i.key_pressed(egui::Key::D)) {
        if let Some(ref sel_id) = chart.selected_id.clone() {
            if let Some(src) = chart.drawings.iter().find(|d| d.id == *sel_id).cloned() {
                let mut dup = src;
                dup.id = new_uuid();
                let bar_shift = if chart.timestamps.len() > 1 { (chart.timestamps[1] - chart.timestamps[0]) * 5 } else { 1500 };
                shift_drawing_time(&mut dup.kind, bar_shift);
                crate::drawing_db::save(&drawing_to_db(&dup, &drawing_persist_key(chart), &chart.timeframe));
                if chart.undo_stack.len() >= 50 { chart.undo_stack.remove(0); }
                chart.undo_stack.push(DrawingAction::Add(dup.clone()));
                chart.redo_stack.clear();
                chart.selected_id = Some(dup.id.clone());
                chart.selected_ids = vec![dup.id.clone()];
                chart.drawings.push(dup);
            }
        }
    }

    // TPS Reports boss-key — configurable, action "tps_toggle".
    // Looks up the hotkey from watchlist.hotkeys so user reconfiguration is respected.
    // Fires even when the overlay is active so it can be dismissed.
    if let Some(hk) = watchlist.hotkeys.iter().find(|h| h.action == "tps_toggle") {
        let key      = hk.key;
        let ctrl     = hk.ctrl;
        let shift    = hk.shift;
        if ui.input(|i| {
            i.key_pressed(key)
                && i.modifiers.command == ctrl
                && i.modifiers.shift   == shift
                && !i.modifiers.alt
        }) {
            watchlist.boss_key_active = !watchlist.boss_key_active;
        }
    }

    // Ctrl+Shift+S: Screenshot — save metadata + open Windows Snip tool
    if ui.input(|i| i.modifiers.command && i.modifiers.shift && i.key_pressed(egui::Key::S)) {
        // Save screenshot metadata to library
        let ss_entry = crate::chart_renderer::ui::panels::screenshot_panel::save_screenshot(&chart.symbol, &chart.timeframe, chart.vs, chart.vc);
        watchlist.screenshot_entries.insert(0, ss_entry);
        watchlist.screenshot_entries.truncate(200);
        PENDING_TOASTS.with(|ts| ts.borrow_mut().push(("Screenshot saved".into(), 0.0, true)));
        #[cfg(target_os = "windows")]
        {
            let _ = std::process::Command::new("cmd")
                .args(["/C", "start", "ms-screenclip:"])
                .creation_flags(0x08000000)
                .spawn();
        }
    }
}
