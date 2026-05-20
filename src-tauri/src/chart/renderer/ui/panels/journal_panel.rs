//! Trade Journal panel — behavioral analytics, trade log, performance stats.
//!
//! Migrated to canonical ui_kit side-panel primitives:
//!   - Standalone `draw` uses `SidePanelShell` (was hand-rolled
//!     `SidePanel + PanelFrame + PanelHeaderWithClose`).
//!   - Section headers (TOTAL P&L / METRICS / INSIGHTS / TRADE LOG /
//!     RECENT TRADES) use `PanelSection`.
//!   - `PanelEmpty` for the "no trades" state.
//!   - `TradeCard` is preserved — that's the canonical trade card primitive.
//!   - The metric grid (Win Rate / Avg R / PF / etc.) keeps its bespoke
//!     4-column layout — `PanelKeyValueRow` only handles 2 columns.

use egui;
use super::super::style::*;
use super::super::super::gpu::{Watchlist, Theme, JournalEntry};
use super::super::components::text::MonospaceCode;
use crate::ui_kit::widgets::{
    PanelEmpty, PanelSection, Pagination, SidePanelShell, TradeCard, TradeCardData, Width,
};
use crate::ui_kit::icons::Icon;

const TRADE_LOG_PAGE_SIZE: usize = 10;

/// Inline content for the Book tab's Journal section.
pub(crate) fn draw_content(
    ui: &mut egui::Ui,
    watchlist: &mut Watchlist,
    t: &Theme,
) {
    let entries = &watchlist.journal_entries;
    if entries.is_empty() {
        PanelEmpty::new("No trades logged")
            .glyph(Icon::NOTEBOOK)
            .hint("Log a trade to see analytics")
            .show(ui, t);
        return;
    }

    PanelSection::new("SUMMARY").show(ui, t, |ui, t| draw_summary(ui, entries, t));

    let take = entries.len().min(5);
    PanelSection::new("RECENT TRADES").count(take).show(ui, t, |ui, t| {
        for entry in entries.iter().take(5) {
            draw_card(ui, entry, t);
        }
    });
}

/// Standalone sidebar panel.
pub(crate) fn draw(
    ctx: &egui::Context,
    watchlist: &mut Watchlist,
    t: &Theme,
) {
    if !watchlist.journal_panel_open { return; }

    let resp = SidePanelShell::new("journal_panel", "TRADE JOURNAL")
        .width(Width::Medium)
        .pane_metrics(
            crate::chart_renderer::gpu::pane_tabs_header_h(watchlist),
            watchlist.pane_header_size.title_font(),
        )
        .show(ctx, t, |ui, t| {
            if watchlist.journal_entries.is_empty() {
                ui.add_space(gap_3xl());
                PanelEmpty::new("No trades logged")
                    .glyph(Icon::NOTEBOOK)
                    .hint("Log a trade to see analytics")
                    .show(ui, t);
                return;
            }

            egui::ScrollArea::vertical().id_salt("journal_main").show(ui, |ui| {
                let entries = &watchlist.journal_entries;

                PanelSection::new("SUMMARY")
                    .show(ui, t, |ui, t| draw_summary(ui, entries, t));

                PanelSection::new("INSIGHTS")
                    .show(ui, t, |ui, t| draw_insights(ui, entries, t));

                let total = watchlist.journal_entries.len();
                PanelSection::new("TRADE LOG").count(total).show(ui, t, |ui, t| {
                    if total <= TRADE_LOG_PAGE_SIZE {
                        for entry in &watchlist.journal_entries {
                            draw_card(ui, entry, t);
                        }
                    } else {
                        let total_pages = (total + TRADE_LOG_PAGE_SIZE - 1) / TRADE_LOG_PAGE_SIZE;
                        if watchlist.journal_page >= total_pages {
                            watchlist.journal_page = total_pages - 1;
                        }
                        let page = watchlist.journal_page;
                        let start = page * TRADE_LOG_PAGE_SIZE;
                        let end = (start + TRADE_LOG_PAGE_SIZE).min(total);
                        for entry in &watchlist.journal_entries[start..end] {
                            draw_card(ui, entry, t);
                        }
                        ui.add_space(gap_xs());
                        ui.horizontal(|ui| {
                            ui.add_space(gap_sm());
                            let _ = Pagination::new(&mut watchlist.journal_page, total_pages)
                                .show(ui, t);
                        });
                    }
                });
            });
        });
    if resp.close_clicked { watchlist.update_sidebar_state(|s| s.journal_panel_open = false); }
}

fn draw_summary(ui: &mut egui::Ui, entries: &[JournalEntry], t: &Theme) {
    let total_pnl: f64 = entries.iter().map(|e| e.pnl).sum();
    let wins = entries.iter().filter(|e| e.pnl > 0.0).count();
    let losses = entries.iter().filter(|e| e.pnl <= 0.0).count();
    let win_rate = if !entries.is_empty() { wins as f32 / entries.len() as f32 * 100.0 } else { 0.0 };
    let avg_win = if wins > 0 { entries.iter().filter(|e| e.pnl > 0.0).map(|e| e.pnl).sum::<f64>() / wins as f64 } else { 0.0 };
    let avg_loss = if losses > 0 { entries.iter().filter(|e| e.pnl <= 0.0).map(|e| e.pnl).sum::<f64>() / losses as f64 } else { 0.0 };
    let avg_r = if !entries.is_empty() { entries.iter().map(|e| e.r_multiple).sum::<f64>() / entries.len() as f64 } else { 0.0 };
    let pf = if avg_loss.abs() > 0.001 { avg_win / avg_loss.abs() } else { 0.0 };
    let pnl_col = if total_pnl >= 0.0 { t.bull } else { t.bear };

    ui.horizontal(|ui| {
        ui.add_space(gap_sm());
        ui.vertical(|ui| {
            ui.label(egui::RichText::new("TOTAL P&L").monospace().size(font_xs()).color(color_half(t.dim)));
            let sign = if total_pnl >= 0.0 { "+" } else { "" };
            ui.label(egui::RichText::new(format!("{}${:.0}", sign, total_pnl)).size(34.0).color(pnl_col));
        });
    });
    ui.add_space(gap_sm());

    let col_w = (ui.available_width() - gap_sm() * 2.0) / 4.0;
    for row_items in [
        vec![("Win Rate", format!("{:.0}%", win_rate), if win_rate > 50.0 { t.bull } else { t.bear }),
             ("Avg R", format!("{:.1}R", avg_r), if avg_r > 0.0 { t.bull } else { t.bear }),
             ("PF", format!("{:.1}", pf), if pf > 1.0 { t.bull } else { t.bear }),
             ("Trades", format!("{}", entries.len()), t.text)],
        vec![("Wins", format!("{}", wins), t.bull),
             ("Losses", format!("{}", losses), t.bear),
             ("Avg Win", format!("${:.0}", avg_win), t.bull),
             ("Avg Loss", format!("${:.0}", avg_loss), t.bear)],
    ] {
        ui.horizontal(|ui| {
            ui.add_space(gap_sm());
            for (label, value, color) in &row_items {
                ui.vertical(|ui| {
                    ui.set_width(col_w);
                    ui.add(MonospaceCode::new(*label).xs().color(t.dim).gamma(0.4));
                    ui.add(MonospaceCode::new(value).sm().color(*color).strong(true));
                });
            }
        });
    }
}

fn draw_insights(ui: &mut egui::Ui, entries: &[JournalEntry], t: &Theme) {
    // BY SETUP TYPE
    PanelSection::new("BY SETUP TYPE").rule(false).show(ui, t, |ui, t| {
        let mut setups: Vec<(String, u32, u32, f64)> = Vec::new();
        for e in entries {
            if let Some(s) = setups.iter_mut().find(|(n, _, _, _)| *n == e.setup_type) {
                s.1 += 1; if e.pnl > 0.0 { s.2 += 1; } s.3 += e.pnl;
            } else {
                setups.push((e.setup_type.clone(), 1, if e.pnl > 0.0 { 1 } else { 0 }, e.pnl));
            }
        }
        setups.sort_by(|a, b| b.3.partial_cmp(&a.3).unwrap_or(std::cmp::Ordering::Equal));
        for (setup, total, wins, pnl) in &setups {
            let wr = if *total > 0 { *wins as f32 / *total as f32 * 100.0 } else { 0.0 };
            draw_insight_row(ui, setup, *total, wr, *pnl, t);
        }
    });

    PanelSection::new("BY HOLDING TIME").rule(false).show(ui, t, |ui, t| {
        for (label, min_m, max_m) in [("< 2 hrs", 0i64, 120i64), ("2h - 1d", 120, 1440), ("> 1 day", 1440, i64::MAX)] {
            let trades: Vec<&JournalEntry> = entries.iter().filter(|e| e.duration_mins >= min_m && e.duration_mins < max_m).collect();
            if trades.is_empty() { continue; }
            let w = trades.iter().filter(|e| e.pnl > 0.0).count() as u32;
            let wr = w as f32 / trades.len() as f32 * 100.0;
            let p: f64 = trades.iter().map(|e| e.pnl).sum();
            draw_insight_row(ui, label, trades.len() as u32, wr, p, t);
        }
    });

    PanelSection::new("BY DIRECTION").rule(false).show(ui, t, |ui, t| {
        for dir in ["Long", "Short"] {
            let trades: Vec<&JournalEntry> = entries.iter().filter(|e| e.side == dir).collect();
            if trades.is_empty() { continue; }
            let w = trades.iter().filter(|e| e.pnl > 0.0).count() as u32;
            let wr = w as f32 / trades.len() as f32 * 100.0;
            let p: f64 = trades.iter().map(|e| e.pnl).sum();
            draw_insight_row(ui, dir, trades.len() as u32, wr, p, t);
        }
    });
}

fn draw_insight_row(ui: &mut egui::Ui, label: &str, total: u32, wr: f32, pnl: f64, t: &Theme) {
    let col = if wr > 50.0 { t.bull } else { t.bear };
    ui.horizontal(|ui| {
        ui.add_space(gap_sm());
        ui.add(MonospaceCode::new(label).xs().color(t.text));
        let bar_w = 40.0;
        let (br, _) = ui.allocate_exact_size(egui::vec2(bar_w, 8.0), egui::Sense::hover());
        let p = ui.painter();
        p.rect_filled(br, 2.0, color_alpha(t.toolbar_border, alpha_faint()));
        p.rect_filled(egui::Rect::from_min_size(br.min, egui::vec2(bar_w * wr / 100.0, 8.0)),
            2.0, color_alpha(col, alpha_dim()));
        ui.label(egui::RichText::new(format!("{:.0}%", wr)).monospace().size(font_xs()).color(col));
        ui.label(egui::RichText::new(format!("{}t", total)).monospace().size(font_xs()).color(color_dim(t.dim)));
        let pc = if pnl >= 0.0 { t.bull } else { t.bear };
        ui.label(egui::RichText::new(format!("${:+.0}", pnl)).monospace().size(font_xs()).color(pc));
    });
}

fn draw_card(ui: &mut egui::Ui, entry: &JournalEntry, t: &Theme) {
    let data = TradeCardData {
        symbol:        &entry.symbol,
        side:          &entry.side,
        entry_price:   entry.entry_price,
        exit_price:    entry.exit_price,
        pnl:           entry.pnl,
        pnl_pct:       entry.pnl_pct,
        setup_type:    &entry.setup_type,
        duration_mins: entry.duration_mins,
        r_multiple:    entry.r_multiple,
        timeframe:     &entry.timeframe,
        notes:         &entry.notes,
    };
    TradeCard::new(&data).show(ui, t);
}
