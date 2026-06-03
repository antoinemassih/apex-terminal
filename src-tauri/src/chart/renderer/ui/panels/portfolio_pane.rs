//! Portfolio pane — positions table, sector breakdown, risk analytics.

use egui;
use crate::ui_kit::sx::Tone;
use super::super::style::*;
use super::super::super::gpu::*;
use crate::chart_renderer::trading::{AccountSummary, Position};
use super::super::components::headers_widget::PaneHeader;
use super::super::components::text::SectionLabel;
use crate::ui_kit::widgets::{
    MetricRow, MetricTone, PanelKeyValueRow, PanelSection,
};
use crate::ui_kit::widgets::panel_section::Tone as PanelToneLocal;

pub(crate) fn render(
    ui: &mut egui::Ui, ctx: &egui::Context,
    panes: &mut [Chart], pane_idx: usize, active_pane: &mut usize,
    visible_count: usize, pane_rects: &[egui::Rect], theme_idx: usize,
    watchlist: &mut Watchlist, account_data: &Option<(AccountSummary, Vec<Position>, Vec<crate::chart_renderer::trading::IbOrder>)>,
) {
    let t_owned = crate::chart_renderer::gpu::get_theme(theme_idx); let t = &t_owned;
    let rect_idx = 0; // body rect passed as single-element slice
    if rect_idx >= pane_rects.len() { return; }
    let rect = pane_rects[rect_idx];

    // Background
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, t.bg);
    if let Some(pos) = ui.ctx().pointer_hover_pos() {
        if rect.contains(pos) { *active_pane = pane_idx; }
    }

    let margin = 16.0;
    let inner = egui::Rect::from_min_max(
        egui::pos2(rect.left() + margin, rect.top() + margin),
        egui::pos2(rect.right() - margin, rect.bottom() - margin));

    // ── Header (chrome widget) ─────────────────────────────────────────────────
    let header_h = 28.0;
    let header_rect = egui::Rect::from_min_size(
        egui::pos2(inner.left(), rect.top()),
        egui::vec2(inner.width(), header_h));
    {
        let mut header_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(header_rect)
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );
        header_ui.add(PaneHeader::new("Portfolio").theme(t));
    }

    // Get position data
    let (positions, summary) = if let Some((sum, pos, _)) = account_data {
        (pos.clone(), Some(sum.clone()))
    } else {
        // Placeholder positions
        let placeholder = vec![
            Position { symbol: "AAPL".into(), qty: 100, avg_price: 185.0, current_price: 192.50, market_value: 19250.0, unrealized_pnl: 750.0, con_id: 0 },
            Position { symbol: "NVDA".into(), qty: 50, avg_price: 120.0, current_price: 115.80, market_value: 5790.0, unrealized_pnl: -210.0, con_id: 0 },
            Position { symbol: "TSLA".into(), qty: -30, avg_price: 245.0, current_price: 238.0, market_value: 7140.0, unrealized_pnl: 210.0, con_id: 0 },
            Position { symbol: "MSFT".into(), qty: 75, avg_price: 415.0, current_price: 422.30, market_value: 31672.5, unrealized_pnl: 547.5, con_id: 0 },
            Position { symbol: "AMZN".into(), qty: 40, avg_price: 185.0, current_price: 190.20, market_value: 7608.0, unrealized_pnl: 208.0, con_id: 0 },
            Position { symbol: "META".into(), qty: 25, avg_price: 510.0, current_price: 495.0, market_value: 12375.0, unrealized_pnl: -375.0, con_id: 0 },
            Position { symbol: "GOOG".into(), qty: 60, avg_price: 168.0, current_price: 172.50, market_value: 10350.0, unrealized_pnl: 270.0, con_id: 0 },
            Position { symbol: "SPY".into(), qty: 200, avg_price: 560.0, current_price: 565.80, market_value: 113160.0, unrealized_pnl: 1160.0, con_id: 0 },
        ];
        (placeholder, None)
    };

    let total_value: f64 = positions.iter().map(|p| p.market_value).sum();
    let total_pnl: f64 = positions.iter().map(|p| p.unrealized_pnl).sum();
    let total_pnl_pct = if total_value > 0.0 { total_pnl / total_value * 100.0 } else { 0.0 };
    let pnl_tone = if total_pnl >= 0.0 { MetricTone::Bull } else { MetricTone::Bear };

    // ── Summary metrics bar — MetricRow (proportional, large value) ────────────
    let metrics_top = rect.top() + header_h + margin;
    let metrics_h = 48.0;
    let metrics_rect = egui::Rect::from_min_size(
        egui::pos2(inner.left(), metrics_top),
        egui::vec2(inner.width(), metrics_h));
    {
        let mut metrics_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(metrics_rect)
                .layout(egui::Layout::left_to_right(egui::Align::Min)),
        );

        // Total Value
        metrics_ui.allocate_ui(egui::vec2(170.0, metrics_h), |ui| {
            MetricRow::new("TOTAL VALUE")
                .value(format!("${:.0}", total_value))
                .tone(MetricTone::Default)
                .value_font(font_lg() * 2.0)
                .label_font(font_xs())
                .proportional()
                .show(ui, t);
        });

        // Unrealized P&L
        let sign = if total_pnl >= 0.0 { "+" } else { "" };
        let pnl_str = format!("{}${:.0} ({:+.2}%)", sign, total_pnl, total_pnl_pct);
        metrics_ui.allocate_ui(egui::vec2(230.0, metrics_h), |ui| {
            MetricRow::new("UNREALIZED P&L")
                .value(pnl_str)
                .tone(pnl_tone)
                .value_font(font_lg() + font_md())
                .label_font(font_xs())
                .proportional()
                .show(ui, t);
        });

        // Position count (only if enough width)
        if metrics_rect.width() > 420.0 {
            metrics_ui.allocate_ui(egui::vec2(100.0, metrics_h), |ui| {
                MetricRow::new("POSITIONS")
                    .value(format!("{}", positions.len()))
                    .tone(MetricTone::Accent)
                    .value_font(font_md() + font_sm())
                    .label_font(font_xs())
                    .proportional()
                    .show(ui, t);
            });
        }
    }

    // ── Separator ─────────────────────────────────────────────────────────────
    let sep_y = metrics_top + metrics_h + 4.0;
    // Hand-rolled: a single hairline rule — no primitive exists for a standalone
    // horizontal divider in an absolute-positioned context.
    ui.painter_at(rect).line_segment(
        [egui::pos2(inner.left(), sep_y), egui::pos2(inner.right(), sep_y)],
        egui::Stroke::new(stroke_thin(), tint(t, Tone::Border, alpha_muted())));

    // ── Positions table ────────────────────────────────────────────────────────
    // Hand-rolled: 7-column absolute-positioned table with per-cell colours.
    // PanelListRow::columns() requires a flow-layout UI; this section is fully
    // absolute and would need a larger layout refactor to adopt it.
    let table_top = sep_y + 8.0;
    let col_widths = [80.0, 50.0, 70.0, 70.0, 80.0, 60.0, 60.0]; // sym, qty, avg, current, P&L, %, port%
    let headers = ["SYMBOL", "QTY", "AVG", "CURRENT", "P&L", "P&L %", "% PORT"];
    let row_h = 24.0;

    // Header row (section label chrome)
    {
        let col_header_rect = egui::Rect::from_min_size(
            egui::pos2(inner.left(), table_top),
            egui::vec2(inner.width().min(col_widths.iter().sum()), 14.0));
        let mut col_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(col_header_rect)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );
        for (i, header) in headers.iter().enumerate() {
            col_ui.allocate_ui(egui::vec2(col_widths[i], 14.0), |ui| {
                ui.add(SectionLabel::new(header).xs().color(color_dim(t.dim)));
            });
        }
    }

    let data_top = table_top + 16.0;
    let painter = ui.painter_at(rect);
    for (ri, pos) in positions.iter().enumerate() {
        let y = data_top + ri as f32 * row_h;
        if y + row_h > inner.bottom() { break; }

        let pnl_c = if pos.unrealized_pnl >= 0.0 { t.bull } else { t.bear };
        let dir_c = if pos.qty > 0 { t.bull } else { t.bear };
        let pnl_pct = if pos.avg_price > 0.0 {
            (pos.current_price - pos.avg_price) / pos.avg_price * 100.0
                * if pos.qty < 0 { -1.0 } else { 1.0 }
        } else { 0.0 };
        let port_pct = if total_value > 0.0 { pos.market_value / total_value * 100.0 } else { 0.0 };

        // Alternating row bg — hand-rolled: no table primitive for this
        // absolute-positioned context.
        if ri % 2 == 1 {
            painter.rect_filled(egui::Rect::from_min_size(
                egui::pos2(inner.left() - 4.0, y), egui::vec2(inner.width() + 8.0, row_h)),
                0.0, tint(t, Tone::Border, alpha_faint()));
        }

        let mut cx = inner.left();
        painter.text(egui::pos2(cx, y + row_h * 0.5), egui::Align2::LEFT_CENTER,
            &pos.symbol, mono_sm(), t.text);
        cx += col_widths[0];
        painter.text(egui::pos2(cx, y + row_h * 0.5), egui::Align2::LEFT_CENTER,
            &format!("{}{}", if pos.qty > 0 { "+" } else { "" }, pos.qty),
            mono_xs(), dir_c);
        cx += col_widths[1];
        painter.text(egui::pos2(cx, y + row_h * 0.5), egui::Align2::LEFT_CENTER,
            &format!("{:.2}", pos.avg_price), mono_xs(), t.dim);
        cx += col_widths[2];
        painter.text(egui::pos2(cx, y + row_h * 0.5), egui::Align2::LEFT_CENTER,
            &format!("{:.2}", pos.current_price), mono_xs(), t.text);
        cx += col_widths[3];
        painter.text(egui::pos2(cx, y + row_h * 0.5), egui::Align2::LEFT_CENTER,
            &format!("{:+.0}", pos.unrealized_pnl), mono_sm(), pnl_c);
        cx += col_widths[4];
        painter.text(egui::pos2(cx, y + row_h * 0.5), egui::Align2::LEFT_CENTER,
            &format!("{:+.1}%", pnl_pct), mono_xs(), pnl_c);
        cx += col_widths[5];
        painter.text(egui::pos2(cx, y + row_h * 0.5), egui::Align2::LEFT_CENTER,
            &format!("{:.1}%", port_pct), mono_xs(), t.dim);
    }

    // ── Sector breakdown (right side if space) ─────────────────────────────────
    let sector_x = inner.left() + 520.0;
    if sector_x + 150.0 < inner.right() {
        // SECTOR ALLOCATION — PanelSection header (no body closure needed,
        // the content below is absolute-positioned).
        {
            let sl_rect = egui::Rect::from_min_size(
                egui::pos2(sector_x, table_top),
                egui::vec2(200.0, 14.0));
            let mut sl_ui = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(sl_rect)
                    .layout(egui::Layout::left_to_right(egui::Align::Center)),
            );
            sl_ui.add(SectionLabel::new("SECTOR ALLOCATION").xs().color(color_dim(t.dim)));
        }

        // ── Donut chart (hand-rolled: custom arc sweep geometry) ──────────────
        // donut_ring_w: arc ring visual thickness — intentional 10 px geometry,
        // not a chrome border; no design-system stroke token covers arc weight.
        let donut_ring_w: f32 = 10.0;
        let donut_cx = sector_x + 60.0;
        let donut_cy = table_top + 80.0;
        let donut_r = 40.0;
        let sectors = [("Tech", 0.55, t.accent), ("Finance", 0.15, t.bull),
                       ("Consumer", 0.12, t.bear), ("Index", 0.18, t.dim)];
        let painter = ui.painter_at(rect);
        let mut angle = -std::f32::consts::FRAC_PI_2;
        for (label, frac, color) in sectors {
            let sweep = frac * std::f32::consts::TAU;
            let segs = (sweep / 0.1).max(4.0) as usize;
            for s in 0..segs {
                let a0 = angle + s as f32 / segs as f32 * sweep;
                let a1 = angle + (s + 1) as f32 / segs as f32 * sweep;
                painter.line_segment([
                    egui::pos2(donut_cx + donut_r * a0.cos(), donut_cy + donut_r * a0.sin()),
                    egui::pos2(donut_cx + donut_r * a1.cos(), donut_cy + donut_r * a1.sin())],
                    egui::Stroke::new(donut_ring_w, color));
            }
            let mid_a = angle + sweep * 0.5;
            let lx = donut_cx + (donut_r + 18.0) * mid_a.cos();
            let ly = donut_cy + (donut_r + 18.0) * mid_a.sin();
            painter.text(egui::pos2(lx, ly), egui::Align2::CENTER_CENTER,
                &format!("{} {:.0}%", label, frac * 100.0), mono_sm(), color);
            angle += sweep;
        }

        // ── Risk Metrics ──────────────────────────────────────────────────────
        let risk_y = donut_cy + donut_r + 30.0;
        if risk_y + 80.0 < inner.bottom() {
            let portfolio_beta = 1.12f32;
            let var_95 = total_value * 0.018;
            let margin_util = 42.0f32;

            let margin_tone = if margin_util > 70.0 { MetricTone::Bear }
                else if margin_util > 50.0 { MetricTone::Warn }
                else { MetricTone::Bull };

            let risk_items: [(&str, String, MetricTone); 4] = [
                ("Beta",     format!("{:.2}", portfolio_beta), MetricTone::Default),
                ("VaR (95%)", format!("${:.0}", var_95),       MetricTone::Bear),
                ("Margin",   format!("{:.0}%", margin_util),  margin_tone),
                ("Sharpe",   format!("{:.2}", 1.45),           MetricTone::Accent),
            ];

            // RISK METRICS section — PanelSection wraps the KV rows.
            let rows_h = risk_items.len() as f32 * gap_lg() + gap_lg();
            let section_rect = egui::Rect::from_min_size(
                egui::pos2(sector_x, risk_y),
                egui::vec2(160.0, rows_h + 24.0)); // 24px for header
            {
                let mut section_ui = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(section_rect)
                        .layout(egui::Layout::top_down(egui::Align::Min)),
                );
                PanelSection::new("RISK METRICS")
                    .rule(true)
                    .show(&mut section_ui, t, |ui, t| {
                        for (label, value, tone) in &risk_items {
                            // PanelKeyValueRow: label LEFT, value RIGHT, themed tone.
                            // Bridges from MetricTone to panel_section::Tone via name.
                            let panel_tone = match tone {
                                MetricTone::Default => PanelToneLocal::Default,
                                MetricTone::Accent  => PanelToneLocal::Accent,
                                MetricTone::Bull    => PanelToneLocal::Bull,
                                MetricTone::Bear    => PanelToneLocal::Bear,
                                MetricTone::Warn    => PanelToneLocal::Warn,
                                MetricTone::Muted   => PanelToneLocal::Default,
                            };
                            PanelKeyValueRow::new(label, value.as_str())
                                .tone(panel_tone)
                                .show(ui, t);
                        }
                    });
            }

            // MARGIN UTILIZATION — MetricRow::bar() replaces 2 painter.rect_filled calls.
            let gauge_y = risk_y + rows_h + 24.0 + gap_sm();
            if gauge_y + gap_lg() < inner.bottom() {
                let gauge_rect = egui::Rect::from_min_size(
                    egui::pos2(sector_x, gauge_y - gap_sm()),
                    egui::vec2(130.0, gap_lg() * 3.0));
                let mut gauge_ui = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(gauge_rect)
                        .layout(egui::Layout::top_down(egui::Align::Min)),
                );
                // Section label for the gauge.
                gauge_ui.add(SectionLabel::new("MARGIN UTILIZATION").xs().color(color_dim(t.dim)));
                // MetricRow with bar — replaces the hand-rolled track + fill painter calls.
                MetricRow::new("Margin")
                    .value(format!("{:.0}%", margin_util))
                    .tone(margin_tone)
                    .bar(margin_util / 100.0)
                    .show(&mut gauge_ui, t);
            }
        }

        // ── Correlation Mini-Matrix (hand-rolled: custom 2D cell grid) ────────
        // No ui_kit primitive covers a correlation heatmap; colours route through theme.
        let risk_y = donut_cy + donut_r + 30.0;
        let rows_h = 4.0 * gap_lg() + gap_lg(); // same as above
        let corr_y = risk_y + rows_h + 24.0 + gap_sm() + gap_lg() * 3.0 + 24.0;
        if corr_y + 100.0 < inner.bottom() {
            {
                let sl_rect = egui::Rect::from_min_size(
                    egui::pos2(sector_x, corr_y),
                    egui::vec2(160.0, 14.0));
                let mut sl_ui = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(sl_rect)
                        .layout(egui::Layout::left_to_right(egui::Align::Center)),
                );
                sl_ui.add(SectionLabel::new("CORRELATION (top 5)").xs().color(color_dim(t.dim)));
            }

            let syms: Vec<&str> = positions.iter().take(5).map(|p| p.symbol.as_str()).collect();
            let n = syms.len();
            let cell_sz = 18.0;
            let grid_x = sector_x;
            let grid_y = corr_y + 14.0;

            let painter = ui.painter_at(rect);
            for (i, sym) in syms.iter().enumerate() {
                painter.text(egui::pos2(grid_x + 28.0 + i as f32 * cell_sz + cell_sz * 0.5, grid_y - 2.0),
                    egui::Align2::CENTER_BOTTOM, &sym[..sym.len().min(3)],
                    mono_sm(), color_half(t.dim));
                painter.text(egui::pos2(grid_x + 26.0, grid_y + i as f32 * cell_sz + cell_sz * 0.5),
                    egui::Align2::RIGHT_CENTER, &sym[..sym.len().min(4)],
                    mono_sm(), color_half(t.dim));
            }

            for row in 0..n {
                for col in 0..n {
                    let cx = grid_x + 28.0 + col as f32 * cell_sz;
                    let cy_pos = grid_y + row as f32 * cell_sz;
                    let corr = if row == col { 1.0f32 }
                        else { ((row as f32 * 3.7 + col as f32 * 5.3).sin() * 0.4 + 0.5).clamp(-0.3, 1.0) };
                    let intensity = corr.abs();
                    // Colours route through theme (bear = positive correlation,
                    // accent = negative) — no raw RGB.
                    let cell_col = if corr > 0.0 {
                        tint(t, Tone::Bear, (intensity * alpha_intense() as f32) as u8)
                    } else {
                        tint(t, Tone::Accent, (intensity * alpha_intense() as f32) as u8)
                    };
                    let cell_rect = egui::Rect::from_min_size(egui::pos2(cx, cy_pos), egui::vec2(cell_sz - 1.0, cell_sz - 1.0));
                    painter.rect_filled(cell_rect, radius_xs(), cell_col);
                    if row != col && cell_sz > 14.0 {
                        painter.text(cell_rect.center(), egui::Align2::CENTER_CENTER,
                            &format!("{:.1}", corr), mono_sm(), t.text);
                    }
                }
            }
        }

        // ── Scenario Simulator (hand-rolled: large value text + per-position rows) ──
        // No primitive for a single large coloured value + subordinate rows;
        // colours route through theme.
        let risk_y_base = donut_cy + donut_r + 30.0;
        let rows_h_base = 4.0 * gap_lg() + gap_lg();
        let corr_y_base = risk_y_base + rows_h_base + 24.0 + gap_sm() + gap_lg() * 3.0 + 24.0;
        let scenario_y = corr_y_base + 120.0;
        if scenario_y + 60.0 < inner.bottom() {
            {
                let sl_rect = egui::Rect::from_min_size(
                    egui::pos2(sector_x, scenario_y),
                    egui::vec2(180.0, 14.0));
                let mut sl_ui = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(sl_rect)
                        .layout(egui::Layout::left_to_right(egui::Align::Center)),
                );
                sl_ui.add(SectionLabel::new("SCENARIO: SPY -5%").xs().color(color_dim(t.dim)));
            }

            let spy_change = -5.0f32;
            let p_beta = 1.12f32;
            let portfolio_impact = total_value * (p_beta as f64 * spy_change as f64 / 100.0);
            let impact_pct = p_beta * spy_change;
            let impact_col = if portfolio_impact >= 0.0 { t.bull } else { t.bear };

            let painter = ui.painter_at(rect);
            painter.text(egui::pos2(sector_x, scenario_y + 18.0), egui::Align2::LEFT_CENTER,
                &format!("${:+.0}", portfolio_impact), egui::FontId::proportional(font_lg()), impact_col);
            painter.text(egui::pos2(sector_x, scenario_y + 36.0), egui::Align2::LEFT_CENTER,
                &format!("{:+.1}% portfolio impact", impact_pct), mono_xs(), impact_col);

            // Per-position impact (top 3) — hand-rolled inline text rows
            let mut impacts: Vec<(&str, f64)> = positions.iter()
                .map(|p| (p.symbol.as_str(), p.market_value * spy_change as f64 / 100.0 * 1.1))
                .collect();
            impacts.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

            let imp_y = scenario_y + 50.0;
            for (i, (sym, imp)) in impacts.iter().take(3).enumerate() {
                if imp_y + i as f32 * 12.0 > inner.bottom() { break; }
                let c = if *imp >= 0.0 { t.bull } else { t.bear };
                painter.text(egui::pos2(sector_x, imp_y + i as f32 * 12.0), egui::Align2::LEFT_CENTER,
                    &format!("{}: ${:+.0}", sym, imp), mono_sm(), c);
            }
        }
    }
}
