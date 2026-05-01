//! Scanner panel — Market Movers & custom scanners.
//!
//! Shows collapsible scanner sections (Top Gainers, Top Losers, Most Active)
//! populated from bulk quote data. Each symbol row is clickable to load a chart.
//! Includes "Save as Watchlist" and a custom scanner builder.

use egui;
use super::style::*;
use super::super::gpu::*;
use crate::ui_kit::icons::Icon;
use super::widgets::frames::CompactPanelFrame;
use super::widgets::status::Spinner;
use super::widgets::layout::EmptyState;
use super::widgets::text::{SectionLabel, MonospaceCode};
use super::widgets::buttons::SimpleBtn;
use super::widgets::form::FormRow;
use super::widgets::rows::WatchlistRow;

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

/// Draw scanner content into `ui` (used by analysis_panel as a tab).
/// Deferred actions (symbol click, save-as-watchlist, delete) are returned via out-params.
pub(crate) fn draw_content(
    ui: &mut egui::Ui,
    watchlist: &mut Watchlist,
    panes: &mut [Chart],
    ap: usize,
    t: &Theme,
    pending_symbol: &mut Option<String>,
    panel_w: f32,
) {
    // ── Auto-fetch ──
    let should_fetch = match watchlist.scanner_last_fetch {
        None => true,
        Some(last) => last.elapsed().as_secs() >= REFRESH_INTERVAL_SECS,
    };
    if should_fetch && !watchlist.scanner_fetching {
        watchlist.scanner_fetching = true;
        watchlist.scanner_last_fetch = Some(std::time::Instant::now());
        fetch_scanner_prices();
    }
    if watchlist.scanner_fetching && !watchlist.scanner_results.is_empty() {
        watchlist.scanner_fetching = false;
    }

    let mut save_as_watchlist: Option<(String, Vec<ScanResult>)> = None;
    let mut delete_scanner_idx: Option<usize> = None;

    ui.set_min_width(0.0);
    ui.set_max_width(panel_w);

    // ── Header ──
    ui.horizontal(|ui| {
        ui.add(SectionLabel::new("SCANNERS").xs().color(t.accent));
        if let Some(last) = watchlist.scanner_last_fetch {
            let elapsed = last.elapsed().as_secs();
            let remaining = if elapsed < REFRESH_INTERVAL_SECS { REFRESH_INTERVAL_SECS - elapsed } else { 0 };
            ui.add(MonospaceCode::new(&format!("{}s", remaining)).size_px(8.0).color(t.dim).gamma(0.4));
        }
        if watchlist.scanner_fetching {
            ui.add(Spinner::new().sm().theme(t));
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if icon_btn(ui, Icon::ARROW_COUNTER_CLOCKWISE, t.dim, FONT_MD)
                .on_hover_text("Refresh now").clicked()
            {
                watchlist.scanner_last_fetch = None;
            }
            if icon_btn(ui, Icon::PLUS, t.dim, FONT_MD)
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
            ui.set_width(panel_w - 6.0);
            ui.add(MonospaceCode::new("New Scanner").size_px(9.0).strong(true).color(t.accent));
            ui.add_space(2.0);

            FormRow::new("Name").gutter(36.0).label_color(t.dim).show(ui, t, |ui| {
                super::widgets::inputs::TextInput::new(&mut watchlist.scanner_new_name)
                    .width(panel_w - 60.0).font_size(9.0).show(ui);
            });
            FormRow::new("Min %").gutter(36.0).label_color(t.dim).show(ui, t, |ui| {
                ui.add(egui::DragValue::new(&mut watchlist.scanner_new_min_change).speed(0.5).range(-100.0..=100.0).suffix("%"));
                ui.add(MonospaceCode::new("Max %").size_px(8.0).color(t.dim));
                ui.add(egui::DragValue::new(&mut watchlist.scanner_new_max_change).speed(0.5).range(-100.0..=100.0).suffix("%"));
            });
            FormRow::new("Min Vol").gutter(36.0).label_color(t.dim).show(ui, t, |ui| {
                super::widgets::inputs::TextInput::new(&mut watchlist.scanner_new_min_volume)
                    .width(80.0).font_size(9.0).placeholder("e.g. 1000000").show(ui);
            });

            ui.horizontal(|ui| {
                if ui.add(SimpleBtn::new("Create").color(t.accent).min_width(60.0)).clicked() {
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
                if ui.add(SimpleBtn::new("Cancel").color(t.dim).min_width(50.0)).clicked() {
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
                ui.vertical_centered(|ui| {
                    ui.add(Spinner::new().md().theme(t));
                    ui.add_space(GAP_SM);
                });
                EmptyState::new("\u{1F50D}", "Fetching quotes",
                    &format!("{} symbols in universe", SCANNER_UNIVERSE.len())).theme(t).show(ui);
                return;
            }

            for scanner_idx in 0..num_scanners {
                let def = &watchlist.scanner_defs[scanner_idx];
                let results = apply_scanner(def, &pool);
                let collapsed = def.collapsed;
                let scanner_name = def.name.clone();
                let is_preset = def.preset.is_some();

                let header_resp = ui.horizontal(|ui| {
                    let caret = if collapsed { Icon::CARET_RIGHT } else { Icon::CARET_DOWN };
                    icon_btn(ui, caret, t.dim, FONT_MD);

                    let color = if is_preset { t.accent } else { t.dim };
                    let label_resp = ui.add(egui::Label::new(
                        egui::RichText::new(format!("{} ({})", &scanner_name, results.len()))
                            .monospace().size(9.0).strong().color(color))
                        .sense(egui::Sense::click()));
                    if label_resp.clicked() {}

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if !is_preset {
                            if icon_btn(ui, Icon::X, t.dim.gamma_multiply(0.5), 8.0)
                                .on_hover_text("Remove scanner").clicked()
                            {
                                delete_scanner_idx = Some(scanner_idx);
                            }
                        }
                        if icon_btn(ui, Icon::FOLDER, t.dim.gamma_multiply(0.5), 8.0)
                            .on_hover_text("Save as Watchlist").clicked()
                        {
                            save_as_watchlist = Some((scanner_name.clone(), results.clone()));
                        }
                    });
                });

                if header_resp.response.clicked() {
                    watchlist.scanner_defs[scanner_idx].collapsed = !collapsed;
                }

                if !collapsed {
                    ui.horizontal(|ui| {
                        ui.add_space(4.0);
                        let cw = (panel_w - 16.0) / 3.0;
                        let hdr_color = t.dim.gamma_multiply(0.4);
                        col_header(ui, "SYMBOL", cw, hdr_color, false);
                        col_header(ui, "PRICE",  cw, hdr_color, true);
                        col_header(ui, "CHG%",   cw, hdr_color, true);
                    });

                    for r in &results {
                        let price_str = if r.price >= 1.0 {
                            format!("{:.2}", r.price)
                        } else {
                            format!("{:.4}", r.price)
                        };
                        let resp = WatchlistRow::new(&r.symbol, r.price, r.change_pct)
                            .height(16.0)
                            .theme(t)
                            .price_string(price_str)
                            .price_right_inset(4.0)
                            .sym_layout(0.0, 0.0, 4.0)
                            .sym_font(egui::FontId::monospace(9.0))
                            .chg_font(egui::FontId::monospace(9.0))
                            .price_font(egui::FontId::monospace(9.0))
                            .fg(egui::Color32::from_gray(200))
                            .hover_overlay(color_alpha(t.accent, ALPHA_GHOST))
                            .show(ui);
                        if resp.response.hovered() {
                            resp.response.clone().on_hover_text(format!("Vol: {}", fmt_volume(r.volume)));
                        }
                        if resp.response.clicked() {
                            *pending_symbol = Some(r.symbol.clone());
                        }
                    }

                    if results.is_empty() {
                        ui.add_space(4.0);
                        ui.add(MonospaceCode::new("No matches").size_px(8.0).color(t.dim).gamma(0.3));
                    }
                }

                ui.add_space(4.0);
                separator(ui, color_alpha(t.toolbar_border, ALPHA_DIM));
                ui.add_space(2.0);
            }

            ui.add_space(4.0);
            ui.add(MonospaceCode::new(&format!("{}/{} symbols loaded", pool.len(), SCANNER_UNIVERSE.len())).size_px(7.5).color(t.dim).gamma(0.3));
        });

    // ── Apply deferred actions ──
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

    if let Some(idx) = delete_scanner_idx {
        if idx < watchlist.scanner_defs.len() {
            watchlist.scanner_defs.remove(idx);
        }
    }

    // Apply pending symbol (if called from standalone draw, not analysis_panel)
    // When called via analysis_panel, the caller handles this.
    let _ = (panes, ap); // silence unused warnings when called from analysis_panel
}

pub(crate) fn draw(
    ctx: &egui::Context,
    watchlist: &mut Watchlist,
    panes: &mut [Chart],
    ap: usize,
    t: &Theme,
) {
    if !watchlist.scanner_open { return; }

    let mut pending_symbol: Option<String> = None;

    egui::SidePanel::right("scanner_panel")
        .default_width(240.0)
        .min_width(180.0)
        .max_width(420.0)
        .resizable(true)
        .frame(CompactPanelFrame::new(t.toolbar_bg, t.toolbar_border).build())
        .show(ctx, |ui| {
            let panel_w = ui.available_width();
            draw_content(ui, watchlist, panes, ap, t, &mut pending_symbol, panel_w);
        });

    if let Some(sym) = pending_symbol {
        if let Some(p) = panes.get_mut(ap) {
            p.pending_symbol_change = Some(sym);
        }
    }
}
