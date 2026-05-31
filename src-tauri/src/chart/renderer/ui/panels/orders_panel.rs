//! Orders / Positions / Alerts side panel.
//!
//! Migrated to the canonical side-panel design system: outer chrome via
//! [`SidePanelShell::tabs`], section structure via [`PanelSection`], and order
//! entries via [`PanelCard`] (with left accent stripe for Long/Short). The
//! in-flight order rows continue to use [`OrderRow`] (a specialized list row
//! widget with built-in cancel-X and selection treatment that `PanelListRow`
//! does not yet model — see the report).

use egui;
use super::super::style::*;
use super::super::super::gpu::*;
use super::super::widgets::rows::order_row::{OrderRow, OrderSideTag};
use crate::ui_kit::widgets::{Badge, Button, MetricRow, MetricTone, PanelCard, PanelEmpty, PanelSection, PanelTone, TagTone, Tooltip};
use crate::chart_renderer::ui::panels::side_panel_shell::{SidePanelShell, Width};
use crate::ui_kit::widgets::tokens::{Variant, Size};
use crate::ui_kit::widgets::icon_placement::IconPlacement;
use super::super::components::text::{self as wtext, MonospaceCode};
use crate::ui_kit::icons::Icon;
use crate::chart_renderer::commands::{self, AppCommand};
use crate::chart_renderer::trading::{AccountSummary, IbOrder, Position, OrderSide, OrderStatus};
use crate::chart_renderer::BookTab;

/// Rail registration — see [`super::right_rail`].
pub(crate) const RAIL: super::right_rail::RailPanelDef = super::right_rail::RailPanelDef {
    id: "orders",
    is_open: |w| w.orders_panel_open,
    render: |cx, slot| { draw(cx.ctx, cx.watchlist, cx.panes, cx.active_pane, cx.t, cx.account_data, Some(slot), None); },
};

fn book_tab_to_u8(t: BookTab) -> u8 { match t { BookTab::Book => 0, BookTab::Journal => 1 } }
fn book_tab_from_u8(v: u8) -> BookTab { match v { 1 => BookTab::Journal, _ => BookTab::Book } }

pub(crate) fn draw(
    ctx: &egui::Context,
    watchlist: &mut Watchlist,
    panes: &mut [Chart],
    _ap: usize,
    t: &Theme,
    account_data_cached: &Option<(AccountSummary, Vec<Position>, Vec<IbOrder>)>,
    slot: Option<super::side_panel_shell::RailSlot>,
    instance_tab: Option<&mut u8>,
) -> bool {
    let is_spawn = instance_tab.is_some();
    let mut spawn_close = false;
    if !is_spawn && !watchlist.orders_panel_open { return false; }

    let mut book_tab = match instance_tab.as_deref() {
        Some(v) => book_tab_from_u8(*v),
        None => watchlist.book_tab,
    };

    let tabs = [
        (BookTab::Book, "BOOK", None),
        (BookTab::Journal, "JOURNAL", None),
    ];

    // Pre-resolve pane metrics so the body closure can borrow `watchlist`
    // mutably without conflicting with `.pane_aligned(&watchlist)`.
    let pane_h = crate::chart_renderer::gpu::pane_tabs_header_h(watchlist);
    let pane_font = watchlist.pane_header_size.title_font();

    let shell_id = if is_spawn { "orders_inst" } else { "orders" };
    let resp = SidePanelShell::tabs(shell_id, &mut book_tab, &tabs)
        .width(Width::Medium)
        .pane_metrics(pane_h, pane_font)
        .rail_slot(slot)
        .on_tab_secondary(|ui, tab| {
            if crate::ui_kit::widgets::MenuItem::new("Open as new instance").show(ui, t).clicked() {
                super::right_rail::request_spawn("orders", book_tab_to_u8(tab));
                ui.close_menu();
            }
        })
        .show(ctx, t, |ui, t, tab| {
            match tab {
                BookTab::Journal => {
                    super::journal_panel::draw_content(ui, watchlist, t);
                }
                BookTab::Book => {
                    draw_book(ui, t, watchlist, panes, account_data_cached);
                }
            }
        });

    // Persist the active tab to its owner (instance store or base panel).
    if let Some(it) = instance_tab { *it = book_tab_to_u8(book_tab); }
    else { watchlist.book_tab = book_tab; }
    if resp.close_clicked {
        if is_spawn { spawn_close = true; }
        else { watchlist.update_sidebar_state(|s| s.orders_panel_open = false); }
    }

    // Update position current prices from chart data.
    for pos in &mut watchlist.positions {
        if let Some(pane) = panes.iter().find(|p| p.symbol == pos.symbol) {
            if let Some(bar) = pane.bars.last() {
                pos.current_price = bar.close;
            }
        }
    }
    spawn_close
}

// ── BOOK TAB BODY ──────────────────────────────────────────────────────────

fn draw_book(
    ui: &mut egui::Ui,
    t: &Theme,
    watchlist: &mut Watchlist,
    panes: &mut [Chart],
    account_data_cached: &Option<(AccountSummary, Vec<Position>, Vec<IbOrder>)>,
) {
    // ── POSITIONS ──────────────────────────────────────────────────────────
    {
        let (ib_positions, _ib_orders) = account_data_cached
            .as_ref()
            .map(|(_, p, o)| (p.clone(), o.clone()))
            .unwrap_or_default();
        let has_positions = !ib_positions.is_empty();
        let pos_count = ib_positions.len();

        let resp = PanelSection::new("POSITIONS")
            .count(pos_count)
            .title_color(t.accent)
            .action("Close All", PanelTone::Bear)
            .show(ui, t, |ui, t| {
                if has_positions {
                    let mut total_pnl: f64 = 0.0;
                    egui::ScrollArea::vertical()
                        .id_salt("positions_scroll")
                        .max_height(ui.available_height() * 0.45)
                        .show(ui, |ui| {
                            for pos in &ib_positions {
                                total_pnl += pos.unrealized_pnl;
                                let pnl_tone = if pos.unrealized_pnl >= 0.0 {
                                    PanelTone::Bull
                                } else {
                                    PanelTone::Bear
                                };
                                let pnl_color = pnl_tone.color(t);
                                PanelCard::new()
                                    .stripe(true)
                                    .tone(pnl_tone)
                                    .show(ui, t, |ui, t| {
                                        // Row 1: symbol, qty@price, close buttons
                                        ui.horizontal(|ui| {
                                            ui.add(MonospaceCode::new(&pos.symbol)
                                                .size_px(font_sm_tight())
                                                .strong(true)
                                                .color(t.text));
                                            ui.add(MonospaceCode::new(&format!(
                                                "{}@{:.2}", pos.qty, pos.avg_price))
                                                .size_px(font_sm_tight())
                                                .color(t.dim)
                                                .gamma(0.6));
                                            ui.with_layout(
                                                egui::Layout::right_to_left(
                                                    egui::Align::Center),
                                                |ui| {
                                                    let r = ui.add(Button::icon(Icon::X)
                                                        .variant(Variant::InlineClose)
                                                        .size(Size::Xs)
                                                        .placement(IconPlacement::ListRow)
                                                        .tone_destructive());
                                                    Tooltip::new("Close position").show(ui, &r, t);
                                                    if r.clicked() {
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
                                                    if pos.qty.abs() > 1 {
                                                        if ui.add(Button::new("\u{00BD}")
                                                            .variant(Variant::Ghost)
                                                            .size(Size::Xs)).clicked() {
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
                                            ui.add(MonospaceCode::new(&format!(
                                                "{:+.2}", pos.unrealized_pnl))
                                                .size_px(font_md())
                                                .strong(true)
                                                .color(pnl_color));
                                            ui.add_space(gap_xs());
                                            ui.add(MonospaceCode::new(&format!(
                                                "({:+.1}%)", pos.pnl_pct()))
                                                .size_px(font_sm_tight())
                                                .color(pnl_color));
                                            ui.with_layout(
                                                egui::Layout::right_to_left(
                                                    egui::Align::Center),
                                                |ui| {
                                                    ui.add(MonospaceCode::new(&format!(
                                                        "${:.0}", pos.market_value))
                                                        .size_px(font_xs())
                                                        .color(t.dim)
                                                        .gamma(0.4));
                                                });
                                        });
                                    });
                            }
                        });
                    let total_tone = if total_pnl >= 0.0 {
                        MetricTone::Bull
                    } else {
                        MetricTone::Bear
                    };
                    ui.add_space(gap_xs());
                    MetricRow::new("Total P&L")
                        .value(format!("{:+.2}", total_pnl))
                        .tone(total_tone)
                        .show(ui, t);
                } else {
                    PanelEmpty::new("No open positions").show(ui, t);
                }
            });

        if resp.action_clicked && has_positions {
            std::thread::spawn(|| {
                let _ = reqwest::blocking::Client::new()
                    .post(format!("{}/risk/flatten", APEXIB_URL))
                    .timeout(std::time::Duration::from_secs(5))
                    .send();
            });
        }
    }

    // ── WORKING ORDERS ─────────────────────────────────────────────────────
    let draft_count: usize = panes.iter()
        .map(|p| p.orders.iter().filter(|o| o.status == OrderStatus::Draft).count())
        .sum();
    let active_count: usize = panes.iter()
        .map(|p| p.orders.iter()
            .filter(|o| o.status == OrderStatus::Draft || o.status == OrderStatus::Placed)
            .count())
        .sum();
    let history_count: usize = panes.iter()
        .map(|p| p.orders.iter()
            .filter(|o| o.status == OrderStatus::Executed || o.status == OrderStatus::Cancelled)
            .count())
        .sum();

    PanelSection::new("ORDERS")
        .count(active_count)
        .title_color(t.accent)
        .show(ui, t, |ui, t| {
            // Drafts / active badges row (preserves "{n}d {n}a" semantics).
            if active_count > 0 || draft_count > 0 {
                ui.horizontal(|ui| {
                    if draft_count > 0 {
                        Badge::count(draft_count as u32).tone(TagTone::Warn).show(ui, t);
                        ui.add(MonospaceCode::new("d")
                            .size_px(font_xs())
                            .color(t.dim)
                            .gamma(0.5));
                    }
                    let active_only = active_count.saturating_sub(draft_count);
                    if active_only > 0 {
                        if draft_count > 0 { ui.add_space(gap_2xs()); }
                        Badge::count(active_only as u32).tone(TagTone::Accent).show(ui, t);
                        ui.add(MonospaceCode::new("a")
                            .size_px(font_xs())
                            .color(t.dim)
                            .gamma(0.5));
                    }
                });
                ui.add_space(gap_xs());
            }

            // Action bar: Place All / Cancel All / Clear / Spread.
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                let place_all_label = format!("Place All ({})", draft_count);
                if Button::action(place_all_label.as_str())
                    .tint(t.accent)
                    .disabled(draft_count == 0)
                    .show(ui, t).clicked()
                {
                    commands::push(AppCommand::PlaceAllDraftOrders);
                }
                if Button::action("Cancel All")
                    .tint(t.bear)
                    .disabled(active_count == 0)
                    .show(ui, t).clicked()
                {
                    commands::push(AppCommand::CancelAllOrders);
                }
                if Button::action("Clear")
                    .tint(t.dim)
                    .disabled(history_count == 0)
                    .show(ui, t).clicked()
                {
                    commands::push(AppCommand::ClearOrderHistory);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if Button::small_action("Spread").tint(t.dim).show(ui, t).clicked() {
                        watchlist.update_sidebar_state(|s| s.spread_open = !s.spread_open);
                    }
                });
            });
            ui.add_space(gap_xs());

            // Group selection bar.
            let sel_count = watchlist.selected_order_ids.len();
            if sel_count > 0 {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    ui.add(MonospaceCode::new(&format!("{} selected", sel_count))
                        .size_px(font_sm_tight())
                        .strong(true)
                        .color(t.accent));
                    if Button::action("Place").tint(t.accent).show(ui, t).clicked() {
                        commands::push(AppCommand::PlaceSelectedOrders);
                    }
                    if Button::action("Cancel").tint(t.bear).show(ui, t).clicked() {
                        commands::push(AppCommand::CancelSelectedOrders);
                    }
                    if Button::small_action("Deselect").tint(t.dim).show(ui, t).clicked() {
                        watchlist.selected_order_ids.clear();
                    }
                });
                ui.add_space(gap_xs());
            }

            // Select-all toggle.
            {
                let active_orders: Vec<(usize, u32)> = panes.iter().enumerate()
                    .flat_map(|(pi, p)| p.orders.iter()
                        .filter(|o| o.status == OrderStatus::Draft || o.status == OrderStatus::Placed)
                        .map(move |o| (pi, o.id)))
                    .collect();
                let all_selected = !active_orders.is_empty()
                    && active_orders.iter().all(|(pi, oid)|
                        watchlist.selected_order_ids.iter().any(|(p, id)| p == pi && id == oid));
                if !active_orders.is_empty() {
                    ui.horizontal(|ui| {
                        let check_icon = if all_selected { Icon::CHECK_SQUARE } else { Icon::SQUARE_EMPTY };
                        let r = ui.add(Button::icon(check_icon)
                            .variant(Variant::MutedIcon)
                            .active(all_selected)
                            .size(Size::Xs)
                            .placement(IconPlacement::ListRow));
                        Tooltip::new("Select all").show(ui, &r, t);
                        if r.clicked() {
                            if all_selected {
                                watchlist.selected_order_ids.clear();
                            } else {
                                watchlist.selected_order_ids = active_orders;
                            }
                        }
                        ui.add(MonospaceCode::new("Select all")
                            .size_px(font_sm_tight())
                            .color(t.dim)
                            .gamma(0.6));
                    });
                    ui.add_space(gap_xs());
                }
            }

            // ── Order rows + history list ──
            egui::ScrollArea::vertical().show(ui, |ui| {
                let mut toggle_select: Option<(usize, u32)> = None;
                let mut any_rows = false;

                for (pi, pane) in panes.iter().enumerate() {
                    for order in &pane.orders {
                        any_rows = true;
                        let status_text = match order.status {
                            OrderStatus::Draft => "DRAFT",
                            OrderStatus::Placed => "PLACED",
                            OrderStatus::Executed => "EXEC",
                            OrderStatus::Cancelled => "CXL",
                        };
                        let is_active = order.status == OrderStatus::Draft
                            || order.status == OrderStatus::Placed;
                        let is_selected = watchlist.selected_order_ids.iter()
                            .any(|(p, id)| *p == pi && *id == order.id);
                        let side_tag = match order.side {
                            OrderSide::Buy | OrderSide::TriggerBuy | OrderSide::OcoTarget
                                => OrderSideTag::Buy,
                            OrderSide::Sell | OrderSide::Stop
                                | OrderSide::OcoStop | OrderSide::TriggerSell
                                => OrderSideTag::Sell,
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

                if !any_rows {
                    PanelEmpty::new("No working orders").show(ui, t);
                }

                if let Some((pi, oid)) = toggle_select {
                    let already = watchlist.selected_order_ids.iter()
                        .any(|(p, id)| *p == pi && *id == oid);
                    if already {
                        watchlist.selected_order_ids
                            .retain(|(p, id)| !(*p == pi && *id == oid));
                    } else {
                        watchlist.selected_order_ids.push((pi, oid));
                    }
                }
            });
        });

    // ── IB ORDER HISTORY ───────────────────────────────────────────────────
    let ib_orders = account_data_cached
        .as_ref()
        .map(|(_, _, o)| o.clone())
        .unwrap_or_default();
    if !ib_orders.is_empty() {
        PanelSection::new("IB ORDERS")
            .count(ib_orders.len())
            .title_color(t.accent)
            .show(ui, t, |ui, t| {
                for o in &ib_orders {
                    let is_fill = o.status == "filled";
                    let is_cancel = o.status == "cancelled";
                    let side_tone = if o.side == "BUY" { PanelTone::Bull } else { PanelTone::Bear };
                    let side_color = side_tone.color(t);
                    let status_color = if is_fill { t.bull }
                        else if is_cancel { color_dim(t.dim) }
                        else { t.accent };
                    let opt_label = if !o.option_type.is_empty() {
                        format!(" {:.0}{}", o.strike, o.option_type)
                    } else { String::new() };

                    PanelCard::new()
                        .stripe(true)
                        .tone(side_tone)
                        .show(ui, t, |ui, t| {
                            ui.horizontal(|ui| {
                                ui.add(MonospaceCode::new(&o.side)
                                    .size_px(font_sm_tight())
                                    .strong(true)
                                    .color(side_color));
                                ui.add(MonospaceCode::new(&format!("{}{}", o.symbol, opt_label))
                                    .size_px(font_sm_tight())
                                    .strong(true)
                                    .color(t.text));
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        status_badge(ui, &o.status.to_uppercase(), status_color);
                                    });
                            });
                            ui.horizontal(|ui| {
                                if o.avg_fill_price > 0.0 {
                                    ui.add(MonospaceCode::new(&format!("{:.2}", o.avg_fill_price))
                                        .size_px(font_sm())
                                        .strong(true)
                                        .color(side_color));
                                } else if o.limit_price > 0.0 {
                                    ui.add(MonospaceCode::new(&format!("{:.2}", o.limit_price))
                                        .size_px(font_sm())
                                        .color(t.dim));
                                }
                                ui.add(MonospaceCode::new(&format!("\u{00D7}{}", o.qty))
                                    .size_px(font_sm_tight())
                                    .color(t.dim)
                                    .gamma(0.6));
                                if o.filled_qty > 0 && o.filled_qty != o.qty {
                                    ui.add(MonospaceCode::new(&format!("filled {}", o.filled_qty))
                                        .size_px(font_xs())
                                        .color(t.dim)
                                        .gamma(0.4));
                                }
                                let notional = if o.avg_fill_price > 0.0 {
                                    o.avg_fill_price * o.qty as f64
                                } else {
                                    o.limit_price * o.qty as f64
                                };
                                if notional > 0.0 {
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            ui.add(MonospaceCode::new(&format!("${:.0}", notional))
                                                .size_px(font_xs())
                                                .color(t.dim)
                                                .gamma(0.4));
                                        });
                                }
                            });
                        });
                }
            });
    }

    // ── ALERTS ─────────────────────────────────────────────────────────────
    if !watchlist.alerts.is_empty() {
        let mut remove_alert: Option<u32> = None;
        let alert_count = watchlist.alerts.len();
        PanelSection::new("ALERTS")
            .count(alert_count)
            .show(ui, t, |ui, t| {
                for alert in &watchlist.alerts {
                    let dir = if alert.above { "\u{2191}" } else { "\u{2193}" };
                    let alert_tone = if alert.triggered { PanelTone::Accent } else { PanelTone::Default };
                    let alert_color = alert_tone.color(t);
                    PanelCard::new()
                        .stripe(true)
                        .tone(alert_tone)
                        .show(ui, t, |ui, t| {
                            ui.horizontal(|ui| {
                                ui.add(MonospaceCode::new(&alert.symbol)
                                    .size_px(font_sm_tight())
                                    .strong(true)
                                    .color(t.text));
                                ui.add(MonospaceCode::new(&format!("{} {:.2}", dir, alert.price))
                                    .size_px(font_sm_tight())
                                    .color(alert_color));
                                if alert.triggered {
                                    status_badge(ui, "TRIGGERED", t.accent);
                                }
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        let r = ui.add(Button::icon(Icon::X)
                                            .variant(Variant::InlineClose)
                                            .size(Size::Xs)
                                            .placement(IconPlacement::ListRow));
                                        Tooltip::new("Remove alert").show(ui, &r, t);
                                        if r.clicked() {
                                            remove_alert = Some(alert.id);
                                        }
                                    });
                            });
                            if !alert.message.is_empty() {
                                ui.add(MonospaceCode::new(&alert.message)
                                    .size_px(font_sm_tight())
                                    .color(t.dim)
                                    .gamma(0.6));
                            }
                        });
                }
            });
        if let Some(id) = remove_alert {
            watchlist.update_alerts_state(|s| s.alerts.retain(|a| a.id != id));
        }
    }

    // Reference to silence "unused import" warning when not all code paths fire.
    let _ = wtext::section_label;
}

// NOTE: Order execution is NOT simulated locally — fills come from the brokerage API.
// The chart only displays order levels; execution status changes are signaled externally.
