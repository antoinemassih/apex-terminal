//! Research panel — fundamentals, insider trades, analyst ratings, filings.

use egui;
use super::style::*;
use super::super::gpu::{Chart, Theme};
use super::widgets::form::{FormRow, FormRowAlign};
use super::widgets::text::{SectionLabel, MonospaceCode};

pub(crate) fn draw_content(
    ui: &mut egui::Ui,
    panes: &[Chart],
    ap: usize,
    t: &Theme,
) {
    if panes.is_empty() { return; }
    let chart = &panes[ap];
    let f = &chart.fundamentals;

    ui.add_space(GAP_SM);
    ui.add(SectionLabel::new(&format!("RESEARCH — {}", chart.symbol)).tiny().color(t.accent));
    ui.add_space(GAP_SM);

    // ── Valuation ──
    ui.add(SectionLabel::new("VALUATION").tiny().color(t.dim));
    ui.add_space(GAP_XS);
    let metrics = [
        ("P/E (TTM)", format!("{:.1}", f.pe_ratio)),
        ("Forward P/E", format!("{:.1}", f.forward_pe)),
        ("EPS (TTM)", format!("${:.2}", f.eps_ttm)),
        ("Market Cap", format!("${:.0}B", f.market_cap)),
        ("Div Yield", format!("{:.2}%", f.dividend_yield)),
        ("Beta", format!("{:.2}", f.beta)),
    ];
    for (label, value) in &metrics {
        FormRow::new(label)
            .label_left(true)
            .leading_space(GAP_SM)
            .alignment(FormRowAlign::Right)
            .show(ui, t, |ui| {
                ui.add(MonospaceCode::new(value).sm().color(t.text));
            });
    }

    ui.add_space(GAP_SM);
    separator(ui, color_alpha(t.toolbar_border, ALPHA_MUTED));
    ui.add_space(GAP_SM);

    // ── Financials ──
    ui.add(SectionLabel::new("FINANCIALS").tiny().color(t.dim));
    ui.add_space(GAP_XS);
    let financials = [
        ("Revenue Growth", format!("{:+.1}%", f.revenue_growth), if f.revenue_growth > 0.0 { t.bull } else { t.bear }),
        ("Profit Margin", format!("{:.1}%", f.profit_margin), if f.profit_margin > 15.0 { t.bull } else { t.dim }),
        ("Debt/Equity", format!("{:.2}", f.debt_to_equity), if f.debt_to_equity > 1.5 { t.bear } else { t.dim }),
    ];
    for (label, value, color) in &financials {
        FormRow::new(label)
            .label_left(true)
            .leading_space(GAP_SM)
            .alignment(FormRowAlign::Right)
            .show(ui, t, |ui| {
                ui.add(MonospaceCode::new(value).sm().color(*color));
            });
    }

    ui.add_space(GAP_SM);
    separator(ui, color_alpha(t.toolbar_border, ALPHA_MUTED));
    ui.add_space(GAP_SM);

    // ── Ownership ──
    ui.add(SectionLabel::new("OWNERSHIP").tiny().color(t.dim));
    ui.add_space(GAP_XS);
    let ownership = [
        ("Institutional", format!("{:.1}%", f.institutional_pct)),
        ("Insider", format!("{:.1}%", f.insider_pct)),
        ("Short Interest", format!("{:.1}%", f.short_interest)),
        ("Shares Out", format!("{:.0}M", f.shares_outstanding / 1_000_000.0)),
    ];
    for (label, value) in &ownership {
        FormRow::new(label)
            .label_left(true)
            .leading_space(GAP_SM)
            .alignment(FormRowAlign::Right)
            .show(ui, t, |ui| {
                ui.add(MonospaceCode::new(value).sm().color(t.text));
            });
    }

    ui.add_space(GAP_SM);
    separator(ui, color_alpha(t.toolbar_border, ALPHA_MUTED));
    ui.add_space(GAP_SM);

    // ── Analyst Consensus ──
    ui.add(SectionLabel::new("ANALYST CONSENSUS").tiny().color(t.dim));
    ui.add_space(GAP_XS);
    let total = (f.analyst_buy + f.analyst_hold + f.analyst_sell) as f32;
    if total > 0.0 {
        ui.horizontal(|ui| {
            ui.add_space(GAP_SM);
            let bar_w = ui.available_width() - GAP_SM;
            let (bar_rect, _) = ui.allocate_exact_size(egui::vec2(bar_w, 10.0), egui::Sense::hover());
            let p = ui.painter();
            let buy_w = bar_w * f.analyst_buy as f32 / total;
            let hold_w = bar_w * f.analyst_hold as f32 / total;
            let sell_w = bar_w - buy_w - hold_w;
            p.rect_filled(egui::Rect::from_min_size(bar_rect.min, egui::vec2(buy_w, 10.0)),
                egui::CornerRadius { nw: 3, sw: 3, ne: 0, se: 0 }, t.bull);
            p.rect_filled(egui::Rect::from_min_size(egui::pos2(bar_rect.left() + buy_w, bar_rect.top()),
                egui::vec2(hold_w, 10.0)), 0.0, egui::Color32::from_rgb(255, 191, 0));
            p.rect_filled(egui::Rect::from_min_size(egui::pos2(bar_rect.left() + buy_w + hold_w, bar_rect.top()),
                egui::vec2(sell_w, 10.0)), egui::CornerRadius { nw: 0, sw: 0, ne: 3, se: 3 }, t.bear);
        });
        ui.horizontal(|ui| {
            ui.add_space(GAP_SM);
            ui.add(MonospaceCode::new(&format!("{} Buy", f.analyst_buy)).xs().color(t.bull));
            ui.add(MonospaceCode::new(&format!("{} Hold", f.analyst_hold)).xs().color(egui::Color32::from_rgb(255, 191, 0)));
            ui.add(MonospaceCode::new(&format!("{} Sell", f.analyst_sell)).xs().color(t.bear));
        });
        ui.add_space(GAP_XS);
        ui.horizontal(|ui| {
            ui.add_space(GAP_SM);
            ui.add(super::widgets::text::DimLabel::new("Price Targets:").color(t.dim));
        });
        ui.horizontal(|ui| {
            ui.add_space(GAP_SM + 4.0);
            ui.add(MonospaceCode::new(&format!("Low ${:.0}", f.analyst_target_low)).xs().color(t.bear));
            ui.add(MonospaceCode::new(&format!("Mean ${:.0}", f.analyst_target_mean)).xs().color(t.accent));
            ui.add(MonospaceCode::new(&format!("High ${:.0}", f.analyst_target_high)).xs().color(t.bull));
        });
    }

    ui.add_space(GAP_SM);
    separator(ui, color_alpha(t.toolbar_border, ALPHA_MUTED));
    ui.add_space(GAP_SM);

    // ── Earnings History ──
    ui.add(SectionLabel::new("EARNINGS HISTORY").tiny().color(t.dim));
    ui.add_space(GAP_XS);
    for eq in &f.earnings {
        ui.horizontal(|ui| {
            ui.add_space(GAP_SM);
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

    ui.add_space(GAP_SM);
    separator(ui, color_alpha(t.toolbar_border, ALPHA_MUTED));
    ui.add_space(GAP_SM);

    // ── Insider Trades ──
    ui.add(SectionLabel::new("INSIDER TRANSACTIONS").tiny().color(t.dim));
    ui.add_space(GAP_XS);
    for trade in &chart.insider_trades {
        let is_buy = trade.shares > 0;
        let col = if is_buy { t.bull } else { t.bear };
        ui.horizontal(|ui| {
            ui.add_space(GAP_SM);
            // Direction dot
            let dot_pos = egui::pos2(ui.cursor().min.x + 4.0, ui.cursor().min.y + 7.0);
            ui.painter().circle_filled(dot_pos, 3.0, col);
            ui.add_space(10.0);
            ui.add(MonospaceCode::new(&trade.transaction).xs().color(col));
            ui.add(MonospaceCode::new(&format!("{}K", trade.shares.abs() / 1000)).xs().color(t.text));
            ui.add(MonospaceCode::new(&format!("${:.0}K", trade.value / 1000.0)).xs().color(t.dim));
        });
        ui.horizontal(|ui| {
            ui.add_space(GAP_SM + 14.0);
            ui.label(egui::RichText::new(&trade.name).monospace().size(7.0).color(t.dim.gamma_multiply(0.5)));
        });
        ui.add_space(GAP_XS);
    }

    ui.add_space(GAP_SM);
    separator(ui, color_alpha(t.toolbar_border, ALPHA_MUTED));
    ui.add_space(GAP_SM);

    // ── Economic Calendar ──
    ui.add(SectionLabel::new("ECONOMIC CALENDAR").tiny().color(t.dim));
    ui.add_space(GAP_XS);
    for event in &chart.econ_calendar {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() as i64;
        let days = ((event.time - now) as f64 / 86400.0).ceil() as i32;
        let imp_col = match event.importance { 3 => t.bear, 2 => egui::Color32::from_rgb(255, 191, 0), _ => t.dim };
        ui.horizontal(|ui| {
            ui.add_space(GAP_SM);
            let dot_pos = egui::pos2(ui.cursor().min.x + 4.0, ui.cursor().min.y + 7.0);
            ui.painter().circle_filled(dot_pos, 3.0, imp_col);
            ui.add_space(10.0);
            ui.add(MonospaceCode::new(&event.name).xs().color(t.text));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add(MonospaceCode::new(&format!("{}d", days)).xs().color(t.dim));
            });
        });
        ui.horizontal(|ui| {
            ui.add_space(GAP_SM + 14.0);
            ui.label(egui::RichText::new(format!("Forecast: {:.1}  Prev: {:.1}", event.forecast, event.previous))
                .monospace().size(7.0).color(t.dim.gamma_multiply(0.4)));
        });
        ui.add_space(GAP_XS);
    }
}
