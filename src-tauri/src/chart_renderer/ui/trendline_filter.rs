//! Trendline Filter UI component.

use egui;
use super::style::*;
use super::super::gpu::*;
use super::widgets::buttons::{SimpleBtn, IconBtn};
use super::widgets::text::MonospaceCode;
use crate::ui_kit::icons::Icon;
use crate::monitoring::{span_begin, span_end};
use crate::chart_renderer::DrawingKind;
use crate::chart_renderer::LineStyle;
const fn rgb(r: u8, g: u8, b: u8) -> egui::Color32 { egui::Color32::from_rgb(r, g, b) }

pub(crate) fn draw(ctx: &egui::Context, watchlist: &mut Watchlist, panes: &mut [Chart], ap: usize, t: &Theme) {
// ── Trendline filter dropdown ────────────────────────────────────────────
if watchlist.trendline_filter_open {
    dialog_window_themed(ctx, "trendline_filter", egui::pos2(300.0, 40.0), 190.0, t.toolbar_bg, t.toolbar_border, None)
        .show(ctx, |ui| {
            if dialog_header(ui, "DRAWING FILTERS", t.dim) { watchlist.trendline_filter_open = false; }
            ui.add_space(6.0);
            let m = 8.0;
            let chart = &mut panes[ap];

            // Per-type visibility toggles
            dialog_section(ui, "BY TYPE", m, t.dim.gamma_multiply(0.5));
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
                    ui.add(MonospaceCode::new(&type_label).size_px(9.0).color(egui::Color32::from_rgb(200,200,210)));
                });
            }

            ui.add_space(6.0);
            dialog_separator_shadow(ui, m, color_alpha(t.toolbar_border, alpha_line()));
            ui.add_space(6.0);

            // Visibility toggles
            dialog_section(ui, "VISIBILITY", m, t.dim.gamma_multiply(0.5));
            let vis_btn = |ui: &mut egui::Ui, hidden: bool, label: &str, count: usize| -> bool {
                let icon = if hidden { Icon::EYE_SLASH } else { Icon::EYE };
                let fg = if hidden { t.dim.gamma_multiply(0.4) } else { t.dim };
                let vis_label = format!("{} {} ({})", icon, label, count);
                ui.horizontal(|ui| {
                    ui.add_space(m);
                    ui.add(SimpleBtn::new(&vis_label).color(fg))
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

            // Groups
            if !chart.groups.is_empty() {
                ui.add_space(6.0);
                dialog_separator_shadow(ui, m, color_alpha(t.toolbar_border, alpha_line()));
                ui.add_space(6.0);
                dialog_section(ui, "GROUPS", m, t.dim.gamma_multiply(0.5));
                for g in chart.groups.clone() {
                    let hidden = chart.hidden_groups.contains(&g.id);
                    let count = chart.drawings.iter().filter(|d| d.group_id == g.id).count();
                    if vis_btn(ui, hidden, &g.name, count) {
                        if hidden { chart.hidden_groups.retain(|x| x != &g.id); }
                        else { chart.hidden_groups.push(g.id.clone()); }
                    }
                }
            }
            ui.add_space(6.0);
        });
}

// Symbol picker popup — render for any pane that has it open
span_begin("symbol_picker");
for picker_pane_idx in 0..panes.len() {
let chart = &mut panes[picker_pane_idx];
if chart.picker_open {
    let mut close_picker = false;
    let mut new_symbol: Option<(String, String)> = None; // (symbol, name)

    // Check for background search results
    if let Some(rx) = &chart.picker_rx {
        if let Ok(results) = rx.try_recv() {
            chart.picker_results = results;
            chart.picker_searching = false;
        }
    }

    // Launch search when query changes
    if chart.picker_query != chart.picker_last_query {
        chart.picker_last_query = chart.picker_query.clone();
        let q = chart.picker_query.trim().to_string();

        if q.is_empty() {
            // Empty query: show recents + popular from static list
            chart.picker_results.clear();
            chart.picker_searching = false;
            chart.picker_rx = None;
        } else {
            // Immediate: show static matches while Yahoo search runs
            let static_results: Vec<(String, String, String)> = crate::ui_kit::symbols::search_symbols(&q, 10)
                .iter().map(|s| (s.symbol.to_string(), s.name.to_string(), String::new())).collect();
            chart.picker_results = static_results;

            // Fire background search: ApexIB first, Yahoo fallback
            chart.picker_searching = true;
            let (tx, rx) = std::sync::mpsc::channel();
            chart.picker_rx = Some(rx);
            let query = q.clone();
            std::thread::spawn(move || {
                let client = reqwest::blocking::Client::builder()
                    .user_agent("Mozilla/5.0")
                    .timeout(std::time::Duration::from_secs(3))
                    .build().unwrap_or_else(|_| reqwest::blocking::Client::new());
                let mut results: Vec<(String, String, String)> = Vec::new();

                // Try ApexIB search first
                let apexib_url = format!("{}/search/{}", APEXIB_URL, query);
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

    let picker_win_resp = egui::Window::new(format!("picker_{}", picker_pane_idx))
        .fixed_pos(chart.picker_pos)
        .fixed_size(egui::vec2(320.0, 420.0))
        .title_bar(false)
        .frame(egui::Frame::popup(&ctx.style())
            .fill(t.toolbar_bg)
            .stroke(egui::Stroke::new(stroke_thin(), color_alpha(t.toolbar_border, alpha_strong())))
            .corner_radius(r_lg_cr())
            .inner_margin(egui::Margin::same(6)))
        .show(ctx, |ui| {
            let input = super::widgets::inputs::TextInput::new(&mut chart.picker_query)
                    .placeholder("Search any stock, ETF, index...")
                    .width(300.0)
                    .font_size(11.0)
                    .show(ui);
            input.request_focus();

            if chart.picker_searching {
                ui.horizontal(|ui| {
                    super::chart_widgets::refined_spinner(ui, t.accent);
                    ui.add(MonospaceCode::new("Searching...").size_px(9.0).color(t.dim));
                });
            }

            ui.separator();

            egui::ScrollArea::vertical().max_height(370.0).show(ui, |ui| {
                let show_recents = chart.picker_query.trim().is_empty();

                if show_recents && !chart.recent_symbols.is_empty() {
                    ui.add(MonospaceCode::new("RECENT").size_px(9.0).color(t.dim));
                    ui.add_space(2.0);
                    for (sym, name) in chart.recent_symbols.clone() {
                        let is_current = sym == chart.symbol;
                        let resp = ui.horizontal(|ui| {
                            let sym_col = if is_current { t.bull } else { TEXT_PRIMARY };
                            let r = ui.add(SimpleBtn::new(&sym).color(sym_col).min_width(65.0));
                            ui.add(MonospaceCode::new(&name).size_px(9.0).color(t.dim));
                            r
                        }).inner;
                        if resp.clicked() {
                            new_symbol = Some((sym.clone(), name.clone()));
                            close_picker = true;
                        }
                    }
                    ui.add_space(6.0);
                    ui.separator();
                    ui.add_space(2.0);
                    ui.add(MonospaceCode::new("POPULAR").size_px(9.0).color(t.dim));
                    ui.add_space(2.0);
                    // Show popular symbols from static catalog
                    for s in crate::ui_kit::symbols::search_symbols("", 20) {
                        if chart.recent_symbols.iter().any(|(r, _)| r == s.symbol) { continue; }
                        let is_current = s.symbol == chart.symbol;
                        let resp = ui.horizontal(|ui| {
                            let sym_col = if is_current { t.bull } else { egui::Color32::from_rgb(200,200,210) };
                            let r = ui.add(SimpleBtn::new(s.symbol).color(sym_col).min_width(65.0));
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
                    for (sym, name, tag) in &chart.picker_results {
                        let is_current = sym == &chart.symbol;
                        let resp = ui.horizontal(|ui| {
                            let sym_col = if is_current { t.bull } else { TEXT_PRIMARY };
                            let r = ui.add(SimpleBtn::new(sym.as_str()).color(sym_col).min_width(65.0));
                            ui.vertical(|ui| {
                                ui.add(MonospaceCode::new(name.as_str()).size_px(9.0).color(egui::Color32::from_rgb(180,180,190)));
                                if !tag.is_empty() {
                                    ui.add(MonospaceCode::new(tag.as_str()).size_px(9.0).color(egui::Color32::from_rgb(100,100,120)));
                                }
                            });
                            r
                        }).inner;
                        if resp.clicked() {
                            new_symbol = Some((sym.clone(), name.clone()));
                            close_picker = true;
                        }
                    }
                    if chart.picker_results.is_empty() && !chart.picker_searching && !chart.picker_query.trim().is_empty() {
                        ui.add(MonospaceCode::new("No results").size_px(9.0).color(t.dim));
                    }
                }
            });

            if ui.input(|i| i.key_pressed(egui::Key::Escape)) { close_picker = true; }
            if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                if let Some((sym, name, _)) = chart.picker_results.first() {
                    new_symbol = Some((sym.clone(), name.clone()));
                    close_picker = true;
                }
            }
        });

    // Click-away closes picker
    if !close_picker {
        if let Some(wr) = &picker_win_resp {
            let picker_rect = wr.response.rect;
            if ctx.input(|i| i.pointer.any_pressed()) {
                if let Some(pos) = ctx.input(|i| i.pointer.interact_pos()) {
                    if !picker_rect.contains(pos) {
                        close_picker = true;
                    }
                }
            }
        }
    }

    if close_picker { chart.picker_open = false; }

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
