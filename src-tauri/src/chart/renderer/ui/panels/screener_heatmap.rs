//! Screener — heatmap view (T-HEAT, Wave S2).
//!
//! Renders a Finviz-style weighted-row treemap over the screener's live result
//! set, driven by `ScreenPanelState` (owned by T-SHELL). Owns:
//!
//! 1. **Heatmap view** — groups scan results by `sector` field, paints
//!    variable-width cells sized by a user-selected size operand (e.g. market
//!    cap or volume) and colored by a user-selected color operand (e.g. score,
//!    change %, RSI). Click a cell → emit `AppCommand::ScreenerSetSectorFilter`
//!    so the GRID sub-mode shows only that sector. Click a sector header →
//!    same filter. A filter chip at the top clears it.
//!
//! 2. **Screen-entry alerts** — additive hook into the existing
//!    `alerts_feed::push_screen_entry` (added in `alerts_feed.rs` by this
//!    wave; see inline docs there). Consumes the `screen_entry_feed::snapshot()`
//!    queue and routes events through the same badge + toast path that playbook
//!    alerts use. Optional sound via `try_sound_entry`.
//!
//! ## Composition strategy — local HeatmapGrid wrapper
//!
//! `HeatmapGrid` / `HeatmapCell` in `ui_kit/widgets/heatmap_grid.rs` expose a
//! fixed equal-width grid. SCR-6 calls for *weighted*-width cells (cell width
//! ∝ `size_weight / row_total_weight`). Rather than touching the shared
//! `HeatmapGrid` (which would require a coordinated merge with any agent that
//! also reads it), this module provides a **local** `WeightedHeatmapCell` and a
//! local `paint_weighted_heatmap` function that replicates the exact pixel
//! geometry (`bar_fill`, `left-edge accent strip`, `symbol + value text`,
//! `hover highlight`, `active border`) but with the proportional-width
//! row-layout engine.
//!
//! **ui_kit/widgets/heatmap_grid.rs is NOT edited by this agent.**
//! Report: `size_weight` and `color_value` fields are handled purely locally.
//!
//! ## Rendering contract
//! `render_screener_heatmap` is the entry point. Caller (T-SHELL's panel host)
//! passes:
//!   - `results`   — current scan result rows (T-SHELL owns `ScreenPanelState`
//!                   which holds `results_buf: Vec<ScreenerRow>`).
//!   - `state`     — `&mut ScreenHeatmapState` (runtime-only UI scratch for
//!                   this sub-mode; NOT on Watchlist god-object).
//!   - `active_sym`— the pane's current symbol (for cell active-border).
//!   - `t`         — theme.
//!   - `cx`        — UiCtx for dispatching AppCommands.
//!
//! ## State requested from T-SHELL
//! T-SHELL owns `ScreenPanelState`. This module reads:
//!   - `state.results_buf: Vec<ScreenerRow>` — current result rows.
//!   - `state.sector_filter: Option<String>` — active sector drill-down.
//! This module writes (via AppCommand):
//!   - `AppCommand::ScreenerSetSectorFilter { sector: Option<String> }` — set/clear filter.
//!   - `AppCommand::ScreenRowToChart { symbol: String }` — cell click loads symbol.
//!
//! ## AppCommands needed from T-SHELL
//! - `AppCommand::ScreenerSetSectorFilter { sector: Option<String> }` (NEW)
//! - `AppCommand::ScreenRowToChart { symbol: String }` (NEW, alias to SwapPaneSymbol path)
//!
//! Report these to T-SHELL for addition to `commands.rs`.

use std::collections::HashMap;
use egui::{Color32, Pos2, Rect, Sense, Ui, vec2};

use crate::chart_renderer::ui::style::*;
use crate::chart_renderer::ui::tools::notification::AlertKind;
use crate::chart_renderer::commands::{AppCommand, UiCtx};
use crate::ui_kit::sx::{palette_ct, Tone};
use crate::ui_kit::tokens as st;
use crate::ui_kit::widgets::theme::ComponentTheme;

// ── Public types (used by T-SHELL to declare ScreenHeatmapState) ─────────────

/// User-selectable column operand for heatmap dimensions.
/// Matches the `ColSpec` key strings used in `ScreenerRow::values`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct HeatOperandKey(pub String);

impl Default for HeatOperandKey {
    fn default() -> Self { Self("change_pct".to_string()) }
}

/// Runtime UI scratch for the heatmap sub-mode.
///
/// T-SHELL should hold this as `screen_heatmap: ScreenHeatmapState` inside
/// `ScreenPanelState`. Serialize so it persists in the workspace JSON.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ScreenHeatmapState {
    /// Operand used for cell *color* tint. Default = "change_pct".
    #[serde(default)]
    pub color_key: HeatOperandKey,
    /// Operand used for cell *width weight* (size). Default = "volume".
    #[serde(default = "ScreenHeatmapState::default_size_key")]
    pub size_key: HeatOperandKey,
    /// Number of layout rows in the grid (used for row-height math). 0 = auto.
    #[serde(default)]
    pub num_rows_hint: u8,
    /// Active sector drill-down filter. `None` = show all sectors.
    /// Set/cleared by `AppCommand::ScreenerSetSectorFilter`.
    #[serde(skip)]
    pub sector_filter: Option<String>,
    /// Operand picker dropdown is open.
    #[serde(skip)]
    pub color_picker_open: bool,
    #[serde(skip)]
    pub size_picker_open: bool,
}

impl ScreenHeatmapState {
    fn default_size_key() -> HeatOperandKey { HeatOperandKey("volume".to_string()) }
}

impl Default for ScreenHeatmapState {
    fn default() -> Self {
        Self {
            color_key: HeatOperandKey::default(),
            size_key: Self::default_size_key(),
            num_rows_hint: 0,
            sector_filter: None,
            color_picker_open: false,
            size_picker_open: false,
        }
    }
}

// ── Screener result row shape ─────────────────────────────────────────────────
//
// Canonical type: `ScreenResultRow` from `screener_results.rs`.
// `ScreenerRow` is a backward-compat alias used throughout this module.

/// Re-exported canonical row type; `ScreenerRow` alias kept for backward compat within this module.
pub use super::screener_results::ScreenResultRow;

/// Backward-compat alias: heatmap code refers to rows as `ScreenerRow`.
/// Both names refer to the same canonical struct in `screener_results.rs`.
pub type ScreenerRow = ScreenResultRow;

// ── Local weighted-cell type ─────────────────────────────────────────────────

/// A cell ready for weighted-row layout painting.
/// Constructed from `ScreenerRow` values; no heap strings except `symbol`.
struct WeightedHeatmapCell<'a> {
    symbol: &'a str,
    /// Fractional width weight within the row (normalized to 1.0 per row).
    size_weight: f32,
    /// Drives the color tint intensity and direction (+/-).
    color_value: f32,
    /// Whether this cell is the active (charted) symbol.
    is_active: bool,
}

// ── Main render entry point ───────────────────────────────────────────────────

/// Render the heatmap sub-mode body inside the RESULTS tab.
///
/// `results`      — current screener result rows from `ScreenPanelState::results_buf`.
/// `sector_filter`— the current sector drill-down (from `ScreenPanelState::sector_filter`).
/// `heat_state`   — per-sub-mode UI scratch (on `ScreenPanelState::screen_heatmap`).
/// `active_sym`   — pane symbol for active-cell border.
/// `available_operands` — list of column names available for picker dropdowns;
///                         T-SHELL can pass `&state.column_keys` or a fixed slice.
/// `cx`           — UiCtx for AppCommand dispatch.
///
/// **T-SHELL must call this in the RESULTS tab body when the view mode is `Heat`.**
pub fn render_screener_heatmap(
    ui: &mut Ui,
    results: &[ScreenerRow],
    sector_filter: Option<&str>,
    heat_state: &mut ScreenHeatmapState,
    active_sym: &str,
    available_operands: &[String],
    cx: &UiCtx<'_>,
) {
    let t = cx.theme;

    // ── Top bar: operand pickers + filter chip ────────────────────────────────
    ui.horizontal_wrapped(|ui| {
        // Color operand picker
        ui.label(egui::RichText::new("COLOR").font(crate::ui_kit::style::mono_xs()).color(t.dim));
        ui.add_space(gap_xs());
        let color_label = format!("{}  ▾", &heat_state.color_key.0);
        if ui.add(
            egui::Button::new(egui::RichText::new(&color_label).font(crate::ui_kit::style::mono_xs()).color(t.accent))
                .fill(st::color_alpha(t.accent, st::alpha_faint()))
                .stroke(egui::Stroke::NONE)
                .corner_radius(egui::CornerRadius::same(st::radius_sm() as u8))
                .min_size(vec2(0.0, row_height_dense()))
        ).clicked() {
            heat_state.color_picker_open = !heat_state.color_picker_open;
        }

        ui.add_space(gap_md());

        // Size operand picker
        ui.label(egui::RichText::new("SIZE").font(crate::ui_kit::style::mono_xs()).color(t.dim));
        ui.add_space(gap_xs());
        let size_label = format!("{}  ▾", &heat_state.size_key.0);
        if ui.add(
            egui::Button::new(egui::RichText::new(&size_label).font(crate::ui_kit::style::mono_xs()).color(t.dim))
                .fill(st::color_alpha(palette_ct(t).base(Tone::Text), 8))
                .stroke(egui::Stroke::NONE)
                .corner_radius(egui::CornerRadius::same(st::radius_sm() as u8))
                .min_size(vec2(0.0, row_height_dense()))
        ).clicked() {
            heat_state.size_picker_open = !heat_state.size_picker_open;
        }

        // Active sector filter chip
        if let Some(sf) = sector_filter {
            ui.add_space(gap_md());
            let chip_label = format!("{}  {}", sf, crate::ui_kit::icons::Icon::X);
            if ui.add(
                egui::Button::new(egui::RichText::new(&chip_label).font(crate::ui_kit::style::mono_xs()).color(t.accent))
                    .fill(st::color_alpha(t.accent, st::alpha_soft()))
                    .stroke(egui::Stroke::NONE)
                    .corner_radius(egui::CornerRadius::same(st::radius_sm() as u8))
                    .min_size(vec2(0.0, row_height_dense()))
            ).clicked() {
                cx.dispatch(AppCommand::ScreenerSetSectorFilter { sector: None });
            }
        }
    });

    // Operand picker dropdowns (popup-adjacent inline list; replaces PopupFrame for simplicity)
    if heat_state.color_picker_open {
        ui.add_space(gap_xs());
        let new_key = render_operand_picker(ui, t, &heat_state.color_key.0, available_operands, "heat_color_picker");
        if let Some(k) = new_key {
            heat_state.color_key = HeatOperandKey(k);
            heat_state.color_picker_open = false;
        }
    }
    if heat_state.size_picker_open {
        ui.add_space(gap_xs());
        let new_key = render_operand_picker(ui, t, &heat_state.size_key.0, available_operands, "heat_size_picker");
        if let Some(k) = new_key {
            heat_state.size_key = HeatOperandKey(k);
            heat_state.size_picker_open = false;
        }
    }

    ui.add_space(gap_sm());

    // ── Build sector groups ───────────────────────────────────────────────────
    if results.is_empty() {
        use crate::ui_kit::widgets::PanelEmpty;
        PanelEmpty::new("No screen results — run a screen first.").show(ui, t);
        return;
    }

    // Group by sector. Preserve insertion order using a Vec of (sector, rows).
    let mut group_order: Vec<String> = Vec::new();
    let mut groups: HashMap<String, Vec<&ScreenerRow>> = HashMap::new();
    for row in results {
        let key = if row.sector.is_empty() { "Other".to_string() } else { row.sector.clone() };
        if !groups.contains_key(&key) {
            group_order.push(key.clone());
        }
        groups.entry(key).or_default().push(row);
    }

    let color_key = heat_state.color_key.0.clone();
    let size_key  = heat_state.size_key.0.clone();

    // Available panel width is fixed for the full body
    let avail_w = ui.available_width();

    egui::ScrollArea::vertical()
        .id_salt("screener_heatmap_scroll")
        .show(ui, |ui| {
            for sector in &group_order {
                let rows = match groups.get(sector) { Some(r) => r, None => continue };

                // Sector header — colored by average color_value sign
                let avg_color: f32 = if rows.is_empty() { 0.0 } else {
                    rows.iter().map(|r| r.get_f32(&color_key)).sum::<f32>() / rows.len() as f32
                };
                let sector_col = if avg_color >= 0.0 { t.bull } else { t.bear };
                let is_filtered = sector_filter.map_or(false, |sf| sf == sector.as_str());

                ui.add_space(gap_xs());
                let hdr_text = format!(
                    "{}  {} ({})  {:+.2}%",
                    if is_filtered { "▸" } else { "▾" },
                    sector,
                    rows.len(),
                    avg_color,
                );
                let hdr_resp = ui.add(
                    egui::Button::new(
                        egui::RichText::new(&hdr_text)
                            .font(crate::ui_kit::style::mono_xs())
                            .color(sector_col)
                    )
                    .fill(st::color_alpha(sector_col, st::alpha_faint()))
                    .stroke(egui::Stroke::NONE)
                    .corner_radius(egui::CornerRadius::same(st::radius_md() as u8))
                    .min_size(vec2(avail_w, row_height_default()))
                    .frame(false)
                );
                if hdr_resp.clicked() {
                    // Toggle sector filter: clicking the active sector clears it
                    if is_filtered {
                        cx.dispatch(AppCommand::ScreenerSetSectorFilter { sector: None });
                    } else {
                        cx.dispatch(AppCommand::ScreenerSetSectorFilter {
                            sector: Some(sector.clone()),
                        });
                    }
                }
                ui.add_space(gap_xs());

                // Build weighted cells for this sector group
                // size_weight: normalize within-group so largest = 1.0
                let raw_sizes: Vec<f32> = rows.iter().map(|r| r.get_f32(&size_key).abs().max(0.001)).collect();
                let max_size = raw_sizes.iter().cloned().fold(0.001_f32, f32::max);
                let cells: Vec<WeightedHeatmapCell<'_>> = rows.iter().zip(raw_sizes.iter()).map(|(r, &sz)| {
                    WeightedHeatmapCell {
                        symbol: &r.symbol,
                        size_weight: sz / max_size,
                        color_value: r.get_f32(&color_key),
                        is_active: r.symbol == active_sym,
                    }
                }).collect();

                // Paint the weighted row grid
                let clicked_sym = paint_weighted_heatmap(ui, t, &cells, avail_w);
                if let Some(sym) = clicked_sym {
                    cx.dispatch(AppCommand::ScreenRowToChart { symbol: sym });
                }
            }
        });
}

// ── Weighted-row layout painter ───────────────────────────────────────────────
//
// Identical cell geometry to HeatmapGrid (bar fill, left-edge accent, text),
// but widths are proportional to size_weight and color comes from color_value.
//
// Layout: pack cells left-to-right until the row is filled, then wrap. Within
// each row the cell widths are proportional to size_weight. A "row" is defined
// as a contiguous sequence of cells whose total weight fits the row budget.
// We use a simple greedy pack: target_row_cells fills until weight threshold.

fn paint_weighted_heatmap(
    ui: &mut Ui,
    t: &dyn ComponentTheme,
    cells: &[WeightedHeatmapCell<'_>],
    avail_w: f32,
) -> Option<String> {
    if cells.is_empty() { return None; }

    let cell_h: f32 = 32.0;
    let gap:    f32 = 2.0;
    let font_sz: f32 = 10.0;

    // Split cells into rows. Target: each row's visual weight ≈ 1.0 normalized.
    // Greedy: fill until adding the next cell would exceed weight budget of 1.5
    // (heuristic — yields rows of 2-6 cells depending on size dispersion).
    let mut rows: Vec<Vec<&WeightedHeatmapCell<'_>>> = Vec::new();
    let mut current_row: Vec<&WeightedHeatmapCell<'_>> = Vec::new();
    let mut current_weight: f32 = 0.0;
    let row_budget: f32 = 1.5;

    for cell in cells {
        let w = cell.size_weight.max(0.05); // enforce min so tiny cells are visible
        if !current_row.is_empty() && current_weight + w > row_budget {
            rows.push(std::mem::take(&mut current_row));
            current_weight = 0.0;
        }
        current_row.push(cell);
        current_weight += w;
    }
    if !current_row.is_empty() {
        rows.push(current_row);
    }

    let n_rows = rows.len();
    let total_h = n_rows as f32 * (cell_h + gap);

    let (outer_rect, resp) = ui.allocate_exact_size(vec2(avail_w, total_h), Sense::click());
    let painter = ui.painter();

    // Hover detection
    let hover_pos: Option<Pos2> = ui.input(|i| i.pointer.hover_pos());
    let mut clicked_symbol: Option<String> = None;

    // Click detection from resp
    let click_pos: Option<Pos2> = if resp.clicked() { resp.interact_pointer_pos() } else { None };

    // Per cell: compute rect, paint
    for (ri, row) in rows.iter().enumerate() {
        let row_y = outer_rect.top() + ri as f32 * (cell_h + gap);
        let row_total_w: f32 = row.iter().map(|c| c.size_weight.max(0.05)).sum();

        let mut cx_offset = outer_rect.left();
        for cell in row.iter() {
            let cell_w_frac = cell.size_weight.max(0.05) / row_total_w;
            let cell_w = (avail_w * cell_w_frac - gap).max(12.0);
            let cell_rect = Rect::from_min_size(egui::pos2(cx_offset, row_y), vec2(cell_w, cell_h));
            cx_offset += cell_w + gap;

            let is_hovered = hover_pos.map_or(false, |p| cell_rect.contains(p));
            if is_hovered { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
            if let Some(cp) = click_pos {
                if cell_rect.contains(cp) {
                    clicked_symbol = Some(cell.symbol.to_string());
                }
            }

            paint_cell(
                painter, t, cell_rect,
                cell.symbol, cell.color_value, cell.is_active, is_hovered,
                font_sz,
            );
        }
    }

    clicked_symbol
}

/// Paint one cell at `rect` with the same visual geometry as `HeatmapGrid` cells.
#[allow(clippy::too_many_arguments)]
fn paint_cell(
    painter: &egui::Painter,
    t: &dyn ComponentTheme,
    rect: Rect,
    symbol: &str,
    color_value: f32,
    is_active: bool,
    is_hovered: bool,
    font_sz: f32,
) {
    let is_up = color_value >= 0.0;
    // Intensity: saturate at ±5% equivalent (arbitrary — treat color_value as %-like)
    let intensity = (color_value.abs() / 5.0).min(1.0);

    // Hover highlight
    if is_hovered {
        painter.rect_filled(
            rect, st::radius_xs(),
            st::color_alpha(palette_ct(t).base(Tone::Text), 12),
        );
    }

    // Active symbol border
    if is_active {
        painter.rect_stroke(
            rect.shrink(1.0), st::radius_xs(),
            egui::Stroke::new(st::stroke_bold(), palette_ct(t).base(Tone::Accent)),
            egui::StrokeKind::Outside,
        );
    }

    // Background bar proportional to intensity
    let bar_w = intensity * rect.width() * 0.65;
    let bar_col = if is_up {
        st::color_alpha(palette_ct(t).base(Tone::Bull), (25.0 + intensity * 55.0) as u8)
    } else {
        st::color_alpha(palette_ct(t).base(Tone::Bear), (25.0 + intensity * 55.0) as u8)
    };
    painter.rect_filled(
        Rect::from_min_size(rect.min + vec2(0.0, 1.0), vec2(bar_w, rect.height() - 2.0)),
        st::radius_xs(), bar_col,
    );

    // Left-edge accent strip
    let edge_a = (120.0 + intensity * 135.0) as u8;
    let edge_col = if is_up {
        st::color_alpha(palette_ct(t).base(Tone::Bull), edge_a)
    } else {
        st::color_alpha(palette_ct(t).base(Tone::Bear), edge_a)
    };
    painter.rect_filled(
        Rect::from_min_size(rect.min + vec2(0.0, 1.0), vec2(3.0, rect.height() - 2.0)),
        0.0, edge_col,
    );

    // Symbol text
    let sym_col: Color32 = if is_active {
        palette_ct(t).base(Tone::Text)
    } else if is_hovered {
        st::color_alpha(palette_ct(t).base(Tone::Text), 230)
    } else {
        st::color_alpha(palette_ct(t).base(Tone::Text), 190)
    };
    painter.text(
        rect.min + vec2(7.0, rect.height() / 2.0),
        egui::Align2::LEFT_CENTER,
        symbol,
        egui::FontId::monospace(font_sz),
        sym_col,
    );

    // Color-value text (right-aligned)
    let val_col = if is_up { palette_ct(t).base(Tone::Bull) } else { palette_ct(t).base(Tone::Bear) };
    painter.text(
        rect.right_center() - vec2(3.0, 0.0),
        egui::Align2::RIGHT_CENTER,
        &format!("{:+.2}", color_value),
        egui::FontId::monospace(font_sz),
        val_col,
    );
}

// ── Operand picker inline list ────────────────────────────────────────────────
//
// A compact scrollable list of operand names. Returns the selected key.
// Replaces a full PopupFrame to avoid pulling modal stack state.

fn render_operand_picker(
    ui: &mut Ui,
    t: &dyn ComponentTheme,
    current: &str,
    operands: &[String],
    id_salt: &str,
) -> Option<String> {
    let mut selected: Option<String> = None;
    egui::Frame::new()
        .fill(st::color_alpha(palette_ct(t).base(Tone::Text), 6))
        .corner_radius(egui::CornerRadius::same(st::radius_sm() as u8))
        .inner_margin(egui::Margin::symmetric(4, 4))
        .show(ui, |ui| {
            egui::ScrollArea::vertical()
                .id_salt(id_salt)
                .max_height(120.0)
                .show(ui, |ui| {
                    for op in operands {
                        let is_cur = op == current;
                        let label_col = if is_cur { palette_ct(t).base(Tone::Accent) } else { palette_ct(t).base(Tone::Text) };
                        let r = ui.add(
                            egui::Button::new(
                                egui::RichText::new(op.as_str())
                                    .font(crate::ui_kit::style::mono_xs())
                                    .color(label_col)
                            )
                            .fill(if is_cur { st::color_alpha(palette_ct(t).base(Tone::Accent), st::alpha_faint()) } else { Color32::TRANSPARENT })
                            .stroke(egui::Stroke::NONE)
                            .min_size(vec2(ui.available_width(), row_height_dense()))
                        );
                        if r.clicked() {
                            selected = Some(op.clone());
                        }
                        if r.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
                    }
                });
        });
    selected
}

// ── Screen-entry alert integration ────────────────────────────────────────────
//
// `alerts_feed.rs` gets an additive `push_screen_entry` function (not refactored).
// This module calls it from the render loop when new enter-events arrive.
// The entry point below is called by T-SHELL's panel host alongside other feed
// ticks (e.g. once per frame or on SSE frame delivery from the race feed).

/// Route a screen-entry event through the alerts path.
///
/// Called by T-SHELL whenever a new symbol enters the active screen
/// (detected from `ScreenPanelState::race_enter_events` queue or the
/// `scans:events` SSE stream with type "enter").
///
/// - Pushes to `alerts_feed::PLAYBOOK_ALERTS` queue via `push_screen_entry`
///   so `alerts_panel.rs` surfaces it under SCREEN ENTRIES.
/// - Pushes a badge via `alert_feed::push`.
/// - Optionally plays a sound for `severity == "critical"` screens.
///
/// **This is additive** — does not change `alerts_feed.rs` logic paths,
/// only calls the new public function added there.
pub fn route_screen_entry_alert(
    symbol: &str,
    screen_name: &str,
    score: f64,
    severity: &str,
) {
    use crate::data::feeds::alerts_feed;
    use crate::chart_renderer::ui::components::toolbar::alert_feed as badge_feed;

    // Push to the playbook alerts ring (shows in SCREEN ENTRIES section)
    alerts_feed::push_screen_entry(symbol, screen_name, score, severity);

    // Badge strip: kind=Signal for normal, Warning for critical/high-priority
    let kind = match severity.to_lowercase().as_str() {
        "critical" | "high" => AlertKind::Warning,
        _                   => AlertKind::Signal,
    };
    let sym = if symbol.is_empty() { None } else { Some(symbol.to_string()) };
    let msg = format!("SCREEN ENTRY {}: {} (score {:.1})", screen_name, symbol, score);
    badge_feed::push(kind, sym, msg);
    crate::wake_native_ui();

    // Optional sound for critical-severity screens
    if severity.eq_ignore_ascii_case("critical") {
        try_sound_entry();
    }
}

/// Fire a short system beep for critical screen-entry events.
/// Degrades silently if platform audio is unavailable.
fn try_sound_entry() {
    // Platform-native beep: on Windows this uses the MessageBeep(0xFFFFFFFF) path;
    // on other platforms it is a no-op stub.
    #[cfg(target_os = "windows")]
    {
        // SAFETY: Beep is a simple fire-and-forget call.
        // MessageBeep is not available in current windows-sys feature set; use Beep instead.
        // MB_OK = 0x0; pass 0xFFFF_FFFF for simple beep.
        // Degrade silently — if Beep is unavailable, skip.
        let _ = (); // no-op beep stub; wire real audio when audio subsystem lands
    }
    // macOS / Linux: pipe a terminal bell character — no external crate needed.
    #[cfg(not(target_os = "windows"))]
    {
        // Silent fallback — platform beep would require external crate (rodio/cpal).
        // Leave as a no-op stub; wire real audio when audio subsystem is available.
        let _ = std::io::Write::write_all(&mut std::io::stderr(), b"\x07");
    }
}

// ── AppCommand variants needed from T-SHELL ───────────────────────────────────
//
// T-SHELL must add the following variants to `AppCommand` in commands.rs.
// This module references them; if T-SHELL hasn't added them yet, the build
// will fail with "variant not found" — that's the integration signal.
//
// ```rust
// // In commands.rs AppCommand enum (T-SHELL owns this file):
//
// /// Set or clear the active sector filter on the screener heatmap.
// /// None = clear filter (show all sectors).
// ScreenerSetSectorFilter { sector: Option<String> },
//
// /// Load a symbol from a screener result row into the active chart pane.
// /// Maps to the existing SwapPaneSymbol reducer path with `pane = ap`.
// ScreenRowToChart { symbol: String },
// ```
//
// In the AppCommand reducer (dispatch fn in commands.rs), add:
//
// ```rust
// AppCommand::ScreenerSetSectorFilter { sector } => {
//     // Update ScreenPanelState.sector_filter — T-SHELL wires this.
//     // store.screen_panel.sector_filter = sector;
// }
// AppCommand::ScreenRowToChart { symbol } => {
//     // Re-use the SwapPaneSymbol path:
//     if let Some(pane) = panes.get_mut(ap) {
//         pane.pending_symbol_change = Some(symbol);
//     }
// }
// ```
