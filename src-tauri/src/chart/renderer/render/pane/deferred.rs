//! Phase 8: Handle deferred actions (option chart open, underlying orders, repaint).
//!
//! Extracted from `core.rs` — interaction-gated, zero per-frame render cost.

#![allow(unused_imports)]
#![allow(unused_variables)]

use crate::chart_renderer::gpu::*;
use crate::chart_renderer::trading::*;

/// Phase 8: Handle deferred actions (option chart open, underlying orders, repaint).
pub(super) fn handle_deferred(
    ctx: &egui::Context,
    panes: &mut Vec<Chart>,
    active_pane: &mut usize,
    layout: &mut Layout,
    watchlist: &mut Watchlist,
) {
    // ── Handle deferred option chart open ──
    // Replaces the CURRENT (active) pane with the option chart
    if let Some(p) = watchlist.pending_opt_chart.take() {
        let (sym, strike, is_call, expiry) = (p.symbol, p.strike, p.is_call, p.expiry);
        let ap = *active_pane;
        let raw_occ = watchlist.pending_opt_chart_contract.take().unwrap_or_default();
        crate::apex_log!("option.click", "sym={sym} strike={strike} is_call={is_call} expiry='{expiry}' raw_occ='{raw_occ}'");
        let occ = if raw_occ.starts_with("O:") {
            raw_occ
        } else {
            let o = synthesize_occ(&sym, strike, is_call, &expiry);
            crate::apex_log!("option.occ", "synthesized OCC: {o}");
            o
        };
        let strike_str = if (strike - strike.round()).abs() < 0.005 { format!("{:.0}", strike) } else { format!("{:.1}", strike) };
        let opt_sym = format!("{} {}{} {}", sym, strike_str, if is_call { "C" } else { "P" }, expiry);
        crate::apex_log!("option.open", "occ={occ} display_sym='{opt_sym}'");
        // Always open the contract in the active pane. The user expects clicks
        // on the chain to land where they're focused, not in some other pane.
        let target = ap.min(panes.len().saturating_sub(1));
        panes[target].symbol = opt_sym.clone();
        panes[target].is_option = true;
        panes[target].underlying = sym.clone();
        panes[target].option_type = if is_call { "C".into() } else { "P".into() };
        panes[target].option_strike = strike;
        panes[target].option_expiry = expiry;
        // Drawings are keyed by OCC — reset the fetch gate when switching contracts
        // or converting an equity pane to option mode (equity drawings would otherwise
        // persist on a price scale they don't belong to).
        if panes[target].option_contract != occ || !panes[target].is_option {
            panes[target].drawings.clear();
            panes[target].drawings_requested = false;
        }
        panes[target].option_contract = occ.clone();

        let tf = panes[target].timeframe.clone();

        // Clear bars — we only want real data. The fetcher will populate via
        // ChartCommand::LoadBars on success and subscribe the WS for live ticks.
        panes[target].bars.clear();
        panes[target].timestamps.clear();

        if occ.is_empty() {
            eprintln!("[option-chart] No OCC contract ticker — cannot fetch bars for {}", opt_sym);
            crate::apex_data::live_state::push_toast(
                format!("\x02No contract symbol for {} — cannot load option chart", opt_sym));
            crate::wake_native_ui();
        } else if !crate::apex_data::is_enabled() {
            eprintln!("[option-chart] ApexData disabled — cannot fetch bars for {}", occ);
            crate::apex_data::live_state::push_toast(
                "\x02ApexData is disabled — option bars require an active ApexData subscription".to_string());
            crate::wake_native_ui();
        } else {
            fetch_option_bars_background(occ.clone(), opt_sym, tf.clone(), panes[target].bar_source_mark);
        }
        panes[target].vs = (panes[target].bars.len() as f32 - panes[target].vc as f32 + CHART_RIGHT_PAD as f32).max(0.0);
        panes[target].auto_scroll = true;
        panes[target].indicator_bar_count = 0;
        *active_pane = target;
    }

    // ── Handle deferred underlying order actions ──
    // Check if any option pane requested to place an order on its underlying
    let mut und_action: Option<(usize, OrderSide, String, String, f32, String, u32)> = None;
    for (pi, pane) in panes.iter_mut().enumerate() {
        if let Some(side) = pane.pending_und_order.take() {
            und_action = Some((pi, side, pane.underlying.clone(), pane.option_type.clone(), pane.option_strike, pane.option_expiry.clone(), pane.order_panel.qty));
        }
    }
    if let Some((source_pi, side, underlying, opt_type, strike, expiry, qty)) = und_action {
        let opt_sym = panes[source_pi].symbol.clone();
        let source_sym = panes[source_pi].symbol.clone();
        // Inherit from the option pane that triggered the order, not panes[0].
        let tf = panes[source_pi].timeframe.clone();
        let theme = panes[source_pi].theme_idx;

        // Find or create the underlying pane
        let und_pane = panes.iter().position(|p| p.symbol == underlying && !p.is_option);
        let target_pi = if let Some(pi) = und_pane {
            pi
        } else if panes.len() <= 1 {
            *layout = Layout::TwoH;
            let mut p = Chart::new_with(&underlying, &tf);
            p.theme_idx = theme;
            p.pending_symbol_change = Some(underlying.clone());
            panes.push(p);
            panes.len() - 1
        } else {
            let other = panes.iter().position(|p| !p.is_option && p.symbol != source_sym);
            let pi = other.unwrap_or((source_pi + 1) % panes.len());
            panes[pi].pending_symbol_change = Some(underlying.clone());
            panes[pi].is_option = false;
            pi
        };

        // Place a draft order level on the underlying pane — same as regular orders
        let last = panes[target_pi].bars.last().map(|b| b.close).unwrap_or(0.0);
        {
            use crate::chart_renderer::trading::order_manager::*;
            let intent = OrderIntent {
                symbol: panes[target_pi].symbol.clone(), side,
                order_type: ManagedOrderType::Limit, price: last, qty,
                source: OrderSource::Trigger, pair_with: None,
                option_symbol: Some(opt_sym.clone()), option_con_id: None, stop_price: 0.0, trail_amount: None, trail_percent: None, last_price: 0.0, tif: 0, outside_rth: false,
                strategy_id: None, override_warnings: false,
            };
            let result = submit_order(intent.clone());
            match result {
                OrderResult::Accepted(id) => {
                    panes[target_pi].orders.push(OrderLevel {
                        id: id as u32, side, price: last, qty, status: OrderStatus::Placed, state: OrderState::Working, pair_id: None,
                        option_symbol: Some(opt_sym), option_con_id: None, trail_amount: None, trail_percent: None, filled_ratio: 0.0,
                    });
                }
                OrderResult::NeedsConfirmation(id) => {
                    panes[target_pi].orders.push(OrderLevel {
                        id: id as u32, side, price: last, qty, status: OrderStatus::Draft, state: OrderState::Draft, pair_id: None,
                        option_symbol: Some(opt_sym), option_con_id: None, trail_amount: None, trail_percent: None, filled_ratio: 0.0,
                    });
                    panes[target_pi].pending_confirms.push((id as u32, std::time::Instant::now()));
                }
                OrderResult::NeedsApproval { reason, .. } => {
                    // Surface a per-frame confirmation modal — operator must
                    // explicitly click "Override and submit" to resubmit with
                    // override_warnings=true. See draw_chart tail for renderer.
                    enqueue_approval(reason, intent);
                }
                OrderResult::Rejected(reason) => {
                    eprintln!("[option-trigger] rejected: {}", reason);
                }
                OrderResult::Duplicate => { /* silently blocked */ }
            }
        }
        *active_pane = target_pi;
    }

    // Catch-all per-frame repaint removed: the app now sleeps when nothing is
    // changing. Background threads (data feeds, fetch jobs, async tasks) call
    // `crate::wake_native_ui()` after sending ChartCommands so the UI wakes
    // for live data; egui's own repaint scheduler handles animations + hover
    // + interaction. If you find a path that updates state but doesn't
    // re-render, add a `crate::wake_native_ui()` (or `ctx.request_repaint()`
    // when ctx is in scope) at the producing site.
}
