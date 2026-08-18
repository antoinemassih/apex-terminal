//! Heat tab — sector / index heatmap rendering, extracted from watchlist_panel.rs.
//!
//! The hardcoded universe arrays (`SP500_SECTORS`, `DOW30`, `QQQ100`) are kept as
//! module-level constants so they can later be swapped for a DB-backed
//! `symbol_universes` lookup in a single place.

use egui;
use super::super::style::*;
use super::super::super::gpu::{Watchlist, Theme};
use super::super::components::text::MonospaceCode;
use crate::ui_kit::widgets::Button;
use crate::ui_kit::widgets::tokens::Variant;
use crate::ui_kit::icons::Icon;
use crate::ui_kit::widgets::{HeatmapGrid, HeatmapCell};

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
            let mut cur: &'static str = HEAT_OPTS.iter().map(|&(v, _)| v).find(|&s| s == watchlist.heat.index.as_str()).unwrap_or("Watchlist");
            if super::super::inputs::select::Dropdown::new()
                .options(HEAT_OPTS)
                .width(100.0)
                .theme(t)
                .show(ui, &mut cur)
            {
                watchlist.heat.index = cur.to_string();
                watchlist.heat.collapsed.clear();
            }
        }
        // Expand / Collapse / Columns / Sort — all with hover cursor
        let hbtn = |ui: &mut egui::Ui, label: &str, col: egui::Color32, tip: &str| -> bool {
            let resp = ui.add(Button::new(label).variant(Variant::Ghost)
                .fg(col)
                .min_size(egui::vec2(20.0, row_height_dense())));
            if resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
            resp.on_hover_text(tip).clicked()
        };
        if hbtn(ui, Icon::PLUS, t.dim, "Expand all") { watchlist.heat.collapsed.clear(); }
        if hbtn(ui, Icon::MINUS, t.dim, "Collapse all") { watchlist.heat.collapsed.insert("__collapse_all__".into()); }
        let col_label = format!("{}c", watchlist.heat.cols);
        if hbtn(ui, &col_label, t.dim, "Toggle 1/2/3 columns") { watchlist.heat.cols = match watchlist.heat.cols { 1 => 2, 2 => 3, _ => 1 }; }
        let sort_label = match watchlist.heat.sort { 1 => Icon::ARROW_FAT_UP, -1 => Icon::ARROW_FAT_DOWN, _ => Icon::DOTS_THREE };
        let sort_col = if watchlist.heat.sort != 0 { t.accent } else { t.dim };
        if hbtn(ui, sort_label, sort_col, "Sort: gainers / losers / default") { watchlist.heat.sort = match watchlist.heat.sort { 0 => 1, 1 => -1, _ => 0 }; }
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
    let preset_groups: Option<Vec<(String, Vec<String>)>> = match watchlist.heat.index.as_str() {
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
            // A heat tile IS its day change — colour and size both encode it.
            // A symbol with no previous close has nothing to render, and the
            // `else { 0.0 }` this replaces gave it a confident neutral-green
            // tile. The preset branch above already filtered on `prev_close`;
            // this branch did not, so the two disagreed about the same symbol.
            .filter_map(|i| {
                let chg = crate::foundation::market::day_change_pct(i.price, i.prev_close)?;
                Some((i.symbol.clone(), chg, "Watchlist".into()))
            }).collect()
    };

    if heat_items.is_empty() {
        ui.add_space(gap_2xl());
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
            let num_cols = watchlist.heat.cols.max(1) as usize;
            let heat_sort = watchlist.heat.sort;
            // render_sector_items extracted to HeatmapGrid widget

            // Render grouped by sector
            let mut groups: Vec<(String, Vec<&HeatItem>)> = vec![];
            for item in &heat_items {
                if groups.last().map_or(true, |(s, _)| *s != item.2) {
                    groups.push((item.2.clone(), vec![]));
                }
                groups.last_mut().unwrap().1.push(item);
            }
            // Handle collapse-all
            if watchlist.heat.collapsed.contains("__collapse_all__") {
                watchlist.heat.collapsed.remove("__collapse_all__");
                for (s, _) in &groups { watchlist.heat.collapsed.insert(s.clone()); }
            }
            for (sector, items) in &groups {
                let is_collapsed = watchlist.heat.collapsed.contains(sector);
                // Sector avg change
                let avg_chg: f32 = if items.is_empty() { 0.0 } else {
                    items.iter().map(|i| i.1).sum::<f32>() / items.len() as f32
                };
                let sector_col = if avg_chg >= 0.0 { t.bull } else { t.bear };

                if groups.len() > 1 {
                    ui.add_space(gap_xs());
                    // Colored sector header — single clickable button
                    let caret = if is_collapsed { Icon::CARET_RIGHT } else { Icon::CARET_DOWN };
                    let header_text = format!("{} {}  ({})  {:+.2}%", caret, sector, items.len(), avg_chg);
                    let header_btn = ui.add(Button::new(header_text.as_str()).variant(Variant::Chrome)
                        .fg(sector_col)
                        .fill(color_alpha(sector_col, alpha_faint()))
                        .corner_radius(crate::ui_kit::style::radius_md())
                        .min_size(egui::vec2(ui.available_width(), row_height_default()))
                        .frameless(true));
                    if header_btn.clicked() {
                        if is_collapsed { watchlist.heat.collapsed.remove(sector); }
                        else { watchlist.heat.collapsed.insert(sector.clone()); }
                    }
                    ui.add_space(gap_xs());
                }
                if !is_collapsed {
                    let mut sorted: Vec<&HeatItem> = items.to_vec();
                    if heat_sort == 1 { sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal)); }
                    else if heat_sort == -1 { sorted.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal)); }
                    let cells: Vec<HeatmapCell> = sorted.iter().map(|i| HeatmapCell { symbol: i.0.clone(), change_pct: i.1 }).collect();
                    let click_ref = &mut heat_click_sym_outer;
                    HeatmapGrid::new(&cells)
                        .num_cols(num_cols)
                        .active_symbol(Some(active_sym))
                        .on_click(|sym| { *click_ref = Some(sym.to_string()); })
                        .show(ui, t);
                }
            }
        });
        // Handle click-to-chart
        if let Some(sym) = heat_click_sym_outer {
            *pending_symbol = Some(sym);
        }
    }
}
