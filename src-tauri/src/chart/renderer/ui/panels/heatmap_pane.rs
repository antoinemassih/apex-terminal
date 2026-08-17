//! Heatmap pane — market/sector treemap visualization.
//!
//! Cold-start: pulls one day of full-market grouped bars from ApexData
//! (`/api/stocks/grouped/:date`) and treemaps them by dollar-volume.
//! Live updates ride the existing watchlist `set_price` path — for any
//! cell whose symbol is also in the watchlist, `change_pct` is rederived
//! from `(price - prev_close) / prev_close * 100` on each frame.

use egui;
use crate::ui_kit::sx::Tone;
use super::super::style::*;
use super::super::super::gpu::*;
use super::super::components::headers_widget::PaneHeader;

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
    let t_owned = crate::chart_renderer::gpu::get_theme(theme_idx); let t = &t_owned;
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
    let needs_fetch = match watchlist.heatmap.last_fetch {
        None => true,
        Some(t) => t.elapsed().as_secs() >= HEATMAP_REFRESH_INTERVAL_SECS,
    };
    if needs_fetch {
        watchlist.heatmap.last_fetch = Some(std::time::Instant::now());
        super::super::super::io::fetch::fetch_heatmap_cold_start();
    }

    // ── Build display cells from backing snapshot ──────────────────────────
    // Pick the top-N by dollar volume so the treemap stays legible. Override
    // change_pct with live watchlist data when present.
    const TOP_N: usize = 60;
    let mut cells: Vec<HeatmapCell> = {
        let mut src: Vec<(String, f32, f64)> = watchlist.heatmap.cells.clone();
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

    // ── Wave 10 additive header rows: breadth strip + sector rotation row ──
    // These read cached projector outputs from live_state (None → row hidden,
    // so the rest of the layout stays identical when the backend isn't up).
    const BREADTH_STRIP_H: f32 = 18.0;
    // Density-aware row height (was a hardcoded 22px).
    let sector_row_h: f32 = crate::chart_renderer::ui::style::style_row_height();

    let mut extra_h = 0.0f32;
    let breadth = crate::apex_data::live_state::get_breadth("us");
    let rotation = crate::apex_data::live_state::get_sector_rotation();

    if let Some(b) = breadth.as_ref() {
        let strip_rect = egui::Rect::from_min_max(
            egui::pos2(rect.left() + crate::ui_kit::style::gap_sm(), rect.top() + header_h + extra_h),
            egui::pos2(rect.right() - crate::ui_kit::style::gap_sm(), rect.top() + header_h + extra_h + BREADTH_STRIP_H),
        );
        painter.rect_filled(strip_rect, radius_xs(), tint(t, Tone::Surface, alpha_muted()));
        let txt = format!(
            "Adv {} / Dec {} · NH {} / NL {} · {:.0}% > SMA50 · {:.0}% > SMA200",
            b.advancers, b.decliners, b.new_highs, b.new_lows,
            b.pct_above_sma50, b.pct_above_sma200,
        );
        painter.text(
            egui::pos2(strip_rect.left() + crate::ui_kit::style::gap_xs_mid(), strip_rect.center().y),
            egui::Align2::LEFT_CENTER, txt,
            crate::ui_kit::style::mono_at(font_xs_plus()), t.text,
        );
        extra_h += BREADTH_STRIP_H + 2.0;
    }

    if let Some(r) = rotation.as_ref() {
        let row_rect = egui::Rect::from_min_max(
            egui::pos2(rect.left() + crate::ui_kit::style::gap_sm(), rect.top() + header_h + extra_h),
            egui::pos2(rect.right() - crate::ui_kit::style::gap_sm(), rect.top() + header_h + extra_h + sector_row_h),
        );
        let rows = &r.rows;
        if !rows.is_empty() {
            let cell_w = row_rect.width() / rows.len() as f32;
            for (i, row) in rows.iter().enumerate() {
                let cr = egui::Rect::from_min_size(
                    egui::pos2(row_rect.left() + i as f32 * cell_w, row_rect.top()),
                    egui::vec2(cell_w - 1.0, row_rect.height()),
                );
                let color = quadrant_color(row.quadrant, t);
                painter.rect_filled(cr, radius_xs(), color);
                painter.text(cr.center(), egui::Align2::CENTER_CENTER,
                    &row.symbol, crate::ui_kit::style::mono_xs(), t.text);
            }
            extra_h += sector_row_h + 2.0;
        }
    }

    if cells.is_empty() {
        let msg = if watchlist.heatmap.last_fetch
            .map_or(false, |inst| inst.elapsed().as_secs() > 15)
        {
            "Heatmap unavailable — check ApexData connection"
        } else {
            "Loading heatmap…"
        };
        painter.text(rect.center(), egui::Align2::CENTER_CENTER,
            msg, crate::ui_kit::style::prop_at(crate::ui_kit::style::font_sm()), t.dim);
        return;
    }

    // Best-effort stable layout: sort by weight desc (already done above), then
    // pass through to the slice-and-dice layout below.
    let _ = &mut cells;

    // Simple treemap layout — squarified algorithm simplified.
    // Top edge slides down by `extra_h` to accommodate the additive rows above.
    let map_rect = egui::Rect::from_min_max(
        egui::pos2(rect.left() + crate::ui_kit::style::gap_sm(), rect.top() + header_h + extra_h),
        egui::pos2(rect.right() - crate::ui_kit::style::gap_sm(), rect.bottom() - crate::ui_kit::style::gap_sm()));
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
        let bg = crate::ui_kit::style::color_alpha(base_color, alpha);

        // Cell — interactive (click to load symbol)
        let inset = egui::Rect::from_min_max(
            egui::pos2(cell_rect.left() + 1.0, cell_rect.top() + 1.0),
            egui::pos2(cell_rect.right() - 1.0, cell_rect.bottom() - 1.0));
        let cell_resp = ui.allocate_rect(inset, egui::Sense::click());
        let cell_hovered = cell_resp.hovered();
        let draw_bg = if cell_hovered {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            crate::ui_kit::style::color_alpha(base_color, (alpha as u16 + 40).min(255) as u8)
        } else { bg };
        painter.rect_filled(inset, radius_xs(), draw_bg);
        if cell_hovered {
            painter.rect_stroke(inset, radius_xs(), egui::Stroke::new(stroke_bold(), t.text), egui::StrokeKind::Outside);
        }
        // Focus ring for keyboard navigation (egui Sense::click() already
        // fires clicked() on Enter/Space when focused).
        cursor::focus_ring(ui, &cell_resp, t.accent);
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
                &cell.symbol, crate::ui_kit::style::prop_at(font_size), t.text);
            // Change %
            if inset.height() > 30.0 {
                // The colour used to be `if change_pct >= 0.0 { t.text } else
                // { t.text }` — a bull/bear branch that selected the same tone
                // twice. Not restored to bull/bear on purpose: the CELL FILL
                // already encodes the sign (`draw_bg` above is the heat
                // colour), so tinting the label as well would say it a second
                // time and fight the fill for contrast. The label's job here is
                // to be readable ON that fill.
                painter.text(inset.center() + egui::vec2(0.0, 8.0), egui::Align2::CENTER_CENTER,
                    &format!("{:+.1}%", cell.change_pct), crate::ui_kit::style::mono_at(font_size * 0.7),
                    t.text);
            }
        }
    }
}

/// Map an RRG quadrant to a theme color band:
///   Leading    → bull (green)
///   Weakening  → muted bull (yellow-ish — falls back to dimmed bull)
///   Lagging    → bear  (red)
///   Improving  → accent (blue/cyan)
///   Unknown    → toolbar_bg (neutral)
fn quadrant_color(q: crate::apex_data::types::RotationQuadrant, t: &Theme) -> egui::Color32 {
    use crate::apex_data::types::RotationQuadrant as Q;
    // RRG convention:
    //   Leading   → green   (full strength)
    //   Weakening → yellow  (rolling over from leading)
    //   Lagging   → red     (full weakness)
    //   Improving → blue    (rolling up from lagging)
    // We don't always have palette-distinct accent/warn, so guarantee uniqueness
    // by giving each quadrant a distinct alpha tier on top of its base hue.
    let base = match q {
        Q::Leading   => t.bull,
        Q::Weakening => t.warn,
        Q::Lagging   => t.bear,
        Q::Improving => t.accent,
        Q::Unknown   => t.toolbar_bg,
    };
    let alpha: u8 = match q {
        Q::Leading   => 200,
        Q::Weakening => 170,
        Q::Lagging   => 200,
        Q::Improving => 150,
        Q::Unknown   => 120,
    };
    crate::ui_kit::style::color_alpha(base, alpha)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::apex_data::types::{RotationQuadrant, SectorRotationReading, SectorRotationRow};

    #[test]
    fn quadrant_colors_are_distinct_for_real_quadrants() {
        // Build a SectorRotationReading covering the 4 real quadrants; the
        // helper must return four distinct colors so the header row is legible.
        let r = SectorRotationReading {
            benchmark: "SPY".into(), session_date: "2026-05-17".into(),
            rows: vec![
                SectorRotationRow { symbol: "XLK".into(), quadrant: RotationQuadrant::Leading,   ..Default::default() },
                SectorRotationRow { symbol: "XLF".into(), quadrant: RotationQuadrant::Improving, ..Default::default() },
                SectorRotationRow { symbol: "XLE".into(), quadrant: RotationQuadrant::Lagging,   ..Default::default() },
                SectorRotationRow { symbol: "XLY".into(), quadrant: RotationQuadrant::Weakening, ..Default::default() },
            ],
            updated_at_ms: 0,
        };
        let t = THEMES.iter().find(|t| t.name == "Midnight")
            .expect("Midnight theme must exist");
        let colors: Vec<egui::Color32> = r.rows.iter().map(|row| quadrant_color(row.quadrant, t)).collect();
        // All four should be distinct.
        for i in 0..colors.len() {
            for j in (i+1)..colors.len() {
                assert_ne!(colors[i], colors[j],
                    "quadrants {:?} and {:?} produced the same color",
                    r.rows[i].quadrant, r.rows[j].quadrant);
            }
        }
    }
}
