//! Portfolio pane — positions table, sector breakdown, risk analytics.

use egui;
use super::style::*;
use super::super::gpu::*;
use crate::chart_renderer::trading::{read_account_data, AccountSummary, Position};

pub(crate) fn render(
    ui: &mut egui::Ui, ctx: &egui::Context,
    panes: &mut [Chart], pane_idx: usize, active_pane: &mut usize,
    visible_count: usize, pane_rects: &[egui::Rect], theme_idx: usize,
    watchlist: &mut Watchlist, account_data: &Option<(AccountSummary, Vec<Position>, Vec<crate::chart_renderer::trading::IbOrder>)>,
) {
    let t = &THEMES[theme_idx];
    let rect_idx = if watchlist.maximized_pane.is_some() { 0 } else { pane_idx };
    if rect_idx >= pane_rects.len() { return; }
    let rect = pane_rects[rect_idx];

    // Background
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, t.bg);

    let margin = 16.0;
    let inner = egui::Rect::from_min_max(
        egui::pos2(rect.left() + margin, rect.top() + margin),
        egui::pos2(rect.right() - margin, rect.bottom() - margin));

    // ── Header ──
    painter.text(egui::pos2(inner.left(), inner.top() + 8.0), egui::Align2::LEFT_CENTER,
        "PORTFOLIO", egui::FontId::monospace(FONT_SM), t.dim);

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

    // ── Summary metrics bar ──
    let metrics_y = inner.top() + 24.0;
    let pnl_col = if total_pnl >= 0.0 { t.bull } else { t.bear };

    // Total Value
    painter.text(egui::pos2(inner.left(), metrics_y), egui::Align2::LEFT_CENTER,
        "TOTAL VALUE", egui::FontId::monospace(7.0), t.dim.gamma_multiply(0.5));
    painter.text(egui::pos2(inner.left(), metrics_y + 18.0), egui::Align2::LEFT_CENTER,
        &format!("${:.0}", total_value), egui::FontId::proportional(28.0), t.text);

    // P&L
    let pnl_x = inner.left() + 180.0;
    painter.text(egui::pos2(pnl_x, metrics_y), egui::Align2::LEFT_CENTER,
        "UNREALIZED P&L", egui::FontId::monospace(7.0), t.dim.gamma_multiply(0.5));
    let sign = if total_pnl >= 0.0 { "+" } else { "" };
    painter.text(egui::pos2(pnl_x, metrics_y + 18.0), egui::Align2::LEFT_CENTER,
        &format!("{}${:.0} ({:+.2}%)", sign, total_pnl, total_pnl_pct),
        egui::FontId::proportional(24.0), pnl_col);

    // Positions count
    let count_x = inner.left() + 420.0;
    if count_x < inner.right() {
        painter.text(egui::pos2(count_x, metrics_y), egui::Align2::LEFT_CENTER,
            "POSITIONS", egui::FontId::monospace(7.0), t.dim.gamma_multiply(0.5));
        painter.text(egui::pos2(count_x, metrics_y + 18.0), egui::Align2::LEFT_CENTER,
            &format!("{}", positions.len()), egui::FontId::proportional(24.0), t.accent);
    }

    // ── Separator ──
    let sep_y = metrics_y + 44.0;
    painter.line_segment(
        [egui::pos2(inner.left(), sep_y), egui::pos2(inner.right(), sep_y)],
        egui::Stroke::new(0.5, color_alpha(t.toolbar_border, ALPHA_MUTED)));

    // ── Positions table ──
    let table_top = sep_y + 8.0;
    let col_widths = [80.0, 50.0, 70.0, 70.0, 80.0, 60.0, 60.0]; // sym, qty, avg, current, P&L, %, port%
    let headers = ["SYMBOL", "QTY", "AVG", "CURRENT", "P&L", "P&L %", "% PORT"];
    let row_h = 24.0;

    // Header row
    let mut x = inner.left();
    for (i, header) in headers.iter().enumerate() {
        painter.text(egui::pos2(x, table_top + 4.0), egui::Align2::LEFT_CENTER,
            header, egui::FontId::monospace(7.0), t.dim.gamma_multiply(0.4));
        x += col_widths[i];
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

    // ── Sector breakdown (right side if space) ──
    let sector_x = inner.left() + 520.0;
    if sector_x + 150.0 < inner.right() {
        painter.text(egui::pos2(sector_x, table_top + 4.0), egui::Align2::LEFT_CENTER,
            "SECTOR ALLOCATION", egui::FontId::monospace(7.0), t.dim.gamma_multiply(0.4));

        // Simple donut
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
    }
}
