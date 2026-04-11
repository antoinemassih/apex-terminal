//! Orders / Positions / Alerts side panel.

use egui;
use super::style::{close_button, separator, color_alpha, section_label, status_badge, order_card, action_btn};
use super::super::gpu::*;
use crate::ui_kit::icons::Icon;
use crate::chart_renderer::trading::{AccountSummary, IbOrder, Position, OrderStatus, OrderLevel, PriceAlert, cancel_order_with_pair, fmt_notional};

pub(crate) fn draw(
    ctx: &egui::Context,
    watchlist: &mut Watchlist,
    panes: &mut [Chart],
    ap: usize,
    t: &Theme,
    account_data_cached: &Option<(AccountSummary, Vec<Position>, Vec<IbOrder>)>,
) {
// ── Orders / Positions / Alerts side panel (left of watchlist) ─────────────
if watchlist.orders_panel_open {
    egui::SidePanel::right("orders_panel")
        .default_width(270.0)
        .min_width(220.0)
        .max_width(350.0)
        .frame(egui::Frame::NONE.fill(t.toolbar_bg)
            .inner_margin(egui::Margin { left: 8, right: 8, top: 8, bottom: 6 })
            .stroke(egui::Stroke::new(1.0, color_alpha(t.toolbar_border, 80))))
        .show(ctx, |ui| {
            // ── Panel close button ──
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("BOOK").monospace().size(11.0).strong().color(t.accent));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if close_button(ui, t.dim) { watchlist.orders_panel_open = false; }
                });
            });
            ui.add_space(4.0);

            // ══════════════════════════════════════════════════════
            // ── POSITIONS SECTION (top half of book) ──
            // ══════════════════════════════════════════════════════
            {
                let (ib_positions, ib_orders) = account_data_cached.as_ref().map(|(_, p, o)| (p.clone(), o.clone())).unwrap_or_default();
                let has_positions = !ib_positions.is_empty();

                // Header + Close All
                ui.horizontal(|ui| {
                    section_label(ui, "POSITIONS", t.accent);
                    if has_positions {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let del_color = t.bear;
                            if ui.add(egui::Button::new(egui::RichText::new("Close All")
                                .monospace().size(8.0).color(del_color))
                                .fill(color_alpha(del_color, 15)).corner_radius(2.0)
                                .stroke(egui::Stroke::new(0.5, color_alpha(del_color, 50)))
                                .min_size(egui::vec2(0.0, 16.0))).clicked() {
                                // Fire close-all via ApexIB
                                std::thread::spawn(|| {
                                    let _ = reqwest::blocking::Client::new()
                                        .post(format!("{}/risk/flatten", APEXIB_URL))
                                        .timeout(std::time::Duration::from_secs(5))
                                        .send();
                                });
                            }
                        });
                    }
                });
                ui.add_space(4.0);

                if has_positions {
                    let mut total_pnl: f64 = 0.0;
                    egui::ScrollArea::vertical().id_salt("positions_scroll").max_height(ui.available_height() * 0.45).show(ui, |ui| {
                        for pos in &ib_positions {
                            total_pnl += pos.unrealized_pnl;
                            let pnl_color = if pos.unrealized_pnl >= 0.0 { t.bull } else { t.bear };
                            order_card(ui, pnl_color, color_alpha(t.toolbar_border, 10), |ui| {
                                // Row 1: symbol, qty@price, close buttons
                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new(&pos.symbol).monospace().size(10.0).strong()
                                        .color(egui::Color32::from_rgb(220,220,230)));
                                    ui.label(egui::RichText::new(format!("{}@{:.2}", pos.qty, pos.avg_price))
                                        .monospace().size(9.0).color(t.dim.gamma_multiply(0.6)));
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        // Close button
                                        let close_color = t.bear;
                                        if ui.add(egui::Button::new(egui::RichText::new(Icon::X).size(9.0).color(close_color))
                                            .fill(color_alpha(close_color, 12)).corner_radius(2.0)
                                            .min_size(egui::vec2(18.0, 16.0))).clicked() {
                                            // Close full position via ApexIB
                                            let sym = pos.symbol.clone();
                                            let qty = pos.qty;
                                            let con_id = pos.con_id;
                                            std::thread::spawn(move || {
                                                let side = if qty > 0 { "SELL" } else { "BUY" };
                                                let _ = reqwest::blocking::Client::new()
                                                    .post(format!("{}/orders", APEXIB_URL))
                                                    .json(&serde_json::json!({
                                                        "conId": con_id, "side": side,
                                                        "quantity": qty.unsigned_abs(),
                                                        "orderType": "market"
                                                    }))
                                                    .timeout(std::time::Duration::from_secs(5))
                                                    .send();
                                            });
                                        }
                                        // Close half button
                                        if pos.qty.abs() > 1 {
                                            if ui.add(egui::Button::new(egui::RichText::new("\u{00BD}").size(9.0).color(t.dim))
                                                .fill(color_alpha(t.toolbar_border, 15)).corner_radius(2.0)
                                                .min_size(egui::vec2(18.0, 16.0))).clicked() {
                                                let sym = pos.symbol.clone();
                                                let half = (pos.qty.abs() / 2).max(1);
                                                let con_id = pos.con_id;
                                                let qty = pos.qty;
                                                std::thread::spawn(move || {
                                                    let side = if qty > 0 { "SELL" } else { "BUY" };
                                                    let _ = reqwest::blocking::Client::new()
                                                        .post(format!("{}/orders", APEXIB_URL))
                                                        .json(&serde_json::json!({
                                                            "conId": con_id, "side": side,
                                                            "quantity": half,
                                                            "orderType": "market"
                                                        }))
                                                        .timeout(std::time::Duration::from_secs(5))
                                                        .send();
                                                });
                                            }
                                        }
                                    });
                                });
                                // Row 2: P&L + market value
                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new(format!("{:+.2}", pos.unrealized_pnl))
                                        .monospace().size(12.0).strong().color(pnl_color));
                                    ui.add_space(4.0);
                                    ui.label(egui::RichText::new(format!("({:+.1}%)", pos.pnl_pct()))
                                        .monospace().size(9.0).color(pnl_color));
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        ui.label(egui::RichText::new(format!("${:.0}", pos.market_value))
                                            .monospace().size(8.0).color(t.dim.gamma_multiply(0.4)));
                                    });
                                });
                            });
                        }
                    });
                    // Total P&L row
                    let total_color = if total_pnl >= 0.0 { t.bull } else { t.bear };
                    ui.add_space(2.0);
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Total P&L").monospace().size(9.0).color(t.dim));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(egui::RichText::new(format!("{:+.2}", total_pnl)).monospace().size(11.0).strong().color(total_color));
                        });
                    });
                } else {
                    ui.add_space(8.0);
                    ui.label(egui::RichText::new("No open positions").monospace().size(10.0).color(t.dim.gamma_multiply(0.4)));
                    ui.add_space(8.0);
                }

                ui.add_space(4.0);
            }

            // ══════════════════════════════════════════════════════
            // ── THICK DIVIDER between positions and orders ──
            // ══════════════════════════════════════════════════════
            ui.add_space(4.0);
            {
                let r = ui.available_rect_before_wrap();
                let y = ui.cursor().min.y;
                // 2px solid line like sidebar border
                ui.painter().rect_filled(
                    egui::Rect::from_min_max(egui::pos2(r.left(), y), egui::pos2(r.right(), y + 2.0)),
                    0.0, color_alpha(t.toolbar_border, 120));
                ui.add_space(6.0);
            }

            // ══════════════════════════════════════════════════════
            // ── ORDERS SECTION (bottom half of book) ──
            // ══════════════════════════════════════════════════════

            // Orders header + action bar
            ui.horizontal(|ui| {
                section_label(ui, "ORDERS", t.accent);
                let draft_count: usize = panes.iter().map(|p| p.orders.iter().filter(|o| o.status == OrderStatus::Draft).count()).sum();
                let active_count: usize = panes.iter().map(|p| p.orders.iter().filter(|o| o.status == OrderStatus::Draft || o.status == OrderStatus::Placed).count()).sum();
                if active_count > 0 || draft_count > 0 {
                    ui.add_space(4.0);
                    ui.label(egui::RichText::new(format!("{}d {}a", draft_count, active_count - draft_count))
                        .monospace().size(8.0).color(t.dim.gamma_multiply(0.5)));
                }
            });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                let draft_count: usize = panes.iter().map(|p| p.orders.iter().filter(|o| o.status == OrderStatus::Draft).count()).sum();
                let active_count: usize = panes.iter().map(|p| p.orders.iter().filter(|o| o.status == OrderStatus::Draft || o.status == OrderStatus::Placed).count()).sum();
                let history_count: usize = panes.iter().map(|p| p.orders.iter().filter(|o| o.status == OrderStatus::Executed || o.status == OrderStatus::Cancelled).count()).sum();
                if action_btn(ui, &format!("Place All ({})", draft_count), t.accent, draft_count > 0) {
                    for pane in panes.iter_mut() {
                        for o in &mut pane.orders { if o.status == OrderStatus::Draft { o.status = OrderStatus::Placed; } }
                    }
                }
                if action_btn(ui, "Cancel All", t.bear, active_count > 0) {
                    for pane in panes.iter_mut() {
                        for o in &mut pane.orders {
                            if o.status == OrderStatus::Draft || o.status == OrderStatus::Placed { o.status = OrderStatus::Cancelled; }
                        }
                    }
                }
                if action_btn(ui, "Clear", t.dim, history_count > 0) {
                    for pane in panes.iter_mut() {
                        pane.orders.retain(|o| o.status == OrderStatus::Draft || o.status == OrderStatus::Placed);
                    }
                }
            });
            ui.add_space(4.0);

            // ── Group selection bar ──
            let sel_count = watchlist.selected_order_ids.len();
            if sel_count > 0 {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    ui.label(egui::RichText::new(format!("{} selected", sel_count)).monospace().size(9.0).strong().color(t.accent));
                    action_btn(ui, "Place", t.accent, true).then(|| {
                        for (pi, oid) in &watchlist.selected_order_ids {
                            if let Some(o) = panes.get_mut(*pi).and_then(|p| p.orders.iter_mut().find(|o| o.id == *oid)) {
                                if o.status == OrderStatus::Draft { o.status = OrderStatus::Placed; }
                                if let Some(pid) = o.pair_id {
                                    if let Some(p) = panes.get_mut(*pi).and_then(|p| p.orders.iter_mut().find(|o| o.id == pid)) {
                                        if p.status == OrderStatus::Draft { p.status = OrderStatus::Placed; }
                                    }
                                }
                            }
                        }
                        watchlist.selected_order_ids.clear();
                    });
                    action_btn(ui, "Cancel", t.bear, true).then(|| {
                        for (pi, oid) in &watchlist.selected_order_ids {
                            if *pi < panes.len() { cancel_order_with_pair(&mut panes[*pi].orders, *oid); }
                        }
                        watchlist.selected_order_ids.clear();
                    });
                    if ui.add(egui::Button::new(egui::RichText::new("Deselect").monospace().size(8.0).color(t.dim)).frame(false)).clicked() {
                        watchlist.selected_order_ids.clear();
                    }
                });
                ui.add_space(4.0);
            }

            // ── Select all toggle ──
            {
                let active_orders: Vec<(usize, u32)> = panes.iter().enumerate()
                    .flat_map(|(pi, p)| p.orders.iter().filter(|o| o.status == OrderStatus::Draft || o.status == OrderStatus::Placed).map(move |o| (pi, o.id)))
                    .collect();
                let all_selected = !active_orders.is_empty() && active_orders.iter().all(|(pi, oid)| watchlist.selected_order_ids.iter().any(|(p, id)| p == pi && id == oid));
                if !active_orders.is_empty() {
                    ui.horizontal(|ui| {
                        let check_icon = if all_selected { Icon::CHECK_SQUARE } else { Icon::SQUARE_EMPTY };
                        let check_color = if all_selected { t.accent } else { t.dim.gamma_multiply(0.4) };
                        if ui.add(egui::Button::new(egui::RichText::new(check_icon).size(11.0).color(check_color))
                            .frame(false).min_size(egui::vec2(14.0, 14.0))).clicked() {
                            if all_selected {
                                watchlist.selected_order_ids.clear();
                            } else {
                                watchlist.selected_order_ids = active_orders;
                            }
                        }
                        ui.label(egui::RichText::new("Select all").monospace().size(9.0).color(t.dim.gamma_multiply(0.6)));
                    });
                    ui.add_space(2.0);
                }
            }

            // ── Order cards ──
            egui::ScrollArea::vertical().show(ui, |ui| {
                let mut cancel_order: Option<(usize, u32)> = None;
                let mut toggle_select: Option<(usize, u32)> = None;

                for (pi, pane) in panes.iter().enumerate() {
                    for order in &pane.orders {
                        let color = order.color(t.bull, t.bear);
                        let status_text = match order.status {
                            OrderStatus::Draft => "DRAFT", OrderStatus::Placed => "PLACED",
                            OrderStatus::Executed => "EXEC", OrderStatus::Cancelled => "CXL",
                        };
                        let status_color = match order.status {
                            OrderStatus::Draft => t.dim, OrderStatus::Placed => t.accent,
                            OrderStatus::Executed => t.bull, OrderStatus::Cancelled => t.bear,
                        };
                        let is_active = order.status == OrderStatus::Draft || order.status == OrderStatus::Placed;
                        let is_selected = watchlist.selected_order_ids.iter().any(|(p, id)| *p == pi && *id == order.id);
                        let card_bg = if is_selected { color_alpha(t.accent, 12) } else { color_alpha(t.toolbar_border, 15) };

                        let card_clicked = order_card(ui, color, card_bg, |ui| {
                            // Card header: checkbox + type + symbol + status + close
                            ui.horizontal(|ui| {
                                // Selection checkbox (visual only — click handled by card)
                                if is_active {
                                    let check_icon = if is_selected { Icon::CHECK_SQUARE } else { Icon::SQUARE_EMPTY };
                                    let check_color = if is_selected { t.accent } else { t.dim.gamma_multiply(0.4) };
                                    ui.label(egui::RichText::new(check_icon).size(11.0).color(check_color));
                                }
                                ui.label(egui::RichText::new(order.label()).monospace().size(10.0).strong().color(color));
                                ui.label(egui::RichText::new(format!("{} {}", &pane.symbol, &pane.timeframe))
                                    .monospace().size(9.0).color(egui::Color32::from_rgba_unmultiplied(200, 200, 210, 180)));
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if is_active {
                                        if ui.add(egui::Button::new(egui::RichText::new(Icon::X).size(9.0).color(t.dim.gamma_multiply(0.5)))
                                            .frame(false)).clicked() {
                                            cancel_order = Some((pi, order.id));
                                        }
                                    }
                                    status_badge(ui, status_text, status_color);
                                });
                            });

                            // Card body: price | qty | notional
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new(format!("{:.2}", order.price)).monospace().size(13.0).strong().color(color));
                                ui.add_space(6.0);
                                ui.label(egui::RichText::new(format!("\u{00D7}{}", order.qty)).monospace().size(10.0).color(t.dim.gamma_multiply(0.6)));
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    ui.label(egui::RichText::new(fmt_notional(order.notional())).monospace().size(9.0).color(t.dim.gamma_multiply(0.4)));
                                });
                            });
                        });

                        // Toggle selection on card click (for active orders)
                        if card_clicked && is_active {
                            toggle_select = Some((pi, order.id));
                        }
                    }
                }

                if let Some((pi, oid)) = cancel_order {
                    cancel_order_with_pair(&mut panes[pi].orders, oid);
                }
                if let Some((pi, oid)) = toggle_select {
                    let already = watchlist.selected_order_ids.iter().any(|(p, id)| *p == pi && *id == oid);
                    if already {
                        watchlist.selected_order_ids.retain(|(p, id)| !(*p == pi && *id == oid));
                    } else {
                        watchlist.selected_order_ids.push((pi, oid));
                    }
                }

                // Positions are now shown above orders via ApexIB live data

                // ── IB Order History ──
                let ib_orders = account_data_cached.as_ref().map(|(_, _, o)| o.clone()).unwrap_or_default();
                if !ib_orders.is_empty() {
                    ui.add_space(4.0);
                    separator(ui, color_alpha(t.toolbar_border, 40));
                    ui.add_space(4.0);
                    section_label(ui, "IB ORDERS", t.accent);
                    ui.add_space(4.0);
                    for o in &ib_orders {
                        let is_fill = o.status == "filled";
                        let is_cancel = o.status == "cancelled";
                        let side_color = if o.side == "BUY" { t.bull } else { t.bear };
                        let status_color = if is_fill { t.bull } else if is_cancel { t.dim.gamma_multiply(0.4) } else { t.accent };
                        let opt_label = if !o.option_type.is_empty() { format!(" {:.0}{}", o.strike, o.option_type) } else { String::new() };
                        order_card(ui, side_color, color_alpha(t.toolbar_border, if is_cancel { 5 } else { 10 }), |ui| {
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new(&o.side).monospace().size(9.0).strong().color(side_color));
                                ui.label(egui::RichText::new(format!("{}{}", o.symbol, opt_label)).monospace().size(10.0).strong()
                                    .color(egui::Color32::from_rgb(220, 220, 230)));
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    status_badge(ui, &o.status.to_uppercase(), status_color);
                                });
                            });
                            ui.horizontal(|ui| {
                                if o.avg_fill_price > 0.0 {
                                    ui.label(egui::RichText::new(format!("{:.2}", o.avg_fill_price)).monospace().size(11.0).strong().color(side_color));
                                } else if o.limit_price > 0.0 {
                                    ui.label(egui::RichText::new(format!("{:.2}", o.limit_price)).monospace().size(11.0).color(t.dim));
                                }
                                ui.label(egui::RichText::new(format!("\u{00D7}{}", o.qty)).monospace().size(9.0).color(t.dim.gamma_multiply(0.6)));
                                if o.filled_qty > 0 && o.filled_qty != o.qty {
                                    ui.label(egui::RichText::new(format!("filled {}", o.filled_qty)).monospace().size(8.0).color(t.dim.gamma_multiply(0.4)));
                                }
                                let notional = if o.avg_fill_price > 0.0 { o.avg_fill_price * o.qty as f64 } else { o.limit_price * o.qty as f64 };
                                if notional > 0.0 {
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        ui.label(egui::RichText::new(format!("${:.0}", notional)).monospace().size(8.0).color(t.dim.gamma_multiply(0.4)));
                                    });
                                }
                            });
                        });
                    }
                }

                // ── Alerts ──
                if !watchlist.alerts.is_empty() {
                    ui.add_space(4.0);
                    separator(ui, color_alpha(t.toolbar_border, 40));
                    ui.add_space(4.0);
                    section_label(ui, "ALERTS", t.dim);
                    ui.add_space(4.0);
                    let mut remove_alert: Option<u32> = None;
                    for alert in &watchlist.alerts {
                        let dir = if alert.above { "\u{2191}" } else { "\u{2193}" };
                        let alert_color = if alert.triggered { t.accent } else { t.dim };
                        order_card(ui, alert_color, color_alpha(t.toolbar_border, 10), |ui| {
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new(&alert.symbol).monospace().size(10.0).strong().color(egui::Color32::from_rgb(220,220,230)));
                                ui.label(egui::RichText::new(format!("{} {:.2}", dir, alert.price)).monospace().size(10.0).color(alert_color));
                                if alert.triggered {
                                    status_badge(ui, "TRIGGERED", t.accent);
                                }
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if ui.add(egui::Button::new(egui::RichText::new(Icon::X).size(9.0).color(t.dim.gamma_multiply(0.5))).frame(false)).clicked() {
                                        remove_alert = Some(alert.id);
                                    }
                                });
                            });
                            if !alert.message.is_empty() {
                                ui.label(egui::RichText::new(&alert.message).monospace().size(9.0).color(t.dim.gamma_multiply(0.6)));
                            }
                        });
                    }
                    if let Some(id) = remove_alert { watchlist.alerts.retain(|a| a.id != id); }
                }
            });
        });
}

// NOTE: Order execution is NOT simulated locally — fills come from the brokerage API.
// The chart only displays order levels; execution status changes are signaled externally.

// Update position current prices from chart data
for pos in &mut watchlist.positions {
    if let Some(pane) = panes.iter().find(|p| p.symbol == pos.symbol) {
        if let Some(bar) = pane.bars.last() {
            pos.current_price = bar.close;
        }
    }
}


}
