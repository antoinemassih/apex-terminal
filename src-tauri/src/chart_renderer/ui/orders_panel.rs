//! Orders / Positions / Alerts side panel.

use egui;
use super::style::*;
use super::super::gpu::*;
use super::widgets::frames::PanelFrame;
use super::widgets::buttons::ChromeBtn;
use super::widgets::rows::order_row::{OrderRow, OrderSideTag};
use super::widgets::tabs::TabBar;
use super::widgets::text::{self as wtext, MonospaceCode};
use crate::ui_kit::icons::Icon;
use crate::chart_renderer::commands::{self, AppCommand};
use crate::chart_renderer::trading::{AccountSummary, IbOrder, Position, OrderSide, OrderStatus};
use crate::chart_renderer::BookTab;

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
        .default_width(250.0)
        .min_width(200.0)
        .max_width(330.0)
        .frame(PanelFrame::new(t.toolbar_bg, t.toolbar_border).build())
        .show(ctx, |ui| {
            // Tab bar with close button
            let tab_row = ui.horizontal(|ui| {
                ui.set_min_height(24.0);
                TabBar::new(&mut watchlist.book_tab, &[
                    (BookTab::Book, "Book"),
                    (BookTab::Journal, "Journal"),
                ]).accent(t.accent).dim(t.dim).show(ui);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if close_button(ui, t.dim) { watchlist.orders_panel_open = false; }
                });
            });
            let line_y = tab_row.response.rect.max.y;
            ui.painter().line_segment(
                [egui::pos2(ui.min_rect().left(), line_y),
                 egui::pos2(ui.min_rect().right(), line_y)],
                egui::Stroke::new(1.0, color_alpha(t.toolbar_border, ALPHA_MUTED)));
            ui.add_space(GAP_SM);

            if watchlist.book_tab == BookTab::Journal {
                super::journal_panel::draw_content(ui, watchlist, t);
                return;
            }

            // ══════════════════════════════════════════════════════
            // ── POSITIONS SECTION (top half of book) ──
            // ══════════════════════════════════════════════════════
            {
                let (ib_positions, ib_orders) = account_data_cached.as_ref().map(|(_, p, o)| (p.clone(), o.clone())).unwrap_or_default();
                let has_positions = !ib_positions.is_empty();

                // Header + Close All
                ui.horizontal(|ui| {
                    wtext::section_label(ui, "POSITIONS", t.accent);
                    if has_positions {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if small_action_btn(ui, "Close All", t.bear) {
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
                            order_card(ui, pnl_color, color_alpha(t.toolbar_border, ALPHA_FAINT), |ui| {
                                // Row 1: symbol, qty@price, close buttons
                                ui.horizontal(|ui| {
                                    ui.add(MonospaceCode::new(&pos.symbol).size_px(9.0).strong(true).color(TEXT_PRIMARY));
                                    ui.add(MonospaceCode::new(&format!("{}@{:.2}", pos.qty, pos.avg_price)).size_px(9.0).color(t.dim).gamma(0.6));
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        // Close button
                                        let close_color = t.bear;
                                        if ui.add(ChromeBtn::new(egui::RichText::new(Icon::X).size(9.0).color(close_color))
                                            .fill(color_alpha(close_color, 12)).corner_radius(r_sm_cr())
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
                                            if ui.add(ChromeBtn::new(egui::RichText::new("\u{00BD}").size(9.0).color(t.dim))
                                                .fill(color_alpha(t.toolbar_border, ALPHA_GHOST)).corner_radius(r_sm_cr())
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
                                    ui.add(MonospaceCode::new(&format!("{:+.2}", pos.unrealized_pnl)).size_px(11.0).strong(true).color(pnl_color));
                                    ui.add_space(4.0);
                                    ui.add(MonospaceCode::new(&format!("({:+.1}%)", pos.pnl_pct())).size_px(9.0).color(pnl_color));
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        ui.add(MonospaceCode::new(&format!("${:.0}", pos.market_value)).size_px(8.0).color(t.dim).gamma(0.4));
                                    });
                                });
                            });
                        }
                    });
                    // Total P&L row
                    let total_color = if total_pnl >= 0.0 { t.bull } else { t.bear };
                    ui.add_space(2.0);
                    ui.horizontal(|ui| {
                        ui.add(MonospaceCode::new("Total P&L").size_px(9.0).color(t.dim));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.add(MonospaceCode::new(&format!("{:+.2}", total_pnl)).size_px(10.0).strong(true).color(total_color));
                        });
                    });
                } else {
                    ui.add_space(6.0);
                    ui.add(MonospaceCode::new("No open positions").size_px(9.0).color(t.dim).gamma(0.4));
                    ui.add_space(6.0);
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
                    0.0, color_alpha(t.toolbar_border, ALPHA_HEAVY));
                ui.add_space(6.0);
            }

            // ══════════════════════════════════════════════════════
            // ── ORDERS SECTION (bottom half of book) ──
            // ══════════════════════════════════════════════════════

            // Orders header + action bar
            ui.horizontal(|ui| {
                wtext::section_label(ui, "ORDERS", t.accent);
                let draft_count: usize = panes.iter().map(|p| p.orders.iter().filter(|o| o.status == OrderStatus::Draft).count()).sum();
                let active_count: usize = panes.iter().map(|p| p.orders.iter().filter(|o| o.status == OrderStatus::Draft || o.status == OrderStatus::Placed).count()).sum();
                if active_count > 0 || draft_count > 0 {
                    ui.add_space(4.0);
                    ui.add(MonospaceCode::new(&format!("{}d {}a", draft_count, active_count - draft_count)).size_px(8.0).color(t.dim).gamma(0.5));
                }
            });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                let draft_count: usize = panes.iter().map(|p| p.orders.iter().filter(|o| o.status == OrderStatus::Draft).count()).sum();
                let active_count: usize = panes.iter().map(|p| p.orders.iter().filter(|o| o.status == OrderStatus::Draft || o.status == OrderStatus::Placed).count()).sum();
                let history_count: usize = panes.iter().map(|p| p.orders.iter().filter(|o| o.status == OrderStatus::Executed || o.status == OrderStatus::Cancelled).count()).sum();
                if action_btn(ui, &format!("Place All ({})", draft_count), t.accent, draft_count > 0) {
                    commands::push(AppCommand::PlaceAllDraftOrders);
                }
                if action_btn(ui, "Cancel All", t.bear, active_count > 0) {
                    commands::push(AppCommand::CancelAllOrders);
                }
                if action_btn(ui, "Clear", t.dim, history_count > 0) {
                    commands::push(AppCommand::ClearOrderHistory);
                }
                // Spread Builder shortcut
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if small_action_btn(ui, "Spread", t.dim) {
                        watchlist.spread_open = !watchlist.spread_open;
                    }
                });
            });
            ui.add_space(4.0);

            // ── Group selection bar ──
            let sel_count = watchlist.selected_order_ids.len();
            if sel_count > 0 {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    ui.add(MonospaceCode::new(&format!("{} selected", sel_count)).size_px(9.0).strong(true).color(t.accent));
                    action_btn(ui, "Place", t.accent, true).then(|| {
                        commands::push(AppCommand::PlaceSelectedOrders);
                    });
                    action_btn(ui, "Cancel", t.bear, true).then(|| {
                        commands::push(AppCommand::CancelSelectedOrders);
                    });
                    if icon_btn(ui, "Deselect", t.dim, 8.0).clicked() {
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
                        if ui.add(ChromeBtn::new(egui::RichText::new(check_icon).size(10.0).color(check_color))
                            .frameless(true).min_size(egui::vec2(14.0, 14.0))).clicked() {
                            if all_selected {
                                watchlist.selected_order_ids.clear();
                            } else {
                                watchlist.selected_order_ids = active_orders;
                            }
                        }
                        ui.add(MonospaceCode::new("Select all").size_px(9.0).color(t.dim).gamma(0.6));
                    });
                    ui.add_space(2.0);
                }
            }

            // ── Order cards ──
            egui::ScrollArea::vertical().show(ui, |ui| {
                let mut toggle_select: Option<(usize, u32)> = None;

                for (pi, pane) in panes.iter().enumerate() {
                    for order in &pane.orders {
                        let status_text = match order.status {
                            OrderStatus::Draft => "DRAFT", OrderStatus::Placed => "PLACED",
                            OrderStatus::Executed => "EXEC", OrderStatus::Cancelled => "CXL",
                        };
                        let is_active = order.status == OrderStatus::Draft || order.status == OrderStatus::Placed;
                        let is_selected = watchlist.selected_order_ids.iter().any(|(p, id)| *p == pi && *id == order.id);
                        let side_tag = match order.side {
                            OrderSide::Buy | OrderSide::TriggerBuy | OrderSide::OcoTarget => OrderSideTag::Buy,
                            OrderSide::Sell | OrderSide::Stop | OrderSide::OcoStop | OrderSide::TriggerSell => OrderSideTag::Sell,
                        };
                        let symbol_label = format!("{} {}", &pane.symbol, &pane.timeframe);

                        let (resp, cancel_clicked) = OrderRow::new(
                            side_tag,
                            &symbol_label,
                            order.qty as i64,
                            order.price,
                            status_text,
                        )
                            .selected(is_selected)
                            .show_cancel(is_active)
                            .theme(t)
                            .show(ui);

                        if cancel_clicked && is_active {
                            commands::push(AppCommand::CancelOrder { pane: pi, id: order.id });
                        }
                        if resp.clicked() && is_active {
                            toggle_select = Some((pi, order.id));
                        }
                    }
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
                    separator(ui, color_alpha(t.toolbar_border, ALPHA_MUTED));
                    ui.add_space(4.0);
                    wtext::section_label(ui, "IB ORDERS", t.accent);
                    ui.add_space(4.0);
                    for o in &ib_orders {
                        let is_fill = o.status == "filled";
                        let is_cancel = o.status == "cancelled";
                        let side_color = if o.side == "BUY" { t.bull } else { t.bear };
                        let status_color = if is_fill { t.bull } else if is_cancel { t.dim.gamma_multiply(0.4) } else { t.accent };
                        let opt_label = if !o.option_type.is_empty() { format!(" {:.0}{}", o.strike, o.option_type) } else { String::new() };
                        order_card(ui, side_color, color_alpha(t.toolbar_border, if is_cancel { 5 } else { 10 }), |ui| {
                            ui.horizontal(|ui| {
                                ui.add(MonospaceCode::new(&o.side).size_px(9.0).strong(true).color(side_color));
                                ui.add(MonospaceCode::new(&format!("{}{}", o.symbol, opt_label)).size_px(9.0).strong(true).color(TEXT_PRIMARY));
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    status_badge(ui, &o.status.to_uppercase(), status_color);
                                });
                            });
                            ui.horizontal(|ui| {
                                if o.avg_fill_price > 0.0 {
                                    ui.add(MonospaceCode::new(&format!("{:.2}", o.avg_fill_price)).size_px(10.0).strong(true).color(side_color));
                                } else if o.limit_price > 0.0 {
                                    ui.add(MonospaceCode::new(&format!("{:.2}", o.limit_price)).size_px(10.0).color(t.dim));
                                }
                                ui.add(MonospaceCode::new(&format!("\u{00D7}{}", o.qty)).size_px(9.0).color(t.dim).gamma(0.6));
                                if o.filled_qty > 0 && o.filled_qty != o.qty {
                                    ui.add(MonospaceCode::new(&format!("filled {}", o.filled_qty)).size_px(8.0).color(t.dim).gamma(0.4));
                                }
                                let notional = if o.avg_fill_price > 0.0 { o.avg_fill_price * o.qty as f64 } else { o.limit_price * o.qty as f64 };
                                if notional > 0.0 {
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        ui.add(MonospaceCode::new(&format!("${:.0}", notional)).size_px(8.0).color(t.dim).gamma(0.4));
                                    });
                                }
                            });
                        });
                    }
                }

                // ── Alerts ──
                if !watchlist.alerts.is_empty() {
                    ui.add_space(4.0);
                    separator(ui, color_alpha(t.toolbar_border, ALPHA_MUTED));
                    ui.add_space(4.0);
                    wtext::section_label(ui, "ALERTS", t.dim);
                    ui.add_space(4.0);
                    let mut remove_alert: Option<u32> = None;
                    for alert in &watchlist.alerts {
                        let dir = if alert.above { "\u{2191}" } else { "\u{2193}" };
                        let alert_color = if alert.triggered { t.accent } else { t.dim };
                        order_card(ui, alert_color, color_alpha(t.toolbar_border, ALPHA_FAINT), |ui| {
                            ui.horizontal(|ui| {
                                ui.add(MonospaceCode::new(&alert.symbol).size_px(9.0).strong(true).color(TEXT_PRIMARY));
                                ui.add(MonospaceCode::new(&format!("{} {:.2}", dir, alert.price)).size_px(9.0).color(alert_color));
                                if alert.triggered {
                                    status_badge(ui, "TRIGGERED", t.accent);
                                }
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if icon_btn(ui, Icon::X, t.dim.gamma_multiply(0.5), FONT_MD).clicked() {
                                        remove_alert = Some(alert.id);
                                    }
                                });
                            });
                            if !alert.message.is_empty() {
                                ui.add(MonospaceCode::new(&alert.message).size_px(9.0).color(t.dim).gamma(0.6));
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
