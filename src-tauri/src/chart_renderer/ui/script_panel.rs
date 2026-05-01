//! Scripting / Backtesting panel — AI-driven strategy editor with backtest results.
//!
//! This is the UI scaffold. The actual script execution engine and AI integration
//! come later; for now, "Run" and "Backtest" produce mock output.

use egui;
use super::style::*;
use super::super::gpu::{Watchlist, Theme};

// ── Preset example scripts ──────────────────────────────────────────────────

const PRESETS: &[(&str, &str)] = &[
    ("SMA Crossover",  "buy when sma(close,10) crosses above sma(close,50)"),
    ("RSI Oversold",   "buy when rsi(close,14) < 30"),
    ("MACD Signal",    "buy when macd_line > signal_line AND macd_line[1] < signal_line[1]"),
];

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
    // Simple LCG-style deterministic sequence for reproducible mock data
    let mut seed: u32 = 42;
    let mut next = || -> f32 {
        seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
        ((seed >> 16) & 0x7FFF) as f32 / 32767.0 // 0.0..1.0
    };

    let mut trades = Vec::with_capacity(18);
    let base_price = 450.0_f32; // ~SPY-like price

    for _ in 0..18 {
        let is_long = next() > 0.4; // ~60% long bias
        let side = if is_long { "LONG" } else { "SHORT" };
        let entry = base_price + (next() - 0.5) * 40.0;
        let move_pct = (next() - 0.42) * 0.06; // slight positive bias → ~55% win rate
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

    // Simple max drawdown from cumulative P&L
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

    // Simplified Sharpe (mean / std of trade returns)
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

// ── draw_content: inner body for use inside analysis_panel tab ─────────────

pub(crate) fn draw_content(ui: &mut egui::Ui, watchlist: &mut Watchlist, t: &Theme) {
    let w = ui.available_width();

    // ── AI Prompt input ─────────────────────────────────────
    ui.horizontal(|ui| {
        ui.add(super::widgets::text::MonospaceCode::new("\u{2728}").xs().color(t.accent));
        ui.add_space(4.0);
        ui.add_sized(
            egui::vec2(w - 36.0, 22.0),
            egui::TextEdit::singleline(&mut watchlist.script_ai_prompt)
                .font(egui::FontId::monospace(9.5))
                .hint_text("Describe your indicator or strategy...")
                .text_color(egui::Color32::from_gray(210))
                .frame(true)
                .margin(egui::Margin::symmetric(6, 3))
        );
    });
    ui.add_space(4.0);

    // ── Preset examples ─────────────────────────────────────
    ui.horizontal(|ui| {
        ui.add(super::widgets::text::MonospaceCode::new("Examples:").xs().color(t.dim).gamma(0.5));
        for (name, source) in PRESETS {
            let btn = ui.add(egui::Button::new(
                egui::RichText::new(*name).monospace().size(8.0).color(t.accent.gamma_multiply(0.8)))
                .fill(color_alpha(t.accent, 12))
                .stroke(egui::Stroke::new(STROKE_THIN, color_alpha(t.accent, 35)))
                .corner_radius(r_md_cr())
                .min_size(egui::vec2(0.0, 16.0))
            );
            if btn.clicked() { watchlist.script_source = source.to_string(); }
            if btn.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
        }
    });
    ui.add_space(4.0);
    separator(ui, t.toolbar_border);
    ui.add_space(4.0);

    // ── Code editor area ────────────────────────────────────
    let editor_bg = color_alpha(t.bg, 200);
    let editor_height = 140.0;

    let (rect, _) = ui.allocate_exact_size(egui::vec2(w - 8.0, editor_height), egui::Sense::hover());
    ui.painter().rect_filled(rect, 4.0, editor_bg);
    ui.painter().rect_stroke(rect, 4.0, egui::Stroke::new(STROKE_THIN, color_alpha(t.toolbar_border, ALPHA_STRONG)), egui::StrokeKind::Outside);
    ui.allocate_new_ui(egui::UiBuilder::new().max_rect(rect.shrink(6.0)), |ui| {
        egui::ScrollArea::vertical()
            .id_salt("script_editor_tab")
            .show(ui, |ui| {
                ui.add_sized(
                    egui::vec2(rect.width() - 12.0, editor_height - 16.0),
                    egui::TextEdit::multiline(&mut watchlist.script_source)
                        .font(egui::FontId::monospace(10.0))
                        .code_editor()
                        .desired_rows(8)
                        .text_color(egui::Color32::from_gray(220))
                        .frame(false)
                );
            });
    });
    ui.add_space(4.0);

    // ── Button row ──────────────────────────────────────────
    ui.horizontal(|ui| {
        if action_button(ui, "\u{25B6} Run", t.bull, t).clicked() {
            if watchlist.script_source.is_empty() {
                watchlist.script_output = "Error: No script to run.".to_string();
            } else {
                watchlist.script_output = format!(
                    "Evaluating: {}\n\n--- Output ---\nScript parsed successfully.\nBars processed: 1,240\nSignals generated: 47",
                    watchlist.script_source
                );
            }
            watchlist.script_result_tab = ScriptResultTab::Output;
        }
        ui.add_space(4.0);
        if action_button(ui, "\u{1F4CA} Backtest", t.accent, t).clicked() {
            let result = mock_backtest();
            let mut out = String::new();
            out.push_str(&format!("Backtesting: {}\n", watchlist.script_source));
            out.push_str(&format!("Period: 252 bars | {} trades\n\n", result.trades.len()));
            out.push_str(&format!("Total P&L:      ${:.2}\n", result.total_pnl));
            out.push_str(&format!("Win Rate:       {:.1}%\n", result.win_rate));
            out.push_str(&format!("Profit Factor:  {:.2}\n", result.profit_factor));
            out.push_str(&format!("Max Drawdown:   {:.2}%\n", result.max_drawdown));
            out.push_str(&format!("Sharpe Ratio:   {:.2}\n", result.sharpe));
            watchlist.script_output = out;
            watchlist.script_backtest = Some(result);
            watchlist.script_result_tab = ScriptResultTab::Backtest;
        }
        ui.add_space(4.0);
        if action_button(ui, "Clear", t.bear.gamma_multiply(0.7), t).clicked() {
            watchlist.script_source.clear();
            watchlist.script_ai_prompt.clear();
            watchlist.script_output.clear();
            watchlist.script_backtest = None;
        }
    });
    ui.add_space(4.0);
    separator(ui, t.toolbar_border);
    ui.add_space(2.0);

    // ── Result tabs ─────────────────────────────────────────
    ui.horizontal(|ui| {
        result_tab_btn(ui, "Output", ScriptResultTab::Output, &mut watchlist.script_result_tab, t);
        ui.add_space(2.0);
        result_tab_btn(ui, "Backtest", ScriptResultTab::Backtest, &mut watchlist.script_result_tab, t);
    });
    ui.add_space(4.0);

    // ── Results area ────────────────────────────────────────
    egui::ScrollArea::vertical()
        .id_salt("script_results_tab")
        .show(ui, |ui| {
            ui.set_min_width(w - 4.0);
            match watchlist.script_result_tab {
                ScriptResultTab::Output => draw_output_tab(ui, watchlist, t),
                ScriptResultTab::Backtest => draw_backtest_tab(ui, watchlist, w, t),
            }
        });
}

fn separator(ui: &mut egui::Ui, color: egui::Color32) {
    super::style::separator(ui, color);
}

// ── Main draw function ──────────────────────────────────────────────────────

pub(crate) fn draw(ctx: &egui::Context, watchlist: &mut Watchlist, t: &Theme) {
    if !watchlist.script_open { return; }

    let mut close = false;

    egui::Window::new("apex_script")
        .default_pos(egui::pos2(280.0, 80.0))
        .default_size(egui::vec2(480.0, 620.0))
        .resizable(true)
        .movable(true)
        .title_bar(false)
        .frame(egui::Frame::popup(&ctx.style())
            .fill(t.toolbar_bg)
            .inner_margin(egui::Margin { left: 0, right: 0, top: 0, bottom: 0 })
            .stroke(egui::Stroke::new(STROKE_STD, color_alpha(t.toolbar_border, ALPHA_HEAVY)))
            .corner_radius(r_lg_cr()))
        .show(ctx, |ui| {
            let w = ui.available_width();

            // ── Header ──────────────────────────────────────────────
            ui.horizontal(|ui| {
                ui.add_space(8.0);
                ui.add(super::widgets::text::SectionLabel::new("APEX SCRIPT").color(t.accent));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(6.0);
                    if close_button(ui, t.dim) { close = true; }
                });
            });
            ui.add_space(4.0);
            divider(ui, w, t);
            ui.add_space(6.0);

            // ── AI Prompt input ─────────────────────────────────────
            ui.horizontal(|ui| {
                ui.add_space(8.0);
                ui.add(super::widgets::text::MonospaceCode::new("\u{2728}").xs().color(t.accent));
                ui.add_space(4.0);
                let prompt_response = ui.add_sized(
                    egui::vec2(w - 36.0, 22.0),
                    egui::TextEdit::singleline(&mut watchlist.script_ai_prompt)
                        .font(egui::FontId::monospace(9.5))
                        .hint_text("Describe your indicator or strategy...")
                        .text_color(egui::Color32::from_gray(210))
                        .frame(true)
                        .margin(egui::Margin::symmetric(6, 3))
                );
                // Style the text edit background
                let bg_rect = prompt_response.rect;
                ui.painter().set(
                    ui.painter().add(egui::Shape::Noop),
                    egui::Shape::Noop,
                );
                // Highlight border on focus
                if prompt_response.has_focus() {
                    ui.painter().rect_stroke(bg_rect, 3.0, egui::Stroke::new(STROKE_STD, color_alpha(t.accent, ALPHA_STRONG)), egui::StrokeKind::Outside);
                }
            });
            ui.add_space(6.0);

            // ── Preset examples ─────────────────────────────────────
            ui.horizontal(|ui| {
                ui.add_space(8.0);
                ui.label(egui::RichText::new("Examples:")
                    .monospace().size(8.0).color(t.dim.gamma_multiply(0.5)));
                ui.add_space(4.0);
                for (name, source) in PRESETS {
                    let btn = ui.add(egui::Button::new(
                        egui::RichText::new(*name).monospace().size(8.0).color(t.accent.gamma_multiply(0.8)))
                        .fill(color_alpha(t.accent, 12))
                        .stroke(egui::Stroke::new(STROKE_THIN, color_alpha(t.accent, 35)))
                        .corner_radius(r_md_cr())
                        .min_size(egui::vec2(0.0, 16.0))
                    );
                    if btn.clicked() {
                        watchlist.script_source = source.to_string();
                    }
                    if btn.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                }
            });
            ui.add_space(6.0);
            divider(ui, w, t);
            ui.add_space(6.0);

            // ── Code editor area ────────────────────────────────────
            let editor_bg = color_alpha(t.bg, 200);
            let editor_height = 160.0;

            // Dark background frame for code area
            ui.horizontal(|ui| {
                ui.add_space(6.0);
                let (rect, _) = ui.allocate_exact_size(
                    egui::vec2(w - 16.0, editor_height),
                    egui::Sense::hover(),
                );
                ui.painter().rect_filled(rect, 4.0, editor_bg);
                ui.painter().rect_stroke(rect, 4.0, egui::Stroke::new(STROKE_THIN, color_alpha(t.toolbar_border, ALPHA_STRONG)), egui::StrokeKind::Outside);

                // Place the text editor inside the rect
                ui.allocate_new_ui(egui::UiBuilder::new().max_rect(rect.shrink(6.0)), |ui| {
                    egui::ScrollArea::vertical()
                        .id_salt("script_editor")
                        .show(ui, |ui| {
                            ui.add_sized(
                                egui::vec2(rect.width() - 12.0, editor_height - 16.0),
                                egui::TextEdit::multiline(&mut watchlist.script_source)
                                    .font(egui::FontId::monospace(10.0))
                                    .code_editor()
                                    .desired_rows(10)
                                    .text_color(egui::Color32::from_gray(220))
                                    .frame(false)
                            );
                        });
                });
            });
            ui.add_space(6.0);

            // ── Button row ──────────────────────────────────────────
            ui.horizontal(|ui| {
                ui.add_space(8.0);
                // Run button (accent/green)
                if action_button(ui, "\u{25B6} Run", t.bull, t).clicked() {
                    // Mock run — just echo the source
                    if watchlist.script_source.is_empty() {
                        watchlist.script_output = "Error: No script to run. Enter a script or select a preset.".to_string();
                    } else {
                        watchlist.script_output = format!(
                            "Evaluating: {}\n\n--- Output ---\nScript parsed successfully.\nBars processed: 1,240\nSignals generated: 47\nLast signal: BUY at bar 1,238 (close = $452.30)",
                            watchlist.script_source
                        );
                    }
                    watchlist.script_result_tab = ScriptResultTab::Output;
                }

                ui.add_space(4.0);

                // Backtest button (accent)
                if action_button(ui, "\u{1F4CA} Backtest", t.accent, t).clicked() {
                    let result = mock_backtest();
                    // Format output
                    let mut out = String::new();
                    out.push_str(&format!("Backtesting: {}\n", watchlist.script_source));
                    out.push_str(&format!("Period: 252 bars | {} trades\n\n", result.trades.len()));
                    out.push_str(&format!("Total P&L:      ${:.2}\n", result.total_pnl));
                    out.push_str(&format!("Win Rate:       {:.1}%\n", result.win_rate));
                    out.push_str(&format!("Profit Factor:  {:.2}\n", result.profit_factor));
                    out.push_str(&format!("Max Drawdown:   {:.2}%\n", result.max_drawdown));
                    out.push_str(&format!("Sharpe Ratio:   {:.2}\n", result.sharpe));
                    watchlist.script_output = out;
                    watchlist.script_backtest = Some(result);
                    watchlist.script_result_tab = ScriptResultTab::Backtest;
                }

                ui.add_space(4.0);

                // Save button (dim)
                if action_button(ui, "Save", t.dim, t).clicked() {
                    watchlist.script_output = "Script saved. (placeholder — persistence coming soon)".to_string();
                    watchlist.script_result_tab = ScriptResultTab::Output;
                }

                ui.add_space(4.0);

                // Clear button (bear/red)
                if action_button(ui, "Clear", t.bear.gamma_multiply(0.7), t).clicked() {
                    watchlist.script_source.clear();
                    watchlist.script_ai_prompt.clear();
                    watchlist.script_output.clear();
                    watchlist.script_backtest = None;
                }
            });
            ui.add_space(6.0);
            divider(ui, w, t);
            ui.add_space(4.0);

            // ── Result tabs ─────────────────────────────────────────
            ui.horizontal(|ui| {
                ui.add_space(8.0);
                result_tab_btn(ui, "Output", ScriptResultTab::Output, &mut watchlist.script_result_tab, t);
                ui.add_space(2.0);
                result_tab_btn(ui, "Backtest", ScriptResultTab::Backtest, &mut watchlist.script_result_tab, t);
            });
            ui.add_space(4.0);

            // ── Results area ────────────────────────────────────────
            egui::ScrollArea::vertical()
                .id_salt("script_results")
                .show(ui, |ui| {
                    ui.set_min_width(w - 4.0);
                    match watchlist.script_result_tab {
                        ScriptResultTab::Output => draw_output_tab(ui, watchlist, t),
                        ScriptResultTab::Backtest => draw_backtest_tab(ui, watchlist, w, t),
                    }
                });
        });

    if close { watchlist.script_open = false; }
}

// ── Output tab ──────────────────────────────────────────────────────────────

fn draw_output_tab(ui: &mut egui::Ui, watchlist: &Watchlist, t: &Theme) {
    let m = 10.0;
    if watchlist.script_output.is_empty() {
        ui.add_space(20.0);
        ui.vertical_centered(|ui| {
            ui.add(super::widgets::text::MonospaceCode::new("Run a script or backtest to see results here.").xs().color(t.dim).gamma(0.4));
        });
    } else {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.add_space(m);
            ui.add(super::widgets::text::MonospaceCode::new(&watchlist.script_output).xs().color(t.dim).gamma(0.85));
        });
    }
    ui.add_space(8.0);
}

// ── Backtest tab ────────────────────────────────────────────────────────────

fn draw_backtest_tab(ui: &mut egui::Ui, watchlist: &Watchlist, w: f32, t: &Theme) {
    let m = 8.0;
    let result = match &watchlist.script_backtest {
        Some(r) => r,
        None => {
            ui.add_space(20.0);
            ui.vertical_centered(|ui| {
                ui.add(super::widgets::text::MonospaceCode::new("Click \"Backtest\" to generate results.").xs().color(t.dim).gamma(0.4));
            });
            return;
        }
    };

    ui.add_space(6.0);

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
            ui.painter().rect_filled(rect, 3.0, color_alpha(t.toolbar_border, ALPHA_TINT));
            ui.painter().rect_stroke(rect, 3.0, egui::Stroke::new(STROKE_THIN, color_alpha(t.toolbar_border, ALPHA_LINE)), egui::StrokeKind::Outside);

            // Label
            ui.painter().text(
                egui::pos2(rect.center().x, rect.min.y + 10.0),
                egui::Align2::CENTER_CENTER,
                label,
                egui::FontId::monospace(7.5),
                t.dim.gamma_multiply(0.5),
            );
            // Value
            ui.painter().text(
                egui::pos2(rect.center().x, rect.min.y + 26.0),
                egui::Align2::CENTER_CENTER,
                value,
                egui::FontId::monospace(9.0),
                *color,
            );
        }
    });

    ui.add_space(6.0);

    // ── Trade list header ───────────────────────────────────
    ui.horizontal(|ui| {
        ui.add_space(m);
        ui.add(super::widgets::text::MonospaceCode::new(&format!("TRADES ({})", result.trades.len())).xs().color(t.dim).gamma(0.5).strong(true));
    });
    ui.add_space(4.0);

    // Column header
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
            egui::FontId::monospace(7.5),
            t.dim.gamma_multiply(0.4),
        );
    }

    // Divider under header
    let div_y = header_y + 14.0;
    let div_rect = egui::Rect::from_min_size(
        egui::pos2(ui.cursor().min.x + m, div_y),
        egui::vec2(w - m * 2.0, 1.0),
    );
    ui.painter().rect_filled(div_rect, 0.0, color_alpha(t.toolbar_border, ALPHA_MUTED));
    ui.add_space(2.0);

    // Trade rows
    for trade in &result.trades {
        let row_y = ui.cursor().min.y;
        let row_rect = egui::Rect::from_min_size(
            egui::pos2(ui.cursor().min.x, row_y),
            egui::vec2(w, 16.0),
        );
        let resp = ui.allocate_rect(row_rect, egui::Sense::hover());
        if resp.hovered() {
            ui.painter().rect_filled(row_rect, 1.0, color_alpha(t.toolbar_border, ALPHA_SUBTLE));
        }

        let base_x = ui.cursor().min.x;
        let cy = row_y + 8.0;
        let pnl_color = if trade.pnl >= 0.0 { t.bull } else { t.bear };

        // Side badge
        let side_col = if trade.side == "LONG" { t.bull } else { t.bear };
        let side_rect = egui::Rect::from_min_size(
            egui::pos2(base_x + col_x[0], row_y + 1.0),
            egui::vec2(32.0, 14.0),
        );
        ui.painter().rect_filled(side_rect, 2.0, color_alpha(side_col, ALPHA_SOFT));
        ui.painter().text(
            side_rect.center(),
            egui::Align2::CENTER_CENTER,
            trade.side,
            egui::FontId::monospace(7.0),
            side_col,
        );

        // Entry price
        ui.painter().text(
            egui::pos2(base_x + col_x[1], cy),
            egui::Align2::LEFT_CENTER,
            format!("{:.2}", trade.entry_price),
            egui::FontId::monospace(8.5),
            t.dim.gamma_multiply(0.8),
        );

        // Exit price
        ui.painter().text(
            egui::pos2(base_x + col_x[2], cy),
            egui::Align2::LEFT_CENTER,
            format!("{:.2}", trade.exit_price),
            egui::FontId::monospace(8.5),
            t.dim.gamma_multiply(0.8),
        );

        // P&L
        let pnl_sign = if trade.pnl >= 0.0 { "+" } else { "" };
        ui.painter().text(
            egui::pos2(base_x + col_x[3], cy),
            egui::Align2::LEFT_CENTER,
            format!("{}${:.2}", pnl_sign, trade.pnl),
            egui::FontId::monospace(8.5),
            pnl_color,
        );

        // P&L %
        ui.painter().text(
            egui::pos2(base_x + col_x[4], cy),
            egui::Align2::LEFT_CENTER,
            format!("{}{:.2}%", pnl_sign, trade.pnl_pct),
            egui::FontId::monospace(8.5),
            pnl_color,
        );
    }

    ui.add_space(10.0);
}

// ── Helper widgets ──────────────────────────────────────────────────────────

/// Horizontal divider line.
fn divider(ui: &mut egui::Ui, w: f32, t: &Theme) {
    let rect = egui::Rect::from_min_size(
        egui::pos2(ui.cursor().min.x, ui.cursor().min.y),
        egui::vec2(w, 1.0),
    );
    ui.painter().rect_filled(rect, 0.0, color_alpha(t.toolbar_border, ALPHA_DIM));
    ui.advance_cursor_after_rect(rect);
}

/// Accent-colored action button for the toolbar row.
fn action_button(ui: &mut egui::Ui, label: &str, color: egui::Color32, t: &Theme) -> egui::Response {
    let resp = ui.add(egui::Button::new(
        egui::RichText::new(label).monospace().size(9.0).color(color))
        .fill(color_alpha(color, ALPHA_GHOST))
        .stroke(egui::Stroke::new(STROKE_THIN, color_alpha(color, ALPHA_LINE)))
        .corner_radius(r_md_cr())
        .min_size(egui::vec2(0.0, 20.0))
    );
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        // Paint subtle highlight
        ui.painter().rect_filled(resp.rect, r_sm_cr(), color_alpha(color, 8));
    }
    let _ = t; // suppress unused warning (theme available for future styling)
    resp
}

/// Tab button for Output / Backtest result tabs.
fn result_tab_btn(ui: &mut egui::Ui, label: &str, tab: ScriptResultTab, active: &mut ScriptResultTab, t: &Theme) {
    let is_active = *active == tab;
    let fg = if is_active { t.accent } else { t.dim.gamma_multiply(0.5) };
    let bg = if is_active { color_alpha(t.accent, 18) } else { egui::Color32::TRANSPARENT };
    let border = if is_active { color_alpha(t.accent, ALPHA_DIM) } else { color_alpha(t.toolbar_border, ALPHA_MUTED) };

    let resp = ui.add(egui::Button::new(
        egui::RichText::new(label).monospace().size(9.0).color(fg))
        .fill(bg)
        .stroke(egui::Stroke::new(STROKE_THIN, border))
        .corner_radius(r_md_cr())
        .min_size(egui::vec2(0.0, 18.0))
    );
    if resp.clicked() {
        *active = tab;
    }
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
}
