//! Heatmap pane — market/sector treemap visualization.
//!
//! Cold-start: pulls one day of full-market grouped bars from ApexData
//! (`/api/stocks/grouped/:date`) and treemaps them by dollar-volume.
//! Live updates ride the existing watchlist `set_price` path — for any
//! cell whose symbol is also in the watchlist, `change_pct` is rederived
//! from `(price - prev_close) / prev_close * 100` on each frame.

use egui;
use super::super::style::*;
use super::super::super::gpu::*;
use super::super::widgets::headers::PaneHeader;

/// Refresh the grouped-daily backing snapshot at most once an hour — the
/// data only changes on session close and intraday changes already flow
/// through the watchlist price path.
const HEATMAP_REFRESH_INTERVAL_SECS: u64 = 3600;

/// Treemap cell built from the backing data on each frame.
struct HeatmapCell {
    symbol: String,
    change_pct: f32,
    /// Dollar volume (vw * v) — proxy for market cap weight in the treemap.
    weight: f64,
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

    // ── Cold-start / refresh ───────────────────────────────────────────────
    let needs_fetch = match watchlist.heatmap_last_fetch {
        None => true,
        Some(t) => t.elapsed().as_secs() >= HEATMAP_REFRESH_INTERVAL_SECS,
    };
    if needs_fetch {
        watchlist.heatmap_last_fetch = Some(std::time::Instant::now());
        super::super::super::io::fetch::fetch_heatmap_cold_start();
    }

    // ── Build display cells from backing snapshot ──────────────────────────
    // Pick the top-N by dollar volume so the treemap stays legible. Override
    // change_pct with live watchlist data when present.
    const TOP_N: usize = 60;
    let mut cells: Vec<HeatmapCell> = {
        let mut src: Vec<(String, f32, f64)> = watchlist.heatmap_cells.clone();
        src.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
        src.truncate(TOP_N);
        src.into_iter().map(|(symbol, change_pct, weight)| {
            // Live override: if the watchlist has a fresh price for this symbol,
            // recompute change_pct from current price / prev_close.
            let live_change = watchlist.get_change_pct(&symbol);
            HeatmapCell {
                symbol,
                change_pct: live_change.unwrap_or(change_pct),
                weight,
            }
        }).collect()
    };

    if cells.is_empty() {
        // No data yet — empty pane (cold-start fetch is in flight).
        painter.text(rect.center(), egui::Align2::CENTER_CENTER,
            "Loading heatmap…", egui::FontId::proportional(12.0), t.dim);
        return;
    }

    // Best-effort stable layout: sort by weight desc (already done above), then
    // pass through to the slice-and-dice layout below.
    let _ = &mut cells;

    // Simple treemap layout — squarified algorithm simplified
    let map_rect = egui::Rect::from_min_max(
        egui::pos2(rect.left() + 8.0, rect.top() + header_h),
        egui::pos2(rect.right() - 8.0, rect.bottom() - 8.0));
    let total_cap: f64 = cells.iter().map(|c| c.weight).sum();
    let map_area = map_rect.width() * map_rect.height();

    // Layout cells using simple slice-and-dice
    let mut remaining = map_rect;
    let mut horizontal = remaining.width() > remaining.height();

    for (i, cell) in cells.iter().enumerate() {
        let frac = if total_cap > 0.0 { (cell.weight / total_cap) as f32 } else { 0.0 };
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
            painter.rect_stroke(inset, 2.0, egui::Stroke::new(stroke_bold(), t.text), egui::StrokeKind::Outside);
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
                &cell.symbol, egui::FontId::proportional(font_size), t.text);
            // Change %
            if inset.height() > 30.0 {
                painter.text(inset.center() + egui::vec2(0.0, 8.0), egui::Align2::CENTER_CENTER,
                    &format!("{:+.1}%", cell.change_pct), egui::FontId::monospace(font_size * 0.7),
                    if cell.change_pct >= 0.0 { t.text } else { t.text });
            }
        }
    }
}
