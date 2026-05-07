//! Trade Journal panel — behavioral analytics, trade log, performance stats.

use egui;
use super::super::style::*;
use super::super::super::gpu::{Watchlist, Theme, JournalEntry};
use super::super::widgets::text::{SectionLabel, MonospaceCode};
use super::super::widgets::layout::EmptyState;
use super::super::widgets::frames::PanelFrame;
use super::super::widgets::headers::PanelHeaderWithClose;
use crate::ui_kit::widgets::Pagination;

const TRADE_LOG_PAGE_SIZE: usize = 10;

/// Inline content for the Book tab's Journal section.
pub(crate) fn draw_content(
    ui: &mut egui::Ui,
    watchlist: &mut Watchlist,
    t: &Theme,
) {
    let entries = &watchlist.journal_entries;
    if entries.is_empty() {
        EmptyState::new("\u{1F4D2}", "No trades logged", "Log a trade to see analytics").theme(t).show(ui);
        return;
    }
    draw_summary(ui, entries, t);
    ui.add_space(gap_sm());
    separator(ui, color_alpha(t.toolbar_border, alpha_muted()));
    ui.add_space(gap_sm());
    ui.add(SectionLabel::new(&format!("RECENT TRADES ({})", entries.len().min(5))).tiny().color(t.dim));
    ui.add_space(gap_xs());
    for entry in entries.iter().take(5) {
        draw_card(ui, entry, t);
    }
}

/// Standalone sidebar panel.
pub(crate) fn draw(
    ctx: &egui::Context,
    watchlist: &mut Watchlist,
    t: &Theme,
) {
    if !watchlist.journal_panel_open { return; }

    egui::SidePanel::right("journal_panel")
        .default_width(300.0)
        .min_width(260.0)
        .max_width(460.0)
        .resizable(true)
        .frame(PanelFrame::new(t.toolbar_bg, t.toolbar_border).build())
        .show(ctx, |ui| {
            if PanelHeaderWithClose::new("TRADE JOURNAL").theme(t).show(ui) {
                watchlist.journal_panel_open = false;
            }
            separator(ui, color_alpha(t.toolbar_border, alpha_muted()));
            ui.add_space(gap_sm());

            if watchlist.journal_entries.is_empty() {
                ui.add_space(gap_3xl());
                EmptyState::new("\u{1F4D2}", "No trades logged", "Log a trade to see analytics").theme(t).show(ui);
                return;
            }

            egui::ScrollArea::vertical().id_salt("journal_main").show(ui, |ui| {
                let entries = &watchlist.journal_entries;
                draw_summary(ui, entries, t);
                ui.add_space(gap_sm());
                separator(ui, color_alpha(t.toolbar_border, alpha_muted()));
                ui.add_space(gap_sm());
                draw_insights(ui, entries, t);
                ui.add_space(gap_sm());
                separator(ui, color_alpha(t.toolbar_border, alpha_muted()));
                ui.add_space(gap_sm());
                let total = watchlist.journal_entries.len();
                ui.add(SectionLabel::new(&format!("TRADE LOG ({})", total)).tiny().color(t.dim));
                ui.add_space(gap_xs());

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
            ui.label(egui::RichText::new("TOTAL P&L").monospace().size(font_xs()).color(t.dim.gamma_multiply(0.5)));
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
    // By setup type
    ui.add(SectionLabel::new("BY SETUP TYPE").tiny().color(t.dim));
    ui.add_space(gap_xs());
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

    ui.add_space(gap_sm());
    ui.add(SectionLabel::new("BY HOLDING TIME").tiny().color(t.dim));
    ui.add_space(gap_xs());
    for (label, min_m, max_m) in [("< 2 hrs", 0i64, 120i64), ("2h - 1d", 120, 1440), ("> 1 day", 1440, i64::MAX)] {
        let trades: Vec<&JournalEntry> = entries.iter().filter(|e| e.duration_mins >= min_m && e.duration_mins < max_m).collect();
        if trades.is_empty() { continue; }
        let w = trades.iter().filter(|e| e.pnl > 0.0).count() as u32;
        let wr = w as f32 / trades.len() as f32 * 100.0;
        let p: f64 = trades.iter().map(|e| e.pnl).sum();
        draw_insight_row(ui, label, trades.len() as u32, wr, p, t);
    }

    ui.add_space(gap_sm());
    ui.add(SectionLabel::new("BY DIRECTION").tiny().color(t.dim));
    ui.add_space(gap_xs());
    for dir in ["Long", "Short"] {
        let trades: Vec<&JournalEntry> = entries.iter().filter(|e| e.side == dir).collect();
        if trades.is_empty() { continue; }
        let w = trades.iter().filter(|e| e.pnl > 0.0).count() as u32;
        let wr = w as f32 / trades.len() as f32 * 100.0;
        let p: f64 = trades.iter().map(|e| e.pnl).sum();
        draw_insight_row(ui, dir, trades.len() as u32, wr, p, t);
    }
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
        ui.label(egui::RichText::new(format!("{}t", total)).monospace().size(font_xs()).color(t.dim.gamma_multiply(0.4)));
        let pc = if pnl >= 0.0 { t.bull } else { t.bear };
        ui.label(egui::RichText::new(format!("${:+.0}", pnl)).monospace().size(font_xs()).color(pc));
    });
}

fn draw_card(ui: &mut egui::Ui, entry: &JournalEntry, t: &Theme) {
    let card_w = ui.available_width();
    let card_h = if entry.notes.is_empty() { 52.0 } else { 66.0 };
    let (card_rect, resp) = ui.allocate_exact_size(egui::vec2(card_w, card_h), egui::Sense::click());
    let p = ui.painter();
    let is_win = entry.pnl > 0.0;
    let pnl_col = if is_win { t.bull } else { t.bear };
    let dir_col = if entry.side == "Long" { t.bull } else { t.bear };

    let bg = if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        color_alpha(t.toolbar_border, alpha_subtle())
    } else { color_alpha(t.toolbar_border, 8) };
    p.rect_filled(card_rect, radius_sm(), bg);
    p.rect_filled(egui::Rect::from_min_max(
        egui::pos2(card_rect.left(), card_rect.top() + 3.0),
        egui::pos2(card_rect.left() + 3.0, card_rect.bottom() - 3.0)), 1.0, pnl_col);

    let cx = card_rect.left() + 8.0;
    let mut cy = card_rect.top() + 8.0;

    p.text(egui::pos2(cx, cy + 4.0), egui::Align2::LEFT_CENTER,
        &entry.symbol, egui::FontId::monospace(FONT_SM), t.text);
    p.text(egui::pos2(cx + 50.0, cy + 4.0), egui::Align2::LEFT_CENTER,
        &entry.side, egui::FontId::monospace(FONT_XS), dir_col);
    let sign = if entry.pnl >= 0.0 { "+" } else { "" };
    p.text(egui::pos2(card_rect.right() - 8.0, cy + 4.0), egui::Align2::RIGHT_CENTER,
        &format!("{}${:.0} ({:+.1}%)", sign, entry.pnl, entry.pnl_pct),
        egui::FontId::monospace(FONT_SM), pnl_col);
    cy += 16.0;

    p.text(egui::pos2(cx, cy + 4.0), egui::Align2::LEFT_CENTER,
        &entry.setup_type, egui::FontId::monospace(11.0), t.accent.gamma_multiply(0.7));
    let dur = if entry.duration_mins >= 1440 { format!("{:.0}d", entry.duration_mins as f64 / 1440.0) }
        else if entry.duration_mins >= 60 { format!("{:.0}h", entry.duration_mins as f64 / 60.0) }
        else { format!("{}m", entry.duration_mins) };
    p.text(egui::pos2(cx + 60.0, cy + 4.0), egui::Align2::LEFT_CENTER,
        &dur, egui::FontId::monospace(11.0), t.dim.gamma_multiply(0.5));
    let r_col = if entry.r_multiple > 0.0 { t.bull } else { t.bear };
    p.text(egui::pos2(cx + 90.0, cy + 4.0), egui::Align2::LEFT_CENTER,
        &format!("{:+.1}R", entry.r_multiple), egui::FontId::monospace(11.0), r_col);
    cy += 14.0;

    p.text(egui::pos2(cx, cy + 4.0), egui::Align2::LEFT_CENTER,
        &format!("{:.2} \u{2192} {:.2}", entry.entry_price, entry.exit_price),
        egui::FontId::monospace(11.0), t.dim.gamma_multiply(0.4));
    p.text(egui::pos2(card_rect.right() - 8.0, cy + 4.0), egui::Align2::RIGHT_CENTER,
        &entry.timeframe, egui::FontId::monospace(11.0), t.dim.gamma_multiply(0.4));

    if !entry.notes.is_empty() {
        cy += 14.0;
        p.text(egui::pos2(cx, cy + 4.0), egui::Align2::LEFT_CENTER,
            &entry.notes, egui::FontId::monospace(11.0), t.dim.gamma_multiply(0.35));
    }

    ui.add_space(gap_xs());
}
