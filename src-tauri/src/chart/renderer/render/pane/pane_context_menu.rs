//! Right-click context menu for the chart pane.
//!
//! Extracted from `core.rs` — interaction-gated (only executes inside
//! `resp.context_menu()`), zero per-frame render cost.

#![allow(unused_imports)]
#![allow(unused_variables)]

use crate::chart_renderer::gpu::*;
use crate::chart_renderer::trading::*;
use crate::chart_renderer::{LineStyle, PlayLineKind};
use crate::chart_renderer::ui::style::{
    color_alpha, mono_xs,
    font_2xs, font_xs, font_sm,
    COLOR_AMBER,
    TEXT_PRIMARY,
};
use crate::ui_kit::icons::Icon;
use crate::ui_kit::widgets::MenuItem;

/// Render the right-click context menu body.
///
/// Called from inside `resp.context_menu(|ui| { pane_context_menu(...) })`.
///
/// Parameters mirror every local captured by the original closure:
/// - `ui`           — the menu's inner `Ui`
/// - `chart`        — the active `Chart` (mutated by menu actions)
/// - `t`            — resolved `Theme` reference
/// - `min_p`/`max_p` — visible price range (for clamping click price)
/// - `n`            — bar count (for reset-view scroll offset)
/// - `watchlist`    — mutable `Watchlist` (templates, screenshots, etc.)
/// - `pos_to_price` — closure mapping a screen `Pos2` → price `f32`
pub(super) fn pane_context_menu<F>(
    ui: &mut egui::Ui,
    chart: &mut Chart,
    t: &Theme,
    min_p: f32,
    max_p: f32,
    n: usize,
    watchlist: &mut Watchlist,
    pos_to_price: &F,
) where F: Fn(egui::Pos2) -> f32 {
        // Clamp the click-derived price to the visible range. Right-click
        // positions outside the chart body (e.g. near the pane header / divider)
        // would otherwise extrapolate beyond min_p/max_p, producing an order
        // that places at an off-screen price and never renders.
        let raw_click_price = ui.input(|i| i.pointer.latest_pos()).map(|p| pos_to_price(p)).unwrap_or(0.0);
        let click_price = raw_click_price.clamp(min_p, max_p);
        let click_pos = ui.input(|i| i.pointer.latest_pos());

        // ── View controls (top) ──
        if MenuItem::new("Reset View").icon(Icon::ARROW_COUNTER_CLOCKWISE).show(ui, t).clicked() {
            chart.auto_scroll = true; chart.price_lock = None;
            chart.vs = (n as f32 - chart.vc as f32 + 8.0).max(0.0);
            ui.close_menu();
        }
        if MenuItem::new("Drag Zoom").icon(Icon::MAGNIFYING_GLASS_PLUS).show(ui, t).clicked() {
            chart.zoom_selecting = true; chart.zoom_start = egui::Pos2::ZERO;
            ui.close_menu();
        }
        if MenuItem::new("Measure (Shift+Drag)").icon(Icon::RULER).show(ui, t).clicked() {
            chart.measure.mode = true; chart.measure.start = None;
            ui.close_menu();
        }
        ui.separator();

        ui.label(egui::RichText::new(format!("ORDERS @ {:.2}", click_price)).small().color(t.dim));
        if MenuItem::new("Buy Order").icon(Icon::ARROW_FAT_UP).tint(t.bull).show(ui, t).clicked() {
            use crate::chart_renderer::trading::order_manager::*;
            if let Some(id) = submit_and_get_id(OrderIntent {
                symbol: chart.symbol.clone(), side: OrderSide::Buy,
                order_type: ManagedOrderType::Limit, price: click_price, qty: chart.order_panel.qty,
                source: OrderSource::ChartClick, pair_with: None, option_symbol: None, option_con_id: None, stop_price: 0.0, trail_amount: None, trail_percent: None, last_price: 0.0, tif: 0, outside_rth: false,
                strategy_id: None, override_warnings: false,
            }) {
                chart.orders.push(OrderLevel { id: id as u32, side: OrderSide::Buy, price: click_price, qty: chart.order_panel.qty, status: OrderStatus::Draft, state: OrderState::Draft, pair_id: None, option_symbol: None, option_con_id: None, trail_amount: None, trail_percent: None, filled_ratio: 0.0 });
            }
            ui.close_menu();
        }
        if MenuItem::new("Sell Order").icon(Icon::ARROW_FAT_DOWN).tint(t.bear).show(ui, t).clicked() {
            use crate::chart_renderer::trading::order_manager::*;
            if let Some(id) = submit_and_get_id(OrderIntent {
                symbol: chart.symbol.clone(), side: OrderSide::Sell,
                order_type: ManagedOrderType::Limit, price: click_price, qty: chart.order_panel.qty,
                source: OrderSource::ChartClick, pair_with: None, option_symbol: None, option_con_id: None, stop_price: 0.0, trail_amount: None, trail_percent: None, last_price: 0.0, tif: 0, outside_rth: false,
                strategy_id: None, override_warnings: false,
            }) {
                chart.orders.push(OrderLevel { id: id as u32, side: OrderSide::Sell, price: click_price, qty: chart.order_panel.qty, status: OrderStatus::Draft, state: OrderState::Draft, pair_id: None, option_symbol: None, option_con_id: None, trail_amount: None, trail_percent: None, filled_ratio: 0.0 });
            }
            ui.close_menu();
        }
        if MenuItem::new("Stop Loss").icon(Icon::SHIELD_WARNING).tint(t.bear).show(ui, t).clicked() {
            use crate::chart_renderer::trading::order_manager::*;
            if let Some(id) = submit_and_get_id(OrderIntent {
                symbol: chart.symbol.clone(), side: OrderSide::Stop,
                order_type: ManagedOrderType::Stop, price: click_price, qty: chart.order_panel.qty,
                source: OrderSource::ChartClick, pair_with: None, option_symbol: None, option_con_id: None, stop_price: 0.0, trail_amount: None, trail_percent: None, last_price: 0.0, tif: 0, outside_rth: false,
                strategy_id: None, override_warnings: false,
            }) {
                chart.orders.push(OrderLevel { id: id as u32, side: OrderSide::Stop, price: click_price, qty: chart.order_panel.qty, status: OrderStatus::Draft, state: OrderState::Draft, pair_id: None, option_symbol: None, option_con_id: None, trail_amount: None, trail_percent: None, filled_ratio: 0.0 });
            }
            ui.close_menu();
        }
        // OCO Bracket (simple) — routed through IB native OCO API
        if MenuItem::new("OCO Bracket").tint(t.accent).show(ui, t).clicked() {
            use crate::chart_renderer::trading::order_manager::*;
            let target_price = click_price * 1.01;
            let stop_price = click_price * 0.99;
            let intents = vec![
                OrderIntent {
                    symbol: chart.symbol.clone(), side: OrderSide::OcoTarget,
                    order_type: ManagedOrderType::Limit, price: target_price, stop_price: 0.0, qty: chart.order_panel.qty,
                    source: OrderSource::Oco, pair_with: None, option_symbol: None, option_con_id: None,
                    trail_amount: None, trail_percent: None, last_price: 0.0, tif: 0, outside_rth: false,
                    strategy_id: None, override_warnings: false,
                },
                OrderIntent {
                    symbol: chart.symbol.clone(), side: OrderSide::OcoStop,
                    order_type: ManagedOrderType::Stop, price: stop_price, stop_price: stop_price, qty: chart.order_panel.qty,
                    source: OrderSource::Oco, pair_with: None, option_symbol: None, option_con_id: None,
                    trail_amount: None, trail_percent: None, last_price: 0.0, tif: 0, outside_rth: false,
                    strategy_id: None, override_warnings: false,
                },
            ];
            let results = submit_oco_order(intents.clone());
            let mut ids: Vec<u64> = Vec::new();
            for (i, r) in results.iter().enumerate() {
                match r {
                    OrderResult::Accepted(id) | OrderResult::NeedsConfirmation(id) => ids.push(*id),
                    OrderResult::NeedsApproval { reason, .. } => {
                        // Per-leg approval: enqueue the matching original
                        // intent so the modal can resubmit just that leg with
                        // override_warnings=true. (OCO leg pairing is rebuilt
                        // server-side from the oca_group on the resubmit.)
                        if let Some(intent) = intents.get(i).cloned() {
                            enqueue_approval(reason.clone(), intent);
                        } else {
                            eprintln!("[oco] leg needs approval (dropped, idx out of range): {}", reason);
                        }
                    }
                    OrderResult::Rejected(reason) => {
                        eprintln!("[oco] leg rejected: {}", reason);
                    }
                    OrderResult::Duplicate => { /* silently blocked */ }
                }
            }
            if ids.len() >= 2 {
                chart.orders.push(OrderLevel { id: ids[0] as u32, side: OrderSide::OcoTarget, price: target_price, qty: chart.order_panel.qty, status: OrderStatus::Draft, state: OrderState::Draft, pair_id: Some(ids[1] as u32), option_symbol: None, option_con_id: None, trail_amount: None, trail_percent: None, filled_ratio: 0.0 });
                chart.orders.push(OrderLevel { id: ids[1] as u32, side: OrderSide::OcoStop, price: stop_price, qty: chart.order_panel.qty, status: OrderStatus::Draft, state: OrderState::Draft, pair_id: Some(ids[0] as u32), option_symbol: None, option_con_id: None, trail_amount: None, trail_percent: None, filled_ratio: 0.0 });
            }
            ui.close_menu();
        }
        // Bracket presets submenu — keep menu_button so the native popup works;
        // only the trigger label changes (no submenu MenuItem replacement needed here
        // because menu_button manages the popup).
        MenuItem::new("\u{21C5} Bracket Presets").tint(t.accent).show_menu(ui, t, |ui| {
            let templates = chart.bracket_templates.clone();
            let mut delete_idx: Option<usize> = None;
            for (ti, tmpl) in templates.iter().enumerate() {
                ui.horizontal(|ui| {
                    if MenuItem::new(format!("{} (+{}% / -{}%)", tmpl.name, tmpl.target_pct, tmpl.stop_pct)).show(ui, t).clicked() {
                        use crate::chart_renderer::trading::order_manager::*;
                        let target_price = click_price * (1.0 + tmpl.target_pct / 100.0);
                        let stop_price   = click_price * (1.0 - tmpl.stop_pct  / 100.0);
                        let intents = vec![
                            OrderIntent {
                                symbol: chart.symbol.clone(), side: OrderSide::OcoTarget,
                                order_type: ManagedOrderType::Limit, price: target_price, stop_price: 0.0, qty: chart.order_panel.qty,
                                source: OrderSource::Oco, pair_with: None, option_symbol: None, option_con_id: None,
                                trail_amount: None, trail_percent: None, last_price: 0.0, tif: 0, outside_rth: false,
                                strategy_id: None, override_warnings: false,
                            },
                            OrderIntent {
                                symbol: chart.symbol.clone(), side: OrderSide::OcoStop,
                                order_type: ManagedOrderType::Stop, price: stop_price, stop_price: stop_price, qty: chart.order_panel.qty,
                                source: OrderSource::Oco, pair_with: None, option_symbol: None, option_con_id: None,
                                trail_amount: None, trail_percent: None, last_price: 0.0, tif: 0, outside_rth: false,
                                strategy_id: None, override_warnings: false,
                            },
                        ];
                        let results = submit_oco_order(intents.clone());
                        let mut ids: Vec<u64> = Vec::new();
                        for (i, r) in results.iter().enumerate() {
                            match r {
                                OrderResult::Accepted(id) | OrderResult::NeedsConfirmation(id) => ids.push(*id),
                                OrderResult::NeedsApproval { reason, .. } => {
                                    if let Some(intent) = intents.get(i).cloned() {
                                        enqueue_approval(reason.clone(), intent);
                                    } else {
                                        eprintln!("[bracket-preset] leg needs approval (dropped, idx out of range): {}", reason);
                                    }
                                }
                                OrderResult::Rejected(reason) => {
                                    eprintln!("[bracket-preset] leg rejected: {}", reason);
                                }
                                OrderResult::Duplicate => { /* silently blocked */ }
                            }
                        }
                        if ids.len() >= 2 {
                            chart.orders.push(OrderLevel { id: ids[0] as u32, side: OrderSide::OcoTarget, price: target_price, qty: chart.order_panel.qty, status: OrderStatus::Draft, state: OrderState::Draft, pair_id: Some(ids[1] as u32), option_symbol: None, option_con_id: None, trail_amount: None, trail_percent: None, filled_ratio: 0.0 });
                            chart.orders.push(OrderLevel { id: ids[1] as u32, side: OrderSide::OcoStop,   price: stop_price,   qty: chart.order_panel.qty, status: OrderStatus::Draft, state: OrderState::Draft, pair_id: Some(ids[0] as u32), option_symbol: None, option_con_id: None, trail_amount: None, trail_percent: None, filled_ratio: 0.0 });
                        }
                        ui.close_menu();
                    }
                    // The inline delete X inside horizontal bracket-preset rows is
                    // a compact frameless icon, not a full-width menu row — left as-is.
                    // Inline delete-X — use ui_kit's `Button::icon` with the
                    // muted-icon variant so we stop bypassing the kit. Visual
                    // result: the same frameless tiny X glyph.
                    if crate::ui_kit::widgets::Button::icon(Icon::X)
                        .variant(crate::ui_kit::widgets::tokens::Variant::MutedIcon)
                        .glyph_size(crate::ui_kit::tokens::font_2xs())
                        .show(ui, t).clicked() {
                        delete_idx = Some(ti);
                    }
                });
            }
            if let Some(idx) = delete_idx { chart.bracket_templates.remove(idx); }
            ui.separator();
            // Create new preset inline
            ui.label(egui::RichText::new("NEW PRESET").monospace().size(font_2xs()).color(t.dim));
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Name").monospace().size(font_xs()).color(t.dim));
                crate::ui_kit::widgets::Input::new(&mut chart.new_bracket_name)
                    .width(60.0)
                    .size(crate::ui_kit::widgets::tokens::Size::Xs)
                    .show(ui, t);
            });
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Target %").monospace().size(font_xs()).color(t.dim));
                crate::ui_kit::widgets::Input::number(&mut chart.new_bracket_target)
                    .width(40.0)
                    .size(crate::ui_kit::widgets::tokens::Size::Xs)
                    .show(ui, t);
                ui.label(egui::RichText::new("Stop %").monospace().size(font_xs()).color(t.dim));
                crate::ui_kit::widgets::Input::number(&mut chart.new_bracket_stop)
                    .width(40.0)
                    .size(crate::ui_kit::widgets::tokens::Size::Xs)
                    .show(ui, t);
            });
            let can_create = !chart.new_bracket_name.trim().is_empty()
                && chart.new_bracket_target.parse::<f32>().is_ok()
                && chart.new_bracket_stop.parse::<f32>().is_ok();
            if MenuItem::new("Create").icon(Icon::PLUS).tint(t.accent).enabled(can_create).show(ui, t).clicked() {
                chart.bracket_templates.push(BracketTemplate {
                    name: chart.new_bracket_name.trim().to_string(),
                    target_pct: chart.new_bracket_target.parse().unwrap_or(1.0),
                    stop_pct: chart.new_bracket_stop.parse().unwrap_or(0.5),
                });
                chart.new_bracket_name.clear();
                chart.new_bracket_target.clear();
                chart.new_bracket_stop.clear();
            }
        });
        if MenuItem::new("Trigger Order").tint(t.accent).show(ui, t).clicked() {
            use crate::chart_renderer::trading::order_manager::*;
            let target_price = click_price * 1.02;
            if let Some(id1) = submit_and_get_id(OrderIntent {
                symbol: chart.symbol.clone(), side: OrderSide::TriggerBuy,
                order_type: ManagedOrderType::Limit, price: click_price, qty: chart.order_panel.qty,
                source: OrderSource::Trigger, pair_with: None, option_symbol: None, option_con_id: None, stop_price: 0.0, trail_amount: None, trail_percent: None, last_price: 0.0, tif: 0, outside_rth: false,
                strategy_id: None, override_warnings: false,
            }) {
                if let Some(id2) = submit_and_get_id(OrderIntent {
                    symbol: chart.symbol.clone(), side: OrderSide::TriggerSell,
                    order_type: ManagedOrderType::Limit, price: target_price, qty: chart.order_panel.qty,
                    source: OrderSource::Trigger, pair_with: Some(id1), option_symbol: None, option_con_id: None, stop_price: 0.0, trail_amount: None, trail_percent: None, last_price: 0.0, tif: 0, outside_rth: false,
                    strategy_id: None, override_warnings: false,
                }) {
                    chart.orders.push(OrderLevel { id: id1 as u32, side: OrderSide::TriggerBuy, price: click_price, qty: chart.order_panel.qty, status: OrderStatus::Draft, state: OrderState::Draft, pair_id: Some(id2 as u32), option_symbol: None, option_con_id: None, trail_amount: None, trail_percent: None, filled_ratio: 0.0 });
                    chart.orders.push(OrderLevel { id: id2 as u32, side: OrderSide::TriggerSell, price: target_price, qty: chart.order_panel.qty, status: OrderStatus::Draft, state: OrderState::Draft, pair_id: Some(id1 as u32), option_symbol: None, option_con_id: None, trail_amount: None, trail_percent: None, filled_ratio: 0.0 });
                }
            }
            ui.close_menu();
        }
        if !chart.orders.is_empty() {
            if MenuItem::new("Cancel All Orders").icon(Icon::TRASH).tint(t.bear).show(ui, t).clicked() {
                crate::chart_renderer::trading::order_manager::cancel_all_orders(&chart.symbol);
                chart.orders.clear(); ui.close_menu();
            }
        }
        // ── Play lines (when editor active) ──
        if !chart.play_lines.is_empty() {
            ui.separator();
            ui.label(egui::RichText::new("PLAY LEVELS").small().color(t.accent));
            if MenuItem::new(format!("Set Entry @ {:.2}", click_price)).show(ui, t).clicked() {
                if let Some(pl) = chart.play_lines.iter_mut().find(|l| l.kind == crate::chart_renderer::PlayLineKind::Entry) {
                    pl.price = click_price;
                }
                ui.close_menu();
            }
            if MenuItem::new(format!("Set Target @ {:.2}", click_price)).show(ui, t).clicked() {
                if let Some(pl) = chart.play_lines.iter_mut().find(|l| l.kind == crate::chart_renderer::PlayLineKind::Target) {
                    pl.price = click_price;
                }
                ui.close_menu();
            }
            if chart.play_lines.iter().any(|l| l.kind == crate::chart_renderer::PlayLineKind::Stop) {
                if MenuItem::new(format!("Set Stop @ {:.2}", click_price)).show(ui, t).clicked() {
                    if let Some(pl) = chart.play_lines.iter_mut().find(|l| l.kind == crate::chart_renderer::PlayLineKind::Stop) {
                        pl.price = click_price;
                    }
                    ui.close_menu();
                }
            }
        }
        ui.separator();
        ui.label(egui::RichText::new(format!("ALERTS @ {:.2}", click_price)).small().color(t.dim));
        // Context-menu alerts are created as DRAFTS — user must Place them from the alerts panel
        // (same pattern as orders: draft → placed → active)
        if MenuItem::new(format!("Alert Above {:.2}", click_price)).icon(Icon::ARROW_FAT_UP).show(ui, t).clicked() {
            let id = chart.next_alert_id; chart.next_alert_id += 1;
            chart.price_alerts.push(PriceAlert { id, price: click_price, above: true, triggered: false, draft: true, symbol: chart.symbol.clone() });
            ui.close_menu();
        }
        if MenuItem::new(format!("Alert Below {:.2}", click_price)).icon(Icon::ARROW_FAT_DOWN).show(ui, t).clicked() {
            let id = chart.next_alert_id; chart.next_alert_id += 1;
            chart.price_alerts.push(PriceAlert { id, price: click_price, above: false, triggered: false, draft: true, symbol: chart.symbol.clone() });
            ui.close_menu();
        }
        ui.separator();
        ui.label(egui::RichText::new("DRAWING TOOLS").small().color(t.dim));
        {
            let dtm_out = crate::chart_renderer::ui::widgets::drawing::show_drawing_tool_menu(ui, chart, watchlist);
            if let Some(tool) = dtm_out.new_tool {
                chart.draw_tool = tool;
                chart.pending_pt = None; chart.pending_pt2 = None; chart.pending_pts.clear();
            }
        }
        ui.separator();

        // ══════════════════════════════════════════════════════
        // ── TEMPLATES section ──
        ui.separator();
        {
            let tmpl_out = crate::chart_renderer::ui::widgets::drawing::show_template_menu(ui, chart, watchlist);
            // Deferred apply: happens after the menu_button closure releases borrows
            if let Some(i) = tmpl_out.apply_tmpl {
                let tmpl = watchlist.pane_templates[i].1.clone();
                let gb = |key: &str, def: bool| -> bool { tmpl.get(key).and_then(|v| v.as_bool()).unwrap_or(def) };
                chart.show_volume = gb("show_volume", true);
                chart.show_oscillators = gb("show_oscillators", true);
                chart.ohlc_tooltip = gb("ohlc_tooltip", true);
                chart.magnet = gb("magnet", true);
                chart.log_scale = gb("log_scale", false);
                chart.show_vwap_bands = gb("show_vwap_bands", false);
                chart.show_cvd = gb("show_cvd", false);
                chart.show_delta_volume = gb("show_delta_volume", false);
                chart.show_rvol = gb("show_rvol", true);
                chart.show_ma_ribbon = gb("show_ma_ribbon", false);
                chart.show_prev_close = gb("show_prev_close", true);
                chart.show_auto_sr = gb("show_auto_sr", false);
                chart.show_auto_fib = gb("show_auto_fib", false);
                chart.show_footprint = gb("show_footprint", false);
                chart.show_gamma = gb("show_gamma", false);
                chart.show_darkpool = gb("show_darkpool", false);
                chart.show_events = gb("show_events", false);
                chart.hit_highlight = gb("hit_highlight", false);
                chart.show_pnl_curve = gb("show_pnl_curve", false);
                chart.show_pattern_labels = gb("show_pattern_labels", true);
                chart.candle_mode = match tmpl.get("candle_mode").and_then(|v| v.as_str()).unwrap_or("std") {
                    "vln" => CandleMode::Violin, "grd" => CandleMode::Gradient, "vg" => CandleMode::ViolinGradient,
                    "ha" => CandleMode::HeikinAshi, "line" => CandleMode::Line, "area" => CandleMode::Area,
                    "rnk" => CandleMode::Renko, "rng" => CandleMode::RangeBar, "tck" => CandleMode::TickBar,
                    _ => CandleMode::Standard,
                };
                if let Some(inds) = tmpl.get("indicators").and_then(|v| v.as_array()) {
                    chart.indicators.clear();
                    for (idx, ind_json) in inds.iter().enumerate() {
                        let kind_label = ind_json.get("kind").and_then(|v| v.as_str()).unwrap_or("SMA");
                        let kind = IndicatorType::all().iter().find(|t| t.label() == kind_label).copied().unwrap_or(IndicatorType::SMA);
                        let period = ind_json.get("period").and_then(|v| v.as_u64()).unwrap_or(20) as usize;
                        let color = ind_json.get("color").and_then(|v| v.as_str()).unwrap_or(INDICATOR_COLORS[idx % INDICATOR_COLORS.len()]);
                        let id = chart.next_indicator_id; chart.next_indicator_id += 1;
                        let mut ind = Indicator::new(id, kind, period, color);
                        ind.visible = ind_json.get("visible").and_then(|v| v.as_bool()).unwrap_or(true);
                        ind.thickness = ind_json.get("thickness").and_then(|v| v.as_f64()).unwrap_or(1.5) as f32;
                        ind.param2 = ind_json.get("param2").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                        ind.param3 = ind_json.get("param3").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                        ind.param4 = ind_json.get("param4").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                        ind.upper_color = ind_json.get("upper_color").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        ind.lower_color = ind_json.get("lower_color").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        ind.fill_color_hex = ind_json.get("fill_color_hex").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        ind.upper_thickness = ind_json.get("upper_thickness").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                        ind.lower_thickness = ind_json.get("lower_thickness").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                        ind.line_style = match ind_json.get("line_style").and_then(|v| v.as_str()).unwrap_or("solid") {
                            "dashed" => LineStyle::Dashed, "dotted" => LineStyle::Dotted, _ => LineStyle::Solid,
                        };
                        chart.indicators.push(ind);
                    }
                    chart.indicator_bar_count = 0;
                }
            }
            // Save current pane as template (button rendered by show_template_menu above)
            if tmpl_out.save_as_template {
                let name = format!("Template {}", watchlist.pane_templates.len() + 1);
                let indicators: Vec<serde_json::Value> = chart.indicators.iter().map(|ind| serde_json::json!({
                    "kind": ind.kind.label(), "period": ind.period, "color": ind.color,
                    "visible": ind.visible, "thickness": ind.thickness,
                    "param2": ind.param2, "param3": ind.param3, "param4": ind.param4,
                    "source": ind.source, "offset": ind.offset,
                    "ob_level": ind.ob_level, "os_level": ind.os_level,
                    "source_tf": ind.source_tf,
                    "line_style": match ind.line_style { LineStyle::Solid => "solid", LineStyle::Dashed => "dashed", LineStyle::Dotted => "dotted" },
                    "upper_color": ind.upper_color, "lower_color": ind.lower_color,
                    "fill_color_hex": ind.fill_color_hex,
                    "upper_thickness": ind.upper_thickness, "lower_thickness": ind.lower_thickness,
                })).collect();
                let tmpl = serde_json::json!({
                    "show_volume": chart.show_volume, "show_oscillators": chart.show_oscillators,
                    "ohlc_tooltip": chart.ohlc_tooltip, "magnet": chart.magnet, "log_scale": chart.log_scale,
                    "show_vwap_bands": chart.show_vwap_bands, "show_cvd": chart.show_cvd,
                    "show_delta_volume": chart.show_delta_volume, "show_rvol": chart.show_rvol,
                    "show_ma_ribbon": chart.show_ma_ribbon, "show_prev_close": chart.show_prev_close,
                    "show_auto_sr": chart.show_auto_sr, "show_auto_fib": chart.show_auto_fib,
                    "show_footprint": chart.show_footprint, "show_gamma": chart.show_gamma,
                    "show_darkpool": chart.show_darkpool, "show_events": chart.show_events,
                    "hit_highlight": chart.hit_highlight, "show_pnl_curve": chart.show_pnl_curve,
                    "show_pattern_labels": chart.show_pattern_labels,
                    "candle_mode": match chart.candle_mode {
                        CandleMode::Standard => "std", CandleMode::Violin => "vln",
                        CandleMode::Gradient => "grd", CandleMode::ViolinGradient => "vg",
                        CandleMode::HeikinAshi => "ha", CandleMode::Line => "line", CandleMode::Area => "area",
                        CandleMode::Renko => "rnk", CandleMode::RangeBar => "rng", CandleMode::TickBar => "tck",
                    },
                    "indicators": indicators,
                });
                watchlist.pane_templates.push((name, tmpl));
                save_templates(&watchlist.pane_templates);
            }
        }

        // ── HIDE section ──
        // ══════════════════════════════════════════════════════
        let everything_hidden = chart.hide_all_drawings && chart.hide_all_indicators && chart.hide_signal_drawings;
        let hide_all_label = if everything_hidden { "Show All" } else { "Hide All" };
        let hide_all_icon = if everything_hidden { Icon::EYE } else { Icon::EYE_SLASH };
        if MenuItem::new(hide_all_label).icon(hide_all_icon).show(ui, t).clicked() {
            let target = !everything_hidden;
            chart.hide_all_drawings    = target;
            chart.hide_all_indicators  = target;
            chart.hide_signal_drawings = target;
            ui.close_menu();
        }
        MenuItem::new("Hide / Show").icon(Icon::EYE).show_menu(ui, t, |ui| {
            // Drawings
            ui.label(egui::RichText::new("DRAWINGS").small().color(t.dim));
            {
                let icon = if chart.hide_all_drawings { Icon::EYE_SLASH } else { Icon::EYE };
                let lbl  = if chart.hide_all_drawings { "Show All Drawings" } else { "Hide All Drawings" };
                if MenuItem::new(lbl).icon(icon).show(ui, t).clicked() {
                    chart.hide_all_drawings = !chart.hide_all_drawings;
                    ui.close_menu();
                }
            }
            // By drawing group
            for g in chart.groups.clone() {
                let count = chart.drawings.iter().filter(|d| d.group_id == g.id).count();
                if count == 0 { continue; }
                let hidden = chart.hidden_groups.contains(&g.id);
                let icon = if hidden { Icon::EYE_SLASH } else { Icon::EYE };
                let label = format!("  {} ({})", g.name, count);
                if MenuItem::new(label).icon(icon).show(ui, t).clicked() {
                    if hidden { chart.hidden_groups.retain(|x| x != &g.id); }
                    else      { chart.hidden_groups.push(g.id.clone()); }
                    ui.close_menu();
                }
            }
            ui.separator();

            // Indicators
            ui.label(egui::RichText::new("INDICATORS").small().color(t.dim));
            {
                let icon = if chart.hide_all_indicators { Icon::EYE_SLASH } else { Icon::EYE };
                let lbl  = if chart.hide_all_indicators { "Show All Indicators" } else { "Hide All Indicators" };
                if MenuItem::new(lbl).icon(icon).show(ui, t).clicked() {
                    chart.hide_all_indicators = !chart.hide_all_indicators;
                    ui.close_menu();
                }
            }
            let ind_snapshot: Vec<(u32, String, bool)> = chart.indicators.iter()
                .map(|i| (i.id, i.display_name(), i.visible)).collect();
            for (id, name, visible) in &ind_snapshot {
                let icon = if *visible { Icon::EYE } else { Icon::EYE_SLASH };
                let label = format!("  {}", name);
                if MenuItem::new(label).icon(icon).show(ui, t).clicked() {
                    if let Some(ind) = chart.indicators.iter_mut().find(|i| i.id == *id) {
                        ind.visible = !ind.visible;
                    }
                    ui.close_menu();
                }
            }
            ui.separator();

            // Signals
            ui.label(egui::RichText::new("SIGNALS").small().color(t.dim));
            {
                let icon = if chart.hide_signal_drawings { Icon::EYE_SLASH } else { Icon::EYE };
                let lbl  = if chart.hide_signal_drawings { "Show Signal Lines" } else { "Hide Signal Lines" };
                if MenuItem::new(lbl).icon(icon).show(ui, t).clicked() {
                    chart.hide_signal_drawings = !chart.hide_signal_drawings;
                    ui.close_menu();
                }
            }
            {
                let icon = if chart.show_pattern_labels { Icon::EYE } else { Icon::EYE_SLASH };
                let lbl  = if chart.show_pattern_labels { "Hide Pattern Labels" } else { "Show Pattern Labels" };
                if MenuItem::new(lbl).icon(icon).show(ui, t).clicked() {
                    chart.show_pattern_labels = !chart.show_pattern_labels;
                    ui.close_menu();
                }
            }
        });

        ui.separator();

        // ══════════════════════════════════════════════════════
        // ── DELETE section ──
        // ══════════════════════════════════════════════════════
        if !chart.selected_ids.is_empty() {
            if MenuItem::new(format!("Delete Selected ({})", chart.selected_ids.len())).icon(Icon::TRASH).tint(t.bear).show(ui, t).clicked() {
                let ids = chart.selected_ids.clone();
                for d in chart.drawings.iter().filter(|d| ids.contains(&d.id)) {
                    if chart.undo_stack.len() >= 50 { chart.undo_stack.remove(0); }
                    chart.undo_stack.push(DrawingAction::Remove(d.clone()));
                }
                chart.redo_stack.clear();
                for id in &ids { crate::drawing_db::remove(id); }
                chart.drawings.retain(|d| !ids.contains(&d.id));
                chart.selected_ids.clear(); chart.selected_id = None;
                ui.close_menu();
            }
        }
        if !chart.drawings.is_empty() {
            if MenuItem::new("Delete All Drawings").icon(Icon::TRASH).tint(t.bear).show(ui, t).clicked() {
                for d in &chart.drawings {
                    if chart.undo_stack.len() >= 50 { chart.undo_stack.remove(0); }
                    chart.undo_stack.push(DrawingAction::Remove(d.clone()));
                }
                chart.redo_stack.clear();
                for d in &chart.drawings { crate::drawing_db::remove(&d.id); }
                chart.drawings.clear();
                chart.selected_ids.clear(); chart.selected_id = None;
                ui.close_menu();
            }
        }
        let temp_count = chart.drawings.iter().filter(|d| d.group_id == "default").count();
        if temp_count > 0 {
            if MenuItem::new(format!("Delete Temp Drawings ({})", temp_count)).icon(Icon::TRASH).tint(t.bear).show(ui, t).clicked() {
                let to_remove: Vec<String> = chart.drawings.iter().filter(|d| d.group_id == "default").map(|d| d.id.clone()).collect();
                for id in &to_remove { crate::drawing_db::remove(id); }
                chart.drawings.retain(|d| d.group_id != "default");
                chart.selected_ids.clear(); chart.selected_id = None;
                ui.close_menu();
            }
        }
        MenuItem::new("Delete").icon(Icon::TRASH).show_menu(ui, t, |ui| {
            let red = t.bear;

            // Drawings
            ui.label(egui::RichText::new("DRAWINGS").small().color(t.dim));
            if !chart.drawings.is_empty() {
                if MenuItem::new("All Drawings").icon(Icon::TRASH).tint(red).show(ui, t).clicked() {
                    for d in &chart.drawings {
                        if chart.undo_stack.len() >= 50 { chart.undo_stack.remove(0); }
                        chart.undo_stack.push(DrawingAction::Remove(d.clone()));
                    }
                    chart.redo_stack.clear();
                    for d in &chart.drawings { crate::drawing_db::remove(&d.id); }
                    chart.drawings.clear();
                    chart.selected_ids.clear(); chart.selected_id = None;
                    ui.close_menu();
                }
            }
            // By group — deletes drawings in THIS chart belonging to that group (not the group itself)
            for g in chart.groups.clone() {
                let count = chart.drawings.iter().filter(|d| d.group_id == g.id).count();
                if count == 0 { continue; }
                let label = format!("  {} ({})", g.name, count);
                if MenuItem::new(label).icon(Icon::TRASH).tint(red).show(ui, t).clicked() {
                    let gid = g.id.clone();
                    let ids: Vec<String> = chart.drawings.iter().filter(|d| d.group_id == gid).map(|d| d.id.clone()).collect();
                    for d in chart.drawings.iter().filter(|d| d.group_id == gid) {
                        if chart.undo_stack.len() >= 50 { chart.undo_stack.remove(0); }
                        chart.undo_stack.push(DrawingAction::Remove(d.clone()));
                    }
                    chart.redo_stack.clear();
                    for id in &ids { crate::drawing_db::remove(id); }
                    chart.drawings.retain(|d| d.group_id != gid);
                    chart.selected_ids.retain(|sid| !ids.contains(sid));
                    if let Some(ref s) = chart.selected_id { if ids.contains(s) { chart.selected_id = None; } }
                    ui.close_menu();
                }
            }
            ui.separator();

            // Indicators
            ui.label(egui::RichText::new("INDICATORS").small().color(t.dim));
            if !chart.indicators.is_empty() {
                if MenuItem::new("All Indicators").icon(Icon::TRASH).tint(red).show(ui, t).clicked() {
                    chart.indicators.clear();
                    chart.indicator_bar_count = 0;
                    ui.close_menu();
                }
            }
            let ind_snapshot: Vec<(u32, String)> = chart.indicators.iter()
                .map(|i| (i.id, i.display_name())).collect();
            for (id, name) in &ind_snapshot {
                let label = format!("  {}", name);
                if MenuItem::new(label).icon(Icon::TRASH).tint(red).show(ui, t).clicked() {
                    chart.indicators.retain(|i| i.id != *id);
                    ui.close_menu();
                }
            }
            ui.separator();

            // Signals
            ui.label(egui::RichText::new("SIGNALS").small().color(t.dim));
            if !chart.signal_drawings.is_empty() {
                // Pin: promote the visible auto-lines into saved, editable drawings.
                if MenuItem::new(format!("Pin auto-lines to chart ({})", chart.signal_drawings.len())).icon(Icon::PUSH_PIN).show(ui, t).clicked() {
                    crate::chart_renderer::gpu::pin_signal_drawings(chart);
                    ui.close_menu();
                }
                if MenuItem::new(format!("Signal Drawings ({})", chart.signal_drawings.len())).icon(Icon::TRASH).tint(red).show(ui, t).clicked() {
                    chart.signal_drawings.clear();
                    ui.close_menu();
                }
            }
            if !chart.pattern_labels.is_empty() {
                if MenuItem::new(format!("Pattern Labels ({})", chart.pattern_labels.len())).icon(Icon::TRASH).tint(red).show(ui, t).clicked() {
                    chart.pattern_labels.clear();
                    ui.close_menu();
                }
            }
        });
}
