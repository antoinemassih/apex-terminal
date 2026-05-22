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
            PENDING_TOASTS.with(|ts| ts.borrow_mut().push(
                crate::chart_renderer::ui::tools::notification::Notification::new(
                    toast_desc,
                    crate::chart_renderer::ui::tools::notification::NotificationSeverity::Info,
                ).with_source("undo")
            ));
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
            PENDING_TOASTS.with(|ts| ts.borrow_mut().push(
                crate::chart_renderer::ui::tools::notification::Notification::new(
                    toast_desc,
                    crate::chart_renderer::ui::tools::notification::NotificationSeverity::Info,
                ).with_source("redo")
            ));
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
        PENDING_TOASTS.with(|ts| ts.borrow_mut().push(
            crate::chart_renderer::ui::tools::notification::Notification::new(
                "Screenshot saved",
                crate::chart_renderer::ui::tools::notification::NotificationSeverity::Success,
            ).with_source("screenshot")
        ));
        #[cfg(target_os = "windows")]
        {
            let _ = std::process::Command::new("cmd")
                .args(["/C", "start", "ms-screenclip:"])
                .creation_flags(0x08000000)
                .spawn();
        }
    }

    // ── Escape: clear draw tool / selection / text-edit ───────────────────
    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        chart.draw_tool.clear(); chart.pending_pt = None; chart.pending_pt2 = None; chart.pending_pts.clear();
        chart.selected_id = None; chart.editing_indicator = None; chart.editing_order = None;
        if let Some(ref edit_id) = chart.text_edit_id.clone() {
            chart.drawings.retain(|d| d.id != *edit_id);
            crate::drawing_db::remove(edit_id);
            chart.text_edit_id = None; chart.text_edit_buf.clear();
        }
    }

    // ── M key: toggle magnet mode ─────────────────────────────────────────
    if ui.input(|i| i.key_pressed(egui::Key::M)) && !ctx.wants_keyboard_input() {
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
        let new_tool: Option<&str> = ui.input(|i| {
            if i.key_pressed(egui::Key::T) { Some("trendline") }
            else if i.key_pressed(egui::Key::H) { Some("hline") }
            else if i.key_pressed(egui::Key::F) { Some("fibonacci") }
            else if i.key_pressed(egui::Key::C) && !i.modifiers.command { Some("channel") }
            else if i.key_pressed(egui::Key::V) && !i.modifiers.command { Some("vline") }
            else if i.key_pressed(egui::Key::R) { Some("ray") }
            // Z is now drag-zoom (handled separately), not hzone
            else if i.key_pressed(egui::Key::P) { Some("pitchfork") }
            else if i.key_pressed(egui::Key::G) { Some("gannfan") }
            else if i.key_pressed(egui::Key::X) { Some("fibext") }
            else if i.key_pressed(egui::Key::N) { Some("textnote") }
            else { None }
        });
        if let Some(tool) = new_tool {
            chart.draw_tool = tool.into();
            chart.pending_pt = None; chart.pending_pt2 = None; chart.pending_pts.clear();
        }
    }

    // ── Trading hotkeys ───────────────────────────────────────────────────
    if !ctx.wants_keyboard_input() {
        use crate::chart_renderer::trading::order_manager::*;
        // Ctrl+B: Buy market at last price
        if ui.input(|i| i.modifiers.command && i.key_pressed(egui::Key::B) && !i.modifiers.shift) {
            // Pass the real last bar close as last_price so fat-finger and
            // buying-power checks engage (previously hardcoded to 0.0).
            let last_price = chart.bars.last().map(|b| b.close).unwrap_or(0.0);
            let result = submit_order(OrderIntent {
                symbol: chart.symbol.clone(), side: OrderSide::Buy,
                order_type: ManagedOrderType::Market, price: last_price, qty: chart.order_qty,
                source: OrderSource::Hotkey, pair_with: None, option_symbol: None, option_con_id: None, stop_price: 0.0, trail_amount: None, trail_percent: None, last_price, tif: 0, outside_rth: false,
                strategy_id: None, override_warnings: false,
            });
            if let OrderResult::Accepted(id) = result {
                chart.orders.push(OrderLevel { id: id as u32, side: OrderSide::Buy, price: last_price, qty: chart.order_qty, status: OrderStatus::Placed, state: OrderState::Working, pair_id: None, option_symbol: None, option_con_id: None, trail_amount: None, trail_percent: None, filled_ratio: 0.0 });
            }
        }
        // Ctrl+Shift+B: Sell market at last price
        if ui.input(|i| i.modifiers.command && i.modifiers.shift && i.key_pressed(egui::Key::B)) {
            let last_price = chart.bars.last().map(|b| b.close).unwrap_or(0.0);
            let result = submit_order(OrderIntent {
                symbol: chart.symbol.clone(), side: OrderSide::Sell,
                order_type: ManagedOrderType::Market, price: last_price, qty: chart.order_qty,
                source: OrderSource::Hotkey, pair_with: None, option_symbol: None, option_con_id: None, stop_price: 0.0, trail_amount: None, trail_percent: None, last_price, tif: 0, outside_rth: false,
                strategy_id: None, override_warnings: false,
            });
            if let OrderResult::Accepted(id) = result {
                chart.orders.push(OrderLevel { id: id as u32, side: OrderSide::Sell, price: last_price, qty: chart.order_qty, status: OrderStatus::Placed, state: OrderState::Working, pair_id: None, option_symbol: None, option_con_id: None, trail_amount: None, trail_percent: None, filled_ratio: 0.0 });
            }
        }
        // Ctrl+Shift+Q: Cancel all orders — routes through the broker abstraction
        // via cancel_all_orders().  The raw reqwest DELETE that used to fire here
        // in parallel was a double-cancel race: cancel_all_orders already sends a
        // bulk DELETE through the Broker trait.  Removed (T3).
        if ui.input(|i| i.modifiers.command && i.modifiers.shift && i.key_pressed(egui::Key::Q)) {
            crate::chart_renderer::trading::order_manager::cancel_all_orders("");
            chart.orders.clear();
        }
        // Ctrl+Shift+F: Flatten all positions — cancel all orders, then route the
        // flatten POST through the broker abstraction (live mode only).
        // The raw reqwest thread is removed (T3); the broker cancel_all path
        // already talks to the real endpoint.
        if ui.input(|i| i.modifiers.command && i.modifiers.shift && i.key_pressed(egui::Key::F)) {
            crate::chart_renderer::trading::order_manager::cancel_all_orders("");
            chart.orders.retain(|o| o.status == OrderStatus::Executed);
            // Flatten POST: only fire against a live broker; paper mode has no
            // real positions to flatten and a raw HTTP call would reach a live
            // endpoint even in paper mode (dangerous).
            if !crate::chart_renderer::trading::order_manager::is_paper_mode() {
                std::thread::spawn(|| {
                    let _ = reqwest::blocking::Client::new()
                        .post(format!("{}/risk/flatten", crate::chart_renderer::gpu::APEXIB_URL))
                        .timeout(std::time::Duration::from_secs(5))
                        .send();
                });
            }
        }
        // Ctrl+Shift+K: Kill Switch — cancel all orders, flatten positions, and
        // halt new trading. Single handler: Halt Trading was relocated off ⌘⇧H
        // (now the TPS boss key) onto ⌘⇧K, which already bound Kill Switch —
        // the two are merged so the chord fires exactly one combined action.
        if ui.input(|i| i.modifiers.command && i.modifiers.shift && i.key_pressed(egui::Key::K)) {
            crate::chart_renderer::trading::order_manager::kill_switch();
            let _ = crate::chart_renderer::trading::order_manager::halt_trading();
            chart.orders.clear();
            PENDING_TOASTS.with(|ts| ts.borrow_mut().push(
                crate::chart_renderer::ui::tools::notification::Notification::new(
                    "KILL SWITCH — orders cancelled, trading halted",
                    crate::chart_renderer::ui::tools::notification::NotificationSeverity::Error,
                ).with_source("trading")
            ));
        }
        // Ctrl+Shift+R: Resume trading
        if ui.input(|i| i.modifiers.command && i.modifiers.shift && i.key_pressed(egui::Key::R)) {
            let _ = crate::chart_renderer::trading::order_manager::resume_trading();
            PENDING_TOASTS.with(|ts| ts.borrow_mut().push(
                crate::chart_renderer::ui::tools::notification::Notification::new(
                    "Trading RESUMED",
                    crate::chart_renderer::ui::tools::notification::NotificationSeverity::Success,
                ).with_source("trading")
            ));
        }
    }
}
