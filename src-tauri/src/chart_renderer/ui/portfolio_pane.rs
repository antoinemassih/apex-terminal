//! Portfolio pane — positions table, sector breakdown, risk analytics.

use egui;
use super::style::*;
use super::super::gpu::*;
use crate::chart_renderer::trading::{read_account_data, AccountSummary, Position};
use super::widgets::headers::PaneHeader;
use super::components::{
    metric_value_with_label, section_label_xs, monospace_label_row,
};

pub(crate) fn render(
    ui: &mut egui::Ui, ctx: &egui::Context,
    panes: &mut [Chart], pane_idx: usize, active_pane: &mut usize,
    visible_count: usize, pane_rects: &[egui::Rect], theme_idx: usize,
    watchlist: &mut Watchlist, account_data: &Option<(AccountSummary, Vec<Position>, Vec<crate::chart_renderer::trading::IbOrder>)>,
) {
    let t = &THEMES[theme_idx];
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
    let pnl_col = if total_pnl >= 0.0 { t.bull } else { t.bear };

    // ── Summary metrics bar (chrome widgets) ──────────────────────────────────
    let metrics_top = rect.top() + header_h + margin;
    let metrics_h = 48.0; // label + value rows
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
        let sign = if total_pnl >= 0.0 { "+" } else { "" };
        let pnl_str = format!("{}${:.0} ({:+.2}%)", sign, total_pnl, total_pnl_pct);
        metrics_ui.allocate_ui(egui::vec2(170.0, metrics_h), |ui| {
            metric_value_with_label(
                ui, "TOTAL VALUE",
                &format!("${:.0}", total_value),
                t.text, 34.0, None, t.dim.gamma_multiply(0.5),
            );
        });
        metrics_ui.allocate_ui(egui::vec2(230.0, metrics_h), |ui| {
            metric_value_with_label(
                ui, "UNREALIZED P&L",
                &pnl_str,
                pnl_col, 28.0, None, t.dim.gamma_multiply(0.5),
            );
        });
        if metrics_rect.width() > 420.0 {
            metrics_ui.allocate_ui(egui::vec2(100.0, metrics_h), |ui| {
                metric_value_with_label(
                    ui, "POSITIONS",
                    &format!("{}", positions.len()),
                    t.accent, 24.0, None, t.dim.gamma_multiply(0.5),
                );
            });
        }
    }

    // ── Separator ─────────────────────────────────────────────────────────────
    let sep_y = metrics_top + metrics_h + 4.0;
    painter.line_segment(
        [egui::pos2(inner.left(), sep_y), egui::pos2(inner.right(), sep_y)],
        egui::Stroke::new(0.5, color_alpha(t.toolbar_border, alpha_muted())));

    // ── Positions table ────────────────────────────────────────────────────────
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
                section_label_xs(ui, header, t.dim.gamma_multiply(0.4));
            });
        }
    }

    let data_top = table_top + 16.0;
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

        // Alternating row bg
        if ri % 2 == 1 {
            painter.rect_filled(egui::Rect::from_min_size(
                egui::pos2(inner.left() - 4.0, y), egui::vec2(inner.width() + 8.0, row_h)),
                0.0, color_alpha(t.toolbar_border, 8));
        }

        let mut cx = inner.left();
        // Symbol
        painter.text(egui::pos2(cx, y + row_h * 0.5), egui::Align2::LEFT_CENTER,
            &pos.symbol, egui::FontId::monospace(FONT_SM), t.text);
        cx += col_widths[0];
        // Qty
        painter.text(egui::pos2(cx, y + row_h * 0.5), egui::Align2::LEFT_CENTER,
            &format!("{}{}", if pos.qty > 0 { "+" } else { "" }, pos.qty),
            egui::FontId::monospace(FONT_XS), dir_c);
        cx += col_widths[1];
        // Avg
        painter.text(egui::pos2(cx, y + row_h * 0.5), egui::Align2::LEFT_CENTER,
            &format!("{:.2}", pos.avg_price), egui::FontId::monospace(FONT_XS), t.dim);
        cx += col_widths[2];
        // Current
        painter.text(egui::pos2(cx, y + row_h * 0.5), egui::Align2::LEFT_CENTER,
            &format!("{:.2}", pos.current_price), egui::FontId::monospace(FONT_XS), t.text);
        cx += col_widths[3];
        // P&L
        painter.text(egui::pos2(cx, y + row_h * 0.5), egui::Align2::LEFT_CENTER,
            &format!("{:+.0}", pos.unrealized_pnl), egui::FontId::monospace(FONT_SM), pnl_c);
        cx += col_widths[4];
        // P&L %
        painter.text(egui::pos2(cx, y + row_h * 0.5), egui::Align2::LEFT_CENTER,
            &format!("{:+.1}%", pnl_pct), egui::FontId::monospace(FONT_XS), pnl_c);
        cx += col_widths[5];
        // Port %
        painter.text(egui::pos2(cx, y + row_h * 0.5), egui::Align2::LEFT_CENTER,
            &format!("{:.1}%", port_pct), egui::FontId::monospace(FONT_XS), t.dim);
    }

    // ── Sector breakdown (right side if space) ─────────────────────────────────
    let sector_x = inner.left() + 520.0;
    if sector_x + 150.0 < inner.right() {
        // "SECTOR ALLOCATION" section label (chrome widget)
        {
            let sl_rect = egui::Rect::from_min_size(
                egui::pos2(sector_x, table_top),
                egui::vec2(200.0, 14.0));
            let mut sl_ui = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(sl_rect)
                    .layout(egui::Layout::left_to_right(egui::Align::Center)),
            );
            section_label_xs(&mut sl_ui, "SECTOR ALLOCATION", t.dim.gamma_multiply(0.4));
        }

        // Simple donut (sacred geometry — stays inline)
        let donut_cx = sector_x + 60.0;
        let donut_cy = table_top + 80.0;
        let donut_r = 40.0;
        let sectors = [("Tech", 0.55, t.accent), ("Finance", 0.15, t.bull),
                       ("Consumer", 0.12, t.bear), ("Index", 0.18, t.dim)];
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
                    egui::Stroke::new(10.0, color));
            }
            // Label
            let mid_a = angle + sweep * 0.5;
            let lx = donut_cx + (donut_r + 18.0) * mid_a.cos();
            let ly = donut_cy + (donut_r + 18.0) * mid_a.sin();
            painter.text(egui::pos2(lx, ly), egui::Align2::CENTER_CENTER,
                &format!("{} {:.0}%", label, frac * 100.0), egui::FontId::monospace(7.0), color);
            angle += sweep;
        }

        // ── Risk Metrics ──────────────────────────────────────────────────────
        let risk_y = donut_cy + donut_r + 30.0;
        if risk_y + 80.0 < inner.bottom() {
            // "RISK METRICS" section label (chrome widget)
            {
                let sl_rect = egui::Rect::from_min_size(
                    egui::pos2(sector_x, risk_y),
                    egui::vec2(160.0, 14.0));
                let mut sl_ui = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(sl_rect)
                        .layout(egui::Layout::left_to_right(egui::Align::Center)),
                );
                section_label_xs(&mut sl_ui, "RISK METRICS", t.dim.gamma_multiply(0.4));
            }

            let portfolio_beta = 1.12f32; // placeholder
            let var_95 = total_value * 0.018; // 1.8% daily VaR placeholder
            let margin_util = 42.0f32; // placeholder %

            let risk_items = [
                ("Beta", format!("{:.2}", portfolio_beta), t.text),
                ("VaR (95%)", format!("${:.0}", var_95), t.bear),
                ("Margin", format!("{:.0}%", margin_util),
                    if margin_util > 70.0 { t.bear } else if margin_util > 50.0 { egui::Color32::from_rgb(255, 191, 0) } else { t.bull }),
                ("Sharpe", format!("{:.2}", 1.45), t.accent),
            ];

            // Risk label/value rows (chrome widgets)
            {
                let rows_h = risk_items.len() as f32 * 16.0 + 16.0;
                let rows_rect = egui::Rect::from_min_size(
                    egui::pos2(sector_x, risk_y + 14.0),
                    egui::vec2(130.0, rows_h));
                let mut rows_ui = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(rows_rect)
                        .layout(egui::Layout::top_down(egui::Align::Min)),
                );
                for (label, value, color) in &risk_items {
                    monospace_label_row(&mut rows_ui, label, value, *color, t.dim.gamma_multiply(0.5));
                }
            }

            // Margin gauge (sacred geometry — stays inline)
            let gauge_y = risk_y + 82.0;
            if gauge_y + 12.0 < inner.bottom() {
                let gauge_w = 130.0;
                // "MARGIN UTILIZATION" label (chrome widget)
                {
                    let sl_rect = egui::Rect::from_min_size(
                        egui::pos2(sector_x, gauge_y - 12.0),
                        egui::vec2(gauge_w, 12.0));
                    let mut sl_ui = ui.new_child(
                        egui::UiBuilder::new()
                            .max_rect(sl_rect)
                            .layout(egui::Layout::left_to_right(egui::Align::Center)),
                    );
                    section_label_xs(&mut sl_ui, "MARGIN UTILIZATION", t.dim.gamma_multiply(0.4));
                }
                painter.rect_filled(egui::Rect::from_min_size(
                    egui::pos2(sector_x, gauge_y), egui::vec2(gauge_w, 6.0)),
                    3.0, color_alpha(t.toolbar_border, alpha_muted()));
                let fill_w = gauge_w * (margin_util / 100.0).min(1.0);
                let gauge_col = if margin_util > 70.0 { t.bear } else if margin_util > 50.0 { egui::Color32::from_rgb(255, 191, 0) } else { t.bull };
                painter.rect_filled(egui::Rect::from_min_size(
                    egui::pos2(sector_x, gauge_y), egui::vec2(fill_w, 6.0)),
                    3.0, gauge_col);
            }
        }

        // ── Correlation Mini-Matrix (sacred geometry) ─────────────────────────
        let corr_y = risk_y + 110.0;
        if corr_y + 100.0 < inner.bottom() {
            // "CORRELATION (top 5)" section label (chrome widget)
            {
                let sl_rect = egui::Rect::from_min_size(
                    egui::pos2(sector_x, corr_y),
                    egui::vec2(160.0, 14.0));
                let mut sl_ui = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(sl_rect)
                        .layout(egui::Layout::left_to_right(egui::Align::Center)),
                );
                section_label_xs(&mut sl_ui, "CORRELATION (top 5)", t.dim.gamma_multiply(0.4));
            }

            let syms: Vec<&str> = positions.iter().take(5).map(|p| p.symbol.as_str()).collect();
            let n = syms.len();
            let cell_sz = 18.0;
            let grid_x = sector_x;
            let grid_y = corr_y + 14.0;

            // Labels
            for (i, sym) in syms.iter().enumerate() {
                painter.text(egui::pos2(grid_x + 28.0 + i as f32 * cell_sz + cell_sz * 0.5, grid_y - 2.0),
                    egui::Align2::CENTER_BOTTOM, &sym[..sym.len().min(3)],
                    egui::FontId::monospace(6.0), t.dim.gamma_multiply(0.5));
                painter.text(egui::pos2(grid_x + 26.0, grid_y + i as f32 * cell_sz + cell_sz * 0.5),
                    egui::Align2::RIGHT_CENTER, &sym[..sym.len().min(4)],
                    egui::FontId::monospace(6.0), t.dim.gamma_multiply(0.5));
            }

            // Cells
            for row in 0..n {
                for col in 0..n {
                    let cx = grid_x + 28.0 + col as f32 * cell_sz;
                    let cy_pos = grid_y + row as f32 * cell_sz;
                    let corr = if row == col { 1.0f32 }
                        else { ((row as f32 * 3.7 + col as f32 * 5.3).sin() * 0.4 + 0.5).clamp(-0.3, 1.0) };
                    // Color: blue (negative) → white → red (positive)
                    let intensity = corr.abs();
                    let cell_col = if corr > 0.0 {
                        egui::Color32::from_rgba_unmultiplied(t.bear.r(), t.bear.g(), t.bear.b(), (intensity * 150.0) as u8)
                    } else {
                        egui::Color32::from_rgba_unmultiplied(t.accent.r(), t.accent.g(), t.accent.b(), (intensity * 150.0) as u8)
                    };
                    let cell_rect = egui::Rect::from_min_size(egui::pos2(cx, cy_pos), egui::vec2(cell_sz - 1.0, cell_sz - 1.0));
                    painter.rect_filled(cell_rect, 2.0, cell_col);
                    if row != col && cell_sz > 14.0 {
                        painter.text(cell_rect.center(), egui::Align2::CENTER_CENTER,
                            &format!("{:.1}", corr), egui::FontId::monospace(6.0), t.text);
                    }
                }
            }
        }

        // ── Scenario Simulator (sacred geometry) ──────────────────────────────
        let scenario_y = corr_y + 120.0;
        if scenario_y + 60.0 < inner.bottom() {
            // "SCENARIO: SPY -5%" section label (chrome widget)
            {
                let sl_rect = egui::Rect::from_min_size(
                    egui::pos2(sector_x, scenario_y),
                    egui::vec2(180.0, 14.0));
                let mut sl_ui = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(sl_rect)
                        .layout(egui::Layout::left_to_right(egui::Align::Center)),
                );
                section_label_xs(&mut sl_ui, "SCENARIO: SPY -5%", t.dim.gamma_multiply(0.4));
            }

            let spy_change = -5.0f32;
            let p_beta = 1.12f32;
            let portfolio_impact = total_value * (p_beta as f64 * spy_change as f64 / 100.0);
            let impact_pct = p_beta * spy_change;
            let impact_col = if portfolio_impact >= 0.0 { t.bull } else { t.bear };

            painter.text(egui::pos2(sector_x, scenario_y + 18.0), egui::Align2::LEFT_CENTER,
                &format!("${:+.0}", portfolio_impact), egui::FontId::proportional(20.0), impact_col);
            painter.text(egui::pos2(sector_x, scenario_y + 36.0), egui::Align2::LEFT_CENTER,
                &format!("{:+.1}% portfolio impact", impact_pct), egui::FontId::monospace(FONT_XS), impact_col);

            // Per-position impact (top 3)
            let mut impacts: Vec<(&str, f64)> = positions.iter()
                .map(|p| (p.symbol.as_str(), p.market_value * spy_change as f64 / 100.0 * 1.1)) // rough beta-adjusted
                .collect();
            impacts.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

            let imp_y = scenario_y + 50.0;
            for (i, (sym, imp)) in impacts.iter().take(3).enumerate() {
                if imp_y + i as f32 * 12.0 > inner.bottom() { break; }
                let c = if *imp >= 0.0 { t.bull } else { t.bear };
                painter.text(egui::pos2(sector_x, imp_y + i as f32 * 12.0), egui::Align2::LEFT_CENTER,
                    &format!("{}: ${:+.0}", sym, imp), egui::FontId::monospace(7.0), c);
            }
        }
    }
}
