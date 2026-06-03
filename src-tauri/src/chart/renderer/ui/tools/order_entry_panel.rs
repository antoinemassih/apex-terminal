//! Floating order entry panel (bottom-left of chart pane).
//!
//! Includes pill (collapsed) mode, draggable header, DOM ladder, and order body.

use egui::Context;
use crate::chart_renderer::ui::style::tint;
use crate::ui_kit::sx::Tone;
use crate::chart_renderer::gpu::{Theme, Chart, render_order_entry_body};
use crate::chart_renderer::trading::{OrderSide, OrderLevel, OrderStatus, OrderState, AccountSummary, Position, IbOrder};
use crate::chart_renderer::gpu::Watchlist;
use crate::chart_renderer::ui::style::{color_alpha, color_subtle, color_muted, color_half, color_dim, color_very_dim, cursor, gap_xs, gap_sm, gap_lg, gap_2xl, font_xs, font_sm, font_md, radius_sm, radius_lg, stroke_std, stroke_thin, alpha_tint, alpha_strong};
use crate::chart_renderer::ui::components::frames_widget::PopupFrame;
use crate::ui_kit::icons::Icon;
use crate::ui_kit::widgets::{PanelEmpty, Button as KitButton};
use crate::ui_kit::widgets::tokens::{Variant as KitVariant, Size as KitSize};

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
    let adv = chart.order_panel.advanced;
    let panel_w = if adv { 270.0 } else { 210.0 };
    let abs_pos = c.abs_pos;

    // ── Collapsed pill ──
    if chart.order_panel.collapsed {
        let pill_w = 90.0;
        // retained as Window: needs corner_radius(12) + custom drag delta capture that bypasses Modal API
        egui::Window::new(format!("order_pill_{}", c.pane_idx))
            .fixed_pos(abs_pos)
            .fixed_size(egui::vec2(pill_w, 24.0))
            .title_bar(false)
            .frame(PopupFrame::new()
                .colors(color_alpha(c.t.toolbar_bg, 235), color_alpha(c.t.toolbar_border, 100))
                .ctx(c.ctx)
                .inner_margin(egui::Margin::symmetric(gap_lg() as i8, gap_sm() as i8))
                .corner_radius(radius_lg())
                .build())
            .show(c.ctx, |ui| {
                let resp = ui.horizontal(|ui| {
                    let armed_dot = if chart.armed { c.t.accent } else { color_very_dim(c.t.dim) };
                    ui.painter().circle_filled(egui::pos2(ui.cursor().min.x + 5.0, ui.cursor().min.y + 8.0), 3.5, armed_dot);
                    ui.add_space(gap_2xl());
                    ui.label(egui::RichText::new("ORDER").monospace().size(font_sm()).strong().color(color_subtle(c.t.dim)));
                });
                let pill_resp = ui.interact(resp.response.rect, egui::Id::new(("order_pill_interact", c.pane_idx)), egui::Sense::click_and_drag());
                if pill_resp.double_clicked() { chart.order_panel.collapsed = false; }
                cursor::draggable(ui, &pill_resp);
                if pill_resp.dragged() {
                    let delta = pill_resp.drag_delta();
                    chart.order_panel.pos.x += delta.x;
                    chart.order_panel.pos.y += delta.y;
                }
            });
        return;
    }

    // ── Expanded panel ── (migrated to ToolOverlay 2026-05-26)
    // Header gets:
    //   • leading slot — armed-state Button (toggles `chart.armed`), inline
    //     position pill (if any), held inside the existing header strip.
    //   • trailing slot — DOM toggle + expand/collapse chevron.
    // Position is host-managed (Chart::order_panel_pos), so drag delta is
    // captured by ToolOverlay and applied here with the same window-edge
    // clamping the old code had.
    use crate::ui_kit::widgets::{ToolOverlay, Button as KitButton, Tooltip};
    use crate::ui_kit::widgets::tokens::{Variant as KitVariant, Size as KitSize};
    let portable_t = crate::chart_renderer::theme_impl::theme_to_portable(c.t);
    // Snapshot data the closures need to capture (immutable view to avoid
    // multi-mut on `chart`).
    let armed_now = chart.armed;
    let adv_now   = adv;
    let dom_open  = chart.dom.open;
    let symbol_for_pos = chart.symbol.clone();
    let position_pill = c.account_data_cached.as_ref()
        .and_then(|(_, positions, _)| positions.iter().find(|p| p.symbol == symbol_for_pos))
        .map(|pos| (pos.qty, if pos.qty > 0 { c.t.bull } else { c.t.bear }));
    // Side-channel flags written by closures (closures take &mut Ui only).
    let mut toggle_armed = false;
    let mut toggle_dom = false;
    let mut toggle_adv = false;

    let overlay_resp = ToolOverlay::new("ORDER")
        .id(&format!("order_entry_{}", c.pane_idx))
        .width(panel_w)
        .controlled_pos(abs_pos)
        .header_leading(|ui| {
            // Armed-state icon button — toggles armed on click.
            let armed_icon = if armed_now { Icon::SHIELD_WARNING } else { Icon::PLAY };
            let r = KitButton::icon(armed_icon)
                .variant(KitVariant::Ghost)
                .size(KitSize::Sm)
                .show(ui, &portable_t);
            Tooltip::new(if armed_now { "Disarm" } else { "Arm — required to place orders" })
                .show(ui, &r, &portable_t);
            if r.clicked() { toggle_armed = true; }
            // Position pill — display only.
            if let Some((qty, col)) = position_pill {
                let txt = if qty > 0 { format!("+{}", qty) } else { format!("{}", qty) };
                ui.label(egui::RichText::new(txt).monospace().size(font_sm()).strong().color(col));
            }
        })
        .header_trailing(|ui| {
            // Right-to-left layout: expand icon FIRST (rightmost), then DOM.
            let exp_icon = if adv_now { Icon::MINUS } else { Icon::PLUS };
            let r = KitButton::icon(exp_icon)
                .variant(KitVariant::Ghost)
                .size(KitSize::Sm)
                .show(ui, &portable_t);
            Tooltip::new(if adv_now { "Compact" } else { "Advanced" }).show(ui, &r, &portable_t);
            if r.clicked() { toggle_adv = true; }
            ui.add(egui::Separator::default().spacing(gap_xs()));
            let dom_col = if dom_open { c.t.accent } else { color_dim(c.t.dim) };
            let dom_resp = ui.add(egui::Label::new(
                egui::RichText::new("DOM").monospace().size(font_xs()).color(dom_col)
            ).sense(egui::Sense::click()));
            crate::chart_renderer::ui::style::cursor::clickable(ui, &dom_resp);
            if dom_resp.clicked() { toggle_dom = true; }
        })
        .show(c.ctx, &portable_t, |ui| {
            // ── DOM ladder (when open) ──
            if chart.dom.open {
                render_dom_ladder(ui, c.t, chart, c.account_data_cached, panel_w);
            }

            // ── Order body ──
            if c.account_data_cached.is_none() {
                PanelEmpty::new("Awaiting account data")
                    .hint("Check broker connection")
                    .show(ui, c.t);
            } else {
                render_order_entry_body(ui, chart, c.t, c.pane_idx as u64, panel_w);
            }
        });

    // Apply header toggles flagged inside the leading/trailing closures.
    if toggle_armed { chart.armed = !chart.armed; }
    if toggle_dom   { chart.dom.open = !chart.dom.open; }
    if toggle_adv   { chart.order_panel.advanced = !chart.order_panel.advanced; }

    // Apply drag delta from ToolOverlay's host-managed position.
    let delta = overlay_resp.drag_delta;
    if delta != egui::Vec2::ZERO {
        chart.order_panel.pos.x += delta.x;
        chart.order_panel.pos.y += delta.y;
        chart.order_panel.pos.x = chart.order_panel.pos.x.clamp(0.0, (c.cw - panel_w).max(0.0));
        if chart.order_panel.pos.y < 0.0 {
            chart.order_panel.pos.y = chart.order_panel.pos.y.clamp(-(c.ch - 30.0), -30.0);
        } else {
            chart.order_panel.pos.y = chart.order_panel.pos.y.clamp(0.0, (c.ch - 30.0).max(0.0));
        }
    }
}

fn render_dom_ladder(
    ui: &mut egui::Ui,
    t: &Theme,
    chart: &mut Chart,
    account_data_cached: &Option<(AccountSummary, Vec<Position>, Vec<IbOrder>)>,
    panel_w: f32,
) {
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
        ui.add_sized(egui::vec2(col_w, 14.0), egui::Label::new(egui::RichText::new("BID").monospace().size(font_xs()).color(color_dim(t.bull))));
        ui.add_sized(egui::vec2(col_w, 14.0), egui::Label::new(egui::RichText::new("PRICE").monospace().size(font_xs()).color(color_dim(t.dim))));
        ui.add_sized(egui::vec2(col_w, 14.0), egui::Label::new(egui::RichText::new("ASK").monospace().size(font_xs()).color(color_dim(t.bear))));
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
            let bg = if is_current { tint(t, Tone::Accent, 35) } else if rh { tint(t, Tone::Border, 30) } else { egui::Color32::TRANSPARENT };
            ui.painter().rect_filled(rr, 0.0, bg);
            if has_buy { ui.painter().rect_filled(rr, 0.0, tint(t, Tone::Bull, 25)); }
            if has_sell { ui.painter().rect_filled(rr, 0.0, tint(t, Tone::Bear, 25)); }
            if is_entry { ui.painter().rect_stroke(rr, 0.0, egui::Stroke::new(stroke_std(), color_alpha(crate::chart_renderer::ui::style::COLOR_AMBER, 150)), egui::StrokeKind::Inside); }
            let col_w = (panel_w - gap_lg()) / 3.0;
            let bc = if rh { t.bull } else { color_muted(t.bull) };
            let bbg = if rh { tint(t, Tone::Bull, 15) } else { egui::Color32::TRANSPARENT };
            let bid_lbl = format!("{}", bid_size);
            if KitButton::new(bid_lbl.as_str()).variant(KitVariant::Ghost).size(KitSize::Sm).fg(bc)
                .min_size(egui::vec2(col_w, row_h)).frameless(true).show(ui, t).clicked() {
                use crate::chart_renderer::trading::order_manager::*;
                if let Some(id) = submit_and_get_id(OrderIntent {
                    symbol: chart.symbol.clone(), side: OrderSide::Buy,
                    order_type: ManagedOrderType::Limit, price, qty: chart.order_panel.qty,
                    source: OrderSource::ChartClick, pair_with: None, option_symbol: None, option_con_id: None, stop_price: 0.0, trail_amount: None, trail_percent: None, last_price: 0.0, tif: 0, outside_rth: false,
                    strategy_id: None, override_warnings: false,
                }) {
                    chart.orders.push(OrderLevel { id: id as u32, side: OrderSide::Buy, price, qty: chart.order_panel.qty, status: OrderStatus::Draft, state: OrderState::Draft, pair_id: None, option_symbol: None, option_con_id: None, trail_amount: None, trail_percent: None, filled_ratio: 0.0 });
                }
            }
            let pc = if is_current { t.text } else if price > current_price { color_subtle(t.bull) } else { color_subtle(t.bear) };
            let pf = if tick >= 1.0 { format!("{:.0}", price) } else { format!("{:.2}", price) };
            ui.add_sized(egui::vec2(col_w, row_h), egui::Label::new(egui::RichText::new(pf).monospace().size(font_sm()).strong().color(pc)));
            let ac = if rh { t.bear } else { color_muted(t.bear) };
            let abg = if rh { tint(t, Tone::Bear, 15) } else { egui::Color32::TRANSPARENT };
            let ask_lbl = format!("{}", ask_size);
            if KitButton::new(ask_lbl.as_str()).variant(KitVariant::Ghost).size(KitSize::Sm).fg(ac)
                .min_size(egui::vec2(col_w, row_h)).frameless(true).show(ui, t).clicked() {
                use crate::chart_renderer::trading::order_manager::*;
                if let Some(id) = submit_and_get_id(OrderIntent {
                    symbol: chart.symbol.clone(), side: OrderSide::Sell,
                    order_type: ManagedOrderType::Limit, price, qty: chart.order_panel.qty,
                    source: OrderSource::ChartClick, pair_with: None, option_symbol: None, option_con_id: None, stop_price: 0.0, trail_amount: None, trail_percent: None, last_price: 0.0, tif: 0, outside_rth: false,
                    strategy_id: None, override_warnings: false,
                }) {
                    chart.orders.push(OrderLevel { id: id as u32, side: OrderSide::Sell, price, qty: chart.order_panel.qty, status: OrderStatus::Draft, state: OrderState::Draft, pair_id: None, option_symbol: None, option_con_id: None, trail_amount: None, trail_percent: None, filled_ratio: 0.0 });
                }
            }
        });
    }
    ui.add_space(gap_xs());
    crate::chart_renderer::ui::style::dialog_separator_shadow(ui, 0.0, tint(t, Tone::Border, 50));
}
