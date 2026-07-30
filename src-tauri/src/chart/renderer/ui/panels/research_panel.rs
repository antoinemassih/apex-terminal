//! Research panel — fundamentals, insider trades, analyst ratings, filings.
//!
//! Always embedded via `draw_content` inside `analysis_panel`'s
//! [`SplitSectionPanel`] (Research tab) — never standalone. So this module
//! only migrates body primitives:
//!   - `PanelSection` (default `.rule(true)` hairline) for VALUATION /
//!     FINANCIALS / OWNERSHIP / ANALYST CONSENSUS / EARNINGS HISTORY /
//!     INSIDER TRANSACTIONS / ECONOMIC CALENDAR.
//!   - `PanelKeyValueRow` for label/value metric stacks.
//!   - `PanelEmpty` for the "no data" guard.
//! The analyst bar, earnings rows and insider/calendar rows keep their
//! bespoke painting (lollipop bar, sparkline, painter-drawn dots) — none of
//! those have a canonical primitive yet.

use egui;
use super::super::style::*;
use super::super::super::gpu::{Chart, Theme};
use super::super::components::text::MonospaceCode;
use crate::ui_kit::widgets::{PanelEmpty, PanelError, PanelKeyValueRow, PanelSection, PanelTone};
use crate::ui_kit::icons::Icon;

pub(crate) fn draw_content(
    ui: &mut egui::Ui,
    panes: &[Chart],
    ap: usize,
    t: &Theme,
) {
    if panes.is_empty() {
        PanelEmpty::new("No symbol selected").glyph(Icon::CHART_BAR).show(ui, t);
        return;
    }
    let chart = &panes[ap];
    let f = &chart.fundamentals;

    // ── Company (ApexData /api/ticker reference detail) ──
    // Lazy, cached, non-blocking: first frame spawns the fetch and repaints
    // when it lands. Only populated for stock symbols.
    if let Some(td) = crate::chart_renderer::gpu::ticker_detail_cached(&chart.symbol) {
        PanelSection::new("COMPANY")
            .show(ui, t, |ui, t| {
                if !td.name.is_empty() {
                    ui.horizontal(|ui| {
                        ui.add_space(gap_sm());
                        ui.add(MonospaceCode::new(&td.name).xs().color(t.text));
                    });
                    ui.add_space(gap_xs());
                }
                let mcap = if td.market_cap >= 1e12 {
                    format!("${:.2}T", td.market_cap / 1e12)
                } else if td.market_cap >= 1e9 {
                    format!("${:.1}B", td.market_cap / 1e9)
                } else if td.market_cap > 0.0 {
                    format!("${:.0}M", td.market_cap / 1e6)
                } else { "—".to_string() };
                let mut rows: Vec<(&str, String)> = vec![
                    ("Exchange", if td.primary_exchange.is_empty() { "—".into() } else { td.primary_exchange.clone() }),
                    ("Type",     if td.kind.is_empty() { "—".into() } else { td.kind.clone() }),
                    ("Market Cap", mcap),
                ];
                if !td.sic_description.is_empty() {
                    rows.push(("Sector", td.sic_description.clone()));
                }
                if td.total_employees > 0 {
                    rows.push(("Employees", format!("{}", td.total_employees)));
                }
                if !td.list_date.is_empty() {
                    rows.push(("Listed", td.list_date.clone()));
                }
                for (label, value) in rows {
                    PanelKeyValueRow::new(label, value).show(ui, t);
                }
            });
    }

    // Fundamentals have no live feed yet — only render the financial sections
    // when real data is present (market cap / P/E populated), otherwise show one
    // honest "not connected" note instead of a wall of zeros.
    let have_fund = f.market_cap > 0.0 || f.pe_ratio > 0.0;

    // ── Valuation ──
    if have_fund {
        PanelSection::new(&format!("VALUATION — {}", chart.symbol))
            .show(ui, t, |ui, t| {
                for (label, value) in [
                    ("P/E (TTM)",  format!("{:.1}",   f.pe_ratio)),
                    ("Forward P/E",format!("{:.1}",   f.forward_pe)),
                    ("EPS (TTM)",  format!("${:.2}",  f.eps_ttm)),
                    ("Market Cap", format!("${:.0}B", f.market_cap)),
                    ("Div Yield",  format!("{:.2}%",  f.dividend_yield)),
                    ("Beta",       format!("{:.2}",   f.beta)),
                ] {
                    PanelKeyValueRow::new(label, value).show(ui, t);
                }
            });
    } else {
        PanelSection::new(&format!("FUNDAMENTALS — {}", chart.symbol))
            .show(ui, t, |ui, t| {
                PanelError::new("Fundamentals feed not connected")
                    .hint("Provider offline or not configured").show(ui, t);
            });
    }

    // ── Options Analytics (ApexData derived endpoints) ──
    // Cached + TTL-refreshed; only drawn when the underlying actually has
    // options data (404s on non-optionable symbols leave every field None).
    if let Some(oa) = crate::chart_renderer::gpu::options_analytics_cached(&chart.symbol) {
        if oa.any() {
            PanelSection::new("OPTIONS ANALYTICS")
                .show(ui, t, |ui, t| {
                    if let Some(em) = &oa.expected_move {
                        let exp = if em.expiry.is_empty() { String::new() } else { format!("  exp {}", em.expiry) };
                        PanelKeyValueRow::new(
                            "Expected Move",
                            format!("\u{00B1}${:.2} ({:.2}%){}", em.expected_move_dollars, em.expected_move_pct, exp),
                        ).show(ui, t);
                    }
                    if let Some(iv) = &oa.iv_rank {
                        PanelKeyValueRow::new("IV Rank", format!("{:.0}", iv.iv_rank))
                            .tone(if iv.iv_rank >= 50.0 { PanelTone::Bear } else { PanelTone::Bull })
                            .show(ui, t);
                        PanelKeyValueRow::new("IV Percentile", format!("{:.0}", iv.iv_percentile)).show(ui, t);
                    }
                    if let Some(p) = &oa.pcr {
                        PanelKeyValueRow::new("Put/Call (OI)", format!("{:.2}", p.pcr_oi))
                            .tone(if p.pcr_oi > 1.0 { PanelTone::Bear } else { PanelTone::Bull })
                            .show(ui, t);
                        PanelKeyValueRow::new("Put/Call (Vol)", format!("{:.2}", p.pcr_volume))
                            .tone(if p.pcr_volume > 1.0 { PanelTone::Bear } else { PanelTone::Bull })
                            .show(ui, t);
                    }
                    if let Some(g) = &oa.gex {
                        let nb = g.net_gex / 1e9;
                        PanelKeyValueRow::new("Net GEX", format!("${:.2}B", nb))
                            .tone(if g.net_gex >= 0.0 { PanelTone::Bull } else { PanelTone::Bear })
                            .show(ui, t);
                        if g.flip_strike > 0.0 {
                            PanelKeyValueRow::new("Gamma Flip", format!("{:.0}", g.flip_strike)).show(ui, t);
                        }
                    }
                });
        }
    }

    // ── Financials ──
    if have_fund {
        PanelSection::new("FINANCIALS")
        .show(ui, t, |ui, t| {
            let rows = [
                ("Revenue Growth", format!("{:+.1}%", f.revenue_growth),
                    if f.revenue_growth > 0.0 { PanelTone::Bull } else { PanelTone::Bear }),
                ("Profit Margin",  format!("{:.1}%", f.profit_margin),
                    if f.profit_margin > 15.0 { PanelTone::Bull } else { PanelTone::Default }),
                ("Debt/Equity",    format!("{:.2}", f.debt_to_equity),
                    if f.debt_to_equity > 1.5 { PanelTone::Bear } else { PanelTone::Default }),
            ];
            for (label, value, tone) in rows {
                PanelKeyValueRow::new(label, value).tone(tone).show(ui, t);
            }
        });
    }

    // ── Ownership ──
    if have_fund {
        PanelSection::new("OWNERSHIP")
        .show(ui, t, |ui, t| {
            for (label, value) in [
                ("Institutional",  format!("{:.1}%", f.institutional_pct)),
                ("Insider",        format!("{:.1}%", f.insider_pct)),
                ("Short Interest", format!("{:.1}%", f.short_interest)),
                ("Shares Out",     format!("{:.0}M", f.shares_outstanding / 1_000_000.0)),
            ] {
                PanelKeyValueRow::new(label, value).show(ui, t);
            }
        });
    }

    // ── Analyst Consensus ──
    PanelSection::new("ANALYST CONSENSUS")
        .show(ui, t, |ui, t| {
            let total = (f.analyst_buy + f.analyst_hold + f.analyst_sell) as f32;
            if total <= 0.0 {
                // W1-12 (audit): chart.fundamentals is only ever reset to
                // default — nothing FETCHES analyst data — so "No coverage" read
                // as "we checked, there's none". Disclose the missing feed
                // honestly instead, matching the Fundamentals section.
                PanelError::new("Analyst feed not connected")
                    .hint("Provider offline or not configured").show(ui, t);
                return;
            }
            ui.horizontal(|ui| {
                ui.add_space(gap_sm());
                let bar_w = ui.available_width() - gap_sm();
                let (bar_rect, _) = ui.allocate_exact_size(egui::vec2(bar_w, 10.0), egui::Sense::hover());
                let p = ui.painter();
                let buy_w = bar_w * f.analyst_buy as f32 / total;
                let hold_w = bar_w * f.analyst_hold as f32 / total;
                let sell_w = bar_w - buy_w - hold_w;
                p.rect_filled(egui::Rect::from_min_size(bar_rect.min, egui::vec2(buy_w, 10.0)),
                    egui::CornerRadius { nw: 3, sw: 3, ne: 0, se: 0 }, t.bull);
                p.rect_filled(egui::Rect::from_min_size(egui::pos2(bar_rect.left() + buy_w, bar_rect.top()),
                    egui::vec2(hold_w, 10.0)), 0.0, t.warn);
                p.rect_filled(egui::Rect::from_min_size(egui::pos2(bar_rect.left() + buy_w + hold_w, bar_rect.top()),
                    egui::vec2(sell_w, 10.0)), egui::CornerRadius { nw: 0, sw: 0, ne: 3, se: 3 }, t.bear);
            });
            ui.horizontal(|ui| {
                ui.add_space(gap_sm());
                ui.add(MonospaceCode::new(&format!("{} Buy",  f.analyst_buy)).xs().color(t.bull));
                ui.add(MonospaceCode::new(&format!("{} Hold", f.analyst_hold)).xs().color(t.warn));
                ui.add(MonospaceCode::new(&format!("{} Sell", f.analyst_sell)).xs().color(t.bear));
            });
            ui.add_space(gap_xs());
            ui.horizontal(|ui| {
                ui.add_space(gap_sm());
                ui.add(super::super::components::text::DimLabel::new("Price Targets:").color(t.dim));
            });
            ui.horizontal(|ui| {
                ui.add_space(gap_sm() + 4.0);
                ui.add(MonospaceCode::new(&format!("Low ${:.0}",  f.analyst_target_low)).xs().color(t.bear));
                ui.add(MonospaceCode::new(&format!("Mean ${:.0}", f.analyst_target_mean)).xs().color(t.accent));
                ui.add(MonospaceCode::new(&format!("High ${:.0}", f.analyst_target_high)).xs().color(t.bull));
            });
        });

    // ── Earnings History ──
    PanelSection::new("EARNINGS HISTORY")
        .show(ui, t, |ui, t| {
            if f.earnings.is_empty() {
                // W1-12: no fetch populates f.earnings — disclose, don't imply
                // this stock has no earnings history.
                PanelError::new("Earnings feed not connected")
                    .hint("Provider offline or not configured").show(ui, t);
                return;
            }
            for eq in &f.earnings {
                ui.horizontal(|ui| {
                    ui.add_space(gap_sm());
                    let surprise = if eq.eps_estimate > 0.0 {
                        (eq.eps_actual - eq.eps_estimate) / eq.eps_estimate * 100.0
                    } else { 0.0 };
                    let beat = surprise > 0.0;
                    let col = if beat { t.bull } else { t.bear };
                    ui.add(MonospaceCode::new(&eq.quarter).xs().color(t.dim));
                    ui.add(MonospaceCode::new(&format!("${:.2}", eq.eps_actual)).xs().color(t.text));
                    ui.add(MonospaceCode::new(&format!("vs ${:.2}", eq.eps_estimate)).xs().gamma(0.5));
                    ui.add(MonospaceCode::new(&format!("{}{:.1}%", if beat { "+" } else { "" }, surprise)).xs().color(col));
                });
            }
        });

    // ── Insider Trades ──
    PanelSection::new("INSIDER TRANSACTIONS")
        .show(ui, t, |ui, t| {
            if chart.insider_trades.is_empty() {
                // W1-12: chart.insider_trades is only ever cleared to Vec::new()
                // (gpu.rs), never populated — disclose the missing feed.
                PanelError::new("Insider feed not connected")
                    .hint("Provider offline or not configured").show(ui, t);
                return;
            }
            for trade in &chart.insider_trades {
                let is_buy = trade.shares > 0;
                let col = if is_buy { t.bull } else { t.bear };
                ui.horizontal(|ui| {
                    ui.add_space(gap_sm());
                    // Direction dot
                    let dot_pos = egui::pos2(ui.cursor().min.x + 4.0, ui.cursor().min.y + 7.0);
                    ui.painter().circle_filled(dot_pos, 3.0, col);
                    ui.add_space(gap_md());
                    ui.add(MonospaceCode::new(&trade.transaction).xs().color(col));
                    ui.add(MonospaceCode::new(&format!("{}K", trade.shares.abs() / 1000)).xs().color(t.text));
                    ui.add(MonospaceCode::new(&format!("${:.0}K", trade.value / 1000.0)).xs().color(t.dim));
                });
                ui.horizontal(|ui| {
                    ui.add_space(gap_sm() + 14.0);
                    ui.label(egui::RichText::new(&trade.name).monospace().size(font_2xs()).color(color_half(t.dim)));
                });
                ui.add_space(gap_xs());
            }
        });

    // ── Economic Calendar ──
    PanelSection::new("ECONOMIC CALENDAR")
        .show(ui, t, |ui, t| {
            if chart.econ_calendar.is_empty() {
                // W1-12: chart.econ_calendar is only ever cleared to Vec::new()
                // (gpu.rs), never populated — disclose the missing feed.
                PanelError::new("Economic calendar feed not connected")
                    .hint("Provider offline or not configured").show(ui, t);
                return;
            }
            for event in &chart.econ_calendar {
                let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() as i64;
                let days = ((event.time - now) as f64 / 86400.0).ceil() as i32;
                let imp_col = match event.importance { 3 => t.bear, 2 => t.warn, _ => t.dim };
                ui.horizontal(|ui| {
                    ui.add_space(gap_sm());
                    let dot_pos = egui::pos2(ui.cursor().min.x + 4.0, ui.cursor().min.y + 7.0);
                    ui.painter().circle_filled(dot_pos, 3.0, imp_col);
                    ui.add_space(gap_md());
                    ui.add(MonospaceCode::new(&event.name).xs().color(t.text));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add(MonospaceCode::new(&format!("{}d", days)).xs().color(t.dim));
                    });
                });
                ui.horizontal(|ui| {
                    ui.add_space(gap_sm() + 14.0);
                    ui.label(egui::RichText::new(format!("Forecast: {:.1}  Prev: {:.1}", event.forecast, event.previous))
                        .monospace().size(font_2xs()).color(color_dim(t.dim)));
                });
                ui.add_space(gap_xs());
            }
        });
}
