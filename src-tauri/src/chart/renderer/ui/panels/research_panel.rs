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
use super::super::widgets::text::MonospaceCode;
use crate::ui_kit::widgets::{PanelEmpty, PanelKeyValueRow, PanelSection, PanelTone};
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

    // ── Valuation ──
    PanelSection::new(&format!("VALUATION — {}", chart.symbol))
        .title_color(t.accent)
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

    // ── Financials ──
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

    // ── Ownership ──
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

    // ── Analyst Consensus ──
    PanelSection::new("ANALYST CONSENSUS")
        .show(ui, t, |ui, t| {
            let total = (f.analyst_buy + f.analyst_hold + f.analyst_sell) as f32;
            if total <= 0.0 {
                PanelEmpty::new("No analyst coverage").show(ui, t);
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
                ui.add(super::super::widgets::text::DimLabel::new("Price Targets:").color(t.dim));
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
                PanelEmpty::new("No earnings history").show(ui, t);
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
                PanelEmpty::new("No insider activity").show(ui, t);
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
                    ui.add_space(12.0);
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
                PanelEmpty::new("No upcoming events").show(ui, t);
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
                    ui.add_space(12.0);
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
