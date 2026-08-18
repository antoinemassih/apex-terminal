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
use crate::ui_kit::sx::Tone;
use super::super::style::*;
use super::super::super::gpu::*;
use crate::ui_kit::icons::Icon;
use super::super::components::text::{SectionLabel, MonospaceCode};
use crate::ui_kit::widgets::FormRow;
use super::super::widgets::rows::WatchlistRow;
use crate::ui_kit::widgets::{Button, Input, NumberStepper, Skeleton, Spinner, PanelCard, PanelEmpty, Tooltip, PanelSubSection};
use crate::chart_renderer::ui::panels::side_panel_shell::{SidePanelShell, Width};
use crate::ui_kit::widgets::tokens::{Variant, Size as KitSize};
use crate::ui_kit::widgets::icon_placement::IconPlacement;

const REFRESH_INTERVAL_SECS: u64 = 30;

/// The sentinel the presets and the builder both use for "no bound on change".
/// `Top Gainers` is `0.0..=999.0`, `Most Active` is `-999.0..=999.0`.
const CHANGE_UNBOUNDED: f32 = 999.0;

/// Does this definition actually constrain the day change?
///
/// It matters because of what an unknown change is allowed to do. A row whose
/// previous close has not arrived cannot be shown to satisfy `>= 0.0`, so it
/// must not be admitted to `Top Gainers`. But `Most Active` sorts by volume and
/// places no change constraint at all, and dropping unknowns there would hide
/// live, heavily-traded symbols for a reason the user never asked for.
fn constrains_change(def: &ScannerDef) -> bool {
    def.min_change > -CHANGE_UNBOUNDED || def.max_change < CHANGE_UNBOUNDED
}

/// Order two possibly-unknown day changes, `desc` selecting the direction.
///
/// A known value always precedes an unknown one, in BOTH directions — an
/// unknown is not the biggest gainer and it is not the biggest loser either, so
/// `None` sinks to the bottom of the list either way.
///
/// This takes `(a, b)` in the caller's order and flips only the Some/Some
/// comparison, deliberately. The first version of this sorted descending by
/// passing the arguments in reverse (`cmp(b, a)`), which silently reversed the
/// None handling too and floated every unknown to the TOP of Top Gainers —
/// precisely the bug being fixed, reintroduced by the fix. It compiled and it
/// looked right.
fn cmp_change(a: Option<f32>, b: Option<f32>, desc: bool) -> std::cmp::Ordering {
    match (a, b) {
        (Some(x), Some(y)) => {
            let o = x.partial_cmp(&y).unwrap_or(std::cmp::Ordering::Equal);
            if desc { o.reverse() } else { o }
        }
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    }
}

/// Apply a scanner definition to the raw result pool and return filtered+sorted results.
///
/// # Unknown day changes
///
/// `ScanResult::change_pct` returns `None` until the previous close arrives.
/// It used to be a plain `f32` that read `0.0` in that case, and `0.0` passes
/// BOTH `>= 0.0` (Top Gainers) and `<= 0.0` (Top Losers) — so a symbol with no
/// change data was listed as a gainer and a loser simultaneously. An unknown is
/// now excluded from any scan that constrains change, and sorts last in the
/// ones that do not.
pub(crate) fn apply_scanner(def: &ScannerDef, pool: &[ScanResult]) -> Vec<ScanResult> {
    let bounded = constrains_change(def);
    let mut filtered: Vec<ScanResult> = pool.iter()
        .filter(|r| r.price > 0.0) // exclude unfetched
        .filter(|r| match r.change_pct() {
            Some(c) => c >= def.min_change && c <= def.max_change,
            None => !bounded,
        })
        .filter(|r| r.volume >= def.min_volume)
        .cloned()
        .collect();

    match def.sort_by {
        ScanSort::ChangeDesc => filtered.sort_by(|a, b| cmp_change(a.change_pct(), b.change_pct(), true)),
        ScanSort::ChangeAsc => filtered.sort_by(|a, b| cmp_change(a.change_pct(), b.change_pct(), false)),
        ScanSort::VolumeDesc => filtered.sort_by(|a, b| b.volume.cmp(&a.volume)),
    }

    filtered.truncate(def.limit);
    filtered
}

/// Format volume with K/M/B suffix.
/// Delegates to the ONE compact-number renderer. K gains a decimal (`4.5K`
/// rather than `4K`), matching the command palette's rendering of the same
/// figure.
fn fmt_volume(v: u64) -> String {
    crate::foundation::num_format::volume(v as f64)
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
    // When rendered standalone, `draw` already puts "SCANNERS" in the
    // SidePanelShell title, so the in-body SectionLabel would double it up.
    // The embedded/tab path (analysis_panel) has a generic shell header and
    // wants the in-body title. (U0-7)
    show_title: bool,
) {
    // ── Auto-fetch ──
    let should_fetch = match watchlist.scanner.last_fetch {
        None => true,
        Some(last) => last.elapsed().as_secs() >= REFRESH_INTERVAL_SECS,
    };
    if should_fetch && !watchlist.scanner.fetching {
        watchlist.scanner.fetching = true;
        watchlist.scanner.last_fetch = Some(std::time::Instant::now());
        fetch_scanner_prices();
    }
    if watchlist.scanner.fetching && !watchlist.scanner.results.is_empty() {
        watchlist.scanner.fetching = false;
    }

    let mut save_as_watchlist: Option<(String, Vec<ScanResult>)> = None;
    let mut delete_scanner_idx: Option<usize> = None;

    ui.set_min_width(0.0);
    ui.set_max_width(panel_w);

    // ── Header (in-body — when rendered as a tab the SidePanelShell header is
    // owned by analysis_panel). The standalone `draw` path attaches SCANNERS
    // as the shell title and skips this row.
    ui.horizontal(|ui| {
        if show_title {
            ui.add(SectionLabel::new("SCANNERS").xs().color(t.accent));
        }
        if let Some(last) = watchlist.scanner.last_fetch {
            let elapsed = last.elapsed().as_secs();
            let remaining = if elapsed < REFRESH_INTERVAL_SECS { REFRESH_INTERVAL_SECS - elapsed } else { 0 };
            ui.add(MonospaceCode::new(&format!("{}s", remaining)).size_px(font_xs()).color(t.dim).gamma(0.4));
        }
        if watchlist.scanner.fetching {
            Spinner::new().size(KitSize::Sm).show(ui, t);
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let r = Button::icon(Icon::ARROW_COUNTER_CLOCKWISE)
                .variant(Variant::Ghost)
                .size(KitSize::Sm)
                .placement(IconPlacement::PanelHeader)
                .show(ui, t);
            Tooltip::new("Refresh now").show(ui, &r, t);
            if r.clicked() {
                watchlist.scanner.last_fetch = None;
            }
            let r = Button::icon(Icon::PLUS)
                .variant(Variant::Ghost)
                .size(KitSize::Sm)
                .placement(IconPlacement::PanelHeader)
                .show(ui, t);
            Tooltip::new("New custom scanner").show(ui, &r, t);
            if r.clicked() {
                watchlist.update_sidebar_state(|s| s.scanner_builder_open = !s.scanner_builder_open);
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
        let mut selected = watchlist.scanner.mover_tab.min(kinds.len().saturating_sub(1));

        // Trailing control FIRST, in a right-to-left region; the chips then get
        // an inner left-to-right region sized to what is actually left over.
        //
        // This was a `horizontal_wrapped` with the chips laid out first and a
        // nested right_to_left for the button. The chips consumed the row, the
        // button's 110px minimum did not fit in the remainder, and it painted
        // straight over them — "…RVOL Le[Configure filt]ers", interleaved
        // letter by letter and unreadable. Reserving the trailing control up
        // front makes the chips bounded by it, so a too-long chip list clips
        // at the boundary instead of colliding.
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(Button::new("Configure filters").variant(Variant::Secondary).simple_treatment(true).fg(t.dim).min_size(egui::vec2(110.0, 0.0))).clicked() {
                    watchlist.scanner.filter_popup_open = !watchlist.scanner.filter_popup_open;
                }
                // WRAPPING and DIRECTION both stated explicitly. Two separate
                // corrections, each needed:
                //
                //  - `with_main_wrap(true)`: reserving the button's width is
                //    necessary but not sufficient, because egui does not clip
                //    an over-long row to its region. Without wrapping the chip
                //    run simply overflowed the remainder and collided with the
                //    button anyway. Wrapping is what actually bounds it.
                //
                //  - `left_to_right`: `ui.horizontal_wrapped()` inherits the
                //    parent's direction, and the parent here is right-to-left
                //    (that is how the button gets pinned to the right edge).
                //    It laid the chips out backwards — "Active Losers Gainers
                //    MOVERS". The collision was gone and the row was still
                //    wrong.
                ui.with_layout(
                    egui::Layout::left_to_right(egui::Align::Center).with_main_wrap(true),
                    |ui| {
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
                            watchlist.scanner.mover_tab = i;
                        }
                    }
                });
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
                    ui.add_space(gap_2xs());
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
                        ui.add_space(gap_2xs());
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
                    let click_resp = row_resp.response.interact(egui::Sense::click());
                    cursor::focus_ring(ui, &click_resp, t.accent);
                    if click_resp.clicked() {
                        *pending_symbol = Some(r.symbol.clone());
                    }
                }
            }
            Some(_) => {
                ui.add(MonospaceCode::new("no movers right now (market closed)").size_px(font_2xs()).color(t.dim).gamma(0.3));
            }
            None => {
                ui.horizontal(|ui| {
                    Spinner::new().size(KitSize::Sm).show(ui, t);
                    ui.add(MonospaceCode::new(&format!("loading {}…", kind.label())).size_px(font_xs()).color(t.dim));
                });
            }
        }
        if watchlist.scanner.filter_popup_open {
            ui.group(|ui| {
                ui.add(MonospaceCode::new("Custom filters").size_px(font_sm_tight()).strong(true).color(t.accent));
                ui.add(MonospaceCode::new("Wave 12 — custom scan endpoint not yet exposed.").size_px(font_xs()).color(t.dim));
                if ui.add(Button::new("Close").variant(Variant::Secondary).simple_treatment(true).fg(t.dim).min_size(egui::vec2(50.0, 0.0))).clicked() {
                    watchlist.scanner.filter_popup_open = false;
                }
            });
        }
        ui.add_space(gap_xs());
        separator(ui, tint(t, Tone::Border, alpha_dim()));
        ui.add_space(gap_xs());
    }

    // ── Custom scanner builder (collapsible) ──
    // ── Custom scanner builder (collapsible) — now a PanelCard instead of
    //    `ui.group()` (which paints egui's default gray frame that matches
    //    nothing else in the app).
    if watchlist.scanner.builder_open {
        PanelCard::new()
            .padding(gap_md())
            .show(ui, t, |ui, t| {
                ui.set_width(panel_w - 6.0);
                ui.add(MonospaceCode::new("New Scanner").size_px(font_sm_tight()).strong(true).color(t.accent));
                ui.add_space(gap_xs());

                FormRow::new("Name").gutter(36.0).label_color(t.dim).show(ui, t, |ui| {
                    Input::new(&mut watchlist.scanner.new_name)
                        .min_width(panel_w - 60.0)
                        .size(KitSize::Xs)
                        .show(ui, t);
                });
                FormRow::new("Min %").gutter(36.0).label_color(t.dim).show(ui, t, |ui| {
                    NumberStepper::new(&mut watchlist.scanner.new_min_change).range(-100.0_f32..=100.0).step(0.5).suffix("%").show(ui, t);
                    ui.add(MonospaceCode::new("Max %").size_px(font_xs()).color(t.dim));
                    NumberStepper::new(&mut watchlist.scanner.new_max_change).range(-100.0_f32..=100.0).step(0.5).suffix("%").show(ui, t);
                });
                FormRow::new("Min Vol").gutter(36.0).label_color(t.dim).show(ui, t, |ui| {
                    Input::new(&mut watchlist.scanner.new_min_volume)
                        .min_width(80.0)
                        .size(KitSize::Xs)
                        .placeholder("e.g. 1000000")
                        .show(ui, t);
                });

                ui.horizontal(|ui| {
                    if Button::new("Create").variant(Variant::Secondary).simple_treatment(true).fg(t.accent).min_size(egui::vec2(60.0, 0.0)).show(ui, t).clicked() {
                        let name = if watchlist.scanner.new_name.trim().is_empty() {
                            "Custom Scanner".to_string()
                        } else {
                            watchlist.scanner.new_name.trim().to_string()
                        };
                        let min_vol: u64 = watchlist.scanner.new_min_volume.trim()
                            .replace(['_', ','], "")
                            .parse().unwrap_or(0);
                        watchlist.scanner.defs.push(ScannerDef {
                            name,
                            preset: None,
                            min_change: watchlist.scanner.new_min_change,
                            max_change: watchlist.scanner.new_max_change,
                            min_volume: min_vol,
                            sort_by: ScanSort::ChangeDesc,
                            limit: 20,
                            collapsed: false,
                        });
                        watchlist.scanner.new_name.clear();
                        watchlist.scanner.new_min_change = -999.0;
                        watchlist.scanner.new_max_change = 999.0;
                        watchlist.scanner.new_min_volume.clear();
                        watchlist.update_sidebar_state(|s| s.scanner_builder_open = false);
                    }
                    if Button::new("Cancel").variant(Variant::Secondary).simple_treatment(true).fg(t.dim).min_size(egui::vec2(50.0, 0.0)).show(ui, t).clicked() {
                        watchlist.update_sidebar_state(|s| s.scanner_builder_open = false);
                    }
                });
            });
        ui.add_space(gap_xs());
        separator(ui, t.toolbar_border);
        ui.add_space(gap_xs());
    }

    // ── Scanner sections ──
    let pool = watchlist.scanner.results.clone();
    let num_scanners = watchlist.scanner.defs.len();

    egui::ScrollArea::vertical()
        .id_salt("scanner_scroll")
        .show(ui, |ui| {
            ui.set_min_width(panel_w - 4.0);

            if pool.is_empty() {
                ui.add_space(gap_lg());
                ui.vertical_centered(|ui| {
                    Spinner::new().size(KitSize::Md).show(ui, t);
                    ui.add_space(gap_sm());
                });
                // Audit fix: a fetch-in-progress is a LOADING state (spinner),
                // not a neutral PanelEmpty.
                crate::ui_kit::widgets::PanelLoading::new()
                    .reason(&format!("Fetching quotes \u{2014} {} symbols\u{2026}", SCANNER_UNIVERSE.len()))
                    .show(ui, t);
                ui.add_space(gap_sm());
                let row_w = (panel_w - 8.0).max(80.0);
                for _ in 0..6 {
                    ui.add_space(gap_2xs());
                    Skeleton::text(row_w).show(ui, t);
                }
                return;
            }

            for scanner_idx in 0..num_scanners {
                let def = &watchlist.scanner.defs[scanner_idx];
                let results = apply_scanner(def, &pool);
                let result_count = results.len();
                // Bridge collapse state: copy → pass &mut local → write back.
                let mut expanded = !def.collapsed;
                let scanner_name = def.name.clone();
                let is_preset = def.preset.is_some();
                let filter_active = def.min_change > -999.0
                    || def.max_change < 999.0
                    || def.min_volume > 0;

                // id_salt must be unique per section; use the scanner index.
                let id_salt = format!("scanner_section_{}", scanner_idx);

                PanelSubSection::new(&id_salt, &scanner_name)
                    .count(result_count)
                    .expanded(&mut expanded)
                    .header_trailing(|ui, t| {
                        // RTL slot: delete (custom only) + save-as-watchlist.
                        if !is_preset {
                            let r = Button::icon(Icon::X)
                                .variant(Variant::Ghost)
                                .placement(IconPlacement::PanelHeader)
                                .tone_destructive()
                                .show(ui, t);
                            Tooltip::new("Remove scanner").show(ui, &r, t);
                            if r.clicked() {
                                delete_scanner_idx = Some(scanner_idx);
                            }
                        }
                        let r = Button::icon(Icon::FOLDER)
                            .variant(Variant::Ghost)
                            .placement(IconPlacement::PanelHeader)
                            .show(ui, t);
                        Tooltip::new("Save as Watchlist").show(ui, &r, t);
                        if r.clicked() {
                            save_as_watchlist = Some((scanner_name.clone(), results.clone()));
                        }
                    })
                    .show(ui, t, |ui, _t| {
                        ui.horizontal(|ui| {
                            ui.add_space(gap_xs());
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
                            let resp = WatchlistRow::new(&r.symbol, r.price, r.change_pct())
                                .height(16.0)
                                .theme(t)
                                .price_string(price_str)
                                .price_right_inset(4.0)
                                .sym_layout(0.0, 0.0, 4.0)
                                .sym_font(mono_sm())
                                .chg_font(mono_sm())
                                .price_font(mono_sm())
                                .fg(t.text)
                                .hover_overlay(tint(t, Tone::Accent, alpha_ghost()))
                                .show(ui);
                            Tooltip::new(format!("Vol: {}", fmt_volume(r.volume)))
                                .show(ui, &resp.response, t);
                            if resp.response.clicked() {
                                *pending_symbol = Some(r.symbol.clone());
                            }
                        }

                        if results.is_empty() {
                            let hint = if filter_active {
                                "Try widening the filter"
                            } else {
                                "Run a scan to see results"
                            };
                            PanelEmpty::new("No matches").hint(hint).show(ui, t);
                        }
                    });

                // Write back the (possibly toggled) expanded state.
                watchlist.scanner.defs[scanner_idx].collapsed = !expanded;
            }

            ui.add_space(gap_xs());
            ui.add(MonospaceCode::new(&format!("{}/{} symbols loaded", pool.len(), SCANNER_UNIVERSE.len())).size_px(font_2xs()).color(t.dim).gamma(0.3));
        });

    // ── Apply deferred actions ──
    if let Some((name, results)) = save_as_watchlist {
        let items: Vec<WatchlistItem> = results.iter().map(|r| {
            let sym_hash = r.symbol.bytes().fold(0u32, |a, b| a.wrapping_mul(31).wrapping_add(b as u32));
            let rvol_seed = 0.5 + (sym_hash % 40) as f32 * 0.1;
            WatchlistItem {
                // `prev_close` is now carried on the scan result, so this
                // copies the real datum. It used to invert `change_pct` back
                // into a previous close and fall back to `r.price` when the
                // change read `0.0` — which is what an UNKNOWN change read as.
                // That wrote `prev_close == price` with `loaded: true` next to
                // it, and every downstream check for "is this known" tests
                // `prev_close > 0.0`, so the unknown passed. Saving a scan as a
                // watchlist committed the fabrication to disk.
                symbol: r.symbol.clone(), price: r.price, prev_close: r.prev_close, day_close: 0.0, change_perc: None, stale: false, loaded: true,
                is_option: false, underlying: String::new(), option_type: String::new(), strike: 0.0, expiry: String::new(), bid: 0.0, ask: 0.0,
                pinned: false, tags: vec![], rvol: rvol_seed, atr: 0.0,
                high_52wk: 0.0, low_52wk: 0.0, day_high: 0.0, day_low: 0.0,
                avg_daily_range: 2.0, earnings_days: -1, alert_triggered: false, price_history: vec![],
                prev_price: 0.0, price_change_at: None,
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
        if idx < watchlist.scanner.defs.len() {
            watchlist.scanner.defs.remove(idx);
        }
    }

    // Apply pending symbol (if called from standalone draw, not analysis_panel)
    // When called via analysis_panel, the caller handles this.
    let _ = (panes, ap); // silence unused warnings when called from analysis_panel
}

/// Rail registration — see [`super::right_rail`].
pub(crate) const RAIL: super::right_rail::RailPanelDef = super::right_rail::RailPanelDef {
    id: "scanner",
    is_open: |w| w.scanner.open,
    render: |cx, slot| draw(cx.ctx, cx.watchlist, cx.panes, cx.active_pane, cx.t, Some(slot)),
};

pub(crate) fn draw(
    ctx: &egui::Context,
    watchlist: &mut Watchlist,
    panes: &mut [Chart],
    ap: usize,
    t: &Theme,
    slot: Option<super::side_panel_shell::RailSlot>,
) {
    if !watchlist.scanner.open { return; }

    let mut pending_symbol: Option<String> = None;

    let resp = SidePanelShell::new("scanner_panel", "SCANNERS")
        .width(Width::Narrow)
        .resizable(180.0..=420.0)
        .rail_slot(slot)
        .show(ctx, t, |ui, t| {
            let panel_w = ui.available_width();
            draw_content(ui, watchlist, panes, ap, t, &mut pending_symbol, panel_w, false);
        });

    if resp.close_clicked { watchlist.update_sidebar_state(|s| s.scanner_open = false); }

    if let Some(sym) = pending_symbol {
        if let Some(p) = panes.get_mut(ap) {
            p.pending_symbol_change = Some(sym);
        }
    }
}

#[cfg(test)]
mod scanner_unknown_change_tests {
    use super::*;

    /// A row whose previous close arrived.
    fn known(sym: &str, price: f32, change_pct: f32, volume: u64) -> ScanResult {
        ScanResult {
            symbol: sym.into(),
            price,
            prev_close: price / (1.0 + change_pct / 100.0),
            volume,
        }
    }

    /// A row the feed has priced but not yet given a previous close for.
    fn unknown(sym: &str, price: f32, volume: u64) -> ScanResult {
        ScanResult { symbol: sym.into(), price, prev_close: 0.0, volume }
    }

    fn syms(v: &[ScanResult]) -> Vec<&str> {
        v.iter().map(|r| r.symbol.as_str()).collect()
    }

    #[test]
    fn an_unknown_change_is_none_not_zero() {
        assert_eq!(unknown("U", 100.0, 1).change_pct(), None);
        let k = known("K", 110.0, 10.0, 1).change_pct().expect("known");
        assert!((k - 10.0).abs() < 0.01, "got {k}");
    }

    /// THE defect. `Top Gainers` filters `change_pct >= 0.0`; an unknown used
    /// to read `0.0` and pass, so a symbol with no move data was listed as a
    /// gainer.
    #[test]
    fn an_unknown_is_not_a_gainer() {
        let pool = vec![known("UP", 110.0, 10.0, 1), unknown("NODATA", 50.0, 9)];
        let out = apply_scanner(&ScannerDef::preset_gainers(), &pool);
        assert_eq!(syms(&out), vec!["UP"], "an unknown change must not be listed as a gainer");
    }

    /// The other half of the same defect: `Top Losers` filters `<= 0.0`, which
    /// the same `0.0` also passed. One symbol, both lists, simultaneously.
    #[test]
    fn an_unknown_is_not_a_loser_either() {
        let pool = vec![known("DOWN", 90.0, -10.0, 1), unknown("NODATA", 50.0, 9)];
        let out = apply_scanner(&ScannerDef::preset_losers(), &pool);
        assert_eq!(syms(&out), vec!["DOWN"], "an unknown change must not be listed as a loser");
    }

    /// A genuinely flat symbol is a real gainer-list entry at 0.00%, and must
    /// NOT be swept out with the unknowns — the two were indistinguishable
    /// before and this is the direction that must not over-correct.
    #[test]
    fn a_genuinely_flat_symbol_still_qualifies() {
        let pool = vec![known("FLAT", 100.0, 0.0, 1)];
        let out = apply_scanner(&ScannerDef::preset_gainers(), &pool);
        assert_eq!(syms(&out), vec!["FLAT"]);
    }

    /// `Most Active` places no constraint on change and sorts by volume.
    /// Dropping unknowns there would hide live, heavily-traded symbols for a
    /// reason the user never asked for.
    #[test]
    fn an_unbounded_scan_keeps_unknowns() {
        let pool = vec![known("A", 110.0, 10.0, 1_000), unknown("NODATA", 50.0, 9_000)];
        let out = apply_scanner(&ScannerDef::preset_most_active(), &pool);
        assert_eq!(syms(&out), vec!["NODATA", "A"], "volume order, unknown retained");
    }

    /// Unknowns sort LAST descending. The first version of this fix sorted by
    /// passing the comparator's arguments reversed, which reversed the `None`
    /// handling too and floated every unknown to the TOP of the list.
    #[test]
    fn unknowns_sort_last_descending() {
        let def = ScannerDef { min_change: -999.0, max_change: 999.0, sort_by: ScanSort::ChangeDesc, ..ScannerDef::preset_most_active() };
        let pool = vec![
            unknown("NODATA", 50.0, 1),
            known("LOW", 95.0, -5.0, 1),
            known("HIGH", 110.0, 10.0, 1),
        ];
        let out = apply_scanner(&def, &pool);
        assert_eq!(syms(&out), vec!["HIGH", "LOW", "NODATA"]);
    }

    /// And last ascending too — an unknown is not the biggest loser.
    #[test]
    fn unknowns_sort_last_ascending() {
        let def = ScannerDef { min_change: -999.0, max_change: 999.0, sort_by: ScanSort::ChangeAsc, ..ScannerDef::preset_most_active() };
        let pool = vec![
            unknown("NODATA", 50.0, 1),
            known("HIGH", 110.0, 10.0, 1),
            known("LOW", 95.0, -5.0, 1),
        ];
        let out = apply_scanner(&def, &pool);
        assert_eq!(syms(&out), vec!["LOW", "HIGH", "NODATA"]);
    }

    /// A quote that arrives without a previous close must not erase one the
    /// pool already holds — otherwise a single sparse tick downgrades a known
    /// row back to unknown.
    #[test]
    fn a_later_quote_without_a_close_does_not_erase_a_known_one() {
        let mut scanner = crate::chart_renderer::watchlist_state::ScannerState::default();
        crate::chart_renderer::gpu::apply_scanner_price(&mut scanner, "AAPL", 100.0, 90.0, 5);
        crate::chart_renderer::gpu::apply_scanner_price(&mut scanner, "AAPL", 101.0, 0.0, 6);
        let r = &scanner.results[0];
        assert_eq!(r.prev_close, 90.0, "the known previous close must survive");
        assert_eq!(r.price, 101.0, "the new price must land");
        assert_eq!(r.volume, 6);
    }
}
