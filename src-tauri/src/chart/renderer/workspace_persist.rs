//! Workspace persistence (WS-E E2 extraction from gpu.rs).
//!
//! Serialize/restore a full workspace: chart panes (symbol + every toggle +
//! indicators), the pane layout, and per-workspace UI state — plus the named-
//! workspace file operations (save/delete/rename/list). This is the single
//! biggest "persist the god-objects" responsibility; it is inherently coupled
//! to Chart/Watchlist/Layout/Indicator, so it glob-imports gpu's surface. The
//! path helpers stay in gpu.rs (used across the file); this module calls them
//! via the glob. Extracted verbatim; re-exported from gpu.rs so
//! `gpu::save_state()` / `gpu::load_state()` / `gpu::save_workspace()` / ... and
//! gpu.rs's own bare calls are unchanged.

use super::gpu::*;
use super::LineStyle; // re-exported from ui_kit at mod.rs

fn workspace_to_json(panes: &[Chart], layout: Layout, wl: &Watchlist) -> String {
    let pane_data: Vec<serde_json::Value> = panes.iter().map(|p| {
        let indicators: Vec<serde_json::Value> = p.indicators.iter().map(|ind| serde_json::json!({
            "kind": ind.kind.label(), "period": ind.period, "color": ind.color,
            "visible": ind.visible, "thickness": ind.thickness,
            "param2": ind.param2, "param3": ind.param3, "param4": ind.param4,
            "source": ind.source, "offset": ind.offset,
            "ob_level": ind.ob_level, "os_level": ind.os_level,
            "source_tf": ind.source_tf,
            "line_style": match ind.line_style { LineStyle::Solid => "solid", LineStyle::Dashed => "dashed", LineStyle::Dotted => "dotted" },
            // Band styling (BB, Keltner, etc.) — v3 parity
            "upper_color": ind.upper_color, "lower_color": ind.lower_color,
            "fill_color_hex": ind.fill_color_hex,
            "upper_thickness": ind.upper_thickness, "lower_thickness": ind.lower_thickness,
        })).collect();
        serde_json::json!({
            "symbol": p.symbol, "timeframe": p.timeframe,
            "show_volume": p.show_volume, "show_oscillators": p.show_oscillators,
            "ohlc_tooltip": p.ohlc_tooltip, "magnet": p.magnet, "log_scale": p.log_scale,
            "show_vwap_bands": p.show_vwap_bands, "show_cvd": p.show_cvd,
            "show_delta_volume": p.show_delta_volume, "show_rvol": p.show_rvol,
            "show_ma_ribbon": p.show_ma_ribbon, "show_prev_close": p.show_prev_close,
            "show_auto_sr": p.show_auto_sr, "show_auto_fib": p.show_auto_fib, "swing_leg_mode": p.swing_leg_mode, "show_footprint": p.show_footprint,
            "show_gamma": p.show_gamma, "show_darkpool": p.show_darkpool, "show_events": p.show_events, "hit_highlight": p.hit_highlight,
            "show_pnl_curve": p.show_pnl_curve, "show_pattern_labels": p.show_pattern_labels,
            "link_group": p.link_group,
            "session_shading": p.session_shading,
            "rth_start_minutes": p.rth_start_minutes,
            "rth_end_minutes": p.rth_end_minutes,
            "eth_bar_opacity": p.eth_bar_opacity,
            "session_bg_tint": p.session_bg_tint,
            "session_bg_color": p.session_bg_color,
            "session_bg_opacity": p.session_bg_opacity,
            "session_break_lines": p.session_break_lines,
            "candle_mode": match p.candle_mode {
                CandleMode::Standard => "std", CandleMode::Violin => "vln",
                CandleMode::Gradient => "grd", CandleMode::ViolinGradient => "vg",
                CandleMode::HeikinAshi => "ha", CandleMode::Line => "line", CandleMode::Area => "area",
                    CandleMode::Renko => "rnk", CandleMode::RangeBar => "rng", CandleMode::TickBar => "tck",
            },
            "renko_brick_size": p.alt.renko_brick,
            "range_bar_size": p.alt.range_size,
            "tick_bar_count": p.alt.tick_count,
            "vp_mode": match p.vp.mode {
                VolumeProfileMode::Off => "off", VolumeProfileMode::Classic => "classic",
                VolumeProfileMode::Heatmap => "heatmap", VolumeProfileMode::Strip => "strip",
                VolumeProfileMode::Clean => "clean",
            },
            "indicators": indicators,
            // v3 parity: chart widgets
            "chart_widgets": serde_json::to_value(&p.chart_widgets).unwrap_or_default(),
            // v3 parity: option-pane state
            "is_option": p.is_option,
            "option_contract": p.option_contract,
            "option_strike": p.option_strike,
            "option_type": p.option_type,
            "option_expiry": p.option_expiry,
            "underlying": p.underlying,
            // v3 parity: bar source (Last vs Mark)
            "bar_source": if p.bar_source_mark { "mark" } else { "last" },
            // v4: per-pane DOM panel open state (floating + sidebar mode).
            "dom_open": p.dom.open,
            "dom_sidebar_open": p.dom.sidebar_open,
            // v5: overlay toggles (command-palette "overlay:" actions).
            // All default to false so older workspaces load with overlays off.
            "show_vol_shelves":    p.show_vol_shelves,
            "show_confluence":     p.show_confluence,
            "show_momentum_heat":  p.show_momentum_heat,
            "show_trend_strip":    p.show_trend_strip,
            "show_breadth_tint":   p.show_breadth_tint,
            "show_vol_cone":       p.show_vol_cone,
            "show_price_memory":   p.show_price_memory,
            "show_liquidity_voids":p.show_liquidity_voids,
            "show_corr_ribbon":    p.show_corr_ribbon,
            "show_analyst_targets":p.show_analyst_targets,
            "show_pe_band":        p.show_pe_band,
            "show_insider_trades": p.show_insider_trades,
            // v6: tab history (symbol/timeframe per tab slot) and symbol overlays
            "tab_symbols":    &p.tab_symbols,
            "tab_timeframes": &p.tab_timeframes,
            "tab_active":     p.tab_active,
            "symbol_overlays": p.symbol_overlays.iter().map(|ov| serde_json::json!({
                "symbol": &ov.symbol, "color": &ov.color,
                "show_candles": ov.show_candles, "visible": ov.visible,
            })).collect::<Vec<_>>(),
        })
    }).collect();
    let state = serde_json::json!({
        // v4: adds pane split ratios so a saved chart restores its exact pane
        // geometry. Drawings are intentionally NOT serialized here — they live
        // in their own per-symbol Postgres store and reload automatically when a
        // restored pane's symbol loads (see drawing_db + fetch.rs symbol-load).
        "version": 4,
        "layout": layout.label(),
        "theme_idx": panes.first().map(|p| p.theme_idx).unwrap_or(5),
        "panes": pane_data,
        "recent_symbols": panes.first().map(|p| &p.recent_symbols).cloned().unwrap_or_default(),
        // Pane geometry: the authoritative recursive split tree (modern path)
        // plus the legacy float ratios (fallback path). Restored together with
        // the layout so split positions survive a save/load round-trip.
        "pane_layout": serde_json::to_value(&wl.pane_layout).unwrap_or(serde_json::Value::Null),
        "splits": {
            "h":  wl.pane_split_h,  "v":  wl.pane_split_v,
            "h2": wl.pane_split_h2, "v2": wl.pane_split_v2,
            "v3": wl.pane_split_v3, "v4": wl.pane_split_v4,
            "v5": wl.pane_split_v5, "v6": wl.pane_split_v6,
        },
        // v4: per-workspace UI state — which side panels are open, the focused
        // pane, and the workspace-rail expand state. Restored on load so a
        // workspace remembers its full view, not just the chart panes.
        "ui": {
            "active_pane":        wl.active_pane_idx,
            "rail_expanded":      wl.workspace_nav_expanded,
            "object_tree_open":   wl.object_tree_open,
            "watchlist_open":     wl.open,
            "signals_panel_open": wl.signals_panel.open,
            "account_strip_open": wl.account_strip_open,
        },
    });
    serde_json::to_string_pretty(&state).unwrap_or_default()
}

/// Filesystem-safe workspace file stem (shared by save/delete/rename so they
/// always resolve to the same path for a given display name).
fn sanitize_workspace_name(name: &str) -> String {
    name.chars().map(|c| if c.is_alphanumeric() || c == '-' || c == '_' || c == ' ' { c } else { '_' }).collect()
}

pub(crate) fn save_workspace(name: &str, panes: &[Chart], layout: Layout, wl: &Watchlist) {
    let json = workspace_to_json(panes, layout, wl);
    let path = workspace_dir().join(format!("{}.json", sanitize_workspace_name(name)));
    let _ = crate::state::persistence::atomic_write(&path, json.as_bytes());
}

/// Delete a saved workspace by display name. No-op if it doesn't exist.
pub(crate) fn delete_workspace(name: &str) {
    let path = workspace_dir().join(format!("{}.json", sanitize_workspace_name(name)));
    let _ = std::fs::remove_file(path);
}

/// Rename a saved workspace file. No-op if `old` is missing or names collide.
pub(crate) fn rename_workspace(old: &str, new: &str) {
    let from = workspace_dir().join(format!("{}.json", sanitize_workspace_name(old)));
    let to   = workspace_dir().join(format!("{}.json", sanitize_workspace_name(new)));
    if from != to && from.exists() {
        let _ = std::fs::rename(from, to);
    }
}

pub(crate) fn list_workspaces() -> Vec<String> {
    let dir = workspace_dir();
    let mut names: Vec<String> = std::fs::read_dir(dir).ok().map(|entries| {
        entries.filter_map(|e| {
            let name = e.ok()?.file_name().to_string_lossy().to_string();
            if name.ends_with(".json") { Some(name.trim_end_matches(".json").to_string()) } else { None }
        }).collect()
    }).unwrap_or_default();
    names.sort();
    names
}

/// Pick a fresh "Untitled" workspace name that doesn't collide with an existing
/// saved workspace. Returns "Untitled", then "Untitled 2", "Untitled 3", …
pub(crate) fn next_untitled_workspace_name() -> String {
    let existing: std::collections::HashSet<String> = list_workspaces().into_iter().collect();
    if !existing.contains("Untitled") { return "Untitled".to_string(); }
    for n in 2..1000 {
        let candidate = format!("Untitled {n}");
        if !existing.contains(&candidate) { return candidate; }
    }
    "Untitled".to_string()
}

pub(crate) fn save_state(panes: &[Chart], layout: Layout, watchlist: &mut Watchlist) {
    // Phase 3 (state): push ALL legacy fields into their stores via the
    // single chokepoint so every aggregate is guaranteed fresh before any
    // bytes hit disk. Replaces the previous ad-hoc subset of push calls.
    watchlist.push_all_stores();
    if let Err(e) = crate::state::save(&ui_settings_path(), &watchlist.ui_settings) {
        eprintln!("[state] ui_settings save failed: {e}");
    }
    // P2 (command-palette-frecency): persist frecency data.  Prune freq to
    // the top 200 by count before saving to keep the file bounded.
    {
        let mut freq = watchlist.cmd_palette.freq.clone();
        if freq.len() > 200 {
            let mut pairs: Vec<_> = freq.iter().map(|(k, &v)| (k.clone(), v)).collect();
            pairs.sort_by(|a, b| b.1.cmp(&a.1));
            pairs.truncate(200);
            freq = pairs.into_iter().collect();
        }
        let ps = crate::state::CmdPaletteState {
            recent: watchlist.cmd_palette.recent.clone(),
            freq,
        };
        if let Err(e) = crate::state::save(&cmd_palette_state_path(), &ps) {
            eprintln!("[state] cmd_palette_state save failed: {e}");
        }
    }
    let pane_data: Vec<serde_json::Value> = panes.iter().map(|p| {
        // Serialize indicators — include ALL styling fields
        let indicators: Vec<serde_json::Value> = p.indicators.iter().map(|ind| serde_json::json!({
            "kind": ind.kind.label(), "period": ind.period, "color": ind.color,
            "visible": ind.visible, "thickness": ind.thickness,
            "param2": ind.param2, "param3": ind.param3, "param4": ind.param4,
            "source": ind.source, "offset": ind.offset,
            "ob_level": ind.ob_level, "os_level": ind.os_level,
            "source_tf": ind.source_tf,
            "line_style": match ind.line_style { LineStyle::Solid => "solid", LineStyle::Dashed => "dashed", LineStyle::Dotted => "dotted" },
            // Band styling (BB, Keltner, etc.)
            "upper_color": ind.upper_color, "lower_color": ind.lower_color,
            "fill_color_hex": ind.fill_color_hex,
            "upper_thickness": ind.upper_thickness, "lower_thickness": ind.lower_thickness,
        })).collect();
        serde_json::json!({
            "symbol": p.symbol, "timeframe": p.timeframe,
            // Toggles
            "show_volume": p.show_volume, "show_oscillators": p.show_oscillators,
            "ohlc_tooltip": p.ohlc_tooltip, "magnet": p.magnet, "log_scale": p.log_scale,
            "show_vwap_bands": p.show_vwap_bands, "show_cvd": p.show_cvd,
            "show_delta_volume": p.show_delta_volume, "show_rvol": p.show_rvol,
            "show_ma_ribbon": p.show_ma_ribbon, "show_prev_close": p.show_prev_close,
            "show_auto_sr": p.show_auto_sr, "show_auto_fib": p.show_auto_fib, "swing_leg_mode": p.swing_leg_mode, "show_footprint": p.show_footprint,
            "show_gamma": p.show_gamma, "show_darkpool": p.show_darkpool, "show_events": p.show_events, "hit_highlight": p.hit_highlight,
            "show_pnl_curve": p.show_pnl_curve, "show_pattern_labels": p.show_pattern_labels,
            "link_group": p.link_group,
            // Session shading
            "session_shading": p.session_shading,
            "rth_start_minutes": p.rth_start_minutes,
            "rth_end_minutes": p.rth_end_minutes,
            "eth_bar_opacity": p.eth_bar_opacity,
            "session_bg_tint": p.session_bg_tint,
            "session_bg_color": p.session_bg_color,
            "session_bg_opacity": p.session_bg_opacity,
            "session_break_lines": p.session_break_lines,
            // Modes
            "candle_mode": match p.candle_mode {
                CandleMode::Standard => "std", CandleMode::Violin => "vln",
                CandleMode::Gradient => "grd", CandleMode::ViolinGradient => "vg",
                CandleMode::HeikinAshi => "ha", CandleMode::Line => "line", CandleMode::Area => "area",
                    CandleMode::Renko => "rnk", CandleMode::RangeBar => "rng", CandleMode::TickBar => "tck",
            },
            "renko_brick_size": p.alt.renko_brick,
            "range_bar_size": p.alt.range_size,
            "tick_bar_count": p.alt.tick_count,
            "vp_mode": match p.vp.mode {
                VolumeProfileMode::Off => "off", VolumeProfileMode::Classic => "classic",
                VolumeProfileMode::Heatmap => "heatmap", VolumeProfileMode::Strip => "strip",
                VolumeProfileMode::Clean => "clean",
            },
            // Indicators
            "indicators": indicators,
            // Chart widgets
            "chart_widgets": serde_json::to_value(&p.chart_widgets).unwrap_or_default(),
            // Option-pane state (preserved across sessions so option charts
            // restore as option charts, not as broken stock fetches).
            "is_option": p.is_option,
            "option_contract": p.option_contract,
            "option_strike": p.option_strike,
            "option_type": p.option_type,
            "option_expiry": p.option_expiry,
            "underlying": p.underlying,
            // MARK_BARS_PROTOCOL — persist Last/Mark choice per chart.
            "bar_source": if p.bar_source_mark { "mark" } else { "last" },
            // v4: per-pane DOM panel state
            "dom_open": p.dom.open,
            "dom_sidebar_open": p.dom.sidebar_open,
            // v6: tab history and symbol overlays
            "tab_symbols":    &p.tab_symbols,
            "tab_timeframes": &p.tab_timeframes,
            "tab_active":     p.tab_active,
            "symbol_overlays": p.symbol_overlays.iter().map(|ov| serde_json::json!({
                "symbol": &ov.symbol, "color": &ov.color,
                "show_candles": ov.show_candles, "visible": ov.visible,
            })).collect::<Vec<_>>(),
        })
    }).collect();
    // Global settings from Watchlist
    let phs = match watchlist.pane_header_size {
        crate::chart_renderer::PaneHeaderSize::Compact => "compact",
        crate::chart_renderer::PaneHeaderSize::Normal => "normal",
        crate::chart_renderer::PaneHeaderSize::Expanded => "expanded",
    };
    let state = serde_json::json!({
        "version": 3,
        "layout": layout.label(),
        "theme_idx": panes.first().map(|p| p.theme_idx).unwrap_or(5),
        "panes": pane_data,
        "recent_symbols": panes.first().map(|p| &p.recent_symbols).cloned().unwrap_or_default(),
        "draw_favorites": watchlist.draw_favorites,
        "style_idx": watchlist.style_idx,
        // P4.3 + P5 — user token-scale overrides (null = inherit from style preset).
        "density_override":       watchlist.density_override.map(|m| m.as_u8()),
        "border_weight_override": watchlist.border_weight_override.map(|m| m.as_u8()),
        "corner_scale_override":  watchlist.corner_scale_override.map(|m| m.as_u8()),
        "spacing_scale_override": watchlist.spacing_scale_override.map(|m| m.as_u8()),
        "motion_speed_override":  watchlist.motion_speed_override.map(|m| m.as_u8()),
        "settings": {
            "font_scale": watchlist.font_scale,
            "font_idx": watchlist.font_idx,
            "compact_mode": watchlist.compact_mode,
            "pane_header_size": phs,
            "toolbar_auto_hide": watchlist.toolbar_auto_hide,
            "show_x_axis": watchlist.show_x_axis,
            "show_y_axis": watchlist.show_y_axis,
            "shared_x_axis": watchlist.shared_x_axis,
            "shared_y_axis": watchlist.shared_y_axis,
            "pane_split_h": watchlist.pane_split_h,
            "pane_split_v": watchlist.pane_split_v,
            "pane_split_h2": watchlist.pane_split_h2,
            "pane_split_v2": watchlist.pane_split_v2,
            "pane_split_v3": watchlist.pane_split_v3,
            "pane_split_v4": watchlist.pane_split_v4,
            "pane_split_v5": watchlist.pane_split_v5,
            "pane_split_v6": watchlist.pane_split_v6,
        },
    });
    let _ = crate::state::persistence::atomic_write(
        &state_path(),
        serde_json::to_string_pretty(&state).unwrap_or_default().as_bytes(),
    );

    // ── Persist alerts ──
    save_alerts(watchlist, panes);
    // ── Persist hotkeys ──
    save_hotkeys(watchlist);
    // ── Persist templates ──
    save_templates(&watchlist.pane_templates);
}

/// Loaded global settings (applied to Watchlist after load)
pub(crate) struct LoadedSettings {
    pub(crate) font_scale: f32,
    pub(crate) font_idx: usize,
    pub(crate) compact_mode: bool,
    pub(crate) pane_header_size: crate::chart_renderer::PaneHeaderSize,
    pub(crate) toolbar_auto_hide: bool,
    pub(crate) show_x_axis: bool, pub(crate) show_y_axis: bool,
    pub(crate) shared_x_axis: bool, pub(crate) shared_y_axis: bool,
    pub(crate) pane_split_h: f32, pub(crate) pane_split_v: f32, pub(crate) pane_split_h2: f32, pub(crate) pane_split_v2: f32,
    pub(crate) pane_split_v3: f32, pub(crate) pane_split_v4: f32, pub(crate) pane_split_v5: f32, pub(crate) pane_split_v6: f32,
    pub(crate) draw_favorites: Option<Vec<String>>,
    pub(crate) style_idx: usize,
    // P4.3 — DensityMode override (None = inherit from style preset).
    pub(crate) density_override: Option<crate::ui_kit::style::DensityMode>,
    // P5 — Border / Corner / Spacing / Motion overrides (None = inherit).
    pub(crate) border_weight_override: Option<crate::ui_kit::style::BorderWeight>,
    pub(crate) corner_scale_override:  Option<crate::ui_kit::style::CornerScale>,
    pub(crate) spacing_scale_override: Option<crate::ui_kit::style::SpacingScale>,
    pub(crate) motion_speed_override:  Option<crate::ui_kit::style::MotionSpeed>,
}
impl Default for LoadedSettings { fn default() -> Self { Self {
    font_scale: 1.6, font_idx: 0, compact_mode: false,
    pane_header_size: crate::chart_renderer::PaneHeaderSize::Compact,
    toolbar_auto_hide: false,
    show_x_axis: true, show_y_axis: true,
    shared_x_axis: false, shared_y_axis: false,
    pane_split_h: 0.5, pane_split_v: 0.5, pane_split_h2: 0.5, pane_split_v2: 0.5,
    pane_split_v3: 0.5, pane_split_v4: 0.5, pane_split_v5: 0.5, pane_split_v6: 0.5,
    draw_favorites: None,
    style_idx: 0,
    density_override: None,
    border_weight_override: None,
    corner_scale_override:  None,
    spacing_scale_override: None,
    motion_speed_override:  None,
}}}

pub(crate) fn load_state() -> (Vec<Chart>, Layout, LoadedSettings) {
    let path = state_path();
    let data = match std::fs::read_to_string(&path) {
        Ok(d) => d,
        Err(_) => return (vec![Chart::new()], Layout::One, LoadedSettings::default()),
    };
    let json: serde_json::Value = match serde_json::from_str(&data) {
        Ok(v) => v,
        Err(_) => return (vec![Chart::new()], Layout::One, LoadedSettings::default()),
    };

    // Wave 6 fix: read and branch on schema version so future migrations have a
    // hook.  Version 1/2 files lack the top-level `settings` block and several
    // per-pane fields added in v3 — apply safe defaults for those below.
    let schema_version = json.get("version").and_then(|v| v.as_u64()).unwrap_or(1) as u32;

    let layout = match json.get("layout").and_then(|v| v.as_str()).unwrap_or("1") {
        "2" => Layout::Two, "2H" => Layout::TwoH, "3" => Layout::Three, "4" => Layout::Four,
        "6" => Layout::Six, "6H" => Layout::SixH, "9" => Layout::Nine, _ => Layout::One,
    };
    let theme_idx = {
        let raw = json.get("theme_idx").and_then(|v| v.as_u64()).unwrap_or(5) as usize;
        raw.min(live_themes().read().unwrap_or_else(|e| e.into_inner()).len().saturating_sub(1))
    };
    let recents: Vec<(String, String)> = json.get("recent_symbols").and_then(|v| v.as_array()).map(|arr| {
        arr.iter().filter_map(|v| {
            let a = v.as_array()?;
            Some((a.first()?.as_str()?.to_string(), a.get(1)?.as_str()?.to_string()))
        }).collect()
    }).unwrap_or_default();

    let pane_arr = json.get("panes").and_then(|v| v.as_array());
    let mut panes = Vec::new();
    if let Some(arr) = pane_arr {
        for p in arr {
            let sym = p.get("symbol").and_then(|v| v.as_str()).unwrap_or("AAPL");
            let tf = p.get("timeframe").and_then(|v| v.as_str()).unwrap_or("5m");
            let mut chart = Chart::new_with(sym, tf);
            chart.theme_idx = theme_idx;
            chart.recent_symbols = recents.clone();
            chart.pending_symbol_change = Some(sym.to_string());

            // Restore toggle states
            let gb = |key: &str, def: bool| -> bool { p.get(key).and_then(|v| v.as_bool()).unwrap_or(def) };
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
            chart.swing_leg_mode = p.get("swing_leg_mode").and_then(|v| v.as_u64()).unwrap_or(0) as u8;
            chart.show_footprint = gb("show_footprint", false);
            chart.show_gamma = gb("show_gamma", false); chart.hit_highlight = gb("hit_highlight", false);
            chart.show_darkpool = gb("show_darkpool", false);
            chart.show_events = gb("show_events", false);
            chart.show_pnl_curve = gb("show_pnl_curve", false);
            chart.show_pattern_labels = gb("show_pattern_labels", true);
            chart.link_group = p.get("link_group").and_then(|v| v.as_u64()).unwrap_or(0) as u8;
            // v5: overlay toggles (command-palette "overlay:" actions)
            chart.show_vol_shelves     = gb("show_vol_shelves", false);
            chart.show_confluence      = gb("show_confluence", false);
            chart.show_momentum_heat   = gb("show_momentum_heat", false);
            chart.show_trend_strip     = gb("show_trend_strip", false);
            chart.show_breadth_tint    = gb("show_breadth_tint", false);
            chart.show_vol_cone        = gb("show_vol_cone", false);
            chart.show_price_memory    = gb("show_price_memory", false);
            chart.show_liquidity_voids = gb("show_liquidity_voids", false);
            chart.show_corr_ribbon     = gb("show_corr_ribbon", false);
            chart.show_analyst_targets = gb("show_analyst_targets", false);
            chart.show_pe_band         = gb("show_pe_band", false);
            chart.show_insider_trades  = gb("show_insider_trades", false);

            // Restore session shading settings
            chart.session_shading = gb("session_shading", false);
            chart.rth_start_minutes = p.get("rth_start_minutes").and_then(|v| v.as_u64()).unwrap_or(570) as u16;
            chart.rth_end_minutes = p.get("rth_end_minutes").and_then(|v| v.as_u64()).unwrap_or(960) as u16;
            chart.eth_bar_opacity = p.get("eth_bar_opacity").and_then(|v| v.as_f64()).unwrap_or(0.35) as f32;
            chart.session_bg_tint = gb("session_bg_tint", false);
            chart.session_bg_color = p.get("session_bg_color").and_then(|v| v.as_str()).unwrap_or("#1a1a2e").to_string();
            chart.session_bg_opacity = p.get("session_bg_opacity").and_then(|v| v.as_f64()).unwrap_or(0.15) as f32;
            chart.session_break_lines = gb("session_break_lines", true);

            // Restore candle mode
            chart.candle_mode = match p.get("candle_mode").and_then(|v| v.as_str()).unwrap_or("std") {
                "vln" => CandleMode::Violin, "grd" => CandleMode::Gradient, "vg" => CandleMode::ViolinGradient,
                "ha" => CandleMode::HeikinAshi, "line" => CandleMode::Line, "area" => CandleMode::Area,
                    "rnk" => CandleMode::Renko, "rng" => CandleMode::RangeBar, "tck" => CandleMode::TickBar,
                _ => CandleMode::Standard,
            };
            chart.alt.renko_brick = p.get("renko_brick_size").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
            chart.alt.range_size = p.get("range_bar_size").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
            chart.alt.tick_count = p.get("tick_bar_count").and_then(|v| v.as_u64()).unwrap_or(500) as u32;
            chart.alt.dirty = true; // force recompute on load
            // Restore volume profile mode
            chart.vp.mode = match p.get("vp_mode").and_then(|v| v.as_str()).unwrap_or("off") {
                "classic" => VolumeProfileMode::Classic, "heatmap" => VolumeProfileMode::Heatmap,
                "strip" => VolumeProfileMode::Strip, "clean" => VolumeProfileMode::Clean,
                _ => VolumeProfileMode::Off,
            };

            // Restore indicators
            if let Some(inds) = p.get("indicators").and_then(|v| v.as_array()) {
                chart.indicators.clear();
                for (idx, ind_json) in inds.iter().enumerate() {
                    let kind_label = ind_json.get("kind").and_then(|v| v.as_str()).unwrap_or("SMA");
                    let kind = IndicatorType::all().iter().find(|t| t.label() == kind_label).copied().unwrap_or(IndicatorType::SMA);
                    let period = ind_json.get("period").and_then(|v| v.as_u64()).unwrap_or(20) as usize;
                    let color = ind_json.get("color").and_then(|v| v.as_str()).unwrap_or(INDICATOR_COLORS[idx % INDICATOR_COLORS.len()]);
                    let visible = ind_json.get("visible").and_then(|v| v.as_bool()).unwrap_or(true);
                    let thickness = ind_json.get("thickness").and_then(|v| v.as_f64()).unwrap_or(1.5) as f32;
                    let id = chart.next_indicator_id; chart.next_indicator_id += 1;
                    let mut ind = Indicator::new(id, kind, period, color);
                    ind.visible = visible;
                    ind.thickness = thickness;
                    ind.param2 = ind_json.get("param2").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                    ind.param3 = ind_json.get("param3").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                    ind.param4 = ind_json.get("param4").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                    ind.source = ind_json.get("source").and_then(|v| v.as_u64()).unwrap_or(0) as u8;
                    ind.offset = ind_json.get("offset").and_then(|v| v.as_i64()).unwrap_or(0) as i16;
                    ind.ob_level = ind_json.get("ob_level").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                    ind.os_level = ind_json.get("os_level").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                    ind.source_tf = ind_json.get("source_tf").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    ind.line_style = match ind_json.get("line_style").and_then(|v| v.as_str()).unwrap_or("solid") {
                        "dashed" => LineStyle::Dashed, "dotted" => LineStyle::Dotted, _ => LineStyle::Solid,
                    };
                    // Band styling (BB, Keltner, etc.)
                    ind.upper_color = ind_json.get("upper_color").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    ind.lower_color = ind_json.get("lower_color").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    ind.fill_color_hex = ind_json.get("fill_color_hex").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    ind.upper_thickness = ind_json.get("upper_thickness").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                    ind.lower_thickness = ind_json.get("lower_thickness").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                    chart.indicators.push(ind);
                }
            }

            // Restore chart widgets
            if let Some(wv) = p.get("chart_widgets") {
                if let Ok(widgets) = serde_json::from_value::<Vec<super::ChartWidget>>(wv.clone()) {
                    chart.chart_widgets = widgets;
                    // Reset animation state (transient, not meaningful from disk)
                    for w in &mut chart.chart_widgets { w.anim_init = false; }
                }
            }

            // Option-pane state — restore the contract metadata so the pane
            // re-fetches via fetch_option_bars_background instead of trying
            // to load the (non-existent) display label as a stock symbol.
            chart.is_option = gb("is_option", false);
            chart.option_contract = p.get("option_contract").and_then(|v| v.as_str()).unwrap_or("").to_string();
            chart.option_strike   = p.get("option_strike").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
            chart.option_type     = p.get("option_type").and_then(|v| v.as_str()).unwrap_or("").to_string();
            chart.option_expiry   = p.get("option_expiry").and_then(|v| v.as_str()).unwrap_or("").to_string();
            chart.underlying      = p.get("underlying").and_then(|v| v.as_str()).unwrap_or("").to_string();
            // MARK_BARS_PROTOCOL — default to "last" on missing.
            chart.bar_source_mark = p.get("bar_source").and_then(|v| v.as_str()).unwrap_or("last") == "mark";
            // v4: per-pane DOM panel open state.
            chart.dom.open         = gb("dom_open", false);
            chart.dom.sidebar_open = gb("dom_sidebar_open", false);
            // v6: tab history
            if let Some(ts) = p.get("tab_symbols").and_then(|v| v.as_array()) {
                chart.tab_symbols = ts.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect();
            }
            if let Some(ts) = p.get("tab_timeframes").and_then(|v| v.as_array()) {
                chart.tab_timeframes = ts.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect();
            }
            chart.tab_active = p.get("tab_active").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            // v6: symbol overlays — bars re-fetched when pending_symbol_change fires
            if let Some(ovs) = p.get("symbol_overlays").and_then(|v| v.as_array()) {
                for ov in ovs {
                    let sym = ov.get("symbol").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    if sym.is_empty() { continue; }
                    chart.symbol_overlays.push(SymbolOverlay {
                        symbol: sym,
                        color: ov.get("color").and_then(|v| v.as_str()).unwrap_or("#FF5733").to_string(),
                        show_candles: ov.get("show_candles").and_then(|v| v.as_bool()).unwrap_or(false),
                        visible: ov.get("visible").and_then(|v| v.as_bool()).unwrap_or(true),
                        bars: vec![], timestamps: vec![], loading: true,
                    });
                }
            }

            panes.push(chart);
        }
    }
    if panes.is_empty() { panes.push(Chart::new()); }
    // Trim excess panes to match layout capacity
    let max = layout.max_panes();
    panes.truncate(max);

    // Wave 6 fix: apply per-version field defaults so older saved workspaces get
    // sensible values for fields that did not exist in those schema versions.
    // The per-pane loop above already uses `.unwrap_or(default)` for every field,
    // so v1/v2 → v3 migration requires no extra field writes today.
    // This branch is the hook for future v4+ migrations.
    #[allow(clippy::match_single_binding)]
    match schema_version {
        1 | 2 => { /* v1/v2: settings block absent — LoadedSettings::default() covers it */ }
        _ => { /* v3+ is the current format — no migration required */ }
    }

    // Restore global settings (version 3+)
    let mut settings = LoadedSettings::default();
    if let Some(s) = json.get("settings") {
        settings.font_scale = s.get("font_scale").and_then(|v| v.as_f64()).unwrap_or(1.6) as f32;
        settings.font_idx = s.get("font_idx").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
        settings.compact_mode = s.get("compact_mode").and_then(|v| v.as_bool()).unwrap_or(false);
        settings.pane_header_size = match s.get("pane_header_size").and_then(|v| v.as_str()).unwrap_or("compact") {
            "normal" => crate::chart_renderer::PaneHeaderSize::Normal,
            "expanded" => crate::chart_renderer::PaneHeaderSize::Expanded,
            _ => crate::chart_renderer::PaneHeaderSize::Compact,
        };
        settings.toolbar_auto_hide = s.get("toolbar_auto_hide").and_then(|v| v.as_bool()).unwrap_or(false);
        settings.show_x_axis = s.get("show_x_axis").and_then(|v| v.as_bool()).unwrap_or(true);
        settings.show_y_axis = s.get("show_y_axis").and_then(|v| v.as_bool()).unwrap_or(true);
        settings.shared_x_axis = s.get("shared_x_axis").and_then(|v| v.as_bool()).unwrap_or(false);
        settings.shared_y_axis = s.get("shared_y_axis").and_then(|v| v.as_bool()).unwrap_or(false);
        settings.pane_split_h = s.get("pane_split_h").and_then(|v| v.as_f64()).unwrap_or(0.5) as f32;
        settings.pane_split_v = s.get("pane_split_v").and_then(|v| v.as_f64()).unwrap_or(0.5) as f32;
        settings.pane_split_h2 = s.get("pane_split_h2").and_then(|v| v.as_f64()).unwrap_or(0.5) as f32;
        settings.pane_split_v2 = s.get("pane_split_v2").and_then(|v| v.as_f64()).unwrap_or(0.5) as f32;
        settings.pane_split_v3 = s.get("pane_split_v3").and_then(|v| v.as_f64()).unwrap_or(0.5) as f32;
        settings.pane_split_v4 = s.get("pane_split_v4").and_then(|v| v.as_f64()).unwrap_or(0.5) as f32;
        settings.pane_split_v5 = s.get("pane_split_v5").and_then(|v| v.as_f64()).unwrap_or(0.5) as f32;
        settings.pane_split_v6 = s.get("pane_split_v6").and_then(|v| v.as_f64()).unwrap_or(0.5) as f32;
    }
    // Drawing-tool favorites — top-level key (added independently of settings).
    if let Some(arr) = json.get("draw_favorites").and_then(|v| v.as_array()) {
        let favs: Vec<String> = arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect();
        if !favs.is_empty() { settings.draw_favorites = Some(favs); }
    }
    // Style index — top-level key, clamped to known list.
    if let Some(s) = json.get("style_idx").and_then(|v| v.as_u64()) {
        settings.style_idx = (s as usize).min(STYLE_NAMES.len().saturating_sub(1));
    }
    // P4.3 + P5 — user token-scale overrides. Each is a top-level key,
    // u8 enum index (0..=N) or null. Missing/null/out-of-range → None
    // (inherit from the active style preset). All four use the same shape.
    settings.density_override = json.get("density_override")
        .and_then(|v| v.as_u64())
        .and_then(|n| if n <= 2 { Some(crate::ui_kit::style::DensityMode::from_u8(n as u8)) } else { None });
    settings.border_weight_override = json.get("border_weight_override")
        .and_then(|v| v.as_u64())
        .and_then(|n| if n <= 2 { Some(crate::ui_kit::style::BorderWeight::from_u8(n as u8)) } else { None });
    settings.corner_scale_override = json.get("corner_scale_override")
        .and_then(|v| v.as_u64())
        .and_then(|n| if n <= 3 { Some(crate::ui_kit::style::CornerScale::from_u8(n as u8)) } else { None });
    settings.spacing_scale_override = json.get("spacing_scale_override")
        .and_then(|v| v.as_u64())
        .and_then(|n| if n <= 2 { Some(crate::ui_kit::style::SpacingScale::from_u8(n as u8)) } else { None });
    settings.motion_speed_override = json.get("motion_speed_override")
        .and_then(|v| v.as_u64())
        .and_then(|n| if n <= 3 { Some(crate::ui_kit::style::MotionSpeed::from_u8(n as u8)) } else { None });

    (panes, layout, settings)
}
