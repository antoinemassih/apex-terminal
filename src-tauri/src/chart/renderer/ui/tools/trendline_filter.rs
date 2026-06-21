//! Trendline Filter UI component.

use egui;
use crate::ui_kit::sx::Tone;
use super::super::style::*;
use super::super::super::gpu::*;
use super::super::components::text::MonospaceCode;
use crate::ui_kit::widgets::Button;
use crate::ui_kit::widgets::tokens::Variant;
use crate::ui_kit::icons::Icon;
use crate::monitoring::{span_begin, span_end};
use crate::chart_renderer::DrawingKind;
use crate::ui_kit::widgets::modal::{Modal, Anchor, HeaderStyle};

pub(crate) fn draw(ctx: &egui::Context, watchlist: &mut Watchlist, panes: &mut [Chart], ap: usize, t: &Theme) {
// ── Trendline filter dropdown ────────────────────────────────────────────
if watchlist.trendline_filter_open {
    use crate::chart_renderer::ui::chrome::FloatingPaneChrome;
    let mut close_clicked = false;
    // retained as Window: wraps FloatingPaneChrome with custom chrome; Modal would double-render headers
    egui::Window::new("trendline_filter")
        .fixed_pos(egui::pos2(300.0, 40.0))
        .fixed_size(egui::vec2(220.0, 0.0))
        .title_bar(false)
        .frame(egui::Frame::NONE)
        .show(ctx, |ui| {
            let cr = FloatingPaneChrome::new(0xF11_7E2, "DRAWING FILTERS")
                .leading_icon(Icon::FUNNEL)
                .width(220.0)
                .theme(t)
                .show(ui, |ui| {
            let m = 8.0;

            // ── Auto-charting (engine-side detection control) ──────────────────
            // Drives what ApexSignals computes (sent as query params) + re-fetch.
            dialog_section(ui, "AUTO-CHARTING", m, color_half(t.dim));
            let mut cfg = auto_draw_config();
            let before = cfg.clone();
            let (sym, tf) = { let c = &panes[ap]; (c.symbol.clone(), c.timeframe.clone()) };
            ui.horizontal(|ui| {
                ui.add_space(m);
                ui.checkbox(&mut cfg.enabled, "On");
                ui.checkbox(&mut cfg.live_feed, "Live feed")
                    .on_hover_text("Use the apex-data unified drawing feed (updates live as the backend recomputes across the universe) instead of the tuned per-chart fetch");
            });
            if cfg.enabled {
                ui.horizontal(|ui| {
                    ui.add_space(m);
                    ui.checkbox(&mut cfg.trendlines, "Trendlines");
                    ui.checkbox(&mut cfg.channels, "Channels");
                });
                ui.horizontal(|ui| {
                    ui.add_space(m);
                    ui.checkbox(&mut cfg.levels, "Levels");
                    ui.checkbox(&mut cfg.patterns, "Patterns");
                    ui.checkbox(&mut cfg.candles, "Candles");
                });
                ui.horizontal(|ui| {
                    ui.add_space(m);
                    ui.label("Pivots:");
                    for mode in ["hybrid", "atr", "percent"] {
                        if ui.selectable_label(cfg.pivot_mode == mode, mode).clicked() {
                            cfg.pivot_mode = mode.to_string();
                        }
                    }
                });
                ui.horizontal(|ui| { ui.add_space(m); ui.add(egui::Slider::new(&mut cfg.atr_k, 0.5..=6.0).text("atr_k")); });
                ui.horizontal(|ui| { ui.add_space(m); ui.add(egui::Slider::new(&mut cfg.pct, 0.0..=0.05).text("pct")); });
                ui.horizontal(|ui| { ui.add_space(m); ui.add(egui::Slider::new(&mut cfg.min_touches, 2..=6).text("min touches")); });
                ui.horizontal(|ui| { ui.add_space(m); ui.add(egui::Slider::new(&mut cfg.max_lines, 4..=30).text("max lines")); });
            }
            if cfg != before {
                set_auto_draw_config(cfg);
                fetch_apexsignals_drawings(sym, tf);
            }
            ui.add_space(gap_sm());
            dialog_separator_shadow(ui, m, tint(t, Tone::Border, alpha_line()));
            ui.add_space(gap_sm());

            let chart = &mut panes[ap];

            // Per-type visibility toggles
            dialog_section(ui, "BY TYPE", m, color_half(t.dim));
            let types = [("trendline", "Trendlines"), ("hline", "H-Lines"), ("hzone", "Zones"), ("barmarker", "Markers"), ("fibonacci", "Fibonacci"), ("channel", "Channels"), ("fibchannel", "Fib Channels")];
            for (dtype, label) in &types {
                let count = chart.drawings.iter().filter(|d| {
                    match (dtype, &d.kind) {
                        (&"trendline", DrawingKind::TrendLine{..}) => true,
                        (&"hline", DrawingKind::HLine{..}) => true,
                        (&"hzone", DrawingKind::HZone{..}) => true,
                        (&"barmarker", DrawingKind::BarMarker{..}) => true,
                        (&"fibonacci", DrawingKind::Fibonacci{..}) => true,
                        (&"channel", DrawingKind::Channel{..}) => true,
                        (&"fibchannel", DrawingKind::FibChannel{..}) => true,
                        _ => false,
                    }
                }).count();
                ui.horizontal(|ui| {
                    ui.add_space(m);
                    let type_label = format!("{} ({})", label, count);
                    ui.add(MonospaceCode::new(&type_label).size_px(9.0).color(color_subtle(t.dim)));
                });
            }

            ui.add_space(gap_sm());
            dialog_separator_shadow(ui, m, tint(t, Tone::Border, alpha_line()));
            ui.add_space(gap_sm());

            // Visibility toggles
            dialog_section(ui, "VISIBILITY", m, color_half(t.dim));
            let vis_btn = |ui: &mut egui::Ui, hidden: bool, label: &str, count: usize| -> bool {
                let icon = if hidden { Icon::EYE_SLASH } else { Icon::EYE };
                let fg = if hidden { color_dim(t.dim) } else { t.dim };
                let vis_label = format!("{} {} ({})", icon, label, count);
                ui.horizontal(|ui| {
                    ui.add_space(m);
                    ui.add(Button::new(vis_label.as_str()).variant(Variant::Secondary).simple_treatment(true).fg(fg))
                        .clicked()
                }).inner
            };
            let sig_count = chart.signal_drawings.len();
            if vis_btn(ui, chart.hide_signal_drawings, "Signals", sig_count) {
                chart.hide_signal_drawings = !chart.hide_signal_drawings;
            }
            if vis_btn(ui, chart.hide_all_drawings, "All Drawings", chart.drawings.len()) {
                chart.hide_all_drawings = !chart.hide_all_drawings;
            }

            // Per-detection-method filters — auto-charting emits a line per method
            // (wick / ransac / kalman / hough / kde / cusum / pca / bayesian / …),
            // each tagged so it can be shown or hidden independently.
            let methods: Vec<(String, usize)> = {
                let mut counts: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
                for sd in &chart.signal_drawings {
                    if sd.detection_method.is_empty() { continue; }
                    *counts.entry(sd.detection_method.clone()).or_insert(0) += 1;
                }
                counts.into_iter().collect()
            };
            if !methods.is_empty() {
                ui.add_space(gap_sm());
                dialog_separator_shadow(ui, m, tint(t, Tone::Border, alpha_line()));
                ui.add_space(gap_sm());
                dialog_section(ui, "BY METHOD", m, color_half(t.dim));
                for (method, count) in &methods {
                    let hidden = chart.hidden_signal_methods.iter().any(|x| x == method);
                    // Toggle on the raw method string; display a human label.
                    if vis_btn(ui, hidden, &method_label(method), *count) {
                        if hidden { chart.hidden_signal_methods.retain(|x| x != method); }
                        else { chart.hidden_signal_methods.push(method.clone()); }
                    }
                }
            }

            // Groups
            if !chart.groups.is_empty() {
                ui.add_space(gap_sm());
                dialog_separator_shadow(ui, m, tint(t, Tone::Border, alpha_line()));
                ui.add_space(gap_sm());
                dialog_section(ui, "GROUPS", m, color_half(t.dim));
                for g in chart.groups.clone() {
                    let hidden = chart.hidden_groups.contains(&g.id);
                    let count = chart.drawings.iter().filter(|d| d.group_id == g.id).count();
                    if vis_btn(ui, hidden, &g.name, count) {
                        if hidden { chart.hidden_groups.retain(|x| x != &g.id); }
                        else { chart.hidden_groups.push(g.id.clone()); }
                    }
                }
            }
            ui.add_space(gap_sm());
                });
            close_clicked = cr.close_clicked;
        });
    if close_clicked { watchlist.update_sidebar_state(|s| s.trendline_filter_open = false); }
}

// Symbol picker popup — render for any pane that has it open
span_begin("symbol_picker");
for picker_pane_idx in 0..panes.len() {
let chart = &mut panes[picker_pane_idx];
if chart.picker.open {
    let mut close_picker = false;
    let mut new_symbol: Option<(String, String)> = None; // (symbol, name)

    // Check for background search results
    if let Some(rx) = &chart.picker.rx {
        if let Ok(results) = rx.try_recv() {
            chart.picker.results = results;
            chart.picker.searching = false;
        }
    }

    // Launch search when query changes
    if chart.picker.query != chart.picker.last_query {
        chart.picker.last_query = chart.picker.query.clone();
        let q = chart.picker.query.trim().to_string();

        if q.is_empty() {
            // Empty query: show recents + popular from static list
            chart.picker.results.clear();
            chart.picker.searching = false;
            chart.picker.rx = None;
        } else {
            // Immediate: show static matches while Yahoo search runs
            let static_results: Vec<(String, String, String)> = crate::ui_kit::symbols::search_symbols(&q, 10)
                .iter().map(|s| (s.symbol.to_string(), s.name.to_string(), String::new())).collect();
            chart.picker.results = static_results;

            // Fire background search: ApexIB first, Yahoo fallback
            chart.picker.searching = true;
            let (tx, rx) = std::sync::mpsc::channel();
            chart.picker.rx = Some(rx);
            let query = q.clone();
            std::thread::spawn(move || {
                let client = reqwest::blocking::Client::builder()
                    .user_agent("Mozilla/5.0")
                    .timeout(std::time::Duration::from_secs(3))
                    .build().unwrap_or_else(|_| reqwest::blocking::Client::new());
                let mut results: Vec<(String, String, String)> = Vec::new();

                // Try ApexIB search first — URL-encode the user query to prevent injection.
                let apexib_url = format!("{}/search/{}", APEXIB_URL, urlencoding::encode(&query));
                if let Ok(resp) = client.get(&apexib_url).send() {
                    if resp.status().is_success() {
                        if let Ok(json) = resp.json::<serde_json::Value>() {
                            if let Some(arr) = json.as_array() {
                                for item in arr.iter().take(MAX_SEARCH_RESULTS) {
                                    if let Some(sym) = item.get("symbol").and_then(|v| v.as_str()) {
                                        let name = item.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                        let sec_type = item.get("secType").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                        results.push((sym.to_string(), name, sec_type));
                                    }
                                }
                            }
                        }
                    }
                }

                // Fallback: Yahoo Finance search API
                if results.is_empty() {
                    let url = format!(
                        "https://query2.finance.yahoo.com/v1/finance/search?q={}&quotesCount=15&newsCount=0",
                        query
                    );
                    if let Ok(resp) = client.get(&url).send() {
                        if let Ok(json) = resp.json::<serde_json::Value>() {
                            if let Some(quotes) = json.get("quotes").and_then(|q| q.as_array()) {
                                for q in quotes.iter().take(MAX_SEARCH_RESULTS) {
                                    if let Some(sym) = q.get("symbol").and_then(|s| s.as_str()) {
                                        let name = q.get("shortname").or_else(|| q.get("longname"))
                                            .and_then(|n| n.as_str()).unwrap_or("").to_string();
                                        let exchange = q.get("exchDisp").and_then(|e| e.as_str()).unwrap_or("").to_string();
                                        let type_disp = q.get("typeDisp").and_then(|t| t.as_str()).unwrap_or("").to_string();
                                        let tag = if !exchange.is_empty() && !type_disp.is_empty() {
                                            format!("{} · {}", exchange, type_disp)
                                        } else if !exchange.is_empty() { exchange }
                                        else { type_disp };
                                        results.push((sym.to_string(), name, tag));
                                    }
                                }
                            }
                        }
                    }
                }

                // If both returned nothing, use static
                if results.is_empty() {
                    results = crate::ui_kit::symbols::search_symbols(&query, MAX_SEARCH_RESULTS)
                        .iter().map(|s| (s.symbol.to_string(), s.name.to_string(), String::new())).collect();
                }
                let _  = tx.send(results);
            });
        }
    }

    // Migrated to ToolPopover (2026-05-26).
    let portable_t = crate::chart_renderer::theme_impl::theme_to_portable(t);
    let picker_id = format!("picker_{}", picker_pane_idx);
    let picker_win_resp = crate::ui_kit::widgets::ToolPopover::new()
        .id(&picker_id)
        .width(320.0)
        .pos(chart.picker.pos)
        .show(ctx, &portable_t, |ui| {
            let input = crate::ui_kit::widgets::Input::new(&mut chart.picker.query)
                    .placeholder("Search any stock, ETF, index...")
                    .width(300.0)
                    .font_size(11.0)
                    .show(ui, t);
            input.request_focus(ui.ctx());

            if chart.picker.searching {
                ui.horizontal(|ui| {
                    crate::ui_kit::widgets::Spinner::new().show(ui, t);
                    ui.add(MonospaceCode::new("Searching...").size_px(9.0).color(t.dim));
                });
            }

            ui.separator();

            egui::ScrollArea::vertical().max_height(370.0).show(ui, |ui| {
                let show_recents = chart.picker.query.trim().is_empty();

                if show_recents && !chart.recent_symbols.is_empty() {
                    ui.add(MonospaceCode::new("RECENT").size_px(9.0).color(t.dim));
                    ui.add_space(gap_xs());
                    for (sym, name) in chart.recent_symbols.clone() {
                        let is_current = sym == chart.symbol;
                        let resp = ui.horizontal(|ui| {
                            let sym_col = if is_current { t.bull } else { t.text };
                            let r = ui.add(Button::new(sym.as_str()).variant(Variant::Secondary).simple_treatment(true).fg(sym_col).min_size(egui::vec2(65.0, 0.0)));
                            ui.add(MonospaceCode::new(&name).size_px(9.0).color(t.dim));
                            r
                        }).inner;
                        if resp.clicked() {
                            new_symbol = Some((sym.clone(), name.clone()));
                            close_picker = true;
                        }
                    }
                    ui.add_space(gap_sm());
                    ui.separator();
                    ui.add_space(gap_xs());
                    ui.add(MonospaceCode::new("POPULAR").size_px(9.0).color(t.dim));
                    ui.add_space(gap_xs());
                    // Show popular symbols from static catalog
                    for s in crate::ui_kit::symbols::search_symbols("", 20) {
                        if chart.recent_symbols.iter().any(|(r, _)| r == s.symbol) { continue; }
                        let is_current = s.symbol == chart.symbol;
                        let resp = ui.horizontal(|ui| {
                            let sym_col = if is_current { t.bull } else { color_subtle(t.dim) };
                            let r = ui.add(Button::new(s.symbol).variant(Variant::Secondary).simple_treatment(true).fg(sym_col).min_size(egui::vec2(65.0, 0.0)));
                            ui.add(MonospaceCode::new(s.name).size_px(9.0).color(t.dim));
                            r
                        }).inner;
                        if resp.clicked() {
                            new_symbol = Some((s.symbol.to_string(), s.name.to_string()));
                            close_picker = true;
                        }
                    }
                } else {
                    // Search results
                    for (sym, name, tag) in &chart.picker.results {
                        let is_current = sym == &chart.symbol;
                        let resp = ui.horizontal(|ui| {
                            let sym_col = if is_current { t.bull } else { t.text };
                            let r = ui.add(Button::new(sym.as_str()).variant(Variant::Secondary).simple_treatment(true).fg(sym_col).min_size(egui::vec2(65.0, 0.0)));
                            ui.vertical(|ui| {
                                ui.add(MonospaceCode::new(name.as_str()).size_px(9.0).color(t.dim));
                                if !tag.is_empty() {
                                    ui.add(MonospaceCode::new(tag.as_str()).size_px(9.0).color(color_muted(t.dim)));
                                }
                            });
                            r
                        }).inner;
                        if resp.clicked() {
                            new_symbol = Some((sym.clone(), name.clone()));
                            close_picker = true;
                        }
                    }
                    if chart.picker.results.is_empty() && !chart.picker.searching && !chart.picker.query.trim().is_empty() {
                        ui.add(MonospaceCode::new("No results").size_px(9.0).color(t.dim));
                    }
                }
            });

            if ui.input(|i| i.key_pressed(egui::Key::Escape)) { close_picker = true; }
            if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                if let Some((sym, name, _)) = chart.picker.results.first() {
                    new_symbol = Some((sym.clone(), name.clone()));
                    close_picker = true;
                }
            }
        });

    // Click-away is handled by Modal::close_on_click_outside; also honour manual close flag.
    if picker_win_resp.dismissed { close_picker = true; }
    if close_picker { chart.picker.open = false; }

    if let Some((sym, name)) = new_symbol {
        // Add to recents (move to front if already there)
        chart.recent_symbols.retain(|(s, _)| s != &sym);
        chart.recent_symbols.insert(0, (sym.clone(), name));
        if chart.recent_symbols.len() > MAX_RECENT_SYMBOLS { chart.recent_symbols.truncate(MAX_RECENT_SYMBOLS); }
        chart.pending_symbol_change = Some(sym);
    }
}
} // end for picker_pane_idx
span_end();

// Old global style_bar removed — unified into per-pane draw_props bar


}

/// Human-readable label for an auto-chart `detection_method` string (the engine
/// emits machine names like `kde_level`, `wavelet_L1`, `pattern:HeadAndShoulders`).
fn method_label(m: &str) -> String {
    match m {
        "wick" => "Wick Trendline".into(),
        "body" => "Body Trendline".into(),
        "inner" => "Inner Trendline".into(),
        "anchored" => "Anchored".into(),
        "volume_weighted" => "Volume-Weighted".into(),
        "regression" => "Regression".into(),
        "kalman" => "Kalman".into(),
        "kde_level" => "KDE Level".into(),
        "cusum" => "CUSUM".into(),
        "pca" => "PCA".into(),
        "tls" => "Total Least Squares".into(),
        "svd" => "SVD".into(),
        "hough" => "Hough".into(),
        "ransac" => "RANSAC".into(),
        "bayesian_cp" => "Bayesian Change-Pt".into(),
        "bayesian_lr" => "Bayesian Regression".into(),
        other => {
            if let Some(n) = other.strip_prefix("wavelet_L") {
                format!("Wavelet L{n}")
            } else if let Some(r) = other.strip_prefix("fib_fan_") {
                format!("Fib Fan {r}")
            } else if let Some(r) = other.strip_prefix("speed_") {
                format!("Speed {r}")
            } else if let Some(p) = other.strip_prefix("pattern:") {
                prettify_camel(p)
            } else {
                other.to_string()
            }
        }
    }
}

/// "HeadAndShoulders" → "Head And Shoulders" for pattern labels.
fn prettify_camel(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 4);
    for (i, c) in s.chars().enumerate() {
        if i > 0 && c.is_uppercase() {
            out.push(' ');
        }
        out.push(c);
    }
    out
}
