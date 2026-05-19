//! Scanner panel — Market Movers & custom scanners.
//!
//! Shows collapsible scanner sections (Top Gainers, Top Losers, Most Active)
//! populated from bulk quote data. Each symbol row is clickable to load a chart.
//! Includes "Save as Watchlist" and a custom scanner builder.
//!
//! Migrated to the canonical ui_kit side-panel primitives:
//!   - `SidePanelShell` for outer chrome (was hand-rolled SidePanel + CompactPanelFrame)
//!   - `PanelCard` for the custom-scanner builder (replaces `ui.group()` default frame)
//!   - `PanelEmpty` for the "fetching quotes" empty state (replaces local `EmptyState`)
//!   - `Button::icon(Icon::ARROW_COUNTER_CLOCKWISE).variant(Variant::Ghost)` for the
//!     refresh control (was hand-rolled `icon_btn` chrome).

use egui;
use super::super::style::*;
use super::super::super::gpu::*;
use crate::ui_kit::icons::Icon;
use super::super::widgets::status::Spinner;
use super::super::widgets::text::{SectionLabel, MonospaceCode};
use crate::ui_kit::widgets::FormRow;
use super::super::widgets::rows::WatchlistRow;
use crate::ui_kit::widgets::{
    Button, Input, NumberStepper, Skeleton, PanelCard, PanelEmpty, SidePanelShell, Tooltip, Width,
};
use crate::ui_kit::widgets::tokens::{Variant, Size as KitSize};
use crate::ui_kit::widgets::icon_placement::IconPlacement;

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

    // ── Header (in-body — when rendered as a tab the SidePanelShell header is
    // owned by analysis_panel). The standalone `draw` path attaches SCANNERS
    // as the shell title and skips this row.
    ui.horizontal(|ui| {
        ui.add(SectionLabel::new("SCANNERS").xs().color(t.accent));
        if let Some(last) = watchlist.scanner_last_fetch {
            let elapsed = last.elapsed().as_secs();
            let remaining = if elapsed < REFRESH_INTERVAL_SECS { REFRESH_INTERVAL_SECS - elapsed } else { 0 };
            ui.add(MonospaceCode::new(&format!("{}s", remaining)).size_px(font_xs()).color(t.dim).gamma(0.4));
        }
        if watchlist.scanner_fetching {
            ui.add(Spinner::new().sm().theme(t));
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let r = Button::icon(Icon::ARROW_COUNTER_CLOCKWISE)
                .variant(Variant::Ghost)
                .size(KitSize::Sm)
                .placement(IconPlacement::PanelHeader)
                .show(ui, t);
            Tooltip::new("Refresh now").show(ui, &r, t);
            if r.clicked() {
                watchlist.scanner_last_fetch = None;
            }
            let r = Button::icon(Icon::PLUS)
                .variant(Variant::Ghost)
                .size(KitSize::Sm)
                .placement(IconPlacement::PanelHeader)
                .show(ui, t);
            Tooltip::new("New custom scanner").show(ui, &r, t);
            if r.clicked() {
                watchlist.scanner_builder_open = !watchlist.scanner_builder_open;
            }
        });
    });
    separator(ui, t.toolbar_border);
    ui.add_space(gap_xs());

    // ── Wave 10 movers tabs (projector-backed) ────────────────────────────
    // Single bucket selector at top of the panel — reads `live_state::get_movers(kind)`
    // populated by the 5s `apex-projectors` poller. Stays cached even when the
    // user has the panel closed; tab switch flips the read key only.
    {
        use crate::apex_data::types::MoverKind;
        let kinds = MoverKind::all();
        let mut selected = watchlist.scanner_mover_tab.min(kinds.len().saturating_sub(1));

        ui.horizontal_wrapped(|ui| {
            ui.add(SectionLabel::new("MOVERS").xs().color(t.accent));
            for (i, k) in kinds.iter().enumerate() {
                let is_sel = i == selected;
                let label = egui::RichText::new(k.label())
                    .monospace()
                    .size(font_xs())
                    .color(if is_sel { t.accent } else { t.dim });
                let resp = ui.add(egui::Label::new(label).sense(egui::Sense::click()));
                if resp.clicked() {
                    selected = i;
                    watchlist.scanner_mover_tab = i;
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(Button::new("Configure filters").variant(Variant::Secondary).simple_treatment(true).fg(t.dim).min_size(egui::vec2(110.0, 0.0))).clicked() {
                    watchlist.scanner_filter_popup_open = !watchlist.scanner_filter_popup_open;
                }
            });
        });
        let kind = kinds[selected];
        let cache = crate::apex_data::live_state::get_movers(kind);
        let age = crate::apex_data::live_state::movers_age_secs(kind);
        match cache {
            Some(m) if !m.rows.is_empty() => {
                ui.add(MonospaceCode::new(&format!(
                    "{} rows · {}s ago", m.rows.len(),
                    if age == u64::MAX { 0 } else { age.min(9999) },
                )).size_px(font_2xs()).color(t.dim).gamma(0.4));
                ui.horizontal(|ui| {
                    ui.add_space(2.0);
                    let cw = (panel_w - 12.0) / 5.0;
                    let hdr_color = color_dim(t.dim);
                    col_header(ui, "SYM",   cw, hdr_color, false);
                    col_header(ui, "LAST",  cw, hdr_color, true);
                    col_header(ui, "CHG%",  cw, hdr_color, true);
                    col_header(ui, "VOL",   cw, hdr_color, true);
                    col_header(ui, "RVOL",  cw, hdr_color, true);
                });
                let max_rows = 20usize;
                for r in m.rows.iter().take(max_rows) {
                    let row_resp = ui.horizontal(|ui| {
                        ui.add_space(2.0);
                        let cw = (panel_w - 12.0) / 5.0;
                        ui.add_sized(egui::vec2(cw, 14.0), egui::Label::new(
                            egui::RichText::new(&r.symbol).monospace().size(font_xs()).color(t.text)
                        ));
                        ui.add_sized(egui::vec2(cw, 14.0), egui::Label::new(
                            egui::RichText::new(format!("{:.2}", r.last)).monospace().size(font_xs()).color(t.text)
                        ));
                        let chg_color = if r.change_pct >= 0.0 { t.bull } else { t.bear };
                        ui.add_sized(egui::vec2(cw, 14.0), egui::Label::new(
                            egui::RichText::new(format!("{:+.2}%", r.change_pct)).monospace().size(font_xs()).color(chg_color)
                        ));
                        ui.add_sized(egui::vec2(cw, 14.0), egui::Label::new(
                            egui::RichText::new(fmt_volume(r.volume)).monospace().size(font_xs()).color(t.dim)
                        ));
                        let rvol_str = r.rvol.map(|v| format!("{:.1}x", v)).unwrap_or_else(|| "—".into());
                        ui.add_sized(egui::vec2(cw, 14.0), egui::Label::new(
                            egui::RichText::new(rvol_str).monospace().size(font_xs()).color(t.dim)
                        ));
                    });
                    if row_resp.response.interact(egui::Sense::click()).clicked() {
                        *pending_symbol = Some(r.symbol.clone());
                    }
                }
            }
            Some(_) => {
                ui.add(MonospaceCode::new("(empty bucket — projector returned no rows)").size_px(font_2xs()).color(t.dim).gamma(0.3));
            }
            None => {
                ui.horizontal(|ui| {
                    ui.add(Spinner::new().sm().theme(t));
                    ui.add(MonospaceCode::new(&format!("loading {}…", kind.label())).size_px(font_xs()).color(t.dim));
                });
                ui.add(MonospaceCode::new(
                    "// TODO(wire-route): /api/stocks/movers/:kind not reachable yet"
                ).size_px(font_2xs()).color(t.dim).gamma(0.3));
            }
        }
        if watchlist.scanner_filter_popup_open {
            ui.group(|ui| {
                ui.add(MonospaceCode::new("Custom filters").size_px(font_sm_tight()).strong(true).color(t.accent));
                ui.add(MonospaceCode::new("Wave 12 — custom scan endpoint not yet exposed.").size_px(font_xs()).color(t.dim));
                if ui.add(Button::new("Close").variant(Variant::Secondary).simple_treatment(true).fg(t.dim).min_size(egui::vec2(50.0, 0.0))).clicked() {
                    watchlist.scanner_filter_popup_open = false;
                }
            });
        }
        ui.add_space(4.0);
        separator(ui, color_alpha(t.toolbar_border, alpha_dim()));
        ui.add_space(4.0);
    }

    // ── Custom scanner builder (collapsible) ──
    // ── Custom scanner builder (collapsible) — now a PanelCard instead of
    //    `ui.group()` (which paints egui's default gray frame that matches
    //    nothing else in the app).
    if watchlist.scanner_builder_open {
        PanelCard::new()
            .padding(gap_md())
            .show(ui, t, |ui, t| {
                ui.set_width(panel_w - 6.0);
                ui.add(MonospaceCode::new("New Scanner").size_px(font_sm_tight()).strong(true).color(t.accent));
                ui.add_space(gap_xs());

                FormRow::new("Name").gutter(36.0).label_color(t.dim).show(ui, t, |ui| {
                    Input::new(&mut watchlist.scanner_new_name)
                        .min_width(panel_w - 60.0)
                        .size(KitSize::Xs)
                        .show(ui, t);
                });
                FormRow::new("Min %").gutter(36.0).label_color(t.dim).show(ui, t, |ui| {
                    NumberStepper::new(&mut watchlist.scanner_new_min_change).range(-100.0_f32..=100.0).step(0.5).suffix("%").show(ui, t);
                    ui.add(MonospaceCode::new("Max %").size_px(font_xs()).color(t.dim));
                    NumberStepper::new(&mut watchlist.scanner_new_max_change).range(-100.0_f32..=100.0).step(0.5).suffix("%").show(ui, t);
                });
                FormRow::new("Min Vol").gutter(36.0).label_color(t.dim).show(ui, t, |ui| {
                    Input::new(&mut watchlist.scanner_new_min_volume)
                        .min_width(80.0)
                        .size(KitSize::Xs)
                        .placeholder("e.g. 1000000")
                        .show(ui, t);
                });

                ui.horizontal(|ui| {
                    if Button::new("Create").variant(Variant::Secondary).simple_treatment(true).fg(t.accent).min_size(egui::vec2(60.0, 0.0)).show(ui, t).clicked() {
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
                    if Button::new("Cancel").variant(Variant::Secondary).simple_treatment(true).fg(t.dim).min_size(egui::vec2(50.0, 0.0)).show(ui, t).clicked() {
                        watchlist.scanner_builder_open = false;
                    }
                });
            });
        ui.add_space(gap_xs());
        separator(ui, t.toolbar_border);
        ui.add_space(gap_xs());
    }

    // ── Scanner sections ──
    let pool = watchlist.scanner_results.clone();
    let num_scanners = watchlist.scanner_defs.len();

    egui::ScrollArea::vertical()
        .id_salt("scanner_scroll")
        .show(ui, |ui| {
            ui.set_min_width(panel_w - 4.0);

            if pool.is_empty() {
                ui.add_space(gap_lg());
                ui.vertical_centered(|ui| {
                    ui.add(Spinner::new().md().theme(t));
                    ui.add_space(gap_sm());
                });
                PanelEmpty::new("Fetching quotes")
                    .glyph("\u{1F50D}")
                    .hint(&format!("{} symbols in universe", SCANNER_UNIVERSE.len()))
                    .show(ui, t);
                ui.add_space(gap_sm());
                let row_w = (panel_w - 8.0).max(80.0);
                for _ in 0..6 {
                    ui.add_space(2.0);
                    Skeleton::text(row_w).show(ui, t);
                }
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
                    icon_btn(ui, caret, t.dim, font_md());

                    let color = if is_preset { t.accent } else { t.dim };
                    let label_resp = ui.add(egui::Label::new(
                        egui::RichText::new(format!("{} ({})", &scanner_name, results.len()))
                            .monospace().size(font_xs()).strong().color(color))
                        .sense(egui::Sense::click()));
                    crate::chart_renderer::ui::style::cursor::clickable(ui, &label_resp);
                    if label_resp.clicked() {}

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if !is_preset {
                            let r = icon_btn(ui, Icon::X, color_half(t.dim), 8.0);
                            Tooltip::new("Remove scanner").show(ui, &r, t);
                            if r.clicked() {
                                delete_scanner_idx = Some(scanner_idx);
                            }
                        }
                        let r = icon_btn(ui, Icon::FOLDER, color_half(t.dim), 8.0);
                        Tooltip::new("Save as Watchlist").show(ui, &r, t);
                        if r.clicked() {
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
                        let hdr_color = color_dim(t.dim);
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
                            .sym_font(mono_sm())
                            .chg_font(mono_sm())
                            .price_font(mono_sm())
                            .fg(t.text)
                            .hover_overlay(color_alpha(t.accent, alpha_ghost()))
                            .show(ui);
                        if resp.response.hovered() {
                            resp.response.clone().on_hover_text(format!("Vol: {}", fmt_volume(r.volume)));
                        }
                        if resp.response.clicked() {
                            *pending_symbol = Some(r.symbol.clone());
                        }
                    }

                    if results.is_empty() {
                        let def = &watchlist.scanner_defs[scanner_idx];
                        let filter_active = def.min_change > -999.0
                            || def.max_change < 999.0
                            || def.min_volume > 0;
                        let hint = if filter_active {
                            "Try widening the filter"
                        } else {
                            "Run a scan to see results"
                        };
                        PanelEmpty::new("No matches").hint(hint).show(ui, t);
                    }
                }

                ui.add_space(4.0);
                separator(ui, color_alpha(t.toolbar_border, alpha_dim()));
                ui.add_space(4.0);
            }

            ui.add_space(4.0);
            ui.add(MonospaceCode::new(&format!("{}/{} symbols loaded", pool.len(), SCANNER_UNIVERSE.len())).size_px(font_2xs()).color(t.dim).gamma(0.3));
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

    let resp = SidePanelShell::new("scanner_panel", "SCANNERS")
        .width(Width::Narrow)
        .resizable(180.0..=420.0)
        .show(ctx, t, |ui, t| {
            let panel_w = ui.available_width();
            draw_content(ui, watchlist, panes, ap, t, &mut pending_symbol, panel_w);
        });

    if resp.close_clicked { watchlist.scanner_open = false; }

    if let Some(sym) = pending_symbol {
        if let Some(p) = panes.get_mut(ap) {
            p.pending_symbol_change = Some(sym);
        }
    }
}
