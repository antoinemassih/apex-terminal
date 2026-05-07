//! Heat tab — sector / index heatmap rendering, extracted from watchlist_panel.rs.
//!
//! The hardcoded universe arrays (`SP500_SECTORS`, `DOW30`, `QQQ100`) are kept as
//! module-level constants so they can later be swapped for a DB-backed
//! `symbol_universes` lookup in a single place.

use egui;
use super::super::style::*;
use super::super::super::gpu::{Watchlist, Theme};
use super::super::widgets::text::MonospaceCode;
use crate::ui_kit::widgets::Button;
use crate::ui_kit::widgets::tokens::Variant;
use crate::ui_kit::icons::Icon;

// ── Heat index dropdown options ────────────────────────────────────────────
const HEAT_OPTS: &[(&str, &str)] = &[
    ("Watchlist", "Watchlist"),
    ("S&P 500", "S&P 500"),
    ("Dow 30", "Dow 30"),
    ("Nasdaq 100", "Nasdaq 100"),
];

// ── Universe lookup ────────────────────────────────────────────────────────
// Phase (d): constituents come from `symbol_universes` (Polygon-backed via
// ApexData). The render thread reads from `watchlist_db::cached_universe`
// — a process-level RAM map populated by `watchlist::refresh`. Never
// blocks on Postgres or HTTP.

/// 11 SPDR sector ETFs in the order we want them rendered.
/// Tuple: (universe_name, display_label).
const SP500_SECTOR_UNIVERSES: &[(&str, &str)] = &[
    ("sp500_xlk",  "XLK Technology"),
    ("sp500_xlf",  "XLF Financials"),
    ("sp500_xlv",  "XLV Healthcare"),
    ("sp500_xly",  "XLY Consumer Disc."),
    ("sp500_xlc",  "XLC Communication"),
    ("sp500_xli",  "XLI Industrials"),
    ("sp500_xle",  "XLE Energy"),
    ("sp500_xlp",  "XLP Consumer Staples"),
    ("sp500_xlu",  "XLU Utilities"),
    ("sp500_xlre", "XLRE Real Estate"),
    ("sp500_xlb",  "XLB Materials"),
];

// (symbol, change%, sector)
type HeatItem = (String, f32, String);

pub(crate) fn render_heat_panel(
    ui: &mut egui::Ui,
    watchlist: &mut Watchlist,
    t: &Theme,
    active_sym: &str,
    pending_symbol: &mut Option<String>,
) {
    // Index preset dropdown + expand/collapse
    ui.horizontal(|ui| {
        {
            let mut cur: &'static str = HEAT_OPTS.iter().map(|&(v, _)| v).find(|&s| s == watchlist.heat_index.as_str()).unwrap_or("Watchlist");
            if super::super::widgets::select::Dropdown::new("heat_idx")
                .options(HEAT_OPTS)
                .width(100.0)
                .theme(t)
                .show(ui, &mut cur)
            {
                watchlist.heat_index = cur.to_string();
                watchlist.heat_collapsed.clear();
            }
        }
        // Expand / Collapse / Columns / Sort — all with hover cursor
        let hbtn = |ui: &mut egui::Ui, label: &str, col: egui::Color32, tip: &str| -> bool {
            let resp = ui.add(Button::new(label).variant(Variant::Chrome)
                .fg(col)
                .min_size(egui::vec2(20.0, 18.0))
                .corner_radius(current().r_md as f32)
                .frameless(true));
            if resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
            resp.on_hover_text(tip).clicked()
        };
        if hbtn(ui, Icon::PLUS, t.dim, "Expand all") { watchlist.heat_collapsed.clear(); }
        if hbtn(ui, Icon::MINUS, t.dim, "Collapse all") { watchlist.heat_collapsed.insert("__collapse_all__".into()); }
        let col_label = format!("{}c", watchlist.heat_cols);
        if hbtn(ui, &col_label, t.dim, "Toggle 1/2/3 columns") { watchlist.heat_cols = match watchlist.heat_cols { 1 => 2, 2 => 3, _ => 1 }; }
        let sort_label = match watchlist.heat_sort { 1 => Icon::ARROW_FAT_UP, -1 => Icon::ARROW_FAT_DOWN, _ => Icon::DOTS_THREE };
        let sort_col = if watchlist.heat_sort != 0 { t.accent } else { t.dim };
        if hbtn(ui, sort_label, sort_col, "Sort: gainers / losers / default") { watchlist.heat_sort = match watchlist.heat_sort { 0 => 1, 1 => -1, _ => 0 }; }
    });
    ui.add_space(gap_xs());

    // Pre-build price lookup from watchlist
    let price_map: std::collections::HashMap<String, f32> = watchlist.sections.iter()
        .flat_map(|sec| sec.items.iter())
        .filter(|i| i.price > 0.0 && i.prev_close > 0.0)
        .map(|i| (i.symbol.clone(), (i.price / i.prev_close - 1.0) * 100.0))
        .collect();
    let lookup = |s: &str| -> f32 { price_map.get(s).copied().unwrap_or(0.0) };

    // Build the (sector_label, [symbols]) groups from the cached universes.
    // For preset indexes other than the watchlist, an empty cache means the
    // refresh thread hasn't populated us yet — render a placeholder.
    let preset_groups: Option<Vec<(String, Vec<String>)>> = match watchlist.heat_index.as_str() {
        "S&P 500" => Some(
            SP500_SECTOR_UNIVERSES.iter()
                .map(|(name, label)| (label.to_string(), crate::watchlist_db::cached_universe(name)))
                .collect()
        ),
        "Dow 30" => Some(vec![("Dow".to_string(), crate::watchlist_db::cached_universe("dow30"))]),
        "Nasdaq 100" => Some(vec![("QQQ".to_string(), crate::watchlist_db::cached_universe("qqq100"))]),
        _ => None,
    };

    let heat_items: Vec<HeatItem> = if let Some(groups) = preset_groups.as_ref() {
        // If every group is empty, the cache is cold — drop through to
        // the placeholder branch below by leaving heat_items empty.
        let all_empty = groups.iter().all(|(_, syms)| syms.is_empty());
        if all_empty {
            Vec::new()
        } else {
            groups.iter().flat_map(|(sector, syms)| {
                syms.iter().map(|s| (s.clone(), lookup(s), sector.clone())).collect::<Vec<_>>()
            }).collect()
        }
    } else {
        watchlist.sections.iter().flat_map(|sec| sec.items.iter())
            .filter(|i| !i.is_option && i.loaded && i.price > 0.0)
            .map(|i| {
                let chg = if i.prev_close > 0.0 { (i.price / i.prev_close - 1.0) * 100.0 } else { 0.0 };
                (i.symbol.clone(), chg, "Watchlist".into())
            }).collect()
    };

    if heat_items.is_empty() {
        ui.add_space(24.0);
        let msg = if preset_groups.is_some() {
            "Loading universe data… check ApexData connectivity"
        } else {
            "No data — add symbols to watchlist"
        };
        ui.add(MonospaceCode::new(msg).size_px(font_sm_tight()).color(t.dim));
    } else {
        let mut heat_click_sym_outer: Option<String> = None;
        egui::ScrollArea::vertical().show(ui, |ui| {

            // Group by sector and render with dividers
            // Configurable N-column layout with click-to-chart
            let num_cols = watchlist.heat_cols.max(1) as usize;
            let heat_sort = watchlist.heat_sort;
            let render_sector_items = |ui: &mut egui::Ui, items_unsorted: &[&HeatItem], t: &Theme, _pm: &std::collections::HashMap<String, f32>, num_cols: usize, sort: i8, click_sym: &mut Option<String>, active_sym: &str| {
                let mut items: Vec<&HeatItem> = items_unsorted.to_vec();
                if sort == 1 { items.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal)); }
                else if sort == -1 { items.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal)); }
                let avail_w = ui.available_width();
                let gap = 3.0;
                let col_w = (avail_w - gap * (num_cols - 1) as f32) / num_cols as f32;
                let cell_h = if num_cols == 1 { 26.0 } else { 28.0 };
                let font_sz = if num_cols >= 3 { 10.0 } else { 12.0 };
                let max_pct = items.iter().map(|i| i.1.abs()).fold(1.0_f32, f32::max);
                let rows = (items.len() + num_cols - 1) / num_cols;
                let total_h = rows as f32 * cell_h;
                let (rect, resp) = ui.allocate_exact_size(egui::vec2(avail_w, total_h), egui::Sense::click());
                let painter = ui.painter();
                // Hover detection — find which cell the mouse is over
                let hover_idx: Option<usize> = ui.input(|i| i.pointer.hover_pos()).and_then(|pos| {
                    if !rect.contains(pos) { return None; }
                    let col = ((pos.x - rect.left()) / (col_w + gap)).floor() as usize;
                    let row = ((pos.y - rect.top()) / cell_h).floor() as usize;
                    let idx = row * num_cols + col;
                    if idx < items.len() { Some(idx) } else { None }
                });
                if hover_idx.is_some() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
                // Click detection
                if resp.clicked() {
                    if let Some(pos) = resp.interact_pointer_pos() {
                        let col = ((pos.x - rect.left()) / (col_w + gap)).floor() as usize;
                        let row = ((pos.y - rect.top()) / cell_h).floor() as usize;
                        let idx = row * num_cols + col;
                        if let Some(item) = items.get(idx) { *click_sym = Some(item.0.clone()); }
                    }
                }
                for (i, item) in items.iter().enumerate() {
                    let col = i % num_cols;
                    let row = i / num_cols;
                    let cx = rect.left() + col as f32 * (col_w + gap);
                    let cy = rect.top() + row as f32 * cell_h;
                    let intensity = (item.1.abs() / 5.0).min(1.0);
                    let is_up = item.1 >= 0.0;
                    let is_active = item.0 == active_sym;
                    let is_hovered = hover_idx == Some(i);
                    // Hover highlight
                    if is_hovered {
                        painter.rect_filled(egui::Rect::from_min_size(egui::pos2(cx, cy), egui::vec2(col_w, cell_h)),
                            2.0, color_alpha(t.text,12));
                    }
                    // Active symbol border
                    if is_active {
                        painter.rect_stroke(egui::Rect::from_min_size(egui::pos2(cx, cy + 1.0), egui::vec2(col_w, cell_h - 2.0)),
                            2.0, egui::Stroke::new(stroke_bold(), t.accent), egui::StrokeKind::Outside);
                    }
                    // Background bar
                    let bar_frac = if max_pct > 0.0 { item.1.abs() / max_pct } else { 0.0 };
                    let bar_w = bar_frac * col_w * 0.6;
                    let bar_col = if is_up {
                        color_alpha(t.bull, (25.0 + intensity * 55.0) as u8)
                    } else {
                        color_alpha(t.bear, (25.0 + intensity * 55.0) as u8)
                    };
                    painter.rect_filled(egui::Rect::from_min_size(egui::pos2(cx, cy + 1.0), egui::vec2(bar_w, cell_h - 2.0)), 2.0, bar_col);
                    // Edge strip
                    let edge_a = (120.0 + intensity * 135.0) as u8;
                    let edge_col = if is_up { color_alpha(t.bull, edge_a) } else { color_alpha(t.bear, edge_a) };
                    painter.rect_filled(egui::Rect::from_min_size(egui::pos2(cx, cy + 1.0), egui::vec2(3.0, cell_h - 2.0)), 0.0, edge_col);
                    // Symbol (bright white for active, slightly dimmer for others)
                    let sym_col = if is_active { egui::Color32::WHITE } else if is_hovered { color_alpha(t.text,230) } else { color_alpha(t.text,190) };
                    painter.text(egui::pos2(cx + 7.0, cy + cell_h / 2.0), egui::Align2::LEFT_CENTER,
                        &item.0, egui::FontId::monospace(font_sz), sym_col);
                    // Change%
                    let chg_col = if is_up { t.bull } else { t.bear };
                    painter.text(egui::pos2(cx + col_w - 3.0, cy + cell_h / 2.0), egui::Align2::RIGHT_CENTER,
                        &format!("{:+.1}%", item.1), egui::FontId::monospace(font_sz), chg_col);
                }
            };

            // Render grouped by sector
            let mut groups: Vec<(String, Vec<&HeatItem>)> = vec![];
            for item in &heat_items {
                if groups.last().map_or(true, |(s, _)| *s != item.2) {
                    groups.push((item.2.clone(), vec![]));
                }
                groups.last_mut().unwrap().1.push(item);
            }
            // Handle collapse-all
            if watchlist.heat_collapsed.contains("__collapse_all__") {
                watchlist.heat_collapsed.remove("__collapse_all__");
                for (s, _) in &groups { watchlist.heat_collapsed.insert(s.clone()); }
            }
            for (sector, items) in &groups {
                let is_collapsed = watchlist.heat_collapsed.contains(sector);
                // Sector avg change
                let avg_chg: f32 = if items.is_empty() { 0.0 } else {
                    items.iter().map(|i| i.1).sum::<f32>() / items.len() as f32
                };
                let sector_col = if avg_chg >= 0.0 { t.bull } else { t.bear };

                if groups.len() > 1 {
                    ui.add_space(4.0);
                    // Colored sector header — single clickable button
                    let caret = if is_collapsed { Icon::CARET_RIGHT } else { Icon::CARET_DOWN };
                    let header_text = format!("{} {}  ({})  {:+.2}%", caret, sector, items.len(), avg_chg);
                    let header_btn = ui.add(Button::new(header_text.as_str()).variant(Variant::Chrome)
                        .fg(sector_col)
                        .fill(color_alpha(sector_col, alpha_faint()))
                        .corner_radius(current().r_md as f32)
                        .min_size(egui::vec2(ui.available_width(), 22.0))
                        .frameless(true));
                    if header_btn.clicked() {
                        if is_collapsed { watchlist.heat_collapsed.remove(sector); }
                        else { watchlist.heat_collapsed.insert(sector.clone()); }
                    }
                    ui.add_space(4.0);
                }
                if !is_collapsed {
                    render_sector_items(ui, items, t, &price_map, num_cols, heat_sort, &mut heat_click_sym_outer, active_sym);
                }
            }
        });
        // Handle click-to-chart
        if let Some(sym) = heat_click_sym_outer {
            *pending_symbol = Some(sym);
        }
    }
}
