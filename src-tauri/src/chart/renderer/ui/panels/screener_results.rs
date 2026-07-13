//! Screener Results panel — T-GRID wave S2.
//!
//! Renders the results grid for a saved scan (view 3 in the 5-view screener
//! UX defined in `docs/audit/screener-next/06-terminal-ux.md`).
//!
//! # Layout
//! - Sortable column headers (symbol + score + user-selected operand columns).
//! - One `PanelListRow::columns` row per match — slice-based, no per-row heap alloc.
//! - `row_tint` encodes severity / threshold: green when score >= 0.7,
//!   amber near, transparent when below.
//! - Sparkline cell: ~30-line inline polyline drawn into a small rect inside
//!   the row. Mini-bars from `GET /api/bars/stocks/:sym/1m` fetched once per
//!   symbol on first render via a background thread. NOT from core.rs.
//! - Row click → `AppCommand::ScreenRowToChart(sym)` (caller dispatches).
//! - Trailing icon buttons: chart / watch / alert / mute via
//!   `PanelListRow::trailing_buttons`.
//!
//! # State contract (T-SHELL owns ScreenPanelState)
//! This module reads:
//! - `screen_state.results_rows: Vec<ScreenResultRow>`
//! - `screen_state.result_sort_col: usize` + `screen_state.result_sort_asc: bool`
//! - `screen_state.result_muted: HashSet<String>`
//! - `screen_state.selected_columns: Vec<String>`
//!
//! # AppCommand needs (request to T-SHELL)
//! - `AppCommand::ScreenRowToChart(sym: String)` — load sym in active pane
//! - `AppCommand::MuteScreenSymbol(sym: String)` — toggle mute
//! - `AppCommand::WatchlistAddSymbol { symbol }` — already exists

use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

use egui::{self, Color32, Pos2, Rect, Stroke, Vec2};

use super::super::style::*;
use super::super::super::gpu::Theme;
use crate::ui_kit::icons::Icon;
use crate::ui_kit::widgets::{
    PanelColAlign, PanelColumn, PanelEmpty, PanelListRow, PanelSection,
    TrailingBtn, TrailingTone,
};

// ── Sparkline cache ──────────────────────────────────────────────────────────

/// A cached sparkline for one symbol: normalised [0..1] close prices +
/// the fetch timestamp. TTL = 60 s; after expiry the next render re-fetches.
#[derive(Clone)]
struct SparkCache {
    points: Vec<f32>,
    fetched_at: Instant,
}

static SPARK: OnceLock<Mutex<HashMap<String, SparkCache>>> = OnceLock::new();
static SPARK_INFLIGHT: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();

fn spark_store() -> &'static Mutex<HashMap<String, SparkCache>> {
    SPARK.get_or_init(|| Mutex::new(HashMap::new()))
}
fn spark_inflight() -> &'static Mutex<HashSet<String>> {
    SPARK_INFLIGHT.get_or_init(|| Mutex::new(HashSet::new()))
}

const SPARK_TTL_SECS: u64 = 60;
const SPARK_BARS: usize = 30;

/// Return cached sparkline (normalised) for `sym`, or `None` while fetching.
/// On first call (or after TTL expiry) kicks off a background thread.
fn get_or_fetch_sparkline(sym: &str) -> Option<Vec<f32>> {
    // Fast path: valid cache hit.
    if let Ok(cache) = spark_store().lock() {
        if let Some(entry) = cache.get(sym) {
            if entry.fetched_at.elapsed().as_secs() < SPARK_TTL_SECS {
                return Some(entry.points.clone());
            }
        }
    }

    // Kick off fetch if not already in-flight.
    {
        let mut inflight = spark_inflight().lock().ok()?;
        if inflight.contains(sym) { return None; }
        inflight.insert(sym.to_string());
    }

    let sym_owned = sym.to_string();
    std::thread::spawn(move || {
        // Use the existing apex_data REST layer — blocking, safe from a worker thread.
        use crate::apex_data::types::{AssetClass, BarSource};

        let result = crate::apex_data::rest::get_bars(
            AssetClass::Stock, &sym_owned, "1m", BarSource::Last,
        );
        let points: Vec<f32> = match result {
            Ok(bars) => {
                let closes: Vec<f64> = bars.iter()
                    .rev().take(SPARK_BARS).rev()
                    .map(|b| b.close)
                    .collect();
                if closes.len() < 2 { vec![0.5, 0.5] } else {
                    let lo = closes.iter().cloned().fold(f64::MAX, f64::min);
                    let hi = closes.iter().cloned().fold(f64::MIN, f64::max);
                    let range = (hi - lo).max(1e-9);
                    closes.iter().map(|&c| ((c - lo) / range) as f32).collect()
                }
            }
            Err(_) => vec![0.5, 0.5],
        };

        if let Ok(mut cache) = spark_store().lock() {
            cache.insert(sym_owned.clone(), SparkCache { points, fetched_at: Instant::now() });
        }
        if let Ok(mut inflight) = spark_inflight().lock() {
            inflight.remove(&sym_owned);
        }
    });

    None
}

// ── Sparkline paint ──────────────────────────────────────────────────────────

/// Paint a mini sparkline polyline into `rect`.
/// `points` are normalised [0..1] y-values in chronological order.
/// Color = bull if last >= first, bear otherwise.
fn paint_sparkline(ui: &mut egui::Ui, rect: Rect, points: &[f32], t: &Theme) {
    if points.len() < 2 { return; }
    let color = if points.last().copied().unwrap_or(0.5)
        >= points.first().copied().unwrap_or(0.5) { t.bull } else { t.bear };
    let stroke = Stroke::new(stroke_thin(), color);
    let w = rect.width();
    let h = rect.height();
    let step = w / (points.len() - 1).max(1) as f32;

    let path: Vec<Pos2> = points.iter().enumerate().map(|(i, &y_norm)| {
        Pos2::new(
            rect.left() + i as f32 * step,
            rect.bottom() - y_norm * h,
        )
    }).collect();

    let painter = ui.painter_at(rect);
    for win in path.windows(2) {
        painter.line_segment([win[0], win[1]], stroke);
    }
}

// ── Row data type ─────────────────────────────────────────────────────────────

/// One screener result row — canonical type shared by Grid, Race, and Heatmap views.
///
/// Field names match what ApexData S1 SSE returns: `symbol`, `score`, `values`.
/// The `sector` field comes from the `ref:sector` operand resolved during the scan;
/// empty string means "Other" for heatmap grouping.
///
/// **Canonical**: `screener_heatmap.rs` aliases this as `ScreenerRow` for backward compat.
/// The `aggregates.rs` `ScreenResultRow` (runtime-only, f32) is a separate type used
/// for the persisted-state-adjacent race buffer; this one is the live render type.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ScreenResultRow {
    pub symbol: String,
    /// Primary rank score (from `rank_by` operand, or first alphabetical).
    pub score: Option<f64>,
    /// All resolved operand values keyed by operand key string.
    pub values: HashMap<String, f64>,
    /// Sector label resolved from `ref:sector` (e.g. "Technology"). Empty = "Other".
    pub sector: String,
}

impl ScreenResultRow {
    /// Extract an operand value as f32, defaulting to 0.0.
    pub fn get_f32(&self, key: &str) -> f32 {
        self.values.get(key).copied().unwrap_or(0.0) as f32
    }
}

// ── Severity tint ─────────────────────────────────────────────────────────────

fn severity_tint(score: Option<f64>, t: &Theme) -> (Color32, u8) {
    let Some(s) = score else { return (t.dim, 0); };
    if s >= 0.7 { (t.bull, 18) }
    else if s >= 0.4 { (t.warn, 14) }
    else { (t.dim, 0) }
}

// ── Public draw entry ─────────────────────────────────────────────────────────

/// Draw the screener results grid into `ui`.
///
/// Called by T-SHELL's tab dispatch when `view_mode == ScreenerView::Results`.
///
/// Outputs are written to `pending_*` out-params; the caller (T-SHELL shell)
/// dispatches the appropriate `AppCommand` after the frame.
pub fn draw_results(
    ui: &mut egui::Ui,
    rows: &[ScreenResultRow],
    selected_cols: &[String],
    sort_col: &mut usize,
    sort_asc: &mut bool,
    muted: &HashSet<String>,
    t: &Theme,
    pending_sym_to_chart: &mut Option<String>,
    pending_watch: &mut Option<String>,
    pending_mute: &mut Option<String>,
) {
    let panel_w = ui.available_width();

    // ── Column widths ─────────────────────────────────────────────────────
    let trailing_w = 76.0_f32;
    let sym_w      = 56.0_f32;
    let spark_w    = 40.0_f32;
    let score_w    = 48.0_f32;
    let n_extra    = selected_cols.len();
    let op_w = if n_extra > 0 {
        ((panel_w - sym_w - spark_w - score_w - trailing_w).max(0.0)
            / n_extra as f32).max(40.0)
    } else { 0.0 };

    // ── Column header row ─────────────────────────────────────────────────
    ui.horizontal(|ui| {
        ui.add_space(gap_2xs());
        let hdr = color_muted(t.dim);

        let make_hdr = |label: &str, sorted: bool, asc: bool| -> String {
            if sorted { if asc { format!("{label} ▲") } else { format!("{label} ▼") } }
            else { label.to_string() }
        };

        // SYM
        let sym_r = ui.add_sized(Vec2::new(sym_w, 13.0), egui::Label::new(
            egui::RichText::new(make_hdr("SYM", *sort_col == 0, *sort_asc))
                .monospace().size(font_2xs()).color(hdr)
        ).sense(egui::Sense::click()));
        if sym_r.clicked() {
            if *sort_col == 0 { *sort_asc = !*sort_asc; }
            else { *sort_col = 0; *sort_asc = true; }
        }

        // SPARK (non-sortable)
        ui.add_sized(Vec2::new(spark_w, 13.0), egui::Label::new(
            egui::RichText::new("SPRK").monospace().size(font_2xs()).color(hdr)
        ));

        // SCORE
        let sc_r = ui.add_sized(Vec2::new(score_w, 13.0), egui::Label::new(
            egui::RichText::new(make_hdr("SCR", *sort_col == 1, *sort_asc))
                .monospace().size(font_2xs()).color(hdr)
        ).sense(egui::Sense::click()));
        if sc_r.clicked() {
            if *sort_col == 1 { *sort_asc = !*sort_asc; }
            else { *sort_col = 1; *sort_asc = false; }
        }

        // Operand columns
        for (i, col_key) in selected_cols.iter().enumerate() {
            let col_idx = i + 2;
            let short = &col_key[col_key.rfind(':').map(|p| p + 1).unwrap_or(0)..];
            let lbl = make_hdr(short, *sort_col == col_idx, *sort_asc);
            let op_r = ui.add_sized(Vec2::new(op_w, 13.0), egui::Label::new(
                egui::RichText::new(&lbl).monospace().size(font_2xs()).color(hdr)
            ).sense(egui::Sense::click()));
            if op_r.clicked() {
                if *sort_col == col_idx { *sort_asc = !*sort_asc; }
                else { *sort_col = col_idx; *sort_asc = false; }
            }
        }
    });
    separator(ui, t.toolbar_border);

    // ── Sort (local, does not mutate caller's slice) ───────────────────────
    let mut sorted: Vec<&ScreenResultRow> = rows.iter()
        .filter(|r| !muted.contains(&r.symbol))
        .collect();

    sorted.sort_by(|a, b| {
        let cmp = match *sort_col {
            0 => a.symbol.cmp(&b.symbol),
            1 => a.score.partial_cmp(&b.score)
                    .unwrap_or(std::cmp::Ordering::Equal),
            col_idx => {
                let key = selected_cols.get(col_idx - 2).map(|s| s.as_str()).unwrap_or("");
                a.values.get(key).partial_cmp(&b.values.get(key))
                    .unwrap_or(std::cmp::Ordering::Equal)
            }
        };
        if *sort_asc { cmp } else { cmp.reverse() }
    });

    if sorted.is_empty() {
        PanelEmpty::new("No results").hint("Run a screen to see matches").show(ui, t);
        return;
    }

    // ── Scrollable rows ───────────────────────────────────────────────────
    egui::ScrollArea::vertical()
        .id_salt("screener_results_scroll")
        .show(ui, |ui| {
            ui.set_min_width(panel_w - 4.0);

            for (row_idx, row) in sorted.iter().enumerate() {
                let id_salt = format!("scr_row_{row_idx}");

                let score_str = row.score
                    .map(|s| format!("{:.3}", s))
                    .unwrap_or_else(|| "—".into());

                let op_strs: Vec<String> = selected_cols.iter()
                    .map(|k| row.values.get(k)
                        .map(|v| format!("{:.3}", v))
                        .unwrap_or_else(|| "—".into()))
                    .collect();

                let (tint_color, tint_alpha) = severity_tint(row.score, t);

                // Build columns slice (borrowed for one frame — no heap allocs).
                let mut cols: Vec<PanelColumn> = Vec::with_capacity(3 + n_extra);
                cols.push(PanelColumn {
                    text: &row.symbol,
                    align: PanelColAlign::Left,
                    color: t.text,
                    weight: sym_w / panel_w.max(1.0),
                    min_width: Some(sym_w),
                    mono: true,
                });
                // Sparkline placeholder — actual paint happens after show_full.
                cols.push(PanelColumn {
                    text: "",
                    align: PanelColAlign::Left,
                    color: t.dim,
                    weight: spark_w / panel_w.max(1.0),
                    min_width: Some(spark_w),
                    mono: false,
                });
                cols.push(PanelColumn {
                    text: &score_str,
                    align: PanelColAlign::Right,
                    color: tint_color,
                    weight: score_w / panel_w.max(1.0),
                    min_width: Some(score_w),
                    mono: true,
                });
                for s in &op_strs {
                    cols.push(PanelColumn {
                        text: s,
                        align: PanelColAlign::Right,
                        color: t.dim,
                        weight: op_w / panel_w.max(1.0),
                        min_width: Some(op_w),
                        mono: true,
                    });
                }

                let resp = PanelListRow::new(&id_salt)
                    .row_tint(tint_color, tint_alpha)
                    .trailing_buttons(&[
                        TrailingBtn::icon(Icon::CHART_LINE_UP_FILL)
                            .tone(TrailingTone::Accent)
                            .tooltip("Load chart"),
                        TrailingBtn::icon(Icon::EYE)
                            .tone(TrailingTone::Default)
                            .tooltip("Add to watchlist"),
                        TrailingBtn::icon(Icon::BELL)
                            .tone(TrailingTone::Warn)
                            .tooltip("Set alert"),
                        TrailingBtn::icon(Icon::BELL_RINGING)
                            .tone(TrailingTone::Muted)
                            .tooltip("Mute from results"),
                    ])
                    .columns(&cols)
                    .show_full(ui, t);

                // Row body click → load chart.
                if resp.clicked {
                    *pending_sym_to_chart = Some(row.symbol.clone());
                }

                // Trailing button dispatch.
                match resp.trailing_clicked_idx {
                    Some(0) => { *pending_sym_to_chart = Some(row.symbol.clone()); }
                    Some(1) => { *pending_watch = Some(row.symbol.clone()); }
                    Some(2) => { /* alert — T-SHELL or dedicated AlertCommand */ }
                    Some(3) => { *pending_mute = Some(row.symbol.clone()); }
                    _ => {}
                }

                // Overlay sparkline into the reserved cell rect.
                // We derive the sparkline cell rect from the row response rect.
                if let Some(spark_pts) = get_or_fetch_sparkline(&row.symbol) {
                    if let Some(row_rect) = resp.response.as_ref().map(|r| r.rect) {
                        let cell_left = row_rect.left() + gap_2xs() + sym_w;
                        let cell_rect = Rect::from_min_size(
                            Pos2::new(cell_left, row_rect.top() + 3.0),
                            Vec2::new(spark_w - gap_xs(), row_rect.height() - 6.0),
                        );
                        paint_sparkline(ui, cell_rect, &spark_pts, t);
                    }
                }
            }

            // Summary of muted symbols.
            let muted_count = rows.len().saturating_sub(sorted.len());
            if muted_count > 0 {
                ui.add_space(gap_xs());
                ui.add(egui::Label::new(
                    egui::RichText::new(format!("{muted_count} muted"))
                        .monospace().size(font_2xs())
                        .color(color_muted(t.dim))
                ));
            }
        });
}

/// Convenience wrapper: draw inside a `PanelSection`.
pub fn draw_section(
    ui: &mut egui::Ui,
    rows: &[ScreenResultRow],
    selected_cols: &[String],
    sort_col: &mut usize,
    sort_asc: &mut bool,
    muted: &HashSet<String>,
    t: &Theme,
    pending_sym_to_chart: &mut Option<String>,
    pending_watch: &mut Option<String>,
    pending_mute: &mut Option<String>,
) {
    let visible = rows.iter().filter(|r| !muted.contains(&r.symbol)).count();
    let title = format!("RESULTS ({visible})");
    PanelSection::new(&title).show(ui, t, |ui, t| {
        draw_results(
            ui, rows, selected_cols, sort_col, sort_asc, muted, t,
            pending_sym_to_chart, pending_watch, pending_mute,
        );
    });
}
