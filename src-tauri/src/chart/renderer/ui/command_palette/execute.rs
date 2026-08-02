//! Action dispatcher — translates an action id into a state mutation.

use super::registry::widget_kind_from_id;
use crate::chart_renderer::gpu::*;
use crate::chart_renderer::gpu::fetch_bars_background;
use crate::chart_renderer::trading::OrderStatus;

pub(super) fn execute(
    id: &str,
    watchlist: &mut Watchlist,
    panes: &mut Vec<Chart>,
    layout: &mut Layout,
    active_pane: &mut usize,
) {
    let ap = *active_pane;

    if id == "ai:chat" { watchlist.cmd_palette.ai_mode = true; return; }
    // AUDIT 2026-08-02 (AT-089): `help:*` and `calc:x` had NO arm in this
    // dispatcher, so they fell through the final `_ => {}` and did nothing while
    // the caller closed the palette — a visibly dead entry.
    //
    // The help view already exists (`render::draw_help_mode`) and is reachable by
    // typing `? widgets`. These registry entries were meant to be the
    // discoverable shortcut into it, so they set the query rather than
    // duplicating the renderer.
    if let Some(topic) = id.strip_prefix("help:") {
        watchlist.cmd_palette.query = format!("? {topic}");
        return;
    }
    if id == "dyn:reorganize" {
        watchlist.cmd_palette.ai_mode = true;
        watchlist.cmd_palette.ai_input =
            "Reorganize the layout for the current task (Dynamic UI placeholder — Gemma 2B)".into();
        return;
    }

    // Symbols
    if let Some(sym) = id.strip_prefix("sym:") {
        let tf = panes[ap].timeframe.clone();
        panes[ap].symbol = sym.to_string();
        panes[ap].symbol_meta = crate::foundation::types::symbol_or_guess(sym);
        panes[ap].pending_symbol_change = Some(sym.to_string());
        fetch_bars_background(sym.to_string(), tf, 0);
        // Wave 12c: the cross-pane SubscriptionBus publish for this
        // symbol change happens centrally in `App::about_to_wait` when
        // `pending_symbol_change` is consumed. No per-call-site publish
        // is needed — the centralized publisher knows the originating
        // pane index (required for the drain-and-apply step to skip the
        // origin), which we can't accurately reconstruct here when the
        // pending change has not yet been applied.
        let _ = watchlist; // bus is reached from about_to_wait, not here
        return;
    }

    // Themes — search the live list so installed themes are reachable
    if let Some(name) = id.strip_prefix("theme:") {
        let all = crate::chart_renderer::gpu::get_all_themes();
        if let Some((i, _)) = all.iter().enumerate().find(|(_, th)| th.name.eq_ignore_ascii_case(name)) {
            for p in panes.iter_mut() { p.theme_idx = i; }
        }
        return;
    }

    // Timeframes
    if let Some(tf) = id.strip_prefix("tf:") {
        panes[ap].timeframe = tf.to_string();
        panes[ap].pending_timeframe_change = Some(tf.to_string());
        let sym = panes[ap].symbol.clone();
        fetch_bars_background(sym, tf.to_string(), 0);
        return;
    }

    // Layouts
    if let Some(ly_id) = id.strip_prefix("layout:") {
        let Some(ly) = Layout::from_label(ly_id) else { return };
        // Grow/shrink panes, swap the template, snapshot undo and regenerate
        // PaneLayout — shared with the dev-harness SetLayoutLive path so there
        // is exactly one implementation of "apply a layout template".
        crate::chart_renderer::gpu::apply_layout_template(
            panes, watchlist, layout, active_pane, ly,
        );
        return;
    }

    // Widgets
    if let Some(wid) = id.strip_prefix("widget:") {
        if let Some(kind) = widget_kind_from_id(wid) {
            // Place at next sensible slot, avoid stacking
            let n = panes[ap].chart_widgets.len();
            let x = 0.02 + (n as f32 * 0.05).min(0.5);
            let y = 0.05 + (n as f32 * 0.08).min(0.6);
            panes[ap].chart_widgets.push(crate::chart_renderer::ChartWidget::new(kind, x, y));
        }
        return;
    }

    // Overlays
    if let Some(ov) = id.strip_prefix("overlay:") {
        let c = &mut panes[ap];
        match ov {
            "vol-shelves"   => c.show_vol_shelves = !c.show_vol_shelves,
            "confluence"    => c.show_confluence = !c.show_confluence,
            "momentum"      => c.show_momentum_heat = !c.show_momentum_heat,
            "trend-strip"   => c.show_trend_strip = !c.show_trend_strip,
            "breadth"       => c.show_breadth_tint = !c.show_breadth_tint,
            "vol-cone"      => c.show_vol_cone = !c.show_vol_cone,
            "price-memory"  => c.show_price_memory = !c.show_price_memory,
            "liquidity"     => c.show_liquidity_voids = !c.show_liquidity_voids,
            "corr-ribbon"   => c.show_corr_ribbon = !c.show_corr_ribbon,
            "analyst"       => c.show_analyst_targets = !c.show_analyst_targets,
            "pe-band"       => c.show_pe_band = !c.show_pe_band,
            "insider"       => c.show_insider_trades = !c.show_insider_trades,
            _ => {}
        }
        return;
    }

    // Settings
    match id {
        "setting:hotkeys"        => { watchlist.update_sidebar_state(|s| s.hotkey_editor_open = true); return; }
        "setting:settings"       => { watchlist.update_sidebar_state(|s| s.settings_open = true); return; }
        "setting:apex-diag"      => { watchlist.update_sidebar_state(|s| s.apex_diag_open = true); return; }
        "setting:replay"         => { watchlist.update_sidebar_state(|s| s.replay_pane_open = true); return; }
        "setting:workspace"      => { watchlist.update_sidebar_state(|s| s.settings_open = true); return; }
        "setting:pane-chart"     => { panes[ap].pane_type = PaneType::Chart; return; }
        "setting:pane-portfolio" => { panes[ap].pane_type = PaneType::Portfolio; return; }
        "setting:pane-dashboard" => { panes[ap].pane_type = PaneType::Dashboard; return; }
        "setting:pane-heatmap"   => { panes[ap].pane_type = PaneType::Heatmap; return; }
        "setting:pane-spreadsheet"     => { panes[ap].pane_type = PaneType::Spreadsheet; return; }
        _ => {}
    }

    // Plays — jump active pane to play's symbol
    if let Some(pid) = id.strip_prefix("play:") {
        if let Some(p) = watchlist.plays.iter().find(|p| p.id == pid) {
            let sym = p.symbol.clone();
            let tf = panes[ap].timeframe.clone();
            panes[ap].symbol = sym.clone();
            panes[ap].symbol_meta = crate::foundation::types::symbol_or_guess(&sym);
            panes[ap].pending_symbol_change = Some(sym.clone());
            fetch_bars_background(sym, tf, 0);
        }
        return;
    }

    // Alerts — jump active pane to alert's symbol
    if let Some(aid) = id.strip_prefix("alert:") {
        if let Ok(parsed_id) = aid.parse::<u32>() {
            if let Some(a) = watchlist.alerts.iter().find(|a| a.id == parsed_id) {
                let sym = a.symbol.clone();
                let tf = panes[ap].timeframe.clone();
                panes[ap].symbol = sym.clone();
                panes[ap].symbol_meta = crate::foundation::types::symbol_or_guess(&sym);
                panes[ap].pending_symbol_change = Some(sym.clone());
                fetch_bars_background(sym, tf, 0);
            }
        }
        return;
    }

    // Trading actions
    match id {
        "cmd:flatten" => {
            for chart in panes.iter_mut() {
                chart.orders.retain(|o| o.status == OrderStatus::Executed);
            }
            // WS-H #41a: guarded account-wide flatten via OrderManager.
            crate::chart_renderer::trading::order_manager::flatten_all();
        }
        "cmd:cancel" => {
            for chart in panes.iter_mut() { chart.orders.clear(); }
            crate::chart_renderer::trading::order_manager::cancel_all_working();
        }
        "cmd:reverse" => {
            crate::chart_renderer::trading::order_manager::reverse_all();
        }
        "cmd:halfsize" => {
            crate::chart_renderer::trading::order_manager::halve_all();
        }
        _ => {}
    }
}
