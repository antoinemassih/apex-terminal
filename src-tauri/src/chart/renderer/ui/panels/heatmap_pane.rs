//! Heatmap pane — market/sector treemap visualization.

use egui;
use super::super::style::*;
use super::super::super::gpu::*;
use super::super::widgets::headers::PaneHeader;

/// Placeholder sector heatmap data.
struct HeatmapCell {
    symbol: &'static str,
    change_pct: f32,
    market_cap: f64, // determines cell size
    sector: &'static str,
}

pub(crate) fn render(
    ui: &mut egui::Ui, _ctx: &egui::Context,
    panes: &mut [Chart], pane_idx: usize, _active_pane: &mut usize,
    visible_count: usize, pane_rects: &[egui::Rect], theme_idx: usize,
    watchlist: &mut Watchlist,
) {
    let t = &THEMES[theme_idx];
    let rect_idx = 0;
    if rect_idx >= pane_rects.len() { return; }
    let rect = pane_rects[rect_idx];

    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, t.bg);
    if let Some(pos) = ui.ctx().pointer_hover_pos() {
        if rect.contains(pos) { *_active_pane = pane_idx; }
    }

    // ── Header (chrome widget) ─────────────────────────────────────────────────
    let header_h = 28.0;
    let header_rect = egui::Rect::from_min_size(rect.min, egui::vec2(rect.width(), header_h));
    {
        let mut header_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(header_rect)
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );
        header_ui.add(PaneHeader::new("Market Heatmap").theme(t));
    }

    // Placeholder data — S&P 500 top holdings by sector
    let cells = vec![
        HeatmapCell { symbol: "AAPL", change_pct: 1.2, market_cap: 3200.0, sector: "Tech" },
        HeatmapCell { symbol: "MSFT", change_pct: 0.8, market_cap: 3100.0, sector: "Tech" },
        HeatmapCell { symbol: "NVDA", change_pct: -2.1, market_cap: 2800.0, sector: "Tech" },
        HeatmapCell { symbol: "GOOG", change_pct: 0.3, market_cap: 2200.0, sector: "Tech" },
        HeatmapCell { symbol: "AMZN", change_pct: 1.5, market_cap: 2000.0, sector: "Consumer" },
        HeatmapCell { symbol: "META", change_pct: -0.6, market_cap: 1400.0, sector: "Tech" },
        HeatmapCell { symbol: "BRK.B", change_pct: 0.1, market_cap: 900.0, sector: "Finance" },
        HeatmapCell { symbol: "JPM", change_pct: -0.3, market_cap: 600.0, sector: "Finance" },
        HeatmapCell { symbol: "V", change_pct: 0.5, market_cap: 550.0, sector: "Finance" },
        HeatmapCell { symbol: "JNJ", change_pct: -0.8, market_cap: 400.0, sector: "Health" },
        HeatmapCell { symbol: "UNH", change_pct: 1.1, market_cap: 500.0, sector: "Health" },
        HeatmapCell { symbol: "XOM", change_pct: -1.5, market_cap: 450.0, sector: "Energy" },
        HeatmapCell { symbol: "CVX", change_pct: -0.9, market_cap: 300.0, sector: "Energy" },
        HeatmapCell { symbol: "PG", change_pct: 0.2, market_cap: 380.0, sector: "Consumer" },
        HeatmapCell { symbol: "HD", change_pct: 0.7, market_cap: 350.0, sector: "Consumer" },
        HeatmapCell { symbol: "DIS", change_pct: -1.8, market_cap: 200.0, sector: "Comms" },
        HeatmapCell { symbol: "NFLX", change_pct: 2.3, market_cap: 280.0, sector: "Comms" },
        HeatmapCell { symbol: "LLY", change_pct: 0.9, market_cap: 700.0, sector: "Health" },
        HeatmapCell { symbol: "AVGO", change_pct: -0.4, market_cap: 650.0, sector: "Tech" },
        HeatmapCell { symbol: "TSLA", change_pct: 3.1, market_cap: 800.0, sector: "Consumer" },
    ];

    // Simple treemap layout — squarified algorithm simplified
    let map_rect = egui::Rect::from_min_max(
        egui::pos2(rect.left() + 8.0, rect.top() + header_h),
        egui::pos2(rect.right() - 8.0, rect.bottom() - 8.0));
    let total_cap: f64 = cells.iter().map(|c| c.market_cap).sum();
    let map_area = map_rect.width() * map_rect.height();

    // Layout cells using simple slice-and-dice
    let mut remaining = map_rect;
    let mut horizontal = remaining.width() > remaining.height();

    for (i, cell) in cells.iter().enumerate() {
        let frac = (cell.market_cap / total_cap) as f32;
        let cell_area = map_area * frac;

        let cell_rect = if horizontal {
            let w = (cell_area / remaining.height().max(1.0)).min(remaining.width());
            let r = egui::Rect::from_min_size(remaining.min, egui::vec2(w, remaining.height()));
            remaining = egui::Rect::from_min_max(
                egui::pos2(remaining.left() + w, remaining.top()), remaining.max);
            r
        } else {
            let h = (cell_area / remaining.width().max(1.0)).min(remaining.height());
            let r = egui::Rect::from_min_size(remaining.min, egui::vec2(remaining.width(), h));
            remaining = egui::Rect::from_min_max(
                egui::pos2(remaining.left(), remaining.top() + h), remaining.max);
            r
        };

        if i % 3 == 0 { horizontal = !horizontal; } // alternate direction

        if cell_rect.width() < 2.0 || cell_rect.height() < 2.0 { continue; }

        // Color by change
        let intensity = (cell.change_pct.abs() / 3.0).clamp(0.0, 1.0);
        let base_color = if cell.change_pct >= 0.0 { t.bull } else { t.bear };
        let alpha = (intensity * 180.0 + 40.0) as u8;
        let bg = egui::Color32::from_rgba_unmultiplied(
            base_color.r(), base_color.g(), base_color.b(), alpha);

        // Cell — interactive (click to load symbol)
        let inset = egui::Rect::from_min_max(
            egui::pos2(cell_rect.left() + 1.0, cell_rect.top() + 1.0),
            egui::pos2(cell_rect.right() - 1.0, cell_rect.bottom() - 1.0));
        let cell_resp = ui.allocate_rect(inset, egui::Sense::click());
        let cell_hovered = cell_resp.hovered();
        let draw_bg = if cell_hovered {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            egui::Color32::from_rgba_unmultiplied(base_color.r(), base_color.g(), base_color.b(),
                (alpha as u16 + 40).min(255) as u8)
        } else { bg };
        painter.rect_filled(inset, 2.0, draw_bg);
        if cell_hovered {
            painter.rect_stroke(inset, 2.0, egui::Stroke::new(1.5, t.text), egui::StrokeKind::Outside);
        }
        if cell_resp.clicked() {
            // Load this symbol into pane 0 (or the active chart pane)
            panes[pane_idx].pane_type = PaneType::Chart;
            panes[pane_idx].pending_symbol_change = Some(cell.symbol.to_string());
        }

        // Symbol label (only if cell is big enough)
        if inset.width() > 30.0 && inset.height() > 20.0 {
            let font_size = if inset.width() > 80.0 && inset.height() > 40.0 { 14.0 }
                else if inset.width() > 50.0 { 10.0 }
                else { 7.0 };
            painter.text(inset.center() - egui::vec2(0.0, 6.0), egui::Align2::CENTER_CENTER,
                cell.symbol, egui::FontId::proportional(font_size), t.text);
            // Change %
            if inset.height() > 30.0 {
                painter.text(inset.center() + egui::vec2(0.0, 8.0), egui::Align2::CENTER_CENTER,
                    &format!("{:+.1}%", cell.change_pct), egui::FontId::monospace(font_size * 0.7),
                    if cell.change_pct >= 0.0 { t.text } else { t.text });
            }
        }
    }
}
