//! Floating order entry panel (bottom-left of chart pane).
//!
//! Includes pill (collapsed) mode, draggable header, DOM ladder, and order body.

use egui::Context;
use crate::chart_renderer::gpu::{Theme, Chart, render_order_entry_body};
use crate::chart_renderer::trading::{OrderSide, OrderLevel, OrderStatus, AccountSummary, Position, IbOrder};
use crate::chart_renderer::trading::order_manager::{OrderIntent, ManagedOrderType, OrderSource};
use crate::chart_renderer::gpu::Watchlist;
use crate::chart_renderer::ui::style::{color_alpha, gap_xs, gap_sm, gap_md, gap_lg, gap_2xl, font_xs, font_sm, font_md};
use crate::chart_renderer::ui::widgets::frames::PopupFrame;
use crate::ui_kit::icons::Icon;

/// Layout parameters passed in from gpu.rs.
pub struct OrderEntryPanelCtx<'a> {
    pub ctx: &'a Context,
    pub t: &'a Theme,
    pub chart: &'a mut Chart,
    pub watchlist: &'a Watchlist,
    pub account_data_cached: &'a Option<(AccountSummary, Vec<Position>, Vec<IbOrder>)>,
    /// Absolute pixel position (already computed from chart rect + panel_pos).
    pub abs_pos: egui::Pos2,
    /// pane index for unique egui Ids.
    pub pane_idx: usize,
    /// Chart rect bounds for drag clamping.
    pub cw: f32,
    pub ch: f32,
}

pub fn show_order_entry_panel(c: OrderEntryPanelCtx<'_>) {
    let chart = c.chart;
    let adv = chart.order_advanced;
    let panel_w = if adv { 270.0 } else { 210.0 };
    let abs_pos = c.abs_pos;

    // ── Collapsed pill ──
    if chart.order_collapsed {
        let pill_w = 90.0;
        egui::Window::new(format!("order_pill_{}", c.pane_idx))
            .fixed_pos(abs_pos)
            .fixed_size(egui::vec2(pill_w, 24.0))
            .title_bar(false)
            .frame(PopupFrame::new()
                .colors(color_alpha(c.t.toolbar_bg, 235), color_alpha(c.t.toolbar_border, 100))
                .ctx(c.ctx)
                .inner_margin(egui::Margin { left: gap_lg() as i8, right: gap_lg() as i8, top: gap_sm() as i8, bottom: gap_sm() as i8 })
                .corner_radius(12.0)
                .build())
            .show(c.ctx, |ui| {
                let resp = ui.horizontal(|ui| {
                    let armed_dot = if chart.armed { c.t.accent } else { c.t.dim.gamma_multiply(0.3) };
                    ui.painter().circle_filled(egui::pos2(ui.cursor().min.x + 5.0, ui.cursor().min.y + 8.0), 3.5, armed_dot);
                    ui.add_space(gap_2xl());
                    ui.label(egui::RichText::new("ORDER").monospace().size(font_sm()).strong().color(c.t.dim.gamma_multiply(0.7)));
                });
                let pill_resp = ui.interact(resp.response.rect, egui::Id::new(("order_pill_interact", c.pane_idx)), egui::Sense::click_and_drag());
                if pill_resp.double_clicked() { chart.order_collapsed = false; }
                if pill_resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
                if pill_resp.dragged() {
                    let delta = pill_resp.drag_delta();
                    chart.order_panel_pos.x += delta.x;
                    chart.order_panel_pos.y += delta.y;
                }
            });
        return;
    }

    // ── Expanded panel ──
    egui::Window::new(format!("order_entry_{}", c.pane_idx))
        .fixed_pos(abs_pos)
        .fixed_size(egui::vec2(panel_w, 0.0))
        .title_bar(false)
        // TODO(ui-kit): expanded panel uses corner_radius(4.0) and zero margin — can't use PopupFrame without changing visuals.
        .frame(egui::Frame::popup(&c.ctx.style())
            .fill(c.t.toolbar_bg)
            .inner_margin(egui::Margin { left: 0, right: 0, top: 0, bottom: 0 })
            .stroke(egui::Stroke::new(1.0, color_alpha(c.t.toolbar_border, 100)))
            .corner_radius(4.0))
        .show(c.ctx, |ui| {
            // ── Header bar ──
            let header_resp = ui.horizontal(|ui| {
                ui.set_min_width(panel_w);
                let hr = ui.max_rect();
                ui.painter().rect_filled(
                    egui::Rect::from_min_size(hr.min, egui::vec2(panel_w, 22.0)),
                    egui::CornerRadius { nw: 4, ne: 4, sw: 0, se: 0 },
                    color_alpha(c.t.toolbar_border, 30));
                ui.add_space(gap_sm());
                let armed_icon = if chart.armed { Icon::SHIELD_WARNING } else { Icon::PLAY };
                let armed_color = if chart.armed { c.t.accent } else { c.t.dim.gamma_multiply(0.4) };
                ui.label(egui::RichText::new(armed_icon).size(font_md()).color(armed_color));
                ui.label(egui::RichText::new("ORDER").monospace().size(font_sm()).strong().color(c.t.dim.gamma_multiply(0.6)));
                if let Some((_, ref positions, _)) = c.account_data_cached {
                    if let Some(pos) = positions.iter().find(|p| p.symbol == chart.symbol) {
                        let pos_color = if pos.qty > 0 { c.t.bull } else { c.t.bear };
                        let pos_text = if pos.qty > 0 { format!("+{}", pos.qty) } else { format!("{}", pos.qty) };
                        ui.label(egui::RichText::new(pos_text).monospace().size(font_sm()).strong().color(pos_color));
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(gap_sm());
                    let exp_icon = if adv { Icon::MINUS } else { Icon::PLUS };
                    ui.label(egui::RichText::new(exp_icon).size(font_sm()).color(c.t.dim.gamma_multiply(0.5)));
                    ui.add(egui::Separator::default().spacing(gap_xs()));
                    let dom_col = if chart.dom_open { c.t.accent } else { c.t.dim.gamma_multiply(0.4) };
                    ui.label(egui::RichText::new("DOM").monospace().size(font_xs()).color(dom_col));
                });
            });
            let hdr_rect = header_resp.response.rect;
            if let Some(mpos) = ui.input(|i| i.pointer.latest_pos()) {
                if hdr_rect.contains(mpos) {
                    let released = ui.input(|i| i.pointer.button_released(egui::PointerButton::Primary));
                    let dbl = ui.input(|i| i.pointer.button_double_clicked(egui::PointerButton::Primary));
                    let armed_rect = egui::Rect::from_min_size(hdr_rect.min, egui::vec2(22.0, 22.0));
                    if armed_rect.contains(mpos) {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        if released { chart.armed = !chart.armed; }
                    } else if mpos.x > hdr_rect.right() - 50.0 && mpos.x < hdr_rect.right() - 20.0 {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        if released { chart.dom_open = !chart.dom_open; }
                    } else if mpos.x > hdr_rect.right() - 20.0 {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        if released { chart.order_advanced = !chart.order_advanced; }
                    } else if dbl {
                        chart.order_collapsed = true;
                    } else {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
                    }
                }
            }
            let drag_resp = ui.interact(hdr_rect, egui::Id::new(("order_panel_drag", c.pane_idx)), egui::Sense::drag());
            if drag_resp.dragged() {
                let delta = drag_resp.drag_delta();
                chart.order_panel_pos.x += delta.x;
                chart.order_panel_pos.y += delta.y;
                chart.order_panel_pos.x = chart.order_panel_pos.x.clamp(0.0, (c.cw - panel_w).max(0.0));
                if chart.order_panel_pos.y < 0.0 {
                    chart.order_panel_pos.y = chart.order_panel_pos.y.clamp(-(c.ch - 30.0), -30.0);
                } else {
                    chart.order_panel_pos.y = chart.order_panel_pos.y.clamp(0.0, (c.ch - 30.0).max(0.0));
                }
            }

            // ── DOM ladder (when open) ──
            if chart.dom_open {
                render_dom_ladder(ui, c.t, chart, c.account_data_cached, panel_w);
            }

            // ── Order body ──
            render_order_entry_body(ui, chart, c.t, c.pane_idx as u64, panel_w);
        });
}

fn render_dom_ladder(
    ui: &mut egui::Ui,
    t: &Theme,
    chart: &mut Chart,
    account_data_cached: &Option<(AccountSummary, Vec<Position>, Vec<IbOrder>)>,
    panel_w: f32,
) {
    use crate::chart_renderer::ui::style::color_alpha;
    use crate::chart_renderer::ui::style::COLOR_AMBER;

    let live_q = if chart.is_option && !chart.option_contract.is_empty() {
        crate::apex_data::live_state::get_quote(&chart.option_contract)
    } else {
        crate::apex_data::live_state::get_quote(&chart.symbol)
    };
    let live_bid = live_q.as_ref().map(|q| q.bid as f32).unwrap_or(0.0);
    let live_ask = live_q.as_ref().map(|q| q.ask as f32).unwrap_or(0.0);
    let live_bid_sz = live_q.as_ref().map(|q| q.bid_size as u32).unwrap_or(0);
    let live_ask_sz = live_q.as_ref().map(|q| q.ask_size as u32).unwrap_or(0);
    let current_price = if live_bid > 0.0 && live_ask > 0.0 {
        (live_bid + live_ask) * 0.5
    } else { chart.bars.last().map(|b| b.close).unwrap_or(100.0) };
    let is_index = chart.symbol == "SPX" || chart.symbol == "NDX" || chart.symbol == "DJI" || chart.symbol == "RUT";
    let tick = if is_index { 1.0_f32 } else { 0.01 };
    let center_price = (current_price / tick).round() * tick;
    let sim_size = |price: f32, is_bid: bool| -> u32 {
        if is_bid && live_bid > 0.0 && (price - live_bid).abs() < tick * 0.5 {
            return live_bid_sz.max(1);
        }
        if !is_bid && live_ask > 0.0 && (price - live_ask).abs() < tick * 0.5 {
            return live_ask_sz.max(1);
        }
        let dist = ((price - current_price).abs() / tick).round() as u32;
        let base = 50u32.saturating_sub(dist * 2).max(1);
        let hash = ((price * 1000.0) as u32).wrapping_mul(2654435761);
        (base + hash % 100) + if !is_bid { 20 } else { 0 }
    };
    let position_entry = account_data_cached.as_ref()
        .and_then(|(_, positions, _)| positions.iter().find(|p| p.symbol == chart.symbol))
        .map(|p| p.avg_price);

    // Column headers
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        let col_w = (panel_w - gap_lg()) / 3.0;
        ui.add_sized(egui::vec2(col_w, 14.0), egui::Label::new(egui::RichText::new("BID").monospace().size(font_xs()).color(t.bull.gamma_multiply(0.4))));
        ui.add_sized(egui::vec2(col_w, 14.0), egui::Label::new(egui::RichText::new("PRICE").monospace().size(font_xs()).color(t.dim.gamma_multiply(0.4))));
        ui.add_sized(egui::vec2(col_w, 14.0), egui::Label::new(egui::RichText::new("ASK").monospace().size(font_xs()).color(t.bear.gamma_multiply(0.4))));
    });

    let rows_above = 10_i32; let rows_below = 10_i32;
    for row in (-rows_above..=rows_below).rev() {
        let price = center_price + (row as f32 * tick * -1.0);
        let is_current = (price - center_price).abs() < tick * 0.5;
        let bid_size = sim_size(price, true);
        let ask_size = sim_size(price, false);
        let has_buy = chart.orders.iter().any(|o| (o.price - price).abs() < tick * 0.5 && matches!(o.side, OrderSide::Buy));
        let has_sell = chart.orders.iter().any(|o| (o.price - price).abs() < tick * 0.5 && matches!(o.side, OrderSide::Sell));
        let is_entry = position_entry.map(|ep| (ep - price).abs() < tick * 0.5).unwrap_or(false);
        let row_h = 20.0;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            let rs = ui.cursor().min;
            let rr = egui::Rect::from_min_size(rs, egui::vec2(panel_w - gap_lg(), row_h));
            let rh = ui.input(|i| i.pointer.hover_pos()).map_or(false, |p| rr.contains(p));
            let bg = if is_current { color_alpha(t.accent, 35) } else if rh { color_alpha(t.toolbar_border, 30) } else { egui::Color32::TRANSPARENT };
            ui.painter().rect_filled(rr, 0.0, bg);
            if has_buy { ui.painter().rect_filled(rr, 0.0, color_alpha(t.bull, 25)); }
            if has_sell { ui.painter().rect_filled(rr, 0.0, color_alpha(t.bear, 25)); }
            if is_entry { ui.painter().rect_stroke(rr, 0.0, egui::Stroke::new(1.0, color_alpha(crate::chart_renderer::ui::style::COLOR_AMBER, 150)), egui::StrokeKind::Inside); }
            let col_w = (panel_w - gap_lg()) / 3.0;
            let bc = if rh { t.bull } else { t.bull.gamma_multiply(0.6) };
            let bbg = if rh { color_alpha(t.bull, 15) } else { egui::Color32::TRANSPARENT };
            if ui.add(egui::Button::new(egui::RichText::new(format!("{}", bid_size)).monospace().size(font_sm()).color(bc)).fill(bbg).frame(false).min_size(egui::vec2(col_w, row_h))).clicked() {
                use crate::chart_renderer::trading::order_manager::*;
                if let Some(id) = submit_and_get_id(OrderIntent {
                    symbol: chart.symbol.clone(), side: OrderSide::Buy,
                    order_type: ManagedOrderType::Limit, price, qty: chart.order_qty,
                    source: OrderSource::ChartClick, pair_with: None, option_symbol: None, option_con_id: None, stop_price: 0.0, trail_amount: None, trail_percent: None, last_price: 0.0, tif: 0, outside_rth: false,
                }) {
                    chart.orders.push(OrderLevel { id: id as u32, side: OrderSide::Buy, price, qty: chart.order_qty, status: OrderStatus::Draft, pair_id: None, option_symbol: None, option_con_id: None, trail_amount: None, trail_percent: None });
                }
            }
            let pc = if is_current { egui::Color32::WHITE } else if price > current_price { t.bull.gamma_multiply(0.7) } else { t.bear.gamma_multiply(0.7) };
            let pf = if tick >= 1.0 { format!("{:.0}", price) } else { format!("{:.2}", price) };
            ui.add_sized(egui::vec2(col_w, row_h), egui::Label::new(egui::RichText::new(pf).monospace().size(font_sm()).strong().color(pc)));
            let ac = if rh { t.bear } else { t.bear.gamma_multiply(0.6) };
            let abg = if rh { color_alpha(t.bear, 15) } else { egui::Color32::TRANSPARENT };
            if ui.add(egui::Button::new(egui::RichText::new(format!("{}", ask_size)).monospace().size(font_sm()).color(ac)).fill(abg).frame(false).min_size(egui::vec2(col_w, row_h))).clicked() {
                use crate::chart_renderer::trading::order_manager::*;
                if let Some(id) = submit_and_get_id(OrderIntent {
                    symbol: chart.symbol.clone(), side: OrderSide::Sell,
                    order_type: ManagedOrderType::Limit, price, qty: chart.order_qty,
                    source: OrderSource::ChartClick, pair_with: None, option_symbol: None, option_con_id: None, stop_price: 0.0, trail_amount: None, trail_percent: None, last_price: 0.0, tif: 0, outside_rth: false,
                }) {
                    chart.orders.push(OrderLevel { id: id as u32, side: OrderSide::Sell, price, qty: chart.order_qty, status: OrderStatus::Draft, pair_id: None, option_symbol: None, option_con_id: None, trail_amount: None, trail_percent: None });
                }
            }
        });
    }
    ui.add_space(gap_xs());
    crate::chart_renderer::ui::style::dialog_separator_shadow(ui, 0.0, color_alpha(t.toolbar_border, 50));
}
