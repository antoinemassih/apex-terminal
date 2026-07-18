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

/// W1-06 (audit): does the current input match the user-configured binding for
/// `action`? `watchlist.hotkeys` is the single source the Settings hotkey editor
/// writes to — but ONLY the tps_toggle handler read it, so remapping any of the
/// other ~26 bindings did nothing (the handlers hard-coded their keys). This is
/// the shared dispatch check that fixes that. Exact modifier match (command ==
/// ctrl, shift, alt), mirroring the original tps_toggle handler. `default_hotkeys()`
/// seeds every action with the SAME key the handler used to hard-code, so an
/// unmodified config reproduces the old behavior exactly.
fn binding_pressed(hotkeys: &[HotKey], ui: &egui::Ui, action: &str) -> bool {
    match hotkeys.iter().find(|h| h.action == action) {
        Some(hk) => ui.input(|i| {
            i.key_pressed(hk.key)
                && i.modifiers.command == hk.ctrl
                && i.modifiers.shift == hk.shift
                && i.modifiers.alt == hk.alt
        }),
        None => false,
    }
}

/// Register keyboard shortcuts (once) and handle per-frame key events.
///
/// Parameters mirror every local captured from `render_chart_pane`:
/// - `ui`        — the pane's `Ui` (used for `ui.input()` checks)
/// - `ctx`       — the egui `Context` (used for `ctx.wants_keyboard_input()`)
/// - `chart`     — the active `Chart` (mutated by undo/redo/duplicate/delete)
/// - `watchlist` — mutable `Watchlist` (screenshot entries)
pub(super) fn handle_keyboard_shortcuts(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
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
            register(ShortcutEntry {
                shortcut: shortcut(egui::Key::F1),
                action: "help.shortcuts",
                description: "Show keyboard shortcuts",
                category: "General",
            });
        });
    }

    // F1 — toggle the keyboard-shortcut cheatsheet (WS-G G4). The caller is
    // active-pane-gated (runs once/frame), so a plain toggle is race-free.
    // No modifiers so Ctrl/Alt+F1 don't trip it.
    if ui.input(|i| i.key_pressed(egui::Key::F1) && i.modifiers.is_none()) {
        crate::chart_renderer::ui::tools::shortcuts_help::toggle(ctx);
    }
    // W1-06: configurable "delete"; Backspace kept as an always-on alias.
    if binding_pressed(&watchlist.hotkeys, ui, "delete")
        || ui.input(|i| i.key_pressed(egui::Key::Backspace))
    {
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
    // Ctrl+Z: Undo (W1-06: configurable)
    if binding_pressed(&watchlist.hotkeys, ui, "undo") {
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
            crate::chart_renderer::ui::tools::notification::push_pending(
                crate::chart_renderer::ui::tools::notification::Notification::new(
                    toast_desc,
                    crate::chart_renderer::ui::tools::notification::NotificationSeverity::Info,
                ).with_source("undo")
            );
        }
    }
    // Redo (W1-06: configurable, default Ctrl+Y; Ctrl+Shift+Z kept as an alias)
    if binding_pressed(&watchlist.hotkeys, ui, "redo")
        || ui.input(|i| i.modifiers.command && i.modifiers.shift && i.key_pressed(egui::Key::Z))
    {
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
            crate::chart_renderer::ui::tools::notification::push_pending(
                crate::chart_renderer::ui::tools::notification::Notification::new(
                    toast_desc,
                    crate::chart_renderer::ui::tools::notification::NotificationSeverity::Info,
                ).with_source("redo")
            );
        }
    }
    // Ctrl+D: Duplicate selected drawing (W1-06: configurable)
    if binding_pressed(&watchlist.hotkeys, ui, "duplicate") {
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

    // Screenshot — save metadata + open Windows Snip tool (W1-06: configurable)
    if binding_pressed(&watchlist.hotkeys, ui, "screenshot") {
        // Save screenshot metadata to library
        let ss_entry = crate::chart_renderer::ui::panels::screenshot_panel::save_screenshot(&chart.symbol, &chart.timeframe, chart.vs, chart.vc);
        watchlist.screenshot_entries.insert(0, ss_entry);
        watchlist.screenshot_entries.truncate(200);
        crate::chart_renderer::ui::tools::notification::push_pending(
            crate::chart_renderer::ui::tools::notification::Notification::new(
                "Screenshot saved",
                crate::chart_renderer::ui::tools::notification::NotificationSeverity::Success,
            ).with_source("screenshot")
        );
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            let _ = std::process::Command::new("cmd")
                .args(["/C", "start", "ms-screenclip:"])
                .creation_flags(0x08000000)
                .spawn();
        }
    }

    // ── Escape: clear draw tool / selection / text-edit (W1-06: configurable) ─
    if binding_pressed(&watchlist.hotkeys, ui, "escape") {
        chart.draw_tool.clear(); chart.pending_pt = None; chart.pending_pt2 = None; chart.pending_pts.clear();
        chart.selected_id = None; chart.editing_indicator = None; chart.editing_order = None;
        if let Some(ref edit_id) = chart.text_edit_id.clone() {
            chart.drawings.retain(|d| d.id != *edit_id);
            crate::drawing_db::remove(edit_id);
            chart.text_edit_id = None; chart.text_edit_buf.clear();
        }
    }

    // ── Toggle magnet mode (W1-06: configurable, default M) ───────────────
    if !ctx.wants_keyboard_input() && binding_pressed(&watchlist.hotkeys, ui, "toggle_magnet") {
        chart.magnet = !chart.magnet;
    }

    // ── Replay mode keyboard controls ─────────────────────────────────────
    if chart.replay_mode && !ctx.wants_keyboard_input() {
        // Space: toggle play/pause
        if ui.input(|i| i.key_pressed(egui::Key::Space)) {
            chart.replay_playing = !chart.replay_playing;
            if chart.replay_playing { chart.replay_last_step = None; }
        }
        // Right arrow: step forward 1 bar (only when paused)
        if !chart.replay_playing && ui.input(|i| i.key_pressed(egui::Key::ArrowRight)) {
            chart.replay_bar_count = (chart.replay_bar_count + 1).min(chart.bars.len());
            chart.indicator_bar_count = 0;
            chart.vs = (chart.replay_bar_count as f32 - chart.vc as f32 + CHART_RIGHT_PAD as f32).max(0.0);
        }
        // Left arrow: step back 1 bar (only when paused)
        if !chart.replay_playing && ui.input(|i| i.key_pressed(egui::Key::ArrowLeft)) {
            chart.replay_bar_count = chart.replay_bar_count.saturating_sub(1).max(1);
            chart.indicator_bar_count = 0;
            chart.vs = (chart.replay_bar_count as f32 - chart.vc as f32 + CHART_RIGHT_PAD as f32).max(0.0);
        }
        // Home: jump to start
        if ui.input(|i| i.key_pressed(egui::Key::Home)) {
            chart.replay_bar_count = 1;
            chart.replay_playing = false;
            chart.indicator_bar_count = 0;
            chart.vs = 0.0;
        }
        // End: jump to end (exit replay)
        if ui.input(|i| i.key_pressed(egui::Key::End)) {
            chart.replay_bar_count = chart.bars.len();
            chart.replay_playing = false;
            chart.indicator_bar_count = 0;
            chart.vs = (chart.replay_bar_count as f32 - chart.vc as f32 + CHART_RIGHT_PAD as f32).max(0.0);
        }
    }

    // ── Keyboard shortcuts for drawing tools ──────────────────────────────
    // Single-key activates tools instantly (only when no tool active and no
    // text input).
    if !ctx.wants_keyboard_input() && chart.draw_tool.is_empty() {
        // W1-06: each tool key is configurable (default_hotkeys seeds the same
        // single-key defaults T/H/F/C/V/R/P/G/X/N). First match wins. binding_pressed
        // does an exact-modifier match, so Ctrl+C / Ctrl+V no longer trip the
        // channel/vline tools (was a special !command guard). Z stays drag-zoom.
        let hk = &watchlist.hotkeys;
        let new_tool: Option<&str> =
            if binding_pressed(hk, ui, "tool_trendline") { Some("trendline") }
            else if binding_pressed(hk, ui, "tool_hline") { Some("hline") }
            else if binding_pressed(hk, ui, "tool_fibonacci") { Some("fibonacci") }
            else if binding_pressed(hk, ui, "tool_channel") { Some("channel") }
            else if binding_pressed(hk, ui, "tool_vline") { Some("vline") }
            else if binding_pressed(hk, ui, "tool_ray") { Some("ray") }
            else if binding_pressed(hk, ui, "tool_pitchfork") { Some("pitchfork") }
            else if binding_pressed(hk, ui, "tool_gannfan") { Some("gannfan") }
            else if binding_pressed(hk, ui, "tool_fibext") { Some("fibext") }
            else if binding_pressed(hk, ui, "tool_textnote") { Some("textnote") }
            else { None };
        if let Some(tool) = new_tool {
            chart.draw_tool = tool.into();
            chart.pending_pt = None; chart.pending_pt2 = None; chart.pending_pts.clear();
        }
    }

    // ── Trading hotkeys ───────────────────────────────────────────────────
    if !ctx.wants_keyboard_input() {
        use crate::chart_renderer::trading::order_manager::*;
        // Buy market at last price (W1-06: configurable, default Ctrl+B)
        if binding_pressed(&watchlist.hotkeys, ui, "buy_market") {
            // Pass the real last bar close as last_price so fat-finger and
            // buying-power checks engage (previously hardcoded to 0.0).
            let last_price = chart.bars.last().map(|b| b.close).unwrap_or(0.0);
            let result = submit_order(OrderIntent {
                symbol: chart.symbol.clone(), side: OrderSide::Buy,
                order_type: ManagedOrderType::Market, price: last_price, qty: chart.order_panel.qty,
                source: OrderSource::Hotkey, pair_with: None, option_symbol: None, option_con_id: None, stop_price: 0.0, trail_amount: None, trail_percent: None, last_price, tif: 0, outside_rth: false,
                strategy_id: None, override_warnings: false,
            });
            if let OrderResult::Accepted(id) = result {
                chart.orders.push(OrderLevel { id: id as u32, side: OrderSide::Buy, price: last_price, qty: chart.order_panel.qty, status: OrderStatus::Placed, state: OrderState::Working, pair_id: None, option_symbol: None, option_con_id: None, trail_amount: None, trail_percent: None, filled_ratio: 0.0 });
            }
        }
        // Sell market at last price (W1-06: configurable, default Ctrl+Shift+B)
        if binding_pressed(&watchlist.hotkeys, ui, "sell_market") {
            let last_price = chart.bars.last().map(|b| b.close).unwrap_or(0.0);
            let result = submit_order(OrderIntent {
                symbol: chart.symbol.clone(), side: OrderSide::Sell,
                order_type: ManagedOrderType::Market, price: last_price, qty: chart.order_panel.qty,
                source: OrderSource::Hotkey, pair_with: None, option_symbol: None, option_con_id: None, stop_price: 0.0, trail_amount: None, trail_percent: None, last_price, tif: 0, outside_rth: false,
                strategy_id: None, override_warnings: false,
            });
            if let OrderResult::Accepted(id) = result {
                chart.orders.push(OrderLevel { id: id as u32, side: OrderSide::Sell, price: last_price, qty: chart.order_panel.qty, status: OrderStatus::Placed, state: OrderState::Working, pair_id: None, option_symbol: None, option_con_id: None, trail_amount: None, trail_percent: None, filled_ratio: 0.0 });
            }
        }
        // Ctrl+Shift+Q: Cancel all orders — routes through the broker abstraction
        // via cancel_all_orders().  The raw reqwest DELETE that used to fire here
        // in parallel was a double-cancel race: cancel_all_orders already sends a
        // bulk DELETE through the Broker trait.  Removed (T3).
        // Cancel all orders (W1-06: configurable, default Ctrl+Shift+Q)
        if binding_pressed(&watchlist.hotkeys, ui, "cancel_all") {
            crate::chart_renderer::trading::order_manager::cancel_all_orders("");
            chart.orders.clear();
            watchlist.selected_order_ids.clear();
        }
        // Ctrl+Shift+F: Flatten all positions — cancel all orders, then route the
        // flatten POST through the broker abstraction (live mode only).
        // The raw reqwest thread is removed (T3); the broker cancel_all path
        // already talks to the real endpoint.
        // Flatten all positions (W1-06: configurable, default Ctrl+Shift+F)
        if binding_pressed(&watchlist.hotkeys, ui, "flatten") {
            crate::chart_renderer::trading::order_manager::cancel_all_orders("");
            chart.orders.retain(|o| o.status == OrderStatus::Executed);
            watchlist.selected_order_ids.clear();
            // WS-H #41a: guarded account-wide flatten via OrderManager (paper
            // guard, risk gate, kill/halt, journal). Replaces the raw broker HTTP
            // — which even with the is_paper_mode() guard bypassed the journal and
            // the kill/halt interlock.
            crate::chart_renderer::trading::order_manager::flatten_all();
        }
        // Ctrl+Shift+K: Kill Switch — cancel all orders, flatten positions, and
        // halt new trading. Single handler: Halt Trading was relocated off ⌘⇧H
        // (now the TPS boss key) onto ⌘⇧K, which already bound Kill Switch —
        // the two are merged so the chord fires exactly one combined action.
        // Kill switch — cancel all + flatten + halt (W1-06: configurable, default
        // Ctrl+Shift+K; kill_switch and halt share this chord in default_hotkeys).
        if binding_pressed(&watchlist.hotkeys, ui, "kill_switch") {
            crate::chart_renderer::trading::order_manager::kill_switch();
            let _ = crate::chart_renderer::trading::order_manager::halt_trading();
            chart.orders.clear();
            watchlist.selected_order_ids.clear();
            crate::chart_renderer::ui::tools::notification::push_pending(
                crate::chart_renderer::ui::tools::notification::Notification::new(
                    "KILL SWITCH — orders cancelled, trading halted",
                    crate::chart_renderer::ui::tools::notification::NotificationSeverity::Error,
                ).with_source("trading")
            );
        }
        // Resume trading (W1-06: configurable, default Ctrl+Shift+R)
        if binding_pressed(&watchlist.hotkeys, ui, "resume_trading") {
            let _ = crate::chart_renderer::trading::order_manager::resume_trading();
            crate::chart_renderer::ui::tools::notification::push_pending(
                crate::chart_renderer::ui::tools::notification::Notification::new(
                    "Trading RESUMED",
                    crate::chart_renderer::ui::tools::notification::NotificationSeverity::Success,
                ).with_source("trading")
            );
        }
    }
}

#[cfg(test)]
mod w1_06_binding_tests {
    use crate::chart_renderer::gpu::default_hotkeys;
    use egui::Key;

    /// W1-06: the invariant that makes this fix safe — every action the handlers
    /// now dispatch through `binding_pressed` must have a default binding whose
    /// key + modifiers EXACTLY match the key the handler used to hard-code. If a
    /// default drifts, an unconfigured user silently loses the shortcut; this
    /// test catches that.
    #[test]
    fn defaults_reproduce_the_old_hardcoded_bindings() {
        // (action, key, ctrl, shift, alt) — mirrors the pre-W1-06 hardcoded sites.
        let expected: &[(&str, Key, bool, bool, bool)] = &[
            ("buy_market",     Key::B, true,  false, false),
            ("sell_market",    Key::B, true,  true,  false),
            ("cancel_all",     Key::Q, true,  true,  false),
            ("flatten",        Key::F, true,  true,  false),
            ("kill_switch",    Key::K, true,  true,  false),
            ("resume_trading", Key::R, true,  true,  false),
            ("undo",           Key::Z, true,  false, false),
            ("redo",           Key::Y, true,  false, false),
            ("duplicate",      Key::D, true,  false, false),
            ("screenshot",     Key::S, true,  true,  false),
            ("delete",         Key::Delete, false, false, false),
            ("escape",         Key::Escape, false, false, false),
            ("toggle_magnet",  Key::M, false, false, false),
            ("tool_trendline", Key::T, false, false, false),
            ("tool_hline",     Key::H, false, false, false),
            ("tool_fibonacci", Key::F, false, false, false),
            ("tool_channel",   Key::C, false, false, false),
            ("tool_vline",     Key::V, false, false, false),
            ("tool_ray",       Key::R, false, false, false),
            ("tool_pitchfork", Key::P, false, false, false),
            ("tool_gannfan",   Key::G, false, false, false),
            ("tool_fibext",    Key::X, false, false, false),
            ("tool_textnote",  Key::N, false, false, false),
        ];
        let hk = default_hotkeys();
        for (action, key, ctrl, shift, alt) in expected {
            let found = hk.iter().find(|h| h.action == *action)
                .unwrap_or_else(|| panic!("default_hotkeys missing action '{action}'"));
            assert_eq!(found.key, *key, "action '{action}' key drifted");
            assert_eq!(found.ctrl, *ctrl, "action '{action}' ctrl drifted");
            assert_eq!(found.shift, *shift, "action '{action}' shift drifted");
            assert_eq!(found.alt, *alt, "action '{action}' alt drifted");
        }
    }
}
