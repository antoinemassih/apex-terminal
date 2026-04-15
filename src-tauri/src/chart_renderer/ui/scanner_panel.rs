//! Scanner panel — Market Movers & custom scanners.
//!
//! Shows collapsible scanner sections (Top Gainers, Top Losers, Most Active)
//! populated from bulk quote data. Each symbol row is clickable to load a chart.
//! Includes "Save as Watchlist" and a custom scanner builder.

use egui;
use super::style::{close_button, separator, color_alpha, simple_btn, col_header};
use super::super::gpu::*;
use crate::ui_kit::icons::Icon;

const REFRESH_INTERVAL_SECS: u64 = 30;

/// Apply a scanner definition to the raw result pool and return filtered+sorted results.
fn apply_scanner(def: &ScannerDef, pool: &[ScanResult]) -> Vec<ScanResult> {
    let mut filtered: Vec<ScanResult> = pool.iter()
        .filter(|r| r.price > 0.0) // exclude unfetched
        .filter(|r| r.change_pct >= def.min_change && r.change_pct <= def.max_change)
        .filter(|r| r.volume >= def.min_volume)
        .cloned()
        .collect();

    match def.sort_by {
        ScanSort::ChangeDesc => filtered.sort_by(|a, b| b.change_pct.partial_cmp(&a.change_pct).unwrap_or(std::cmp::Ordering::Equal)),
        ScanSort::ChangeAsc  => filtered.sort_by(|a, b| a.change_pct.partial_cmp(&b.change_pct).unwrap_or(std::cmp::Ordering::Equal)),
        ScanSort::VolumeDesc => filtered.sort_by(|a, b| b.volume.cmp(&a.volume)),
    }

    filtered.truncate(def.limit);
    filtered
}

/// Format volume with K/M/B suffix.
fn fmt_volume(v: u64) -> String {
    if v >= 1_000_000_000 { format!("{:.1}B", v as f64 / 1e9) }
    else if v >= 1_000_000 { format!("{:.1}M", v as f64 / 1e6) }
    else if v >= 1_000 { format!("{:.0}K", v as f64 / 1e3) }
    else { format!("{}", v) }
}

pub(crate) fn draw(
    ctx: &egui::Context,
    watchlist: &mut Watchlist,
    panes: &mut [Chart],
    ap: usize,
    t: &Theme,
) {
    if !watchlist.scanner_open { return; }

    // ── Auto-fetch: trigger on open and every REFRESH_INTERVAL_SECS ──
    let should_fetch = match watchlist.scanner_last_fetch {
        None => true,
        Some(last) => last.elapsed().as_secs() >= REFRESH_INTERVAL_SECS,
    };
    if should_fetch && !watchlist.scanner_fetching {
        watchlist.scanner_fetching = true;
        watchlist.scanner_last_fetch = Some(std::time::Instant::now());
        fetch_scanner_prices();
    }
    // Clear fetching flag once we have some results
    if watchlist.scanner_fetching && !watchlist.scanner_results.is_empty() {
        watchlist.scanner_fetching = false;
    }

    // Collect actions to apply after the panel closes its borrow
    let mut pending_symbol: Option<String> = None;
    let mut save_as_watchlist: Option<(String, Vec<ScanResult>)> = None;
    let mut delete_scanner_idx: Option<usize> = None;

    egui::SidePanel::right("scanner_panel")
        .default_width(260.0)
        .min_width(180.0)
        .max_width(420.0)
        .resizable(true)
        .frame(egui::Frame::NONE.fill(t.toolbar_bg)
            .inner_margin(egui::Margin { left: 6, right: 6, top: 6, bottom: 4 })
            .stroke(egui::Stroke::new(1.0, color_alpha(t.toolbar_border, 80))))
        .show(ctx, |ui| {
            let panel_w = ui.available_width();
            ui.set_min_width(0.0);
            ui.set_max_width(panel_w);

            // ── Header ──
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("SCANNERS").monospace().size(10.0).strong().color(t.accent));
                // Refresh timer
                if let Some(last) = watchlist.scanner_last_fetch {
                    let elapsed = last.elapsed().as_secs();
                    let remaining = if elapsed < REFRESH_INTERVAL_SECS { REFRESH_INTERVAL_SECS - elapsed } else { 0 };
                    ui.label(egui::RichText::new(format!("{}s", remaining)).monospace().size(8.0).color(t.dim.gamma_multiply(0.4)));
                }
                // Loading indicator
                if watchlist.scanner_fetching {
                    ui.label(egui::RichText::new("...").monospace().size(9.0).color(t.dim.gamma_multiply(0.5)));
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if close_button(ui, t.dim) { watchlist.scanner_open = false; }
                    // Manual refresh
                    if ui.add(egui::Button::new(egui::RichText::new(Icon::ARROW_COUNTER_CLOCKWISE).size(10.0).color(t.dim)).frame(false))
                        .on_hover_text("Refresh now").clicked()
                    {
                        watchlist.scanner_last_fetch = None; // force re-fetch next frame
                    }
                    // Add custom scanner
                    if ui.add(egui::Button::new(egui::RichText::new(Icon::PLUS).size(10.0).color(t.dim)).frame(false))
                        .on_hover_text("New custom scanner").clicked()
                    {
                        watchlist.scanner_builder_open = !watchlist.scanner_builder_open;
                    }
                });
            });
            separator(ui, t.toolbar_border);
            ui.add_space(2.0);

            // ── Custom scanner builder (collapsible) ──
            if watchlist.scanner_builder_open {
                ui.group(|ui| {
                    ui.set_width(panel_w - 8.0);
                    ui.label(egui::RichText::new("New Scanner").monospace().size(9.0).strong().color(t.accent));
                    ui.add_space(2.0);

                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Name").monospace().size(8.0).color(t.dim));
                        ui.add(egui::TextEdit::singleline(&mut watchlist.scanner_new_name)
                            .desired_width(panel_w - 60.0)
                            .font(egui::FontId::monospace(9.0)));
                    });
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Min %").monospace().size(8.0).color(t.dim));
                        ui.add(egui::DragValue::new(&mut watchlist.scanner_new_min_change).speed(0.5).range(-100.0..=100.0).suffix("%"));
                        ui.label(egui::RichText::new("Max %").monospace().size(8.0).color(t.dim));
                        ui.add(egui::DragValue::new(&mut watchlist.scanner_new_max_change).speed(0.5).range(-100.0..=100.0).suffix("%"));
                    });
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Min Vol").monospace().size(8.0).color(t.dim));
                        ui.add(egui::TextEdit::singleline(&mut watchlist.scanner_new_min_volume)
                            .desired_width(80.0)
                            .font(egui::FontId::monospace(9.0))
                            .hint_text("e.g. 1000000"));
                    });

                    ui.horizontal(|ui| {
                        if simple_btn(ui, "Create", t.accent, 60.0) {
                            let name = if watchlist.scanner_new_name.trim().is_empty() {
                                "Custom Scanner".to_string()
                            } else {
                                watchlist.scanner_new_name.trim().to_string()
                            };
                            let min_vol: u64 = watchlist.scanner_new_min_volume.trim()
                                .replace(['_', ','], "")
                                .parse().unwrap_or(0);
                            watchlist.scanner_defs.push(ScannerDef {
                                name,
                                preset: None,
                                min_change: watchlist.scanner_new_min_change,
                                max_change: watchlist.scanner_new_max_change,
                                min_volume: min_vol,
                                sort_by: ScanSort::ChangeDesc,
                                limit: 20,
                                collapsed: false,
                            });
                            watchlist.scanner_new_name.clear();
                            watchlist.scanner_new_min_change = -999.0;
                            watchlist.scanner_new_max_change = 999.0;
                            watchlist.scanner_new_min_volume.clear();
                            watchlist.scanner_builder_open = false;
                        }
                        if simple_btn(ui, "Cancel", t.dim, 50.0) {
                            watchlist.scanner_builder_open = false;
                        }
                    });
                });
                ui.add_space(4.0);
                separator(ui, t.toolbar_border);
                ui.add_space(2.0);
            }

            // ── Scanner sections ──
            let pool = watchlist.scanner_results.clone();
            let num_scanners = watchlist.scanner_defs.len();

            egui::ScrollArea::vertical()
                .id_salt("scanner_scroll")
                .show(ui, |ui| {
                    ui.set_min_width(panel_w - 4.0);

                    if pool.is_empty() {
                        ui.add_space(20.0);
                        ui.label(egui::RichText::new("Fetching quotes...").monospace().size(9.0).color(t.dim.gamma_multiply(0.4)));
                        ui.add_space(4.0);
                        ui.label(egui::RichText::new(format!("{} symbols in universe", SCANNER_UNIVERSE.len()))
                            .monospace().size(8.0).color(t.dim.gamma_multiply(0.3)));
                        return;
                    }

                    for scanner_idx in 0..num_scanners {
                        let def = &watchlist.scanner_defs[scanner_idx];
                        let results = apply_scanner(def, &pool);
                        let collapsed = def.collapsed;
                        let scanner_name = def.name.clone();
                        let is_preset = def.preset.is_some();

                        // Section header
                        let header_resp = ui.horizontal(|ui| {
                            // Collapse caret
                            let caret = if collapsed { Icon::CARET_RIGHT } else { Icon::CARET_DOWN };
                            if ui.add(egui::Button::new(egui::RichText::new(caret).size(9.0).color(t.dim)).frame(false)).clicked() {
                                // Toggle handled below
                            }

                            // Scanner name + result count
                            let color = if is_preset { t.accent } else { t.dim };
                            let label_resp = ui.add(egui::Label::new(
                                egui::RichText::new(format!("{} ({})", &scanner_name, results.len()))
                                    .monospace().size(9.0).strong().color(color))
                                .sense(egui::Sense::click()));

                            if label_resp.clicked() {
                                // handled below
                            }

                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                // Delete (custom scanners only)
                                if !is_preset {
                                    if ui.add(egui::Button::new(egui::RichText::new(Icon::X).size(8.0).color(t.dim.gamma_multiply(0.5))).frame(false))
                                        .on_hover_text("Remove scanner").clicked()
                                    {
                                        delete_scanner_idx = Some(scanner_idx);
                                    }
                                }
                                // Save as Watchlist
                                if ui.add(egui::Button::new(egui::RichText::new(Icon::FOLDER).size(8.0).color(t.dim.gamma_multiply(0.5))).frame(false))
                                    .on_hover_text("Save as Watchlist").clicked()
                                {
                                    save_as_watchlist = Some((scanner_name.clone(), results.clone()));
                                }
                            });
                        });

                        // Toggle collapse on header click
                        if header_resp.response.clicked() {
                            watchlist.scanner_defs[scanner_idx].collapsed = !collapsed;
                        }

                        if !collapsed {
                            // Column headers
                            ui.horizontal(|ui| {
                                ui.add_space(4.0);
                                let cw = (panel_w - 16.0) / 3.0;
                                let hdr_color = t.dim.gamma_multiply(0.4);
                                col_header(ui, "SYMBOL", cw, hdr_color, false);
                                col_header(ui, "PRICE",  cw, hdr_color, true);
                                col_header(ui, "CHG%",   cw, hdr_color, true);
                            });

                            // Result rows
                            for r in &results {
                                let row_h = 16.0;
                                let (rect, resp) = ui.allocate_exact_size(egui::vec2(panel_w - 4.0, row_h), egui::Sense::click());

                                // Hover highlight
                                if resp.hovered() {
                                    ui.painter().rect_filled(rect, 2.0, color_alpha(t.accent, 15));
                                }

                                let change_col = if r.change_pct >= 0.0 { t.bull } else { t.bear };
                                let font = egui::FontId::monospace(9.0);
                                let cw = (panel_w - 16.0) / 3.0;

                                // Symbol
                                ui.painter().text(
                                    egui::pos2(rect.left() + 4.0, rect.center().y),
                                    egui::Align2::LEFT_CENTER,
                                    &r.symbol,
                                    font.clone(),
                                    egui::Color32::from_gray(200),
                                );

                                // Price
                                let price_str = if r.price >= 100.0 {
                                    format!("{:.2}", r.price)
                                } else if r.price >= 1.0 {
                                    format!("{:.2}", r.price)
                                } else {
                                    format!("{:.4}", r.price)
                                };
                                ui.painter().text(
                                    egui::pos2(rect.left() + 4.0 + cw + cw * 0.9, rect.center().y),
                                    egui::Align2::RIGHT_CENTER,
                                    &price_str,
                                    font.clone(),
                                    egui::Color32::from_gray(180),
                                );

                                // Change %
                                let sign = if r.change_pct >= 0.0 { "+" } else { "" };
                                let change_str = format!("{}{:.2}%", sign, r.change_pct);
                                ui.painter().text(
                                    egui::pos2(rect.right() - 4.0, rect.center().y),
                                    egui::Align2::RIGHT_CENTER,
                                    &change_str,
                                    font.clone(),
                                    change_col,
                                );

                                // Volume on hover tooltip
                                if resp.hovered() {
                                    resp.clone().on_hover_text(format!("Vol: {}", fmt_volume(r.volume)));
                                }

                                // Click => load chart
                                if resp.clicked() {
                                    pending_symbol = Some(r.symbol.clone());
                                }
                            }

                            if results.is_empty() {
                                ui.add_space(4.0);
                                ui.label(egui::RichText::new("No matches").monospace().size(8.0).color(t.dim.gamma_multiply(0.3)));
                            }
                        }

                        ui.add_space(4.0);
                        separator(ui, color_alpha(t.toolbar_border, 60));
                        ui.add_space(2.0);
                    }

                    // Universe info
                    ui.add_space(4.0);
                    ui.label(egui::RichText::new(format!("{}/{} symbols loaded", pool.len(), SCANNER_UNIVERSE.len()))
                        .monospace().size(7.5).color(t.dim.gamma_multiply(0.3)));
                });
        });

    // ── Apply deferred actions ──

    // Symbol click -> load chart on active pane
    if let Some(sym) = pending_symbol {
        if let Some(p) = panes.get_mut(ap) {
            p.pending_symbol_change = Some(sym);
        }
    }

    // Save scanner results as a new watchlist
    if let Some((name, results)) = save_as_watchlist {
        let items: Vec<WatchlistItem> = results.iter().map(|r| {
            let sym_hash = r.symbol.bytes().fold(0u32, |a, b| a.wrapping_mul(31).wrapping_add(b as u32));
            let rvol_seed = 0.5 + (sym_hash % 40) as f32 * 0.1;
            WatchlistItem {
                symbol: r.symbol.clone(), price: r.price, prev_close: if r.change_pct != 0.0 { r.price / (1.0 + r.change_pct / 100.0) } else { r.price }, loaded: true,
                is_option: false, underlying: String::new(), option_type: String::new(), strike: 0.0, expiry: String::new(), bid: 0.0, ask: 0.0,
                pinned: false, tags: vec![], rvol: rvol_seed, atr: 0.0,
                high_52wk: 0.0, low_52wk: 0.0, day_high: 0.0, day_low: 0.0,
                avg_daily_range: 2.0, earnings_days: -1, alert_triggered: false, price_history: vec![],
            }
        }).collect();

        let next_id = watchlist.saved_watchlists.iter()
            .flat_map(|w| w.sections.iter().map(|s| s.id))
            .max().unwrap_or(0) + 1;

        watchlist.saved_watchlists.push(SavedWatchlist {
            name: format!("Scan: {}", name),
            sections: vec![WatchlistSection {
                id: next_id,
                title: String::new(),
                color: None,
                collapsed: false,
                items,
            }],
            next_section_id: next_id + 1,
        });
        watchlist.persist();
    }

    // Delete custom scanner
    if let Some(idx) = delete_scanner_idx {
        if idx < watchlist.scanner_defs.len() {
            watchlist.scanner_defs.remove(idx);
        }
    }
}
