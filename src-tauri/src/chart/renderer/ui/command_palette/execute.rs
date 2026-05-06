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

    if id == "ai:chat" { watchlist.cmd_palette_ai_mode = true; return; }
    if id == "dyn:reorganize" {
        watchlist.cmd_palette_ai_mode = true;
        watchlist.cmd_palette_ai_input =
            "Reorganize the layout for the current task (Dynamic UI placeholder — Gemma 2B)".into();
        return;
    }

    // Symbols
    if let Some(sym) = id.strip_prefix("sym:") {
        let tf = panes[ap].timeframe.clone();
        panes[ap].symbol = sym.to_string();
        panes[ap].pending_symbol_change = Some(sym.to_string());
        fetch_bars_background(sym.to_string(), tf);
        return;
    }

    // Themes
    if let Some(name) = id.strip_prefix("theme:") {
        if let Some((i, _)) = THEMES.iter().enumerate().find(|(_, th)| th.name.eq_ignore_ascii_case(name)) {
            for p in panes.iter_mut() { p.theme_idx = i; }
        }
        return;
    }

    // Timeframes
    if let Some(tf) = id.strip_prefix("tf:") {
        panes[ap].timeframe = tf.to_string();
        let sym = panes[ap].symbol.clone();
        fetch_bars_background(sym, tf.to_string());
        return;
    }

    // Layouts
    if let Some(ly_id) = id.strip_prefix("layout:") {
        let ly = match ly_id {
            "1" => Layout::One, "2" => Layout::Two, "2H" => Layout::TwoH,
            "3" => Layout::Three, "3L" => Layout::ThreeL,
            "4" => Layout::Four, "4L" => Layout::FourL,
            "5C" => Layout::FiveC, "6" => Layout::Six, "9" => Layout::Nine,
            _ => return,
        };
        let max = ly.max_panes();
        while panes.len() < max {
            let syms = ["SPY","AAPL","MSFT","NVDA","TSLA","AMZN","META","GOOG","AMD"];
            let sym = syms.get(panes.len()).copied().unwrap_or("SPY");
            let tf = panes[0].timeframe.clone();
            let mut p = Chart::new_with(sym, &tf);
            p.theme_idx = panes[0].theme_idx;
            panes.push(p);
        }
        *layout = ly;
        if *active_pane >= max { *active_pane = 0; }
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
        "setting:hotkeys"        => { watchlist.hotkey_editor_open = true; return; }
        "setting:settings"       => { watchlist.settings_open = true; return; }
        "setting:apex-diag"      => { watchlist.apex_diag_open = true; return; }
        "setting:workspace"      => { watchlist.settings_open = true; return; }
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
            panes[ap].pending_symbol_change = Some(sym.clone());
            fetch_bars_background(sym, tf);
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
                panes[ap].pending_symbol_change = Some(sym.clone());
                fetch_bars_background(sym, tf);
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
            std::thread::spawn(|| {
                let _ = reqwest::blocking::Client::new()
                    .post(format!("{}/risk/flatten", APEXIB_URL))
                    .timeout(std::time::Duration::from_secs(5)).send();
            });
        }
        "cmd:cancel" => {
            for chart in panes.iter_mut() { chart.orders.clear(); }
            std::thread::spawn(|| {
                let _ = reqwest::blocking::Client::new()
                    .delete(format!("{}/orders", APEXIB_URL))
                    .timeout(std::time::Duration::from_secs(5)).send();
            });
        }
        "cmd:reverse" => {
            // Placeholder — signal via API if present
            std::thread::spawn(|| {
                let _ = reqwest::blocking::Client::new()
                    .post(format!("{}/risk/reverse", APEXIB_URL))
                    .timeout(std::time::Duration::from_secs(5)).send();
            });
        }
        "cmd:halfsize" => {
            std::thread::spawn(|| {
                let _ = reqwest::blocking::Client::new()
                    .post(format!("{}/risk/halve", APEXIB_URL))
                    .timeout(std::time::Duration::from_secs(5)).send();
            });
        }
        _ => {}
    }
}
