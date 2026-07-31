//! Scripting / Backtesting panel — AI-driven strategy editor with backtest results.
//!
//! This is the UI scaffold. The actual script execution engine and AI integration
//! come later; for now, "Run" and "Backtest" produce mock output.
//!
//! Migrated to the canonical ui_kit side-panel primitives:
//!   - `SidePanelShell` for outer chrome (replaces the bespoke Modal +
//!     PopupFrame chrome the standalone draw used to roll by hand).
//!   - `PanelSection` for section headers (AI PROMPT / EXAMPLES / EDITOR /
//!     ACTIONS / RESULT).
//!   - Result tabs already on `Button::toggle` from Agent H.
//!   - The `draw` and `draw_content` paths now share one body via
//!     `draw_body`, so the modal chrome no longer duplicates the tab body.
//!
//! NOTE: Previously the standalone `draw` was a floating Modal at
//! (280, 80) sized 480x620 with `PopupFrame`. Per Agent Q's brief, all
//! mid-tier panel chrome is being unified on `SidePanelShell`. If product
//! needs the floating dialog back, switch the chrome only — the body is
//! shell-agnostic.

use egui;
use crate::ui_kit::sx::Tone;
use super::super::style::*;
use crate::ui_kit::widgets::Input;
use crate::ui_kit::widgets::tokens::Size as KitSize;
use crate::ui_kit::widgets::Button;
use crate::ui_kit::widgets::TextArea;
use crate::ui_kit::widgets::tokens::{Variant, Size};
use crate::ui_kit::widgets::{PanelSection};
use crate::chart_renderer::ui::panels::side_panel_shell::{SidePanelShell, Width};
use super::super::components::text::MonospaceCode;
use super::super::widgets::cards::Card;
use super::super::super::gpu::{Watchlist, Theme};

// ── Preset example scripts (W3-02 slice 2) ──────────────────────────────────
// Real, runnable Rhai for the custom-indicator engine. Each returns an array
// aligned to `close`; warmup slots use NAN. These are the templates the spec
// asked for, expressed against the single-series contract the engine supports
// today (multi-lane outputs — banded VWAP, CVD coloring — are a follow-up).

const PRESETS: &[(&str, &str)] = &[
    ("SMA", r#"// Simple moving average of close over `period`.
let out = [];
for i in 0..close.len() {
    if i < period - 1 { out.push(NAN); }
    else { let s = 0.0; for j in 0..period { s += close[i-j]; } out.push(s / period); }
}
out"#),
    ("Momentum", r#"// Price change over `period` bars.
let out = [];
for i in 0..close.len() {
    if i < period { out.push(NAN); }
    else { out.push(close[i] - close[i - period]); }
}
out"#),
    ("Range %", r#"// Bar range as a percent of close — uses high/low/close.
let out = [];
for i in 0..close.len() {
    if close[i] == 0.0 { out.push(NAN); }
    else { out.push((high[i] - low[i]) / close[i] * 100.0); }
}
out"#),
];

/// W3-02 slice 2: validate a script by running it over a synthetic fixture — the
/// same sandboxed engine that renders it on the chart. Returns a human summary
/// on success, or the compile/runtime error for inline display. The chart's
/// real bars drive the actual indicator once added; this just proves the script
/// compiles and runs without hanging (op-capped) before the trader commits it.
fn validate_script(src: &str) -> Result<String, String> {
    if src.trim().is_empty() {
        return Err("No script to run — the editor is empty.".to_string());
    }
    // Deterministic fixture: a 60-bar ramp+wiggle. Enough to exercise typical
    // period windows without depending on live data.
    let n = 60usize;
    let close: Vec<f32> = (0..n).map(|i| 100.0 + i as f32 * 0.1 + (i as f32 * 0.4).sin() * 2.0).collect();
    let high: Vec<f32> = close.iter().map(|c| c + 0.4).collect();
    let low: Vec<f32> = close.iter().map(|c| c - 0.4).collect();
    let volume: Vec<f32> = (0..n).map(|i| 1000.0 + i as f32).collect();
    let ts: Vec<i64> = (0..n as i64).map(|i| i * 60).collect();
    let data = crate::chart::indicators::Ohlcv {
        open: &close, high: &high, low: &low, close: &close, volume: &volume, timestamps: &ts,
    };
    let params = crate::chart::indicators::Params::with_period(20);
    let vals = crate::chart::indicators::script::eval_script_indicator(src, &data, &params)?;
    let finite = vals.iter().filter(|v| v.is_finite()).count();
    let last = vals.iter().rev().find(|v| v.is_finite()).copied();
    Ok(format!(
        "OK — compiled and ran on a {n}-bar fixture.\n\
         {finite}/{n} finite values{}.\n\n\
         Click \u{201C}Add to Chart\u{201D} to render it on the active pane with live bars.",
        last.map(|v| format!(", last = {v:.4}")).unwrap_or_default()
    ))
}

/// W3-02 slice 2: path of the workbench draft script under the state dir.
fn workbench_path() -> std::path::PathBuf {
    let dir = std::env::current_exe().ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    let scripts = dir.join("state").join("scripts");
    let _ = std::fs::create_dir_all(&scripts);
    scripts.join("workbench.rhai")
}

/// W3-02 slice 2: persist the editor draft to the workbench file. Returns the
/// path on success. Indicators added to a chart persist with the workspace; this
/// keeps an un-added draft across restarts.
fn save_workbench_script(src: &str) -> Result<String, String> {
    let path = workbench_path();
    std::fs::write(&path, src).map_err(|e| e.to_string())?;
    Ok(path.display().to_string())
}

/// Load the workbench draft into the editor once per process, only if the editor
/// is still empty — so a saved draft survives a restart without fighting the
/// user's Clear/edits within a session.
fn maybe_load_workbench(watchlist: &mut Watchlist) {
    use std::sync::atomic::{AtomicBool, Ordering};
    static LOADED: AtomicBool = AtomicBool::new(false);
    if LOADED.swap(true, Ordering::Relaxed) {
        return;
    }
    if watchlist.script.source.is_empty() {
        if let Ok(src) = std::fs::read_to_string(workbench_path()) {
            if !src.trim().is_empty() {
                watchlist.script.source = src;
            }
        }
    }
}

// ── Backtest data structures ────────────────────────────────────────────────

#[derive(Clone)]
pub(crate) struct BacktestTrade {
    pub side: &'static str, // "LONG" / "SHORT"
    pub entry_price: f32,
    pub exit_price: f32,
    pub pnl: f32,
    pub pnl_pct: f32,
}

#[derive(Clone)]
pub(crate) struct BacktestResult {
    pub trades: Vec<BacktestTrade>,
    pub total_pnl: f32,
    pub win_rate: f32,
    pub profit_factor: f32,
    pub max_drawdown: f32,
    pub sharpe: f32,
}

/// Generate deterministic mock backtest results (no rand crate needed).
fn mock_backtest() -> BacktestResult {
    let mut seed: u32 = 42;
    let mut next = || -> f32 {
        seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
        ((seed >> 16) & 0x7FFF) as f32 / 32767.0
    };

    let mut trades = Vec::with_capacity(18);
    let base_price = 450.0_f32;

    for _ in 0..18 {
        let is_long = next() > 0.4;
        let side = if is_long { "LONG" } else { "SHORT" };
        let entry = base_price + (next() - 0.5) * 40.0;
        let move_pct = (next() - 0.42) * 0.06;
        let exit = if is_long {
            entry * (1.0 + move_pct)
        } else {
            entry * (1.0 - move_pct)
        };
        let pnl = if is_long { exit - entry } else { entry - exit };
        let pnl_pct = (pnl / entry) * 100.0;
        trades.push(BacktestTrade { side, entry_price: entry, exit_price: exit, pnl, pnl_pct });
    }

    let total_pnl: f32 = trades.iter().map(|t| t.pnl).sum();
    let wins = trades.iter().filter(|t| t.pnl > 0.0).count();
    let win_rate = wins as f32 / trades.len() as f32 * 100.0;

    let gross_profit: f32 = trades.iter().filter(|t| t.pnl > 0.0).map(|t| t.pnl).sum();
    let gross_loss: f32 = trades.iter().filter(|t| t.pnl < 0.0).map(|t| t.pnl.abs()).sum();
    let profit_factor = if gross_loss > 0.0 { gross_profit / gross_loss } else { 99.9 };

    let mut peak = 0.0_f32;
    let mut max_dd = 0.0_f32;
    let mut cum = 0.0_f32;
    for t in &trades {
        cum += t.pnl;
        if cum > peak { peak = cum; }
        let dd = peak - cum;
        if dd > max_dd { max_dd = dd; }
    }
    let max_drawdown = if base_price > 0.0 { max_dd / base_price * 100.0 } else { 0.0 };

    let mean = total_pnl / trades.len() as f32;
    let variance: f32 = trades.iter().map(|t| (t.pnl - mean).powi(2)).sum::<f32>() / trades.len() as f32;
    let sharpe = if variance > 0.0 { mean / variance.sqrt() * (252.0_f32).sqrt() } else { 0.0 };

    BacktestResult { trades, total_pnl, win_rate, profit_factor, max_drawdown, sharpe }
}

// ── Result tab ──────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum ScriptResultTab {
    Output,
    Backtest,
}

// ── Shared body ─────────────────────────────────────────────────────────────

/// Shared body for both the standalone `draw` (SidePanelShell) and the
/// `draw_content` path used by analysis_panel as a tab. `show_save` is `true`
/// only for the standalone shell (analysis_panel doesn't expose Save).
fn draw_body(ui: &mut egui::Ui, watchlist: &mut Watchlist, active_pane: Option<usize>, t: &Theme, show_save: bool) {
    // W3-02 slice 2: seed the editor from the last saved draft (once per process).
    maybe_load_workbench(watchlist);
    let w = ui.available_width();

    // ── AI Prompt ─────────────────────────────────────────────
    PanelSection::new("AI PROMPT").show(ui, t, |ui, t| {
        ui.horizontal(|ui| {
            ui.add(MonospaceCode::new("\u{2728}").xs().color(t.accent));
            ui.add_space(gap_xs());
            Input::new(&mut watchlist.script.ai_prompt)
                .min_width(w - 36.0)
                .size(KitSize::Sm)
                .placeholder("Describe your indicator or strategy...")
                .show(ui, t);
        });
    });

    // ── Presets ───────────────────────────────────────────────
    PanelSection::new("EXAMPLES").show(ui, t, |ui, t| {
        ui.horizontal_wrapped(|ui| {
            for (name, source) in PRESETS {
                let btn = Button::new(*name).variant(Variant::Chip).size(Size::Xs)
                    .show(ui, t);
                if btn.clicked() { watchlist.script.source = source.to_string(); }
                if btn.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
            }
        });
    });

    // ── Editor ────────────────────────────────────────────────
    PanelSection::new("EDITOR").show(ui, t, |ui, t| {
        TextArea::new(&mut watchlist.script.source)
            .min_rows(8)
            .max_rows(10)
            .monospace(true)
            .full_width()
            .show(ui, t);
    });

    // ── Actions ───────────────────────────────────────────────
    PanelSection::new("ACTIONS").show(ui, t, |ui, t| {
        ui.horizontal(|ui| {
            // W3-02 slice 2: Run now REALLY evaluates the script (sandboxed,
            // op-capped) over a fixture and shows the result or the compile/
            // runtime error inline — the "NO SCRIPT ENGINE" scaffold is gone.
            if action_button(ui, "\u{25B6} Run", t.bull, t).clicked() {
                watchlist.script.output = match validate_script(&watchlist.script.source) {
                    Ok(summary) => summary,
                    Err(e) => format!("Script error:\n{e}"),
                };
                watchlist.script.result_tab = ScriptResultTab::Output;
            }
            ui.add_space(gap_xs());
            // W3-02 slice 2: the usability payoff — add the editor's script as a
            // live indicator on the active pane (renders via the gated compute
            // path; persists with the workspace). Only when we know the pane.
            if let Some(pane) = active_pane {
                if action_button(ui, "\u{2795} Add to Chart", t.accent, t).clicked() {
                    let src = watchlist.script.source.clone();
                    if src.trim().is_empty() {
                        watchlist.script.output = "Nothing to add — the editor is empty.".to_string();
                    } else {
                        crate::chart_renderer::commands::push(
                            crate::chart_renderer::commands::AppCommand::AddScriptIndicator { pane, src });
                        watchlist.script.output = format!("Added as a custom indicator on pane {pane}.");
                    }
                    watchlist.script.result_tab = ScriptResultTab::Output;
                }
                ui.add_space(gap_xs());
            }
            if action_button(ui, "\u{1F4CA} Backtest", t.accent, t).clicked() {
                let result = mock_backtest();
                let mut out = String::new();
                // AUDIT P0-5: these figures are fabricated by a seed=42 LCG that
                // never reads the script. Disclose it in the text output too —
                // the badge only covers the Backtest tab.
                out.push_str("*** SIMULATED — MOCK DATA, NOT A REAL BACKTEST ***\n");
                out.push_str("These figures are generated by a fixed-seed placeholder and\n");
                out.push_str("do NOT reflect the script above. No backtest engine exists yet.\n\n");
                out.push_str(&format!("Backtesting: {}\n", watchlist.script.source));
                out.push_str(&format!("Period: 252 bars | {} trades\n\n", result.trades.len()));
                out.push_str(&format!("Total P&L:      ${:.2}\n", result.total_pnl));
                out.push_str(&format!("Win Rate:       {:.1}%\n", result.win_rate));
                out.push_str(&format!("Profit Factor:  {:.2}\n", result.profit_factor));
                out.push_str(&format!("Max Drawdown:   {:.2}%\n", result.max_drawdown));
                out.push_str(&format!("Sharpe Ratio:   {:.2}\n", result.sharpe));
                watchlist.script.output = out;
                watchlist.script.backtest = Some(result);
                watchlist.script.result_tab = ScriptResultTab::Backtest;
            }
            if show_save {
                ui.add_space(gap_xs());
                if action_button(ui, "Save", t.dim, t).clicked() {
                    // W3-02 slice 2: real persistence — write the editor source to
                    // a workbench file under the state dir. (Indicators added to a
                    // chart also persist with the workspace; this keeps the
                    // editor's draft across restarts even before it's added.)
                    watchlist.script.output = match save_workbench_script(&watchlist.script.source) {
                        Ok(path) => format!("Saved to {path}"),
                        Err(e) => format!("Save failed: {e}"),
                    };
                    watchlist.script.result_tab = ScriptResultTab::Output;
                }
            }
            ui.add_space(gap_xs());
            if action_button(ui, "Clear", color_subtle(t.bear), t).clicked() {
                watchlist.script.source.clear();
                watchlist.script.ai_prompt.clear();
                watchlist.script.output.clear();
                watchlist.script.backtest = None;
            }
        });
    });

    // ── Result tabs + body ────────────────────────────────────
    PanelSection::new("RESULT")
        .show(ui, t, |ui, t| {
            ui.horizontal(|ui| {
                result_tab_btn(ui, "Output", ScriptResultTab::Output, &mut watchlist.script.result_tab, t);
                ui.add_space(gap_xs());
                result_tab_btn(ui, "Backtest", ScriptResultTab::Backtest, &mut watchlist.script.result_tab, t);
            });
            ui.add_space(gap_xs());

            egui::ScrollArea::vertical()
                .id_salt("script_results")
                .show(ui, |ui| {
                    ui.set_min_width(w - 4.0);
                    match watchlist.script.result_tab {
                        ScriptResultTab::Output => draw_output_tab(ui, watchlist, t),
                        ScriptResultTab::Backtest => draw_backtest_tab(ui, watchlist, w, t),
                    }
                });
        });
}

// ── Public entry points ─────────────────────────────────────────────────────

/// Inner body for use inside analysis_panel tab. No Save action here.
pub(crate) fn draw_content(ui: &mut egui::Ui, watchlist: &mut Watchlist, active_pane: usize, t: &Theme) {
    draw_body(ui, watchlist, Some(active_pane), t, false);
}

/// Standalone panel — outer chrome via canonical SidePanelShell.
/// Rail registration — see [`super::right_rail`].
pub(crate) const RAIL: super::right_rail::RailPanelDef = super::right_rail::RailPanelDef {
    id: "script",
    is_open: |w| w.script.open,
    render: |cx, slot| draw(cx.ctx, cx.watchlist, cx.active_pane, cx.t, Some(slot)),
};

pub(crate) fn draw(ctx: &egui::Context, watchlist: &mut Watchlist, active_pane: usize, t: &Theme, slot: Option<super::side_panel_shell::RailSlot>) {
    if !watchlist.script.open { return; }

    let resp = SidePanelShell::new("apex_script", "APEX SCRIPT")
        .width(Width::Wide)
        .pane_metrics(
            crate::chart_renderer::gpu::pane_tabs_header_h(watchlist),
            watchlist.pane_header_size.title_font(),
        )
        .rail_slot(slot)
        .show(ctx, t, |ui, t| {
            draw_body(ui, watchlist, Some(active_pane), t, true);
        });
    if resp.close_clicked { watchlist.update_sidebar_state(|s| s.script_open = false); }
}

// ── Output tab ──────────────────────────────────────────────────────────────

fn draw_output_tab(ui: &mut egui::Ui, watchlist: &Watchlist, t: &Theme) {
    let m = 10.0;
    if watchlist.script.output.is_empty() {
        ui.add_space(gap_xl());
        ui.vertical_centered(|ui| {
            ui.add(MonospaceCode::new("Run a script or backtest to see results here.").xs().color(t.dim).gamma(0.4));
        });
    } else {
        ui.add_space(gap_xs());
        let is_error = watchlist.script.output.starts_with("Error");
        let (card_bg, card_border) = if is_error {
            (tint(t, Tone::Bear, 18), tint(t, Tone::Bear, alpha_line()))
        } else {
            (tint(t, Tone::Border, alpha_tint()), tint(t, Tone::Border, alpha_muted()))
        };
        let text_color = if is_error { t.bear } else { color_subtle(t.dim) };
        ui.horizontal(|ui| {
            ui.add_space(m);
            Card::new().colors(card_bg, card_border).show(ui, |ui| {
                ui.add(MonospaceCode::new(&watchlist.script.output).xs().color(text_color));
            });
        });
    }
    ui.add_space(gap_sm());
}

// ── Backtest tab ────────────────────────────────────────────────────────────

fn draw_backtest_tab(ui: &mut egui::Ui, watchlist: &Watchlist, w: f32, t: &Theme) {
    let m = 8.0;
    let result = match &watchlist.script.backtest {
        Some(r) => r,
        None => {
            ui.add_space(gap_xl());
            ui.vertical_centered(|ui| {
                ui.add(MonospaceCode::new("Click \"Backtest\" to generate results.").xs().color(t.dim).gamma(0.4));
            });
            return;
        }
    };

    ui.add_space(gap_sm());

    // ── SIMULATED badge ─────────────────────────────────────
    // AUDIT P0-5: `mock_backtest()` is a seed=42 LCG that never reads the script
    // source — every figure below (P&L, Win Rate, PF, Max DD, Sharpe) is
    // FABRICATED, not measured. Until a real backtest engine exists these must
    // never read as real performance statistics in a live-money app. Same
    // treatment as the SYNTHETIC gamma badge (F3) and the DOM's SIMULATED badge.
    ui.horizontal(|ui| {
        ui.add_space(m);
        let badge_font = egui::FontId::monospace(font_sm());
        let painter = ui.painter();
        let galley = painter.layout_no_wrap(
            "SIMULATED \u{2014} mock data, not a real backtest".to_string(),
            badge_font.clone(), t.warn);
        let pad_x = gap_xs();
        let pad_y = 2.0_f32;
        let (rect, _) = ui.allocate_exact_size(
            egui::vec2(galley.size().x + pad_x * 2.0, galley.size().y + pad_y * 2.0),
            egui::Sense::hover());
        ui.painter().rect_filled(rect, radius_xs(), tint(t, Tone::Warn, 28));
        ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER,
            "SIMULATED \u{2014} mock data, not a real backtest", badge_font, t.warn);
    });

    ui.add_space(gap_sm());

    // ── Stats row ───────────────────────────────────────────
    let stats = [
        ("P&L", format!("${:.2}", result.total_pnl), if result.total_pnl >= 0.0 { t.bull } else { t.bear }),
        ("Win Rate", format!("{:.1}%", result.win_rate), if result.win_rate >= 50.0 { t.bull } else { t.bear }),
        ("PF", format!("{:.2}", result.profit_factor), if result.profit_factor >= 1.0 { t.bull } else { t.bear }),
        ("Max DD", format!("{:.2}%", result.max_drawdown), t.bear),
        ("Sharpe", format!("{:.2}", result.sharpe), if result.sharpe >= 1.0 { t.bull } else { t.dim }),
    ];

    ui.horizontal(|ui| {
        ui.add_space(m);
        let card_w = (w - m * 2.0 - 8.0) / stats.len() as f32;
        for (label, value, color) in &stats {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(card_w, 38.0), egui::Sense::hover());
            ui.painter().rect_filled(rect, radius_sm(), tint(t, Tone::Border, alpha_tint()));
            ui.painter().rect_stroke(rect, radius_sm(), egui::Stroke::new(stroke_thin(), tint(t, Tone::Border, alpha_line())), egui::StrokeKind::Outside);

            ui.painter().text(
                egui::pos2(rect.center().x, rect.min.y + 10.0),
                egui::Align2::CENTER_CENTER,
                label,
                mono_sm(),
                color_half(t.dim),
            );
            ui.painter().text(
                egui::pos2(rect.center().x, rect.min.y + 26.0),
                egui::Align2::CENTER_CENTER,
                value,
                mono_sm(),
                *color,
            );
        }
    });

    ui.add_space(gap_sm());

    // ── Trade list header ───────────────────────────────────
    ui.horizontal(|ui| {
        ui.add_space(m);
        ui.add(MonospaceCode::new(&format!("TRADES ({})", result.trades.len())).xs().color(t.dim).gamma(0.5).strong(true));
    });
    ui.add_space(gap_xs());

    let col_x = [m, m + 42.0, m + 112.0, m + 192.0, m + 262.0];
    let header_y = ui.cursor().min.y;
    let header_rect = egui::Rect::from_min_size(
        egui::pos2(ui.cursor().min.x, header_y),
        egui::vec2(w, 14.0),
    );
    ui.allocate_rect(header_rect, egui::Sense::hover());
    let headers = ["Side", "Entry", "Exit", "P&L", "P&L %"];
    for (i, hdr) in headers.iter().enumerate() {
        ui.painter().text(
            egui::pos2(ui.cursor().min.x + col_x[i], header_y + 7.0),
            egui::Align2::LEFT_CENTER,
            hdr,
            mono_sm(),
            color_dim(t.dim),
        );
    }

    let div_y = header_y + 14.0;
    let div_rect = egui::Rect::from_min_size(
        egui::pos2(ui.cursor().min.x + m, div_y),
        egui::vec2(w - m * 2.0, 1.0),
    );
    ui.painter().rect_filled(div_rect, 0.0, tint(t, Tone::Border, alpha_muted()));
    ui.add_space(gap_xs());

    for trade in &result.trades {
        let row_y = ui.cursor().min.y;
        let row_rect = egui::Rect::from_min_size(
            egui::pos2(ui.cursor().min.x, row_y),
            egui::vec2(w, 16.0),
        );
        let resp = ui.allocate_rect(row_rect, egui::Sense::hover());
        if resp.hovered() {
            ui.painter().rect_filled(row_rect, radius_xs(), tint(t, Tone::Border, alpha_subtle()));
        }

        let base_x = ui.cursor().min.x;
        let cy = row_y + 8.0;
        let pnl_color = if trade.pnl >= 0.0 { t.bull } else { t.bear };

        let side_col = if trade.side == "LONG" { t.bull } else { t.bear };
        let side_rect = egui::Rect::from_min_size(
            egui::pos2(base_x + col_x[0], row_y + 1.0),
            egui::vec2(32.0, 14.0),
        );
        ui.painter().rect_filled(side_rect, radius_xs(), color_alpha(side_col, alpha_soft()));
        ui.painter().text(
            side_rect.center(),
            egui::Align2::CENTER_CENTER,
            trade.side,
            mono_sm(),
            side_col,
        );

        ui.painter().text(
            egui::pos2(base_x + col_x[1], cy),
            egui::Align2::LEFT_CENTER,
            format!("{:.2}", trade.entry_price),
            mono_sm(),
            color_subtle(t.dim),
        );

        ui.painter().text(
            egui::pos2(base_x + col_x[2], cy),
            egui::Align2::LEFT_CENTER,
            format!("{:.2}", trade.exit_price),
            mono_sm(),
            color_subtle(t.dim),
        );

        let pnl_sign = if trade.pnl >= 0.0 { "+" } else { "" };
        ui.painter().text(
            egui::pos2(base_x + col_x[3], cy),
            egui::Align2::LEFT_CENTER,
            format!("{}${:.2}", pnl_sign, trade.pnl),
            mono_sm(),
            pnl_color,
        );

        ui.painter().text(
            egui::pos2(base_x + col_x[4], cy),
            egui::Align2::LEFT_CENTER,
            format!("{}{:.2}%", pnl_sign, trade.pnl_pct),
            mono_sm(),
            pnl_color,
        );
    }

    ui.add_space(gap_md());
}

// ── Helper widgets ──────────────────────────────────────────────────────────

/// Accent-colored action button for the toolbar row.
fn action_button(ui: &mut egui::Ui, label: &str, color: egui::Color32, t: &Theme) -> egui::Response {
    let resp = Button::new(label).variant(Variant::Secondary).simple_treatment(true).fg(color).show(ui, t);
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        ui.painter().rect_filled(resp.rect, r_sm_cr(), color_alpha(color, 8));
    }
    resp
}

/// Tab button for Output / Backtest result tabs (already on Button::toggle from Agent H).
fn result_tab_btn(ui: &mut egui::Ui, label: &str, tab: ScriptResultTab, active: &mut ScriptResultTab, t: &Theme) {
    let is_active = *active == tab;
    let resp = Button::toggle(label, is_active)
        .corner_radius(crate::chart_renderer::ui::style::current().r_md as f32)
        .show(ui, t);
    if resp.clicked() {
        *active = tab;
    }
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
}

#[cfg(test)]
mod w3_02_slice2_tests {
    use super::{PRESETS, validate_script};

    #[test]
    fn every_shipped_preset_actually_runs() {
        // The spec ships example templates; a template that doesn't compile/run
        // is worse than none. Validate each through the real engine.
        for (name, src) in PRESETS {
            let r = validate_script(src);
            assert!(r.is_ok(), "preset '{name}' failed to run: {:?}", r.err());
            assert!(r.unwrap().contains("OK"), "preset '{name}' should report OK");
        }
    }

    #[test]
    fn validate_reports_errors_and_empty() {
        assert!(validate_script("").is_err(), "empty script is an error");
        assert!(validate_script("   \n  ").is_err(), "whitespace-only is an error");
        let err = validate_script("let x = ;;;").unwrap_err();
        assert!(!err.is_empty(), "a syntax error surfaces a message");
        // a runaway is caught by the engine op-limit, surfaced here as Err
        assert!(validate_script("while true { let x = 1; } []").is_err());
    }
}
