//! Chart pane rendering — render_toolbar, render_chart_pane, draw_chart, and helpers.
//!
//! This module is a direct extraction from gpu.rs. All types are imported from the
//! gpu module. Private imports use super::super::* to reach the renderer crate root.

#![allow(unused_imports)]
#![allow(clippy::wildcard_imports)]

use std::sync::{Arc, Mutex};
use std::sync::mpsc;
use std::fmt::Write as FmtWrite;
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
use winit::window::Window;

use crate::chart_renderer::gpu::*;
use crate::chart_renderer::gpu;
use crate::chart_renderer::trading::*;
use crate::chart_renderer::{ui, compute, LineStyle, Bar, ChartCommand,
    Drawing, DrawingGroup, DrawingKind, DrawingSignificance, PatternLabel,
    AnalysisTab, SignalsTab, FeedTab, BookTab, PaneHeaderSize,
    ChartWidgetKind, ChartWidget, WidgetPreset, WidgetDisplayMode, WidgetDock,
    PlayType, PlayDirection, PlayStatus, PlayLine, PlayLineKind, Play, PlayTarget,
    SpreadLeg, PlayTemplate, SignalZone, DivergenceMarker,
};
use crate::chart_renderer::ui::style::{
    hex_to_color, dashed_line, draw_line_rgba, section_label, dim_label, color_alpha,
    separator, status_badge, order_card, action_btn, trade_btn, close_button,
    dialog_window_themed, dialog_header, dialog_separator_shadow, dialog_section,
    paint_tooltip_shadow, tooltip_frame, stat_row, segmented_control,
    paint_chrome_tile_button, ChromeTileState, chrome_tile_fg,
    FONT_LG, FONT_MD, FONT_SM, STROKE_THIN, STROKE_STD,
    ALPHA_FAINT, ALPHA_GHOST, ALPHA_SUBTLE, ALPHA_TINT, ALPHA_MUTED,
    ALPHA_LINE, ALPHA_DIM, ALPHA_STRONG, ALPHA_ACTIVE, ALPHA_HEAVY,
    TEXT_PRIMARY, COLOR_AMBER,
};
use crate::chart_renderer::ui::style as style;
use crate::chart_renderer::ui::widgets::foundation::text_style::TextStyle;
use crate::chart_renderer::compute::{
    compute_sma, compute_ema, compute_rsi, compute_macd, compute_stochastic,
    compute_vwap, detect_divergences, bs_price, strike_interval, atm_strike,
    get_iv, sim_oi, compute_atr, compute_bollinger, compute_ichimoku,
    compute_psar, compute_supertrend, compute_keltner, compute_adx,
    compute_cci, compute_williams_r,
};
use crate::ui_kit::icons::Icon;
// Disambiguate APEXIB_URL (exists in both gpu and trading)
use crate::chart_renderer::gpu::APEXIB_URL;

pub(crate) fn render_toolbar(
    ctx: &egui::Context,
    panes: &mut Vec<Chart>,
    active_pane: &mut usize,
    layout: &mut Layout,
    watchlist: &mut Watchlist,
    t: &Theme,
    theme_idx: usize,
    account_data_cached: &Option<(AccountSummary, Vec<Position>, Vec<IbOrder>)>,
    win_ref: Option<Arc<Window>>,
    conn_panel_open: &mut bool,
    toasts: &[(String, f32, std::time::Instant, bool)],
) {
    crate::chart_renderer::ui::widgets::toolbar::top_nav::TopNav::new()
        .panes(panes)
        .active_pane(active_pane)
        .layout(layout)
        .watchlist(watchlist)
        .theme(t, theme_idx)
        .account(Some(account_data_cached))
        .window(win_ref)
        .conn_panel_open(conn_panel_open)
        .toasts(toasts)
        .show(ctx);
}

// ── toolbar body moved to ui/widgets/toolbar/top_nav.rs ───────────────────────






/// Phase 6+7: Render a single chart pane (candles, overlays, indicators, interactions).
#[allow(unused_assignments)]
fn render_chart_pane(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    panes: &mut [Chart],
    pane_idx: usize,
    active_pane: &mut usize,
    visible_count: usize,
    pane_rects: &[egui::Rect],
    theme_idx: usize,
    watchlist: &mut Watchlist,
    account_data_cached: &Option<(AccountSummary, Vec<Position>, Vec<IbOrder>)>,
) {
    use crate::monitoring::{span_begin, span_end};
    let pane_rect = if pane_idx < pane_rects.len() { pane_rects[pane_idx] } else { pane_rects[0] };
    crate::design_tokens::register_hit(
        [pane_rect.min.x, pane_rect.min.y, pane_rect.width(), pane_rect.height()],
        "CHART_PANE", "Chart");
    let chart = &mut panes[pane_idx];
    // ── Sync orders from OrderManager (single source of truth) ──
    // Merge: OrderManager orders take precedence, keep local-only orders too
    {
        let mgr_orders = crate::chart_renderer::trading::order_manager::all_order_levels_for(&chart.symbol);
        // Update existing local orders with manager state, add new ones
        for mo in &mgr_orders {
            if let Some(local) = chart.orders.iter_mut().find(|o| o.id == mo.id) {
                local.status = mo.status; local.price = mo.price; local.qty = mo.qty;
            } else {
                chart.orders.push(mo.clone());
            }
        }
        // Remove local orders that were cancelled/filled in the manager
        chart.orders.retain(|o| {
            if mgr_orders.iter().any(|m| m.id == o.id) { return true; } // manager knows about it
            o.status != OrderStatus::Cancelled // keep non-cancelled local-only orders
        });
    }
    let is_active = pane_idx == *active_pane;
    let _t_owned = get_theme(chart.theme_idx);
    let t = &_t_owned;

    // ── Replay auto-advance when playing ──
    if chart.replay_mode && chart.replay_playing {
        let interval_ms = (1000.0 / chart.replay_speed) as u128;
        let should_step = chart.replay_last_step
            .map_or(true, |ts| ts.elapsed().as_millis() >= interval_ms);
        if should_step && chart.replay_bar_count < chart.bars.len() {
            chart.replay_bar_count += 1;
            chart.indicator_bar_count = 0; // force recompute
            chart.replay_last_step = Some(std::time::Instant::now());
            // Keep viewport following the replay head
            chart.vs = (chart.replay_bar_count as f32 - chart.vc as f32 + CHART_RIGHT_PAD as f32).max(0.0);
            ctx.request_repaint();
        }
        if chart.replay_bar_count >= chart.bars.len() {
            chart.replay_playing = false; // stop at end
        }
    }

    let n = chart.bars.len();
    let n = if chart.replay_mode { chart.replay_bar_count.min(n) } else { n };

    // Draw pane border — dispatch on hairline_borders for Meridien vs legacy (#8).
    {
        let st = crate::chart_renderer::ui::style::current();
        if st.hairline_borders {
            // Meridien: crisp hairline rules top/left/bottom + accent top accent on active pane.
            let painter = ui.painter();
            let rule_col = crate::chart_renderer::ui::style::rule_stroke_for(t.bg, t.toolbar_border);
            // Top hairline
            painter.line_segment(
                [pane_rect.left_top(), pane_rect.right_top()],
                rule_col,
            );
            if visible_count > 1 {
                // Left hairline
                painter.line_segment(
                    [pane_rect.left_top(), pane_rect.left_bottom()],
                    rule_col,
                );
                // Bottom hairline
                painter.line_segment(
                    [pane_rect.left_bottom(), pane_rect.right_bottom()],
                    rule_col,
                );
                // Right hairline
                painter.line_segment(
                    [pane_rect.right_top(), pane_rect.right_bottom()],
                    rule_col,
                );
            }
        } else if visible_count > 1 {
            // Legacy: uniform dim hairline — no active/inactive distinction.
            let bw = st.pane_border_width;
            let border_color = t.dim.gamma_multiply(0.3);
            ui.painter().rect_stroke(pane_rect, 0.0, egui::Stroke::new(bw * 0.5, border_color), egui::StrokeKind::Inside);
        }
    }

    // (restore button drawn later, after chart content, so it's on top)

    // Pane header (symbol + timeframe + per-pane selector) for multi-pane or tabbed layouts
    // Sync tab state: ensure active tab matches current symbol
    if !chart.tab_symbols.is_empty() {
        let ai = chart.tab_active.min(chart.tab_symbols.len().saturating_sub(1));
        chart.tab_active = ai;
        chart.tab_symbols[ai] = chart.symbol.clone();
        chart.tab_timeframes[ai] = chart.timeframe.clone();
        // Keep tab_prices in sync with tab_symbols length
        while chart.tab_prices.len() < chart.tab_symbols.len() { chart.tab_prices.push(0.0); }
        // Update cached change % AND price for active tab from current bars
        if let (Some(first), Some(last)) = (chart.bars.first(), chart.bars.last()) {
            if first.open > 0.0 && chart.tab_changes.len() > ai {
                chart.tab_changes[ai] = (last.close - first.open) / first.open * 100.0;
            }
            if chart.tab_prices.len() > ai {
                chart.tab_prices[ai] = last.close;
            }
        }
    }
    let has_tabs = chart.tab_symbols.len() > 1;
    // Always show header (18px min) so + tab button is accessible even in single-pane
    let pane_top_offset = if has_tabs {
        pane_tabs_header_h(watchlist)
    } else {
        pane_header_h(watchlist)
    };
    let title_font_size = watchlist.pane_header_size.title_font();
    let show_header = true;
    if show_header {
        use crate::chart_renderer::ui::widgets::painter_pane::PainterPaneHeader;

        let header_rect = egui::Rect::from_min_size(pane_rect.min, egui::vec2(pane_rect.width(), pane_top_offset));
        crate::design_tokens::register_hit(
            [header_rect.min.x, header_rect.min.y, header_rect.width(), header_rect.height()],
            "PANE_HEADER", "Pane Header");

        // Header chrome (bg fill, outer border, post-nav divider) is fully owned
        // by `PainterPaneHeader` — every pane-header style knob lives in
        // `StyleSettings` (`active_header_fill_multiply`,
        // `inactive_header_fill_multiply`, `header_outer_border_alpha`,
        // `header_outer_border_width`, `header_divider_alpha`).

        // ── Build tab slice for widget ─────────────────────────────────────
        let can_go_back = chart.symbol_history_idx > 0;
        let can_go_fwd  = chart.symbol_history_idx < chart.symbol_history.len();

        // Option badges data (shared between tab/simple paths)
        let opt_side   = if chart.is_option { chart.option_type.as_str() } else { "" };
        let opt_expiry = if chart.is_option { chart.option_expiry.as_str() } else { "" };

        // Price text for simple-label mode
        let (price_text_owned, price_col_val) = if let Some(last) = chart.bars.last() {
            let chg_col = if let Some(first) = chart.bars.first() {
                if first.open > 0.0 && last.close >= first.open { t.bull } else { t.bear }
            } else { t.dim };
            (format!("${:.2}", last.close), chg_col)
        } else {
            (String::new(), t.dim)
        };

        // Symbol label for simple-label mode.
        // Chart mode: show symbol (or underlying+strike for options).
        // Other modes: show "Mode · Template" shape.
        let sym_label_owned: String = if !has_tabs {
            match chart.pane_type {
                PaneType::Chart => {
                    if chart.is_option && !chart.underlying.is_empty() {
                        let strike = chart.option_strike;
                        let strike_str = if (strike - strike.round()).abs() < 0.005 {
                            format!("{:.0}", strike)
                        } else { format!("{:.1}", strike) };
                        if let Some(tpl) = &chart.pane_template_name {
                            format!("{} · {} {}", tpl, chart.underlying, strike_str)
                        } else {
                            format!("{} {}", chart.underlying, strike_str)
                        }
                    } else if let Some(tpl) = &chart.pane_template_name {
                        format!("{} · {}", tpl, chart.symbol)
                    } else {
                        chart.symbol.clone()
                    }
                }
                PaneType::Portfolio => {
                    let tpl = chart.pane_template_name.as_deref().unwrap_or("Default");
                    format!("Portfolio · {}", tpl)
                }
                PaneType::Dashboard => {
                    let tpl = chart.pane_template_name.as_deref().unwrap_or("Default");
                    format!("Dashboard · {}", tpl)
                }
                PaneType::Heatmap => {
                    let tpl = chart.pane_template_name.as_deref().unwrap_or("Default");
                    format!("Heatmap · {}", tpl)
                }
                PaneType::Spreadsheet => {
                    let tpl = chart.pane_template_name.as_deref().unwrap_or("Default");
                    format!("Spreadsheet · {}", tpl)
                }
            }
        } else { String::new() };

        // Tab data: (display_sym, price_text, change_pct) — widget uses (sym, price, chg)
        let tab_price_texts: Vec<String> = if has_tabs {
            (0..chart.tab_symbols.len()).map(|i| {
                let price = if i < chart.tab_prices.len() { chart.tab_prices[i] } else { 0.0 };
                if price > 0.0 { format!("${:.2}", price) } else { String::new() }
            }).collect()
        } else { vec![] };

        // For tab display symbols:
        //   - non-Chart panes show the template name (or mode name if none)
        //   - Chart panes show the template name when set, otherwise the symbol
        //     (with "UNDER STRIKE" treatment for active option-mode tabs)
        let mode_label = match chart.pane_type {
            PaneType::Chart       => "Chart",
            PaneType::Portfolio   => "Portfolio",
            PaneType::Dashboard   => "Dashboard",
            PaneType::Heatmap     => "Heatmap",
            PaneType::Spreadsheet => "Spreadsheet",
        };
        let tab_display_syms: Vec<String> = if has_tabs {
            (0..chart.tab_symbols.len()).map(|i| {
                let is_active_tab = i == chart.tab_active;
                // For non-Chart modes, every tab shows the template/mode name.
                if chart.pane_type != PaneType::Chart {
                    return chart.pane_template_name.clone()
                        .unwrap_or_else(|| mode_label.to_string());
                }
                // Chart mode with a selected template — show the template name.
                if let Some(tpl) = chart.pane_template_name.as_deref() {
                    return tpl.to_string();
                }
                // Chart mode default — symbol (with option-strike treatment).
                if is_active_tab && chart.is_option && !chart.underlying.is_empty() {
                    let strike = chart.option_strike;
                    let strike_str = if (strike - strike.round()).abs() < 0.005 {
                        format!("{:.0}", strike)
                    } else { format!("{:.1}", strike) };
                    format!("{} {}", chart.underlying, strike_str)
                } else {
                    chart.tab_symbols[i].clone()
                }
            }).collect()
        } else { vec![] };

        let tab_changes: Vec<f32> = if has_tabs {
            (0..chart.tab_symbols.len()).map(|i|
                if i < chart.tab_changes.len() { chart.tab_changes[i] } else { 0.0 }
            ).collect()
        } else { vec![] };

        let tab_refs: Vec<(&str, &str, f32)> = tab_display_syms.iter()
            .zip(tab_price_texts.iter())
            .zip(tab_changes.iter())
            .map(|((s, p), c)| (s.as_str(), p.as_str(), *c))
            .collect();

        // ── Widget call ───────────────────────────────────────────────────
        let mut builder = PainterPaneHeader::new(header_rect, t)
            .is_active(is_active)
            .visible_count(visible_count)
            .show_link_dot(true)
            .link_group(chart.link_group)
            .show_back_fwd(true)
            .can_go_back(can_go_back)
            .can_go_fwd(can_go_fwd)
            .show_plus_tab(true)
            .show_order_btn(watchlist.order_entry_open)
            .show_dom_btn(chart.dom_sidebar_open)
            .tab_sense(egui::Sense::click_and_drag())
            .pane_index(pane_idx)
            .title_font_size(title_font_size)
            .active_tab(chart.tab_active)
            .hovered_tab(chart.tab_hovered);

        if has_tabs {
            builder = builder.tabs(&tab_refs);
        } else {
            builder = builder
                .symbol(sym_label_owned.as_str())
                .price(price_text_owned.as_str(), price_col_val);
            if chart.is_option && (!opt_side.is_empty() || !opt_expiry.is_empty()) {
                builder = builder.option_badges(opt_side, opt_expiry);
            }
        }
        // Also paint option badges in tab mode
        if has_tabs && chart.is_option && (!opt_side.is_empty() || !opt_expiry.is_empty()) {
            builder = builder.option_badges(opt_side, opt_expiry);
        }

        let hdr = builder.show(ui);

        // ── Wire response → chart mutations ───────────────────────────────

        // Link group cycle
        if hdr.clicked_link {
            chart.link_group = (chart.link_group + 1) % 5;
        }

        // Back / Fwd navigation
        if hdr.clicked_back && can_go_back {
            chart.symbol_history_idx -= 1;
            let target = chart.symbol_history[chart.symbol_history_idx].clone();
            chart.symbol_nav_in_progress = true;
            chart.pending_symbol_change = Some(target);
        }
        if hdr.clicked_fwd && can_go_fwd {
            let target = chart.symbol_history[chart.symbol_history_idx].clone();
            chart.symbol_history_idx += 1;
            chart.symbol_nav_in_progress = true;
            chart.pending_symbol_change = Some(target);
        }

        // Tab hovered tracking — widget uses chart.tab_hovered as input each frame.
        // We can't recover which tab index is hovered from the response without tab rects,
        // so we clear hovered when pointer leaves the header area and rely on the widget
        // painting the correct cursor icon from the previous-frame hovered_tab input.
        if hdr.hover_pos.is_none() {
            chart.tab_hovered = None;
        }

        // Tab drag (cross-pane)
        if let Some(ti) = hdr.tab_drag_started {
            if watchlist.dragging_tab.is_none() {
                let price = if ti < chart.tab_prices.len() { chart.tab_prices[ti] } else { 0.0 };
                let chg   = if ti < chart.tab_changes.len() { chart.tab_changes[ti] } else { 0.0 };
                let start_pos = hdr.tab_drag_pos.map(|(_, p)| p).unwrap_or(header_rect.center());
                watchlist.dragging_tab = Some(TabDragState {
                    source_pane: pane_idx,
                    tab_idx: ti,
                    symbol: if ti < chart.tab_symbols.len() { chart.tab_symbols[ti].clone() } else { String::new() },
                    timeframe: if ti < chart.tab_timeframes.len() { chart.tab_timeframes[ti].clone() } else { String::new() },
                    price, change: chg,
                    current_pos: start_pos,
                });
            }
        }
        if let Some((ti, pos)) = hdr.tab_drag_pos {
            if let Some(drag) = watchlist.dragging_tab.as_mut() {
                if drag.source_pane == pane_idx && drag.tab_idx == ti {
                    drag.current_pos = pos;
                }
            }
        }

        // Plus tab
        if hdr.clicked_plus {
            if has_tabs {
                chart.tab_symbols.push(chart.symbol.clone());
                chart.tab_timeframes.push(chart.timeframe.clone());
                chart.tab_changes.push(0.0);
                chart.tab_prices.push(0.0);
                chart.tab_active = chart.tab_symbols.len() - 1;
                *active_pane = pane_idx;
                if chart.is_option {
                    chart.option_quick_open = true;
                    chart.option_quick_pos = egui::pos2(header_rect.left(), header_rect.bottom() + 4.0);
                    if !chart.underlying.is_empty() {
                        let dte_list = [0, 1, 2, 3, 7, 14, 30, 60];
                        let dte = dte_list[chart.option_quick_dte_idx.min(dte_list.len() - 1)];
                        let px = chart.bars.last().map(|b| b.close).unwrap_or(0.0);
                        fetch_chain_background(chart.underlying.clone(), 15, dte, px);
                    }
                } else {
                    chart.picker_open = true;
                    chart.picker_query.clear();
                    chart.picker_results.clear();
                    chart.picker_last_query.clear();
                    chart.picker_pos = egui::pos2(header_rect.left(), header_rect.bottom());
                }
            } else {
                // No-tab mode: initialize tabs then add empty tab 1
                if chart.tab_symbols.is_empty() {
                    chart.tab_symbols.push(chart.symbol.clone());
                    chart.tab_timeframes.push(chart.timeframe.clone());
                    let (chg, px) = if let (Some(f), Some(l)) = (chart.bars.first(), chart.bars.last()) {
                        let c = if f.open > 0.0 { (l.close - f.open) / f.open * 100.0 } else { 0.0 };
                        (c, l.close)
                    } else { (0.0, 0.0) };
                    chart.tab_changes.push(chg);
                    chart.tab_prices.push(px);
                }
                chart.tab_symbols.push("".into());
                chart.tab_timeframes.push(chart.timeframe.clone());
                chart.tab_changes.push(0.0);
                chart.tab_prices.push(0.0);
                chart.tab_active = chart.tab_symbols.len() - 1;
                if chart.is_option {
                    chart.option_quick_open = true;
                    chart.option_quick_pos = egui::pos2(header_rect.left(), header_rect.bottom() + 4.0);
                    if !chart.underlying.is_empty() {
                        let dte_list = [0, 1, 2, 3, 7, 14, 30, 60];
                        let dte = dte_list[chart.option_quick_dte_idx.min(dte_list.len() - 1)];
                        let px = chart.bars.last().map(|b| b.close).unwrap_or(0.0);
                        fetch_chain_background(chart.underlying.clone(), 15, dte, px);
                    }
                } else {
                    chart.picker_open = true;
                    chart.picker_query.clear();
                    chart.picker_results.clear();
                }
            }
        }

        // Template button
        if hdr.clicked_template {
            chart.template_popup_open = !chart.template_popup_open;
            chart.template_popup_pos = egui::pos2(header_rect.right() - 30.0, header_rect.bottom() + 4.0);
        }

        // Symbol click (simple-label mode — opens pane picker)
        if hdr.clicked_symbol {
            *active_pane = pane_idx;
            chart.pane_picker_open = !chart.pane_picker_open;
            chart.pane_picker_query.clear();
            chart.pane_picker_option_mode = chart.is_option;
            if let Some(sr) = hdr.symbol_rect {
                chart.pane_picker_pos = egui::pos2(sr.left(), sr.bottom() + 4.0);
            } else {
                chart.pane_picker_pos = egui::pos2(header_rect.left() + 4.0, header_rect.bottom() + 4.0);
            }
        }

        // Tab close
        if hdr.clicked_close {
            if let Some(ci) = hdr.clicked_tab {
                if ci < chart.tab_symbols.len() {
                    chart.tab_symbols.remove(ci);
                    chart.tab_timeframes.remove(ci);
                    if ci < chart.tab_changes.len() { chart.tab_changes.remove(ci); }
                    if ci < chart.tab_prices.len()  { chart.tab_prices.remove(ci);  }
                    if chart.tab_active >= chart.tab_symbols.len() {
                        chart.tab_active = chart.tab_symbols.len().saturating_sub(1);
                    } else if chart.tab_active > ci {
                        chart.tab_active -= 1;
                    }
                    if !chart.tab_symbols.is_empty() {
                        let ai = chart.tab_active;
                        let new_sym = chart.tab_symbols[ai].clone();
                        let new_tf  = chart.tab_timeframes[ai].clone();
                        if new_sym != chart.symbol { chart.pending_symbol_change = Some(new_sym); }
                        if new_tf  != chart.timeframe { chart.pending_timeframe_change = Some(new_tf); }
                    }
                }
            }
        } else if let Some(ci) = hdr.clicked_tab {
            // Tab click (no close)
            if ci != chart.tab_active {
                chart.tab_active = ci;
                let new_sym = chart.tab_symbols[ci].clone();
                let new_tf  = chart.tab_timeframes[ci].clone();
                if !new_sym.is_empty() && new_sym != chart.symbol {
                    chart.pending_symbol_change = Some(new_sym.clone());
                }
                if !new_tf.is_empty() && new_tf != chart.timeframe {
                    chart.pending_timeframe_change = Some(new_tf);
                }
                if new_sym == chart.symbol && chart.bars.is_empty() && !new_sym.is_empty() {
                    fetch_bars_background(new_sym, chart.timeframe.clone());
                }
            } else {
                // Already-active tab clicked — open the unified pane picker
                // (mode tabs + template selector + symbol/option search).
                chart.pane_picker_open = true;
                chart.pane_picker_query.clear();
                chart.pane_picker_option_mode = chart.is_option;
                chart.pane_picker_pos = egui::pos2(header_rect.left(), header_rect.bottom() + 4.0);
            }
        }

        // Rest of header — click to activate pane
        if hdr.response.clicked() { *active_pane = pane_idx; }

        // Order-entry toggle
        if hdr.clicked_order { watchlist.order_entry_open = !watchlist.order_entry_open; }
        // DOM sidebar toggle
        if hdr.clicked_dom { chart.dom_sidebar_open = !chart.dom_sidebar_open; }
    }

    // ── Pane content picker popup ────────────────────────────────────────────
    if chart.pane_picker_open {
        let popup_id = egui::Id::new(("pane_picker", pane_idx));
        let anchor = chart.pane_picker_pos;
        let mut close_picker = false;

        egui::Window::new("__pane_picker")
            .id(popup_id)
            .fixed_pos(anchor)
            .title_bar(false)
            .resizable(false)
            .frame(egui::Frame::popup(ui.style()))
            .show(ui.ctx(), |ui| {
                ui.set_min_width(320.0);
                ui.set_max_width(420.0);

                // ── Mode tab row ─────────────────────────────────────────────
                ui.horizontal(|ui| {
                    for (ptype, label) in [
                        (PaneType::Chart,       "Chart"),
                        (PaneType::Portfolio,   "Portfolio"),
                        (PaneType::Dashboard,   "Dashboard"),
                        (PaneType::Heatmap,     "Heatmap"),
                        (PaneType::Spreadsheet, "Spreadsheet"),
                    ] {
                        let active = chart.pane_type == ptype;
                        let fg = if active { t.accent } else { t.dim };
                        let bg = if active {
                            crate::chart_renderer::ui::style::color_alpha(t.accent, crate::chart_renderer::ui::style::alpha_tint())
                        } else {
                            egui::Color32::TRANSPARENT
                        };
                        if ui.add(egui::Button::new(
                            egui::RichText::new(label).size(crate::chart_renderer::ui::style::font_sm()).color(fg))
                            .fill(bg)
                            .corner_radius(4.0)
                        ).clicked() {
                            chart.pane_type = ptype;
                            chart.pane_template_name = None;
                        }
                    }
                });

                ui.separator();

                match chart.pane_type {
                    PaneType::Chart => {
                        // Template selector + save row + Stock/Option toggle
                        let template_names: Vec<String> = std::iter::once("None".to_string())
                            .chain(watchlist.pane_templates.iter().map(|(n, _)| n.clone()))
                            .collect();
                        let cur_template: String = chart.pane_template_name.clone().unwrap_or_else(|| "None".to_string());
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("Template:").size(crate::chart_renderer::ui::style::font_sm()).color(t.dim));
                            egui::ComboBox::from_id_salt(("pane_picker_template", pane_idx))
                                .selected_text(cur_template.as_str())
                                .show_ui(ui, |ui| {
                                    for name in &template_names {
                                        if ui.selectable_label(cur_template.as_str() == name.as_str(), name.as_str()).clicked() {
                                            chart.pane_template_name = if name == "None" { None } else { Some(name.clone()) };
                                        }
                                    }
                                });
                            // Stock | Option toggle — switches the body below between
                            // ticker search and option-chain selector.
                            let stock_active = !chart.pane_picker_option_mode;
                            let opt_active = chart.pane_picker_option_mode;
                            let stock_btn = ui.add(egui::Button::new(
                                egui::RichText::new("Stock").size(crate::chart_renderer::ui::style::font_sm())
                                    .color(if stock_active { t.accent } else { t.dim }))
                                .fill(if stock_active { crate::chart_renderer::ui::style::color_alpha(t.accent, crate::chart_renderer::ui::style::alpha_tint()) } else { egui::Color32::TRANSPARENT })
                                .corner_radius(4.0));
                            if stock_btn.clicked() { chart.pane_picker_option_mode = false; }
                            let opt_btn = ui.add(egui::Button::new(
                                egui::RichText::new("Option").size(crate::chart_renderer::ui::style::font_sm())
                                    .color(if opt_active { t.accent } else { t.dim }))
                                .fill(if opt_active { crate::chart_renderer::ui::style::color_alpha(t.accent, crate::chart_renderer::ui::style::alpha_tint()) } else { egui::Color32::TRANSPARENT })
                                .corner_radius(4.0));
                            if opt_btn.clicked() { chart.pane_picker_option_mode = true; }
                            ui.add(egui::TextEdit::singleline(&mut chart.pane_picker_save_name)
                                .hint_text("Name…").desired_width(110.0));
                            let can_save = !chart.pane_picker_save_name.trim().is_empty();
                            if ui.add_enabled(can_save, egui::Button::new(egui::RichText::new("Save").size(crate::chart_renderer::ui::style::font_sm()))).clicked() {
                                let name = chart.pane_picker_save_name.trim().to_string();
                                let indicators: Vec<serde_json::Value> = chart.indicators.iter().map(|ind| serde_json::json!({
                                    "kind": ind.kind.label(), "period": ind.period, "color": ind.color,
                                    "visible": ind.visible, "thickness": ind.thickness,
                                    "param2": ind.param2, "param3": ind.param3, "param4": ind.param4,
                                    "source": ind.source, "offset": ind.offset,
                                    "ob_level": ind.ob_level, "os_level": ind.os_level,
                                    "source_tf": ind.source_tf,
                                    "line_style": match ind.line_style { LineStyle::Solid => "solid", LineStyle::Dashed => "dashed", LineStyle::Dotted => "dotted" },
                                    "upper_color": ind.upper_color, "lower_color": ind.lower_color,
                                    "fill_color_hex": ind.fill_color_hex,
                                    "upper_thickness": ind.upper_thickness, "lower_thickness": ind.lower_thickness,
                                })).collect();
                                let tmpl = serde_json::json!({
                                    "show_volume": chart.show_volume, "show_oscillators": chart.show_oscillators,
                                    "ohlc_tooltip": chart.ohlc_tooltip, "magnet": chart.magnet, "log_scale": chart.log_scale,
                                    "show_vwap_bands": chart.show_vwap_bands, "show_cvd": chart.show_cvd,
                                    "show_delta_volume": chart.show_delta_volume, "show_rvol": chart.show_rvol,
                                    "show_ma_ribbon": chart.show_ma_ribbon, "show_prev_close": chart.show_prev_close,
                                    "show_auto_sr": chart.show_auto_sr, "show_auto_fib": chart.show_auto_fib,
                                    "show_footprint": chart.show_footprint, "show_gamma": chart.show_gamma,
                                    "show_darkpool": chart.show_darkpool, "show_events": chart.show_events,
                                    "hit_highlight": chart.hit_highlight, "show_pnl_curve": chart.show_pnl_curve,
                                    "show_pattern_labels": chart.show_pattern_labels,
                                    "candle_mode": match chart.candle_mode {
                                        CandleMode::Standard => "std", CandleMode::Violin => "vln",
                                        CandleMode::Gradient => "grd", CandleMode::ViolinGradient => "vg",
                                        CandleMode::HeikinAshi => "ha", CandleMode::Line => "line", CandleMode::Area => "area",
                                        CandleMode::Renko => "rnk", CandleMode::RangeBar => "rng", CandleMode::TickBar => "tck",
                                    },
                                    "indicators": indicators,
                                });
                                watchlist.pane_templates.push((name.clone(), tmpl));
                                save_templates(&watchlist.pane_templates);
                                chart.pane_template_name = Some(name);
                                chart.pane_picker_save_name.clear();
                            }
                        });
                        ui.add_space(4.0);

                        if !chart.pane_picker_option_mode {
                            // ── Stock body: ticker search + recents ──
                            let resp = ui.add(
                                egui::TextEdit::singleline(&mut chart.pane_picker_query)
                                    .hint_text("Search symbol…")
                                    .desired_width(f32::INFINITY)
                            );
                            if resp.changed()
                                && !chart.pane_picker_query.is_empty()
                                && chart.pane_picker_query != chart.picker_last_query
                            {
                                let q = chart.pane_picker_query.clone();
                                chart.picker_last_query = q.clone();
                                chart.picker_searching = true;
                                chart.picker_results.clear();
                                fetch_search_background(q, format!("pane_picker_{}", pane_idx));
                            }

                            if chart.pane_picker_query.is_empty() {
                                ui.add_space(4.0);
                                ui.label(egui::RichText::new("Recent").size(crate::chart_renderer::ui::style::font_sm()).color(t.dim));
                                let recents = chart.recent_symbols.clone();
                                for (sym, name) in &recents {
                                    ui.horizontal(|ui| {
                                        if ui.add(egui::Button::new(
                                            egui::RichText::new(sym).size(crate::chart_renderer::ui::style::font_sm()).color(t.text))
                                            .fill(egui::Color32::TRANSPARENT)
                                        ).clicked() {
                                            chart.pending_symbol_change = Some(sym.clone());
                                            fetch_bars_background(sym.clone(), chart.timeframe.clone());
                                            close_picker = true;
                                        }
                                        ui.label(egui::RichText::new(name).size(crate::chart_renderer::ui::style::font_sm()).color(t.dim));
                                    });
                                }
                            } else {
                                let results = chart.picker_results.clone();
                                for (sym, name, _exch) in &results {
                                    ui.horizontal(|ui| {
                                        if ui.add(egui::Button::new(
                                            egui::RichText::new(sym).size(crate::chart_renderer::ui::style::font_sm()).color(t.text))
                                            .fill(egui::Color32::TRANSPARENT)
                                        ).clicked() {
                                            chart.pending_symbol_change = Some(sym.clone());
                                            fetch_bars_background(sym.clone(), chart.timeframe.clone());
                                            close_picker = true;
                                        }
                                        ui.label(egui::RichText::new(name).size(crate::chart_renderer::ui::style::font_sm()).color(t.dim));
                                    });
                                }
                            }
                        } else {
                            // ── Option body: DTE nav + chain table ──
                            // Reuses watchlist.chain_0dte / chain_far data, same source
                            // as the floating option_quick_picker.
                            const DTE_LIST: &[i32] = &[0, 1, 2, 3, 7, 14, 30, 60];
                            let dte_label = |dte: i32| -> String {
                                match dte {
                                    0 => "0DTE".into(),
                                    1 => "1D".into(),
                                    d if d < 7 => format!("{}D", d),
                                    7 => "1W".into(),
                                    14 => "2W".into(),
                                    30 => "1M".into(),
                                    60 => "2M".into(),
                                    d => format!("{}D", d),
                                }
                            };
                            // Underlying = pane's underlying when in option mode, else current symbol.
                            let underlying = if !chart.underlying.is_empty() {
                                chart.underlying.clone()
                            } else {
                                chart.symbol.clone()
                            };
                            let spot = chart.bars.last().map(|b| b.close).unwrap_or(0.0);
                            let dte_idx = chart.option_quick_dte_idx.min(DTE_LIST.len() - 1);
                            let current_dte = DTE_LIST[dte_idx];

                            // Underlying display + DTE nav
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new(&underlying)
                                    .monospace().size(crate::chart_renderer::ui::style::font_lg()).strong().color(t.accent));
                                if spot > 0.0 {
                                    ui.label(egui::RichText::new(format!("@ {:.2}", spot))
                                        .monospace().size(crate::chart_renderer::ui::style::font_sm()).color(t.dim));
                                }
                            });
                            ui.horizontal(|ui| {
                                let can_back = dte_idx > 0;
                                let back_col = if can_back { t.accent } else { t.dim.gamma_multiply(0.3) };
                                if ui.add(egui::Button::new(egui::RichText::new(crate::ui_kit::icons::Icon::CARET_LEFT)
                                    .size(crate::chart_renderer::ui::style::font_lg()).color(back_col))
                                    .fill(egui::Color32::TRANSPARENT)
                                ).clicked() && can_back {
                                    chart.option_quick_dte_idx = dte_idx - 1;
                                    let new_dte = DTE_LIST[dte_idx - 1];
                                    fetch_chain_background(underlying.clone(), 15, new_dte, spot);
                                }
                                ui.label(egui::RichText::new(dte_label(current_dte))
                                    .monospace().size(crate::chart_renderer::ui::style::font_lg()).strong().color(t.text));
                                let can_fwd = dte_idx < DTE_LIST.len() - 1;
                                let fwd_col = if can_fwd { t.accent } else { t.dim.gamma_multiply(0.3) };
                                if ui.add(egui::Button::new(egui::RichText::new(crate::ui_kit::icons::Icon::CARET_RIGHT)
                                    .size(crate::chart_renderer::ui::style::font_lg()).color(fwd_col))
                                    .fill(egui::Color32::TRANSPARENT)
                                ).clicked() && can_fwd {
                                    chart.option_quick_dte_idx = dte_idx + 1;
                                    let new_dte = DTE_LIST[dte_idx + 1];
                                    fetch_chain_background(underlying.clone(), 15, new_dte, spot);
                                }
                            });

                            // Refresh chain data if DTE changed
                            if current_dte != 0 && watchlist.chain_far_dte != current_dte {
                                watchlist.chain_far_dte = current_dte;
                                fetch_chain_background(underlying.clone(), 15, current_dte, spot);
                            }

                            // Chain table
                            let chain_ref = if current_dte == 0 { &watchlist.chain_0dte } else { &watchlist.chain_far };
                            let (calls, puts) = (&chain_ref.0, &chain_ref.1);
                            let mut pending_load: Option<(f32, bool)> = None;

                            // Quick prev/next strike navigation — only meaningful when
                            // the pane is already on an option contract.
                            let cur_strike = chart.option_strike;
                            let cur_is_call = chart.option_type == "C";
                            if chart.is_option && cur_strike > 0.0 {
                                ui.add_space(4.0);
                                ui.horizontal(|ui| {
                                    let half_w = 150.0;
                                    let prev_btn = ui.add_sized(
                                        egui::vec2(half_w, 22.0),
                                        egui::Button::new(egui::RichText::new(
                                            format!("{} Prev Strike", crate::ui_kit::icons::Icon::CARET_LEFT))
                                            .monospace().size(crate::chart_renderer::ui::style::font_sm())
                                            .color(t.text))
                                            .fill(crate::chart_renderer::ui::style::color_alpha(t.toolbar_border, crate::chart_renderer::ui::style::alpha_subtle()))
                                            .corner_radius(4.0),
                                    );
                                    if prev_btn.clicked() {
                                        let rows = if cur_is_call { calls } else { puts };
                                        let mut sorted: Vec<f32> = rows.iter().map(|r| r.strike).collect();
                                        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
                                        if let Some(&lower) = sorted.iter().rev().find(|&&s| s < cur_strike) {
                                            pending_load = Some((lower, cur_is_call));
                                        }
                                    }
                                    let next_btn = ui.add_sized(
                                        egui::vec2(half_w, 22.0),
                                        egui::Button::new(egui::RichText::new(
                                            format!("Next Strike {}", crate::ui_kit::icons::Icon::CARET_RIGHT))
                                            .monospace().size(crate::chart_renderer::ui::style::font_sm())
                                            .color(t.text))
                                            .fill(crate::chart_renderer::ui::style::color_alpha(t.toolbar_border, crate::chart_renderer::ui::style::alpha_subtle()))
                                            .corner_radius(4.0),
                                    );
                                    if next_btn.clicked() {
                                        let rows = if cur_is_call { calls } else { puts };
                                        let mut sorted: Vec<f32> = rows.iter().map(|r| r.strike).collect();
                                        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
                                        if let Some(&higher) = sorted.iter().find(|&&s| s > cur_strike) {
                                            pending_load = Some((higher, cur_is_call));
                                        }
                                    }
                                });
                                ui.add_space(4.0);
                            }

                            ui.add_space(4.0);
                            ui.horizontal(|ui| {
                                let cw = 110.0;
                                ui.add_sized(egui::vec2(cw, 16.0), egui::Label::new(
                                    egui::RichText::new("CALL").monospace().size(crate::chart_renderer::ui::style::font_xs()).color(t.dim.gamma_multiply(0.6))));
                                ui.add_sized(egui::vec2(cw, 16.0), egui::Label::new(
                                    egui::RichText::new("STRIKE").monospace().size(crate::chart_renderer::ui::style::font_xs()).color(t.dim.gamma_multiply(0.6))));
                                ui.add_sized(egui::vec2(cw, 16.0), egui::Label::new(
                                    egui::RichText::new("PUT").monospace().size(crate::chart_renderer::ui::style::font_xs()).color(t.dim.gamma_multiply(0.6))));
                            });

                            if calls.is_empty() && puts.is_empty() {
                                ui.add_space(8.0);
                                ui.vertical_centered(|ui| {
                                    ui.label(egui::RichText::new("Loading chain…")
                                        .monospace().size(crate::chart_renderer::ui::style::font_sm()).color(t.dim));
                                });
                            } else {
                                let mut strikes: Vec<f32> = calls.iter().map(|r| r.strike)
                                    .chain(puts.iter().map(|r| r.strike))
                                    .collect();
                                strikes.sort_by(|a, b| a.partial_cmp(b).unwrap());
                                strikes.dedup_by(|a, b| (*a - *b).abs() < 0.01);

                                egui::ScrollArea::vertical()
                                    .id_salt(("pane_picker_chain", pane_idx))
                                    .max_height(260.0)
                                    .show(ui, |ui| {
                                        for strike in &strikes {
                                            let call_row = calls.iter().find(|r| (r.strike - strike).abs() < 0.01);
                                            let put_row  = puts.iter().find(|r| (r.strike - strike).abs() < 0.01);
                                            let is_atm = (strike - spot).abs() < (spot * 0.005).max(0.5);
                                            ui.horizontal(|ui| {
                                                let cw = 110.0;
                                                let call_text = call_row.map(|r| format!("{:.2}", r.bid)).unwrap_or_else(|| "-".into());
                                                let call_btn = ui.add_sized(egui::vec2(cw, 18.0), egui::Button::new(
                                                    egui::RichText::new(&call_text).monospace().size(crate::chart_renderer::ui::style::font_sm())
                                                        .color(if call_row.is_some() { t.bull } else { t.dim.gamma_multiply(0.4) }))
                                                    .fill(egui::Color32::TRANSPARENT));
                                                if call_btn.clicked() && call_row.is_some() { pending_load = Some((*strike, true)); }

                                                let strike_txt = if (strike - strike.round()).abs() < 0.005 { format!("{:.0}", strike) } else { format!("{:.1}", strike) };
                                                ui.add_sized(egui::vec2(cw, 18.0), egui::Label::new(
                                                    egui::RichText::new(strike_txt).monospace().size(crate::chart_renderer::ui::style::font_sm())
                                                        .color(if is_atm { t.accent } else { t.text })));

                                                let put_text = put_row.map(|r| format!("{:.2}", r.bid)).unwrap_or_else(|| "-".into());
                                                let put_btn = ui.add_sized(egui::vec2(cw, 18.0), egui::Button::new(
                                                    egui::RichText::new(&put_text).monospace().size(crate::chart_renderer::ui::style::font_sm())
                                                        .color(if put_row.is_some() { t.bear } else { t.dim.gamma_multiply(0.4) }))
                                                    .fill(egui::Color32::TRANSPARENT));
                                                if put_btn.clicked() && put_row.is_some() { pending_load = Some((*strike, false)); }
                                            });
                                        }
                                    });
                            }

                            // Apply selected contract
                            if let Some((strike, is_call)) = pending_load {
                                let rows = if current_dte == 0 {
                                    if is_call { &watchlist.chain_0dte.0 } else { &watchlist.chain_0dte.1 }
                                } else if is_call { &watchlist.chain_far.0 } else { &watchlist.chain_far.1 };
                                let occ = rows.iter()
                                    .find(|r| (r.strike - strike).abs() < 0.01)
                                    .map(|r| r.contract.clone())
                                    .unwrap_or_default();
                                if chart.is_option {
                                    let occ_final = if occ.starts_with("O:") { occ.clone() }
                                        else { synthesize_occ(&underlying, strike, is_call, "") };
                                    let strike_str = if (strike - strike.round()).abs() < 0.005 { format!("{:.0}", strike) } else { format!("{:.1}", strike) };
                                    let opt_sym = format!("{} {}{}", underlying, strike_str, if is_call { "C" } else { "P" });
                                    chart.symbol = opt_sym.clone();
                                    chart.option_type = if is_call { "C".into() } else { "P".into() };
                                    chart.option_strike = strike;
                                    chart.option_contract = occ_final.clone();
                                    chart.bars.clear();
                                    chart.timestamps.clear();
                                    let tf = chart.timeframe.clone();
                                    if !occ_final.is_empty() && crate::apex_data::is_enabled() {
                                        let mark = chart.bar_source_mark;
                                        fetch_option_bars_background(occ_final, opt_sym, tf, mark);
                                    }
                                } else {
                                    watchlist.pending_opt_chart = Some((underlying.clone(), strike, is_call, String::new()));
                                    watchlist.pending_opt_chart_contract = Some(occ);
                                }
                                close_picker = true;
                            }
                        }
                    }
                    other => {
                        // Portfolio / Dashboard / Heatmap / Spreadsheet — all share
                        // the same template-row UI (selector + name input + save).
                        // Only the underlying templates Vec differs per mode.
                        let (id_salt, templates_ref): (&str, &mut Vec<String>) = match other {
                            PaneType::Portfolio   => ("pane_picker_portfolio_tpl",   &mut watchlist.portfolio_templates),
                            PaneType::Dashboard   => ("pane_picker_dashboard_tpl",   &mut watchlist.dashboard_templates),
                            PaneType::Heatmap     => ("pane_picker_heatmap_tpl",     &mut watchlist.heatmap_templates),
                            PaneType::Spreadsheet => ("pane_picker_spreadsheet_tpl", &mut watchlist.spreadsheet_templates),
                            PaneType::Chart       => unreachable!(),
                        };
                        let cur: String = chart.pane_template_name.clone().unwrap_or_else(|| "Default".to_string());
                        let templates_snapshot = templates_ref.clone();
                        let mut newly_selected: Option<String> = None;
                        let mut do_save = false;
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("Template:").size(crate::chart_renderer::ui::style::font_sm()).color(t.dim));
                            egui::ComboBox::from_id_salt((id_salt, pane_idx))
                                .selected_text(cur.as_str())
                                .show_ui(ui, |ui| {
                                    for name in &templates_snapshot {
                                        if ui.selectable_label(cur.as_str() == name.as_str(), name.as_str()).clicked() {
                                            newly_selected = Some(name.clone());
                                        }
                                    }
                                });
                            ui.add(egui::TextEdit::singleline(&mut chart.pane_picker_save_name)
                                .hint_text("Name…").desired_width(110.0));
                            let can_save = !chart.pane_picker_save_name.trim().is_empty();
                            if ui.add_enabled(can_save, egui::Button::new(egui::RichText::new("Save").size(crate::chart_renderer::ui::style::font_sm()))).clicked() {
                                do_save = true;
                            }
                        });
                        if let Some(name) = newly_selected {
                            chart.pane_template_name = Some(name);
                            close_picker = true;
                        }
                        if do_save {
                            let name = chart.pane_picker_save_name.trim().to_string();
                            if !templates_ref.iter().any(|n| n == &name) {
                                templates_ref.push(name.clone());
                            }
                            chart.pane_template_name = Some(name);
                            chart.pane_picker_save_name.clear();
                        }
                    }
                }

                ui.add_space(4.0);
                if ui.add(egui::Button::new(egui::RichText::new("Close").size(crate::chart_renderer::ui::style::font_sm()).color(t.dim))
                    .fill(egui::Color32::TRANSPARENT)
                ).clicked() {
                    close_picker = true;
                }
            });

        if close_picker { chart.pane_picker_open = false; }

        // Close on Escape
        if ui.ctx().input(|i| i.key_pressed(egui::Key::Escape)) {
            chart.pane_picker_open = false;
        }
    }

    // ── DOM Sidebar (left side of pane) ─────────────────────────────────────
    let dom_w = if chart.dom_sidebar_open { chart.dom_width } else { 0.0 };
    let full_rect = egui::Rect::from_min_size(
        egui::pos2(pane_rect.left(), pane_rect.top() + pane_top_offset),
        egui::vec2(pane_rect.width(), pane_rect.height() - pane_top_offset),
    );
    if chart.dom_sidebar_open {
        let dom_rect = egui::Rect::from_min_size(full_rect.min, egui::vec2(dom_w, full_rect.height()));
        let current_price = chart.bars.last().map(|b| b.close).unwrap_or(100.0);
        // Auto-detect tick size based on symbol
        let is_index = chart.symbol == "SPX" || chart.symbol == "NDX" || chart.symbol == "DJI" || chart.symbol == "RUT";
        if chart.dom_tick_size < 0.001 || (is_index && chart.dom_tick_size < 0.5) {
            chart.dom_tick_size = if is_index { 1.0 } else { 0.01 };
        }
        // Auto-center on current price if center_price is 0 (first open)
        if chart.dom_center_price == 0.0 {
            chart.dom_center_price = (current_price / chart.dom_tick_size).round() * chart.dom_tick_size;
        }
        // Generate mock levels if empty or stale
        if chart.dom_levels.is_empty() || (chart.dom_levels.first().map(|l| (l.price - chart.dom_center_price).abs() > chart.dom_tick_size * 40.0).unwrap_or(true)) {
            chart.dom_levels = crate::chart_renderer::ui::panels::dom_panel::generate_mock_levels(chart.dom_center_price, chart.dom_tick_size, 30);
        }
        // Sync OrderManager armed state
        crate::chart_renderer::trading::order_manager::set_armed(chart.dom_armed);
        // Feed DOM with orders from both local chart.orders AND OrderManager
        let mgr_orders = crate::chart_renderer::trading::order_manager::active_orders_for(&chart.symbol);
        let mut combined_orders = chart.orders.clone();
        for mo in &mgr_orders {
            if !combined_orders.iter().any(|o| o.id == mo.id) { combined_orders.push(mo.clone()); }
        }

        let mut dom_new_order: Option<(OrderSide, f32, u32)> = None;
        let mut dom_cancel_all = false;
        let mut dom_cancel_order_id: Option<u32> = None;
        let mut dom_move_order: Option<(u32, f32)> = None;
        {
            use crate::chart_renderer::ui::pane::{Pane as _, PaneContext, DomPaneAdapter};
            let mut adapter = DomPaneAdapter {
                dom_rect,
                current_price,
                levels: &chart.dom_levels,
                tick_size: chart.dom_tick_size,
                center_price: &mut chart.dom_center_price,
                dom_width: &mut chart.dom_width,
                orders: &combined_orders,
                dom_selected_price: &mut chart.dom_selected_price,
                dom_order_type: &mut chart.dom_order_type,
                order_qty: &mut chart.order_qty,
                new_order: &mut dom_new_order,
                cancel_all: &mut dom_cancel_all,
                cancel_order_id: &mut dom_cancel_order_id,
                move_order: &mut dom_move_order,
                dom_armed: &mut chart.dom_armed,
                dom_col_mode: &mut chart.dom_col_mode,
                dom_dragging: &mut chart.dom_dragging,
            };
            // DomPaneAdapter does not read PaneContext::panes; we pass an
            // empty slice to avoid a second mutable borrow of `panes` while
            // chart fields are already borrowed via the adapter.
            let dummy_rects = [dom_rect];
            let mut cx = PaneContext {
                theme: t,
                panes: &mut [],
                pane_idx,
                active_pane,
                pane_rects: &dummy_rects,
                watchlist,
            };
            adapter.render(ui, ctx, &mut cx);
        }

        // Process DOM order actions through OrderManager
        if let Some((side, price, qty)) = dom_new_order {
            use crate::chart_renderer::trading::order_manager::*;
            let ot = if chart.dom_order_type == crate::chart_renderer::ui::panels::dom_panel::DomOrderType::Market {
                ManagedOrderType::Market
            } else {
                ManagedOrderType::Limit
            };
            let result = submit_order(OrderIntent {
                symbol: chart.symbol.clone(), side, order_type: ot, price, qty,
                source: OrderSource::DomLadder, pair_with: None,
                option_symbol: None, option_con_id: None, stop_price: 0.0, trail_amount: None, trail_percent: None, last_price: 0.0, tif: chart.order_tif_idx as u8, outside_rth: chart.order_outside_rth,
            });
            match result {
                OrderResult::Accepted(id) => {
                    // Also add to local chart.orders for rendering compat (transitional)
                    chart.orders.push(OrderLevel { id: id as u32, side, price, qty, status: OrderStatus::Placed, pair_id: None, option_symbol: None, option_con_id: None, trail_amount: None, trail_percent: None });
                }
                OrderResult::NeedsConfirmation(id) => {
                    chart.orders.push(OrderLevel { id: id as u32, side, price, qty, status: OrderStatus::Draft, pair_id: None, option_symbol: None, option_con_id: None, trail_amount: None, trail_percent: None });
                }
                OrderResult::Duplicate => { /* silently blocked */ }
                OrderResult::Rejected(reason) => {
                    eprintln!("[order-manager] Rejected: {}", reason);
                }
            }
        }
        if dom_cancel_all {
            crate::chart_renderer::trading::order_manager::cancel_all_orders(&chart.symbol);
            for o in &mut chart.orders {
                if o.status == OrderStatus::Draft || o.status == OrderStatus::Placed {
                    o.status = OrderStatus::Cancelled;
                }
            }
        }
        if let Some(cid) = dom_cancel_order_id {
            crate::chart_renderer::trading::order_manager::cancel_order(cid as u64);
            cancel_order_with_pair(&mut chart.orders, cid);
        }
        if let Some((oid, new_price)) = dom_move_order {
            crate::chart_renderer::trading::order_manager::modify_order_price(oid as u64, new_price);
            if let Some(o) = chart.orders.iter_mut().find(|o| o.id == oid) {
                o.price = new_price;
            }
        }
    }
    let rect = egui::Rect::from_min_size(
        egui::pos2(full_rect.left() + dom_w, full_rect.top()),
        egui::vec2(full_rect.width() - dom_w, full_rect.height()),
    );

    // ── Non-chart pane types: render their content in the body area, then return ──
    // The header/tabs above have already rendered, so these panes get the full header UX
    // We pass `rect` (body below header, minus DOM) as a single-element slice
    match chart.pane_type {
        PaneType::Portfolio => {
            let body_rects = [rect];
            // Migrated to Pane trait — proof-of-concept call site.
            use crate::chart_renderer::ui::pane::{Pane as _, PaneContext, PortfolioPaneAdapter};
            let mut adapter = PortfolioPaneAdapter { account_data: account_data_cached, theme_idx };
            let mut cx = PaneContext {
                theme: &THEMES[theme_idx],
                panes,
                pane_idx,
                active_pane,
                pane_rects: &body_rects,
                watchlist,
            };
            adapter.render(ui, ctx, &mut cx);
            return;
        }
        PaneType::Dashboard => {
            let body_rects = [rect];
            crate::chart_renderer::ui::panels::dashboard_pane::render(ui, ctx, panes, pane_idx, active_pane, 1, &body_rects, theme_idx, watchlist);
            return;
        }
        PaneType::Heatmap => {
            let body_rects = [rect];
            crate::chart_renderer::ui::panels::heatmap_pane::render(ui, ctx, panes, pane_idx, active_pane, 1, &body_rects, theme_idx, watchlist);
            return;
        }
        PaneType::Spreadsheet => {
            let body_rects = [rect];
            use crate::chart_renderer::ui::pane::{Pane as _, PaneContext, SpreadsheetPaneAdapter};
            let mut adapter = SpreadsheetPaneAdapter { theme_idx };
            let mut cx = PaneContext {
                theme: &THEMES[theme_idx],
                panes,
                pane_idx,
                active_pane,
                pane_rects: &body_rects,
                watchlist,
            };
            adapter.render(ui, ctx, &mut cx);
            return;
        }
        PaneType::Chart => {} // continue to chart rendering below
    }

    // Shared X-axis: detect if this pane has a bottom neighbor (skip X labels on upper panes)
    let pane_has_bottom_neighbor = visible_count > 1 && pane_rects.iter().any(|r| (r.top() - pane_rect.bottom()).abs() < 5.0);
    let pane_has_right_neighbor = visible_count > 1 && pane_rects.iter().any(|r| (r.left() - pane_rect.right()).abs() < 5.0);
    let skip_x_labels = watchlist.shared_x_axis && pane_has_bottom_neighbor;
    let skip_y_axis = watchlist.shared_y_axis && pane_has_right_neighbor;
    let (w,h) = (rect.width(), rect.height());
    let pr = if !watchlist.show_y_axis || skip_y_axis { 0.0_f32 } else { crate::dt_f32!(chart.padding_right, 80.0) * 0.525 };
    let pt = if watchlist.compact_mode { 1.0_f32 } else { crate::dt_f32!(chart.padding_top, 4.0) };
    let pb = crate::dt_f32!(chart.padding_bottom, 30.0) * 0.0; // scaled — 0 by default, token controls base
    // Reserve space for oscillator sub-panel if any oscillator indicators or CVD is active
    let has_oscillators = chart.show_oscillators && chart.indicators.iter().any(|i| i.visible && i.kind.category() == IndicatorCategory::Oscillator);
    let needs_osc_panel = has_oscillators || chart.show_cvd;
    let osc_h = if needs_osc_panel { (h * 0.22).min(120.0) } else { 0.0 };
    let (cw,ch) = (w-pr, h-pt-pb-osc_h);
    if cw<=0.0 || ch<=0.0 { return; }
    if n==0 {
        // ── Refined loading indicator ──
        if !chart.symbol.is_empty() {
            let center = egui::pos2(rect.left() + cw / 2.0, rect.top() + pt + ch / 2.0);
            let lp = ui.painter();
            crate::chart_renderer::ui::chart_widgets::draw_refined_spinner(lp, center, 14.0, t.accent);
            let time = ui.ctx().input(|i| i.time);
            let text_alpha = (110.0 + 30.0 * (time * 1.4).sin()) as u8;
            lp.text(egui::pos2(center.x, center.y + 28.0), egui::Align2::CENTER_CENTER,
                &format!("{} {}", chart.symbol, chart.timeframe),
                egui::FontId::monospace(10.0), color_alpha(t.dim, text_alpha));
            ui.ctx().request_repaint();
        }
        // ── Option-pane MARK toggle (visible even while bars are loading) ──
        if chart.is_option {
            let pad = 6.0_f32;
            let bar_h = 18.0_f32;
            let y = rect.top() + pt + pad;
            let mut x = rect.left() + pad;
            let p = ui.painter_at(rect);
            // TF pill
            if !chart.timeframe.is_empty() {
                let tf = chart.timeframe.to_uppercase();
                let font = egui::FontId::monospace(10.0);
                let g = p.layout_no_wrap(tf.clone(), font.clone(), t.text);
                let w = g.size().x + 10.0;
                let r = egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(w, bar_h));
                p.rect_filled(r, 3.0, t.bg.gamma_multiply(0.4));
                p.text(r.center(), egui::Align2::CENTER_CENTER, &tf, font, t.text);
                x += w + 4.0;
            }
            // LAST | MARK segmented
            let font = egui::FontId::monospace(10.0);
            let parts = [("LAST", false), ("MARK", true)];
            let part_w = 36.0_f32;
            let total_w = part_w * parts.len() as f32;
            let outer = egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(total_w, bar_h));
            p.rect_filled(outer, 3.0, t.bg.gamma_multiply(0.4));
            p.rect_stroke(outer, 3.0, egui::Stroke::new(0.5, t.toolbar_border), egui::StrokeKind::Inside);
            let mark_color = t.bear;
            for (idx, (label, is_mark)) in parts.iter().enumerate() {
                let r = egui::Rect::from_min_size(
                    egui::pos2(x + part_w * idx as f32, y), egui::vec2(part_w, bar_h));
                let resp = ui.allocate_rect(r, egui::Sense::click());
                let active = chart.bar_source_mark == *is_mark;
                let hovered = resp.hovered();
                let bg_col = if active {
                    if *is_mark { color_alpha(mark_color, 70) } else { color_alpha(t.accent, 60) }
                } else if hovered {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    t.bg.gamma_multiply(0.55)
                } else { egui::Color32::TRANSPARENT };
                let fg_col = if active {
                    if *is_mark { mark_color } else { t.accent }
                } else { t.dim.gamma_multiply(0.95) };
                if bg_col != egui::Color32::TRANSPARENT { p.rect_filled(r, 3.0, bg_col); }
                p.text(r.center(), egui::Align2::CENTER_CENTER, *label, font.clone(), fg_col);
                if resp.clicked() && chart.bar_source_mark != *is_mark {
                    chart.bar_source_mark = *is_mark;
                    let occ = chart.option_contract.clone();
                    let display_sym = chart.symbol.clone();
                    let tf = chart.timeframe.clone();
                    if !occ.is_empty() && crate::apex_data::is_enabled() {
                        fetch_option_bars_background(occ, display_sym, tf, *is_mark);
                    }
                }
            }
        }
        // Empty / loading panes still need to be selectable — without this the
        // bar-rendering early-return skips the interaction code that sets
        // active_pane, so the user can never click into a blank pane.
        if visible_count > 1 {
            let body = egui::Rect::from_min_size(
                egui::pos2(rect.left(), rect.top() + pt),
                egui::vec2(cw + pr, ch),
            );
            let resp = ui.allocate_rect(body, egui::Sense::click());
            if resp.clicked() { *active_pane = pane_idx; }
        }
        return;
    }

    // Only set cursors for the pane the pointer is actually over
    let pointer_in_pane = ui.input(|i| i.pointer.hover_pos()).map_or(false, |p| pane_rect.contains(p));

    // ── Smooth zoom: lerp vc toward vc_target ──
    if chart.vc != chart.vc_target {
        let diff = chart.vc_target as f32 - chart.vc as f32;
        let step = (diff * 0.5).abs().max(1.0).min(diff.abs()) * diff.signum();
        chart.vc = (chart.vc as f32 + step) as u32;
        // Update vs to keep center point stable during smooth zoom
        let center = chart.vs + chart.vc as f32 * 0.5;
        chart.vs = (center - chart.vc as f32 * 0.5).max(0.0);
        ctx.request_repaint();
    }

    compute_volume_analytics(chart);

    let (target_min, target_max) = chart.price_range();
    let (min_p, max_p) = if chart.price_lock.is_none() {
        if let Some((cur_min, cur_max)) = chart.price_range_animated {
            let lerp = 0.55_f32;
            let new_min = cur_min + (target_min - cur_min) * lerp;
            let new_max = cur_max + (target_max - cur_max) * lerp;
            if (new_min - target_min).abs() < 0.001 && (new_max - target_max).abs() < 0.001 {
                chart.price_range_animated = Some((target_min, target_max));
                (target_min, target_max)
            } else {
                chart.price_range_animated = Some((new_min, new_max));
                ctx.request_repaint();
                (new_min, new_max)
            }
        } else {
            chart.price_range_animated = Some((target_min, target_max));
            (target_min, target_max)
        }
    } else {
        // When price is locked (manual pan), skip animation
        chart.price_range_animated = None;
        (target_min, target_max)
    };
    // Proportional right padding: ~8% of visible bars, min 5, max 30
    let dynamic_pad = ((chart.vc as f32 * 0.08) as u32).max(5).min(30);
    let total = chart.vc + dynamic_pad;
    let bs = cw/total as f32;
    let vs = chart.vs;
    // Render bars for full screen width (vc + padding) so bars fill to the edge
    let end = ((vs as u32) + chart.vc + dynamic_pad).min(n as u32);
    let frac = vs-vs.floor();
    let off = frac*bs;

    let log_scale = chart.log_scale;
    let py = |p:f32| -> f32 {
        if log_scale && p > 0.0 && min_p > 0.0 {
            let log_min = min_p.ln();
            let log_max = max_p.ln();
            let log_range = log_max - log_min;
            if log_range.abs() < 0.0001 { return rect.top() + pt + ch * 0.5; }
            rect.top() + pt + (log_max - p.ln()) / log_range * ch
        } else {
            rect.top()+pt+(max_p-p)/(max_p-min_p)*ch
        }
    };
    let py_inv = |y:f32| -> f32 {
        if log_scale && min_p > 0.0 {
            let log_min = min_p.ln();
            let log_max = max_p.ln();
            let log_val = log_max - (y - rect.top() - pt) / ch * (log_max - log_min);
            log_val.exp()
        } else {
            max_p - (y - rect.top() - pt) / ch * (max_p - min_p)
        }
    };
    let bx = |i:f32| rect.left()+(i-vs)*bs+bs*0.5-off;
    let last_price = chart.bars.last().map(|b| b.close).unwrap_or(0.0);
    let painter = ui.painter_at(rect);

    // Grid + price labels
    let rng=max_p-min_p; let rs=rng/8.0; let mg=10.0_f32.powf(rs.log10().floor());
    let ns=[1.0,2.0,2.5,5.0,10.0]; let step=ns.iter().map(|&s|s*mg).find(|&s|s>=rs).unwrap_or(rs);
    let mut p=(min_p/step).ceil()*step;
    while p<=max_p { let y=py(p);
        painter.line_segment([egui::pos2(rect.left(),y),egui::pos2(rect.left()+cw,y)], egui::Stroke::new(0.5,t.dim.gamma_multiply(0.3)));
        if watchlist.show_y_axis {
            chart.fmt_buf.clear(); let _ = write!(chart.fmt_buf, "{:.2}", p);
            let f = egui::FontId::monospace(11.5);
            // poor-man's bold: 0.5px x-offset double-draw
            painter.text(egui::pos2(rect.left()+cw+3.5,y),egui::Align2::LEFT_CENTER,&chart.fmt_buf,f.clone(),t.text);
            painter.text(egui::pos2(rect.left()+cw+3.0,y),egui::Align2::LEFT_CENTER,&chart.fmt_buf,f,t.text);
        }
        p+=step;
    }

    // Current price indicator — faint horizontal level line + Y-axis badge
    if last_price > 0.0 {
        let price_y = py(last_price);
        if price_y.is_finite() && price_y >= rect.top() + pt && price_y <= rect.top() + pt + ch {
            // Direction color from day change
            let price_col = if let Some(first) = chart.bars.first() {
                if first.open > 0.0 && last_price >= first.open { t.bull } else { t.bear }
            } else { t.dim };

            // Faint horizontal level line spanning the chart — very subtle dashed
            let line_col = color_alpha(price_col, 28);
            let mut dx = rect.left();
            while dx < rect.left() + cw {
                let end_x = (dx + 3.0).min(rect.left() + cw);
                painter.line_segment(
                    [egui::pos2(dx, price_y), egui::pos2(end_x, price_y)],
                    egui::Stroke::new(0.5, line_col));
                dx += 10.0;
            }

            // Y-axis price badge (only if axis is visible) — dark text on colored fill
            if watchlist.show_y_axis {
                let price_text = format!("{:.2}", last_price);
                let badge_font = egui::FontId::monospace(13.0);
                // Dark foreground derived from the price color — high contrast but tinted
                let fg_col = egui::Color32::from_rgb(
                    (price_col.r() as f32 * 0.15) as u8,
                    (price_col.g() as f32 * 0.15) as u8,
                    (price_col.b() as f32 * 0.15) as u8);
                let galley = painter.layout_no_wrap(price_text.clone(), badge_font.clone(), fg_col);
                let pad_x = 4.0;
                let pad_y = 2.0;
                let badge_w = galley.size().x + pad_x * 2.0;
                let badge_h = galley.size().y + pad_y * 2.0;
                let badge_rect = egui::Rect::from_min_size(
                    egui::pos2(rect.left() + cw + 1.0, price_y - badge_h / 2.0),
                    egui::vec2(badge_w, badge_h));
                // Solid colored badge
                painter.rect_filled(badge_rect, 2.0, price_col);
                // Tiny left arrow/triangle pointing at the chart
                painter.add(egui::Shape::convex_polygon(
                    vec![
                        egui::pos2(badge_rect.left(), price_y - 3.0),
                        egui::pos2(badge_rect.left() - 4.0, price_y),
                        egui::pos2(badge_rect.left(), price_y + 3.0),
                    ],
                    price_col,
                    egui::Stroke::NONE));
                // Bolder via 0.5px x-offset double-draw
                painter.text(
                    egui::pos2(badge_rect.left() + pad_x + 0.5, price_y),
                    egui::Align2::LEFT_CENTER,
                    &price_text, badge_font.clone(), fg_col);
                painter.text(
                    egui::pos2(badge_rect.left() + pad_x, price_y),
                    egui::Align2::LEFT_CENTER,
                    &price_text, badge_font, fg_col);
            }
        }
    }

    // Time labels on bottom axis (extends into future beyond last bar)
    // When shared_x_axis is on and this pane has a bottom neighbor, skip X labels
    if watchlist.show_x_axis && !skip_x_labels && !chart.timestamps.is_empty() && end > vs as u32 {
        let candle_sec = if chart.timestamps.len() > 1 { (chart.timestamps[1] - chart.timestamps[0]).max(60) } else { 86400 };
        let nice_int: &[i64] = &[60,300,900,1800,3600,7200,14400,28800,86400,172800,604800,2592000];
        let min_label_px = 70.0;
        let bars_per_label = (min_label_px / bs).ceil() as i64;
        let min_interval = bars_per_label * candle_sec;
        let time_interval = nice_int.iter().copied().find(|&i| i >= min_interval).unwrap_or(86400);

        if let Some(&first_ts) = chart.timestamps.get(vs as usize) {
            let first_label = ((first_ts / time_interval) + 1) * time_interval;
            let mut ti = first_label;
            // Extend labels 200 bars into the future beyond the last bar
            let last_real_ts = chart.timestamps.last().copied().unwrap_or(first_ts);
            let future_ts = last_real_ts + candle_sec * 200;
            while ti <= future_ts {
                // For timestamps within real data, use partition_point; for future, extrapolate
                let bar_f = if ti <= last_real_ts {
                    chart.timestamps.partition_point(|&ts| ts < ti) as f32
                } else {
                    n as f32 + ((ti - last_real_ts) as f32 / candle_sec as f32)
                };
                let x = bx(bar_f);
                if x > rect.left() + 20.0 && x < rect.left() + cw - 10.0 {
                    chart.fmt_buf.clear();
                    if time_interval >= 86400 {
                        let days = (ti / 86400) as i32; let y2k = days - 10957;
                        let month = ((y2k % 365) / 30 + 1).min(12).max(1);
                        let day = ((y2k % 365) % 30 + 1).min(31).max(1);
                        let _ = write!(chart.fmt_buf, "{:02}/{:02}", month, day);
                    } else {
                        let h = ((ti % 86400) / 3600) as u32;
                        let m = ((ti % 3600) / 60) as u32;
                        let _ = write!(chart.fmt_buf, "{:02}:{:02}", h, m);
                    };
                    let y = rect.top() + pt + ch - 10.0;
                    painter.text(egui::pos2(x, y), egui::Align2::CENTER_BOTTOM, &chart.fmt_buf, egui::FontId::monospace(8.0), t.dim.gamma_multiply(0.6));
                }
                ti += time_interval;
            }
        }
    }

    // (Timeframe quick selector moved to toolbar dropdown)

    // Extended hours helper — true when timestamp is outside regular trading hours
    // Uses chart session settings when session_shading is enabled, otherwise defaults to US equities (9:30-16:00 ET)
    let is_crypto = crate::data::is_crypto(&chart.symbol);
    let (rth_start_utc_secs, rth_end_utc_secs) = if chart.session_shading && !is_crypto {
        // Convert ET minutes to UTC seconds (UTC-4 offset for EDT)
        let et_offset_min: u16 = 240; // 4 hours in minutes
        ((chart.rth_start_minutes + et_offset_min) as i64 * 60,
         (chart.rth_end_minutes + et_offset_min) as i64 * 60)
    } else {
        // Default: 9:30-16:00 ET = 13:30-20:00 UTC
        (13 * 3600 + 30 * 60, 20 * 3600)
    };
    let is_extended_hour = |ts: i64| -> bool {
        if is_crypto { return false; }
        let secs_in_day = ((ts % 86400) + 86400) % 86400;
        secs_in_day < rth_start_utc_secs || secs_in_day >= rth_end_utc_secs
    };

    // Volume + candles + indicators + oscillators + drawings
    span_begin("pane_render");
    span_begin("chart_canvas");

    // Volume bars (gated by show_volume).
    // MARK_BARS_PROTOCOL: in Mark mode bars carry volume=0 — hide the histogram
    // entirely so traders aren't fooled by an empty pane.
    if chart.show_volume && !chart.bar_source_mark {
        let vol_top = rect.top() + pt + ch * 0.8;
        let vol_bottom = rect.top() + pt + ch;
        let vol_h = vol_bottom - vol_top;

        if chart.show_delta_volume && chart.delta_data.len() == n {
            // Delta volume bars — positive above midline, negative below
            let start_d = vs as usize;
            let end_d = (start_d + chart.vc as usize + 8).min(n);
            let max_delta = chart.delta_data[start_d..end_d].iter()
                .map(|d| d.abs()).fold(0.0_f32, f32::max).max(1.0);
            let zero_y = vol_top + vol_h / 2.0;
            painter.line_segment(
                [egui::pos2(rect.left(), zero_y), egui::pos2(rect.left()+cw, zero_y)],
                egui::Stroke::new(0.5, color_alpha(t.text,25)));
            for i in start_d..end_d {
                let x = bx(i as f32);
                let delta = chart.delta_data[i];
                let norm = delta / max_delta;
                let bar_h = norm.abs() * vol_h / 2.0;
                let (color, bar_top) = if delta >= 0.0 {
                    (egui::Color32::from_rgba_unmultiplied(46, 204, 113, 140), zero_y - bar_h)
                } else {
                    (egui::Color32::from_rgba_unmultiplied(231, 76, 60, 140), zero_y)
                };
                let bw = (bs * 0.7).max(1.0);
                painter.rect_filled(
                    egui::Rect::from_min_size(egui::pos2(x - bw/2.0, bar_top), egui::vec2(bw, bar_h)),
                    0.0, color);
            }
        } else {
            // Standard volume bars — batched into single mesh
            let mut mv: f32 = 0.0;
            for i in (vs as u32)..end { if let Some(b) = chart.bars.get(i as usize) { mv = mv.max(b.volume); } }
            if mv == 0.0 { mv = 1.0; }
            let mut vol_mesh = egui::Mesh::default();
            vol_mesh.texture_id = egui::TextureId::default();
            for i in (vs as u32)..end { if let Some(b) = chart.bars.get(i as usize) {
                let idx = i as usize;
                let x = bx(i as f32);
                let vh = (b.volume / mv) * vol_h;
                let bw = (bs * 0.7).max(1.0);
                let is_bull = b.close >= b.open;
                let rvol = if chart.show_rvol && idx < chart.rvol_data.len() { chart.rvol_data[idx] } else { 1.0_f32 };
                let intensity = if chart.show_rvol { (rvol / 3.0_f32).min(1.0_f32) } else { 0.4_f32 };
                let base_color = if is_bull { t.bull } else { t.bear };
                let vol_extended = chart.timestamps.get(idx).map_or(false, |&ts| is_extended_hour(ts));
                let alpha_base = (40.0_f32 + intensity * 160.0_f32) as u8;
                let vol_dim = if chart.session_shading && !is_crypto { chart.eth_bar_opacity } else { 0.4 };
                let alpha = if vol_extended { (alpha_base as f32 * vol_dim) as u8 } else { alpha_base };
                let bar_color = egui::Color32::from_rgba_unmultiplied(base_color.r(), base_color.g(), base_color.b(), alpha);
                let vi = vol_mesh.vertices.len() as u32;
                let top = vol_bottom - vh;
                vol_mesh.vertices.push(egui::epaint::Vertex { pos: egui::pos2(x - bw/2.0, top), uv: egui::epaint::WHITE_UV, color: bar_color });
                vol_mesh.vertices.push(egui::epaint::Vertex { pos: egui::pos2(x + bw/2.0, top), uv: egui::epaint::WHITE_UV, color: bar_color });
                vol_mesh.vertices.push(egui::epaint::Vertex { pos: egui::pos2(x + bw/2.0, vol_bottom), uv: egui::epaint::WHITE_UV, color: bar_color });
                vol_mesh.vertices.push(egui::epaint::Vertex { pos: egui::pos2(x - bw/2.0, vol_bottom), uv: egui::epaint::WHITE_UV, color: bar_color });
                vol_mesh.indices.extend_from_slice(&[vi, vi+1, vi+2, vi, vi+2, vi+3]);
                if chart.show_rvol && rvol > 2.5_f32 {
                    painter.text(egui::pos2(x, top - 2.0), egui::Align2::CENTER_BOTTOM,
                        &format!("{:.1}x", rvol), egui::FontId::monospace(7.0),
                        egui::Color32::from_rgba_unmultiplied(255, 255, 255, 150));
                }
            }}
            if !vol_mesh.vertices.is_empty() {
                painter.add(egui::Shape::mesh(vol_mesh));
            }
        }
    }

    // Volume Profile — cache recompute + rendering (behind candles).
    // MARK_BARS_PROTOCOL: skip when in Mark mode (volume=0 → empty profile).
    if chart.vp_mode != VolumeProfileMode::Off && !chart.bar_source_mark {
        if chart.vp_data.is_none() || chart.vp_last_vs != chart.vs || chart.vp_last_vc != chart.vc {
            let start = chart.vs.max(0.0) as usize;
            let end_vp = (start + chart.vc as usize + 8).min(chart.bars.len());
            chart.vp_data = compute_volume_profile(&chart.bars, start, end_vp, 60);
            chart.vp_last_vs = chart.vs;
            chart.vp_last_vc = chart.vc;
        }
    }

    if chart.vp_mode == VolumeProfileMode::Classic {
        if let Some(ref vp) = chart.vp_data {
            let max_bar_width = cw * 0.25;
            for level in &vp.levels {
                let y = py(level.price);
                let h = (py(level.price - vp.price_step / 2.0) - py(level.price + vp.price_step / 2.0)).abs().max(1.0);
                if !y.is_finite() || y < rect.top() + pt || y > rect.top() + pt + ch { continue; }
                let norm = if vp.max_vol > 0.0 { level.total_vol / vp.max_vol } else { 0.0 };
                let bar_w = norm * max_bar_width;
                let buy_w = bar_w * (level.buy_vol / level.total_vol.max(0.001));
                let sell_w = bar_w - buy_w;
                painter.rect_filled(egui::Rect::from_min_size(egui::pos2(rect.left(), y - h/2.0), egui::vec2(sell_w, h)),
                    0.0, egui::Color32::from_rgba_unmultiplied(231, 76, 60, 40));
                painter.rect_filled(egui::Rect::from_min_size(egui::pos2(rect.left() + sell_w, y - h/2.0), egui::vec2(buy_w, h)),
                    0.0, egui::Color32::from_rgba_unmultiplied(46, 204, 113, 40));
            }
            let poc_y = py(vp.poc_price);
            if poc_y.is_finite() {
                painter.line_segment([egui::pos2(rect.left(), poc_y), egui::pos2(rect.left()+cw, poc_y)],
                    egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(255, 193, 37, 180)));
            }
        }
    }

    if chart.vp_mode == VolumeProfileMode::Heatmap {
        if let Some(ref vp) = chart.vp_data {
            for level in &vp.levels {
                let y_top = py(level.price + vp.price_step / 2.0);
                let y_bot = py(level.price - vp.price_step / 2.0);
                if !y_top.is_finite() || !y_bot.is_finite() { continue; }
                let h = (y_bot - y_top).abs().max(1.0);
                let y = y_top.min(y_bot);
                if y > rect.top() + pt + ch || y + h < rect.top() + pt { continue; }
                let norm = if vp.max_vol > 0.0 { level.total_vol / vp.max_vol } else { 0.0 };
                let alpha = (norm * 60.0) as u8;
                let delta = level.buy_vol - level.sell_vol;
                let color = if delta >= 0.0 { egui::Color32::from_rgba_unmultiplied(46, 204, 113, alpha) }
                            else { egui::Color32::from_rgba_unmultiplied(231, 76, 60, alpha) };
                painter.rect_filled(egui::Rect::from_min_size(egui::pos2(rect.left(), y), egui::vec2(cw, h)), 0.0, color);
            }
            let poc_y = py(vp.poc_price);
            if poc_y.is_finite() {
                painter.line_segment([egui::pos2(rect.left(), poc_y), egui::pos2(rect.left()+cw, poc_y)],
                    egui::Stroke::new(1.5, egui::Color32::from_rgba_unmultiplied(255, 193, 37, 160)));
            }
            let vah_y = py(vp.vah); let val_y = py(vp.val);
            if vah_y.is_finite() && val_y.is_finite() {
                let va_rect = egui::Rect::from_min_max(
                    egui::pos2(rect.left(), vah_y.min(val_y)), egui::pos2(rect.left()+cw, vah_y.max(val_y)));
                painter.rect_stroke(va_rect, 0.0, egui::Stroke::new(0.5, egui::Color32::from_rgba_unmultiplied(255, 193, 37, 60)), egui::StrokeKind::Outside);
            }
        }
    }

    if chart.vp_mode == VolumeProfileMode::Strip {
        if let Some(ref vp) = chart.vp_data {
            let strip_w = 50.0_f32;
            let strip_x = rect.left() + cw - strip_w;
            for level in &vp.levels {
                let y_top = py(level.price + vp.price_step / 2.0);
                let y_bot = py(level.price - vp.price_step / 2.0);
                if !y_top.is_finite() || !y_bot.is_finite() { continue; }
                let h = (y_bot - y_top).abs().max(1.0);
                let y = y_top.min(y_bot);
                if y > rect.top() + pt + ch || y + h < rect.top() + pt { continue; }
                let norm = if vp.max_vol > 0.0 { level.total_vol / vp.max_vol } else { 0.0 };
                let bar_w = norm * strip_w;
                let buy_frac = level.buy_vol / level.total_vol.max(0.001);
                let buy_w = bar_w * buy_frac;
                let sell_w = bar_w - buy_w;
                painter.rect_filled(egui::Rect::from_min_size(egui::pos2(strip_x + strip_w - bar_w, y), egui::vec2(sell_w, h)),
                    0.0, egui::Color32::from_rgba_unmultiplied(231, 76, 60, 100));
                painter.rect_filled(egui::Rect::from_min_size(egui::pos2(strip_x + strip_w - buy_w, y), egui::vec2(buy_w, h)),
                    0.0, egui::Color32::from_rgba_unmultiplied(46, 204, 113, 100));
            }
            let poc_y = py(vp.poc_price);
            if poc_y.is_finite() {
                painter.line_segment([egui::pos2(strip_x, poc_y), egui::pos2(strip_x+strip_w, poc_y)],
                    egui::Stroke::new(1.5, egui::Color32::from_rgba_unmultiplied(255, 193, 37, 200)));
                painter.text(egui::pos2(strip_x - 2.0, poc_y), egui::Align2::RIGHT_CENTER, "POC",
                    egui::FontId::monospace(7.0), egui::Color32::from_rgba_unmultiplied(255, 193, 37, 180));
            }
            for (price, label) in [(vp.vah, "VAH"), (vp.val, "VAL")] {
                let y = py(price);
                if y.is_finite() {
                    painter.line_segment([egui::pos2(strip_x, y), egui::pos2(strip_x+strip_w, y)],
                        egui::Stroke::new(0.5, egui::Color32::from_rgba_unmultiplied(255, 193, 37, 80)));
                    painter.text(egui::pos2(strip_x - 2.0, y), egui::Align2::RIGHT_CENTER, label,
                        egui::FontId::monospace(7.0), egui::Color32::from_rgba_unmultiplied(255, 193, 37, 100));
                }
            }
        }
    }

    if chart.vp_mode == VolumeProfileMode::Clean {
        if let Some(ref vp) = chart.vp_data {
            let gold = egui::Color32::from_rgb(255, 193, 37);
            let vah_y = py(vp.vah); let val_y = py(vp.val);
            if vah_y.is_finite() && val_y.is_finite() {
                painter.rect_filled(
                    egui::Rect::from_min_max(egui::pos2(rect.left(), vah_y.min(val_y)), egui::pos2(rect.left()+cw, vah_y.max(val_y))),
                    0.0, egui::Color32::from_rgba_unmultiplied(255, 193, 37, 10));
                dashed_line(&painter, egui::pos2(rect.left(), vah_y), egui::pos2(rect.left()+cw, vah_y),
                    egui::Stroke::new(0.5, color_alpha(gold, 60)), LineStyle::Dashed);
                dashed_line(&painter, egui::pos2(rect.left(), val_y), egui::pos2(rect.left()+cw, val_y),
                    egui::Stroke::new(0.5, color_alpha(gold, 60)), LineStyle::Dashed);
                painter.text(egui::pos2(rect.left()+cw+3.0, vah_y), egui::Align2::LEFT_CENTER, "VAH", egui::FontId::monospace(7.0), color_alpha(gold, 140));
                painter.text(egui::pos2(rect.left()+cw+3.0, val_y), egui::Align2::LEFT_CENTER, "VAL", egui::FontId::monospace(7.0), color_alpha(gold, 140));
            }
            let poc_y = py(vp.poc_price);
            if poc_y.is_finite() {
                painter.line_segment([egui::pos2(rect.left(), poc_y), egui::pos2(rect.left()+cw, poc_y)],
                    egui::Stroke::new(1.5, color_alpha(gold, 180)));
                painter.text(egui::pos2(rect.left()+cw+3.0, poc_y), egui::Align2::LEFT_CENTER,
                    &format!("POC {:.2}", vp.poc_price), egui::FontId::monospace(7.5), color_alpha(gold, 200));
            }
            let avg_vol = vp.levels.iter().map(|l| l.total_vol).sum::<f32>() / vp.levels.len().max(1) as f32;
            for level in &vp.levels {
                let y = py(level.price);
                if !y.is_finite() || y < rect.top() + pt || y > rect.top() + pt + ch { continue; }
                if level.total_vol > avg_vol * 1.5 {
                    painter.circle_filled(egui::pos2(rect.left()+cw+2.0, y), 2.5, color_alpha(gold, 120));
                } else if level.total_vol < avg_vol * 0.3 {
                    painter.circle_filled(egui::pos2(rect.left()+cw+2.0, y), 1.5, color_alpha(egui::Color32::from_rgb(150, 150, 180), 60));
                }
            }
        }
    }

    // ── Alternative chart types (Renko, Range, Tick) — rendered from alt_bars ──
    let is_alt_mode = matches!(chart.candle_mode, CandleMode::Renko | CandleMode::RangeBar | CandleMode::TickBar);
    if is_alt_mode && !chart.alt_bars.is_empty() {
        let alt_n = chart.alt_bars.len();
        let alt_vs = chart.vs.min(alt_n as f32 - 1.0).max(0.0);
        let alt_end = ((alt_vs as u32) + chart.vc + dynamic_pad).min(alt_n as u32);

        let mut alt_body_mesh = egui::Mesh::default();
        alt_body_mesh.texture_id = egui::TextureId::default();
        let mut alt_wick_mesh = egui::Mesh::default();
        alt_wick_mesh.texture_id = egui::TextureId::default();

        for i in (alt_vs as u32)..alt_end {
            if let Some(b) = chart.alt_bars.get(i as usize) {
                let x = bx(i as f32);
                let is_bull = b.close >= b.open;
                let c = if is_bull { t.bull } else { t.bear };
                let bt = py(b.open.max(b.close));
                let bb = py(b.open.min(b.close));
                let body_h = (bb - bt).max(1.0);
                let bw_alt = (bs * 0.35).max(1.0);

                match chart.candle_mode {
                    CandleMode::Renko => {
                        let vi = alt_body_mesh.vertices.len() as u32;
                        alt_body_mesh.vertices.push(egui::epaint::Vertex { pos: egui::pos2(x - bw_alt, bt), uv: egui::epaint::WHITE_UV, color: c });
                        alt_body_mesh.vertices.push(egui::epaint::Vertex { pos: egui::pos2(x + bw_alt, bt), uv: egui::epaint::WHITE_UV, color: c });
                        alt_body_mesh.vertices.push(egui::epaint::Vertex { pos: egui::pos2(x + bw_alt, bt + body_h), uv: egui::epaint::WHITE_UV, color: c });
                        alt_body_mesh.vertices.push(egui::epaint::Vertex { pos: egui::pos2(x - bw_alt, bt + body_h), uv: egui::epaint::WHITE_UV, color: c });
                        alt_body_mesh.indices.extend_from_slice(&[vi, vi+1, vi+2, vi, vi+2, vi+3]);
                    }
                    CandleMode::RangeBar | CandleMode::TickBar => {
                        let wt = py(b.high);
                        let wb = py(b.low);
                        let hw = 0.5_f32;
                        let vi = alt_wick_mesh.vertices.len() as u32;
                        alt_wick_mesh.vertices.push(egui::epaint::Vertex { pos: egui::pos2(x - hw, wt), uv: egui::epaint::WHITE_UV, color: c });
                        alt_wick_mesh.vertices.push(egui::epaint::Vertex { pos: egui::pos2(x + hw, wt), uv: egui::epaint::WHITE_UV, color: c });
                        alt_wick_mesh.vertices.push(egui::epaint::Vertex { pos: egui::pos2(x + hw, wb), uv: egui::epaint::WHITE_UV, color: c });
                        alt_wick_mesh.vertices.push(egui::epaint::Vertex { pos: egui::pos2(x - hw, wb), uv: egui::epaint::WHITE_UV, color: c });
                        alt_wick_mesh.indices.extend_from_slice(&[vi, vi+1, vi+2, vi, vi+2, vi+3]);
                        let vi = alt_body_mesh.vertices.len() as u32;
                        alt_body_mesh.vertices.push(egui::epaint::Vertex { pos: egui::pos2(x - bw_alt, bt), uv: egui::epaint::WHITE_UV, color: c });
                        alt_body_mesh.vertices.push(egui::epaint::Vertex { pos: egui::pos2(x + bw_alt, bt), uv: egui::epaint::WHITE_UV, color: c });
                        alt_body_mesh.vertices.push(egui::epaint::Vertex { pos: egui::pos2(x + bw_alt, bt + body_h), uv: egui::epaint::WHITE_UV, color: c });
                        alt_body_mesh.vertices.push(egui::epaint::Vertex { pos: egui::pos2(x - bw_alt, bt + body_h), uv: egui::epaint::WHITE_UV, color: c });
                        alt_body_mesh.indices.extend_from_slice(&[vi, vi+1, vi+2, vi, vi+2, vi+3]);
                    }
                    _ => {}
                }
            }
        }
        if !alt_wick_mesh.vertices.is_empty() { painter.add(egui::Shape::mesh(alt_wick_mesh)); }
        if !alt_body_mesh.vertices.is_empty() { painter.add(egui::Shape::mesh(alt_body_mesh)); }
    } else if !is_alt_mode {
    // Candles — batched into meshes for fast GPU rendering
    // Build wick mesh + body mesh + session lines in a single pass
    {
    let mut wick_mesh = egui::Mesh::default();
    let mut body_mesh = egui::Mesh::default();
    let clip = painter.clip_rect();
    wick_mesh.texture_id = egui::TextureId::default();
    body_mesh.texture_id = egui::TextureId::default();

    // Precompute ETH alpha for session shading
    let eth_alpha = if chart.session_shading && !is_crypto {
        (chart.eth_bar_opacity * 255.0).round() as u8
    } else {
        45_u8 // existing default dim for extended hours
    };
    // Session background tint color (precomputed)
    let session_bg_c = if chart.session_shading && chart.session_bg_tint && !is_crypto {
        let base = hex_to_color(&chart.session_bg_color, 1.0);
        let a = (chart.session_bg_opacity * 255.0).round() as u8;
        Some(egui::Color32::from_rgba_unmultiplied(base.r(), base.g(), base.b(), a))
    } else {
        None
    };

    for i in (vs as u32)..end { if let Some(b)=chart.bars.get(i as usize) {
        let x=bx(i as f32); let c=if b.close>=b.open{t.bull}else{t.bear};
        let bt=py(b.open.max(b.close)); let bb=py(b.open.min(b.close));
        let wt=py(b.high); let wb=py(b.low); let bw=(bs*0.35).max(1.0);
        let extended = chart.timestamps.get(i as usize).map_or(false, |&ts| is_extended_hour(ts));
        // Session background tint — draw colored rect behind ETH bars
        if extended {
            if let Some(bg_c) = session_bg_c {
                let bar_left = x - bs / 2.0;
                let bar_right = x + bs / 2.0;
                let vi = body_mesh.vertices.len() as u32;
                body_mesh.vertices.push(egui::epaint::Vertex { pos: egui::pos2(bar_left, rect.top()+pt), uv: egui::epaint::WHITE_UV, color: bg_c });
                body_mesh.vertices.push(egui::epaint::Vertex { pos: egui::pos2(bar_right, rect.top()+pt), uv: egui::epaint::WHITE_UV, color: bg_c });
                body_mesh.vertices.push(egui::epaint::Vertex { pos: egui::pos2(bar_right, rect.top()+pt+ch), uv: egui::epaint::WHITE_UV, color: bg_c });
                body_mesh.vertices.push(egui::epaint::Vertex { pos: egui::pos2(bar_left, rect.top()+pt+ch), uv: egui::epaint::WHITE_UV, color: bg_c });
                body_mesh.indices.extend_from_slice(&[vi, vi+1, vi+2, vi, vi+2, vi+3]);
            }
        }
        // Session boundary line (dashed when session_shading + session_break_lines, else solid)
        if i > vs as u32 {
            if let (Some(&ts_prev), Some(&ts_cur)) = (chart.timestamps.get((i-1) as usize), chart.timestamps.get(i as usize)) {
                let prev_ext = is_extended_hour(ts_prev);
                if prev_ext != extended || (ts_cur - ts_prev) > 1800 {
                    let sx = bx(i as f32) - bs / 2.0;
                    if chart.session_shading && chart.session_break_lines && !is_crypto {
                        // Dashed session break line
                        let line_c = color_alpha(t.text,40);
                        dashed_line(&painter,
                            egui::pos2(sx, rect.top()+pt), egui::pos2(sx, rect.top()+pt+ch),
                            egui::Stroke::new(0.5, line_c), LineStyle::Dashed);
                    } else {
                        // Default: thin solid separator
                        let sep_c = color_alpha(t.text,15);
                        let vi = wick_mesh.vertices.len() as u32;
                        wick_mesh.vertices.push(egui::epaint::Vertex { pos: egui::pos2(sx - 0.25, rect.top()+pt), uv: egui::epaint::WHITE_UV, color: sep_c });
                        wick_mesh.vertices.push(egui::epaint::Vertex { pos: egui::pos2(sx + 0.25, rect.top()+pt), uv: egui::epaint::WHITE_UV, color: sep_c });
                        wick_mesh.vertices.push(egui::epaint::Vertex { pos: egui::pos2(sx + 0.25, rect.top()+pt+ch), uv: egui::epaint::WHITE_UV, color: sep_c });
                        wick_mesh.vertices.push(egui::epaint::Vertex { pos: egui::pos2(sx - 0.25, rect.top()+pt+ch), uv: egui::epaint::WHITE_UV, color: sep_c });
                        wick_mesh.indices.extend_from_slice(&[vi, vi+1, vi+2, vi, vi+2, vi+3]);
                    }
                }
            }
        }
        // Wick — add as thin rect to wick mesh
        if !matches!(chart.candle_mode, CandleMode::Line | CandleMode::Area | CandleMode::HeikinAshi) {
            let wick_c = if extended { color_alpha(c, eth_alpha) } else { c };
            let hw = 0.5_f32; // half wick width
            let vi = wick_mesh.vertices.len() as u32;
            wick_mesh.vertices.push(egui::epaint::Vertex { pos: egui::pos2(x - hw, wt), uv: egui::epaint::WHITE_UV, color: wick_c });
            wick_mesh.vertices.push(egui::epaint::Vertex { pos: egui::pos2(x + hw, wt), uv: egui::epaint::WHITE_UV, color: wick_c });
            wick_mesh.vertices.push(egui::epaint::Vertex { pos: egui::pos2(x + hw, wb), uv: egui::epaint::WHITE_UV, color: wick_c });
            wick_mesh.vertices.push(egui::epaint::Vertex { pos: egui::pos2(x - hw, wb), uv: egui::epaint::WHITE_UV, color: wick_c });
            wick_mesh.indices.extend_from_slice(&[vi, vi+1, vi+2, vi, vi+2, vi+3]);
        }
        // Body rendering depends on candle mode
        match chart.candle_mode {
            CandleMode::Standard => {
                let c_final = if extended { color_alpha(c, eth_alpha) } else { c };
                let body_h = (bb - bt).max(1.0);
                let vi = body_mesh.vertices.len() as u32;
                body_mesh.vertices.push(egui::epaint::Vertex { pos: egui::pos2(x - bw, bt), uv: egui::epaint::WHITE_UV, color: c_final });
                body_mesh.vertices.push(egui::epaint::Vertex { pos: egui::pos2(x + bw, bt), uv: egui::epaint::WHITE_UV, color: c_final });
                body_mesh.vertices.push(egui::epaint::Vertex { pos: egui::pos2(x + bw, bt + body_h), uv: egui::epaint::WHITE_UV, color: c_final });
                body_mesh.vertices.push(egui::epaint::Vertex { pos: egui::pos2(x - bw, bt + body_h), uv: egui::epaint::WHITE_UV, color: c_final });
                body_mesh.indices.extend_from_slice(&[vi, vi+1, vi+2, vi, vi+2, vi+3]);
            }
            CandleMode::Violin | CandleMode::ViolinGradient => {
                // Volume-profile candle: width = volume at each price level
                // Rendered as overlapping rounded rects for a smooth blob shape
                let micro = bar_micro_profile(b, 12);
                let is_bull = b.close >= b.open;
                let base_color = if is_bull { t.bull } else { t.bear };
                let max_half_w = (bs * 0.40).max(1.5);
                let bar_top_y = py(b.high);
                let bar_bot_y = py(b.low);
                let body_top = py(b.open.max(b.close));
                let body_bot = py(b.open.min(b.close));

                // Thin wick line
                painter.line_segment([egui::pos2(x, bar_top_y), egui::pos2(x, bar_bot_y)],
                    egui::Stroke::new(0.8, color_alpha(base_color, 60)));

                // Each level: a rounded rect centered on x, width from volume
                let slice_h = (bar_bot_y - bar_top_y).abs() / micro.len() as f32;

                for (level_price, width_frac, buy_ratio) in &micro {
                    let y = py(*level_price);
                    let in_body = y >= body_top && y <= body_bot;

                    // Width from volume concentration
                    let hw = max_half_w * width_frac;
                    if hw < 0.5 { continue; }

                    // Color
                    let alpha = if in_body { 190u8 } else { 40 };
                    let color = if chart.candle_mode == CandleMode::ViolinGradient {
                        let alignment = if is_bull { *buy_ratio } else { 1.0 - *buy_ratio };
                        let brightness = 0.35 + alignment * 0.65;
                        egui::Color32::from_rgba_unmultiplied(
                            (base_color.r() as f32 * brightness) as u8,
                            (base_color.g() as f32 * brightness) as u8,
                            (base_color.b() as f32 * brightness) as u8, alpha)
                    } else {
                        egui::Color32::from_rgba_unmultiplied(
                            base_color.r(), base_color.g(), base_color.b(), alpha)
                    };

                    // Rounded rect for this level — overlap creates smooth shape
                    let h = (slice_h * 1.3).max(2.0); // slight overlap between levels
                    let r = (hw * 0.5).min(h * 0.4); // corner radius for smoothness
                    painter.rect_filled(
                        egui::Rect::from_center_size(egui::pos2(x, y), egui::vec2(hw * 2.0, h)),
                        r, color);
                }

                // Open/Close marks — crisp horizontal ticks
                let oc_w = max_half_w * 0.7;
                painter.line_segment(
                    [egui::pos2(x - oc_w, py(b.open)), egui::pos2(x + oc_w, py(b.open))],
                    egui::Stroke::new(1.2, color_alpha(base_color, 200)));
                painter.line_segment(
                    [egui::pos2(x - oc_w, py(b.close)), egui::pos2(x + oc_w, py(b.close))],
                    egui::Stroke::new(1.2, color_alpha(base_color, 200)));
            }
            CandleMode::Gradient => {
                let micro = bar_micro_profile(b, 10);
                let is_bull = b.close >= b.open;
                let base_color = if is_bull { t.bull } else { t.bear };
                let body_top = py(b.open.max(b.close));
                let body_bot = py(b.open.min(b.close));
                let body_h = (body_bot - body_top).max(1.0);
                let bw_g = (bs * 0.7).max(1.0);

                let num_slices = 10;
                let slice_h = body_h / num_slices as f32;

                for si in 0..num_slices {
                    let slice_y = body_top + si as f32 * slice_h;
                    let frac = si as f32 / num_slices as f32;

                    let buy_ratio = micro.get(
                        ((1.0 - frac) * (micro.len() as f32 - 1.0)).round() as usize
                    ).map(|m| m.2).unwrap_or(0.5);

                    // Shade-based: base hue maintained, brightness shows alignment
                    let alignment = if is_bull { buy_ratio } else { 1.0 - buy_ratio };
                    let brightness = 0.3 + alignment * 0.7; // range 0.3 to 1.0
                    let r = (base_color.r() as f32 * brightness) as u8;
                    let g = (base_color.g() as f32 * brightness) as u8;
                    let bv = (base_color.b() as f32 * brightness) as u8;
                    let color = egui::Color32::from_rgba_unmultiplied(r, g, bv, 220);

                    painter.rect_filled(
                        egui::Rect::from_min_size(egui::pos2(x - bw_g/2.0, slice_y), egui::vec2(bw_g, slice_h + 0.5)),
                        0.0, color);
                }
            }
            CandleMode::HeikinAshi => {
                let idx = i as usize;
                let ha_close = (b.open + b.high + b.low + b.close) / 4.0;
                let ha_open = if idx > 0 {
                    if let Some(prev) = chart.bars.get(idx - 1) {
                        let prev_ha_c = (prev.open + prev.high + prev.low + prev.close) / 4.0;
                        let prev_ha_o = (prev.open + prev.close) / 2.0;
                        (prev_ha_o + prev_ha_c) / 2.0
                    } else { (b.open + b.close) / 2.0 }
                } else { (b.open + b.close) / 2.0 };
                let ha_high = b.high.max(ha_open).max(ha_close);
                let ha_low = b.low.min(ha_open).min(ha_close);
                let ha_bull = ha_close >= ha_open;
                let c = if ha_bull { t.bull } else { t.bear };
                let bt = py(ha_open.max(ha_close));
                let bb = py(ha_open.min(ha_close));
                let wt = py(ha_high);
                let wb = py(ha_low);
                painter.line_segment([egui::pos2(x,wt),egui::pos2(x,wb)],egui::Stroke::new(1.0,c));
                painter.rect_filled(egui::Rect::from_min_size(egui::pos2(x-bw,bt),egui::vec2(bw*2.0,(bb-bt).max(1.0))),0.0,c);
            }
            CandleMode::Line => {
                let idx = i as usize;
                if idx > 0 {
                    if let Some(prev) = chart.bars.get(idx - 1) {
                        let prev_x = bx((idx - 1) as f32);
                        painter.line_segment(
                            [egui::pos2(prev_x, py(prev.close)), egui::pos2(x, py(b.close))],
                            egui::Stroke::new(1.5, t.accent));
                    }
                }
            }
            CandleMode::Area => {
                let idx = i as usize;
                if idx > 0 {
                    if let Some(prev) = chart.bars.get(idx - 1) {
                        let prev_x = bx((idx - 1) as f32);
                        let bottom_y = rect.top() + pt + ch;
                        let pts = vec![
                            egui::pos2(prev_x, py(prev.close)),
                            egui::pos2(x, py(b.close)),
                            egui::pos2(x, bottom_y),
                            egui::pos2(prev_x, bottom_y),
                        ];
                        painter.add(egui::Shape::convex_polygon(pts,
                            egui::Color32::from_rgba_unmultiplied(t.accent.r(), t.accent.g(), t.accent.b(), 20),
                            egui::Stroke::NONE));
                        painter.line_segment(
                            [egui::pos2(prev_x, py(prev.close)), egui::pos2(x, py(b.close))],
                            egui::Stroke::new(1.5, t.accent));
                    }
                }
            }
            // Renko/RangeBar/TickBar are rendered via the alt-mode path above
            CandleMode::Renko | CandleMode::RangeBar | CandleMode::TickBar => {}
        }
    }}
    // Submit batched candle meshes (1 draw call each instead of 200+ individual shapes)
    if !wick_mesh.vertices.is_empty() {
        painter.add(egui::Shape::mesh(wick_mesh));
    }
    if !body_mesh.vertices.is_empty() {
        painter.add(egui::Shape::mesh(body_mesh));
    }
    } // end candle batch block
    } // end else if !is_alt_mode

    // ── Multi-symbol overlays ─────────────────────────────────────────────
    for ov in &chart.symbol_overlays {
        if ov.symbol.is_empty() || ov.bars.is_empty() || !ov.visible { continue; }
        let start_idx = vs.floor() as usize;
        let end_idx = end as usize;
        let main_base = chart.bars.get(start_idx).map(|b| b.close).unwrap_or(1.0);
        let overlay_base = ov.bars.get(start_idx).map(|b| b.close).unwrap_or(1.0);
        if main_base > 0.0 && overlay_base > 0.0 {
            let overlay_color = hex_to_color(&ov.color, 1.0);
            let scale = |price: f32| -> f32 { (price / overlay_base - 1.0) * main_base + main_base };
            if ov.show_candles {
                // Candle rendering for overlay
                let bw = (bs * 0.25).max(0.5);
                for i in start_idx..end_idx {
                    if let Some(ob) = ov.bars.get(i) {
                        let x = bx(i as f32);
                        let o = scale(ob.open); let h = scale(ob.high);
                        let l = scale(ob.low); let c = scale(ob.close);
                        let bull = c >= o;
                        let alpha = if bull { 160u8 } else { 120 };
                        let col = egui::Color32::from_rgba_unmultiplied(overlay_color.r(), overlay_color.g(), overlay_color.b(), alpha);
                        let wick_col = color_alpha(overlay_color, 100);
                        // Wick
                        painter.line_segment([egui::pos2(x, py(h)), egui::pos2(x, py(l))], egui::Stroke::new(0.5, wick_col));
                        // Body
                        let bt = py(o.max(c)); let bb = py(o.min(c));
                        painter.rect_filled(egui::Rect::from_min_size(egui::pos2(x - bw, bt), egui::vec2(bw * 2.0, (bb - bt).max(0.5))), 0.0, col);
                    }
                }
            } else {
                // Line rendering
                for i in start_idx..end_idx.saturating_sub(1) {
                    if let (Some(ob0), Some(ob1)) = (ov.bars.get(i), ov.bars.get(i+1)) {
                        let pct0 = scale(ob0.close);
                        let pct1 = scale(ob1.close);
                        let y0 = py(pct0); let y1 = py(pct1);
                        if y0.is_finite() && y1.is_finite() {
                            painter.line_segment(
                                [egui::pos2(bx(i as f32), y0), egui::pos2(bx((i+1) as f32), y1)],
                                egui::Stroke::new(1.5, overlay_color));
                        }
                    }
                }
            }
            // Badge in the middle of the overlay line
            let badge_bar = (start_idx + end_idx) / 2;
            if let Some(ob) = ov.bars.get(badge_bar) {
                let pct_mid = (ob.close / overlay_base - 1.0) * main_base + main_base;
                let badge_x = bx(badge_bar as f32);
                let badge_y = py(pct_mid);
                if badge_y.is_finite() {
                    let badge_text = &ov.symbol;
                    let bg = painter.layout_no_wrap(badge_text.to_string(), egui::FontId::monospace(8.0), overlay_color);
                    let br = egui::Rect::from_center_size(egui::pos2(badge_x, badge_y - 10.0), bg.size() + egui::vec2(8.0, 4.0));
                    painter.rect_filled(br, 3.0, egui::Color32::from_rgba_unmultiplied(overlay_color.r(), overlay_color.g(), overlay_color.b(), 30));
                    painter.rect_stroke(br, 3.0, egui::Stroke::new(0.5, color_alpha(overlay_color, 100)), egui::StrokeKind::Outside);
                    painter.text(br.center(), egui::Align2::CENTER_CENTER, badge_text, egui::FontId::monospace(8.0), overlay_color);
                }
            }
            // Right edge label
            painter.text(egui::pos2(rect.left() + cw + 3.0, py(main_base)),
                egui::Align2::LEFT_CENTER, &ov.symbol,
                egui::FontId::monospace(8.0), overlay_color);
        }
    }

    // Session VWAP + standard deviation bands
    if chart.show_vwap_bands && chart.vwap_data.len() == n {
        let start_v = vs.floor() as usize;
        let end_v = (start_v + chart.vc as usize + 8).min(n);
        let vwap_color = egui::Color32::from_rgb(33, 150, 243);
        let band1_color = egui::Color32::from_rgba_unmultiplied(33, 150, 243, 30);
        let band2_color = egui::Color32::from_rgba_unmultiplied(33, 150, 243, 15);
        // ±2σ fill
        for i in start_v..end_v.saturating_sub(1) {
            if chart.vwap_upper2[i].is_nan() || chart.vwap_upper2[i+1].is_nan() { continue; }
            let x0 = bx(i as f32); let x1 = bx((i+1) as f32);
            let pts = vec![
                egui::pos2(x0, py(chart.vwap_upper2[i])), egui::pos2(x1, py(chart.vwap_upper2[i+1])),
                egui::pos2(x1, py(chart.vwap_lower2[i+1])), egui::pos2(x0, py(chart.vwap_lower2[i])),
            ];
            painter.add(egui::Shape::convex_polygon(pts, band2_color, egui::Stroke::NONE));
        }
        // ±1σ fill
        for i in start_v..end_v.saturating_sub(1) {
            if chart.vwap_upper1[i].is_nan() || chart.vwap_upper1[i+1].is_nan() { continue; }
            let x0 = bx(i as f32); let x1 = bx((i+1) as f32);
            let pts = vec![
                egui::pos2(x0, py(chart.vwap_upper1[i])), egui::pos2(x1, py(chart.vwap_upper1[i+1])),
                egui::pos2(x1, py(chart.vwap_lower1[i+1])), egui::pos2(x0, py(chart.vwap_lower1[i])),
            ];
            painter.add(egui::Shape::convex_polygon(pts, band1_color, egui::Stroke::NONE));
        }
        // VWAP line
        for i in start_v..end_v.saturating_sub(1) {
            if chart.vwap_data[i].is_nan() || chart.vwap_data[i+1].is_nan() { continue; }
            painter.line_segment(
                [egui::pos2(bx(i as f32), py(chart.vwap_data[i])), egui::pos2(bx((i+1) as f32), py(chart.vwap_data[i+1]))],
                egui::Stroke::new(1.5, vwap_color));
        }
        // ±1σ and ±2σ band lines
        for (data, alpha) in [(&chart.vwap_upper1, 50u8), (&chart.vwap_lower1, 50u8), (&chart.vwap_upper2, 30u8), (&chart.vwap_lower2, 30u8)] {
            for i in start_v..end_v.saturating_sub(1) {
                if data[i].is_nan() || data[i+1].is_nan() { continue; }
                painter.line_segment(
                    [egui::pos2(bx(i as f32), py(data[i])), egui::pos2(bx((i+1) as f32), py(data[i+1]))],
                    egui::Stroke::new(0.5, egui::Color32::from_rgba_unmultiplied(33, 150, 243, alpha)));
            }
        }
        // Label at right edge
        if let Some(&last_vwap) = chart.vwap_data.last() {
            if !last_vwap.is_nan() {
                painter.text(egui::pos2(rect.left()+cw+3.0, py(last_vwap)), egui::Align2::LEFT_CENTER,
                    &format!("VWAP {:.2}", last_vwap), egui::FontId::monospace(7.5), vwap_color);
            }
        }
    }

    span_begin("indicator_paint");
    // ── MA Ribbon (6 EMAs) ───────────────────────────────────────────────
    if chart.show_ma_ribbon && !chart.hide_all_indicators {
        let ribbon_periods = [8_usize, 13, 21, 34, 55, 89];
        let closes_v: Vec<f32> = chart.bars.iter().map(|b| b.close).collect();
        let emas: Vec<Vec<f32>> = ribbon_periods.iter().map(|&p| compute_ema(&closes_v, p)).collect();
        let start = vs.floor() as usize;
        let end_idx = (start + chart.vc as usize + 8).min(n);
        for k in 0..emas.len().saturating_sub(1) {
            for i in start..end_idx.saturating_sub(1) {
                let v0a = emas[k].get(i).copied().unwrap_or(f32::NAN);
                let v0b = emas[k+1].get(i).copied().unwrap_or(f32::NAN);
                let v1a = emas[k].get(i+1).copied().unwrap_or(f32::NAN);
                let v1b = emas[k+1].get(i+1).copied().unwrap_or(f32::NAN);
                if v0a.is_nan() || v0b.is_nan() || v1a.is_nan() || v1b.is_nan() { continue; }
                let bullish = v0a > v0b;
                let alpha = 15 + k as u8 * 5;
                let color = if bullish {
                    egui::Color32::from_rgba_unmultiplied(46, 204, 113, alpha)
                } else {
                    egui::Color32::from_rgba_unmultiplied(231, 76, 60, alpha)
                };
                let pts = vec![
                    egui::pos2(bx(i as f32), py(v0a)), egui::pos2(bx((i+1) as f32), py(v1a)),
                    egui::pos2(bx((i+1) as f32), py(v1b)), egui::pos2(bx(i as f32), py(v0b)),
                ];
                painter.add(egui::Shape::convex_polygon(pts, color, egui::Stroke::NONE));
            }
        }
    }

    // ── Prev Close / Session Open lines ──────────────────────────────────
    if chart.show_prev_close && !chart.timestamps.is_empty() {
        let mut prev_close: Option<f32> = None;
        let mut session_open: Option<f32> = None;
        for i in (1..chart.bars.len()).rev() {
            if i >= chart.timestamps.len() { continue; }
            let gap = chart.timestamps[i] - chart.timestamps[i-1];
            if gap > 14400 {
                prev_close = Some(chart.bars[i-1].close);
                session_open = Some(chart.bars[i].open);
                break;
            }
        }
        if let Some(pc) = prev_close {
            let y = py(pc);
            if y.is_finite() {
                dashed_line(&painter, egui::pos2(rect.left(), y), egui::pos2(rect.left()+cw, y),
                    egui::Stroke::new(0.5, egui::Color32::from_rgba_unmultiplied(200, 200, 200, 80)), LineStyle::Dashed);
                painter.text(egui::pos2(rect.left()+cw+3.0, y), egui::Align2::LEFT_CENTER,
                    &format!("PC {:.2}", pc), egui::FontId::monospace(7.0), color_alpha(t.text,80));
            }
        }
        if let Some(so) = session_open {
            let y = py(so);
            if y.is_finite() {
                dashed_line(&painter, egui::pos2(rect.left(), y), egui::pos2(rect.left()+cw, y),
                    egui::Stroke::new(0.5, egui::Color32::from_rgba_unmultiplied(100, 180, 255, 60)), LineStyle::Dotted);
                painter.text(egui::pos2(rect.left()+cw+3.0, y), egui::Align2::LEFT_CENTER,
                    &format!("SO {:.2}", so), egui::FontId::monospace(7.0), egui::Color32::from_rgba_unmultiplied(100, 180, 255, 60));
            }
        }
    }

    // ── Auto Support/Resistance ───────────────────────────────────────────
    // ── Gamma Levels Overlay (GEX) ────────────────────────────────────────
    if chart.show_gamma && !chart.gamma_levels.is_empty() {
        let max_gex = chart.gamma_levels.iter().map(|(_, g)| g.abs()).fold(0.0_f32, f32::max).max(1.0);
        let max_bar_w = cw * 0.12; // max band width as fraction of chart width

        let last_price = chart.bars.last().map_or(0.0, |b| b.close);
        let zero_y = py(chart.gamma_zero);
        let price_y = py(last_price);

        // Gamma territory label (top-left of pane)
        if chart.gamma_zero > 0.0 && last_price > 0.0 {
            let above_zero = last_price > chart.gamma_zero;
            let dist = (last_price - chart.gamma_zero).abs();
            let pct = dist / chart.gamma_zero * 100.0;
            let (label, col) = if above_zero {
                ("STABLE", egui::Color32::from_rgb(40, 200, 230))
            } else {
                ("VOLATILE", egui::Color32::from_rgb(240, 160, 40))
            };
            let arrow = if above_zero { "\u{2191}" } else { "\u{2193}" };
            let info = format!("{} {}  {:.2} ({:.2}%) to 0\u{03B3}", arrow, label, dist, pct);
            let font = egui::FontId::monospace(10.0);
            let galley = painter.layout_no_wrap(info.clone(), font.clone(), col);
            let lx = rect.left() + 8.0;
            let ly = rect.top() + pt + 8.0;
            painter.rect_filled(egui::Rect::from_min_size(
                egui::pos2(lx - 5.0, ly - 3.0), galley.size() + egui::vec2(10.0, 6.0)),
                4.0, egui::Color32::from_rgba_unmultiplied(t.toolbar_bg.r(), t.toolbar_bg.g(), t.toolbar_bg.b(), 220));
            painter.rect_stroke(egui::Rect::from_min_size(
                egui::pos2(lx - 5.0, ly - 3.0), galley.size() + egui::vec2(10.0, 6.0)),
                4.0, egui::Stroke::new(0.5, color_alpha(col, 60)), egui::StrokeKind::Outside);
            painter.text(egui::pos2(lx, ly + galley.size().y / 2.0), egui::Align2::LEFT_CENTER, &info, font, col);
        }

        // Gamma bands at each level
        for &(price, gex) in &chart.gamma_levels {
            let y = py(price);
            if !y.is_finite() || y < rect.top() + pt || y > rect.top() + pt + ch { continue; }
            let norm = gex.abs() / max_gex;
            let band_w = norm * max_bar_w;
            let band_h = 3.0 + norm * 4.0; // thicker bands for stronger levels

            let (color, glow_color) = if gex > 0.0 {
                // Positive gamma: cyan/blue — stabilizing, magnetic
                let alpha = (30.0 + norm * 80.0) as u8;
                let glow_alpha = (10.0 + norm * 30.0) as u8;
                (egui::Color32::from_rgba_unmultiplied(40, 180, 220, alpha),
                 egui::Color32::from_rgba_unmultiplied(40, 180, 220, glow_alpha))
            } else {
                // Negative gamma: amber/orange — accelerating, volatile
                let alpha = (30.0 + norm * 80.0) as u8;
                let glow_alpha = (10.0 + norm * 30.0) as u8;
                (egui::Color32::from_rgba_unmultiplied(240, 160, 40, alpha),
                 egui::Color32::from_rgba_unmultiplied(240, 160, 40, glow_alpha))
            };

            // Glow/aura (wider, more transparent)
            if norm > 0.2 {
                painter.rect_filled(egui::Rect::from_center_size(
                    egui::pos2(rect.left() + band_w / 2.0, y), egui::vec2(band_w * 1.5, band_h * 2.5)),
                    band_h, glow_color);
            }
            // Main band (from left edge, width proportional to magnitude)
            painter.rect_filled(egui::Rect::from_min_size(
                egui::pos2(rect.left(), y - band_h / 2.0), egui::vec2(band_w, band_h)),
                2.0, color);
        }

        // Key levels: Call Wall (prominent cyan line)
        let cw_y = py(chart.gamma_call_wall);
        if cw_y.is_finite() && cw_y > rect.top() + pt && cw_y < rect.top() + pt + ch {
            let cyan = egui::Color32::from_rgb(40, 200, 230);
            let label_font = egui::FontId::monospace(10.0);
            painter.line_segment([egui::pos2(rect.left(), cw_y), egui::pos2(rect.left() + cw, cw_y)],
                egui::Stroke::new(2.0, color_alpha(cyan, 160)));
            let cw_label = format!("CALL WALL  {:.2}", chart.gamma_call_wall);
            let galley = painter.layout_no_wrap(cw_label.clone(), label_font.clone(), cyan);
            let lx = rect.left() + cw - galley.size().x - 8.0;
            painter.rect_filled(egui::Rect::from_min_size(egui::pos2(lx - 6.0, cw_y - galley.size().y / 2.0 - 3.0), galley.size() + egui::vec2(12.0, 6.0)),
                4.0, egui::Color32::from_rgba_unmultiplied(t.toolbar_bg.r(), t.toolbar_bg.g(), t.toolbar_bg.b(), 230));
            painter.text(egui::pos2(lx, cw_y), egui::Align2::LEFT_CENTER, &cw_label, label_font.clone(), cyan);
        }

        // Put Wall (prominent amber line)
        let pw_y = py(chart.gamma_put_wall);
        if pw_y.is_finite() && pw_y > rect.top() + pt && pw_y < rect.top() + pt + ch {
            let amber = egui::Color32::from_rgb(240, 160, 40);
            let label_font = egui::FontId::monospace(10.0);
            painter.line_segment([egui::pos2(rect.left(), pw_y), egui::pos2(rect.left() + cw, pw_y)],
                egui::Stroke::new(2.0, color_alpha(amber, 160)));
            let pw_label = format!("PUT WALL  {:.2}", chart.gamma_put_wall);
            let galley = painter.layout_no_wrap(pw_label.clone(), label_font.clone(), amber);
            let lx = rect.left() + cw - galley.size().x - 8.0;
            painter.rect_filled(egui::Rect::from_min_size(egui::pos2(lx - 6.0, pw_y - galley.size().y / 2.0 - 3.0), galley.size() + egui::vec2(12.0, 6.0)),
                4.0, egui::Color32::from_rgba_unmultiplied(t.toolbar_bg.r(), t.toolbar_bg.g(), t.toolbar_bg.b(), 230));
            painter.text(egui::pos2(lx, pw_y), egui::Align2::LEFT_CENTER, &pw_label, label_font.clone(), amber);
        }

        // Zero Gamma line (dashed white)
        if zero_y.is_finite() && zero_y > rect.top() + pt && zero_y < rect.top() + pt + ch {
            dashed_line(&painter, egui::pos2(rect.left(), zero_y), egui::pos2(rect.left() + cw, zero_y),
                egui::Stroke::new(1.0, color_alpha(t.text,70)), LineStyle::Dashed);
            painter.text(egui::pos2(rect.left() + 8.0, zero_y - 10.0), egui::Align2::LEFT_BOTTOM,
                "ZERO GAMMA", egui::FontId::monospace(10.0), color_alpha(t.text,100));
        }

        // HVL — Highest Volume Level (gold diamond marker)
        let hvl_y = py(chart.gamma_hvl);
        if hvl_y.is_finite() && hvl_y > rect.top() + pt && hvl_y < rect.top() + pt + ch {
            let gold = egui::Color32::from_rgb(255, 193, 37);
            let sz = 6.0;
            let diamond = vec![
                egui::pos2(rect.left() + cw - 16.0, hvl_y - sz),
                egui::pos2(rect.left() + cw - 16.0 + sz, hvl_y),
                egui::pos2(rect.left() + cw - 16.0, hvl_y + sz),
                egui::pos2(rect.left() + cw - 16.0 - sz, hvl_y),
            ];
            painter.add(egui::Shape::convex_polygon(diamond, gold, egui::Stroke::NONE));
            painter.text(egui::pos2(rect.left() + cw - 26.0, hvl_y), egui::Align2::RIGHT_CENTER,
                &format!("HVL {:.2}", chart.gamma_hvl), egui::FontId::monospace(10.0), gold);
        }
    }

    // ── Event Markers Overlay ─────────────────────────────────────────────
    if chart.show_events && !chart.event_markers.is_empty() && !chart.timestamps.is_empty() {
        let hover_pos = ui.input(|i| i.pointer.hover_pos());
        let chart_top = rect.top() + pt;
        let chart_bot = chart_top + ch;
        let marker_y = chart_top + 10.0;
        let mut hovered_tooltip: Option<(egui::Pos2, String, String, egui::Color32)> = None;

        for em in &chart.event_markers {
            let bar_f = SignalDrawing::time_to_bar(em.time, &chart.timestamps);
            let x = bx(bar_f);
            if x < rect.left() - 5.0 || x > rect.left() + cw + 5.0 { continue; }

            let is_earnings = em.event_type == 0;
            let base_col = match em.event_type {
                0 => { // Earnings — color by beat/miss
                    match em.impact { 1 => t.bull, -1 => t.bear, _ => t.accent }
                }
                1 => egui::Color32::from_rgb(46, 204, 113),  // dividend
                2 => egui::Color32::from_rgb(52, 152, 219),  // split
                3 => egui::Color32::from_rgb(243, 156, 18),  // economic
                _ => t.accent,
            };

            dashed_line(&painter, egui::pos2(x, chart_top), egui::pos2(x, chart_bot),
                egui::Stroke::new(if is_earnings { 1.0 } else { 0.7 }, color_alpha(base_col, if is_earnings { 70 } else { 50 })),
                LineStyle::Dashed);

            let sq_sz = if is_earnings { 9.0 } else { 7.0 };
            let sq_rect = egui::Rect::from_center_size(egui::pos2(x, marker_y), egui::vec2(sq_sz, sq_sz));
            painter.rect_filled(sq_rect, if is_earnings { 3.0 } else { 2.0 }, color_alpha(base_col, 200));

            // Impact indicator
            let impact_col = match em.impact {
                1 => t.bull, -1 => t.bear, _ => t.dim,
            };
            if is_earnings {
                // Show beat/miss arrow instead of dot
                let arrow_icon = match em.impact { 1 => "\u{25B2}", -1 => "\u{25BC}", _ => "\u{25C6}" };
                painter.text(egui::pos2(x, marker_y + sq_sz + 3.0), egui::Align2::CENTER_TOP,
                    arrow_icon, egui::FontId::proportional(7.0), impact_col);
            } else {
                painter.circle_filled(egui::pos2(x, marker_y + sq_sz + 2.0), 2.0, color_alpha(impact_col, 160));
            }

            let label_icon = match em.event_type {
                0 => "E", 1 => "$", 2 => "S", 3 => "F", _ => "?",
            };
            painter.text(egui::pos2(x, marker_y + sq_sz + (if is_earnings { 12.0 } else { 7.0 })), egui::Align2::CENTER_TOP,
                label_icon, egui::FontId::monospace(7.0), color_alpha(base_col, 180));

            if let Some(hp) = hover_pos {
                if (hp.x - x).abs() < 8.0 && hp.y > chart_top && hp.y < chart_bot {
                    hovered_tooltip = Some((egui::pos2(x, marker_y + sq_sz + 18.0), em.label.clone(), em.details.clone(), base_col));
                }
            }
        }

        if let Some((pos, label, details, col)) = hovered_tooltip {
            let font = egui::FontId::monospace(9.0);
            let label_galley = painter.layout_no_wrap(label.clone(), font.clone(), col);
            let detail_galley = painter.layout_no_wrap(details.clone(), font.clone(), t.dim);
            let w = label_galley.size().x.max(detail_galley.size().x) + 16.0;
            let h = label_galley.size().y + detail_galley.size().y + 12.0;
            let mut tip_rect = egui::Rect::from_min_size(egui::pos2(pos.x - w / 2.0, pos.y), egui::vec2(w, h));
            if tip_rect.right() > rect.left() + cw { tip_rect = tip_rect.translate(egui::vec2(rect.left() + cw - tip_rect.right(), 0.0)); }
            if tip_rect.left() < rect.left() { tip_rect = tip_rect.translate(egui::vec2(rect.left() - tip_rect.left(), 0.0)); }
            painter.rect_filled(tip_rect, 4.0, egui::Color32::from_rgba_unmultiplied(t.toolbar_bg.r(), t.toolbar_bg.g(), t.toolbar_bg.b(), 235));
            painter.rect_stroke(tip_rect, 4.0, egui::Stroke::new(0.5, color_alpha(col, 80)), egui::StrokeKind::Outside);
            painter.text(egui::pos2(tip_rect.left() + 8.0, tip_rect.top() + 4.0), egui::Align2::LEFT_TOP, &label, font.clone(), col);
            painter.text(egui::pos2(tip_rect.left() + 8.0, tip_rect.top() + 4.0 + label_galley.size().y + 2.0), egui::Align2::LEFT_TOP, &details, font, t.dim);
        }
    }

    // ── Dark Pool Overlay ────────────────────────────────────────────────
    if chart.show_darkpool && !chart.darkpool_prints.is_empty() && !chart.bars.is_empty() && !chart.timestamps.is_empty() {
        let dp_vs = chart.vs;
        let dp_ts = &chart.timestamps;
        let dp_bars_len = chart.bars.len();
        let dp_vis_start = dp_vs.floor().max(0.0) as usize;
        let dp_vis_end = (dp_vis_start + chart.vc as usize + 2).min(dp_bars_len);

        // Aggregate volume at each price level for level lines
        let mut price_volume: std::collections::BTreeMap<i64, (f32, u64)> = std::collections::BTreeMap::new();

        for dp_print in &chart.darkpool_prints {
            // Find closest bar by timestamp
            let mut best_idx: Option<usize> = None;
            let mut best_dist = i64::MAX;
            match dp_ts.binary_search(&dp_print.time) {
                Ok(i) => { best_idx = Some(i); }
                Err(i) => {
                    if i < dp_ts.len() && (dp_ts[i] - dp_print.time).abs() < best_dist {
                        best_dist = (dp_ts[i] - dp_print.time).abs();
                        best_idx = Some(i);
                    }
                    if i > 0 && (dp_ts[i - 1] - dp_print.time).abs() < best_dist {
                        best_idx = Some(i - 1);
                    }
                }
            }

            let bar_idx = match best_idx {
                Some(i) => i,
                None => continue,
            };

            // Skip if not in visible range
            if bar_idx < dp_vis_start || bar_idx >= dp_vis_end { continue; }

            let cx = bx(bar_idx as f32);
            let cy = py(dp_print.price);
            if !cy.is_finite() || cy < rect.top() + pt || cy > rect.top() + pt + ch { continue; }

            // Circle radius: log10(size) scaled, clamped [4, 20]
            let radius = ((dp_print.size as f32).log10() * 2.0).clamp(4.0, 20.0);

            // Color by side
            let (fill_col, stroke_col) = match dp_print.side {
                1 => (
                    egui::Color32::from_rgba_unmultiplied(t.bull.r(), t.bull.g(), t.bull.b(), 80),
                    egui::Color32::from_rgba_unmultiplied(t.bull.r(), t.bull.g(), t.bull.b(), 160),
                ),
                -1 => (
                    egui::Color32::from_rgba_unmultiplied(t.bear.r(), t.bear.g(), t.bear.b(), 80),
                    egui::Color32::from_rgba_unmultiplied(t.bear.r(), t.bear.g(), t.bear.b(), 160),
                ),
                _ => (
                    egui::Color32::from_rgba_unmultiplied(t.dim.r(), t.dim.g(), t.dim.b(), 60),
                    egui::Color32::from_rgba_unmultiplied(t.dim.r(), t.dim.g(), t.dim.b(), 100),
                ),
            };

            // Outer glow for large prints
            if radius > 8.0 {
                painter.circle_filled(egui::pos2(cx, cy), radius + 3.0,
                    egui::Color32::from_rgba_unmultiplied(fill_col.r(), fill_col.g(), fill_col.b(), 25));
            }

            // Main circle
            painter.circle_filled(egui::pos2(cx, cy), radius, fill_col);
            painter.circle_stroke(egui::pos2(cx, cy), radius, egui::Stroke::new(1.0, stroke_col));

            // Size label inside large circles
            if radius > 10.0 {
                let label = if dp_print.size >= 1_000_000 {
                    format!("{:.1}M", dp_print.size as f32 / 1_000_000.0)
                } else {
                    format!("{}K", dp_print.size / 1000)
                };
                painter.text(egui::pos2(cx, cy), egui::Align2::CENTER_CENTER,
                    &label, egui::FontId::monospace(7.0),
                    color_alpha(t.text,200));
            }

            // Aggregate for level lines (bucket by price rounded to 2 decimal places)
            let key = (dp_print.price * 100.0) as i64;
            let entry = price_volume.entry(key).or_insert((dp_print.price, 0u64));
            entry.1 += dp_print.size;
        }

        // Draw horizontal dashed lines at prices with large aggregate dark pool volume
        let volume_threshold = 200_000u64;
        for (_key, (level_price, total_vol)) in &price_volume {
            if *total_vol < volume_threshold { continue; }
            let ly = py(*level_price);
            if !ly.is_finite() || ly < rect.top() + pt || ly > rect.top() + pt + ch { continue; }

            let line_alpha = (40.0 + (*total_vol as f32 / 500_000.0 * 60.0).min(60.0)) as u8;
            let line_col = egui::Color32::from_rgba_unmultiplied(180, 140, 255, line_alpha);
            dashed_line(&painter, egui::pos2(rect.left(), ly), egui::pos2(rect.left() + cw, ly),
                egui::Stroke::new(1.0, line_col), LineStyle::Dashed);

            // Right-edge label showing aggregate volume
            let vol_label = if *total_vol >= 1_000_000 {
                format!("DP {:.1}M", *total_vol as f32 / 1_000_000.0)
            } else {
                format!("DP {}K", total_vol / 1000)
            };
            let label_font = egui::FontId::monospace(9.0);
            let galley = painter.layout_no_wrap(vol_label.clone(), label_font.clone(), line_col);
            let lx = rect.left() + cw - galley.size().x - 8.0;
            painter.rect_filled(egui::Rect::from_min_size(
                egui::pos2(lx - 4.0, ly - galley.size().y / 2.0 - 2.0),
                galley.size() + egui::vec2(8.0, 4.0)),
                3.0, egui::Color32::from_rgba_unmultiplied(t.toolbar_bg.r(), t.toolbar_bg.g(), t.toolbar_bg.b(), 220));
            painter.text(egui::pos2(lx, ly), egui::Align2::LEFT_CENTER, &vol_label, label_font, line_col);
        }
    }

    // ── Hit-test highlighting: flash indicators/drawings when current price touches ──
    if chart.hit_highlight && n > 1 {
        let last_bar = &chart.bars[n - 1];
        let price_h = last_bar.high;
        let price_l = last_bar.low;
        let price_range = (price_h - price_l).max(0.01);
        // Generous threshold: 50% of candle range OR 0.3% of price, whichever is larger
        let touch_threshold = (price_range * 0.5).max(last_bar.close * 0.003);
        let now_inst = std::time::Instant::now();
        let flash_duration = std::time::Duration::from_millis(800);

        // Check overlay indicators (MAs, BB bands, etc.)
        for ind in &chart.indicators {
            if !ind.visible || ind.kind.category() != IndicatorCategory::Overlay { continue; }
            // Check primary line (MA value at last bar)
            if let Some(&val) = ind.values.get(n - 1) {
                if !val.is_nan() && val >= price_l - touch_threshold && val <= price_h + touch_threshold {
                    // Hit detected — register if not already tracked
                    let key = ind.id;
                    let on_cd = chart.hit_cooldowns.iter().any(|(k, bar)| *k == key && (n - 1).saturating_sub(*bar) < 10);
                    if !on_cd && !chart.hit_highlights.iter().any(|(k, t)| *k == key && t.elapsed() < flash_duration) {
                        chart.hit_highlights.push((key, now_inst));
                        chart.hit_cooldowns.push((key, n - 1));
                    }
                }
            }
            // Check upper band (BB/KC)
            if let Some(&val) = ind.values2.get(n - 1) {
                if !val.is_nan() && val >= price_l - touch_threshold && val <= price_h + touch_threshold {
                    let key = ind.id + 10000;
                    let on_cd = chart.hit_cooldowns.iter().any(|(k, bar)| *k == key && (n - 1).saturating_sub(*bar) < 10);
                    if !on_cd && !chart.hit_highlights.iter().any(|(k, t)| *k == key && t.elapsed() < flash_duration) {
                        chart.hit_highlights.push((key, now_inst));
                        chart.hit_cooldowns.push((key, n - 1));
                    }
                }
            }
            // Check lower band
            if let Some(&val) = ind.values3.get(n - 1) {
                if !val.is_nan() && val >= price_l - touch_threshold && val <= price_h + touch_threshold {
                    let key = ind.id + 20000;
                    let on_cd = chart.hit_cooldowns.iter().any(|(k, bar)| *k == key && (n - 1).saturating_sub(*bar) < 10);
                    if !on_cd && !chart.hit_highlights.iter().any(|(k, t)| *k == key && t.elapsed() < flash_duration) {
                        chart.hit_highlights.push((key, now_inst));
                        chart.hit_cooldowns.push((key, n - 1));
                    }
                }
            }
        }

        // Check trendlines
        for (di, drawing) in chart.drawings.iter().enumerate() {
            if let crate::chart_renderer::DrawingKind::TrendLine { price0, time0, price1, time1 } = &drawing.kind {
                // Interpolate trendline price at the last bar's timestamp
                if let Some(&last_ts) = chart.timestamps.last() {
                    let t0 = *time0 as f64; let t1 = *time1 as f64; let tc = last_ts as f64;
                    if (t1 - t0).abs() > 1.0 {
                        let frac = (tc - t0) / (t1 - t0);
                        let trend_price = *price0 + (*price1 - *price0) * frac as f32;
                        if trend_price >= price_l - touch_threshold && trend_price <= price_h + touch_threshold {
                            let key = 50000 + di as u32;
                            let on_cd = chart.hit_cooldowns.iter().any(|(k, bar)| *k == key && (n - 1).saturating_sub(*bar) < 10);
                            if !on_cd && !chart.hit_highlights.iter().any(|(k, t)| *k == key && t.elapsed() < flash_duration) {
                                chart.hit_highlights.push((key, now_inst));
                                chart.hit_cooldowns.push((key, n - 1));
                            }
                        }
                    }
                }
            }
            // HLine check
            if let crate::chart_renderer::DrawingKind::HLine { price } = &drawing.kind {
                if *price >= price_l - touch_threshold && *price <= price_h + touch_threshold {
                    let key = 50000 + di as u32;
                    let on_cd = chart.hit_cooldowns.iter().any(|(k, bar)| *k == key && (n - 1).saturating_sub(*bar) < 10);
                    if !on_cd && !chart.hit_highlights.iter().any(|(k, t)| *k == key && t.elapsed() < flash_duration) {
                        chart.hit_highlights.push((key, now_inst));
                        chart.hit_cooldowns.push((key, n - 1));
                    }
                }
            }
        }

        // GC expired highlights and old cooldowns
        chart.hit_highlights.retain(|(_, t)| t.elapsed() < flash_duration);
        chart.hit_cooldowns.retain(|(_, bar)| (n - 1).saturating_sub(*bar) < 20);

        // Render flash: draw the indicator/drawing line AGAIN on top in white at 3x thickness
        let start_i = vs as u32;
        for &(key, ref hit_time) in &chart.hit_highlights {
            let elapsed = hit_time.elapsed().as_secs_f32();
            let alpha = ((1.0 - elapsed / 0.8) * 255.0).clamp(0.0, 255.0) as u8;
            if alpha < 5 { continue; }
            let flash_color = egui::Color32::from_rgba_unmultiplied(255, 255, 255, alpha);

            if key < 50000 {
                // Indicator — trace the actual polyline
                let (ind_id, vals_idx) = if key < 10000 { (key, 0u8) }
                    else if key < 20000 { (key - 10000, 1) }
                    else { (key - 20000, 2) };
                if let Some(ind) = chart.indicators.iter().find(|i| i.id == ind_id) {
                    let vals = match vals_idx { 1 => &ind.values2, 2 => &ind.values3, _ => &ind.values };
                    let mut pts: Vec<egui::Pos2> = Vec::new();
                    for i in start_i..end {
                        if let Some(&v) = vals.get(i as usize) {
                            if !v.is_nan() { pts.push(egui::pos2(bx(i as f32), py(v))); }
                        }
                    }
                    if pts.len() > 1 {
                        painter.add(egui::Shape::line(pts, egui::Stroke::new(ind.thickness * 2.0, flash_color)));
                    }
                }
            } else {
                // Drawing — HLine or TrendLine
                let di = (key - 50000) as usize;
                if let Some(drawing) = chart.drawings.get(di) {
                    match &drawing.kind {
                        crate::chart_renderer::DrawingKind::HLine { price } => {
                            let fy = py(*price);
                            if fy.is_finite() {
                                painter.line_segment([egui::pos2(rect.left(), fy), egui::pos2(rect.left() + cw, fy)],
                                    egui::Stroke::new(drawing.thickness * 2.0, flash_color));
                            }
                        }
                        crate::chart_renderer::DrawingKind::TrendLine { price0, time0, price1, time1 } => {
                            let bar0 = SignalDrawing::time_to_bar(*time0, &chart.timestamps);
                            let bar1 = SignalDrawing::time_to_bar(*time1, &chart.timestamps);
                            let x0 = bx(bar0); let y0 = py(*price0);
                            let x1 = bx(bar1); let y1 = py(*price1);
                            if y0.is_finite() && y1.is_finite() {
                                painter.line_segment([egui::pos2(x0, y0), egui::pos2(x1, y1)],
                                    egui::Stroke::new(drawing.thickness * 2.0, flash_color));
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        if !chart.hit_highlights.is_empty() { ctx.request_repaint(); }
        ctx.request_repaint();
    }

    // (hit flash rendered as white overlay on top of the indicator line)

    // ── Strikes overlay circle button (O) on chart — EQUITY only ──
    let ovl_chart_x = rect.left() + cw - 18.0;
    let ovl_chart_y = rect.top() + pt + 18.0;
    if !chart.is_option {
        let btn_col = if chart.show_strikes_overlay { t.accent } else { t.dim.gamma_multiply(0.3) };
        painter.circle_filled(egui::pos2(ovl_chart_x, ovl_chart_y), 9.0, color_alpha(t.toolbar_bg, 220));
        painter.circle_stroke(egui::pos2(ovl_chart_x, ovl_chart_y), 9.0, egui::Stroke::new(1.0, btn_col));
        if chart.overlay_chain_loading {
            let angle = ctx.input(|i| i.time) as f32 * 4.0;
            for k in 0..8 {
                let a = angle + k as f32 * std::f32::consts::TAU / 8.0;
                painter.circle_filled(egui::pos2(ovl_chart_x + a.cos() * 5.0, ovl_chart_y + a.sin() * 5.0),
                    1.2, color_alpha(t.accent, 40 + (k as u8) * 25));
            }
            ctx.request_repaint();
        } else {
            painter.text(egui::pos2(ovl_chart_x, ovl_chart_y), egui::Align2::CENTER_CENTER, "O", egui::FontId::monospace(9.0), btn_col);
        }
    }

    // Auto-fetch overlay chain if on but data missing or symbol changed
    if chart.show_strikes_overlay && !chart.overlay_chain_loading && !chart.bars.is_empty()
        && (chart.overlay_chain_symbol != chart.symbol || (chart.overlay_calls.is_empty() && chart.overlay_puts.is_empty())) {
        chart.overlay_chain_loading = true;
        let sym = chart.symbol.clone();
        let price = chart.bars.last().map(|b| b.close).unwrap_or(0.0);
        fetch_overlay_chain_background(sym, price);
    }

    // ── Options strikes overlay on chart ─────────────────────────────────
    if chart.show_strikes_overlay {
        let calls = &chart.overlay_calls;
        let puts = &chart.overlay_puts;
        let last_price = chart.bars.last().map(|b| b.close).unwrap_or(0.0);

        if (!calls.is_empty() || !puts.is_empty()) && last_price > 0.0 {
            let pill_w = 130.0;
            let pill_h = 22.0;
            let min_gap = 2.0; // minimum pixels between pills
            let pill_right = rect.left() + cw - 26.0; // right edge of pills
            let pill_left = pill_right - pill_w;
            let measure_x = pill_left - 40.0; // measurement arrow X position
            let hover_pos = ui.input(|i| i.pointer.hover_pos());
            let price_y = py(last_price);

            // Collect visible strikes with their natural Y positions
            struct StrikeInfo { strike: f32, bid: f32, ask: f32, is_call: bool, natural_y: f32, display_y: f32, contract: String }
            let mut strikes: Vec<StrikeInfo> = Vec::new();
            for (row, is_call) in calls.iter().map(|r| (r, true)).chain(puts.iter().map(|r| (r, false))) {
                let sy = py(row.strike);
                if !sy.is_finite() || sy < rect.top() + pt + 5.0 || sy > rect.top() + pt + ch - 5.0 { continue; }
                if is_call && row.strike <= last_price { continue; }
                if !is_call && row.strike > last_price { continue; }
                strikes.push(StrikeInfo { strike: row.strike, bid: row.bid, ask: row.ask, is_call, natural_y: sy, display_y: sy, contract: row.contract.clone() });
            }
            // Sort by natural_y (top to bottom)
            strikes.sort_by(|a, b| a.natural_y.partial_cmp(&b.natural_y).unwrap_or(std::cmp::Ordering::Equal));

            // Deconflict overlapping pills — push apart with minimum gap
            for i in 1..strikes.len() {
                let prev_bottom = strikes[i - 1].display_y + pill_h / 2.0 + min_gap;
                let cur_top = strikes[i].display_y - pill_h / 2.0;
                if cur_top < prev_bottom {
                    strikes[i].display_y = prev_bottom + pill_h / 2.0;
                }
            }

            // Render each strike pill
            for si in &strikes {
                let base_col = if si.is_call { t.bull } else { t.bear };
                let displaced = (si.display_y - si.natural_y).abs() > 2.0;
                let pill_rect = egui::Rect::from_min_size(egui::pos2(pill_left, si.display_y - pill_h / 2.0), egui::vec2(pill_w, pill_h));
                let is_hovered = hover_pos.map_or(false, |p| {
                    egui::Rect::from_min_size(egui::pos2(pill_left - 30.0, si.display_y - pill_h / 2.0), egui::vec2(pill_w + 35.0, pill_h)).contains(p)
                });

                // Leader line to actual price level (stronger visibility)
                if displaced {
                    painter.line_segment([egui::pos2(pill_right, si.display_y), egui::pos2(pill_right + 6.0, si.natural_y)],
                        egui::Stroke::new(1.0, color_alpha(base_col, if is_hovered { 120 } else { 60 })));
                    painter.circle_filled(egui::pos2(pill_right + 6.0, si.natural_y), 2.0, color_alpha(base_col, if is_hovered { 180 } else { 80 }));
                }

                // Pill background — solid color (call=green tint, put=red tint)
                let pill_alpha = if is_hovered { 50u8 } else { 35 };
                let pill_bg = egui::Color32::from_rgba_unmultiplied(base_col.r(), base_col.g(), base_col.b(), pill_alpha);
                painter.rect_filled(pill_rect, 4.0, pill_bg);

                // Split pill: strike on left (colored) | bid×ask on right (white, larger)
                let split_x = pill_left + 44.0;
                // Strike price — colored by call/put, bold
                painter.text(egui::pos2(pill_left + 6.0, si.display_y), egui::Align2::LEFT_CENTER,
                    &format!("{:.0}", si.strike), egui::FontId::monospace(9.0),
                    color_alpha(base_col, 255));
                // Separator line
                painter.line_segment([egui::pos2(split_x, si.display_y - pill_h / 2.0 + 3.0), egui::pos2(split_x, si.display_y + pill_h / 2.0 - 3.0)],
                    egui::Stroke::new(0.5, color_alpha(base_col, 40)));
                // Bid × Ask — white, larger font
                painter.text(egui::pos2(split_x + 5.0, si.display_y), egui::Align2::LEFT_CENTER,
                    &format!("{:.2} × {:.2}", si.bid, si.ask), egui::FontId::monospace(9.0),
                    egui::Color32::from_rgb(230, 230, 240));

                // Dashed horizontal line across chart on hover (visible)
                if is_hovered {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    let strike_y = si.natural_y;
                    let mut dx = rect.left();
                    while dx < rect.left() + cw {
                        let end_x = (dx + 6.0).min(rect.left() + cw);
                        painter.line_segment([egui::pos2(dx, strike_y), egui::pos2(end_x, strike_y)],
                            egui::Stroke::new(1.0, color_alpha(base_col, 100)));
                        dx += 10.0;
                    }

                    // Price label on Y-axis
                    let axis_x = rect.left() + cw;
                    let tag_w = 42.0;
                    let tag_rect = egui::Rect::from_min_size(egui::pos2(axis_x, strike_y - 8.0), egui::vec2(tag_w, 16.0));
                    painter.rect_filled(tag_rect, 2.0, color_alpha(base_col, 180));
                    painter.text(tag_rect.center(), egui::Align2::CENTER_CENTER,
                        &format!("{:.0}", si.strike), egui::FontId::monospace(9.0), egui::Color32::WHITE);

                    // Chart button to the LEFT (only on hover)
                    let btn_y = si.display_y - pill_h / 2.0;
                    let chart_btn_rect = egui::Rect::from_min_size(egui::pos2(pill_left - 26.0, btn_y), egui::vec2(24.0, pill_h));
                    painter.rect_filled(chart_btn_rect, 3.0, color_alpha(t.accent, 45));
                    painter.rect_stroke(chart_btn_rect, 3.0, egui::Stroke::new(0.5, color_alpha(t.accent, 80)), egui::StrokeKind::Outside);
                    painter.text(chart_btn_rect.center(), egui::Align2::CENTER_CENTER, Icon::CHART_LINE, egui::FontId::proportional(11.0), t.accent);

                    // Click: pill anywhere = open order, chart button = open option chart
                    if let Some(pos) = hover_pos {
                        if ui.input(|i| i.pointer.button_clicked(egui::PointerButton::Primary)) {
                            if chart_btn_rect.contains(pos) {
                                watchlist.pending_opt_chart = Some((chart.symbol.clone(), si.strike, si.is_call, String::new()));
                                watchlist.pending_opt_chart_contract = Some(si.contract.clone());
                            } else if pill_rect.contains(pos) {
                                // Open floating order pane
                                let opt_type = if si.is_call { "C" } else { "P" };
                                let title = format!("{} {:.0}{}", chart.symbol, si.strike, opt_type);
                                let fid = chart.floating_order_panes.len() as u32 + 1;
                                chart.floating_order_panes.push(FloatingOrderPane {
                                    id: fid, title: title.clone(), symbol: chart.symbol.clone(),
                                    strike: si.strike, is_call: si.is_call, qty: 1,
                                    pos: egui::pos2(rect.left() + cw * 0.3, rect.top() + pt + ch * 0.3),
                                });
                            }
                        }
                    }

                    // Measurement: dashed vertical arrow + % label between current price and strike
                    if price_y.is_finite() {
                        let strike_screen_y = si.natural_y;
                        let dist_pct = ((si.strike - last_price) / last_price * 100.0).abs();
                        let top_y = strike_screen_y.min(price_y);
                        let bot_y = strike_screen_y.max(price_y);
                        let mid_y = (top_y + bot_y) / 2.0;
                        // Dashed vertical line
                        let mut dy = top_y;
                        while dy < bot_y {
                            let end = (dy + 3.0).min(bot_y);
                            painter.line_segment([egui::pos2(measure_x, dy), egui::pos2(measure_x, end)],
                                egui::Stroke::new(1.0, color_alpha(base_col, 120)));
                            dy += 6.0;
                        }
                        // Arrow tips
                        painter.line_segment([egui::pos2(measure_x - 3.0, top_y + 4.0), egui::pos2(measure_x, top_y)], egui::Stroke::new(1.0, color_alpha(base_col, 120)));
                        painter.line_segment([egui::pos2(measure_x + 3.0, top_y + 4.0), egui::pos2(measure_x, top_y)], egui::Stroke::new(1.0, color_alpha(base_col, 120)));
                        painter.line_segment([egui::pos2(measure_x - 3.0, bot_y - 4.0), egui::pos2(measure_x, bot_y)], egui::Stroke::new(1.0, color_alpha(base_col, 120)));
                        painter.line_segment([egui::pos2(measure_x + 3.0, bot_y - 4.0), egui::pos2(measure_x, bot_y)], egui::Stroke::new(1.0, color_alpha(base_col, 120)));
                        // Horizontal connector lines
                        painter.line_segment([egui::pos2(measure_x - 8.0, strike_screen_y), egui::pos2(pill_left - 2.0, strike_screen_y)],
                            egui::Stroke::new(0.5, color_alpha(base_col, 80)));
                        painter.line_segment([egui::pos2(measure_x - 8.0, price_y), egui::pos2(measure_x + 8.0, price_y)],
                            egui::Stroke::new(0.5, color_alpha(base_col, 80)));
                        // % label with background
                        if dist_pct > 0.01 {
                            let pct_text = format!("{:.2}%", dist_pct);
                            let pct_galley = painter.layout_no_wrap(pct_text.clone(), egui::FontId::monospace(10.0), base_col);
                            let pct_rect = egui::Rect::from_center_size(egui::pos2(measure_x, mid_y), pct_galley.size() + egui::vec2(8.0, 4.0));
                            painter.rect_filled(pct_rect, 4.0, color_alpha(t.toolbar_bg, 230));
                            painter.text(egui::pos2(measure_x, mid_y), egui::Align2::CENTER_CENTER, &pct_text, egui::FontId::monospace(10.0), base_col);
                        }
                    }
                }
            }
        }
    }

    // ── Floating order panes (reuse shared order entry component) ─────
    {
        // Take panes out of chart to avoid borrow conflict with &mut Chart
        let mut floating_panes = std::mem::take(&mut chart.floating_order_panes);
        let mut close_ids: Vec<u32> = Vec::new();

        for pane in &mut floating_panes {
            let adv = chart.order_advanced;
            let fp_panel_w = if adv { 270.0 } else { 210.0 };

            egui::Window::new(format!("float_order_{}", pane.id))
                .fixed_pos(pane.pos)
                .fixed_size(egui::vec2(fp_panel_w, 0.0))
                .title_bar(false)
                .frame(egui::Frame::popup(&ctx.style())
                    .fill(t.toolbar_bg)
                    .inner_margin(egui::Margin { left: 0, right: 0, top: 0, bottom: 0 })
                    .stroke(egui::Stroke::new(1.0, color_alpha(t.toolbar_border, 100)))
                    .corner_radius(4.0))
                .show(ctx, |ui| {
                    use crate::chart_renderer::ui::widgets::pane::FloatingOrderPaneChrome;

                    // Optional position indicator text
                    let pos_info: Option<(String, egui::Color32)> =
                        account_data_cached.as_ref().and_then(|(_, positions, _)| {
                            positions.iter().find(|p| p.symbol == chart.symbol).map(|pos| {
                                let color = if pos.qty > 0 { t.bull } else { t.bear };
                                let text  = if pos.qty > 0 {
                                    format!("+{}", pos.qty)
                                } else {
                                    format!("{}", pos.qty)
                                };
                                (text, color)
                            })
                        });

                    let mut chrome = FloatingOrderPaneChrome::new(pane.id)
                        .title(&pane.title)
                        .width(fp_panel_w)
                        .armed(chart.armed)
                        .advanced(adv)
                        .theme(t);
                    if let Some((ref text, color)) = pos_info {
                        chrome = chrome.position_text(text, color);
                    }

                    let cr = chrome.show(ui, |ui| {
                        // ── Body (shared component) ──
                        render_order_entry_body(ui, chart, t, 1000 + pane.id as u64, fp_panel_w);
                    });

                    if cr.close_clicked    { close_ids.push(pane.id); }
                    if cr.armed_toggled    { chart.armed = !chart.armed; }
                    if cr.advanced_toggled { chart.order_advanced = !chart.order_advanced; }
                    if cr.drag_delta != egui::Vec2::ZERO {
                        pane.pos.x += cr.drag_delta.x;
                        pane.pos.y += cr.drag_delta.y;
                    }
                });
        }

        floating_panes.retain(|p| !close_ids.contains(&p.id));
        chart.floating_order_panes = floating_panes;
    }

    // ── Order fill markers on chart ──────────────────────────────────────
    // Plot buy/sell arrows at the bar where the fill occurred
    if let Some((_, _, ref ib_orders)) = account_data_cached {
        for order in ib_orders {
            if order.symbol != chart.symbol || order.avg_fill_price <= 0.0 || order.status != "filled" { continue; }
            // Find bar closest to fill time
            let fill_ts = order.submitted_at / 1000; // ms → sec
            let bar_f = SignalDrawing::time_to_bar(fill_ts, &chart.timestamps);
            let x = bx(bar_f);
            let y = py(order.avg_fill_price as f32);
            if !x.is_finite() || !y.is_finite() { continue; }
            if x < rect.left() || x > rect.left() + cw { continue; }

            let is_buy = order.side == "BUY" || order.side == "BOT";
            let color = if is_buy { t.bull } else { t.bear };
            let arrow_size = 8.0_f32;

            if is_buy {
                // Upward arrow below the bar
                let tip = egui::pos2(x, y + 4.0);
                painter.add(egui::Shape::convex_polygon(
                    vec![tip, egui::pos2(x - arrow_size * 0.6, y + 4.0 + arrow_size), egui::pos2(x + arrow_size * 0.6, y + 4.0 + arrow_size)],
                    color, egui::Stroke::new(0.5, egui::Color32::WHITE)));
            } else {
                // Downward arrow above the bar
                let tip = egui::pos2(x, y - 4.0);
                painter.add(egui::Shape::convex_polygon(
                    vec![tip, egui::pos2(x - arrow_size * 0.6, y - 4.0 - arrow_size), egui::pos2(x + arrow_size * 0.6, y - 4.0 - arrow_size)],
                    color, egui::Stroke::new(0.5, egui::Color32::WHITE)));
            }
            // Qty label
            let label = format!("{}x{}", if is_buy { "B" } else { "S" }, order.filled_qty.abs());
            let label_y = if is_buy { y + 4.0 + arrow_size + 8.0 } else { y - 4.0 - arrow_size - 8.0 };
            painter.text(egui::pos2(x, label_y), egui::Align2::CENTER_CENTER, &label,
                egui::FontId::monospace(7.0), color);
        }
    }

    if chart.show_auto_sr && n > 20 {
        let lookback = 10;
        let mut levels: Vec<(f32, bool)> = vec![];
        for i in lookback..n.saturating_sub(lookback) {
            let is_pivot_high = (1..=lookback).all(|j| chart.bars[i].high >= chart.bars[i-j].high && chart.bars[i].high >= chart.bars[i+j].high);
            let is_pivot_low = (1..=lookback).all(|j| chart.bars[i].low <= chart.bars[i-j].low && chart.bars[i].low <= chart.bars[i+j].low);
            if is_pivot_high { levels.push((chart.bars[i].high, true)); }
            if is_pivot_low { levels.push((chart.bars[i].low, false)); }
        }
        levels.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        let mut clustered: Vec<(f32, usize, bool)> = vec![];
        for (price, is_res) in &levels {
            let merged = clustered.iter_mut().find(|(p, _, _)| (*price - *p).abs() / p.max(0.001) < 0.003);
            if let Some(existing) = merged {
                existing.0 = (existing.0 * existing.1 as f32 + *price) / (existing.1 + 1) as f32;
                existing.1 += 1;
            } else {
                clustered.push((*price, 1, *is_res));
            }
        }
        for (price, touches, is_res) in &clustered {
            if *touches < 2 { continue; }
            let y = py(*price);
            if !y.is_finite() || y < rect.top() + pt || y > rect.top() + pt + ch { continue; }
            let alpha = (40 + (*touches as u8).min(6) * 15).min(120);
            let col = if *is_res {
                egui::Color32::from_rgba_unmultiplied(231, 76, 60, alpha)
            } else {
                egui::Color32::from_rgba_unmultiplied(46, 204, 113, alpha)
            };
            dashed_line(&painter, egui::pos2(rect.left(), y), egui::pos2(rect.left()+cw, y),
                egui::Stroke::new(0.5, col), LineStyle::Dotted);
            painter.text(egui::pos2(rect.left()+cw+3.0, y), egui::Align2::LEFT_CENTER,
                &format!("{}{:.2} ({}x)", if *is_res { "R " } else { "S " }, price, touches),
                egui::FontId::monospace(7.0), col);
        }
    }

    // ── Swing Legs overlay — measures from most recent pivot to current price ──
    if chart.swing_leg_mode > 0 && n > 20 {
        let vis_start = vs as usize;
        let vis_end = (vis_start + chart.vc as usize).min(n);
        let pivot_n = 10_usize;
        let mut last_peak: Option<(usize, f32)> = None;
        let mut last_trough: Option<(usize, f32)> = None;
        for i in vis_start..vis_end {
            if i >= pivot_n && i + pivot_n < n {
                let is_ph = (1..=pivot_n).all(|j| chart.bars[i].high >= chart.bars[i-j].high && chart.bars[i].high >= chart.bars[i+j].high);
                let is_pl = (1..=pivot_n).all(|j| chart.bars[i].low <= chart.bars[i-j].low && chart.bars[i].low <= chart.bars[i+j].low);
                if is_ph { last_peak = Some((i, chart.bars[i].high)); }
                if is_pl { last_trough = Some((i, chart.bars[i].low)); }
            }
        }
        // Find the most recent pivot (whichever came last)
        let cur_price = last_price;
        let cur_bar = n - 1;
        let most_recent = match (last_peak, last_trough) {
            (Some((pi, _)), Some((ti, _))) => if pi > ti { last_peak.map(|p| (p, true)) } else { last_trough.map(|p| (p, false)) },
            (Some(p), None) => Some((p, true)),
            (None, Some(p)) => Some((p, false)),
            _ => None,
        };
        if let Some(((pivot_i, pivot_price), is_peak)) = most_recent {
            // Line from pivot to current price
            let from_above = is_peak; // peak → current = falling, trough → current = rising
            let col = if from_above {
                egui::Color32::from_rgba_unmultiplied(t.bear.r(), t.bear.g(), t.bear.b(), 120)
            } else {
                egui::Color32::from_rgba_unmultiplied(t.bull.r(), t.bull.g(), t.bull.b(), 120)
            };
            let pivot_x = bx(pivot_i as f32);
            let y_pivot = py(pivot_price);
            let y_current = py(cur_price);
            let dist_pct = ((cur_price - pivot_price) / pivot_price * 100.0).abs();

            if chart.swing_leg_mode == 1 {
                // Mode 1: Vertical — straight up/down at pivot X
                dashed_line(&painter, egui::pos2(pivot_x, y_pivot), egui::pos2(pivot_x, y_current),
                    egui::Stroke::new(1.0, col), LineStyle::Dashed);
                painter.circle_filled(egui::pos2(pivot_x, y_pivot), 3.5, col);
                painter.line_segment([egui::pos2(pivot_x - 4.0, y_current), egui::pos2(pivot_x + 4.0, y_current)],
                    egui::Stroke::new(1.0, col));
                let mid_y = (y_pivot + y_current) / 2.0;
                let label = format!("{:.1}%", dist_pct);
                let lg = painter.layout_no_wrap(label.clone(), egui::FontId::monospace(14.0), col);
                let lr = egui::Rect::from_center_size(egui::pos2(pivot_x + lg.size().x / 2.0 + 8.0, mid_y), lg.size() + egui::vec2(10.0, 6.0));
                painter.rect_filled(lr, 4.0, egui::Color32::from_rgba_unmultiplied(t.bg.r(), t.bg.g(), t.bg.b(), 220));
                painter.text(lr.center(), egui::Align2::CENTER_CENTER, &label, egui::FontId::monospace(14.0), col);
            } else {
                // Mode 2: Diagonal — line from pivot to current bar
                let p2 = egui::pos2(bx(cur_bar as f32), y_current);
                dashed_line(&painter, egui::pos2(pivot_x, y_pivot), p2, egui::Stroke::new(1.0, col), LineStyle::Dashed);
                painter.circle_filled(egui::pos2(pivot_x, y_pivot), 3.5, col);
                let mid = egui::pos2((pivot_x + p2.x) / 2.0, (y_pivot + y_current) / 2.0);
                let label = format!("{:.1}%", dist_pct);
                let lg = painter.layout_no_wrap(label.clone(), egui::FontId::monospace(14.0), col);
                let lr = egui::Rect::from_center_size(mid, lg.size() + egui::vec2(10.0, 6.0));
                painter.rect_filled(lr, 4.0, egui::Color32::from_rgba_unmultiplied(t.bg.r(), t.bg.g(), t.bg.b(), 220));
                painter.text(lr.center(), egui::Align2::CENTER_CENTER, &label, egui::FontId::monospace(14.0), col);
            }
        }
    }

    // ── Auto Fibonacci Retracement overlay ──────────────────────────────
    if chart.show_auto_fib && n > 20 {
        let pivot_n = 10_usize;
        let mut last_high: Option<(usize, f32)> = None;
        let mut last_low: Option<(usize, f32)> = None;
        for i in pivot_n..n.saturating_sub(pivot_n) {
            let is_ph = (1..=pivot_n).all(|j| chart.bars[i].high >= chart.bars[i-j].high && chart.bars[i].high >= chart.bars[i+j].high);
            let is_pl = (1..=pivot_n).all(|j| chart.bars[i].low <= chart.bars[i-j].low && chart.bars[i].low <= chart.bars[i+j].low);
            if is_ph { last_high = Some((i, chart.bars[i].high)); }
            if is_pl { last_low = Some((i, chart.bars[i].low)); }
        }
        if let (Some((_hi_i, high_price)), Some((_lo_i, low_price))) = (last_high, last_low) {
            let range = high_price - low_price;
            if range > 0.0 {
                let fib_levels: &[(f32, &str)] = &[
                    (0.0, "0%"), (0.236, "23.6%"), (0.382, "38.2%"), (0.5, "50%"),
                    (0.618, "61.8%"), (0.786, "78.6%"), (1.0, "100%"),
                ];
                for &(ratio, label) in fib_levels {
                    let price = low_price + range * ratio;
                    let y = py(price);
                    if !y.is_finite() || y < rect.top() + pt || y > rect.top() + pt + ch { continue; }
                    let is_key = ratio == 0.5 || ratio == 0.618;
                    let is_boundary = ratio == 0.0 || ratio == 1.0;
                    let alpha = if is_boundary { 150 } else if is_key { 80 } else { 45 };
                    let label_alpha = if is_boundary { 230 } else if is_key { 180 } else { 120 };
                    let lw = if is_boundary { 1.5 } else { 0.5 };
                    let line_col = egui::Color32::from_rgba_unmultiplied(255, 193, 37, alpha);
                    let label_col = egui::Color32::from_rgba_unmultiplied(255, 193, 37, label_alpha);
                    if is_boundary {
                        // Solid line for 0% and 100%
                        painter.line_segment([egui::pos2(rect.left(), y), egui::pos2(rect.left()+cw, y)],
                            egui::Stroke::new(lw, line_col));
                    } else {
                        dashed_line(&painter, egui::pos2(rect.left(), y), egui::pos2(rect.left()+cw, y),
                            egui::Stroke::new(lw, line_col), LineStyle::Dashed);
                    }
                    let fib_label = format!("Fib {} \u{2014} ${:.2}", label, price);
                    painter.text(egui::pos2(rect.left()+cw+3.0, y), egui::Align2::LEFT_CENTER,
                        &fib_label, egui::FontId::monospace(7.0), label_col);
                }
            }
        }
    }

    // Indicators (overlay only)
    if !chart.hide_all_indicators {
        for ind in &chart.indicators {
            if !ind.visible || ind.kind.category() != IndicatorCategory::Overlay { continue; }
            let color = hex_to_color(&ind.color, 1.0);
            let base_rgb = hex_to_color(&ind.color, 1.0);
            let dim_color = egui::Color32::from_rgba_unmultiplied(base_rgb.r(), base_rgb.g(), base_rgb.b(), 120);
            let fill_color = egui::Color32::from_rgba_unmultiplied(base_rgb.r(), base_rgb.g(), base_rgb.b(), 18);
            let start_i = vs as u32;

            // ── Bollinger Bands ──
            if ind.kind == IndicatorType::BollingerBands {
                // Resolve per-component colors (empty = inherit from main color)
                let bb_upper_color = if ind.upper_color.is_empty() { dim_color } else { hex_to_color(&ind.upper_color, 1.0) };
                let bb_lower_color = if ind.lower_color.is_empty() { dim_color } else { hex_to_color(&ind.lower_color, 1.0) };
                let bb_fill = if ind.fill_color_hex.is_empty() { fill_color } else { hex_to_color(&ind.fill_color_hex, 0.12) };
                let bb_upper_thick = if ind.upper_thickness > 0.0 { ind.upper_thickness } else { ind.thickness * 0.7 };
                let bb_lower_thick = if ind.lower_thickness > 0.0 { ind.lower_thickness } else { ind.thickness * 0.7 };
                // Fill between upper and lower
                for i in start_i..end.saturating_sub(1) {
                    let u0 = ind.values2.get(i as usize).copied().unwrap_or(f32::NAN);
                    let l0 = ind.values3.get(i as usize).copied().unwrap_or(f32::NAN);
                    let u1 = ind.values2.get(i as usize + 1).copied().unwrap_or(f32::NAN);
                    let l1 = ind.values3.get(i as usize + 1).copied().unwrap_or(f32::NAN);
                    if u0.is_nan() || l0.is_nan() || u1.is_nan() || l1.is_nan() { continue; }
                    let pts = vec![egui::pos2(bx(i as f32), py(u0)), egui::pos2(bx((i+1) as f32), py(u1)),
                        egui::pos2(bx((i+1) as f32), py(l1)), egui::pos2(bx(i as f32), py(l0))];
                    painter.add(egui::Shape::convex_polygon(pts, bb_fill, egui::Stroke::NONE));
                }
                // Upper band
                let mut pts: Vec<egui::Pos2> = vec![];
                for i in start_i..end { if let Some(&v) = ind.values2.get(i as usize) { if !v.is_nan() { pts.push(egui::pos2(bx(i as f32), py(v))); } } }
                if pts.len() > 1 { painter.add(egui::Shape::line(pts, egui::Stroke::new(bb_upper_thick, bb_upper_color))); }
                // Middle (SMA)
                let mut pts: Vec<egui::Pos2> = vec![];
                for i in start_i..end { if let Some(&v) = ind.values.get(i as usize) { if !v.is_nan() { pts.push(egui::pos2(bx(i as f32), py(v))); } } }
                if pts.len() > 1 { painter.add(egui::Shape::line(pts, egui::Stroke::new(ind.thickness, color))); }
                // Lower band
                let mut pts: Vec<egui::Pos2> = vec![];
                for i in start_i..end { if let Some(&v) = ind.values3.get(i as usize) { if !v.is_nan() { pts.push(egui::pos2(bx(i as f32), py(v))); } } }
                if pts.len() > 1 { painter.add(egui::Shape::line(pts, egui::Stroke::new(bb_lower_thick, bb_lower_color))); }
                // Value label
                if let Some(&last_val) = ind.values.iter().rev().find(|v| !v.is_nan()) {
                    let label_y = py(last_val);
                    if label_y.is_finite() && label_y > rect.top() + pt && label_y < rect.top() + pt + ch {
                        painter.text(egui::pos2(rect.left()+cw+3.0, label_y), egui::Align2::LEFT_CENTER,
                            &format!("BB {:.2}", last_val), egui::FontId::monospace(8.0), color);
                    }
                }
                continue;
            }

            // ── Keltner Channels ──
            if ind.kind == IndicatorType::KeltnerChannels {
                for i in start_i..end.saturating_sub(1) {
                    let u0 = ind.values2.get(i as usize).copied().unwrap_or(f32::NAN);
                    let l0 = ind.values3.get(i as usize).copied().unwrap_or(f32::NAN);
                    let u1 = ind.values2.get(i as usize + 1).copied().unwrap_or(f32::NAN);
                    let l1 = ind.values3.get(i as usize + 1).copied().unwrap_or(f32::NAN);
                    if u0.is_nan() || l0.is_nan() || u1.is_nan() || l1.is_nan() { continue; }
                    let pts = vec![egui::pos2(bx(i as f32), py(u0)), egui::pos2(bx((i+1) as f32), py(u1)),
                        egui::pos2(bx((i+1) as f32), py(l1)), egui::pos2(bx(i as f32), py(l0))];
                    painter.add(egui::Shape::convex_polygon(pts, fill_color, egui::Stroke::NONE));
                }
                let mut pts: Vec<egui::Pos2> = vec![];
                for i in start_i..end { if let Some(&v) = ind.values2.get(i as usize) { if !v.is_nan() { pts.push(egui::pos2(bx(i as f32), py(v))); } } }
                if pts.len() > 1 { painter.add(egui::Shape::line(pts, egui::Stroke::new(ind.thickness * 0.7, dim_color))); }
                let mut pts: Vec<egui::Pos2> = vec![];
                for i in start_i..end { if let Some(&v) = ind.values.get(i as usize) { if !v.is_nan() { pts.push(egui::pos2(bx(i as f32), py(v))); } } }
                if pts.len() > 1 { painter.add(egui::Shape::line(pts, egui::Stroke::new(ind.thickness, color))); }
                let mut pts: Vec<egui::Pos2> = vec![];
                for i in start_i..end { if let Some(&v) = ind.values3.get(i as usize) { if !v.is_nan() { pts.push(egui::pos2(bx(i as f32), py(v))); } } }
                if pts.len() > 1 { painter.add(egui::Shape::line(pts, egui::Stroke::new(ind.thickness * 0.7, dim_color))); }
                if let Some(&last_val) = ind.values.iter().rev().find(|v| !v.is_nan()) {
                    let label_y = py(last_val);
                    if label_y.is_finite() && label_y > rect.top() + pt && label_y < rect.top() + pt + ch {
                        painter.text(egui::pos2(rect.left()+cw+3.0, label_y), egui::Align2::LEFT_CENTER,
                            &format!("KC {:.2}", last_val), egui::FontId::monospace(8.0), color);
                    }
                }
                continue;
            }

            // ── Ichimoku Cloud ──
            if ind.kind == IndicatorType::Ichimoku {
                // Cloud fill (senkou_a vs senkou_b)
                for i in start_i..end.saturating_sub(1) {
                    let sa0 = ind.values3.get(i as usize).copied().unwrap_or(f32::NAN);
                    let sb0 = ind.values4.get(i as usize).copied().unwrap_or(f32::NAN);
                    let sa1 = ind.values3.get(i as usize + 1).copied().unwrap_or(f32::NAN);
                    let sb1 = ind.values4.get(i as usize + 1).copied().unwrap_or(f32::NAN);
                    if sa0.is_nan() || sb0.is_nan() || sa1.is_nan() || sb1.is_nan() { continue; }
                    let bullish = sa0 > sb0;
                    let cloud_col = if bullish {
                        egui::Color32::from_rgba_unmultiplied(46, 204, 113, 22)
                    } else {
                        egui::Color32::from_rgba_unmultiplied(231, 76, 60, 22)
                    };
                    let pts = vec![egui::pos2(bx(i as f32), py(sa0)), egui::pos2(bx((i+1) as f32), py(sa1)),
                        egui::pos2(bx((i+1) as f32), py(sb1)), egui::pos2(bx(i as f32), py(sb0))];
                    painter.add(egui::Shape::convex_polygon(pts, cloud_col, egui::Stroke::NONE));
                }
                // Tenkan (thin)
                let mut pts: Vec<egui::Pos2> = vec![];
                for i in start_i..end { if let Some(&v) = ind.values.get(i as usize) { if !v.is_nan() { pts.push(egui::pos2(bx(i as f32), py(v))); } } }
                if pts.len() > 1 { painter.add(egui::Shape::line(pts, egui::Stroke::new(ind.thickness * 0.8, egui::Color32::from_rgba_unmultiplied(230, 100, 100, 220)))); }
                // Kijun (thicker)
                let mut pts: Vec<egui::Pos2> = vec![];
                for i in start_i..end { if let Some(&v) = ind.values2.get(i as usize) { if !v.is_nan() { pts.push(egui::pos2(bx(i as f32), py(v))); } } }
                if pts.len() > 1 { painter.add(egui::Shape::line(pts, egui::Stroke::new(ind.thickness * 1.2, egui::Color32::from_rgba_unmultiplied(100, 140, 230, 220)))); }
                // Chikou (dotted)
                let mut prev_pt: Option<egui::Pos2> = None;
                for i in start_i..end {
                    if let Some(&v) = ind.values5.get(i as usize) {
                        if !v.is_nan() {
                            let p = egui::pos2(bx(i as f32), py(v));
                            if let Some(pp) = prev_pt {
                                let dir = p - pp; let len = dir.length();
                                if len > 1.0 { let norm = dir / len; let mut d = 0.0;
                                    while d < len { let a = pp + norm * d; let b = pp + norm * (d + 2.0).min(len);
                                        painter.line_segment([a, b], egui::Stroke::new(0.8, egui::Color32::from_rgba_unmultiplied(180, 230, 100, 160))); d += 4.0; }
                                }
                            }
                            prev_pt = Some(p);
                        } else { prev_pt = None; }
                    }
                }
                continue;
            }

            // ── Parabolic SAR (dots) ──
            if ind.kind == IndicatorType::ParabolicSAR {
                for i in start_i..end {
                    if let Some(&v) = ind.values.get(i as usize) {
                        if v.is_nan() { continue; }
                        let x = bx(i as f32); let y = py(v);
                        if !y.is_finite() { continue; }
                        let bar_close = chart.bars.get(i as usize).map(|b| b.close).unwrap_or(v);
                        let is_below = v < bar_close; // below price = uptrend
                        let dot_col = if is_below { egui::Color32::from_rgb(46, 204, 113) } else { egui::Color32::from_rgb(231, 76, 60) };
                        painter.circle_filled(egui::pos2(x, y), 2.0, dot_col);
                    }
                }
                continue;
            }

            // ── Supertrend (colored line) ──
            if ind.kind == IndicatorType::Supertrend {
                for i in start_i..end.saturating_sub(1) {
                    let v0 = ind.values.get(i as usize).copied().unwrap_or(f32::NAN);
                    let v1 = ind.values.get(i as usize + 1).copied().unwrap_or(f32::NAN);
                    if v0.is_nan() || v1.is_nan() { continue; }
                    let bullish = ind.supertrend_dir.get(i as usize).copied().unwrap_or(true);
                    let st_col = if bullish { egui::Color32::from_rgb(46, 204, 113) } else { egui::Color32::from_rgb(231, 76, 60) };
                    painter.line_segment([egui::pos2(bx(i as f32), py(v0)), egui::pos2(bx((i+1) as f32), py(v1))],
                        egui::Stroke::new(ind.thickness, st_col));
                }
                continue;
            }

            // ── Generic overlay (SMA, EMA, WMA, DEMA, TEMA, VWAP) ──
            chart.indicator_pts_buf.clear();
            for i in start_i..end {
                if let Some(&v) = ind.values.get(i as usize) {
                    if !v.is_nan() { chart.indicator_pts_buf.push(egui::pos2(bx(i as f32), py(v))); }
                }
            }
            if chart.indicator_pts_buf.len() > 1 {
                let stroke = egui::Stroke::new(ind.thickness, color);
                match ind.line_style {
                    LineStyle::Solid => { painter.add(egui::Shape::line(chart.indicator_pts_buf.clone(), stroke)); }
                    LineStyle::Dashed | LineStyle::Dotted => {
                        let (dash, gap) = if ind.line_style == LineStyle::Dashed { (6.0, 3.0) } else { (2.0, 2.0) };
                        for w in chart.indicator_pts_buf.windows(2) {
                            let a = w[0]; let b = w[1];
                            let dir = b - a; let len = dir.length();
                            if len < 1.0 { continue; }
                            let norm = dir / len; let mut d = 0.0;
                            while d < len {
                                let p0 = a + norm * d;
                                let p1 = a + norm * (d + dash).min(len);
                                painter.line_segment([p0, p1], stroke);
                                d += dash + gap;
                            }
                        }
                    }
                }
            }
            // Value label on right edge
            if let Some(&last_val) = ind.values.iter().rev().find(|v| !v.is_nan()) {
                let label_y = py(last_val);
                if label_y.is_finite() && label_y > rect.top() + pt && label_y < rect.top() + pt + ch {
                    let label = format!("{} {:.2}", ind.display_name(), last_val);
                    let galley = painter.layout_no_wrap(label.clone(), egui::FontId::monospace(8.0), color);
                    let lx = rect.left() + cw + 3.0;
                    let bg = egui::Rect::from_min_size(egui::pos2(lx - 2.0, label_y - galley.size().y / 2.0 - 1.0), galley.size() + egui::vec2(4.0, 2.0));
                    painter.rect_filled(bg, 2.0, egui::Color32::from_rgba_unmultiplied(t.toolbar_bg.r(), t.toolbar_bg.g(), t.toolbar_bg.b(), 200));
                    painter.text(egui::pos2(lx, label_y), egui::Align2::LEFT_CENTER, &label, egui::FontId::monospace(8.0), color);
                }
            }
            // Multi-TF badge on chart line
            if !ind.source_tf.is_empty() {
                // Show TF badge at ~1/3 from left (2/3 back from right edge)
                let visible_start = vs as u32;
                let visible_end = end.min(n as u32);
                let badge_bar = (visible_start + (visible_end - visible_start) / 3) as usize;
                if let Some(&v) = ind.values.get(badge_bar) {
                    if !v.is_nan() {
                        let bx_pos = bx(badge_bar as f32);
                        let by_pos = py(v);
                        if bx_pos.is_finite() && by_pos.is_finite() && by_pos > rect.top() + pt && by_pos < rect.top() + pt + ch {
                            let badge_text = &ind.source_tf;
                            let badge_galley = painter.layout_no_wrap(badge_text.to_string(), egui::FontId::monospace(7.0), color);
                            let badge_rect = egui::Rect::from_min_size(
                                egui::pos2(bx_pos - badge_galley.size().x / 2.0 - 3.0, by_pos - badge_galley.size().y - 6.0),
                                badge_galley.size() + egui::vec2(6.0, 3.0));
                            painter.rect_filled(badge_rect, 3.0, color_alpha(color, 25));
                            painter.rect_stroke(badge_rect, 3.0, egui::Stroke::new(0.5, color_alpha(color, 80)), egui::StrokeKind::Outside);
                            painter.text(badge_rect.center(), egui::Align2::CENTER_CENTER, badge_text, egui::FontId::monospace(7.0), color);
                        }
                    }
                }
            }
        }
    }

    span_begin("drawings_paint");
    // Drawings (with selection highlight + endpoint handles)
    // Clamp helper — prevents extreme coordinates from causing massive tessellation allocations
    // ── Trigger level lines (options conditional orders on underlying) ──
    for tl in &chart.trigger_levels {
        let y = py(tl.trigger_price);
        if !y.is_finite() || y.abs() > 50000.0 { continue; }
        let is_buy = tl.side == OrderSide::Buy;
        let color = if is_buy { t.bull } else { t.bear };
        let alpha = if tl.submitted { 180 } else { 255 };
        let label = format!("{} {} {} {:.2} x{}", Icon::LIGHTNING,
            if is_buy { "BUY" } else { "SELL" }, tl.option_type, tl.trigger_price, tl.qty);
        let status = if tl.submitted { " LIVE" } else { " DRAFT" };
        // Dashed line
        dashed_line(&painter, egui::pos2(rect.left(), y), egui::pos2(rect.left()+cw, y),
            egui::Stroke::new(1.5, color_alpha(color, alpha)), LineStyle::Dashed);
        // Label on the left
        painter.text(egui::pos2(rect.left() + 4.0, y - 12.0), egui::Align2::LEFT_BOTTOM,
            &label, egui::FontId::monospace(9.0), color_alpha(color, alpha));
        // Status badge on the right
        painter.text(egui::pos2(rect.left() + cw - 4.0, y - 12.0), egui::Align2::RIGHT_BOTTOM,
            status, egui::FontId::monospace(8.0), color_alpha(color, 120));
        // Y-axis price tag
        let tag_w = 54.0;
        let tag_rect = egui::Rect::from_min_size(egui::pos2(rect.left() + cw, y - 8.0), egui::vec2(tag_w, 16.0));
        painter.rect_filled(tag_rect, 2.0, color_alpha(color, alpha));
        painter.text(tag_rect.center(), egui::Align2::CENTER_CENTER,
            &format!("{:.2}", tl.trigger_price), egui::FontId::monospace(9.0), egui::Color32::WHITE);
    }

    let clamp_pt = |p: egui::Pos2| -> egui::Pos2 {
        let margin = 10000.0;
        egui::pos2(p.x.clamp(-margin, margin), p.y.clamp(-margin, margin))
    };
    let in_bounds = |p: egui::Pos2| -> bool { p.x.is_finite() && p.y.is_finite() && p.x.abs() < 50000.0 && p.y.abs() < 50000.0 };

    for d in &chart.drawings {
        if chart.hide_all_drawings { break; }
        if chart.hidden_groups.contains(&d.group_id) { continue; }
        let is_sel = chart.selected_ids.contains(&d.id);
        let dc = hex_to_color(&d.color, d.opacity);
        let sc = egui::Stroke::new(if is_sel { d.thickness + 1.0 } else { d.thickness }, if is_sel { egui::Color32::WHITE } else { dc });
        let ls = d.line_style;
        match &d.kind {
            DrawingKind::HLine{price}=>{
                let y=py(*price);
                if y.is_finite() && y.abs() < 50000.0 {
                    dashed_line(&painter, egui::pos2(rect.left(),y), egui::pos2(rect.left()+cw,y), sc, ls);
                    if is_sel {
                        let hsel_st = style::current();
                        painter.circle_filled(egui::pos2(rect.left()+cw-10.0,y), (hsel_st.r_xs as f32 + 3.0).max(4.0), t.accent);
                    }
                }
            }
            DrawingKind::TrendLine{price0,time0,price1,time1}=>{
                let p0=egui::pos2(bx(SignalDrawing::time_to_bar(*time0, &chart.timestamps)),py(*price0));
                let p1=egui::pos2(bx(SignalDrawing::time_to_bar(*time1, &chart.timestamps)),py(*price1));
                if p0.x.is_finite() && p1.x.is_finite() && p0.y.is_finite() && p1.y.is_finite() {
                    let chart_left = rect.left();
                    let chart_right = rect.left() + cw;
                    let dx = p1.x - p0.x;
                    let (mut draw_a, mut draw_b) = (p0, p1);
                    if dx.abs() > 0.001 {
                        let slope = (p1.y - p0.y) / dx;
                        if d.extend_left {
                            let left_y = p0.y + slope * (chart_left - p0.x);
                            draw_a = egui::pos2(chart_left, left_y);
                        }
                        if d.extend_right {
                            let right_y = p0.y + slope * (chart_right - p0.x);
                            draw_b = egui::pos2(chart_right, right_y);
                        }
                    }
                    dashed_line(&painter, clamp_pt(draw_a), clamp_pt(draw_b), sc, ls);
                    if is_sel {
                        let sel_st = style::current();
                        let handle_r = (sel_st.r_xs as f32 + 3.0).max(4.0);
                        let handle_stroke = egui::Stroke::new(sel_st.stroke_std, egui::Color32::WHITE);
                        painter.circle_filled(clamp_pt(p0), handle_r, t.accent);
                        painter.circle_stroke(clamp_pt(p0), handle_r, handle_stroke);
                        painter.circle_filled(clamp_pt(p1), handle_r, t.accent);
                        painter.circle_stroke(clamp_pt(p1), handle_r, handle_stroke);
                        // Info label
                        let mid = egui::pos2((p0.x + p1.x) / 2.0, (p0.y + p1.y) / 2.0);
                        let dp = *price1 - *price0;
                        let pct = if *price0 != 0.0 { dp / *price0 * 100.0 } else { 0.0 };
                        let b0f = SignalDrawing::time_to_bar(*time0, &chart.timestamps);
                        let b1f = SignalDrawing::time_to_bar(*time1, &chart.timestamps);
                        let bars = (b1f - b0f).abs().round() as i32;
                        let info = format!("{:+.2} ({:+.1}%) {} bars", dp, pct, bars);
                        let ig = painter.layout_no_wrap(info.clone(), egui::FontId::monospace(style::font_xs()), color_alpha(t.text,180));
                        let info_rect = egui::Rect::from_center_size(mid - egui::vec2(0.0, 12.0), ig.size() + egui::vec2(8.0, 4.0));
                        painter.rect_filled(info_rect, sel_st.r_xs as f32, egui::Color32::from_rgba_unmultiplied(t.toolbar_bg.r(), t.toolbar_bg.g(), t.toolbar_bg.b(), 210));
                        painter.text(mid - egui::vec2(0.0, 12.0), egui::Align2::CENTER_CENTER, &info, egui::FontId::monospace(style::font_xs()), color_alpha(t.text,180));
                    }
                }
            }
            DrawingKind::HZone{price0,price1}=>{
                let(y0,y1)=(py(*price0),py(*price1));
                if y0.is_finite() && y1.is_finite() && y0.abs() < 50000.0 && y1.abs() < 50000.0 {
                    let fill = hex_to_color(&d.color, d.opacity * 0.1);
                    painter.rect_filled(egui::Rect::from_min_max(egui::pos2(rect.left(),y0.min(y1)),egui::pos2(rect.left()+cw,y0.max(y1))),0.0,fill);
                    dashed_line(&painter, egui::pos2(rect.left(),y0), egui::pos2(rect.left()+cw,y0), sc, ls);
                    dashed_line(&painter, egui::pos2(rect.left(),y1), egui::pos2(rect.left()+cw,y1), sc, ls);
                    if is_sel {
                        let hsel_st = style::current();
                        let hzone_hr = (hsel_st.r_xs as f32 + 3.0).max(4.0);
                        painter.circle_filled(egui::pos2(rect.left()+cw-10.0,y0), hzone_hr, t.accent);
                        painter.circle_filled(egui::pos2(rect.left()+cw-10.0,y1), hzone_hr, t.accent);
                    }
                }
            }
            DrawingKind::BarMarker{time,price,up}=>{
                let x=bx(SignalDrawing::time_to_bar(*time, &chart.timestamps)); let y=py(*price);
                if !in_bounds(egui::pos2(x, y)) { continue; }
                let dir = if *up { -1.0 } else { 1.0 };
                let sz = 6.0;
                let pts = vec![
                    egui::pos2(x, y + dir*2.0),
                    egui::pos2(x - sz, y + dir*(sz+4.0)),
                    egui::pos2(x + sz, y + dir*(sz+4.0)),
                ];
                painter.add(egui::Shape::convex_polygon(pts, dc, egui::Stroke::NONE));
                if is_sel {
                    painter.circle_stroke(egui::pos2(x, y), 8.0, egui::Stroke::new(1.5, egui::Color32::WHITE));
                }
            }
            DrawingKind::Fibonacci{price0,time0,price1,time1}=>{
                let x0 = bx(SignalDrawing::time_to_bar(*time0, &chart.timestamps));
                let x1 = bx(SignalDrawing::time_to_bar(*time1, &chart.timestamps));
                let xl = x0.min(x1); let xr = x0.max(x1);
                let range = *price1 - *price0;
                // Retracement levels (solid) + extension levels (dashed, dimmer)
                let retrace: &[(f32, bool)] = &[
                    (0.0, true), (0.236, true), (0.382, true), (0.5, true),
                    (0.618, true), (0.786, true), (1.0, true),
                ];
                let extensions: &[(f32, bool)] = &[
                    (-0.272, false), (-0.618, false),
                    (1.272, false), (1.414, false), (1.618, false),
                    (2.0, false), (2.618, false), (3.146, false),
                ];
                for &(lv, is_retrace) in retrace.iter().chain(extensions.iter()) {
                    let lp = *price0 + range * lv;
                    let y = py(lp);
                    if y.is_finite() && y.abs() < 50000.0 {
                        let alpha = if lv == 0.0 || lv == 1.0 { 255 }
                            else if is_retrace { 160 }
                            else { 100 }; // extensions dimmer
                        let thick = if is_retrace { d.thickness * 0.7 } else { d.thickness * 0.5 };
                        let lsc = egui::Stroke::new(if is_sel { thick + 0.5 } else { thick }, color_alpha(dc, alpha as u8));
                        let line_style = if is_retrace { ls } else { LineStyle::Dashed };
                        dashed_line(&painter, egui::pos2(xl, y), egui::pos2(xr, y), lsc, line_style);
                        let label_alpha = if is_retrace { 200 } else { 130 };
                        painter.text(egui::pos2(xr + 4.0, y), egui::Align2::LEFT_CENTER,
                            &format!("{:.1}%  {:.2}", lv * 100.0, lp), egui::FontId::monospace(8.0), color_alpha(dc, label_alpha));
                    }
                }
                // Shaded golden zone (38.2%-61.8%)
                let y382 = py(*price0 + range * 0.382);
                let y618 = py(*price0 + range * 0.618);
                if y382.is_finite() && y618.is_finite() && y382.abs() < 50000.0 && y618.abs() < 50000.0 {
                    painter.rect_filled(egui::Rect::from_min_max(
                        egui::pos2(xl, y382.min(y618)), egui::pos2(xr, y382.max(y618))),
                        0.0, hex_to_color(&d.color, d.opacity * 0.08));
                }
                // Shaded extension zone (161.8%-261.8%) — lighter
                let y1618 = py(*price0 + range * 1.618);
                let y2618 = py(*price0 + range * 2.618);
                if y1618.is_finite() && y2618.is_finite() && y1618.abs() < 50000.0 && y2618.abs() < 50000.0 {
                    painter.rect_filled(egui::Rect::from_min_max(
                        egui::pos2(xl, y1618.min(y2618)), egui::pos2(xr, y1618.max(y2618))),
                        0.0, hex_to_color(&d.color, d.opacity * 0.04));
                }
                if is_sel {
                    let p0s = egui::pos2(x0, py(*price0));
                    let p1s = egui::pos2(x1, py(*price1));
                    painter.circle_filled(p0s, 5.0, egui::Color32::from_rgb(74,158,255));
                    painter.circle_filled(p1s, 5.0, egui::Color32::from_rgb(74,158,255));
                }
            }
            DrawingKind::Channel{price0,time0,price1,time1,offset} | DrawingKind::FibChannel{price0,time0,price1,time1,offset}=>{
                let is_fib_chan = matches!(&d.kind, DrawingKind::FibChannel{..});
                let p0 = egui::pos2(bx(SignalDrawing::time_to_bar(*time0, &chart.timestamps)), py(*price0));
                let p1 = egui::pos2(bx(SignalDrawing::time_to_bar(*time1, &chart.timestamps)), py(*price1));
                let q0 = egui::pos2(p0.x, py(*price0 + *offset));
                let q1 = egui::pos2(p1.x, py(*price1 + *offset));
                if p0.x.is_finite() && p1.x.is_finite() && p0.y.is_finite() && p1.y.is_finite() {
                    // Extend lines to full chart edges (TradingView style)
                    let chart_left = rect.left();
                    let chart_right = rect.left() + cw;
                    let dx = p1.x - p0.x;
                    let extend_line = |a: egui::Pos2, b: egui::Pos2| -> (egui::Pos2, egui::Pos2) {
                        let ldx = b.x - a.x;
                        if ldx.abs() < 0.001 { return (a, b); } // vertical — don't extend
                        let slope = (b.y - a.y) / ldx;
                        let left_y = a.y + slope * (chart_left - a.x);
                        let right_y = a.y + slope * (chart_right - a.x);
                        (egui::pos2(chart_left, left_y), egui::pos2(chart_right, right_y))
                    };
                    let (bp0, bp1) = extend_line(p0, p1); // base extended
                    let (pq0, pq1) = extend_line(q0, q1); // parallel extended
                    // Fill between anchor-to-anchor region (not full extension)
                    let fill_pts = vec![
                        clamp_pt(p0), clamp_pt(p1), clamp_pt(q1), clamp_pt(q0)
                    ];
                    painter.add(egui::Shape::convex_polygon(fill_pts, hex_to_color(&d.color, d.opacity * 0.06), egui::Stroke::NONE));
                    // Base line + parallel line (full extension)
                    dashed_line(&painter, clamp_pt(bp0), clamp_pt(bp1), sc, ls);
                    dashed_line(&painter, clamp_pt(pq0), clamp_pt(pq1), sc, ls);
                    // Standard channel subdivision lines (TradingView style):
                    // -0.25, 0.25, 0.5, 0.75, 1.25 (0 and 1 are base/parallel already drawn)
                    if !is_fib_chan {
                        let subdivisions: &[(f32, u8, LineStyle)] = &[
                            (-0.25, 50, LineStyle::Dashed),  // breakout extension below
                            (0.25,  70, LineStyle::Dotted),  // quarter
                            (0.5,   90, LineStyle::Dashed),  // midline
                            (0.75,  70, LineStyle::Dotted),  // three-quarter
                            (1.25,  50, LineStyle::Dashed),  // breakout extension above
                        ];
                        for &(ratio, alpha, sub_ls) in subdivisions {
                            let s0 = egui::pos2(p0.x, p0.y + (q0.y - p0.y) * ratio);
                            let s1 = egui::pos2(p1.x, p1.y + (q1.y - p1.y) * ratio);
                            let (es0, es1) = extend_line(s0, s1);
                            let is_ext = ratio < 0.0 || ratio > 1.0;
                            let sub_sc = egui::Stroke::new(
                                d.thickness * if is_ext { 0.4 } else { 0.5 },
                                color_alpha(dc, alpha));
                            dashed_line(&painter, clamp_pt(es0), clamp_pt(es1), sub_sc, sub_ls);
                        }
                    }
                    // Fibonacci channel: internal lines at fib ratios + extensions
                    if is_fib_chan {
                        let fib_ratios: &[(f32, u8)] = &[
                            (0.236, 70), (0.382, 90), (0.5, 100), (0.618, 90), (0.786, 70),
                            // Extensions beyond the channel
                            (1.272, 50), (1.618, 50), (2.0, 40), (2.618, 35),
                            (-0.272, 50), (-0.618, 50),
                        ];
                        for &(ratio, alpha) in fib_ratios {
                            let f0 = egui::pos2(p0.x, p0.y + (q0.y - p0.y) * ratio);
                            let f1 = egui::pos2(p1.x, p1.y + (q1.y - p1.y) * ratio);
                            let (ef0, ef1) = extend_line(f0, f1);
                            let is_ext = ratio < 0.0 || ratio > 1.0;
                            let fib_sc = egui::Stroke::new(
                                d.thickness * if is_ext { 0.4 } else { 0.5 },
                                color_alpha(dc, alpha));
                            let fib_ls = if is_ext { LineStyle::Dashed } else { LineStyle::Dotted };
                            dashed_line(&painter, clamp_pt(ef0), clamp_pt(ef1), fib_sc, fib_ls);
                            // Label on right edge
                            let label_y = ef1.y.clamp(rect.top() + pt, rect.top() + pt + ch);
                            painter.text(egui::pos2(chart_right + 4.0, label_y), egui::Align2::LEFT_CENTER,
                                &format!("{:.1}%", ratio * 100.0), egui::FontId::monospace(7.5), color_alpha(dc, alpha));
                        }
                    }
                    if is_sel {
                        painter.circle_filled(p0, 5.0, egui::Color32::from_rgb(74,158,255));
                        painter.circle_stroke(p0, 5.0, egui::Stroke::new(1.0, egui::Color32::WHITE));
                        painter.circle_filled(p1, 5.0, egui::Color32::from_rgb(74,158,255));
                        painter.circle_stroke(p1, 5.0, egui::Stroke::new(1.0, egui::Color32::WHITE));
                        let qm = egui::pos2((q0.x+q1.x)/2.0, (q0.y+q1.y)/2.0);
                        painter.circle_filled(qm, 4.0, egui::Color32::from_rgb(74,158,255));
                    }
                }
            }
            DrawingKind::Pitchfork{price0,time0,price1,time1,price2,time2} => {
                let p0 = egui::pos2(bx(SignalDrawing::time_to_bar(*time0, &chart.timestamps)), py(*price0));
                let p1 = egui::pos2(bx(SignalDrawing::time_to_bar(*time1, &chart.timestamps)), py(*price1));
                let p2 = egui::pos2(bx(SignalDrawing::time_to_bar(*time2, &chart.timestamps)), py(*price2));
                if !in_bounds(p0) && !in_bounds(p1) && !in_bounds(p2) { continue; }
                let mid = egui::pos2((p1.x+p2.x)/2.0, (p1.y+p2.y)/2.0);
                let chart_left = rect.left(); let chart_right = rect.left() + cw;
                let extend_line = |a: egui::Pos2, b: egui::Pos2| -> (egui::Pos2, egui::Pos2) {
                    let ldx = b.x - a.x;
                    if ldx.abs() < 0.001 { return (a, b); }
                    let slope = (b.y - a.y) / ldx;
                    (egui::pos2(chart_left, a.y + slope * (chart_left - a.x)),
                     egui::pos2(chart_right, a.y + slope * (chart_right - a.x)))
                };
                let (em0, em1) = extend_line(p0, mid);
                let dy_per_dx = if (mid.x - p0.x).abs() > 0.001 { (mid.y - p0.y) / (mid.x - p0.x) } else { 0.0 };
                let up1 = egui::pos2(chart_left, p1.y + dy_per_dx * (chart_left - p1.x));
                let up2 = egui::pos2(chart_right, p1.y + dy_per_dx * (chart_right - p1.x));
                let lp1 = egui::pos2(chart_left, p2.y + dy_per_dx * (chart_left - p2.x));
                let lp2 = egui::pos2(chart_right, p2.y + dy_per_dx * (chart_right - p2.x));
                let fill_pts = vec![clamp_pt(up1), clamp_pt(up2), clamp_pt(lp2), clamp_pt(lp1)];
                painter.add(egui::Shape::convex_polygon(fill_pts, hex_to_color(&d.color, d.opacity * 0.05), egui::Stroke::NONE));
                dashed_line(&painter, clamp_pt(em0), clamp_pt(em1), sc, ls);
                dashed_line(&painter, clamp_pt(up1), clamp_pt(up2), egui::Stroke::new(d.thickness * 0.8, color_alpha(dc, 180)), ls);
                dashed_line(&painter, clamp_pt(lp1), clamp_pt(lp2), egui::Stroke::new(d.thickness * 0.8, color_alpha(dc, 180)), ls);
                let h05_1 = egui::pos2(chart_left, (up1.y + em0.y) / 2.0);
                let h05_2 = egui::pos2(chart_right, (up2.y + em1.y) / 2.0);
                let l05_1 = egui::pos2(chart_left, (lp1.y + em0.y) / 2.0);
                let l05_2 = egui::pos2(chart_right, (lp2.y + em1.y) / 2.0);
                dashed_line(&painter, clamp_pt(h05_1), clamp_pt(h05_2), egui::Stroke::new(d.thickness * 0.4, color_alpha(dc, 80)), LineStyle::Dotted);
                dashed_line(&painter, clamp_pt(l05_1), clamp_pt(l05_2), egui::Stroke::new(d.thickness * 0.4, color_alpha(dc, 80)), LineStyle::Dotted);
                painter.line_segment([clamp_pt(p1), clamp_pt(p2)], egui::Stroke::new(d.thickness * 0.5, color_alpha(dc, 100)));
                if is_sel {
                    for &pt in &[p0, p1, p2] {
                        painter.circle_filled(pt, 5.0, egui::Color32::from_rgb(74,158,255));
                        painter.circle_stroke(pt, 5.0, egui::Stroke::new(1.0, egui::Color32::WHITE));
                    }
                }
            }
            DrawingKind::GannFan{price0,time0,price1,time1} => {
                let p0 = egui::pos2(bx(SignalDrawing::time_to_bar(*time0, &chart.timestamps)), py(*price0));
                let p1 = egui::pos2(bx(SignalDrawing::time_to_bar(*time1, &chart.timestamps)), py(*price1));
                if !in_bounds(p0) { continue; }
                let chart_right = rect.left() + cw;
                let ref_dx = p1.x - p0.x; let ref_dy = p1.y - p0.y;
                if ref_dx.abs() < 0.1 { continue; }
                let fans: &[(f32, u8, &str)] = &[
                    (8.0, 50, "1x8"), (4.0, 60, "1x4"), (3.0, 70, "1x3"), (2.0, 90, "1x2"),
                    (1.0, 200, "1x1"),
                    (0.5, 90, "2x1"), (1.0/3.0, 70, "3x1"), (0.25, 60, "4x1"), (0.125, 50, "8x1"),
                ];
                for &(ratio, alpha, label) in fans {
                    let slope = ref_dy / ref_dx * ratio;
                    let right_y = p0.y + slope * (chart_right - p0.x);
                    let end = egui::pos2(chart_right, right_y);
                    let thick = if (ratio - 1.0).abs() < 0.01 { d.thickness } else { d.thickness * 0.6 };
                    let fan_ls = if (ratio - 1.0).abs() < 0.01 { ls } else { LineStyle::Dashed };
                    dashed_line(&painter, clamp_pt(p0), clamp_pt(end), egui::Stroke::new(thick, color_alpha(dc, alpha)), fan_ls);
                    let label_y = right_y.clamp(rect.top() + pt, rect.top() + pt + ch);
                    painter.text(egui::pos2(chart_right + 4.0, label_y), egui::Align2::LEFT_CENTER,
                        label, egui::FontId::monospace(7.5), color_alpha(dc, alpha));
                }
                if is_sel {
                    painter.circle_filled(p0, 5.0, egui::Color32::from_rgb(74,158,255));
                    painter.circle_filled(p1, 5.0, egui::Color32::from_rgb(74,158,255));
                }
            }
            DrawingKind::RegressionChannel{time0,time1} => {
                let bar0 = SignalDrawing::time_to_bar(*time0, &chart.timestamps);
                let bar1 = SignalDrawing::time_to_bar(*time1, &chart.timestamps);
                let start_idx = (bar0.round() as isize).max(0) as usize;
                let end_idx = (bar1.round() as isize).max(0).min(chart.bars.len() as isize - 1) as usize;
                if end_idx <= start_idx + 1 { continue; }
                let n_reg = end_idx - start_idx + 1;
                let mut sx = 0.0_f64; let mut sy = 0.0_f64; let mut sxx = 0.0_f64; let mut sxy = 0.0_f64;
                for i in start_idx..=end_idx {
                    let x = (i - start_idx) as f64;
                    let y = chart.bars[i].close as f64;
                    sx += x; sy += y; sxx += x*x; sxy += x*y;
                }
                let nf = n_reg as f64;
                let denom = nf * sxx - sx * sx;
                if denom.abs() < 1e-10 { continue; }
                let slope = (nf * sxy - sx * sy) / denom;
                let intercept = (sy - slope * sx) / nf;
                let mut ss = 0.0_f64;
                for i in start_idx..=end_idx {
                    let x = (i - start_idx) as f64;
                    ss += (chart.bars[i].close as f64 - (intercept + slope * x)).powi(2);
                }
                let sigma = (ss / nf).sqrt() as f32;
                let reg_pts: Vec<egui::Pos2> = (start_idx..=end_idx).map(|i| {
                    let predicted = (intercept + slope * (i - start_idx) as f64) as f32;
                    egui::pos2(bx(i as f32), py(predicted))
                }).collect();
                if reg_pts.len() > 1 { painter.add(egui::Shape::line(reg_pts.clone(), sc)); }
                for &(sig, alpha) in &[(sigma, 120u8), (sigma * 2.0, 70u8)] {
                    let upper: Vec<egui::Pos2> = (start_idx..=end_idx).map(|i| {
                        let pred = (intercept + slope * (i - start_idx) as f64) as f32;
                        egui::pos2(bx(i as f32), py(pred + sig))
                    }).collect();
                    let lower: Vec<egui::Pos2> = (start_idx..=end_idx).map(|i| {
                        let pred = (intercept + slope * (i - start_idx) as f64) as f32;
                        egui::pos2(bx(i as f32), py(pred - sig))
                    }).collect();
                    let band_sc = egui::Stroke::new(d.thickness * 0.6, color_alpha(dc, alpha));
                    if upper.len() > 1 { painter.add(egui::Shape::line(upper, band_sc)); }
                    if lower.len() > 1 { painter.add(egui::Shape::line(lower, band_sc)); }
                }
                // 1σ shaded band
                if reg_pts.len() > 1 {
                    let mut fill_pts: Vec<egui::Pos2> = (start_idx..=end_idx).map(|i| {
                        let pred = (intercept + slope * (i - start_idx) as f64) as f32;
                        egui::pos2(bx(i as f32), py(pred + sigma))
                    }).collect();
                    let lower_rev: Vec<egui::Pos2> = (start_idx..=end_idx).rev().map(|i| {
                        let pred = (intercept + slope * (i - start_idx) as f64) as f32;
                        egui::pos2(bx(i as f32), py(pred - sigma))
                    }).collect();
                    fill_pts.extend(lower_rev);
                    painter.add(egui::Shape::convex_polygon(fill_pts, hex_to_color(&d.color, d.opacity * 0.06), egui::Stroke::NONE));
                }
                if is_sel {
                    if let Some(&fp) = reg_pts.first() { painter.circle_filled(fp, 5.0, egui::Color32::from_rgb(74,158,255)); }
                    if let Some(&lp) = reg_pts.last()  { painter.circle_filled(lp, 5.0, egui::Color32::from_rgb(74,158,255)); }
                }
            }
            DrawingKind::XABCD{points} if points.len() >= 2 => {
                let pts: Vec<egui::Pos2> = points.iter().map(|&(t, p)| {
                    egui::pos2(bx(SignalDrawing::time_to_bar(t, &chart.timestamps)), py(p))
                }).collect();
                let labels = ["X","A","B","C","D"];
                for i in 0..pts.len().saturating_sub(1) {
                    if in_bounds(pts[i]) || in_bounds(pts[i+1]) {
                        dashed_line(&painter, clamp_pt(pts[i]), clamp_pt(pts[i+1]), sc, ls);
                    }
                }
                if pts.len() >= 5 {
                    let fill = vec![clamp_pt(pts[0]), clamp_pt(pts[1]), clamp_pt(pts[4])];
                    painter.add(egui::Shape::convex_polygon(fill, hex_to_color(&d.color, d.opacity * 0.07), egui::Stroke::NONE));
                    let xa = (pts[1].y - pts[0].y).abs();
                    let ab = (pts[2].y - pts[1].y).abs();
                    let bc = (pts[3].y - pts[2].y).abs();
                    let ad = (pts[4].y - pts[1].y).abs();
                    if xa > 0.1 {
                        let ab_xa = ab / xa; let bc_ab = if ab > 0.1 { bc / ab } else { 0.0 }; let ad_xa = ad / xa;
                        if in_bounds(pts[2]) { painter.text(pts[2] + egui::vec2(4.0,-4.0), egui::Align2::LEFT_BOTTOM, &format!("{:.3}", ab_xa), egui::FontId::monospace(7.5), color_alpha(dc, 160)); }
                        if in_bounds(pts[3]) { painter.text(pts[3] + egui::vec2(4.0,-4.0), egui::Align2::LEFT_BOTTOM, &format!("{:.3}", bc_ab), egui::FontId::monospace(7.5), color_alpha(dc, 160)); }
                        if in_bounds(pts[4]) { painter.text(pts[4] + egui::vec2(4.0,-4.0), egui::Align2::LEFT_BOTTOM, &format!("{:.3}", ad_xa), egui::FontId::monospace(7.5), color_alpha(dc, 160)); }
                    }
                }
                for (i, &pt) in pts.iter().enumerate() {
                    if in_bounds(pt) {
                        painter.circle_filled(pt, 4.0, dc);
                        let lbl = labels.get(i).copied().unwrap_or("?");
                        painter.text(pt + egui::vec2(0.0, -10.0), egui::Align2::CENTER_CENTER, lbl, egui::FontId::monospace(9.0), dc);
                    }
                }
                if is_sel { for &pt in &pts { if in_bounds(pt) { painter.circle_stroke(pt, 6.0, egui::Stroke::new(1.5, egui::Color32::WHITE)); } } }
            }
            DrawingKind::ElliottWave{points,wave_type} if !points.is_empty() => {
                let impulse_labels = ["1","2","3","4","5"];
                let corrective_labels = ["A","B","C"];
                let wxy_labels = ["W","X","Y"];
                let wxyxz_labels = ["W","X","Y","X","Z"];
                let sub_impulse_labels = ["i","ii","iii","iv","v"];
                let sub_corrective_labels = ["a","b","c"];
                let labels: &[&str] = match *wave_type {
                    0 => &impulse_labels,
                    1 => &corrective_labels,
                    2 => &wxy_labels,
                    3 => &wxyxz_labels,
                    4 => &sub_impulse_labels,
                    5 => &sub_corrective_labels,
                    _ => &corrective_labels,
                };
                let pts: Vec<egui::Pos2> = points.iter().map(|&(t, p)| {
                    egui::pos2(bx(SignalDrawing::time_to_bar(t, &chart.timestamps)), py(p))
                }).collect();
                for i in 0..pts.len().saturating_sub(1) {
                    if in_bounds(pts[i]) || in_bounds(pts[i+1]) {
                        dashed_line(&painter, clamp_pt(pts[i]), clamp_pt(pts[i+1]), sc, ls);
                    }
                }
                for (i, &pt) in pts.iter().enumerate() {
                    if in_bounds(pt) {
                        painter.circle_filled(pt, 7.0, hex_to_color(&d.color, d.opacity * 0.4));
                        painter.circle_stroke(pt, 7.0, sc);
                        let lbl = labels.get(i).copied().unwrap_or("?");
                        painter.text(pt, egui::Align2::CENTER_CENTER, lbl, egui::FontId::monospace(7.5), egui::Color32::WHITE);
                    }
                }
                if is_sel { for &pt in &pts { if in_bounds(pt) { painter.circle_stroke(pt, 9.0, egui::Stroke::new(1.5, egui::Color32::WHITE)); } } }
            }
            DrawingKind::AnchoredVWAP{time} => {
                let anchor_bar = SignalDrawing::time_to_bar(*time, &chart.timestamps);
                let start_idx = (anchor_bar.round() as isize).max(0) as usize;
                if start_idx >= chart.bars.len() { continue; }
                let vwap_color = egui::Color32::from_rgb(180, 100, 255);
                let vwap_sc = egui::Stroke::new(d.thickness, if is_sel { egui::Color32::WHITE } else { vwap_color });
                let mut cum_tp_vol = 0.0_f64;
                let mut cum_vol = 0.0_f64;
                let mut pts = Vec::new();
                for i in start_idx..chart.bars.len() {
                    let b = &chart.bars[i];
                    let tp = (b.high + b.low + b.close) as f64 / 3.0;
                    cum_tp_vol += tp * b.volume as f64;
                    cum_vol += b.volume as f64;
                    if cum_vol > 0.0 {
                        let vwap = (cum_tp_vol / cum_vol) as f32;
                        let sx = bx(i as f32); let sy = py(vwap);
                        if sy.is_finite() && sy.abs() < 50000.0 { pts.push(egui::pos2(sx, sy)); }
                    }
                }
                if pts.len() > 1 { painter.add(egui::Shape::line(pts, vwap_sc)); }
                let ax = bx(anchor_bar);
                painter.line_segment([egui::pos2(ax, rect.top()+pt), egui::pos2(ax, rect.top()+pt+ch)],
                    egui::Stroke::new(0.5, color_alpha(vwap_color, 60)));
                if is_sel {
                    let anchor_close = chart.bars.get(start_idx).map(|b| b.close).unwrap_or(0.0);
                    painter.circle_filled(egui::pos2(ax, py(anchor_close)), 5.0, vwap_color);
                }
            }
            DrawingKind::PriceRange{price0,time0,price1,time1} => {
                let p0 = egui::pos2(bx(SignalDrawing::time_to_bar(*time0, &chart.timestamps)), py(*price0));
                let p1 = egui::pos2(bx(SignalDrawing::time_to_bar(*time1, &chart.timestamps)), py(*price1));
                if !in_bounds(p0) && !in_bounds(p1) { continue; }
                let xl = p0.x.min(p1.x); let xr = p0.x.max(p1.x);
                let yt = p0.y.min(p1.y); let yb = p0.y.max(p1.y);
                painter.rect_filled(egui::Rect::from_min_max(egui::pos2(xl,yt), egui::pos2(xr,yb)),
                    0.0, hex_to_color(&d.color, d.opacity * 0.08));
                painter.rect_stroke(egui::Rect::from_min_max(egui::pos2(xl,yt), egui::pos2(xr,yb)),
                    0.0, sc, egui::StrokeKind::Outside);
                let dp_range = *price1 - *price0;
                let pct = if *price0 != 0.0 { dp_range / *price0 * 100.0 } else { 0.0 };
                let b0i = SignalDrawing::time_to_bar(*time0, &chart.timestamps).round() as isize;
                let b1i = SignalDrawing::time_to_bar(*time1, &chart.timestamps).round() as isize;
                let bar_count = (b1i - b0i).abs();
                let stats = format!("{:+.2}  {:.2}%  {}bars", dp_range, pct, bar_count);
                painter.text(egui::pos2(xl + (xr-xl)*0.5, yt + (yb-yt)*0.5), egui::Align2::CENTER_CENTER,
                    &stats, egui::FontId::monospace(9.0), color_alpha(dc, 200));
                if is_sel {
                    painter.circle_filled(p0, 5.0, egui::Color32::from_rgb(74,158,255));
                    painter.circle_filled(p1, 5.0, egui::Color32::from_rgb(74,158,255));
                }
            }
            DrawingKind::RiskReward{entry_price,entry_time,stop_price,target_price} => {
                let ex = bx(SignalDrawing::time_to_bar(*entry_time, &chart.timestamps));
                let chart_right = rect.left() + cw;
                let ey = py(*entry_price); let sy = py(*stop_price); let ty = py(*target_price);
                if !ey.is_finite() || !sy.is_finite() || !ty.is_finite() { continue; }
                painter.rect_filled(egui::Rect::from_min_max(egui::pos2(ex, ey.min(ty)), egui::pos2(chart_right, ey.max(ty))),
                    0.0, egui::Color32::from_rgba_unmultiplied(46, 204, 113, 30));
                painter.rect_filled(egui::Rect::from_min_max(egui::pos2(ex, ey.min(sy)), egui::pos2(chart_right, ey.max(sy))),
                    0.0, egui::Color32::from_rgba_unmultiplied(231, 76, 60, 30));
                painter.line_segment([egui::pos2(ex, ey), egui::pos2(chart_right, ey)],
                    egui::Stroke::new(d.thickness, color_alpha(dc, 200)));
                painter.line_segment([egui::pos2(ex, sy), egui::pos2(chart_right, sy)],
                    egui::Stroke::new(d.thickness * 0.8, egui::Color32::from_rgb(231, 76, 60)));
                painter.line_segment([egui::pos2(ex, ty), egui::pos2(chart_right, ty)],
                    egui::Stroke::new(d.thickness * 0.8, egui::Color32::from_rgb(46, 204, 113)));
                let reward = (*target_price - *entry_price).abs();
                let risk   = (*entry_price - *stop_price).abs();
                if risk > 0.0 {
                    let rr = reward / risk;
                    painter.text(egui::pos2(chart_right - 4.0, ey.min(ty) + (ey - ty).abs() * 0.5),
                        egui::Align2::RIGHT_CENTER, &format!("{:.2}:1 R", rr), egui::FontId::monospace(9.0), egui::Color32::from_rgb(46,204,113));
                    painter.text(egui::pos2(chart_right - 4.0, ey.min(sy) + (ey - sy).abs() * 0.5),
                        egui::Align2::RIGHT_CENTER, &format!("-{:.2} ({:.2}%)", risk, risk / *entry_price * 100.0),
                        egui::FontId::monospace(8.0), egui::Color32::from_rgb(231,76,60));
                }
                if is_sel { painter.circle_filled(egui::pos2(ex, ey), 5.0, egui::Color32::from_rgb(74,158,255)); }
            }
            DrawingKind::VerticalLine{time} => {
                let x = bx(SignalDrawing::time_to_bar(*time, &chart.timestamps));
                if x.is_finite() && x >= rect.left() && x <= rect.left() + cw {
                    dashed_line(&painter, egui::pos2(x, rect.top()+pt), egui::pos2(x, rect.top()+pt+ch), sc, LineStyle::Dashed);
                    let ts_label = *time;
                    let dt = chrono::NaiveDateTime::from_timestamp_opt(ts_label, 0).map(|d| d.format("%m/%d %H:%M").to_string()).unwrap_or_default();
                    painter.text(egui::pos2(x + 3.0, rect.top()+pt+4.0), egui::Align2::LEFT_TOP,
                        &dt, egui::FontId::monospace(7.5), color_alpha(dc, 180));
                    if is_sel { painter.circle_filled(egui::pos2(x, rect.top()+pt+ch*0.5), 4.0, egui::Color32::from_rgb(74,158,255)); }
                }
            }
            DrawingKind::Ray{price0,time0,price1,time1} => {
                let p0 = egui::pos2(bx(SignalDrawing::time_to_bar(*time0, &chart.timestamps)), py(*price0));
                let p1 = egui::pos2(bx(SignalDrawing::time_to_bar(*time1, &chart.timestamps)), py(*price1));
                if p0.x.is_finite() && p1.x.is_finite() {
                    let chart_left = rect.left();
                    let chart_right = rect.left() + cw;
                    let dx = p1.x - p0.x;
                    let (mut draw_a, mut draw_b) = (p0, p1);
                    if dx.abs() > 0.001 {
                        let slope = (p1.y - p0.y) / dx;
                        if d.extend_left {
                            let left_y = p0.y + slope * (chart_left - p0.x);
                            draw_a = egui::pos2(chart_left, left_y);
                        }
                        if d.extend_right {
                            let right_y = p0.y + slope * (chart_right - p0.x);
                            draw_b = egui::pos2(chart_right, right_y);
                        }
                    }
                    dashed_line(&painter, clamp_pt(draw_a), clamp_pt(draw_b), sc, ls);
                    if is_sel {
                        painter.circle_filled(clamp_pt(p0), 5.0, egui::Color32::from_rgb(74,158,255));
                        painter.circle_filled(clamp_pt(p1), 5.0, egui::Color32::from_rgb(74,158,255));
                    }
                }
            }
            DrawingKind::FibExtension{price0,time0,price1,time1,price2,time2} => {
                let x0 = bx(SignalDrawing::time_to_bar(*time0, &chart.timestamps));
                let x1 = bx(SignalDrawing::time_to_bar(*time1, &chart.timestamps));
                let x2 = bx(SignalDrawing::time_to_bar(*time2, &chart.timestamps));
                let chart_right = rect.left() + cw;
                // Draw A→B and B→C construction lines
                let p0s = egui::pos2(x0, py(*price0));
                let p1s = egui::pos2(x1, py(*price1));
                let p2s = egui::pos2(x2, py(*price2));
                let con_sc = egui::Stroke::new(d.thickness * 0.6, color_alpha(dc, 100));
                dashed_line(&painter, clamp_pt(p0s), clamp_pt(p1s), con_sc, LineStyle::Dashed);
                dashed_line(&painter, clamp_pt(p1s), clamp_pt(p2s), con_sc, LineStyle::Dashed);
                // AB range for projection
                let ab_range = *price1 - *price0;
                // Direction: if A<B (up) and C<B (pullback), targets project up; else down
                let dir = if ab_range >= 0.0 { 1.0_f32 } else { -1.0_f32 };
                let levels: &[(f32, &str)] = &[
                    (0.0, "0%"), (0.618, "61.8%"), (1.0, "100%"),
                    (1.272, "127.2%"), (1.618, "161.8%"), (2.0, "200%"), (2.618, "261.8%"),
                ];
                for &(ratio, label) in levels {
                    let lp = *price2 + dir * ratio * ab_range.abs();
                    let y = py(lp);
                    if y.is_finite() && y.abs() < 50000.0 {
                        let alpha = if ratio == 0.0 || ratio == 1.0 { 220u8 } else if ratio <= 1.618 { 160 } else { 100 };
                        let lsc = egui::Stroke::new(d.thickness * 0.7, color_alpha(dc, alpha));
                        dashed_line(&painter, egui::pos2(x2.min(chart_right), y), egui::pos2(chart_right, y), lsc, LineStyle::Solid);
                        painter.text(egui::pos2(chart_right + 3.0, y), egui::Align2::LEFT_CENTER,
                            &format!("{} {:.2}", label, lp), egui::FontId::monospace(7.5), color_alpha(dc, alpha));
                    }
                }
                if is_sel {
                    for &pt in &[p0s, p1s, p2s] { painter.circle_filled(clamp_pt(pt), 5.0, egui::Color32::from_rgb(74,158,255)); }
                }
            }
            DrawingKind::FibTimeZone{time} => {
                let anchor_bar = SignalDrawing::time_to_bar(*time, &chart.timestamps);
                let fib_nums: &[u32] = &[1, 1, 2, 3, 5, 8, 13, 21, 34, 55, 89];
                let chart_right = rect.left() + cw;
                let mut seen_bars: std::collections::HashSet<u32> = std::collections::HashSet::new();
                for (idx, &fib) in fib_nums.iter().enumerate() {
                    if seen_bars.contains(&fib) { continue; }
                    seen_bars.insert(fib);
                    let target_bar = anchor_bar + fib as f32;
                    let x = bx(target_bar);
                    if x < rect.left() || x > chart_right { continue; }
                    let alpha = (220_u8).saturating_sub((idx as u8) * 18);
                    let zone_sc = egui::Stroke::new(d.thickness * 0.7, color_alpha(dc, alpha));
                    dashed_line(&painter, egui::pos2(x, rect.top()+pt), egui::pos2(x, rect.top()+pt+ch), zone_sc, LineStyle::Dashed);
                    painter.text(egui::pos2(x + 2.0, rect.top()+pt+4.0), egui::Align2::LEFT_TOP,
                        &fib.to_string(), egui::FontId::monospace(7.5), color_alpha(dc, alpha));
                }
                // Anchor line
                let ax = bx(anchor_bar);
                if ax.is_finite() && ax >= rect.left() && ax <= chart_right {
                    dashed_line(&painter, egui::pos2(ax, rect.top()+pt), egui::pos2(ax, rect.top()+pt+ch),
                        egui::Stroke::new(d.thickness, color_alpha(dc, 200)), LineStyle::Solid);
                }
                if is_sel { painter.circle_filled(egui::pos2(ax, rect.top()+pt+ch*0.5), 4.0, egui::Color32::from_rgb(74,158,255)); }
            }
            DrawingKind::FibArc{price0,time0,price1,time1} => {
                let p0 = egui::pos2(bx(SignalDrawing::time_to_bar(*time0, &chart.timestamps)), py(*price0));
                let p1 = egui::pos2(bx(SignalDrawing::time_to_bar(*time1, &chart.timestamps)), py(*price1));
                if !p0.x.is_finite() || !p1.x.is_finite() { continue; }
                let dist = p0.distance(p1);
                if dist < 1.0 { continue; }
                let ratios: &[(f32, &str)] = &[
                    (0.236, "23.6%"), (0.382, "38.2%"), (0.5, "50%"),
                    (0.618, "61.8%"), (0.786, "78.6%"), (1.0, "100%"),
                ];
                // Draw arcs centered at p1, curving on the left side
                for &(ratio, label) in ratios {
                    let r = dist * ratio;
                    let alpha = if ratio >= 0.618 { 200u8 } else { 120 };
                    let arc_color = color_alpha(dc, alpha);
                    // Approximate arc with line segments (semicircle on left half)
                    let n_seg = 40;
                    let mut arc_pts: Vec<egui::Pos2> = Vec::with_capacity(n_seg + 1);
                    for k in 0..=n_seg {
                        let angle = std::f32::consts::PI * (k as f32 / n_seg as f32) + std::f32::consts::FRAC_PI_2;
                        let ax = p1.x + r * angle.cos();
                        let ay = p1.y + r * angle.sin();
                        let apt = egui::pos2(ax, ay);
                        if apt.x >= rect.left() && apt.x <= rect.left()+cw && apt.y >= rect.top()+pt && apt.y <= rect.top()+pt+ch {
                            arc_pts.push(clamp_pt(apt));
                        }
                    }
                    if arc_pts.len() > 1 {
                        painter.add(egui::Shape::line(arc_pts, egui::Stroke::new(d.thickness * 0.7, arc_color)));
                    }
                    // Label at the leftmost edge of the arc
                    let lx = p1.x - r;
                    let label_y = p1.y.clamp(rect.top()+pt, rect.top()+pt+ch);
                    if lx >= rect.left() { painter.text(egui::pos2(lx - 3.0, label_y), egui::Align2::RIGHT_CENTER, label, egui::FontId::monospace(7.5), arc_color); }
                }
                if is_sel {
                    painter.circle_filled(clamp_pt(p0), 5.0, egui::Color32::from_rgb(74,158,255));
                    painter.circle_filled(clamp_pt(p1), 5.0, egui::Color32::from_rgb(74,158,255));
                }
            }
            DrawingKind::GannBox{price0,time0,price1,time1} => {
                let p0 = egui::pos2(bx(SignalDrawing::time_to_bar(*time0, &chart.timestamps)), py(*price0));
                let p1 = egui::pos2(bx(SignalDrawing::time_to_bar(*time1, &chart.timestamps)), py(*price1));
                if !p0.x.is_finite() || !p1.x.is_finite() { continue; }
                let xl = p0.x.min(p1.x); let xr = p0.x.max(p1.x);
                let yt = p0.y.min(p1.y); let yb = p0.y.max(p1.y);
                let pw = xr - xl; let ph = yb - yt;
                // Outer box
                painter.rect_stroke(egui::Rect::from_min_max(egui::pos2(xl,yt), egui::pos2(xr,yb)),
                    0.0, sc, egui::StrokeKind::Outside);
                painter.rect_filled(egui::Rect::from_min_max(egui::pos2(xl,yt), egui::pos2(xr,yb)),
                    0.0, hex_to_color(&d.color, d.opacity * 0.04));
                let sub_sc = egui::Stroke::new(d.thickness * 0.5, color_alpha(dc, 80));
                let dot_sc = egui::Stroke::new(d.thickness * 0.4, color_alpha(dc, 50));
                // Horizontal divisions at 25%, 50%, 75%
                for &frac in &[0.25_f32, 0.5, 0.75] {
                    let y = yt + ph * frac;
                    dashed_line(&painter, egui::pos2(xl, y), egui::pos2(xr, y), if frac == 0.5 { sub_sc } else { dot_sc }, LineStyle::Dotted);
                }
                // Vertical divisions at 25%, 50%, 75%
                for &frac in &[0.25_f32, 0.5, 0.75] {
                    let x = xl + pw * frac;
                    dashed_line(&painter, egui::pos2(x, yt), egui::pos2(x, yb), if frac == 0.5 { sub_sc } else { dot_sc }, LineStyle::Dotted);
                }
                // Main diagonals
                let diag_sc = egui::Stroke::new(d.thickness * 0.7, color_alpha(dc, 140));
                dashed_line(&painter, egui::pos2(xl, yt), egui::pos2(xr, yb), diag_sc, LineStyle::Solid);
                dashed_line(&painter, egui::pos2(xl, yb), egui::pos2(xr, yt), diag_sc, LineStyle::Solid);
                // Gann angle diagonals from the starting corner (top-left if P0 is top-left)
                let (corner_x, corner_y) = (p0.x, p0.y);
                let angles_sc = egui::Stroke::new(d.thickness * 0.5, color_alpha(dc, 70));
                // 1x2 and 2x1 angles from corner
                dashed_line(&painter, clamp_pt(egui::pos2(corner_x, corner_y)), clamp_pt(egui::pos2(xr, corner_y + ph * 0.5)), angles_sc, LineStyle::Dashed);
                dashed_line(&painter, clamp_pt(egui::pos2(corner_x, corner_y)), clamp_pt(egui::pos2(corner_x + pw * 0.5, yb)), angles_sc, LineStyle::Dashed);
                if is_sel {
                    painter.circle_filled(p0, 5.0, egui::Color32::from_rgb(74,158,255));
                    painter.circle_filled(p1, 5.0, egui::Color32::from_rgb(74,158,255));
                }
            }
            // Partial XABCD/Elliott with < 2 pts — nothing to draw yet
            DrawingKind::XABCD{..} | DrawingKind::ElliottWave{..} => {}
            DrawingKind::TextNote { price, time, text, font_size } => {
                let x = bx(SignalDrawing::time_to_bar(*time, &chart.timestamps));
                let y = py(*price);
                if x.is_finite() && y.is_finite() && x.abs() < 50000.0 && y.abs() < 50000.0 {
                    painter.text(egui::pos2(x, y), egui::Align2::LEFT_TOP, text,
                        egui::FontId::proportional(*font_size), dc);
                    if is_sel {
                        let galley = painter.layout_no_wrap(text.clone(), egui::FontId::proportional(*font_size), dc);
                        let text_rect = egui::Rect::from_min_size(egui::pos2(x, y), galley.size());
                        painter.rect_stroke(text_rect, 2.0, egui::Stroke::new(1.0, egui::Color32::from_rgb(74,158,255)), egui::StrokeKind::Outside);
                    }
                }
            }
        }
        // Alert bell dot indicator
        if d.alert_enabled {
            let bell_pos = match &d.kind {
                DrawingKind::HLine { price } => Some(egui::pos2(rect.left() + 12.0, py(*price))),
                DrawingKind::TrendLine { time0, price0, .. } => Some(egui::pos2(bx(SignalDrawing::time_to_bar(*time0, &chart.timestamps)), py(*price0))),
                DrawingKind::HZone { price0, .. } => Some(egui::pos2(rect.left() + 12.0, py(*price0))),
                _ => None,
            };
            if let Some(bp) = bell_pos {
                if bp.x.is_finite() && bp.y.is_finite() && bp.x.abs() < 50000.0 && bp.y.abs() < 50000.0 {
                    painter.circle_filled(bp + egui::vec2(-8.0, -8.0), 3.0, egui::Color32::from_rgb(255, 193, 37));
                }
            }
        }
    }

    // ── Price labels at selected drawing anchors ─────────────────────────
    if let Some(ref sel_id) = chart.selected_id {
        if let Some(d) = chart.drawings.iter().find(|d| &d.id == sel_id) {
            let label_col = color_alpha(t.text,200);
            let label_bg = egui::Color32::from_rgba_unmultiplied(t.toolbar_bg.r(), t.toolbar_bg.g(), t.toolbar_bg.b(), 220);
            let font = egui::FontId::monospace(8.0);
            // Collect anchor prices to label
            let mut anchors: Vec<(f32, f32)> = vec![]; // (screen_x, price)
            match &d.kind {
                DrawingKind::HLine { price } => { anchors.push((rect.left() + cw, *price)); }
                DrawingKind::TrendLine { price0, time0, price1, time1 } | DrawingKind::Ray { price0, time0, price1, time1 } => {
                    anchors.push((bx(SignalDrawing::time_to_bar(*time0, &chart.timestamps)), *price0));
                    anchors.push((bx(SignalDrawing::time_to_bar(*time1, &chart.timestamps)), *price1));
                }
                DrawingKind::Fibonacci { price0, time0, price1, time1 } => {
                    anchors.push((bx(SignalDrawing::time_to_bar(*time0, &chart.timestamps)), *price0));
                    anchors.push((bx(SignalDrawing::time_to_bar(*time1, &chart.timestamps)), *price1));
                }
                DrawingKind::Channel { price0, time0, price1, time1, offset } | DrawingKind::FibChannel { price0, time0, price1, time1, offset } => {
                    anchors.push((bx(SignalDrawing::time_to_bar(*time0, &chart.timestamps)), *price0));
                    anchors.push((bx(SignalDrawing::time_to_bar(*time1, &chart.timestamps)), *price1));
                    anchors.push((bx(SignalDrawing::time_to_bar(*time0, &chart.timestamps)), *price0 + *offset));
                }
                DrawingKind::HZone { price0, price1 } => {
                    anchors.push((rect.left() + cw, *price0));
                    anchors.push((rect.left() + cw, *price1));
                }
                _ => {} // Other types: skip for now
            }
            for (sx, price) in &anchors {
                let sy = py(*price);
                if sy.is_finite() && sy.abs() < 50000.0 {
                    let d = if *price >= 10.0 { 2 } else { 4 };
                    let label = format!("{:.1$}", price, d);
                    let galley = painter.layout_no_wrap(label.clone(), font.clone(), label_col);
                    let lx = sx + 6.0;
                    let ly = sy - galley.size().y / 2.0;
                    painter.rect_filled(egui::Rect::from_min_size(egui::pos2(lx - 3.0, ly - 1.0), galley.size() + egui::vec2(6.0, 2.0)), 2.0, label_bg);
                    painter.text(egui::pos2(lx + galley.size().x / 2.0, sy), egui::Align2::CENTER_CENTER, &label, font.clone(), label_col);
                }
            }
        }
    }

    span_begin("oscillator_paint");
    // ── Oscillator sub-panel (RSI, MACD, Stochastic, CVD) ───────────────
    if needs_osc_panel && osc_h > 10.0 {
        let osc_top = rect.top() + pt + ch + 2.0;
        let osc_bottom = osc_top + osc_h - 4.0;
        let osc_height = osc_bottom - osc_top;

        // Separator line
        painter.line_segment([egui::pos2(rect.left(), osc_top - 1.0), egui::pos2(rect.left() + cw, osc_top - 1.0)],
            egui::Stroke::new(1.0, t.toolbar_border));

        for ind in &chart.indicators {
            if !ind.visible || ind.kind.category() != IndicatorCategory::Oscillator { continue; }
            let color = hex_to_color(&ind.color, 1.0);

            // Determine value range for this oscillator
            let (osc_min, osc_max) = match ind.kind {
                IndicatorType::RSI => (0.0_f32, 100.0),
                IndicatorType::Stochastic => (0.0, 100.0),
                IndicatorType::ADX => (0.0, 100.0),
                IndicatorType::WilliamsR => (-100.0, 0.0),
                IndicatorType::MACD => {
                    let mut lo = f32::MAX; let mut hi = f32::MIN;
                    for i in (vs as u32)..end {
                        if let Some(&v) = ind.values.get(i as usize) { if !v.is_nan() { lo = lo.min(v); hi = hi.max(v); } }
                        if let Some(&v) = ind.histogram.get(i as usize) { if !v.is_nan() { lo = lo.min(v); hi = hi.max(v); } }
                    }
                    if lo >= hi { lo -= 1.0; hi += 1.0; }
                    let pad = (hi - lo) * 0.1;
                    (lo - pad, hi + pad)
                }
                IndicatorType::CCI | IndicatorType::ATR => {
                    let mut lo = f32::MAX; let mut hi = f32::MIN;
                    for i in (vs as u32)..end {
                        if let Some(&v) = ind.values.get(i as usize) { if !v.is_nan() { lo = lo.min(v); hi = hi.max(v); } }
                    }
                    if lo >= hi { lo = if ind.kind == IndicatorType::ATR { 0.0 } else { -200.0 }; hi = hi.max(lo + 1.0); }
                    let pad = (hi - lo) * 0.1;
                    (lo - pad, hi + pad)
                }
                _ => (0.0, 100.0),
            };

            let osc_y = |v: f32| -> f32 { osc_top + (osc_max - v) / (osc_max - osc_min) * osc_height };

            // Reference lines for RSI/Stochastic (30/70 or 20/80)
            if ind.kind == IndicatorType::RSI || ind.kind == IndicatorType::Stochastic {
                let (low_ref, high_ref) = if ind.kind == IndicatorType::RSI { (30.0, 70.0) } else { (20.0, 80.0) };
                for &level in &[low_ref, 50.0, high_ref] {
                    let y = osc_y(level);
                    painter.line_segment([egui::pos2(rect.left(), y), egui::pos2(rect.left() + cw, y)],
                        egui::Stroke::new(0.3, t.dim.gamma_multiply(0.3)));
                }
                // Overbought/oversold zones
                painter.rect_filled(egui::Rect::from_min_max(
                    egui::pos2(rect.left(), osc_y(high_ref)), egui::pos2(rect.left() + cw, osc_y(osc_max))),
                    0.0, t.bear.gamma_multiply(0.04));
                painter.rect_filled(egui::Rect::from_min_max(
                    egui::pos2(rect.left(), osc_y(osc_min)), egui::pos2(rect.left() + cw, osc_y(low_ref))),
                    0.0, t.bull.gamma_multiply(0.04));

                // Enhanced RSI: gradient fill between line and 50 level
                if ind.kind == IndicatorType::RSI {
                    let fifty_y = osc_y(50.0);
                    for i in (vs as u32)..end.saturating_sub(1) {
                        if let (Some(&v0), Some(&v1)) = (ind.values.get(i as usize), ind.values.get((i+1) as usize)) {
                            if v0.is_nan() || v1.is_nan() { continue; }
                            let x0 = bx(i as f32); let x1 = bx((i+1) as f32);
                            let y0 = osc_y(v0); let y1 = osc_y(v1);
                            let above = (v0 + v1) / 2.0 > 50.0;
                            let dist = ((v0 + v1) / 2.0 - 50.0).abs() / 50.0;
                            let alpha = (dist * 40.0).min(30.0) as u8;
                            let fill_col = if above {
                                egui::Color32::from_rgba_unmultiplied(46, 204, 113, alpha)
                            } else {
                                egui::Color32::from_rgba_unmultiplied(231, 76, 60, alpha)
                            };
                            let pts = vec![egui::pos2(x0, y0), egui::pos2(x1, y1), egui::pos2(x1, fifty_y), egui::pos2(x0, fifty_y)];
                            painter.add(egui::Shape::convex_polygon(pts, fill_col, egui::Stroke::NONE));
                        }
                    }
                    // RSI value label on right edge
                    if let Some(&last_rsi) = ind.values.last() {
                        if !last_rsi.is_nan() {
                            let rsi_y = osc_y(last_rsi);
                            let rsi_col = if last_rsi > 70.0 { t.bear } else if last_rsi < 30.0 { t.bull } else { color };
                            painter.text(egui::pos2(rect.left() + cw + 3.0, rsi_y), egui::Align2::LEFT_CENTER,
                                &format!("{:.1}", last_rsi), egui::FontId::monospace(8.0), rsi_col);
                        }
                    }
                }
            }

            // ADX reference lines (20=no trend, 40=strong trend)
            if ind.kind == IndicatorType::ADX {
                for &level in &[20.0_f32, 40.0] {
                    let y = osc_y(level);
                    painter.line_segment([egui::pos2(rect.left(), y), egui::pos2(rect.left() + cw, y)],
                        egui::Stroke::new(0.3, t.dim.gamma_multiply(0.3)));
                }
            }

            // CCI reference lines (-100, 0, +100)
            if ind.kind == IndicatorType::CCI {
                for &level in &[-100.0_f32, 0.0, 100.0] {
                    let y = osc_y(level);
                    painter.line_segment([egui::pos2(rect.left(), y), egui::pos2(rect.left() + cw, y)],
                        egui::Stroke::new(0.3, t.dim.gamma_multiply(0.3)));
                }
            }

            // Williams %R reference lines (-80, -50, -20)
            if ind.kind == IndicatorType::WilliamsR {
                for &level in &[-80.0_f32, -50.0, -20.0] {
                    let y = osc_y(level);
                    painter.line_segment([egui::pos2(rect.left(), y), egui::pos2(rect.left() + cw, y)],
                        egui::Stroke::new(0.3, t.dim.gamma_multiply(0.3)));
                }
            }

            // Zero line for MACD
            if ind.kind == IndicatorType::MACD {
                let y = osc_y(0.0);
                painter.line_segment([egui::pos2(rect.left(), y), egui::pos2(rect.left() + cw, y)],
                    egui::Stroke::new(0.5, t.dim.gamma_multiply(0.3)));
            }

            // MACD histogram bars — GPU mesh batched (colored by direction of change)
            if ind.kind == IndicatorType::MACD && !ind.histogram.is_empty() {
                let zero_y = osc_y(0.0);
                let mut mesh = egui::Mesh::default();
                for i in (vs as u32)..end {
                    if let Some(&h) = ind.histogram.get(i as usize) {
                        if !h.is_nan() {
                            let prev_h = if i > 0 { ind.histogram.get(i as usize - 1).copied().unwrap_or(0.0) } else { 0.0 };
                            let prev_h = if prev_h.is_nan() { 0.0 } else { prev_h };
                            let x = bx(i as f32);
                            let y = osc_y(h);
                            let bw = (bs * 0.4).max(1.0);
                            let c = if h >= 0.0 {
                                if h >= prev_h { egui::Color32::from_rgba_unmultiplied(46, 204, 113, 200) }
                                else { egui::Color32::from_rgba_unmultiplied(46, 204, 113, 80) }
                            } else {
                                if h <= prev_h { egui::Color32::from_rgba_unmultiplied(231, 76, 60, 200) }
                                else { egui::Color32::from_rgba_unmultiplied(231, 76, 60, 80) }
                            };
                            let top = y.min(zero_y);
                            let bot = y.max(zero_y);
                            let left = x - bw;
                            let right = x + bw;
                            let idx = mesh.vertices.len() as u32;
                            mesh.vertices.push(egui::epaint::Vertex { pos: egui::pos2(left, top), uv: egui::epaint::WHITE_UV, color: c });
                            mesh.vertices.push(egui::epaint::Vertex { pos: egui::pos2(right, top), uv: egui::epaint::WHITE_UV, color: c });
                            mesh.vertices.push(egui::epaint::Vertex { pos: egui::pos2(right, bot), uv: egui::epaint::WHITE_UV, color: c });
                            mesh.vertices.push(egui::epaint::Vertex { pos: egui::pos2(left, bot), uv: egui::epaint::WHITE_UV, color: c });
                            mesh.indices.extend_from_slice(&[idx, idx+1, idx+2, idx, idx+2, idx+3]);
                        }
                    }
                }
                if !mesh.vertices.is_empty() {
                    painter.add(egui::Shape::mesh(mesh));
                }
            }

            // Primary line (thicker for MACD)
            let primary_thickness = if ind.kind == IndicatorType::MACD { 1.5 } else { ind.thickness };
            let mut pts = Vec::new();
            for i in (vs as u32)..end {
                if let Some(&v) = ind.values.get(i as usize) {
                    if !v.is_nan() { pts.push(egui::pos2(bx(i as f32), osc_y(v))); }
                }
            }
            if pts.len() > 1 { painter.add(egui::Shape::line(pts, egui::Stroke::new(primary_thickness, color))); }

            // Secondary line (MACD signal = orange dashed, Stochastic %D = dim)
            if !ind.values2.is_empty() {
                let mut pts2 = Vec::new();
                for i in (vs as u32)..end {
                    if let Some(&v) = ind.values2.get(i as usize) {
                        if !v.is_nan() { pts2.push(egui::pos2(bx(i as f32), osc_y(v))); }
                    }
                }
                if pts2.len() > 1 {
                    if ind.kind == IndicatorType::MACD {
                        // Signal line: solid orange, clearly visible second line
                        let orange = egui::Color32::from_rgb(255, 152, 56);
                        painter.add(egui::Shape::line(pts2, egui::Stroke::new(1.2, orange)));
                    } else {
                        let c2 = egui::Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 140);
                        painter.add(egui::Shape::line(pts2, egui::Stroke::new(1.0, c2)));
                    }
                }
            }

            // Divergence markers
            for i in (vs as u32)..end {
                if let Some(&d) = ind.divergences.get(i as usize) {
                    if d != 0 {
                        let x = bx(i as f32);
                        if let Some(&v) = ind.values.get(i as usize) {
                            if !v.is_nan() {
                                let y = osc_y(v);
                                let div_color = if d > 0 { t.bull } else { t.bear };
                                // Small triangle marker
                                let dir = if d > 0 { -1.0 } else { 1.0 };
                                painter.add(egui::Shape::convex_polygon(vec![
                                    egui::pos2(x, y + dir * 2.0),
                                    egui::pos2(x - 4.0, y + dir * 7.0),
                                    egui::pos2(x + 4.0, y + dir * 7.0),
                                ], div_color, egui::Stroke::NONE));
                            }
                        }
                    }
                }
            }

            // Clickable label — click to edit, shows [x] delete on hover
            let label_text = ind.display_name();
            let label_rect = egui::Rect::from_min_size(
                egui::pos2(rect.left() + 4.0, osc_top + 2.0),
                egui::vec2(label_text.len() as f32 * 6.0 + 20.0, 14.0),
            );
            let label_hovered = ui.input(|i| i.pointer.hover_pos()).map_or(false, |p| label_rect.contains(p));
            let label_bg = if label_hovered { t.toolbar_border.gamma_multiply(0.5) } else { egui::Color32::TRANSPARENT };
            painter.rect_filled(label_rect, 2.0, label_bg);
            painter.text(egui::pos2(label_rect.left() + 2.0, label_rect.center().y), egui::Align2::LEFT_CENTER,
                &label_text, egui::FontId::monospace(9.0), color.gamma_multiply(if label_hovered { 1.0 } else { 0.7 }));
            // [x] delete button at end of label
            if label_hovered {
                let x_rect = egui::Rect::from_min_size(
                    egui::pos2(label_rect.right() - 12.0, label_rect.top()),
                    egui::vec2(12.0, 14.0),
                );
                painter.text(x_rect.center(), egui::Align2::CENTER_CENTER, Icon::X,
                    egui::FontId::proportional(8.0), t.bear);
            }
        }

        // CVD (Cumulative Volume Delta) in oscillator panel
        if chart.show_cvd && chart.cvd_data.len() == n {
            let start_c = vs.floor() as usize;
            let end_c = (start_c + chart.vc as usize + 8).min(n);
            let mut cvd_min = f32::MAX;
            let mut cvd_max = f32::MIN;
            for i in start_c..end_c {
                let v = chart.cvd_data[i];
                if cvd_min > v { cvd_min = v; }
                if cvd_max < v { cvd_max = v; }
            }
            if cvd_max <= cvd_min { cvd_max = cvd_min + 1.0; }
            let cvd_range = cvd_max - cvd_min;
            let cvd_py = |v: f32| -> f32 { osc_bottom - (v - cvd_min) / cvd_range * (osc_bottom - osc_top) };
            let zero_y = cvd_py(0.0_f32);
            if zero_y >= osc_top && zero_y <= osc_bottom {
                painter.line_segment([egui::pos2(rect.left(), zero_y), egui::pos2(rect.left()+cw, zero_y)],
                    egui::Stroke::new(0.5, color_alpha(t.text,30)));
            }
            for i in start_c..end_c.saturating_sub(1) {
                let y0 = cvd_py(chart.cvd_data[i]);
                let y1 = cvd_py(chart.cvd_data[i+1]);
                let rising = chart.cvd_data[i+1] > chart.cvd_data[i];
                let color = if rising {
                    egui::Color32::from_rgba_unmultiplied(46, 204, 113, 200)
                } else {
                    egui::Color32::from_rgba_unmultiplied(231, 76, 60, 200)
                };
                painter.line_segment([egui::pos2(bx(i as f32), y0), egui::pos2(bx((i+1) as f32), y1)],
                    egui::Stroke::new(1.5, color));
            }
            painter.text(egui::pos2(rect.left() + 4.0, osc_top + 2.0), egui::Align2::LEFT_TOP,
                "CVD", egui::FontId::monospace(8.0), color_alpha(t.text,120));
        }

        // ── Divergence lines on oscillator pane ──
        if chart.show_divergences && !chart.divergence_markers.is_empty() {
            for dm in &chart.divergence_markers {
                if dm.confidence < 0.3 { continue; }
                let x0 = bx(dm.start_bar as f32);
                let x1 = bx(dm.end_bar as f32);
                if x1 < rect.left() - 10.0 || x0 > rect.left() + cw + 10.0 { continue; }

                // Find matching oscillator indicator to get values at bar indices
                let ind_name_upper = dm.indicator.to_uppercase();
                if let Some(ind) = chart.indicators.iter().find(|i| {
                    i.visible && i.kind.label().to_uppercase().starts_with(&ind_name_upper)
                }) {
                    let v0 = ind.values.get(dm.start_bar as usize).copied().unwrap_or(f32::NAN);
                    let v1 = ind.values.get(dm.end_bar as usize).copied().unwrap_or(f32::NAN);
                    if v0.is_nan() || v1.is_nan() { continue; }

                    // Compute osc_y range for this indicator
                    let (osc_min_d, osc_max_d) = match ind.kind {
                        IndicatorType::RSI | IndicatorType::Stochastic | IndicatorType::ADX => (0.0, 100.0),
                        IndicatorType::WilliamsR => (-100.0, 0.0),
                        _ => {
                            let mut lo = f32::MAX; let mut hi = f32::MIN;
                            for i in (vs as u32)..(vs as u32 + total as u32).min(ind.values.len() as u32) {
                                if let Some(&v) = ind.values.get(i as usize) { if !v.is_nan() { lo = lo.min(v); hi = hi.max(v); } }
                            }
                            if lo >= hi { (lo - 1.0, hi + 1.0) } else { let pad = (hi - lo) * 0.1; (lo - pad, hi + pad) }
                        }
                    };
                    let osc_y_d = |v: f32| -> f32 { osc_top + (osc_max_d - v) / (osc_max_d - osc_min_d) * osc_height };

                    let y0 = osc_y_d(v0);
                    let y1 = osc_y_d(v1);
                    let is_bullish = dm.div_type.contains("bullish");
                    let is_hidden = dm.div_type.contains("hidden");
                    let color = if is_bullish { t.bull } else { t.bear };
                    let alpha = (180.0 * dm.confidence.clamp(0.3, 1.0)) as u8;
                    let line_color = egui::Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha);
                    let stroke_w = if is_hidden { 1.0 } else { 1.5 };

                    if is_hidden {
                        let steps = ((x1 - x0).abs() / 5.0) as usize;
                        for s in (0..steps.max(1)).step_by(2) {
                            let t0 = s as f32 / steps.max(1) as f32;
                            let t1 = ((s + 1) as f32 / steps.max(1) as f32).min(1.0);
                            painter.line_segment(
                                [egui::pos2(x0 + (x1 - x0) * t0, y0 + (y1 - y0) * t0),
                                 egui::pos2(x0 + (x1 - x0) * t1, y0 + (y1 - y0) * t1)],
                                egui::Stroke::new(stroke_w, line_color));
                        }
                    } else {
                        dashed_line(&painter, egui::pos2(x0, y0), egui::pos2(x1, y1),
                            egui::Stroke::new(stroke_w, line_color), crate::chart_renderer::LineStyle::Dashed);
                    }

                    // Small circles at indicator values
                    painter.circle_filled(egui::pos2(x0, y0), 2.5, line_color);
                    painter.circle_filled(egui::pos2(x1, y1), 2.5, line_color);
                }
            }
        }

        // Oscillator click interaction — allocate rect over the whole panel
        let osc_rect = egui::Rect::from_min_size(egui::pos2(rect.left(), osc_top), egui::vec2(cw, osc_height));
        let osc_resp = ui.allocate_rect(osc_rect, egui::Sense::click());

        if osc_resp.clicked() {
            if let Some(pos) = osc_resp.interact_pointer_pos() {
                // Check if clicked on a label's [x] delete button
                let mut deleted_id: Option<u32> = None;
                let mut label_y_offset = 0.0_f32;
                for ind in &chart.indicators {
                    if !ind.visible || ind.kind.category() != IndicatorCategory::Oscillator { continue; }
                    let label_text = ind.display_name();
                    let label_rect = egui::Rect::from_min_size(
                        egui::pos2(rect.left() + 4.0, osc_top + 2.0 + label_y_offset),
                        egui::vec2(label_text.len() as f32 * 6.0 + 20.0, 14.0),
                    );
                    let x_rect = egui::Rect::from_min_size(
                        egui::pos2(label_rect.right() - 12.0, label_rect.top()),
                        egui::vec2(12.0, 14.0),
                    );
                    if x_rect.contains(pos) {
                        deleted_id = Some(ind.id);
                        break;
                    }
                    if label_rect.contains(pos) {
                        chart.editing_indicator = Some(ind.id);
                        break;
                    }
                    label_y_offset += 16.0;
                }
                if let Some(id) = deleted_id {
                    chart.indicators.retain(|i| i.id != id);
                    chart.indicator_bar_count = 0;
                }
            }
        }

        // Double-click on oscillator line to edit
        if osc_resp.double_clicked() {
            if let Some(pos) = osc_resp.interact_pointer_pos() {
                for ind in &chart.indicators {
                    if !ind.visible || ind.kind.category() != IndicatorCategory::Oscillator { continue; }
                    // Check proximity to the oscillator's primary line
                    let (osc_min, osc_max) = match ind.kind {
                        IndicatorType::RSI | IndicatorType::Stochastic | IndicatorType::ADX => (0.0_f32, 100.0),
                        IndicatorType::WilliamsR => (-100.0_f32, 0.0),
                        _ => {
                            let mut lo = f32::MAX; let mut hi = f32::MIN;
                            for &v in &ind.values { if !v.is_nan() { lo = lo.min(v); hi = hi.max(v); } }
                            if lo >= hi { lo -= 1.0; hi += 1.0; }
                            let pad = (hi - lo) * 0.1; (lo - pad, hi + pad)
                        }
                    };
                    let osc_y = |v: f32| -> f32 { osc_top + (osc_max - v) / (osc_max - osc_min) * osc_height };
                    let bar_at_x = ((pos.x - rect.left() + off - bs * 0.5) / bs + vs) as usize;
                    for di in 0..7 {
                        let idx = match di { 0 => bar_at_x, 1 => bar_at_x.saturating_sub(1), 2 => bar_at_x + 1, 3 => bar_at_x.saturating_sub(2), 4 => bar_at_x + 2, 5 => bar_at_x.saturating_sub(3), _ => bar_at_x + 3 };
                        if let Some(&v) = ind.values.get(idx) {
                            if !v.is_nan() && (pos.y - osc_y(v)).abs() < 18.0 {
                                chart.editing_indicator = Some(ind.id);
                                break;
                            }
                        }
                    }
                    if chart.editing_indicator.is_some() { break; }
                }
            }
        }

        if osc_resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
    }

    span_begin("signal_overlays");
    // ── Signal drawings (auto-generated trendlines from server) ──────────
    if !chart.hide_signal_drawings && !chart.signal_drawings.is_empty() {
        for sd in &chart.signal_drawings {
            let color = hex_to_color(&sd.color, sd.opacity);
            let stroke = egui::Stroke::new(sd.thickness, color);
            match sd.drawing_type.as_str() {
                "trendline" if sd.points.len() >= 2 => {
                    let b0 = SignalDrawing::time_to_bar(sd.points[0].0, &chart.timestamps);
                    let b1 = SignalDrawing::time_to_bar(sd.points[1].0, &chart.timestamps);
                    let p0 = egui::pos2(bx(b0), py(sd.points[0].1));
                    let p1 = egui::pos2(bx(b1), py(sd.points[1].1));
                    match sd.line_style {
                        LineStyle::Solid => { painter.line_segment([p0, p1], stroke); }
                        _ => {
                            let (dash, gap) = if sd.line_style == LineStyle::Dashed { (6.0, 3.0) } else { (2.0, 2.0) };
                            let dir = p1 - p0; let len = dir.length();
                            if len > 1.0 { let norm = dir / len; let mut d = 0.0;
                                while d < len { let a = p0 + norm * d; let b = p0 + norm * (d+dash).min(len);
                                    painter.line_segment([a, b], stroke); d += dash + gap; }
                            }
                        }
                    }
                    // Strength indicator — small dot at midpoint, size = strength
                    if sd.strength > 0.0 {
                        let mid = egui::pos2((p0.x+p1.x)/2.0, (p0.y+p1.y)/2.0);
                        painter.circle_filled(mid, 2.0 + sd.strength * 3.0, color);
                    }
                }
                "hline" if !sd.points.is_empty() => {
                    let y = py(sd.points[0].1);
                    match sd.line_style {
                        LineStyle::Solid => { painter.line_segment([egui::pos2(rect.left(), y), egui::pos2(rect.left()+cw, y)], stroke); }
                        _ => {
                            let mut dx = rect.left(); while dx < rect.left()+cw {
                                painter.line_segment([egui::pos2(dx, y), egui::pos2((dx+6.0).min(rect.left()+cw), y)], stroke); dx += 10.0;
                            }
                        }
                    }
                }
                "hzone" if sd.points.len() >= 2 => {
                    let y0 = py(sd.points[0].1); let y1 = py(sd.points[1].1);
                    let fill = hex_to_color(&sd.color, sd.opacity * 0.15);
                    painter.rect_filled(egui::Rect::from_min_max(egui::pos2(rect.left(), y0.min(y1)), egui::pos2(rect.left()+cw, y0.max(y1))), 0.0, fill);
                    painter.line_segment([egui::pos2(rect.left(), y0), egui::pos2(rect.left()+cw, y0)], stroke);
                    painter.line_segment([egui::pos2(rect.left(), y1), egui::pos2(rect.left()+cw, y1)], stroke);
                }
                _ => {}
            }
        }
    }

    // ── Divergence overlays (price chart lines) ────────────────────────
    if chart.show_divergences && !chart.divergence_markers.is_empty() {
        for dm in &chart.divergence_markers {
            if dm.confidence < 0.3 { continue; } // skip low-confidence
            let x0 = bx(dm.start_bar as f32);
            let x1 = bx(dm.end_bar as f32);
            // Skip if completely outside viewport
            if x1 < rect.left() - 10.0 || x0 > rect.left() + cw + 10.0 { continue; }

            let is_bullish = dm.div_type.contains("bullish");
            let is_hidden = dm.div_type.contains("hidden");
            let color = if is_bullish { t.bull } else { t.bear };
            let alpha = (200.0 * dm.confidence.clamp(0.3, 1.0)) as u8;
            let line_color = egui::Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha);

            // Price chart line: connect the two pivot prices
            let y0 = py(dm.start_price);
            let y1 = py(dm.end_price);
            let stroke_w = if is_hidden { 1.0 } else { 1.5 };

            if is_hidden {
                // Dotted line for hidden divergences
                let steps = ((x1 - x0).abs() / 6.0) as usize;
                for s in (0..steps).step_by(2) {
                    let t0 = s as f32 / steps as f32;
                    let t1 = ((s + 1) as f32 / steps as f32).min(1.0);
                    let sx = x0 + (x1 - x0) * t0;
                    let sy = y0 + (y1 - y0) * t0;
                    let ex = x0 + (x1 - x0) * t1;
                    let ey = y0 + (y1 - y0) * t1;
                    painter.line_segment([egui::pos2(sx, sy), egui::pos2(ex, ey)],
                        egui::Stroke::new(stroke_w, line_color));
                }
            } else {
                // Dashed line for regular divergences
                dashed_line(&painter, egui::pos2(x0, y0), egui::pos2(x1, y1),
                    egui::Stroke::new(stroke_w, line_color), crate::chart_renderer::LineStyle::Dashed);
            }

            // Small circles at the pivot points
            painter.circle_filled(egui::pos2(x0, y0), 3.0, line_color);
            painter.circle_filled(egui::pos2(x1, y1), 3.0, line_color);

            // Label at midpoint
            let mid_x = (x0 + x1) * 0.5;
            let mid_y = (y0 + y1) * 0.5;
            let label = if is_hidden {
                if is_bullish { "H.Bull" } else { "H.Bear" }
            } else {
                if is_bullish { "Bull Div" } else { "Bear Div" }
            };
            // Label background pill
            let label_w = label.len() as f32 * 5.0 + 8.0;
            let pill = egui::Rect::from_center_size(
                egui::pos2(mid_x, mid_y - 8.0), egui::vec2(label_w, 12.0));
            painter.rect_filled(pill, 3.0,
                egui::Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 30));
            painter.text(pill.center(), egui::Align2::CENTER_CENTER,
                label, egui::FontId::monospace(7.0), line_color);

            // Indicator name (smaller, below)
            if !dm.indicator.is_empty() {
                painter.text(egui::pos2(mid_x, mid_y + 4.0), egui::Align2::CENTER_CENTER,
                    &dm.indicator, egui::FontId::monospace(6.0),
                    egui::Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha / 2));
            }
        }
    }

    // ── Candlestick pattern labels (from ApexSignals) ────────────────────
    if chart.show_pattern_labels && !chart.pattern_labels.is_empty() {
        let bars_ref = if chart.candle_mode == CandleMode::Standard { &chart.bars } else { &chart.alt_bars };
        let ts_ref = if chart.candle_mode == CandleMode::Standard { &chart.timestamps } else { &chart.alt_timestamps };
        for pl in &chart.pattern_labels {
            let bar_f = SignalDrawing::time_to_bar(pl.time, ts_ref);
            let x = bx(bar_f);
            if x < rect.left() - 5.0 || x > rect.left() + cw + 5.0 { continue; }
            let bar_idx = bar_f.round() as usize;
            let (bar_low, bar_high) = if let Some(bar) = bars_ref.get(bar_idx) {
                (bar.low, bar.high)
            } else { continue; };
            let alpha = (180.0 * pl.confidence.clamp(0.3, 1.0)) as u8;
            if pl.bullish {
                // Green upward triangle below bar's low
                let base_y = py(bar_low) + 4.0;
                let tri_col = color_alpha(t.bull, alpha);
                let tri = vec![
                    egui::pos2(x, base_y),
                    egui::pos2(x - 4.0, base_y + 7.0),
                    egui::pos2(x + 4.0, base_y + 7.0),
                ];
                painter.add(egui::Shape::convex_polygon(tri, tri_col, egui::Stroke::NONE));
                // Abbreviated label below triangle
                let abbrev: &str = if pl.label.len() > 3 { &pl.label[..3] } else { &pl.label };
                painter.text(egui::pos2(x, base_y + 10.0), egui::Align2::CENTER_TOP,
                    abbrev, egui::FontId::monospace(7.0), color_alpha(t.bull, alpha));
            } else {
                // Red downward triangle above bar's high
                let base_y = py(bar_high) - 4.0;
                let tri_col = color_alpha(t.bear, alpha);
                let tri = vec![
                    egui::pos2(x, base_y),
                    egui::pos2(x - 4.0, base_y - 7.0),
                    egui::pos2(x + 4.0, base_y - 7.0),
                ];
                painter.add(egui::Shape::convex_polygon(tri, tri_col, egui::Stroke::NONE));
                let abbrev: &str = if pl.label.len() > 3 { &pl.label[..3] } else { &pl.label };
                painter.text(egui::pos2(x, base_y - 10.0), egui::Align2::CENTER_BOTTOM,
                    abbrev, egui::FontId::monospace(7.0), color_alpha(t.bear, alpha));
            }
        }
    }

    // ── Demo signal data (toggled by chart.signal_demo flag) ───────────
    if chart.signal_demo_toggle {
        chart.signal_demo_toggle = false; // consume the toggle
        if chart.trend_health_score == 0.0 {
            // Turn ON demo
            chart.trend_health_score = 72.0;
            chart.trend_health_direction = 1;
            chart.trend_health_regime = "strong_trend".into();
            chart.exit_gauge_score = 35.0;
            chart.exit_gauge_urgency = "HOLD".into();
            chart.precursor_active = true;
            chart.precursor_score = 78.0;
            chart.precursor_direction = 1;
            chart.precursor_description = "5.2x baseline, 82% calls, 3 TF cascade".into();
            // Demo zones
            if chart.signal_zones.is_empty() {
                let price = if !chart.bars.is_empty() {
                    chart.bars.last().unwrap().close
                } else { 100.0 };
                chart.signal_zones = vec![
                    crate::chart_renderer::SignalZone { zone_type: "demand".into(), price_high: price * 0.985, price_low: price * 0.978, start_time: 0, strength: 8.2, touches: 3, fresh: true },
                    crate::chart_renderer::SignalZone { zone_type: "supply".into(), price_high: price * 1.025, price_low: price * 1.018, start_time: 0, strength: 7.5, touches: 2, fresh: false },
                    crate::chart_renderer::SignalZone { zone_type: "fvg".into(), price_high: price * 0.995, price_low: price * 0.991, start_time: 0, strength: 5.0, touches: 0, fresh: true },
                ];
                // Demo trade plan
                chart.trade_plan = Some((1, price, price * 1.02, price * 0.985, format!("{} {}C 5DTE", chart.symbol, (price / 5.0).round() * 5.0), 2.8, 85.0));
                // Demo change points
                if chart.timestamps.len() > 20 {
                    chart.change_points.push((chart.timestamps[chart.timestamps.len() - 15], "directional".into(), 0.85));
                    chart.change_points.push((chart.timestamps[chart.timestamps.len() - 8], "volume".into(), 0.72));
                }
                // Demo VIX expiry alert
                chart.vix_expiry_active = true;
                chart.vix_expiry_days = 3;
                chart.vix_expiry_date = "Wed Apr 16".into();
                chart.vix_spot = 27.3;
                chart.vix_expiring_future = 20.1;
                chart.vix_realized_vol = 16.2;
                chart.vix_gap_pct = 35.7;
                chart.vix_convergence_score = 82.0;
            }
        } else {
            // Turn OFF demo
            chart.trend_health_score = 0.0;
            chart.exit_gauge_score = 0.0;
            chart.precursor_active = false;
            chart.precursor_score = 0.0;
            chart.signal_zones.clear();
            chart.trade_plan = None;
            chart.change_points.clear();
            chart.vix_expiry_active = false;
        }
    }

    // ── Signal gauges — compact pill design, top-right ─────────────────
    {
        let gauge_x = rect.right() - 100.0;
        let mut gauge_y = rect.top() + 6.0;
        let pill_h = 18.0;
        let pill_w = 90.0;
        let pill_r = pill_h / 2.0; // fully rounded ends
        let bg = egui::Color32::from_rgba_unmultiplied(18, 18, 24, 210);

        // Helper: draw one gauge pill
        let draw_pill = |painter: &egui::Painter, y: f32, label: &str, score: f32, color: egui::Color32| {
            // Dark pill background
            painter.rect_filled(
                egui::Rect::from_min_size(egui::pos2(gauge_x, y), egui::vec2(pill_w, pill_h)),
                pill_r, bg,
            );
            // Thin fill bar inside (2px from edges)
            let bar_y = y + pill_h - 4.0;
            let bar_w = (pill_w - 8.0) * (score / 100.0).min(1.0);
            painter.rect_filled(
                egui::Rect::from_min_size(egui::pos2(gauge_x + 4.0, bar_y), egui::vec2(bar_w, 2.0)),
                1.0, color_alpha(color, 200),
            );
            // Left: colored dot + label
            painter.circle_filled(egui::pos2(gauge_x + 10.0, y + pill_h / 2.0), 3.0, color);
            painter.text(
                egui::pos2(gauge_x + 17.0, y + pill_h / 2.0), egui::Align2::LEFT_CENTER,
                label, egui::FontId::monospace(8.5),
                t.dim,
            );
            // Right: score
            painter.text(
                egui::pos2(gauge_x + pill_w - 6.0, y + pill_h / 2.0), egui::Align2::RIGHT_CENTER,
                format!("{:.0}", score), egui::FontId::monospace(9.0), color,
            );
        };

        // ── Trend Health ─────────────────────────────────────────────────
        if chart.show_trend_health && chart.trend_health_score > 0.0 {
            let th = chart.trend_health_score;
            let th_color = if th > 70.0 { t.bull }
                else if th > 40.0 { COLOR_AMBER }
                else { t.bear };
            let dir = match chart.trend_health_direction { 1 => "TH ▲", -1 => "TH ▼", _ => "TH ─" };
            draw_pill(&painter, gauge_y, dir, th, th_color);
            gauge_y += pill_h + 2.0;
        }

        // ── Exit Gauge ───────────────────────────────────────────────────
        if chart.show_exit_gauge && chart.exit_gauge_score > 0.0 {
            let eg = chart.exit_gauge_score;
            let eg_color = if eg > 80.0 { t.bear }
                else if eg > 60.0 { COLOR_AMBER }
                else if eg > 40.0 { COLOR_AMBER }
                else { t.bull };
            let label = if eg > 80.0 { "EXIT" } else if eg > 60.0 { "CLOSE" } else if eg > 40.0 { "TIGHT" } else { "HOLD" };
            draw_pill(&painter, gauge_y, label, eg, eg_color);

            // Subtle glow when critical
            if eg > 80.0 {
                let pulse = ((ctx.input(|i| i.time) * 3.0).sin() * 0.3 + 0.7) as f32;
                painter.rect_stroke(
                    egui::Rect::from_min_size(egui::pos2(gauge_x - 1.0, gauge_y - 1.0), egui::vec2(pill_w + 2.0, pill_h + 2.0)),
                    pill_r, egui::Stroke::new(1.0, color_alpha(t.bear, (pulse * 120.0) as u8)), egui::StrokeKind::Outside,
                );
                ctx.request_repaint();
            }
            gauge_y += pill_h + 2.0;
        }

        // ── Precursor Badge ──────────────────────────────────────────────
        if chart.show_precursor && chart.precursor_active && chart.precursor_score > 30.0 {
            let pr_color = match chart.precursor_direction {
                d if d > 0 => t.bull,
                d if d < 0 => t.bear,
                _ => COLOR_AMBER,
            };
            let dir = if chart.precursor_direction > 0 { "PRE ▲" } else if chart.precursor_direction < 0 { "PRE ▼" } else { "PRE ?" };
            draw_pill(&painter, gauge_y, dir, chart.precursor_score, pr_color);

            // Subtle pulse
            let pulse = ((ctx.input(|i| i.time) * 2.5).sin() * 0.3 + 0.7) as f32;
            painter.rect_stroke(
                egui::Rect::from_min_size(egui::pos2(gauge_x - 1.0, gauge_y - 1.0), egui::vec2(pill_w + 2.0, pill_h + 2.0)),
                pill_r, egui::Stroke::new(1.0, color_alpha(pr_color, (pulse * 80.0) as u8)), egui::StrokeKind::Outside,
            );
            ctx.request_repaint();
        }
    }

    // ── VIX Expiry Alert Card (bottom-right when active) ────────────────
    if chart.show_vix_alert && chart.vix_expiry_active && chart.vix_expiry_days <= 5 {
        let card_w = 240.0;
        let card_h = 120.0;
        let card_x = rect.right() - card_w - 8.0;
        let card_y = rect.bottom() - card_h - 24.0;
        let card_rect = egui::Rect::from_min_size(egui::pos2(card_x, card_y), egui::vec2(card_w, card_h));

        // Card background
        let bg = color_alpha(t.toolbar_bg, 230);
        painter.rect_filled(card_rect, 6.0, bg);

        // Top accent — amber warning stripe
        let accent = COLOR_AMBER;
        painter.rect_filled(
            egui::Rect::from_min_size(egui::pos2(card_x, card_y), egui::vec2(card_w, 3.0)),
            egui::Rounding { nw: 6, ne: 6, sw: 0, se: 0 }, accent,
        );

        let text_x = card_x + 10.0;
        let dim = t.dim;
        let bright = t.text;

        // Title
        painter.text(egui::pos2(text_x, card_y + 12.0), egui::Align2::LEFT_CENTER,
            format!("VIX EXPIRY — {} days ({})", chart.vix_expiry_days, chart.vix_expiry_date),
            egui::FontId::monospace(9.5), accent);

        // VIX spot vs future
        let y = card_y + 28.0;
        painter.text(egui::pos2(text_x, y), egui::Align2::LEFT_CENTER,
            format!("VIX spot:      {:.1}", chart.vix_spot),
            egui::FontId::monospace(9.0), t.bear);
        painter.text(egui::pos2(text_x, y + 13.0), egui::Align2::LEFT_CENTER,
            format!("Expiring fut:  {:.1}  ← settlement target", chart.vix_expiring_future),
            egui::FontId::monospace(9.0), t.bull);
        painter.text(egui::pos2(text_x, y + 26.0), egui::Align2::LEFT_CENTER,
            format!("Realized vol:  {:.1}%", chart.vix_realized_vol),
            egui::FontId::monospace(9.0), dim);
        painter.text(egui::pos2(text_x, y + 39.0), egui::Align2::LEFT_CENTER,
            format!("Gap:           {:.1}%  {}", chart.vix_gap_pct,
                if chart.vix_gap_pct > 25.0 { "EXTREME" } else if chart.vix_gap_pct > 15.0 { "ELEVATED" } else { "" }),
            egui::FontId::monospace(9.0), if chart.vix_gap_pct > 25.0 { accent } else { bright });

        // Signal line
        let signal_text = if chart.vix_gap_pct > 20.0 {
            "SIGNAL: Mean reversion HIGH → bullish SPY"
        } else if chart.vix_gap_pct > 10.0 {
            "SIGNAL: Moderate convergence pressure"
        } else {
            "SIGNAL: VIX near fair value"
        };
        let signal_color = if chart.vix_gap_pct > 20.0 { t.bull } else { dim };
        painter.text(egui::pos2(text_x, y + 56.0), egui::Align2::LEFT_CENTER,
            signal_text, egui::FontId::monospace(8.5), signal_color);

        // Convergence pressure bar
        let bar_y = y + 70.0;
        let bar_w = card_w - 20.0;
        painter.rect_filled(
            egui::Rect::from_min_size(egui::pos2(text_x, bar_y), egui::vec2(bar_w, 8.0)),
            4.0, color_alpha(t.toolbar_bg, 180),
        );
        let fill = bar_w * (chart.vix_convergence_score / 100.0).min(1.0);
        let bar_color = if chart.vix_convergence_score > 70.0 { t.bull }
            else if chart.vix_convergence_score > 40.0 { accent }
            else { dim };
        painter.rect_filled(
            egui::Rect::from_min_size(egui::pos2(text_x, bar_y), egui::vec2(fill, 8.0)),
            4.0, color_alpha(bar_color, 200),
        );
        painter.text(egui::pos2(text_x + bar_w + 4.0, bar_y + 4.0), egui::Align2::LEFT_CENTER,
            format!("{:.0}", chart.vix_convergence_score), egui::FontId::monospace(8.0), bar_color);

        // Subtle border
        painter.rect_stroke(card_rect, 6.0,
            egui::Stroke::new(1.0, color_alpha(accent, 40)), egui::StrokeKind::Outside);
    }

    // ── Supply/Demand zones — faint fill, clean edge labels ──────────────
    if chart.show_signal_zones { for zone in &chart.signal_zones {
        let y_high = py(zone.price_high);
        let y_low = py(zone.price_low);
        if y_high > rect.bottom() || y_low < rect.top() { continue; }

        let zone_color = match zone.zone_type.as_str() {
            "demand" | "order_block" => egui::Color32::from_rgb(56, 203, 137),
            "supply" => egui::Color32::from_rgb(224, 82, 82),
            "fvg" => egui::Color32::from_rgb(90, 120, 220),
            "breaker" => egui::Color32::from_rgb(210, 150, 40),
            _ => egui::Color32::from_rgb(100, 100, 110),
        };

        // Very faint fill
        let fill_alpha = if zone.fresh { 15u8 } else { 8 };
        let zone_rect = egui::Rect::from_min_max(
            egui::pos2(rect.left(), y_high.max(rect.top())),
            egui::pos2(rect.right(), y_low.min(rect.bottom())),
        );
        painter.rect_filled(zone_rect, 0.0, color_alpha(zone_color, fill_alpha));

        // Top and bottom edge lines (subtle)
        let edge_alpha = if zone.fresh { 50u8 } else { 25 };
        painter.line_segment(
            [egui::pos2(rect.left(), y_high.max(rect.top())), egui::pos2(rect.right(), y_high.max(rect.top()))],
            egui::Stroke::new(0.5, color_alpha(zone_color, edge_alpha)),
        );
        painter.line_segment(
            [egui::pos2(rect.left(), y_low.min(rect.bottom())), egui::pos2(rect.right(), y_low.min(rect.bottom()))],
            egui::Stroke::new(0.5, color_alpha(zone_color, edge_alpha)),
        );

        // Right-edge label (small, clean)
        let label_str = match zone.zone_type.as_str() {
            "demand" => "D", "supply" => "S", "fvg" => "FVG",
            "order_block" => "OB", "breaker" => "BRK", _ => "?",
        };
        let label_y = (y_high + y_low) / 2.0;
        if label_y > rect.top() && label_y < rect.bottom() {
            painter.text(
                egui::pos2(rect.right() - 3.0, label_y), egui::Align2::RIGHT_CENTER,
                format!("{} {:.0}", label_str, zone.strength),
                egui::FontId::monospace(7.5),
                color_alpha(zone_color, 100),
            );
        }
    }}

    // ── Change-point markers — small diamonds on the time axis ───────────
    if chart.show_change_points {
        let ts_ref = if chart.candle_mode == CandleMode::Standard { &chart.timestamps } else { &chart.alt_timestamps };
        let bars_ref = if chart.candle_mode == CandleMode::Standard { &chart.bars } else { &chart.alt_bars };
        for (cp_time, cp_type, cp_conf) in &chart.change_points {
            let bar_f = SignalDrawing::time_to_bar(*cp_time, ts_ref);
            let x = bx(bar_f);
            if x < rect.left() || x > rect.left() + cw { continue; }
            let cp_color = match cp_type.as_str() {
                "volume" => egui::Color32::from_rgb(90, 160, 235),
                "directional" => egui::Color32::from_rgb(230, 186, 57),
                "volatility" => egui::Color32::from_rgb(170, 100, 230),
                "institutional" => egui::Color32::from_rgb(230, 140, 40),
                _ => egui::Color32::from_rgb(130, 130, 140),
            };
            let alpha = ((cp_conf * 80.0) as u8).saturating_add(40);

            // Small diamond marker at the bottom of the chart area
            let dy = rect.bottom() - 8.0;
            let sz = 4.0;
            let diamond = vec![
                egui::pos2(x, dy - sz),
                egui::pos2(x + sz, dy),
                egui::pos2(x, dy + sz),
                egui::pos2(x - sz, dy),
            ];
            painter.add(egui::Shape::convex_polygon(diamond, color_alpha(cp_color, alpha), egui::Stroke::NONE));

            // Very thin vertical line — only on the candle body, not full height
            let bar_idx = bar_f.round() as usize;
            if let Some(bar) = bars_ref.get(bar_idx) {
                let y_top = py(bar.high) - 3.0;
                let y_bot = py(bar.low) + 3.0;
                painter.line_segment(
                    [egui::pos2(x, y_top), egui::pos2(x, y_bot)],
                    egui::Stroke::new(0.5, color_alpha(cp_color, alpha / 2)),
                );
            }
        }
    }

    // ── Trade plan — floating card + subtle chart lines ──────────────────
    if chart.show_trade_plan {
    if let Some((dir, entry, target, stop, ref contract, rr, conviction)) = chart.trade_plan {
        let entry_y = py(entry);
        let target_y = py(target);
        let stop_y = py(stop);

        // Subtle zone fill between target and stop (the trade's range)
        if target_y > rect.top() && stop_y < rect.bottom() {
            let zone_top = target_y.max(rect.top());
            let zone_bot = stop_y.min(rect.bottom());
            let mid_y = entry_y.clamp(zone_top, zone_bot);
            // Green zone: entry to target
            painter.rect_filled(
                egui::Rect::from_min_max(egui::pos2(rect.left(), zone_top), egui::pos2(rect.right(), mid_y)),
                0.0, color_alpha(egui::Color32::from_rgb(56, 203, 137), 6),
            );
            // Red zone: entry to stop
            painter.rect_filled(
                egui::Rect::from_min_max(egui::pos2(rect.left(), mid_y), egui::pos2(rect.right(), zone_bot)),
                0.0, color_alpha(egui::Color32::from_rgb(224, 82, 82), 6),
            );
        }

        // Thin dotted lines for entry/target/stop
        if entry_y > rect.top() && entry_y < rect.bottom() {
            dashed_line(&painter, egui::pos2(rect.left(), entry_y), egui::pos2(rect.right(), entry_y),
                egui::Stroke::new(0.8, color_alpha(t.dim, 100)), LineStyle::Dotted);
        }
        if target_y > rect.top() && target_y < rect.bottom() {
            dashed_line(&painter, egui::pos2(rect.left(), target_y), egui::pos2(rect.right(), target_y),
                egui::Stroke::new(0.8, color_alpha(egui::Color32::from_rgb(56, 203, 137), 80)), LineStyle::Dotted);
        }
        if stop_y > rect.top() && stop_y < rect.bottom() {
            dashed_line(&painter, egui::pos2(rect.left(), stop_y), egui::pos2(rect.right(), stop_y),
                egui::Stroke::new(0.8, color_alpha(egui::Color32::from_rgb(224, 82, 82), 80)), LineStyle::Dotted);
        }

        // Price labels on the right edge (price axis)
        let price_axis_x = rect.right() + 2.0;
        let label_bg = color_alpha(t.toolbar_bg, 220);
        for (price, y, color, label) in [
            (entry, entry_y, t.dim, ""),
            (target, target_y, t.bull, "T"),
            (stop, stop_y, t.bear, "S"),
        ] {
            if y > rect.top() && y < rect.bottom() {
                let txt = if label.is_empty() { format!("{:.2}", price) } else { format!("{} {:.2}", label, price) };
                let txt_rect = egui::Rect::from_min_size(egui::pos2(price_axis_x, y - 7.0), egui::vec2(52.0, 14.0));
                painter.rect_filled(txt_rect, 2.0, label_bg);
                painter.rect_filled(
                    egui::Rect::from_min_size(egui::pos2(price_axis_x, y - 7.0), egui::vec2(2.0, 14.0)),
                    0.0, color,
                );
                painter.text(egui::pos2(price_axis_x + 5.0, y), egui::Align2::LEFT_CENTER,
                    txt, egui::FontId::monospace(8.0), color);
            }
        }

        // Floating trade card — bottom-left of chart
        let card_w = 200.0;
        let card_h = 52.0;
        let card_x = rect.left() + 8.0;
        let card_y = rect.bottom() - card_h - 24.0;
        let card_rect = egui::Rect::from_min_size(egui::pos2(card_x, card_y), egui::vec2(card_w, card_h));

        // Card background
        painter.rect_filled(card_rect, 6.0, color_alpha(t.toolbar_bg, 230));
        // Left accent stripe
        let accent = if dir > 0 { t.bull } else { t.bear };
        painter.rect_filled(
            egui::Rect::from_min_size(egui::pos2(card_x, card_y), egui::vec2(3.0, card_h)),
            egui::Rounding { nw: 6, sw: 6, ne: 0, se: 0 }, accent,
        );

        // Contract name (bold, first line)
        painter.text(egui::pos2(card_x + 10.0, card_y + 10.0), egui::Align2::LEFT_CENTER,
            contract, egui::FontId::monospace(10.0), t.text);
        // R:R and conviction (second line)
        let move_pct = ((target - entry) / entry * 100.0).abs();
        painter.text(egui::pos2(card_x + 10.0, card_y + 24.0), egui::Align2::LEFT_CENTER,
            format!("R:R {:.1}  |  +{:.1}%  |  CVT {:.0}", rr, move_pct, conviction),
            egui::FontId::monospace(8.0), t.dim);
        // Entry → Target (third line)
        painter.text(egui::pos2(card_x + 10.0, card_y + 38.0), egui::Align2::LEFT_CENTER,
            format!("{:.2} → {:.2}  stop {:.2}", entry, target, stop),
            egui::FontId::monospace(8.0), t.dim.gamma_multiply(0.75));
    }

    // ── Periodic signal fetch (every 30s) ────────────────────────────────
    if chart.last_signal_fetch.elapsed().as_secs() >= 30 {
        chart.last_signal_fetch = std::time::Instant::now();
        fetch_signal_drawings(chart.symbol.clone());
    }

    // ── Position overlay — open IB positions on chart ─────────────────────
    if let Some((ref _acct, ref positions, ref _orders)) = account_data_cached {
        for pos in positions {
            if pos.symbol != chart.symbol || pos.qty == 0 { continue; }
            let entry_price = pos.avg_price;
            let entry_y = py(entry_price);
            if !entry_y.is_finite() || entry_y < rect.top() + pt || entry_y > rect.top() + pt + ch { continue; }
            let is_long = pos.qty > 0;
            let pos_color = if is_long { t.bull } else { t.bear };
            let last = chart.bars.last().map(|b| b.close).unwrap_or(entry_price);
            let pnl = (last - entry_price) * pos.qty as f32;
            let pnl_pct = if entry_price > 0.0 { (last / entry_price - 1.0) * 100.0 } else { 0.0 };
            // Dashed entry line (accent/cyan, thicker than order lines)
            let pos_line_color = color_alpha(t.accent, 180);
            {
                let mut dx = rect.left();
                while dx < rect.left() + cw {
                    let end = (dx + 8.0).min(rect.left() + cw);
                    painter.line_segment([egui::pos2(dx, entry_y), egui::pos2(end, entry_y)], egui::Stroke::new(1.8, pos_line_color));
                    dx += 14.0;
                }
            }
            // P&L fill zone (alpha 15)
            let current_y = py(last);
            if current_y.is_finite() {
                let profit = (is_long && last > entry_price) || (!is_long && last < entry_price);
                let fill_col = if profit {
                    egui::Color32::from_rgba_unmultiplied(46, 204, 113, 15)
                } else {
                    egui::Color32::from_rgba_unmultiplied(231, 76, 60, 15)
                };
                painter.rect_filled(egui::Rect::from_min_max(
                    egui::pos2(rect.left(), entry_y.min(current_y)),
                    egui::pos2(rect.left() + cw, entry_y.max(current_y))), 0.0, fill_col);
            }
            // Position badge (left side)
            let side_label = if is_long { "LONG" } else { "SHORT" };
            let badge = format!("POS {} {} @ {:.2}", side_label, pos.qty.abs(), entry_price);
            let badge_font = egui::FontId::monospace(9.0);
            let galley = painter.layout_no_wrap(badge.clone(), badge_font.clone(), pos_color);
            let bx_pos = rect.left() + 8.0;
            let bg = t.toolbar_bg;
            painter.rect_filled(
                egui::Rect::from_min_size(egui::pos2(bx_pos - 4.0, entry_y - galley.size().y / 2.0 - 2.0), egui::vec2(galley.size().x + 12.0, galley.size().y + 4.0)),
                4.0, egui::Color32::from_rgba_unmultiplied(bg.r(), bg.g(), bg.b(), 230));
            painter.rect_stroke(
                egui::Rect::from_min_size(egui::pos2(bx_pos - 4.0, entry_y - galley.size().y / 2.0 - 2.0), egui::vec2(galley.size().x + 12.0, galley.size().y + 4.0)),
                4.0, egui::Stroke::new(1.0, color_alpha(t.accent, 100)), egui::StrokeKind::Outside);
            painter.text(egui::pos2(bx_pos, entry_y), egui::Align2::LEFT_CENTER, &badge, badge_font, pos_color);
            // Right-edge P&L label
            let pnl_sign = if pnl >= 0.0 { "+" } else { "" };
            let pnl_color = if pnl >= 0.0 { t.bull } else { t.bear };
            let pnl_text = format!("{}${:.2} ({:+.1}%)", pnl_sign, pnl, pnl_pct);
            let pnl_font = egui::FontId::monospace(9.0);
            let pnl_galley = painter.layout_no_wrap(pnl_text.clone(), pnl_font.clone(), pnl_color);
            let pnl_x = rect.left() + cw - pnl_galley.size().x - 12.0;
            painter.rect_filled(
                egui::Rect::from_min_size(egui::pos2(pnl_x - 4.0, entry_y - pnl_galley.size().y / 2.0 - 2.0), egui::vec2(pnl_galley.size().x + 12.0, pnl_galley.size().y + 4.0)),
                4.0, egui::Color32::from_rgba_unmultiplied(bg.r(), bg.g(), bg.b(), 230));
            painter.rect_stroke(
                egui::Rect::from_min_size(egui::pos2(pnl_x - 4.0, entry_y - pnl_galley.size().y / 2.0 - 2.0), egui::vec2(pnl_galley.size().x + 12.0, pnl_galley.size().y + 4.0)),
                4.0, egui::Stroke::new(1.0, color_alpha(pnl_color, 80)), egui::StrokeKind::Outside);
            painter.text(egui::pos2(pnl_x, entry_y), egui::Align2::LEFT_CENTER, &pnl_text, pnl_font, pnl_color);
            // Y-axis position badge
            let yaxis_x = rect.left() + cw + 1.0;
            let yaxis_badge = if is_long { "L" } else { "S" };
            let yb_font = egui::FontId::monospace(7.0);
            let yb_galley = painter.layout_no_wrap(yaxis_badge.to_string(), yb_font.clone(), pos_color);
            painter.rect_filled(
                egui::Rect::from_min_size(egui::pos2(yaxis_x, entry_y - yb_galley.size().y / 2.0 - 1.0), yb_galley.size() + egui::vec2(4.0, 2.0)),
                2.0, color_alpha(t.accent, 40));
            painter.text(egui::pos2(yaxis_x + 2.0, entry_y), egui::Align2::LEFT_CENTER, yaxis_badge, yb_font, pos_color);
        }
    }
    } // end if chart.show_trade_plan

    // ── Analytics overlays ─────────────────────────────────────────────

    // Volume Shelves — horizontal shaded bands at high-volume price levels
    if chart.show_vol_shelves && !chart.bars.is_empty() {
        let bars = &chart.bars;
        let n = bars.len();
        let recent = &bars[n.saturating_sub(100)..n];
        let lo = recent.iter().map(|b| b.low).fold(f32::INFINITY, f32::min);
        let hi = recent.iter().map(|b| b.high).fold(f32::NEG_INFINITY, f32::max);
        let range = (hi - lo).max(0.01);
        let bins = 20;
        let mut vol = vec![0.0f32; bins];
        for b in recent {
            let mid = (b.high + b.low) / 2.0;
            let idx = ((mid - lo) / range * (bins - 1) as f32) as usize;
            vol[idx.min(bins - 1)] += b.volume;
        }
        let max_vol = vol.iter().cloned().fold(0.0f32, f32::max).max(1.0);
        let last = bars[n-1].close;

        for (i, &v) in vol.iter().enumerate() {
            if v < max_vol * 0.35 { continue; } // only show significant shelves
            let price = lo + (i as f32 + 0.5) * range / bins as f32;
            let price_top = lo + (i as f32 + 1.0) * range / bins as f32;
            let y1 = py(price);
            let y2 = py(price_top);
            if y1 < rect.top() || y2 > rect.bottom() { continue; }
            let strength = v / max_vol;
            let is_support = price < last;
            let color = if is_support {
                egui::Color32::from_rgba_unmultiplied(t.bull.r(), t.bull.g(), t.bull.b(), (strength * 25.0) as u8)
            } else {
                egui::Color32::from_rgba_unmultiplied(t.bear.r(), t.bear.g(), t.bear.b(), (strength * 25.0) as u8)
            };
            let band_w = cw * strength * 0.4; // width from right edge proportional to volume
            painter.rect_filled(egui::Rect::from_min_max(
                egui::pos2(rect.left() + cw - band_w, y2.min(y1)),
                egui::pos2(rect.left() + cw, y2.max(y1))),
                0.0, color);
        }
    }

    // S/R Confluence — horizontal lines at confluence zones
    if chart.show_confluence && !chart.bars.is_empty() {
        let bars = &chart.bars;
        let n = bars.len();
        let last = bars[n-1].close;
        // Compute levels
        let mut levels: Vec<f32> = Vec::new();
        for per in [20, 50, 100, 200] {
            if n >= per { levels.push(bars[n.saturating_sub(per)..n].iter().map(|b| b.close).sum::<f32>() / per as f32); }
        }
        let (h20, l20) = (bars[n.saturating_sub(20)..n].iter().map(|b| b.high).fold(f32::NEG_INFINITY, f32::max),
                          bars[n.saturating_sub(20)..n].iter().map(|b| b.low).fold(f32::INFINITY, f32::min));
        let pp = (h20 + l20 + last) / 3.0;
        levels.extend_from_slice(&[pp, 2.0 * pp - l20, 2.0 * pp - h20]);
        if n > 1 { levels.push(bars[n-2].high); levels.push(bars[n-2].low); }
        levels.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        // Cluster within 0.3%
        let mut i = 0;
        while i < levels.len() {
            let base = levels[i];
            let mut count = 1u32;
            while i + (count as usize) < levels.len() && (levels[i + (count as usize)] - base).abs() / last.max(0.01) < 0.003 {
                count += 1;
            }
            if count >= 2 {
                let avg: f32 = levels[i..i + (count as usize)].iter().sum::<f32>() / count as f32;
                let y = py(avg);
                if y > rect.top() && y < rect.bottom() {
                    let thickness = (count as f32).min(4.0);
                    let alpha = ((count as f32 / 4.0).min(1.0) * 120.0) as u8;
                    let col = if avg > last {
                        egui::Color32::from_rgba_unmultiplied(t.bear.r(), t.bear.g(), t.bear.b(), alpha)
                    } else {
                        egui::Color32::from_rgba_unmultiplied(t.bull.r(), t.bull.g(), t.bull.b(), alpha)
                    };
                    // Dashed line
                    let mut dx = rect.left();
                    while dx < rect.left() + cw {
                        let end = (dx + 6.0).min(rect.left() + cw);
                        painter.line_segment([egui::pos2(dx, y), egui::pos2(end, y)],
                            egui::Stroke::new(thickness, col));
                        dx += 10.0;
                    }
                    // Count badge at right edge
                    painter.text(egui::pos2(rect.left() + cw - 4.0, y - 6.0), egui::Align2::RIGHT_BOTTOM,
                        &format!("{}x", count), egui::FontId::monospace(7.0), col);
                }
            }
            i += count as usize;
        }
    }

    // Momentum Heatmap — per-bar colored strip at bottom of chart
    if chart.show_momentum_heat && chart.bars.len() > 20 {
        let strip_h = 4.0;
        let strip_y = rect.top() + pt + ch - strip_h - 1.0;
        let vis_start = chart.vs.floor().max(0.0) as usize;
        let vis_end = (vis_start + chart.vc as usize + 2).min(chart.bars.len());
        let lookback = 10;

        for bi in vis_start..vis_end {
            if bi < lookback { continue; }
            let roc = if chart.bars[bi - lookback].close > 0.0 {
                (chart.bars[bi].close - chart.bars[bi - lookback].close) / chart.bars[bi - lookback].close
            } else { 0.0 };
            let intensity = (roc.abs() * 20.0).clamp(0.0, 1.0);
            let color = if roc > 0.0 { t.bull } else { t.bear };
            let alpha = (intensity * 180.0 + 30.0) as u8;
            let bar_sp = bs;
            let bx = rect.left() + (bi as f32 - chart.vs) * bar_sp;
            if bx < rect.left() || bx + bar_sp > rect.left() + cw { continue; }
            painter.rect_filled(egui::Rect::from_min_size(
                egui::pos2(bx, strip_y), egui::vec2(bar_sp.max(1.0), strip_h)),
                0.0, egui::Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha));
        }
    }

    // Trend Alignment Strip — vertical colored cells on right edge
    if chart.show_trend_strip && !chart.bars.is_empty() {
        let strip_w = 14.0;
        let strip_x = rect.left() + cw - strip_w - 2.0;
        let strip_top = rect.top() + pt + 4.0;
        let strip_total_h = ch - 8.0;
        let tf_labels = ["5m", "15", "30", "1h", "4h", "1D", "1W"];
        let periods = [7usize, 10, 14, 21, 42, 70, 140];
        let cell_h = strip_total_h / 7.0;

        for (i, &per) in periods.iter().enumerate() {
            let n = chart.bars.len();
            let bullish = if n >= per {
                let sma: f32 = chart.bars[n.saturating_sub(per)..n].iter().map(|b| b.close).sum::<f32>() / per.min(n) as f32;
                chart.bars[n-1].close > sma
            } else { false };
            let cy = strip_top + i as f32 * cell_h;
            let color = if bullish { t.bull } else { t.bear };
            painter.rect_filled(egui::Rect::from_min_size(
                egui::pos2(strip_x, cy), egui::vec2(strip_w, cell_h - 1.0)),
                2.0, egui::Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 60));
            painter.text(egui::pos2(strip_x + strip_w * 0.5, cy + cell_h * 0.5),
                egui::Align2::CENTER_CENTER, tf_labels[i], egui::FontId::monospace(6.0),
                egui::Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 160));
        }
    }

    // Breadth Tint — subtle background tint based on market health
    if chart.show_breadth_tint && chart.bars.len() > 50 {
        let n = chart.bars.len();
        let last = chart.bars[n-1].close;
        let mut score = 0.0f32;
        for per in [10, 20, 50] {
            if n >= per {
                let sma: f32 = chart.bars[n-per..n].iter().map(|b| b.close).sum::<f32>() / per as f32;
                if last > sma { score += 33.3; }
            }
        }
        let alpha = 6u8; // very subtle
        let color = if score > 60.0 {
            egui::Color32::from_rgba_unmultiplied(t.bull.r(), t.bull.g(), t.bull.b(), alpha)
        } else if score < 40.0 {
            egui::Color32::from_rgba_unmultiplied(t.bear.r(), t.bear.g(), t.bear.b(), alpha)
        } else {
            egui::Color32::TRANSPARENT
        };
        if color != egui::Color32::TRANSPARENT {
            painter.rect_filled(egui::Rect::from_min_size(
                egui::pos2(rect.left(), rect.top() + pt), egui::vec2(cw, ch)),
                0.0, color);
        }
    }

    // Volatility Cone — forward-looking expected range projection
    if chart.show_vol_cone && chart.bars.len() > 20 {
        let n = chart.bars.len();
        let last = chart.bars[n-1].close;
        // Compute ATR inline
        let atr_val = {
            let mut sum = 0.0f32;
            let p = 14usize.min(n - 1);
            for i in (n - p)..n {
                let tr = (chart.bars[i].high - chart.bars[i].low)
                    .max((chart.bars[i].high - chart.bars[i-1].close).abs())
                    .max((chart.bars[i].low - chart.bars[i-1].close).abs());
                sum += tr;
            }
            sum / p as f32
        };
        let last_bar_x = rect.left() + (n as f32 - 1.0 - chart.vs) * bs;

        for &sigma in &[1.0f32, 2.0, 3.0] {
            let alpha = match sigma as u32 { 1 => 18u8, 2 => 10, _ => 5 };
            let col = egui::Color32::from_rgba_unmultiplied(t.accent.r(), t.accent.g(), t.accent.b(), alpha);
            let mut points_upper = Vec::new();
            let mut points_lower = Vec::new();
            for bars_ahead in 0..25 {
                let x = last_bar_x + bars_ahead as f32 * bs;
                if x > rect.left() + cw { break; }
                let spread = atr_val * sigma * (bars_ahead as f32 + 1.0).sqrt() * 0.5;
                points_upper.push(egui::pos2(x, py(last + spread)));
                points_lower.push(egui::pos2(x, py(last - spread)));
            }
            // Fill between upper and lower
            for i in 0..points_upper.len().saturating_sub(1) {
                let quad = [points_upper[i], points_upper[i+1], points_lower[i+1], points_lower[i]];
                painter.rect_filled(egui::Rect::from_min_max(
                    egui::pos2(quad[0].x.min(quad[3].x), quad[0].y.min(quad[1].y)),
                    egui::pos2(quad[1].x.max(quad[2].x), quad[3].y.max(quad[2].y))),
                    0.0, col);
            }
            // Edge lines
            if points_upper.len() >= 2 {
                let line_alpha = (alpha as u16 * 4).min(120) as u8;
                let line_col = egui::Color32::from_rgba_unmultiplied(t.accent.r(), t.accent.g(), t.accent.b(), line_alpha);
                for pts in [&points_upper, &points_lower] {
                    for i in 0..pts.len()-1 {
                        painter.line_segment([pts[i], pts[i+1]], egui::Stroke::new(0.5, line_col));
                    }
                }
            }
        }
    }

    // Price Memory Heatmap — glow at frequently tested price levels
    if chart.show_price_memory && chart.bars.len() > 20 {
        let n = chart.bars.len();
        let recent = &chart.bars[n.saturating_sub(200)..n];
        let lo = recent.iter().map(|b| b.low).fold(f32::INFINITY, f32::min);
        let hi = recent.iter().map(|b| b.high).fold(f32::NEG_INFINITY, f32::max);
        let range = (hi - lo).max(0.01);
        let bins = 40;
        let mut touches = vec![0u32; bins];
        for b in recent {
            for price in [b.high, b.low, b.open, b.close] {
                let idx = ((price - lo) / range * (bins - 1) as f32) as usize;
                touches[idx.min(bins - 1)] += 1;
            }
        }
        let max_t = *touches.iter().max().unwrap_or(&1) as f32;
        for (i, &count) in touches.iter().enumerate() {
            if count < 3 { continue; } // skip low-touch levels
            let price = lo + (i as f32 + 0.5) * range / bins as f32;
            let y = py(price);
            if y < rect.top() || y > rect.bottom() { continue; }
            let intensity = (count as f32 / max_t).clamp(0.0, 1.0);
            let glow_h = (range / bins as f32) * 0.8;
            let y_top = py(price + glow_h * 0.5);
            let y_bot = py(price - glow_h * 0.5);
            let alpha = (intensity * 20.0) as u8;
            painter.rect_filled(egui::Rect::from_min_max(
                egui::pos2(rect.left(), y_top.min(y_bot)),
                egui::pos2(rect.left() + cw, y_top.max(y_bot))),
                0.0, egui::Color32::from_rgba_unmultiplied(t.accent.r(), t.accent.g(), t.accent.b(), alpha));
        }
    }

    // Liquidity Voids — shaded rectangles for unfilled price gaps
    if chart.show_liquidity_voids && chart.bars.len() > 5 {
        let vis_start = chart.vs.floor().max(0.0) as usize;
        let vis_end = (vis_start + chart.vc as usize + 2).min(chart.bars.len());
        for i in (vis_start + 1)..vis_end {
            // A void exists when bar i's low > bar i-1's high (gap up)
            // or bar i's high < bar i-1's low (gap down)
            let prev = &chart.bars[i - 1];
            let curr = &chart.bars[i];
            let (gap_top, gap_bot, is_up) = if curr.low > prev.high {
                (curr.low, prev.high, true) // gap up
            } else if curr.high < prev.low {
                (prev.low, curr.high, false) // gap down
            } else { continue; };

            // Check if gap has been filled by subsequent bars
            let filled = chart.bars[i..vis_end.min(chart.bars.len())].iter()
                .any(|b| b.low <= gap_bot && b.high >= gap_top);
            if filled { continue; } // skip filled gaps

            let y1 = py(gap_top);
            let y2 = py(gap_bot);
            let x_start = rect.left() + (i as f32 - 1.0 - chart.vs) * bs;
            let color = if is_up {
                egui::Color32::from_rgba_unmultiplied(t.bull.r(), t.bull.g(), t.bull.b(), 15)
            } else {
                egui::Color32::from_rgba_unmultiplied(t.bear.r(), t.bear.g(), t.bear.b(), 15)
            };
            // Extend void to right edge
            painter.rect_filled(egui::Rect::from_min_max(
                egui::pos2(x_start.max(rect.left()), y1.min(y2)),
                egui::pos2(rect.left() + cw, y1.max(y2))),
                0.0, color);
            // Border
            let border_col = if is_up {
                egui::Color32::from_rgba_unmultiplied(t.bull.r(), t.bull.g(), t.bull.b(), 40)
            } else {
                egui::Color32::from_rgba_unmultiplied(t.bear.r(), t.bear.g(), t.bear.b(), 40)
            };
            painter.line_segment([egui::pos2(x_start.max(rect.left()), y1), egui::pos2(rect.left() + cw, y1)],
                egui::Stroke::new(0.5, border_col));
            painter.line_segment([egui::pos2(x_start.max(rect.left()), y2), egui::pos2(rect.left() + cw, y2)],
                egui::Stroke::new(0.5, border_col));
        }
    }

    // Correlation Ribbon — thin strip at top showing rolling autocorrelation
    if chart.show_corr_ribbon && chart.bars.len() > 25 {
        let ribbon_h = 3.0;
        let ribbon_y = rect.top() + pt + 1.0;
        let vis_start = chart.vs.floor().max(0.0) as usize;
        let vis_end = (vis_start + chart.vc as usize + 2).min(chart.bars.len());
        let lookback = 20;

        for bi in vis_start..vis_end {
            if bi < lookback + 1 { continue; }
            // Compute serial correlation of returns
            let returns: Vec<f32> = (bi-lookback..bi).map(|j| {
                if chart.bars[j].close > 0.0 { (chart.bars[j+1].close - chart.bars[j].close) / chart.bars[j].close } else { 0.0 }
            }).collect();
            let mean: f32 = returns.iter().sum::<f32>() / returns.len() as f32;
            let var: f32 = returns.iter().map(|r| (r - mean).powi(2)).sum::<f32>() / returns.len() as f32;
            let mut autocorr = 0.0f32;
            if var > 0.0001 {
                let cov: f32 = returns.iter().skip(1).zip(returns.iter())
                    .map(|(r1, r0)| (r1 - mean) * (r0 - mean)).sum::<f32>() / (returns.len() - 1) as f32;
                autocorr = (cov / var).clamp(-1.0, 1.0);
            }
            // Green = trending (high positive autocorr), amber = random, red = mean-reverting
            let color = if autocorr > 0.2 { t.bull }
                else if autocorr < -0.2 { t.bear }
                else { egui::Color32::from_rgb(255, 191, 0) };
            let alpha = (autocorr.abs() * 200.0 + 40.0) as u8;
            let bx = rect.left() + (bi as f32 - chart.vs) * bs;
            if bx < rect.left() || bx + bs > rect.left() + cw { continue; }
            painter.rect_filled(egui::Rect::from_min_size(
                egui::pos2(bx, ribbon_y), egui::vec2(bs.max(1.0), ribbon_h)),
                0.0, egui::Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha));
        }
    }

    // Analyst Price Targets — horizontal lines at mean/high/low targets
    if chart.show_analyst_targets && chart.fundamentals.analyst_target_mean > 0.0 {
        let f = &chart.fundamentals;
        for (price, label, color, dash) in [
            (f.analyst_target_mean, "PT Mean", t.accent, 8.0f32),
            (f.analyst_target_high, "PT High", t.bull, 5.0),
            (f.analyst_target_low, "PT Low", t.bear, 5.0),
        ] {
            let y = py(price);
            if y > rect.top() + pt && y < rect.top() + pt + ch {
                let mut dx = rect.left();
                while dx < rect.left() + cw {
                    let end = (dx + dash).min(rect.left() + cw);
                    painter.line_segment([egui::pos2(dx, y), egui::pos2(end, y)],
                        egui::Stroke::new(0.8, color_alpha(color, 100)));
                    dx += dash * 2.0;
                }
                // Label
                painter.text(egui::pos2(rect.left() + 4.0, y - 7.0), egui::Align2::LEFT_BOTTOM,
                    &format!("{} ${:.0}", label, price), egui::FontId::monospace(7.0),
                    color_alpha(color, 140));
            }
        }
    }

    // PE Band — shaded channel showing historical PE valuation range
    if chart.show_pe_band && chart.fundamentals.pe_ratio > 0.0 && !chart.bars.is_empty() {
        let pe = chart.fundamentals.pe_ratio;
        let eps = chart.fundamentals.eps_ttm;
        if eps > 0.0 {
            let pe_fair = eps * pe;
            let pe_high = eps * (pe * 1.2); // 20% premium
            let pe_low = eps * (pe * 0.8);  // 20% discount
            let y_fair = py(pe_fair);
            let y_high = py(pe_high);
            let y_low = py(pe_low);
            // Premium zone (above fair value)
            if y_high > rect.top() && y_fair < rect.bottom() {
                painter.rect_filled(egui::Rect::from_min_max(
                    egui::pos2(rect.left(), y_high.max(rect.top())),
                    egui::pos2(rect.left() + cw, y_fair.min(rect.bottom()))),
                    0.0, egui::Color32::from_rgba_unmultiplied(t.bear.r(), t.bear.g(), t.bear.b(), 6));
            }
            // Discount zone (below fair value)
            if y_fair > rect.top() && y_low < rect.bottom() {
                painter.rect_filled(egui::Rect::from_min_max(
                    egui::pos2(rect.left(), y_fair.max(rect.top())),
                    egui::pos2(rect.left() + cw, y_low.min(rect.bottom()))),
                    0.0, egui::Color32::from_rgba_unmultiplied(t.bull.r(), t.bull.g(), t.bull.b(), 6));
            }
            // Fair value line
            if y_fair > rect.top() && y_fair < rect.bottom() {
                painter.line_segment([egui::pos2(rect.left(), y_fair), egui::pos2(rect.left() + cw, y_fair)],
                    egui::Stroke::new(0.5, color_alpha(t.dim, 60)));
                painter.text(egui::pos2(rect.left() + 4.0, y_fair - 7.0), egui::Align2::LEFT_BOTTOM,
                    &format!("Fair Value ${:.0} (PE {:.1})", pe_fair, pe), egui::FontId::monospace(7.0),
                    color_alpha(t.dim, 100));
            }
        }
    }

    // Insider Trade markers on chart
    if chart.show_insider_trades && !chart.insider_trades.is_empty() && !chart.timestamps.is_empty() {
        for trade in &chart.insider_trades {
            // Find the closest bar to this trade date
            let bar_idx = chart.timestamps.partition_point(|&ts| ts < trade.date);
            if bar_idx >= chart.bars.len() { continue; }
            let x = rect.left() + (bar_idx as f32 - chart.vs) * bs;
            if x < rect.left() || x > rect.left() + cw { continue; }

            let is_buy = trade.shares > 0;
            let color = if is_buy { t.bull } else { t.bear };
            let y_base = rect.top() + pt + ch - 2.0;

            // Arrow marker at bottom of chart
            let arrow_h = 10.0;
            if is_buy {
                // Up arrow
                painter.line_segment([egui::pos2(x, y_base), egui::pos2(x, y_base - arrow_h)],
                    egui::Stroke::new(1.5, color));
                painter.line_segment([egui::pos2(x - 3.0, y_base - arrow_h + 3.0), egui::pos2(x, y_base - arrow_h)],
                    egui::Stroke::new(1.5, color));
                painter.line_segment([egui::pos2(x + 3.0, y_base - arrow_h + 3.0), egui::pos2(x, y_base - arrow_h)],
                    egui::Stroke::new(1.5, color));
            } else {
                // Down arrow
                painter.line_segment([egui::pos2(x, y_base - arrow_h), egui::pos2(x, y_base)],
                    egui::Stroke::new(1.5, color));
                painter.line_segment([egui::pos2(x - 3.0, y_base - 3.0), egui::pos2(x, y_base)],
                    egui::Stroke::new(1.5, color));
                painter.line_segment([egui::pos2(x + 3.0, y_base - 3.0), egui::pos2(x, y_base)],
                    egui::Stroke::new(1.5, color));
            }
            // Small label
            let label = if is_buy { "B" } else { "S" };
            painter.text(egui::pos2(x, y_base - arrow_h - 4.0), egui::Align2::CENTER_BOTTOM,
                label, egui::FontId::monospace(6.0), color);
        }
    }

    // ── OCO/Trigger bracket bands with connectors & R:R ─────────────────
    {
        let active_orders: Vec<&OrderLevel> = chart.orders.iter().filter(|o| o.status != OrderStatus::Cancelled && o.status != OrderStatus::Executed).collect();
        for order in &active_orders {
            if let Some(pair_id) = order.pair_id {
                if let Some(pair) = active_orders.iter().find(|o| o.id == pair_id) {
                    // Only draw once (from higher-id order to avoid double-draw)
                    if order.id > pair.id {
                        let y1 = py(order.price);
                        let y2 = py(pair.price);
                        // Identify target vs stop in the OCO pair
                        let (target_order, stop_order) = if matches!(order.side, OrderSide::OcoTarget) {
                            (*order, *pair)
                        } else if matches!(pair.side, OrderSide::OcoTarget) {
                            (*pair, *order)
                        } else {
                            (*order, *pair)
                        };
                        let is_oco = matches!(order.side, OrderSide::OcoTarget | OrderSide::OcoStop)
                            || matches!(pair.side, OrderSide::OcoTarget | OrderSide::OcoStop);
                        if is_oco {
                            let tp_y = py(target_order.price);
                            let sl_y = py(stop_order.price);
                            // Green-tinted zone (profit zone: between midpoint and TP)
                            let mid_price = (target_order.price + stop_order.price) / 2.0;
                            let mid_y = py(mid_price);
                            painter.rect_filled(egui::Rect::from_min_max(
                                egui::pos2(rect.left(), tp_y.min(mid_y)), egui::pos2(rect.left() + cw, tp_y.max(mid_y))),
                                0.0, color_alpha(t.bull, 12));
                            // Red-tinted zone (loss zone: between midpoint and SL)
                            painter.rect_filled(egui::Rect::from_min_max(
                                egui::pos2(rect.left(), sl_y.min(mid_y)), egui::pos2(rect.left() + cw, sl_y.max(mid_y))),
                                0.0, color_alpha(t.bear, 12));
                            // Vertical dotted connector line on right side of chart
                            let connector_x = rect.left() + cw - 20.0;
                            let top_y = y1.min(y2);
                            let bot_y = y1.max(y2);
                            {
                                let mut dy = top_y;
                                while dy < bot_y {
                                    let end = (dy + 3.0).min(bot_y);
                                    painter.line_segment(
                                        [egui::pos2(connector_x, dy), egui::pos2(connector_x, end)],
                                        egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(167, 139, 250, 120)));
                                    dy += 6.0;
                                }
                            }
                            // Small horizontal ticks at each end of the connector
                            painter.line_segment(
                                [egui::pos2(connector_x - 4.0, tp_y), egui::pos2(connector_x + 4.0, tp_y)],
                                egui::Stroke::new(1.0, color_alpha(t.bull, 180)));
                            painter.line_segment(
                                [egui::pos2(connector_x - 4.0, sl_y), egui::pos2(connector_x + 4.0, sl_y)],
                                egui::Stroke::new(1.0, color_alpha(t.bear, 180)));
                            // R:R ratio label at midpoint of connector
                            let reward = (target_order.price - mid_price).abs();
                            let risk = (stop_order.price - mid_price).abs();
                            if risk > 0.0 {
                                let rr = reward / risk;
                                let rr_text = format!("R:R {:.1}:1", rr);
                                let rr_font = egui::FontId::monospace(8.0);
                                let rr_galley = painter.layout_no_wrap(rr_text.clone(), rr_font.clone(), egui::Color32::from_rgb(167, 139, 250));
                                let rr_bg_rect = egui::Rect::from_min_size(
                                    egui::pos2(connector_x - rr_galley.size().x / 2.0 - 3.0, mid_y - rr_galley.size().y / 2.0 - 2.0),
                                    egui::vec2(rr_galley.size().x + 6.0, rr_galley.size().y + 4.0));
                                painter.rect_filled(rr_bg_rect, 3.0, egui::Color32::from_rgba_unmultiplied(t.toolbar_bg.r(), t.toolbar_bg.g(), t.toolbar_bg.b(), 220));
                                painter.text(egui::pos2(connector_x, mid_y), egui::Align2::CENTER_CENTER, &rr_text, rr_font,
                                    egui::Color32::from_rgb(167, 139, 250));
                            }
                        } else {
                            // Non-OCO bracket (trigger pairs) — single color band
                            let band_color = match order.side {
                                OrderSide::TriggerBuy | OrderSide::TriggerSell => egui::Color32::from_rgba_unmultiplied(t.bull.r(), t.bull.g(), t.bull.b(), 12),
                                _ => egui::Color32::TRANSPARENT,
                            };
                            painter.rect_filled(egui::Rect::from_min_max(
                                egui::pos2(rect.left(), y1.min(y2)), egui::pos2(rect.left() + cw, y1.max(y2))),
                                0.0, band_color);
                            // Vertical dotted connector for triggers too
                            let connector_x = rect.left() + cw - 20.0;
                            {
                                let mut dy = y1.min(y2);
                                while dy < y1.max(y2) {
                                    let end = (dy + 3.0).min(y1.max(y2));
                                    painter.line_segment(
                                        [egui::pos2(connector_x, dy), egui::pos2(connector_x, end)],
                                        egui::Stroke::new(0.8, egui::Color32::from_rgba_unmultiplied(t.bull.r(), t.bull.g(), t.bull.b(), 80)));
                                    dy += 6.0;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // ── Order lines on chart ──────────────────────────────────────────────
    for order in &chart.orders {
        if order.status == OrderStatus::Cancelled || order.status == OrderStatus::Executed { continue; }
        let y = py(order.price);
        if y < rect.top() + pt || y > rect.top() + pt + ch { continue; }
        let color = order.color(t.bull, t.bear);
        let is_draft = order.status == OrderStatus::Draft;
        let dark = t.bg;
        let badge_h = 24.0;

        // Dashed line across full width
        let dash_alpha = if is_draft { 120 } else { 200 };
        let dash_color = egui::Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), dash_alpha);
        let mut dx = rect.left();
        while dx < rect.left() + cw {
            let end = (dx + 6.0).min(rect.left() + cw);
            painter.line_segment([egui::pos2(dx, y), egui::pos2(end, y)], egui::Stroke::new(1.0, dash_color));
            dx += 10.0;
        }

        // ── Badge: [B/S] [QTY] [notional] [DRAFT/LIVE] [SEND?] [X] ──
        let side_ch = match order.side {
            OrderSide::Buy | OrderSide::TriggerBuy => "B",
            OrderSide::Sell | OrderSide::TriggerSell => "S",
            OrderSide::Stop | OrderSide::OcoStop => "S",
            OrderSide::OcoTarget => "T",
        };
        let qty_str = format!("{}", order.qty);
        let notional_str = fmt_notional(order.notional());
        let status_label = if is_draft { "DRAFT" } else { "LIVE" };
        let side_w = 20.0;
        let qty_w = qty_str.len() as f32 * 9.0 + 12.0;
        let notional_w = notional_str.len() as f32 * 9.0 + 12.0;
        let status_w = status_label.len() as f32 * 6.0 + 8.0;
        let send_w = if is_draft { 38.0 } else { 0.0 };
        let x_btn_w = 22.0;
        let total_w = side_w + qty_w + notional_w + status_w + send_w + x_btn_w + 4.0;
        // Position badge ~60% from left (shifted 40px left from 2/3)
        let bx = rect.left() + cw * 0.60 - total_w * 0.5;
        let by = y - badge_h * 0.5;
        let badge_alpha: u8 = 220;

        // Check hover on entire badge for pointer + hover highlight
        let full_badge = egui::Rect::from_min_size(egui::pos2(bx, by), egui::vec2(total_w, badge_h));
        let badge_hovered = ui.input(|i| i.pointer.hover_pos()).map_or(false, |p| full_badge.contains(p));
        if badge_hovered { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
        let hover_boost: u8 = if badge_hovered { 30 } else { 0 };

        // Side letter section
        let side_rect = egui::Rect::from_min_size(egui::pos2(bx, by), egui::vec2(side_w, badge_h));
        let side_bg = egui::Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), badge_alpha.saturating_add(20).saturating_add(hover_boost));
        painter.rect_filled(side_rect, egui::CornerRadius { nw: 3, sw: 3, ne: 0, se: 0 }, side_bg);
        painter.text(side_rect.center(), egui::Align2::CENTER_CENTER, side_ch, egui::FontId::monospace(9.0), dark);

        // Qty section
        let qty_rect = egui::Rect::from_min_size(egui::pos2(side_rect.right(), by), egui::vec2(qty_w, badge_h));
        painter.rect_filled(qty_rect, 0.0, egui::Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), badge_alpha.saturating_add(hover_boost)));
        painter.text(qty_rect.center(), egui::Align2::CENTER_CENTER, &qty_str, egui::FontId::monospace(13.0), dark);

        // Notional section
        let not_rect = egui::Rect::from_min_size(egui::pos2(qty_rect.right(), by), egui::vec2(notional_w, badge_h));
        painter.rect_filled(not_rect, 0.0, egui::Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), badge_alpha.saturating_sub(10).saturating_add(hover_boost)));
        painter.text(not_rect.center(), egui::Align2::CENTER_CENTER, &notional_str, egui::FontId::monospace(13.0), dark);

        // Status section (DRAFT / LIVE)
        let status_rect = egui::Rect::from_min_size(egui::pos2(not_rect.right(), by), egui::vec2(status_w, badge_h));
        painter.rect_filled(status_rect, 0.0, egui::Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), badge_alpha.saturating_sub(40).saturating_add(hover_boost)));
        painter.text(status_rect.center(), egui::Align2::CENTER_CENTER, status_label, egui::FontId::monospace(7.5), dark);

        // SEND button for drafts (clickable, with hover)
        if is_draft {
            let send_rect = egui::Rect::from_min_size(egui::pos2(status_rect.right(), by), egui::vec2(send_w, badge_h));
            let send_hovered = ui.input(|i| i.pointer.hover_pos()).map_or(false, |p| send_rect.contains(p));
            let send_bg = if send_hovered {
                egui::Color32::from_rgba_unmultiplied(t.accent.r(), t.accent.g(), t.accent.b(), 180)
            } else {
                egui::Color32::from_rgba_unmultiplied(t.accent.r(), t.accent.g(), t.accent.b(), 120)
            };
            painter.rect_filled(send_rect, 0.0, send_bg);
            painter.text(send_rect.center(), egui::Align2::CENTER_CENTER, "SEND", egui::FontId::monospace(8.0), egui::Color32::WHITE);
            if send_hovered { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
        }

        // X cancel button (hover state)
        let x_start = if is_draft { status_rect.right() + send_w } else { status_rect.right() };
        let x_rect = egui::Rect::from_min_size(egui::pos2(x_start, by), egui::vec2(x_btn_w, badge_h));
        let x_hovered = ui.input(|i| i.pointer.hover_pos()).map_or(false, |p| x_rect.contains(p));
        let x_bg_alpha = if x_hovered { 160 } else { 80 };
        painter.rect_filled(x_rect, egui::CornerRadius { nw: 0, sw: 0, ne: 3, se: 3 },
            color_alpha(t.bear, x_bg_alpha));
        painter.text(x_rect.center(), egui::Align2::CENTER_CENTER, Icon::X, egui::FontId::monospace(9.0),
            if x_hovered { egui::Color32::WHITE } else { t.bear });
        if x_hovered { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }

        // Price label — outside badge, slightly above the line, right of badge
        let price_d = if order.price >= 10.0 { 2 } else { 4 };
        chart.fmt_buf.clear(); let _ = write!(chart.fmt_buf, "{:.1$}", order.price, price_d);
        painter.text(
            egui::pos2(full_badge.right() + 6.0, y - 11.0),
            egui::Align2::LEFT_BOTTOM, &chart.fmt_buf, egui::FontId::monospace(9.0),
            egui::Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 200));

        // Y-axis price label
        let axis_rect = egui::Rect::from_min_size(egui::pos2(rect.left() + cw + 1.0, y - 9.0), egui::vec2(pr - 2.0, 18.0));
        painter.rect_filled(axis_rect, 2.0, color);
        painter.text(egui::pos2(axis_rect.center().x, axis_rect.center().y), egui::Align2::CENTER_CENTER,
            &chart.fmt_buf, egui::FontId::monospace(9.0), dark);
    }

    // ── Play lines on chart (companion for play editor) ────────────
    if !chart.play_lines.is_empty() {
        // Zone bands: entry→T1 (green), T1→T2 (teal), T2→T3 (cyan), entry→stop (red)
        let entry_price = chart.play_lines.iter().find(|l| l.kind == crate::chart_renderer::PlayLineKind::Entry).map(|l| l.price);
        let target_price = chart.play_lines.iter().find(|l| l.kind == crate::chart_renderer::PlayLineKind::Target).map(|l| l.price);
        let t2_price = chart.play_lines.iter().find(|l| l.kind == crate::chart_renderer::PlayLineKind::Target2).map(|l| l.price);
        let t3_price = chart.play_lines.iter().find(|l| l.kind == crate::chart_renderer::PlayLineKind::Target3).map(|l| l.price);
        let stop_price = chart.play_lines.iter().find(|l| l.kind == crate::chart_renderer::PlayLineKind::Stop).map(|l| l.price);

        // Entry → T1 (green)
        if let (Some(ep), Some(tp)) = (entry_price, target_price) {
            let (y1, y2) = (py(ep), py(tp));
            painter.rect_filled(egui::Rect::from_min_max(
                egui::pos2(rect.left(), y1.min(y2)), egui::pos2(rect.left() + cw, y1.max(y2))),
                0.0, egui::Color32::from_rgba_unmultiplied(46, 204, 113, 12));
        }
        // T1 → T2 (teal — slightly different shade)
        if let (Some(tp), Some(t2)) = (target_price, t2_price) {
            let (y1, y2) = (py(tp), py(t2));
            painter.rect_filled(egui::Rect::from_min_max(
                egui::pos2(rect.left(), y1.min(y2)), egui::pos2(rect.left() + cw, y1.max(y2))),
                0.0, egui::Color32::from_rgba_unmultiplied(26, 188, 156, 10));
        }
        // T2 → T3 (cyan — another shade)
        if let (Some(t2), Some(t3)) = (t2_price, t3_price) {
            let (y1, y2) = (py(t2), py(t3));
            painter.rect_filled(egui::Rect::from_min_max(
                egui::pos2(rect.left(), y1.min(y2)), egui::pos2(rect.left() + cw, y1.max(y2))),
                0.0, egui::Color32::from_rgba_unmultiplied(52, 152, 219, 10));
        }
        // Entry → Stop (red)
        if let (Some(ep), Some(sp)) = (entry_price, stop_price) {
            let (y1, y2) = (py(ep), py(sp));
            painter.rect_filled(egui::Rect::from_min_max(
                egui::pos2(rect.left(), y1.min(y2)), egui::pos2(rect.left() + cw, y1.max(y2))),
                0.0, egui::Color32::from_rgba_unmultiplied(231, 76, 60, 10));
        }

        // R:R connector + label
        if let (Some(tp), Some(sp)) = (target_price, stop_price) {
            let ty_y = py(tp);
            let sy_y = py(sp);
            let cx_x = rect.left() + cw - 30.0;
            // Dotted connector
            let mut dy = ty_y.min(sy_y);
            while dy < ty_y.max(sy_y) {
                let end = (dy + 3.0).min(ty_y.max(sy_y));
                painter.line_segment(
                    [egui::pos2(cx_x, dy), egui::pos2(cx_x, end)],
                    egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(100, 140, 255, 100)));
                dy += 6.0;
            }
            // Ticks
            painter.line_segment(
                [egui::pos2(cx_x - 4.0, ty_y), egui::pos2(cx_x + 4.0, ty_y)],
                egui::Stroke::new(1.0, color_alpha(t.bull, 180)));
            painter.line_segment(
                [egui::pos2(cx_x - 4.0, sy_y), egui::pos2(cx_x + 4.0, sy_y)],
                egui::Stroke::new(1.0, color_alpha(t.bear, 180)));
            // R:R label
            if let Some(ep) = entry_price {
                let reward = (tp - ep).abs();
                let risk = (sp - ep).abs();
                if risk > 0.0 {
                    let rr = reward / risk;
                    let mid_y = (ty_y + sy_y) / 2.0;
                    let rr_text = format!("R:R {:.1}:1", rr);
                    let rr_galley = painter.layout_no_wrap(rr_text.clone(), egui::FontId::monospace(8.0), egui::Color32::from_rgb(100, 140, 255));
                    let rr_bg = egui::Rect::from_min_size(
                        egui::pos2(cx_x - rr_galley.size().x / 2.0 - 3.0, mid_y - rr_galley.size().y / 2.0 - 2.0),
                        egui::vec2(rr_galley.size().x + 6.0, rr_galley.size().y + 4.0));
                    painter.rect_filled(rr_bg, 3.0, egui::Color32::from_rgba_unmultiplied(t.toolbar_bg.r(), t.toolbar_bg.g(), t.toolbar_bg.b(), 220));
                    painter.text(egui::pos2(cx_x, mid_y), egui::Align2::CENTER_CENTER, &rr_text,
                        egui::FontId::monospace(8.0), egui::Color32::from_rgb(100, 140, 255));
                }
            }
        }

        // Individual play lines
        let play_color_base = egui::Color32::from_rgb(100, 140, 255); // blue
        for pl in &chart.play_lines {
            let y = py(pl.price);
            if y < rect.top() + pt || y > rect.top() + pt + ch { continue; }

            let line_color = match pl.kind {
                crate::chart_renderer::PlayLineKind::Entry => play_color_base,
                crate::chart_renderer::PlayLineKind::Target | crate::chart_renderer::PlayLineKind::Target2 | crate::chart_renderer::PlayLineKind::Target3 => t.bull,
                crate::chart_renderer::PlayLineKind::Stop => t.bear,
            };

            // Dashed line (longer dashes than orders for distinction)
            let dash_color = egui::Color32::from_rgba_unmultiplied(line_color.r(), line_color.g(), line_color.b(), 150);
            let mut dx = rect.left();
            while dx < rect.left() + cw {
                let end = (dx + 8.0).min(rect.left() + cw);
                painter.line_segment([egui::pos2(dx, y), egui::pos2(end, y)], egui::Stroke::new(1.0, dash_color));
                dx += 14.0;
            }

            // Badge: [E/T/S] [PRICE] [PLAY]
            let kind_label = pl.kind.short();
            let price_d = if pl.price >= 10.0 { 2 } else { 4 };
            let price_str = format!("{:.1$}", pl.price, price_d);
            let dark = t.bg;
            let badge_h = 20.0;

            let kind_w = if kind_label.len() > 1 { 24.0 } else { 18.0 };
            let price_w = price_str.len() as f32 * 8.0 + 10.0;
            let label_w = 32.0; // "PLAY"
            let total_w = kind_w + price_w + label_w;
            let bx = rect.left() + cw * 0.50 - total_w * 0.5;
            let by = y - badge_h * 0.5;

            let badge_hovered = ui.input(|i| i.pointer.hover_pos()).map_or(false, |p|
                egui::Rect::from_min_size(egui::pos2(bx, by), egui::vec2(total_w, badge_h)).contains(p));
            let hb: u8 = if badge_hovered { 30 } else { 0 };

            // Kind section
            let kind_rect = egui::Rect::from_min_size(egui::pos2(bx, by), egui::vec2(kind_w, badge_h));
            painter.rect_filled(kind_rect, egui::CornerRadius { nw: 3, sw: 3, ne: 0, se: 0 },
                egui::Color32::from_rgba_unmultiplied(line_color.r(), line_color.g(), line_color.b(), 220u8.saturating_add(hb)));
            painter.text(kind_rect.center(), egui::Align2::CENTER_CENTER, kind_label,
                egui::FontId::monospace(10.0), dark);

            // Price section
            let price_rect = egui::Rect::from_min_size(egui::pos2(kind_rect.right(), by), egui::vec2(price_w, badge_h));
            painter.rect_filled(price_rect, 0.0,
                egui::Color32::from_rgba_unmultiplied(line_color.r(), line_color.g(), line_color.b(), 180u8.saturating_add(hb)));
            painter.text(price_rect.center(), egui::Align2::CENTER_CENTER, &price_str,
                egui::FontId::monospace(9.0), dark);

            // "PLAY" label section
            let play_rect = egui::Rect::from_min_size(egui::pos2(price_rect.right(), by), egui::vec2(label_w, badge_h));
            painter.rect_filled(play_rect, egui::CornerRadius { nw: 0, sw: 0, ne: 3, se: 3 },
                egui::Color32::from_rgba_unmultiplied(line_color.r(), line_color.g(), line_color.b(), 140u8.saturating_add(hb)));
            painter.text(play_rect.center(), egui::Align2::CENTER_CENTER, "PLAY",
                egui::FontId::monospace(7.0), dark);

            // Y-axis price label
            let axis_rect = egui::Rect::from_min_size(egui::pos2(rect.left() + cw + 1.0, y - 8.0), egui::vec2(pr - 2.0, 16.0));
            painter.rect_filled(axis_rect, 2.0, line_color);
            painter.text(axis_rect.center(), egui::Align2::CENTER_CENTER, &price_str,
                egui::FontId::monospace(8.0), dark);

            if badge_hovered { ui.ctx().set_cursor_icon(egui::CursorIcon::Grab); }
        }
    }

    // ── Trailing stop distance visualization ─────────────────────────
    for order in &chart.orders {
        if order.status == OrderStatus::Cancelled || order.status == OrderStatus::Executed { continue; }
        if order.side != OrderSide::Stop { continue; }
        if let Some(trail_amt) = order.trail_amount {
            if trail_amt <= 0.0 { continue; }
            // The trail distance is the amount below (for long) or above (for short) the current price
            // Show a faint dashed line at the trail offset from the current stop price
            let trail_offset_price = order.price + trail_amt;
            let trail_y = py(trail_offset_price);
            if trail_y.is_finite() && trail_y >= rect.top() + pt && trail_y <= rect.top() + pt + ch {
                let trail_color = egui::Color32::from_rgba_unmultiplied(t.bear.r(), t.bear.g(), t.bear.b(), 60);
                let mut dx = rect.left();
                while dx < rect.left() + cw {
                    let end = (dx + 4.0).min(rect.left() + cw);
                    painter.line_segment([egui::pos2(dx, trail_y), egui::pos2(end, trail_y)], egui::Stroke::new(0.8, trail_color));
                    dx += 8.0;
                }
                // Small label
                let trail_label = format!("TRAIL +{:.2}", trail_amt);
                let trail_font = egui::FontId::monospace(7.0);
                painter.text(egui::pos2(rect.left() + cw - 8.0, trail_y - 7.0), egui::Align2::RIGHT_CENTER, &trail_label, trail_font, trail_color);
                // Dotted vertical connector from stop to trail reference
                let stop_y = py(order.price);
                if stop_y.is_finite() {
                    let top = stop_y.min(trail_y);
                    let bot = stop_y.max(trail_y);
                    let cx = rect.left() + cw - 30.0;
                    let mut dy = top;
                    while dy < bot {
                        let end = (dy + 2.0).min(bot);
                        painter.line_segment([egui::pos2(cx, dy), egui::pos2(cx, end)], egui::Stroke::new(0.6, trail_color));
                        dy += 5.0;
                    }
                }
            }
        } else if let Some(trail_pct) = order.trail_percent {
            if trail_pct <= 0.0 { continue; }
            // Calculate the trail reference price from the stop price and trail percent
            let trail_offset_price = order.price * (1.0 + trail_pct / 100.0);
            let trail_y = py(trail_offset_price);
            if trail_y.is_finite() && trail_y >= rect.top() + pt && trail_y <= rect.top() + pt + ch {
                let trail_color = egui::Color32::from_rgba_unmultiplied(t.bear.r(), t.bear.g(), t.bear.b(), 60);
                let mut dx = rect.left();
                while dx < rect.left() + cw {
                    let end = (dx + 4.0).min(rect.left() + cw);
                    painter.line_segment([egui::pos2(dx, trail_y), egui::pos2(end, trail_y)], egui::Stroke::new(0.8, trail_color));
                    dx += 8.0;
                }
                let trail_label = format!("TRAIL +{:.1}%", trail_pct);
                let trail_font = egui::FontId::monospace(7.0);
                painter.text(egui::pos2(rect.left() + cw - 8.0, trail_y - 7.0), egui::Align2::RIGHT_CENTER, &trail_label, trail_font, trail_color);
                // Dotted vertical connector
                let stop_y = py(order.price);
                if stop_y.is_finite() {
                    let top = stop_y.min(trail_y);
                    let bot = stop_y.max(trail_y);
                    let cx = rect.left() + cw - 30.0;
                    let mut dy = top;
                    while dy < bot {
                        let end = (dy + 2.0).min(bot);
                        painter.line_segment([egui::pos2(cx, dy), egui::pos2(cx, end)], egui::Stroke::new(0.6, trail_color));
                        dy += 5.0;
                    }
                }
            }
        }
    }

    // ── Price alert lines on chart ────────────────────────
    // Interactive state (drag, click) is handled in the event-priority blocks
    // below (hit_alert_line, chart.dragging_alert). This block only paints.
    // Click hitboxes for PLACE/X are stashed in thread-locals and handled in
    // the same priority-0 block that processes other overlay clicks.
    {
        let placed_color = COLOR_AMBER; // amber = placed
        let draft_color  = t.bear; // red = draft (needs attention)
        let hover_pos = ui.input(|i| i.pointer.hover_pos());
        ALERT_BADGE_HITS.with(|h| h.borrow_mut().clear());
        let alert_ids: Vec<u32> = chart.price_alerts.iter()
            .filter(|a| !a.triggered && a.symbol == chart.symbol)
            .map(|a| a.id).collect();
        for &aid in &alert_ids {
            let alert = chart.price_alerts.iter().find(|a| a.id == aid).unwrap();
            let is_draft = alert.draft;
            let alert_color = if is_draft { draft_color } else { placed_color };
            let y = py(alert.price);
            if !y.is_finite() || y < rect.top() + pt || y > rect.top() + pt + ch { continue; }

            // Hover/drag feedback based on chart state
            let is_dragging = chart.dragging_alert == Some(aid);
            let is_hovered = hover_pos.map_or(false, |p| (p.y - y).abs() <= 10.0 && p.x >= rect.left() && p.x <= rect.left() + cw);

            // Line: drafts = red dashed, placed = amber dashed
            let base_alpha = if is_draft { 220 } else { 180 };
            let dash_col = egui::Color32::from_rgba_unmultiplied(
                alert_color.r(), alert_color.g(), alert_color.b(),
                if is_hovered || is_dragging { 255 } else { base_alpha });
            let mut dx = rect.left();
            let (dash, gap) = if is_draft { (6.0, 4.0) } else { (5.0, 4.0) };
            let line_width = if is_draft { 1.5 } else { 1.0 };
            while dx < rect.left() + cw {
                let end_x = (dx + dash).min(rect.left() + cw);
                painter.line_segment([egui::pos2(dx, y), egui::pos2(end_x, y)],
                    egui::Stroke::new(if is_hovered { line_width + 0.5 } else { line_width }, dash_col));
                dx += dash + gap;
            }

            // Drag handle pill at center of line — visual grab target
            {
                let cx = rect.left() + cw / 2.0;
                let (pill_w, pill_h) = if is_hovered || is_dragging { (12.0, 7.0) } else { (8.0, 5.0) };
                let pill_rect = egui::Rect::from_center_size(
                    egui::pos2(cx, y),
                    egui::vec2(pill_w, pill_h));
                // Solid fill in alert color, slightly darker border
                painter.rect_filled(pill_rect, pill_h / 2.0, alert_color);
                painter.rect_stroke(pill_rect, pill_h / 2.0,
                    egui::Stroke::new(0.5, egui::Color32::from_rgba_unmultiplied(0, 0, 0, 120)),
                    egui::StrokeKind::Outside);
                // 3 dots for a "grab handle" visual cue
                let dot_col = egui::Color32::from_rgba_unmultiplied(255, 255, 255, 180);
                painter.circle_filled(egui::pos2(cx - 2.0, y), 0.7, dot_col);
                painter.circle_filled(egui::pos2(cx,       y), 0.7, dot_col);
                painter.circle_filled(egui::pos2(cx + 2.0, y), 0.7, dot_col);
            }

            // Badge
            let dir_arrow = if alert.above { "\u{25B2}" } else { "\u{25BC}" };
            let d = if alert.price >= 10.0 { 2 } else { 4 };
            let label_font = egui::FontId::monospace(if is_draft { 10.0 } else { 9.0 });

            if is_draft {
                // ── DRAFT: big red badge with solid fill + PLACE + X buttons ──
                let label_text = format!("DRAFT {} {:.prec$}", dir_arrow, alert.price, prec = d);
                let galley = painter.layout_no_wrap(label_text.clone(), label_font.clone(), egui::Color32::WHITE);
                let place_w = 42.0; let x_w = 20.0; let pad = 8.0;
                let badge_h = galley.size().y + 10.0;
                let badge_w = galley.size().x + pad * 2.0 + place_w + x_w + 6.0;
                let lx = rect.left() + cw - badge_w - 4.0;
                let badge_rect = egui::Rect::from_min_size(egui::pos2(lx, y - badge_h / 2.0), egui::vec2(badge_w, badge_h));
                painter.rect_filled(badge_rect, 4.0, alert_color);
                painter.rect_stroke(badge_rect, 4.0,
                    egui::Stroke::new(1.0, t.bear), egui::StrokeKind::Outside);
                painter.text(egui::pos2(lx + pad, y), egui::Align2::LEFT_CENTER, &label_text, label_font, egui::Color32::WHITE);

                // PLACE button
                let place_rect = egui::Rect::from_min_size(
                    egui::pos2(badge_rect.right() - place_w - x_w - 4.0, badge_rect.top() + 3.0),
                    egui::vec2(place_w, badge_h - 6.0));
                let place_hover = hover_pos.map_or(false, |p| place_rect.contains(p));
                let place_bg = if place_hover { egui::Color32::WHITE }
                    else { egui::Color32::from_rgba_unmultiplied(255, 255, 255, 230) };
                painter.rect_filled(place_rect, 3.0, place_bg);
                let place_fg = if place_hover { alert_color } else { t.bear };
                painter.text(place_rect.center(), egui::Align2::CENTER_CENTER, "PLACE", egui::FontId::monospace(9.0), place_fg);

                // X cancel
                let x_rect = egui::Rect::from_min_size(
                    egui::pos2(badge_rect.right() - x_w - 2.0, badge_rect.top() + 3.0),
                    egui::vec2(x_w, badge_h - 6.0));
                let x_hover = hover_pos.map_or(false, |p| x_rect.contains(p));
                if x_hover { painter.rect_filled(x_rect, 3.0, egui::Color32::from_rgba_unmultiplied(0, 0, 0, 80)); }
                painter.text(x_rect.center(), egui::Align2::CENTER_CENTER, "\u{00D7}",
                    egui::FontId::monospace(14.0), egui::Color32::WHITE);

                // Stash rects for the priority-0 click handler
                ALERT_BADGE_HITS.with(|h| h.borrow_mut().push(AlertBadgeHit {
                    alert_id: aid, is_draft: true, place_rect, x_rect, drag_line_y: y,
                }));
            } else {
                // ── PLACED: amber compact badge with X ──
                let label_text = format!("Alert {} {:.prec$}", dir_arrow, alert.price, prec = d);
                let galley = painter.layout_no_wrap(label_text.clone(), label_font.clone(), alert_color);
                let lx = rect.left() + cw - galley.size().x - 24.0;
                let badge_rect = egui::Rect::from_min_size(
                    egui::pos2(lx - 4.0, y - galley.size().y / 2.0 - 2.0),
                    egui::vec2(galley.size().x + 24.0, galley.size().y + 4.0));
                painter.rect_filled(badge_rect, 3.0, egui::Color32::from_rgba_unmultiplied(
                    t.toolbar_bg.r(), t.toolbar_bg.g(), t.toolbar_bg.b(), 220));
                painter.rect_stroke(badge_rect, 3.0, egui::Stroke::new(0.5, alert_color), egui::StrokeKind::Outside);
                painter.text(egui::pos2(lx, y), egui::Align2::LEFT_CENTER, &label_text, label_font, alert_color);
                let x_rect = egui::Rect::from_min_size(
                    egui::pos2(badge_rect.right() - 16.0, badge_rect.top() + 2.0),
                    egui::vec2(14.0, badge_rect.height() - 4.0));
                let x_hover = hover_pos.map_or(false, |p| x_rect.contains(p));
                painter.text(x_rect.center(), egui::Align2::CENTER_CENTER, "\u{00D7}",
                    egui::FontId::monospace(10.0),
                    if x_hover { alert_color } else { alert_color.gamma_multiply(0.6) });
                ALERT_BADGE_HITS.with(|h| h.borrow_mut().push(AlertBadgeHit {
                    alert_id: aid, is_draft: false,
                    place_rect: egui::Rect::NOTHING, x_rect, drag_line_y: y,
                }));
            }
        }
    }

    // ── Order edit popup (double-click) ──────────────────────────────────
    if let Some(edit_id) = chart.editing_order {
        // Extract order data to avoid borrow conflict
        let order_data = chart.orders.iter().find(|o| o.id == edit_id)
            .map(|o| (o.price, o.color(t.bull, t.bear), o.label(), o.option_symbol.clone(), o.side));

        if let Some((order_price, color, order_label, opt_sym, side)) = order_data {
            let y = py(order_price);
            let approx_badge_center = rect.left() + cw * 0.60;
            let out = crate::chart_renderer::ui::widgets::trading::show_order_edit_dialog(
                crate::chart_renderer::ui::widgets::trading::order_edit_dialog::OrderEditCtx {
                    ctx,
                    t,
                    edit_id,
                    badge_y: y,
                    approx_badge_center,
                    edit_price: &mut chart.edit_order_price,
                    edit_qty: &mut chart.edit_order_qty,
                    order_price,
                    order_label: order_label.to_string(),
                    order_color: color,
                    order_side: side,
                    opt_sym,
                    symbol: chart.symbol.clone(),
                },
            );
            if let Some(p) = out.apply_price {
                crate::chart_renderer::trading::order_manager::modify_order_price(edit_id as u64, p);
                if let Some(o) = chart.orders.iter_mut().find(|o| o.id == edit_id) { o.price = p; }
            }
            if let Some(q) = out.apply_qty {
                if let Some(o) = chart.orders.iter_mut().find(|o| o.id == edit_id) { o.qty = q; }
            }
            if out.cancel_it {
                crate::chart_renderer::trading::order_manager::cancel_order(edit_id as u64);
                cancel_order_with_pair(&mut chart.orders, edit_id);
                chart.editing_order = None;
            }
            if out.close_editor { chart.editing_order = None; }
        } else {
            chart.editing_order = None;
        }
    }

    // ── Order entry panel (bottom-left of pane) ─────────────────────────
    if watchlist.order_entry_open {
        // Auto-expand advanced mode for option charts (UND is there)
        if chart.is_option && !chart.order_advanced {
            chart.order_advanced = true;
            chart.order_type_idx = 5; // default to UND for options
        }
        let abs_pos = if chart.order_panel_pos.y < 0.0 {
            egui::pos2(rect.left() + chart.order_panel_pos.x, rect.top() + pt + ch + chart.order_panel_pos.y)
        } else {
            egui::pos2(rect.left() + chart.order_panel_pos.x, rect.top() + pt + chart.order_panel_pos.y)
        };

        crate::chart_renderer::ui::widgets::trading::show_order_entry_panel(
            crate::chart_renderer::ui::widgets::trading::order_entry_panel::OrderEntryPanelCtx {
                ctx,
                t,
                chart,
                watchlist,
                account_data_cached: &account_data_cached,
                abs_pos,
                pane_idx,
                cw,
                ch,
            },
        );

        // ── Pending confirm toasts (above order entry panel) ─────────
        let base_y = rect.top() + pt + ch - 120.0 - 28.0;
        crate::chart_renderer::ui::widgets::trading::show_pending_order_toasts(
            crate::chart_renderer::ui::widgets::trading::pending_order_toasts::PendingOrderToastsCtx {
                ctx,
                t,
                chart,
                pane_idx,
                base_y,
                rect_left: rect.left(),
            },
        );
    }


    // Middle-click flow:
    //   1st click (no tool active) → activate trendline (quick path)
    //   2nd click (a tool already active) → open the favorites picker at cursor
    //   Click while picker is open → close it
    if ui.input(|i| i.pointer.button_clicked(egui::PointerButton::Middle)) && pointer_in_pane {
        if chart.draw_picker_open {
            chart.draw_picker_open = false;
        } else if chart.draw_tool.is_empty() {
            chart.draw_tool = "trendline".to_string();
            chart.pending_pt = None; chart.pending_pt2 = None; chart.pending_pts.clear();
        } else {
            // Open picker at the cursor position
            if let Some(pos) = ui.input(|i| i.pointer.hover_pos()) {
                chart.draw_picker_pos = pos;
                chart.draw_picker_open = true;
            }
        }
    }

    // ── OHLC Magnet snap ─────────────────────────────────────────────────
    // When magnet is on and we're either placing or dragging a drawing,
    // snap to the nearest Open/High/Low/Close of the bar under the cursor.
    // snap_bar/snap_price are available for both preview and placement.
    let mut snap_bar: Option<f32> = None;
    let mut snap_price: Option<f32> = None;
    let magnet_active = chart.magnet && pointer_in_pane
        && (!chart.draw_tool.is_empty() || chart.dragging_drawing.is_some());
    if magnet_active {
        if let Some(pos) = ui.input(|i| i.pointer.hover_pos()) {
            let raw_bar = (pos.x - rect.left() + off - bs*0.5) / bs + vs;
            let bar_idx = if raw_bar < 0.0 { 0 } else { (raw_bar.round() as usize).min(chart.bars.len().saturating_sub(1)) };
            if !chart.bars.is_empty() { if let Some(bar_data) = chart.bars.get(bar_idx) {
                let ohlc = [bar_data.open, bar_data.high, bar_data.low, bar_data.close];
                let bar_x = bx(bar_idx as f32);
                let magnet_radius = 20.0_f32; // pixels

                // Highlight the candle body with a subtle glow
                let body_top = py(bar_data.open.max(bar_data.close));
                let body_bot = py(bar_data.open.min(bar_data.close));
                let wick_top = py(bar_data.high);
                let wick_bot = py(bar_data.low);
                let bw = (bs * 0.35).max(1.0);
                painter.rect_filled(
                    egui::Rect::from_min_size(egui::pos2(bar_x - bw - 2.0, wick_top - 2.0), egui::vec2(bw * 2.0 + 4.0, (wick_bot - wick_top) + 4.0)),
                    3.0, color_alpha(t.accent, 20));
                painter.rect_stroke(
                    egui::Rect::from_min_size(egui::pos2(bar_x - bw - 2.0, wick_top - 2.0), egui::vec2(bw * 2.0 + 4.0, (wick_bot - wick_top) + 4.0)),
                    3.0, egui::Stroke::new(0.5, color_alpha(t.accent, 50)), egui::StrokeKind::Outside);

                // Draw small dots at all 4 OHLC levels
                for &p in &ohlc {
                    let y = py(p);
                    if y.is_finite() && y.abs() < 50000.0 {
                        painter.circle_filled(egui::pos2(bar_x, y), 2.0, color_alpha(t.text,60));
                    }
                }

                // Find nearest OHLC to cursor y
                let mut best_dist = f32::MAX;
                let mut best_price = 0.0_f32;
                for &p in &ohlc {
                    let y = py(p);
                    let dist = (pos.y - y).abs();
                    if dist < best_dist {
                        best_dist = dist;
                        best_price = p;
                    }
                }

                if best_dist < magnet_radius {
                    snap_bar = Some(bar_idx as f32);
                    snap_price = Some(best_price);
                    // Highlight the snapped level
                    let sy = py(best_price);
                    painter.circle_filled(egui::pos2(bar_x, sy), 4.5, t.accent);
                    painter.circle_stroke(egui::pos2(bar_x, sy), 4.5, egui::Stroke::new(1.0, egui::Color32::WHITE));
                    // Label which OHLC level
                    let ohlc_labels = ["O", "H", "L", "C"];
                    let snapped_idx = ohlc.iter().position(|&p| (p - best_price).abs() < 0.0001).unwrap_or(0);
                    let label = ohlc_labels[snapped_idx];
                    painter.text(egui::pos2(bar_x + 8.0, sy - 1.0), egui::Align2::LEFT_CENTER, label, egui::FontId::monospace(8.0), t.accent);
                    // Horizontal guide line
                    painter.line_segment(
                        [egui::pos2(rect.left(), sy), egui::pos2(rect.left() + cw, sy)],
                        egui::Stroke::new(0.5, color_alpha(t.text,30)));
                }
            }
        } }
    }

    // Drawing preview + custom cursors (only in hovered pane)
    let blue = egui::Color32::from_rgb(70, 130, 255);
    if pointer_in_pane { if let Some(pos) = ui.input(|i| i.pointer.hover_pos()) {
        // Tool status indicator — floating label at top of chart
        if !chart.draw_tool.is_empty() {
            let tool_name = match chart.draw_tool.as_str() {
                "trendline" => "Trendline — click 2 points",
                "hline" => "H-Line — click to place",
                "hzone" => "Zone — click 2 prices",
                "fibonacci" => "Fibonacci — click start then end",
                "channel" => "Channel — click 2 points then offset",
                "fibchannel" => "Fib Channel — click 2 points then offset",
                "pitchfork" => "Pitchfork — click pivot then 2 reactions",
                "gannfan" => "Gann Fan — click origin then scale point",
                "regression" => "Regression — click start then end",
                "xabcd" => "XABCD — click 5 points (X,A,B,C,D)",
                "vline" => "Vertical Line — click to place",
                "ray" => "Ray — click 2 points",
                "fibext" => "Fib Extension — click A, B, then C",
                "fibtimezone" => "Fib Time Zones — click anchor",
                "fibarc" => "Fib Arcs — click 2 points",
                "gannbox" => "Gann Box — click 2 corners",
                "pricerange" => "Price Range — click 2 corners",
                "riskreward" => "Risk/Reward — click entry, stop, target",
                "textnote" => "Text Note — click to place",
                s if s.starts_with("elliott") => "Elliott Wave — click wave points",
                _ => "",
            };
            if !tool_name.is_empty() {
                let status_text = format!("{}  [ESC cancel] [M magnet {}]", tool_name, if chart.magnet { "ON" } else { "OFF" });
                let galley = painter.layout_no_wrap(status_text.clone(), egui::FontId::monospace(9.0), color_alpha(t.text,180));
                let status_pos = egui::pos2(rect.left() + (cw - galley.size().x) / 2.0, rect.top() + pt + 6.0);
                let bg_rect = egui::Rect::from_min_size(status_pos - egui::vec2(6.0, 3.0), galley.size() + egui::vec2(12.0, 6.0));
                painter.rect_filled(bg_rect, 4.0, egui::Color32::from_rgba_unmultiplied(t.toolbar_bg.r(), t.toolbar_bg.g(), t.toolbar_bg.b(), 200));
                painter.text(status_pos + egui::vec2(galley.size().x / 2.0, galley.size().y / 2.0), egui::Align2::CENTER_CENTER, &status_text, egui::FontId::monospace(9.0), color_alpha(t.text,180));
            }
        }
        if chart.draw_tool == "trendline" {
            // Accent-colored crosshair + dashed preview line from first click
            if let Some((b0, p0)) = chart.pending_pt {
                let start = egui::pos2(bx(b0), py(p0));
                let dir = pos - start;
                let len = dir.length();
                if len > 2.0 {
                    let dash_len = 6.0; let gap_len = 4.0; let step = dash_len + gap_len;
                    let norm = dir / len;
                    let mut d = 0.0;
                    while d < len {
                        let a = start + norm * d;
                        let b = start + norm * (d + dash_len).min(len);
                        painter.line_segment([a, b], egui::Stroke::new(1.5, color_alpha(t.accent, 200)));
                        d += step;
                    }
                }
                painter.circle_filled(start, 3.0, t.accent);
                painter.circle_filled(pos, 3.0, t.accent);
            }
            // Accent crosshair
            let ch_len = 8.0;
            painter.line_segment([pos - egui::vec2(ch_len, 0.0), pos + egui::vec2(ch_len, 0.0)], egui::Stroke::new(1.0, t.accent));
            painter.line_segment([pos - egui::vec2(0.0, ch_len), pos + egui::vec2(0.0, ch_len)], egui::Stroke::new(1.0, t.accent));
            ui.ctx().set_cursor_icon(egui::CursorIcon::None);
        } else if chart.draw_tool == "hline" {
            // Blue horizontal line preview following mouse
            if pos.y >= rect.top()+pt && pos.y < rect.top()+pt+ch {
                painter.line_segment(
                    [egui::pos2(rect.left(), pos.y), egui::pos2(rect.left()+cw, pos.y)],
                    egui::Stroke::new(1.0, color_alpha(blue, 160)),
                );
                // Price label on right edge
                let hp = min_p + (max_p - min_p) * (1.0 - (pos.y - rect.top() - pt) / ch);
                let price_label = format!("{:.2}", hp);
                painter.text(egui::pos2(rect.left() + cw + 3.0, pos.y), egui::Align2::LEFT_CENTER, &price_label, egui::FontId::monospace(8.5), blue);
            }
            // Blue crosshair
            let ch_len = 8.0;
            painter.line_segment([pos - egui::vec2(ch_len, 0.0), pos + egui::vec2(ch_len, 0.0)], egui::Stroke::new(1.0, blue));
            painter.line_segment([pos - egui::vec2(0.0, ch_len), pos + egui::vec2(0.0, ch_len)], egui::Stroke::new(1.0, blue));
            ui.ctx().set_cursor_icon(egui::CursorIcon::None);
        } else if chart.draw_tool == "hzone" {
            if let Some((_b0, p0)) = chart.pending_pt {
                // Rectangle preview from first click to cursor
                let y0 = py(p0);
                painter.rect_filled(
                    egui::Rect::from_min_max(egui::pos2(rect.left(), y0.min(pos.y)), egui::pos2(rect.left()+cw, y0.max(pos.y))),
                    0.0, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 15));
                painter.line_segment([egui::pos2(rect.left(),y0),egui::pos2(rect.left()+cw,y0)], egui::Stroke::new(1.0, color_alpha(t.text,120)));
                painter.line_segment([egui::pos2(rect.left(),pos.y),egui::pos2(rect.left()+cw,pos.y)], egui::Stroke::new(1.0, color_alpha(t.text,120)));
            }
            // White rectangle cursor
            let sz = 6.0;
            painter.rect_stroke(
                egui::Rect::from_center_size(pos, egui::vec2(sz * 2.0, sz * 2.0)),
                1.0, egui::Stroke::new(1.0, egui::Color32::WHITE), egui::StrokeKind::Outside);
            ui.ctx().set_cursor_icon(egui::CursorIcon::None);
        } else if chart.draw_tool == "fibonacci" {
            // Fibonacci preview: retracement + extension levels
            let fib_color = egui::Color32::from_rgb(255, 193, 37); // gold
            if let Some((b0, p0)) = chart.pending_pt {
                let price_cursor = min_p + (max_p - min_p) * (1.0 - (pos.y - rect.top() - pt) / ch);
                let x0 = bx(b0); let x1 = pos.x;
                let xl = x0.min(x1); let xr = x0.max(x1);
                let range = price_cursor - p0;
                // Retracement levels
                let retrace = [0.0_f32, 0.236, 0.382, 0.5, 0.618, 0.786, 1.0];
                // Extension levels
                let extensions = [-0.272_f32, -0.618, 1.272, 1.414, 1.618, 2.0, 2.618, 3.146];
                for &lv in retrace.iter() {
                    let lp = p0 + range * lv;
                    let y = py(lp);
                    if y.is_finite() && y.abs() < 50000.0 {
                        painter.line_segment([egui::pos2(xl, y), egui::pos2(xr, y)],
                            egui::Stroke::new(0.8, color_alpha(fib_color, 140)));
                        painter.text(egui::pos2(xr + 4.0, y), egui::Align2::LEFT_CENTER,
                            &format!("{:.1}% {:.2}", lv * 100.0, lp), egui::FontId::monospace(8.0), color_alpha(fib_color, 200));
                    }
                }
                for &lv in extensions.iter() {
                    let lp = p0 + range * lv;
                    let y = py(lp);
                    if y.is_finite() && y.abs() < 50000.0 {
                        // Extensions: dashed, dimmer
                        let dash_len = 4.0; let gap_len = 4.0;
                        let a = egui::pos2(xl, y); let b_pt = egui::pos2(xr, y);
                        let dir = b_pt - a; let len = dir.length();
                        if len > 0.0 {
                            let norm = dir / len; let mut dd = 0.0;
                            while dd < len {
                                let s = a + norm * dd;
                                let e = a + norm * (dd + dash_len).min(len);
                                painter.line_segment([s, e], egui::Stroke::new(0.5, color_alpha(fib_color, 80)));
                                dd += dash_len + gap_len;
                            }
                        }
                        painter.text(egui::pos2(xr + 4.0, y), egui::Align2::LEFT_CENTER,
                            &format!("{:.1}% {:.2}", lv * 100.0, lp), egui::FontId::monospace(8.0), color_alpha(fib_color, 130));
                    }
                }
                // Shaded golden zone (38.2%-61.8%)
                let y382 = py(p0 + range * 0.382);
                let y618 = py(p0 + range * 0.618);
                if y382.is_finite() && y618.is_finite() {
                    painter.rect_filled(egui::Rect::from_min_max(
                        egui::pos2(xl, y382.min(y618)), egui::pos2(xr, y382.max(y618))),
                        0.0, egui::Color32::from_rgba_unmultiplied(255, 193, 37, 12));
                }
            }
            // Gold crosshair
            let ch_len = 8.0;
            painter.line_segment([pos - egui::vec2(ch_len, 0.0), pos + egui::vec2(ch_len, 0.0)], egui::Stroke::new(1.0, fib_color));
            painter.line_segment([pos - egui::vec2(0.0, ch_len), pos + egui::vec2(0.0, ch_len)], egui::Stroke::new(1.0, fib_color));
            ui.ctx().set_cursor_icon(egui::CursorIcon::None);
        } else if chart.draw_tool == "channel" || chart.draw_tool == "fibchannel" {
            // Channel / Fib-channel preview
            let is_fib = chart.draw_tool == "fibchannel";
            let chan_color = if is_fib { egui::Color32::from_rgb(196, 163, 90) } else { egui::Color32::from_rgb(130, 220, 180) };
            if let Some((b0, p0)) = chart.pending_pt {
                if let Some((b1, p1)) = chart.pending_pt2 {
                    let cursor_price = min_p + (max_p - min_p) * (1.0 - (pos.y - rect.top() - pt) / ch);
                    let base_mid_price = (p0 + p1) / 2.0;
                    let offset_price = cursor_price - base_mid_price;
                    let sx0 = bx(b0); let sx1 = bx(b1);
                    let sy0 = py(p0); let sy1 = py(p1);
                    let oy0 = py(p0 + offset_price); let oy1 = py(p1 + offset_price);
                    // Fill
                    let pts = vec![egui::pos2(sx0, sy0), egui::pos2(sx1, sy1), egui::pos2(sx1, oy1), egui::pos2(sx0, oy0)];
                    painter.add(egui::Shape::convex_polygon(pts, color_alpha(chan_color, 15), egui::Stroke::NONE));
                    // Base + parallel
                    painter.line_segment([egui::pos2(sx0, sy0), egui::pos2(sx1, sy1)], egui::Stroke::new(1.5, color_alpha(chan_color, 200)));
                    painter.line_segment([egui::pos2(sx0, oy0), egui::pos2(sx1, oy1)], egui::Stroke::new(1.5, color_alpha(chan_color, 200)));
                    // Midline
                    let my0 = (sy0 + oy0) / 2.0; let my1 = (sy1 + oy1) / 2.0;
                    painter.line_segment([egui::pos2(sx0, my0), egui::pos2(sx1, my1)], egui::Stroke::new(0.7, color_alpha(chan_color, 80)));
                    // Fib internal lines preview
                    if is_fib {
                        for &ratio in &[0.236_f32, 0.382, 0.618, 0.786] {
                            let fy0 = sy0 + (oy0 - sy0) * ratio;
                            let fy1 = sy1 + (oy1 - sy1) * ratio;
                            painter.line_segment([egui::pos2(sx0, fy0), egui::pos2(sx1, fy1)],
                                egui::Stroke::new(0.5, color_alpha(chan_color, 60)));
                        }
                    }
                } else {
                    let start = egui::pos2(bx(b0), py(p0));
                    painter.line_segment([start, pos], egui::Stroke::new(1.5, color_alpha(chan_color, 180)));
                    painter.circle_filled(start, 3.0, chan_color);
                    painter.circle_filled(pos, 3.0, chan_color);
                }
            }
            let ch_len = 8.0;
            painter.line_segment([pos - egui::vec2(ch_len, 0.0), pos + egui::vec2(ch_len, 0.0)], egui::Stroke::new(1.0, chan_color));
            painter.line_segment([pos - egui::vec2(0.0, ch_len), pos + egui::vec2(0.0, ch_len)], egui::Stroke::new(1.0, chan_color));
            ui.ctx().set_cursor_icon(egui::CursorIcon::None);
        } else if chart.draw_tool == "barmarker" {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
        } else if chart.draw_tool == "pitchfork" {
            let fork_color = egui::Color32::from_rgb(126, 207, 207);
            let n_pts = chart.pending_pts.len();
            if n_pts == 0 {
                // First click hint
            } else if n_pts == 1 {
                let p0 = egui::pos2(bx(chart.pending_pts[0].0), py(chart.pending_pts[0].1));
                painter.line_segment([p0, pos], egui::Stroke::new(1.5, color_alpha(fork_color, 180)));
                painter.circle_filled(p0, 3.0, fork_color);
            } else if n_pts == 2 {
                let p0 = egui::pos2(bx(chart.pending_pts[0].0), py(chart.pending_pts[0].1));
                let p1 = egui::pos2(bx(chart.pending_pts[1].0), py(chart.pending_pts[1].1));
                painter.line_segment([p0, pos], egui::Stroke::new(1.5, color_alpha(fork_color, 180)));
                painter.circle_filled(p0, 3.0, fork_color);
                painter.circle_filled(p1, 3.0, fork_color);
                painter.line_segment([p1, pos], egui::Stroke::new(0.8, color_alpha(fork_color, 100)));
            }
            let ch_len = 8.0;
            painter.line_segment([pos - egui::vec2(ch_len, 0.0), pos + egui::vec2(ch_len, 0.0)], egui::Stroke::new(1.0, fork_color));
            painter.line_segment([pos - egui::vec2(0.0, ch_len), pos + egui::vec2(0.0, ch_len)], egui::Stroke::new(1.0, fork_color));
            ui.ctx().set_cursor_icon(egui::CursorIcon::None);
        } else if chart.draw_tool == "gannfan" {
            let fan_color = egui::Color32::from_rgb(232, 201, 107);
            if let Some((b0, p0)) = chart.pending_pt {
                let origin = egui::pos2(bx(b0), py(p0));
                let chart_right = rect.left() + cw;
                let ref_dx = pos.x - origin.x; let ref_dy = pos.y - origin.y;
                if ref_dx.abs() > 1.0 {
                    let fans: &[(f32, u8)] = &[(8.0,50),(4.0,60),(3.0,70),(2.0,90),(1.0,200),(0.5,90),(1.0/3.0,70),(0.25,60),(0.125,50)];
                    for &(ratio, alpha) in fans {
                        let slope = ref_dy / ref_dx * ratio;
                        let end = egui::pos2(chart_right, origin.y + slope * (chart_right - origin.x));
                        painter.line_segment([origin, clamp_pt(end)], egui::Stroke::new(if (ratio-1.0).abs()<0.01 {1.5} else {0.8}, color_alpha(fan_color, alpha)));
                    }
                }
                painter.circle_filled(origin, 3.0, fan_color);
            }
            let ch_len = 8.0;
            painter.line_segment([pos - egui::vec2(ch_len, 0.0), pos + egui::vec2(ch_len, 0.0)], egui::Stroke::new(1.0, fan_color));
            painter.line_segment([pos - egui::vec2(0.0, ch_len), pos + egui::vec2(0.0, ch_len)], egui::Stroke::new(1.0, fan_color));
            ui.ctx().set_cursor_icon(egui::CursorIcon::None);
        } else if chart.draw_tool == "regression" {
            let reg_color = egui::Color32::from_rgb(180, 128, 232);
            if let Some((b0, _)) = chart.pending_pt {
                let x0 = bx(b0); let x1 = pos.x;
                painter.line_segment([egui::pos2(x0, rect.top()+pt), egui::pos2(x0, rect.top()+pt+ch)],
                    egui::Stroke::new(1.0, color_alpha(reg_color, 120)));
                painter.line_segment([egui::pos2(x1, rect.top()+pt), egui::pos2(x1, rect.top()+pt+ch)],
                    egui::Stroke::new(1.0, color_alpha(reg_color, 120)));
                painter.rect_filled(egui::Rect::from_x_y_ranges(x0.min(x1)..=x0.max(x1), (rect.top()+pt)..=(rect.top()+pt+ch)),
                    0.0, egui::Color32::from_rgba_unmultiplied(180, 128, 232, 15));
            }
            let ch_len = 8.0;
            painter.line_segment([pos - egui::vec2(ch_len, 0.0), pos + egui::vec2(ch_len, 0.0)], egui::Stroke::new(1.0, reg_color));
            painter.line_segment([pos - egui::vec2(0.0, ch_len), pos + egui::vec2(0.0, ch_len)], egui::Stroke::new(1.0, reg_color));
            ui.ctx().set_cursor_icon(egui::CursorIcon::None);
        } else if chart.draw_tool == "xabcd" {
            let xabcd_color = egui::Color32::from_rgb(255, 159, 67);
            let labels = ["X","A","B","C","D"];
            for i in 0..chart.pending_pts.len().saturating_sub(1) {
                let pa = egui::pos2(bx(chart.pending_pts[i].0), py(chart.pending_pts[i].1));
                let pb = egui::pos2(bx(chart.pending_pts[i+1].0), py(chart.pending_pts[i+1].1));
                painter.line_segment([pa, pb], egui::Stroke::new(1.5, color_alpha(xabcd_color, 200)));
            }
            for (i, &(b, p)) in chart.pending_pts.iter().enumerate() {
                let pt = egui::pos2(bx(b), py(p));
                painter.circle_filled(pt, 4.0, xabcd_color);
                painter.text(pt + egui::vec2(0.0, -10.0), egui::Align2::CENTER_CENTER,
                    labels.get(i).copied().unwrap_or("?"), egui::FontId::monospace(9.0), xabcd_color);
            }
            if !chart.pending_pts.is_empty() {
                let last = chart.pending_pts.last().unwrap();
                let lp = egui::pos2(bx(last.0), py(last.1));
                painter.line_segment([lp, pos], egui::Stroke::new(1.5, color_alpha(xabcd_color, 160)));
                let next_label = labels.get(chart.pending_pts.len()).copied().unwrap_or("?");
                painter.text(pos + egui::vec2(0.0, -10.0), egui::Align2::CENTER_CENTER,
                    next_label, egui::FontId::monospace(9.0), xabcd_color);
            }
            let ch_len = 8.0;
            painter.line_segment([pos - egui::vec2(ch_len, 0.0), pos + egui::vec2(ch_len, 0.0)], egui::Stroke::new(1.0, xabcd_color));
            painter.line_segment([pos - egui::vec2(0.0, ch_len), pos + egui::vec2(0.0, ch_len)], egui::Stroke::new(1.0, xabcd_color));
            ui.ctx().set_cursor_icon(egui::CursorIcon::None);
        } else if chart.draw_tool == "vline" {
            let vl_color = egui::Color32::from_rgb(100, 160, 255);
            let x = pos.x;
            painter.line_segment([egui::pos2(x, rect.top()+pt), egui::pos2(x, rect.top()+pt+ch)],
                egui::Stroke::new(1.0, color_alpha(vl_color, 160)));
            let ch_len = 8.0;
            painter.line_segment([pos - egui::vec2(ch_len, 0.0), pos + egui::vec2(ch_len, 0.0)], egui::Stroke::new(1.0, vl_color));
            painter.line_segment([pos - egui::vec2(0.0, ch_len), pos + egui::vec2(0.0, ch_len)], egui::Stroke::new(1.0, vl_color));
            ui.ctx().set_cursor_icon(egui::CursorIcon::None);
        } else if chart.draw_tool == "ray" {
            let ray_color = egui::Color32::from_rgb(100, 200, 255);
            if let Some((b0, p0)) = chart.pending_pt {
                let start = egui::pos2(bx(b0), py(p0));
                let chart_right = rect.left() + cw;
                painter.line_segment([start, pos], egui::Stroke::new(1.5, color_alpha(ray_color, 200)));
                // Extended preview
                let dx = pos.x - start.x;
                if dx.abs() > 0.001 {
                    let slope = (pos.y - start.y) / dx;
                    let ext_y = pos.y + slope * (chart_right - pos.x);
                    painter.line_segment([pos, clamp_pt(egui::pos2(chart_right, ext_y))], egui::Stroke::new(1.0, color_alpha(ray_color, 120)));
                }
                painter.circle_filled(start, 3.0, ray_color);
                painter.circle_filled(pos, 3.0, ray_color);
            }
            let ch_len = 8.0;
            painter.line_segment([pos - egui::vec2(ch_len, 0.0), pos + egui::vec2(ch_len, 0.0)], egui::Stroke::new(1.0, ray_color));
            painter.line_segment([pos - egui::vec2(0.0, ch_len), pos + egui::vec2(0.0, ch_len)], egui::Stroke::new(1.0, ray_color));
            ui.ctx().set_cursor_icon(egui::CursorIcon::None);
        } else if chart.draw_tool == "fibext" {
            let fibext_color = egui::Color32::from_rgb(255, 215, 0);
            let n_pts = chart.pending_pts.len();
            let labels = ["A","B","C"];
            for i in 0..n_pts.saturating_sub(1) {
                let pa = egui::pos2(bx(chart.pending_pts[i].0), py(chart.pending_pts[i].1));
                let pb = egui::pos2(bx(chart.pending_pts[i+1].0), py(chart.pending_pts[i+1].1));
                painter.line_segment([pa, pb], egui::Stroke::new(1.5, color_alpha(fibext_color, 180)));
            }
            for (i, &(b, p)) in chart.pending_pts.iter().enumerate() {
                let pt = egui::pos2(bx(b), py(p));
                painter.circle_filled(pt, 4.0, fibext_color);
                painter.text(pt + egui::vec2(0.0, -10.0), egui::Align2::CENTER_CENTER, labels.get(i).copied().unwrap_or("?"), egui::FontId::monospace(9.0), fibext_color);
            }
            if n_pts == 2 {
                // Show extension levels preview
                let (b0, p0) = chart.pending_pts[0]; let (_, p1) = chart.pending_pts[1];
                let ab_range = p1 - p0;
                let cursor_price = min_p + (max_p - min_p) * (1.0 - (pos.y - rect.top() - pt) / ch);
                let dir: f32 = if ab_range >= 0.0 { 1.0 } else { -1.0 };
                let chart_right = rect.left() + cw;
                let cx = bx(chart.pending_pts[1].0.max(pos.x as f32));
                for &(ratio, label) in &[(0.0_f32,"0%"),(0.618,"61.8%"),(1.0,"100%"),(1.618,"161.8%"),(2.618,"261.8%")] {
                    let lp = cursor_price + dir * ratio * ab_range.abs();
                    let y = py(lp);
                    if y.is_finite() && y.abs() < 50000.0 {
                        painter.line_segment([egui::pos2(pos.x, y), egui::pos2(chart_right, y)], egui::Stroke::new(0.8, color_alpha(fibext_color, 140)));
                        painter.text(egui::pos2(chart_right + 2.0, y), egui::Align2::LEFT_CENTER, &format!("{} {:.2}", label, lp), egui::FontId::monospace(7.5), color_alpha(fibext_color, 180));
                    }
                }
                let _ = (b0, cx);
            }
            if !chart.pending_pts.is_empty() {
                let last = chart.pending_pts.last().unwrap();
                let lp = egui::pos2(bx(last.0), py(last.1));
                painter.line_segment([lp, pos], egui::Stroke::new(1.5, color_alpha(fibext_color, 140)));
            }
            let ch_len = 8.0;
            painter.line_segment([pos - egui::vec2(ch_len, 0.0), pos + egui::vec2(ch_len, 0.0)], egui::Stroke::new(1.0, fibext_color));
            painter.line_segment([pos - egui::vec2(0.0, ch_len), pos + egui::vec2(0.0, ch_len)], egui::Stroke::new(1.0, fibext_color));
            ui.ctx().set_cursor_icon(egui::CursorIcon::None);
        } else if chart.draw_tool == "fibtimezone" {
            let ftz_color = egui::Color32::from_rgb(255, 193, 37);
            let anchor_bar = (pos.x - rect.left() + off - bs*0.5) / bs + vs;
            let fib_nums: &[u32] = &[1, 1, 2, 3, 5, 8, 13, 21, 34, 55, 89];
            let chart_right = rect.left() + cw;
            let mut seen: std::collections::HashSet<u32> = std::collections::HashSet::new();
            for (idx, &fib) in fib_nums.iter().enumerate() {
                if seen.contains(&fib) { continue; } seen.insert(fib);
                let x = bx(anchor_bar + fib as f32);
                if x < rect.left() || x > chart_right { continue; }
                let alpha = (180_u8).saturating_sub((idx as u8) * 16);
                painter.line_segment([egui::pos2(x, rect.top()+pt), egui::pos2(x, rect.top()+pt+ch)],
                    egui::Stroke::new(0.8, color_alpha(ftz_color, alpha)));
            }
            painter.line_segment([pos, egui::pos2(pos.x, rect.top()+pt+ch)], egui::Stroke::new(1.5, color_alpha(ftz_color, 200)));
            let ch_len = 8.0;
            painter.line_segment([pos - egui::vec2(ch_len, 0.0), pos + egui::vec2(ch_len, 0.0)], egui::Stroke::new(1.0, ftz_color));
            painter.line_segment([pos - egui::vec2(0.0, ch_len), pos + egui::vec2(0.0, ch_len)], egui::Stroke::new(1.0, ftz_color));
            ui.ctx().set_cursor_icon(egui::CursorIcon::None);
        } else if chart.draw_tool == "fibarc" {
            let farc_color = egui::Color32::from_rgb(255, 193, 37);
            if let Some((b0, p0)) = chart.pending_pt {
                let center = egui::pos2(bx(b0), py(p0));
                let dist = center.distance(pos);
                for &ratio in &[0.236_f32, 0.382, 0.5, 0.618, 0.786, 1.0] {
                    let r = dist * ratio;
                    let alpha = if ratio >= 0.618 { 180u8 } else { 100 };
                    let n_seg = 30;
                    let mut arc_pts: Vec<egui::Pos2> = Vec::with_capacity(n_seg + 1);
                    for k in 0..=n_seg {
                        let angle = std::f32::consts::PI * (k as f32 / n_seg as f32) + std::f32::consts::FRAC_PI_2;
                        arc_pts.push(clamp_pt(egui::pos2(pos.x + r * angle.cos(), pos.y + r * angle.sin())));
                    }
                    if arc_pts.len() > 1 { painter.add(egui::Shape::line(arc_pts, egui::Stroke::new(0.8, color_alpha(farc_color, alpha)))); }
                }
                painter.circle_filled(center, 3.0, farc_color);
                painter.line_segment([center, pos], egui::Stroke::new(0.7, color_alpha(farc_color, 100)));
            }
            let ch_len = 8.0;
            painter.line_segment([pos - egui::vec2(ch_len, 0.0), pos + egui::vec2(ch_len, 0.0)], egui::Stroke::new(1.0, farc_color));
            painter.line_segment([pos - egui::vec2(0.0, ch_len), pos + egui::vec2(0.0, ch_len)], egui::Stroke::new(1.0, farc_color));
            ui.ctx().set_cursor_icon(egui::CursorIcon::None);
        } else if chart.draw_tool == "gannbox" {
            let gb_color = egui::Color32::from_rgb(232, 201, 107);
            if let Some((_b0, _p0)) = chart.pending_pt {
                let start = egui::pos2(bx(_b0), py(_p0));
                let xl = start.x.min(pos.x); let xr = start.x.max(pos.x);
                let yt = start.y.min(pos.y); let yb = start.y.max(pos.y);
                painter.rect_stroke(egui::Rect::from_min_max(egui::pos2(xl,yt), egui::pos2(xr,yb)),
                    0.0, egui::Stroke::new(1.5, color_alpha(gb_color, 200)), egui::StrokeKind::Outside);
                painter.rect_filled(egui::Rect::from_min_max(egui::pos2(xl,yt), egui::pos2(xr,yb)),
                    0.0, color_alpha(gb_color, 12));
                // Diagonals
                painter.line_segment([egui::pos2(xl,yt), egui::pos2(xr,yb)], egui::Stroke::new(0.8, color_alpha(gb_color, 120)));
                painter.line_segment([egui::pos2(xl,yb), egui::pos2(xr,yt)], egui::Stroke::new(0.8, color_alpha(gb_color, 120)));
                painter.circle_filled(start, 3.0, gb_color);
            }
            let ch_len = 8.0;
            painter.line_segment([pos - egui::vec2(ch_len, 0.0), pos + egui::vec2(ch_len, 0.0)], egui::Stroke::new(1.0, gb_color));
            painter.line_segment([pos - egui::vec2(0.0, ch_len), pos + egui::vec2(0.0, ch_len)], egui::Stroke::new(1.0, gb_color));
            ui.ctx().set_cursor_icon(egui::CursorIcon::None);
        } else if chart.draw_tool == "elliott_impulse" || chart.draw_tool == "elliott_corrective"
               || chart.draw_tool == "elliott_wxy" || chart.draw_tool == "elliott_wxyxz"
               || chart.draw_tool == "elliott_sub_impulse" || chart.draw_tool == "elliott_sub_corrective" {
            let wave_color = egui::Color32::from_rgb(78, 205, 196);
            let impulse_labels = ["1","2","3","4","5"];
            let corrective_labels = ["A","B","C"];
            let wxy_labels = ["W","X","Y"];
            let wxyxz_labels = ["W","X","Y","X","Z"];
            let sub_impulse_labels = ["i","ii","iii","iv","v"];
            let sub_corrective_labels = ["a","b","c"];
            let labels: &[&str] = match chart.draw_tool.as_str() {
                "elliott_impulse" => &impulse_labels,
                "elliott_wxy" => &wxy_labels,
                "elliott_wxyxz" => &wxyxz_labels,
                "elliott_sub_impulse" => &sub_impulse_labels,
                "elliott_sub_corrective" => &sub_corrective_labels,
                _ => &corrective_labels,
            };
            for i in 0..chart.pending_pts.len().saturating_sub(1) {
                let pa = egui::pos2(bx(chart.pending_pts[i].0), py(chart.pending_pts[i].1));
                let pb = egui::pos2(bx(chart.pending_pts[i+1].0), py(chart.pending_pts[i+1].1));
                painter.line_segment([pa, pb], egui::Stroke::new(1.5, color_alpha(wave_color, 200)));
            }
            for (i, &(b, p)) in chart.pending_pts.iter().enumerate() {
                let pt = egui::pos2(bx(b), py(p));
                painter.circle_filled(pt, 7.0, color_alpha(wave_color, 80));
                painter.circle_stroke(pt, 7.0, egui::Stroke::new(1.0, wave_color));
                painter.text(pt, egui::Align2::CENTER_CENTER, labels.get(i).copied().unwrap_or("?"), egui::FontId::monospace(7.5), egui::Color32::WHITE);
            }
            if !chart.pending_pts.is_empty() {
                let last = chart.pending_pts.last().unwrap();
                let lp = egui::pos2(bx(last.0), py(last.1));
                painter.line_segment([lp, pos], egui::Stroke::new(1.5, color_alpha(wave_color, 160)));
            }
            let ch_len = 8.0;
            painter.line_segment([pos - egui::vec2(ch_len, 0.0), pos + egui::vec2(ch_len, 0.0)], egui::Stroke::new(1.0, wave_color));
            painter.line_segment([pos - egui::vec2(0.0, ch_len), pos + egui::vec2(0.0, ch_len)], egui::Stroke::new(1.0, wave_color));
            ui.ctx().set_cursor_icon(egui::CursorIcon::None);
        } else if chart.draw_tool == "avwap" {
            let av_color = egui::Color32::from_rgb(180, 100, 255);
            painter.line_segment([egui::pos2(pos.x, rect.top()+pt), egui::pos2(pos.x, rect.top()+pt+ch)],
                egui::Stroke::new(1.0, color_alpha(av_color, 120)));
            let ch_len = 8.0;
            painter.line_segment([pos - egui::vec2(ch_len, 0.0), pos + egui::vec2(ch_len, 0.0)], egui::Stroke::new(1.0, av_color));
            painter.line_segment([pos - egui::vec2(0.0, ch_len), pos + egui::vec2(0.0, ch_len)], egui::Stroke::new(1.0, av_color));
            ui.ctx().set_cursor_icon(egui::CursorIcon::None);
        } else if chart.draw_tool == "pricerange" {
            let pr_color = egui::Color32::from_rgb(116, 185, 255);
            if let Some((_b0, p0)) = chart.pending_pt {
                let y0 = py(p0);
                painter.rect_filled(
                    egui::Rect::from_min_max(egui::pos2(chart.pending_pt.map(|_| pos.x).unwrap_or(pos.x), y0.min(pos.y)),
                                             egui::pos2(pos.x.max(chart.pending_pt.map(|_| pos.x).unwrap_or(pos.x)), y0.max(pos.y))),
                    0.0, egui::Color32::from_rgba_unmultiplied(116, 185, 255, 20));
                painter.line_segment([egui::pos2(rect.left(), y0), egui::pos2(rect.left()+cw, y0)],
                    egui::Stroke::new(1.0, color_alpha(pr_color, 160)));
                painter.line_segment([egui::pos2(rect.left(), pos.y), egui::pos2(rect.left()+cw, pos.y)],
                    egui::Stroke::new(1.0, color_alpha(pr_color, 160)));
            }
            let ch_len = 8.0;
            painter.line_segment([pos - egui::vec2(ch_len, 0.0), pos + egui::vec2(ch_len, 0.0)], egui::Stroke::new(1.0, pr_color));
            painter.line_segment([pos - egui::vec2(0.0, ch_len), pos + egui::vec2(0.0, ch_len)], egui::Stroke::new(1.0, pr_color));
            ui.ctx().set_cursor_icon(egui::CursorIcon::None);
        } else if chart.draw_tool == "riskreward" {
            let rr_color = egui::Color32::from_rgb(46, 204, 113);
            let n_pts = chart.pending_pts.len();
            if n_pts == 0 {
                // waiting for entry click
            } else if n_pts == 1 {
                // entry placed, waiting for stop
                let entry_y = py(chart.pending_pts[0].1);
                let chart_right = rect.left() + cw;
                let ex = bx(chart.pending_pts[0].0);
                painter.line_segment([egui::pos2(ex, entry_y), egui::pos2(chart_right, entry_y)],
                    egui::Stroke::new(1.5, color_alpha(rr_color, 200)));
                painter.line_segment([egui::pos2(ex, pos.y), egui::pos2(chart_right, pos.y)],
                    egui::Stroke::new(1.0, color_alpha(egui::Color32::from_rgb(231,76,60), 160)));
                let risk = (py(chart.pending_pts[0].1) - pos.y).abs();
                let stop_side = pos.y.min(entry_y);
                painter.rect_filled(egui::Rect::from_min_max(egui::pos2(ex, stop_side), egui::pos2(chart_right, stop_side + risk)),
                    0.0, egui::Color32::from_rgba_unmultiplied(231, 76, 60, 20));
            } else if n_pts == 2 {
                // entry + stop placed, waiting for target
                let chart_right = rect.left() + cw;
                let ex = bx(chart.pending_pts[0].0);
                let entry_y = py(chart.pending_pts[0].1);
                let stop_y  = py(chart.pending_pts[1].1);
                let risk_h  = (entry_y - stop_y).abs();
                painter.rect_filled(egui::Rect::from_min_max(egui::pos2(ex, entry_y.min(stop_y)), egui::pos2(chart_right, entry_y.max(stop_y))),
                    0.0, egui::Color32::from_rgba_unmultiplied(231, 76, 60, 25));
                painter.rect_filled(egui::Rect::from_min_max(egui::pos2(ex, entry_y.min(pos.y)), egui::pos2(chart_right, entry_y.max(pos.y))),
                    0.0, egui::Color32::from_rgba_unmultiplied(46, 204, 113, 25));
                painter.line_segment([egui::pos2(ex, entry_y), egui::pos2(chart_right, entry_y)], egui::Stroke::new(1.5, color_alpha(rr_color, 200)));
                painter.line_segment([egui::pos2(ex, stop_y), egui::pos2(chart_right, stop_y)], egui::Stroke::new(1.0, color_alpha(egui::Color32::from_rgb(231,76,60), 160)));
                painter.line_segment([egui::pos2(ex, pos.y), egui::pos2(chart_right, pos.y)], egui::Stroke::new(1.0, color_alpha(rr_color, 160)));
                if risk_h > 0.0 {
                    let reward_h = (entry_y - pos.y).abs();
                    let rr = reward_h / risk_h;
                    painter.text(egui::pos2(chart_right - 4.0, entry_y.min(pos.y) + reward_h * 0.5),
                        egui::Align2::RIGHT_CENTER, &format!("{:.2}:1", rr), egui::FontId::monospace(9.0), rr_color);
                }
            }
            let ch_len = 8.0;
            painter.line_segment([pos - egui::vec2(ch_len, 0.0), pos + egui::vec2(ch_len, 0.0)], egui::Stroke::new(1.0, rr_color));
            painter.line_segment([pos - egui::vec2(0.0, ch_len), pos + egui::vec2(0.0, ch_len)], egui::Stroke::new(1.0, rr_color));
            ui.ctx().set_cursor_icon(egui::CursorIcon::None);
        }
    } } // end pointer_in_pane + hover_pos

    // Crosshair (only when not in drawing mode, only in hovered pane)
    if pointer_in_pane && chart.draw_tool.is_empty() {
        if let Some(pos) = ui.input(|i| i.pointer.hover_pos()) {
            if pos.x >= rect.left() && pos.x < rect.left()+cw && pos.y >= rect.top()+pt && pos.y < rect.top()+pt+ch {
                let ch_line_w = style::stroke_thin();
                painter.line_segment([egui::pos2(rect.left(),pos.y),egui::pos2(rect.left()+cw,pos.y)],egui::Stroke::new(ch_line_w,color_alpha(t.text,50)));
                painter.line_segment([egui::pos2(pos.x,rect.top()+pt),egui::pos2(pos.x,rect.top()+pt+ch)],egui::Stroke::new(ch_line_w,color_alpha(t.text,50)));
                let hp = min_p+(max_p-min_p)*(1.0-(pos.y-rect.top()-pt)/ch);
                chart.fmt_buf.clear(); let _ = write!(chart.fmt_buf, "{:.2}", hp);
                let ch_st = style::current();
                let ch_badge_cr = ch_st.r_xs as f32;
                let ch_badge_stroke_w = if ch_st.hairline_borders { ch_st.stroke_std } else { style::stroke_thin() };
                let cf = egui::FontId::monospace(style::font_lg());
                let cg = painter.layout_no_wrap(chart.fmt_buf.clone(), cf.clone(), egui::Color32::WHITE);
                let cpad_x = 5.0; let cpad_y = 2.0;
                let cbw = cg.size().x + cpad_x * 2.0;
                let cbh = cg.size().y + cpad_y * 2.0;
                let cbr = egui::Rect::from_min_size(
                    egui::pos2(rect.left() + cw + 1.0, pos.y - cbh / 2.0),
                    egui::vec2(cbw, cbh));
                painter.rect_filled(cbr, ch_badge_cr, egui::Color32::from_rgba_unmultiplied(20, 20, 26, 240));
                painter.rect_stroke(cbr, ch_badge_cr, egui::Stroke::new(ch_badge_stroke_w, color_alpha(t.text, 80)), egui::StrokeKind::Inside);
                // Bolder via 0.5px double-draw
                painter.text(egui::pos2(cbr.left() + cpad_x + 0.5, pos.y), egui::Align2::LEFT_CENTER, &chart.fmt_buf, cf.clone(), egui::Color32::WHITE);
                painter.text(egui::pos2(cbr.left() + cpad_x, pos.y), egui::Align2::LEFT_CENTER, &chart.fmt_buf, cf, egui::Color32::WHITE);

                // Time label at crosshair X position (bottom of chart)
                let bar_idx_f = (pos.x - rect.left() + off - bs * 0.5) / bs + vs;
                let bar_idx = bar_idx_f.round() as usize;
                if let Some(&ts) = chart.timestamps.get(bar_idx) {
                    CROSSHAIR_SYNC_TIME.with(|c| c.set(ts));
                    let dt = chrono::DateTime::from_timestamp(ts, 0);
                    if let Some(dt) = dt {
                        let dt = dt.naive_utc();
                        let time_str = match chart.timeframe.as_str() {
                            "1m" | "5m" | "15m" | "30m" | "1h" | "2h" | "4h" => dt.format("%b %d %H:%M").to_string(),
                            "1d" => dt.format("%b %d %Y").to_string(),
                            "1w" | "1M" => dt.format("%b %Y").to_string(),
                            _ => dt.format("%Y-%m-%d %H:%M").to_string(),
                        };
                        let tf_font = egui::FontId::monospace(style::font_lg());
                        let tg = painter.layout_no_wrap(time_str.clone(), tf_font.clone(), egui::Color32::WHITE);
                        let tpad_x = 5.0; let tpad_y = 2.0;
                        let tbw = tg.size().x + tpad_x * 2.0;
                        let tbh = tg.size().y + tpad_y * 2.0;
                        // Position badge in the time-axis gutter just below the chart canvas,
                        // clamped so it never extends past the pane bottom.
                        let pane_bottom = rect.bottom();
                        let mut time_y_top = rect.top() + pt + ch + 2.0;
                        if time_y_top + tbh > pane_bottom { time_y_top = pane_bottom - tbh; }
                        let mut time_x_left = pos.x - tbw / 2.0;
                        if time_x_left < rect.left() { time_x_left = rect.left(); }
                        if time_x_left + tbw > rect.left() + cw { time_x_left = rect.left() + cw - tbw; }
                        let tbr = egui::Rect::from_min_size(
                            egui::pos2(time_x_left, time_y_top),
                            egui::vec2(tbw, tbh));
                        painter.rect_filled(tbr, ch_badge_cr, egui::Color32::from_rgba_unmultiplied(20, 20, 26, 240));
                        painter.rect_stroke(tbr, ch_badge_cr, egui::Stroke::new(ch_badge_stroke_w, color_alpha(t.text, 80)), egui::StrokeKind::Inside);
                        // Bolder via 0.5px double-draw
                        painter.text(egui::pos2(tbr.center().x + 0.5, tbr.center().y), egui::Align2::CENTER_CENTER, &time_str, tf_font.clone(), egui::Color32::WHITE);
                        painter.text(tbr.center(), egui::Align2::CENTER_CENTER, &time_str, tf_font, egui::Color32::WHITE);
                    }
                }
                // Measure tooltip — big clean distance display
                if chart.measure_tooltip && last_price > 0.0 {
                    let dist = hp - last_price;
                    let dist_pct = dist / last_price * 100.0;
                    let dist_col = if dist >= 0.0 { t.bull } else { t.bear };
                    let m_price_text = format!("{:+.2}", dist);
                    let m_pct_text = format!("{:+.2}%", dist_pct);
                    // Big floating box near cursor
                    let mx = pos.x + 20.0;
                    let my = pos.y;
                    let meas_st = style::current();
                    let meas_cr = meas_st.r_sm as f32;
                    let meas_stroke_w = if meas_st.hairline_borders { meas_st.stroke_std } else { style::stroke_thin() };
                    let pct_galley = painter.layout_no_wrap(m_pct_text.clone(), egui::FontId::monospace(style::font_md()), dist_col);
                    let price_galley = painter.layout_no_wrap(m_price_text.clone(), egui::FontId::monospace(style::font_sm()), color_alpha(dist_col, 160));
                    let box_w = pct_galley.size().x.max(price_galley.size().x) + 16.0;
                    let box_h = pct_galley.size().y + price_galley.size().y + 10.0;
                    // Flip left if near right edge
                    let bx_pos = if mx + box_w > rect.left() + cw { pos.x - box_w - 10.0 } else { mx };
                    let measure_rect = egui::Rect::from_min_size(egui::pos2(bx_pos, my - box_h / 2.0), egui::vec2(box_w, box_h));
                    painter.rect_filled(measure_rect, meas_cr, color_alpha(t.toolbar_bg, 240));
                    painter.rect_stroke(measure_rect, meas_cr, egui::Stroke::new(meas_stroke_w, color_alpha(dist_col, 80)), egui::StrokeKind::Outside);
                    painter.text(egui::pos2(measure_rect.center().x, measure_rect.top() + pct_galley.size().y / 2.0 + 4.0),
                        egui::Align2::CENTER_CENTER, &m_pct_text, egui::FontId::monospace(style::font_md()), dist_col);
                    painter.text(egui::pos2(measure_rect.center().x, measure_rect.bottom() - price_galley.size().y / 2.0 - 3.0),
                        egui::Align2::CENTER_CENTER, &m_price_text, egui::FontId::monospace(style::font_sm()), color_alpha(dist_col, 160));
                }

                // OHLC tooltip (togglable — hidden when footprint is active)
                if chart.ohlc_tooltip && !chart.show_footprint && !chart.draw_picker_open {
                    if let Some(bar_data) = chart.bars.get(bar_idx) {
                        let tooltip_x = pos.x + 15.0;
                        let tooltip_y = pos.y - 5.0;
                        let font = egui::FontId::monospace(style::font_sm());
                        let o = bar_data.open; let h = bar_data.high; let l = bar_data.low; let c = bar_data.close; let v = bar_data.volume;
                        let is_bull = c >= o;
                        let change = c - o;
                        let pct = if o != 0.0 { change / o * 100.0 } else { 0.0 };
                        let chg_col = if is_bull { t.bull } else { t.bear };

                        // Format volume compactly
                        let vol_str = if v >= 1_000_000.0 { format!("Vol {:.1}M", v / 1_000_000.0) }
                            else if v >= 1_000.0 { format!("Vol {:.1}K", v / 1_000.0) }
                            else { format!("Vol {:.0}", v) };

                        // Build lines: compact OHLC + volume + change
                        let mut tip_lines: Vec<(String, egui::Color32)> = vec![
                            (format!("O {:.2}  H {:.2}", o, h), color_alpha(t.text,180)),
                            (format!("L {:.2}  C {:.2}", l, c), color_alpha(t.text,180)),
                            (format!("{:+.2} ({:+.1}%)", change, pct), chg_col),
                            (vol_str, color_alpha(t.text,140)),
                        ];

                        // Volume X-Ray — buy/sell split, delta, RVOL
                        if v > 0.0 {
                            // Estimate buy/sell from close position within bar
                            let range = (h - l).max(0.001);
                            let buy_ratio = (c - l) / range; // close near high = more buying
                            let buy_vol = v * buy_ratio;
                            let sell_vol = v * (1.0 - buy_ratio);
                            let delta = buy_vol - sell_vol;

                            // RVOL
                            let avg_vol = if bar_idx > 20 {
                                chart.bars[bar_idx.saturating_sub(21)..bar_idx].iter()
                                    .map(|b| b.volume).sum::<f32>() / 20.0
                            } else { v };
                            let rvol = if avg_vol > 0.0 { v / avg_vol } else { 1.0 };

                            tip_lines.push(("---".into(), color_alpha(t.text, 40)));

                            // Buy/Sell bar visual as text
                            let buy_pct = (buy_ratio * 100.0) as u32;
                            let buy_str = if buy_vol >= 1_000_000.0 { format!("{:.1}M", buy_vol / 1_000_000.0) }
                                else { format!("{:.0}K", buy_vol / 1_000.0) };
                            let sell_str = if sell_vol >= 1_000_000.0 { format!("{:.1}M", sell_vol / 1_000_000.0) }
                                else { format!("{:.0}K", sell_vol / 1_000.0) };
                            tip_lines.push((format!("Buy {}  Sell {}", buy_str, sell_str),
                                if buy_ratio > 0.5 { t.bull } else { t.bear }));
                            let delta_str = if delta.abs() >= 1_000_000.0 { format!("{:+.1}M", delta / 1_000_000.0) }
                                else { format!("{:+.0}K", delta / 1_000.0) };
                            tip_lines.push((format!("Delta {}  B/S {:.0}%", delta_str, buy_pct),
                                if delta > 0.0 { t.bull } else { t.bear }));
                            let rvol_col = if rvol > 2.0 { t.accent } else if rvol > 1.2 { t.bull } else { t.dim };
                            tip_lines.push((format!("RVOL {:.1}x", rvol), rvol_col));
                        }

                        // Detect hovered indicator (within 5px of cursor Y)
                        let mut hovered_ind_id: Option<u32> = None;
                        for ind in &chart.indicators {
                            if !ind.visible { continue; }
                            if ind.kind.category() == IndicatorCategory::Overlay {
                                if let Some(&v) = ind.values.get(bar_idx) {
                                    if !v.is_nan() && (pos.y - py(v)).abs() < 5.0 {
                                        hovered_ind_id = Some(ind.id);
                                        break;
                                    }
                                }
                            }
                        }

                        // Add indicator values at this bar
                        let mut has_ind = false;
                        for ind in &chart.indicators {
                            if !ind.visible { continue; }
                            let label = ind.kind.label();
                            let period = ind.period;
                            let ind_color = hex_to_color(&ind.color, 1.0);
                            let is_hovered = hovered_ind_id == Some(ind.id);
                            let alpha = if is_hovered { 255u8 } else { 160 };
                            let col = egui::Color32::from_rgba_unmultiplied(ind_color.r(), ind_color.g(), ind_color.b(), alpha);
                            match ind.kind {
                                IndicatorType::MACD => {
                                    if let (Some(&mv), Some(&sv)) = (ind.values.get(bar_idx), ind.values2.get(bar_idx)) {
                                        if !mv.is_nan() && !sv.is_nan() {
                                            if !has_ind { tip_lines.push(("---".into(), color_alpha(t.text,40))); has_ind = true; }
                                            let prefix = if is_hovered { "\u{25B6} " } else { "" };
                                            tip_lines.push((format!("{}MACD {:.2}  S {:.2}", prefix, mv, sv), col));
                                        }
                                    }
                                }
                                IndicatorType::RSI | IndicatorType::Stochastic | IndicatorType::ADX
                                | IndicatorType::CCI | IndicatorType::WilliamsR | IndicatorType::ATR => {
                                    if let Some(&v) = ind.values.get(bar_idx) {
                                        if !v.is_nan() {
                                            if !has_ind { tip_lines.push(("---".into(), color_alpha(t.text,40))); has_ind = true; }
                                            let prefix = if is_hovered { "\u{25B6} " } else { "" };
                                            tip_lines.push((format!("{}{} {:.1}", prefix, label, v), col));
                                        }
                                    }
                                }
                                _ => {
                                    // Overlay indicators (SMA, EMA, BB, etc.)
                                    if let Some(&v) = ind.values.get(bar_idx) {
                                        if !v.is_nan() {
                                            if !has_ind { tip_lines.push(("---".into(), color_alpha(t.text,40))); has_ind = true; }
                                            let prefix = if is_hovered { "\u{25B6} " } else { "" };
                                            tip_lines.push((format!("{}{}{} {:.2}", prefix, label, period, v), col));
                                        }
                                    }
                                }
                            }
                        }

                        let line_h = 12.0;
                        let tip_h = tip_lines.len() as f32 * line_h + 8.0;
                        let tip_w = 170.0;
                        let tx = if tooltip_x + tip_w > rect.left() + cw { pos.x - tip_w - 16.0 } else { tooltip_x };
                        let ty = (tooltip_y - tip_h).max(rect.top() + pt).min(rect.top() + pt + ch - tip_h);
                        let tip_rect = egui::Rect::from_min_size(egui::pos2(tx, ty), egui::vec2(tip_w, tip_h));
                        // Rich tooltip: shadow + bevel + crisp border (style-aware)
                        let tip_st = style::current();
                        let tip_cr = tip_st.r_md as f32;
                        let tip_cr_egui = egui::CornerRadius::same(tip_st.r_md);
                        let tip_stroke_w = if tip_st.hairline_borders { tip_st.stroke_std } else { style::stroke_thin() };
                        let tip_border_alpha = if t.is_light() { 50u8 } else { 40u8 };
                        if tip_st.shadows_enabled {
                            painter.rect_filled(tip_rect.translate(egui::vec2(0.0, style::shadow_offset())).expand(1.0), tip_cr,
                                egui::Color32::from_rgba_unmultiplied(0, 0, 0, style::shadow_alpha()));
                        }
                        painter.rect_filled(tip_rect, tip_cr,
                            egui::Color32::from_rgba_unmultiplied(t.toolbar_bg.r(), t.toolbar_bg.g(), t.toolbar_bg.b(), 240));
                        // Top bevel (only for non-Meridien styles that have rounded corners)
                        if tip_st.r_md > 0 {
                            painter.rect_filled(egui::Rect::from_min_max(tip_rect.min, egui::pos2(tip_rect.right(), tip_rect.top() + 1.0)),
                                egui::CornerRadius { nw: tip_st.r_md, ne: tip_st.r_md, sw: 0, se: 0 },
                                egui::Color32::from_rgba_unmultiplied(255, 255, 255, if t.is_light() { 30 } else { 8 }));
                        }
                        painter.rect_stroke(tip_rect, tip_cr_egui,
                            egui::Stroke::new(tip_stroke_w, color_alpha(t.toolbar_border, tip_border_alpha)),
                            egui::StrokeKind::Outside);
                        for (i, (line, col)) in tip_lines.iter().enumerate() {
                            if line == "---" {
                                let sep_y = ty + 4.0 + i as f32 * line_h + line_h / 2.0;
                                painter.line_segment([egui::pos2(tx + 4.0, sep_y), egui::pos2(tx + tip_w - 4.0, sep_y)],
                                    egui::Stroke::new(0.5, color_alpha(t.text,30)));
                            } else {
                                painter.text(egui::pos2(tx + 6.0, ty + 4.0 + i as f32 * line_h + line_h / 2.0), egui::Align2::LEFT_CENTER, line, font.clone(), *col);
                            }
                        }
                    }
                }
                // ── Exploded Volume Footprint ─────────────────────────────────
                // Infographic-style: callout lines radiate from the candle to
                // large readable cards. Alternates left/right. Highlights POC,
                // heaviest buy/sell, and absorption levels.
                if chart.show_footprint {
                    if let Some(bar_data) = chart.bars.get(bar_idx) {
                        let bar_x = bx(bar_idx as f32);
                        let bar_range = bar_data.high - bar_data.low;
                        if bar_range > 0.0 && bar_data.volume > 0.0 {
                            let num_levels = 8;
                            let micro = bar_micro_profile(bar_data, num_levels);
                            let is_bull = bar_data.close >= bar_data.open;
                            let total_weight: f32 = micro.iter().map(|m| m.1).sum();
                            let bar_top_y = py(bar_data.high);
                            let bar_bot_y = py(bar_data.low);

                            // ── Compute all insights before rendering ──
                            let total_weight: f32 = micro.iter().map(|m| m.1).sum();
                            struct FpLevel { price: f32, vol: f32, buy: f32, sell: f32, delta: f32, buy_ratio: f32, imbalance: f32 }
                            let fp_levels: Vec<FpLevel> = micro.iter().map(|(price, wf, br)| {
                                let vol = bar_data.volume * wf / total_weight.max(0.001);
                                let buy = vol * br; let sell = vol * (1.0 - br);
                                let bigger = buy.max(sell); let smaller = buy.min(sell);
                                let imbalance = if smaller > 0.0 { bigger / smaller } else { 10.0 };
                                FpLevel { price: *price, vol, buy, sell, delta: buy - sell, buy_ratio: *br, imbalance }
                            }).collect();

                            let total_delta: f32 = fp_levels.iter().map(|l| l.delta).sum();
                            let total_buy: f32 = fp_levels.iter().map(|l| l.buy).sum();
                            let total_sell: f32 = fp_levels.iter().map(|l| l.sell).sum();
                            let buy_pct = total_buy / (total_buy + total_sell).max(0.001) * 100.0;
                            let conviction = (total_delta.abs() / bar_data.volume.max(0.001) * 100.0).min(100.0);
                            let poc_idx = fp_levels.iter().enumerate().max_by(|a, b| a.1.vol.partial_cmp(&b.1.vol).unwrap_or(std::cmp::Ordering::Equal)).map(|(i,_)| i).unwrap_or(0);
                            let max_buy_idx = fp_levels.iter().enumerate().max_by(|a, b| a.1.delta.partial_cmp(&b.1.delta).unwrap_or(std::cmp::Ordering::Equal)).map(|(i,_)| i).unwrap_or(0);
                            let max_sell_idx = fp_levels.iter().enumerate().min_by(|a, b| a.1.delta.partial_cmp(&b.1.delta).unwrap_or(std::cmp::Ordering::Equal)).map(|(i,_)| i).unwrap_or(0);

                            // Upper/lower half volume concentration
                            let half = fp_levels.len() / 2;
                            let upper_vol: f32 = fp_levels[half..].iter().map(|l| l.vol).sum();
                            let lower_vol: f32 = fp_levels[..half].iter().map(|l| l.vol).sum();
                            let upper_pct = upper_vol / (upper_vol + lower_vol).max(0.001) * 100.0;

                            // Wick analysis
                            let upper_wick = bar_data.high - bar_data.open.max(bar_data.close);
                            let lower_wick = bar_data.open.min(bar_data.close) - bar_data.low;
                            let body_size = (bar_data.close - bar_data.open).abs();
                            let wick_insight = if upper_wick > body_size * 1.5 && upper_wick > lower_wick * 2.0 {
                                Some(("REJECTION", "Long upper wick — sellers rejected higher prices", t.bear))
                            } else if lower_wick > body_size * 1.5 && lower_wick > upper_wick * 2.0 {
                                Some(("ABSORPTION", "Long lower wick — buyers absorbed selling", t.bull))
                            } else if upper_wick > body_size && lower_wick > body_size {
                                Some(("INDECISION", "Long wicks both sides — neither side in control", t.dim))
                            } else { None };

                            // Exhaustion: counter-trend volume at wick tips
                            let top_level = fp_levels.last();
                            let bot_level = fp_levels.first();
                            let exhaustion = if is_bull {
                                // Bull candle with heavy selling at the top = exhaustion
                                top_level.map_or(false, |l| l.sell > l.buy * 1.3 && l.vol > bar_data.volume / num_levels as f32)
                            } else {
                                // Bear candle with heavy buying at the bottom = exhaustion
                                bot_level.map_or(false, |l| l.buy > l.sell * 1.3 && l.vol > bar_data.volume / num_levels as f32)
                            };

                            // Trapped traders: high volume at wick extremes
                            let trapped = if is_bull {
                                bot_level.map_or(false, |l| l.sell > l.buy * 1.5 && l.vol > bar_data.volume / num_levels as f32 * 1.2)
                            } else {
                                top_level.map_or(false, |l| l.buy > l.sell * 1.5 && l.vol > bar_data.volume / num_levels as f32 * 1.2)
                            };

                            // RVOL for this bar
                            let rvol = if bar_idx < chart.rvol_data.len() { chart.rvol_data[bar_idx] } else { 1.0 };

                            // ── Header insights panel (across the top) ──
                            let header_h = 68.0;
                            let dim_w = 520.0;
                            let dim_x = (bar_x - dim_w / 2.0).max(rect.left());
                            let header_y = (bar_top_y - header_h - 16.0).max(rect.top() + pt + 2.0);
                            let dim_top = header_y;
                            let dim_bot = (bar_bot_y + 8.0).min(rect.top() + pt + ch);

                            // Dim background behind the entire infographic area
                            painter.rect_filled(egui::Rect::from_min_max(
                                egui::pos2(dim_x, dim_top), egui::pos2(dim_x + dim_w, dim_bot)),
                                0.0, egui::Color32::from_rgba_unmultiplied(t.toolbar_bg.r(), t.toolbar_bg.g(), t.toolbar_bg.b(), 190));

                            // Header card
                            let hdr_rect = egui::Rect::from_min_size(egui::pos2(dim_x + 4.0, header_y + 2.0), egui::vec2(dim_w - 8.0, header_h - 4.0));
                            painter.rect_filled(hdr_rect, 4.0, egui::Color32::from_rgba_unmultiplied(t.toolbar_bg.r(), t.toolbar_bg.g(), t.toolbar_bg.b(), 240));
                            let dir_col = if is_bull { t.bull } else { t.bear };
                            painter.rect_stroke(hdr_rect, 4.0, egui::Stroke::new(1.0, color_alpha(dir_col, 80)), egui::StrokeKind::Outside);

                            let hx = hdr_rect.left() + 10.0;
                            let hy = hdr_rect.top();
                            let hdr_font = egui::FontId::monospace(13.0);
                            let hdr_med = egui::FontId::monospace(9.0);
                            let hdr_sm = egui::FontId::monospace(10.0);

                            // Row 1: Direction + Delta + Buy/Sell split + Conviction + RVOL
                            let dir_label = if is_bull { "BULL" } else { "BEAR" };
                            painter.text(egui::pos2(hx, hy + 14.0), egui::Align2::LEFT_CENTER, dir_label, hdr_font.clone(), dir_col);
                            painter.text(egui::pos2(hx + 50.0, hy + 14.0), egui::Align2::LEFT_CENTER,
                                &format!("\u{0394} {:+.0}", total_delta), hdr_font.clone(), dir_col);
                            painter.text(egui::pos2(hx + 150.0, hy + 14.0), egui::Align2::LEFT_CENTER,
                                &format!("Buy {:.0}%  Sell {:.0}%", buy_pct, 100.0 - buy_pct), hdr_med.clone(), color_alpha(t.text,180));
                            // Conviction bar (visual)
                            let conv_x = hx + 320.0;
                            let conv_w = 80.0;
                            painter.rect_filled(egui::Rect::from_min_size(egui::pos2(conv_x, hy + 8.0), egui::vec2(conv_w, 12.0)),
                                3.0, color_alpha(t.text,15));
                            painter.rect_filled(egui::Rect::from_min_size(egui::pos2(conv_x, hy + 8.0), egui::vec2(conv_w * conviction / 100.0, 12.0)),
                                3.0, color_alpha(dir_col, if conviction > 60.0 { 150 } else { 60 }));
                            painter.text(egui::pos2(conv_x + conv_w + 6.0, hy + 14.0), egui::Align2::LEFT_CENTER,
                                &format!("{:.0}%", conviction), hdr_sm.clone(),
                                if conviction > 60.0 { dir_col } else { t.dim });
                            if rvol > 1.5 {
                                painter.text(egui::pos2(hdr_rect.right() - 10.0, hy + 14.0), egui::Align2::RIGHT_CENTER,
                                    &format!("{:.1}x vol", rvol), hdr_med.clone(),
                                    if rvol > 2.5 { COLOR_AMBER } else { color_alpha(t.text,160) });
                            }

                            // Row 2: Volume concentration (visual bar) + POC price
                            let conc_bar_x = hx;
                            let conc_bar_w = 120.0;
                            let conc_y = hy + 32.0;
                            let upper_w = conc_bar_w * upper_pct / 100.0;
                            painter.rect_filled(egui::Rect::from_min_size(egui::pos2(conc_bar_x, conc_y), egui::vec2(upper_w, 10.0)),
                                2.0, egui::Color32::from_rgba_unmultiplied(100, 180, 255, 80));
                            painter.rect_filled(egui::Rect::from_min_size(egui::pos2(conc_bar_x + upper_w, conc_y), egui::vec2(conc_bar_w - upper_w, 10.0)),
                                2.0, egui::Color32::from_rgba_unmultiplied(180, 130, 255, 80));
                            painter.text(egui::pos2(conc_bar_x + conc_bar_w + 6.0, conc_y + 5.0), egui::Align2::LEFT_CENTER,
                                &format!("Upper {:.0}%  Lower {:.0}%", upper_pct, 100.0 - upper_pct), hdr_sm.clone(), color_alpha(t.text,130));
                            painter.text(egui::pos2(hx + 320.0, conc_y + 5.0), egui::Align2::LEFT_CENTER,
                                &format!("POC {:.2}", fp_levels[poc_idx].price), hdr_med.clone(), COLOR_AMBER);

                            // Row 3: Insight tags (larger, pill-shaped)
                            let mut tag_x = hx;
                            let tag_y = hy + 52.0;
                            let draw_tag = |painter: &egui::Painter, x: &mut f32, label: &str, col: egui::Color32| {
                                let tag_font = egui::FontId::monospace(9.5);
                                let galley = painter.layout_no_wrap(label.to_string(), tag_font.clone(), col);
                                let tw = galley.size().x + 14.0;
                                let th = 16.0;
                                painter.rect_filled(egui::Rect::from_min_size(egui::pos2(*x, tag_y - th / 2.0), egui::vec2(tw, th)),
                                    th / 2.0, egui::Color32::from_rgba_unmultiplied(col.r(), col.g(), col.b(), 30));
                                painter.rect_stroke(egui::Rect::from_min_size(egui::pos2(*x, tag_y - th / 2.0), egui::vec2(tw, th)),
                                    th / 2.0, egui::Stroke::new(0.5, egui::Color32::from_rgba_unmultiplied(col.r(), col.g(), col.b(), 60)), egui::StrokeKind::Outside);
                                painter.text(egui::pos2(*x + tw / 2.0, tag_y), egui::Align2::CENTER_CENTER, label, tag_font, col);
                                *x += tw + 6.0;
                            };
                            if exhaustion { draw_tag(&painter, &mut tag_x, "EXHAUSTION", COLOR_AMBER); }
                            if trapped { draw_tag(&painter, &mut tag_x, "TRAPPED", egui::Color32::from_rgb(200, 100, 200)); }
                            if let Some((label, _, col)) = wick_insight { draw_tag(&painter, &mut tag_x, label, col); }
                            // Imbalance tags
                            for l in &fp_levels {
                                if l.imbalance > 2.5 {
                                    let side = if l.buy > l.sell { "BUY" } else { "SELL" };
                                    draw_tag(&painter, &mut tag_x, &format!("{:.0}:1 {} @ {:.2}", l.imbalance, side, l.price),
                                        if l.buy > l.sell { t.bull } else { t.bear });
                                    break; // only show strongest imbalance
                                }
                            }

                            // Highlight the candle itself
                            let candle_w = (bs * 0.8).max(4.0);
                            painter.rect_stroke(egui::Rect::from_min_max(
                                egui::pos2(bar_x - candle_w, bar_top_y - 2.0),
                                egui::pos2(bar_x + candle_w, bar_bot_y + 2.0)),
                                2.0, egui::Stroke::new(1.5, color_alpha(t.text,100)), egui::StrokeKind::Outside);

                            let card_w = 200.0;
                            let card_h = 48.0;
                            let arm_len = 110.0;

                            for (li, info) in fp_levels.iter().enumerate() {
                                let y = py(info.price);
                                if !y.is_finite() || y < rect.top() + pt + 5.0 || y > rect.top() + pt + ch - 5.0 { continue; }

                                // Alternate left/right
                                let go_left = li % 2 == 0;
                                let card_x = if go_left { bar_x - arm_len - card_w } else { bar_x + arm_len };
                                let elbow_x = if go_left { bar_x - candle_w - 4.0 } else { bar_x + candle_w + 4.0 };
                                let arm_end_x = if go_left { card_x + card_w } else { card_x };

                                // Callout line: horizontal from candle edge to card
                                let line_col = color_alpha(t.text,60);
                                painter.line_segment([egui::pos2(elbow_x, y), egui::pos2(arm_end_x, y)], egui::Stroke::new(1.0, line_col));
                                // Dot at the candle connection point
                                painter.circle_filled(egui::pos2(elbow_x, y), 3.5, color_alpha(t.text,100));
                                painter.circle_stroke(egui::pos2(elbow_x, y), 3.5, egui::Stroke::new(0.5, color_alpha(t.text,40)));

                                // Card background
                                let card_rect = egui::Rect::from_min_size(egui::pos2(card_x, y - card_h / 2.0), egui::vec2(card_w, card_h));
                                let is_poc = li == poc_idx;
                                let is_max_buy = li == max_buy_idx && info.delta > 0.0;
                                let is_max_sell = li == max_sell_idx && info.delta < 0.0;
                                let card_border = if is_poc {
                                    color_alpha(COLOR_AMBER, 120)
                                } else if is_max_buy {
                                    color_alpha(t.bull, 80)
                                } else if is_max_sell {
                                    color_alpha(t.bear, 80)
                                } else {
                                    color_alpha(t.toolbar_border, 40)
                                };
                                painter.rect_filled(card_rect, 6.0, egui::Color32::from_rgba_unmultiplied(t.toolbar_bg.r(), t.toolbar_bg.g(), t.toolbar_bg.b(), 245));
                                painter.rect_stroke(card_rect, 6.0, egui::Stroke::new(if is_poc { 2.0 } else { 1.0 }, card_border), egui::StrokeKind::Outside);

                                // Card content — bigger, more visual
                                let font_price = egui::FontId::monospace(13.0);
                                let font_vol = egui::FontId::monospace(9.0);
                                let font_delta = egui::FontId::monospace(10.0);
                                let font_tag = egui::FontId::monospace(9.0);
                                let cx = card_x + 8.0;
                                let cy = y - card_h / 2.0;

                                // Line 1: Price (large, bright) + Tag
                                painter.text(egui::pos2(cx, cy + 13.0), egui::Align2::LEFT_CENTER,
                                    &format!("{:.2}", info.price), font_price.clone(), egui::Color32::WHITE);

                                // Tag badge (POC / BUY / SELL / ABS) — right-aligned on line 1
                                let absorption = info.vol > bar_data.volume / num_levels as f32 * 1.3 && info.delta.abs() < info.vol * 0.15;
                                let (tag_text, tag_col) = if is_poc {
                                    ("POC", COLOR_AMBER)
                                } else if is_max_buy && info.delta > 0.0 {
                                    ("BUY", t.bull)
                                } else if is_max_sell && info.delta < 0.0 {
                                    ("SELL", t.bear)
                                } else if absorption {
                                    ("ABS", egui::Color32::from_rgb(180, 160, 220))
                                } else {
                                    ("", egui::Color32::TRANSPARENT)
                                };
                                if !tag_text.is_empty() {
                                    let tag_galley = painter.layout_no_wrap(tag_text.to_string(), font_tag.clone(), tag_col);
                                    let tw = tag_galley.size().x + 10.0;
                                    let tag_x = card_x + card_w - tw - 6.0;
                                    painter.rect_filled(egui::Rect::from_min_size(egui::pos2(tag_x, cy + 5.0), egui::vec2(tw, 16.0)),
                                        4.0, egui::Color32::from_rgba_unmultiplied(tag_col.r(), tag_col.g(), tag_col.b(), 30));
                                    painter.text(egui::pos2(tag_x + tw / 2.0, cy + 13.0), egui::Align2::CENTER_CENTER,
                                        tag_text, font_tag.clone(), tag_col);
                                }

                                // Line 2: Buy/Sell bar (tall, clear) + Delta (large)
                                let bar_y = cy + 28.0;
                                let bar_h = 12.0;
                                let bar_total_w = card_w - 80.0;
                                let buy_frac = info.buy_ratio;
                                let sell_bar_w = bar_total_w * (1.0 - buy_frac);
                                let buy_bar_w = bar_total_w * buy_frac;
                                painter.rect_filled(egui::Rect::from_min_size(egui::pos2(cx, bar_y), egui::vec2(sell_bar_w, bar_h)),
                                    3.0, color_alpha(t.bear, 150));
                                painter.rect_filled(egui::Rect::from_min_size(egui::pos2(cx + sell_bar_w, bar_y), egui::vec2(buy_bar_w, bar_h)),
                                    3.0, color_alpha(t.bull, 150));
                                // Sell/Buy labels inside the bars (if wide enough)
                                if sell_bar_w > 30.0 {
                                    painter.text(egui::pos2(cx + sell_bar_w / 2.0, bar_y + bar_h / 2.0), egui::Align2::CENTER_CENTER,
                                        &format!("{:.0}", info.sell), egui::FontId::monospace(8.0), color_alpha(t.text,200));
                                }
                                if buy_bar_w > 30.0 {
                                    painter.text(egui::pos2(cx + sell_bar_w + buy_bar_w / 2.0, bar_y + bar_h / 2.0), egui::Align2::CENTER_CENTER,
                                        &format!("{:.0}", info.buy), egui::FontId::monospace(8.0), color_alpha(t.text,200));
                                }
                                // Delta — large, right side
                                let delta_col = if info.delta > 0.0 { t.bull } else { t.bear };
                                painter.text(egui::pos2(card_x + card_w - 8.0, bar_y + bar_h / 2.0), egui::Align2::RIGHT_CENTER,
                                    &format!("{:+.0}", info.delta), font_delta.clone(), delta_col);
                            }

                            // (Summary moved to header panel above)
                        }
                    }
                }
            }
        }
    }

    // Synced crosshair from other panes
    if !pointer_in_pane && !chart.timestamps.is_empty() {
        let sync_ts = CROSSHAIR_SYNC_TIME.with(|t| t.get());
        if sync_ts > 0 {
            let sync_bar = SignalDrawing::time_to_bar(sync_ts, &chart.timestamps);
            let sync_x = bx(sync_bar);
            if sync_x >= rect.left() && sync_x <= rect.left() + cw {
                painter.line_segment(
                    [egui::pos2(sync_x, rect.top()+pt), egui::pos2(sync_x, rect.top()+pt+ch)],
                    egui::Stroke::new(style::stroke_thin(), color_alpha(t.text,30)));
            }
        }
    }

    // ── P&L equity curve (mini overlay) ──────────────────────────────────
    if chart.show_pnl_curve {
        let pnl_h = 60.0_f32;
        let pnl_top = rect.top() + pt + ch - pnl_h;
        let pnl_bottom = rect.top() + pt + ch;
        let pnl_rect = egui::Rect::from_min_max(
            egui::pos2(rect.left(), pnl_top),
            egui::pos2(rect.left() + cw, pnl_bottom),
        );
        // Subtle background
        painter.rect_filled(pnl_rect, 0.0, egui::Color32::from_rgba_unmultiplied(t.bg.r(), t.bg.g(), t.bg.b(), 180));
        painter.line_segment(
            [egui::pos2(rect.left(), pnl_top), egui::pos2(rect.left() + cw, pnl_top)],
            egui::Stroke::new(0.5, t.dim.gamma_multiply(0.3)));

        // TODO: accumulate P&L snapshots over session for a real curve
        // For now show unrealized P&L as a single value label from account_data_cached
        if let Some((ref acct, _, _)) = account_data_cached {
            let daily = acct.daily_pnl;
            let unr = acct.unrealized_pnl;
            let pnl_color = if daily >= 0.0 { t.bull } else { t.bear };
            let unr_color = if unr >= 0.0 { t.bull } else { t.bear };
            painter.text(
                egui::pos2(rect.left() + 8.0, pnl_top + 14.0),
                egui::Align2::LEFT_CENTER,
                "P&L",
                egui::FontId::monospace(9.0),
                t.dim.gamma_multiply(0.5),
            );
            painter.text(
                egui::pos2(rect.left() + 36.0, pnl_top + 14.0),
                egui::Align2::LEFT_CENTER,
                &format!("Day {:+.0}", daily),
                egui::FontId::monospace(10.0),
                pnl_color,
            );
            painter.text(
                egui::pos2(rect.left() + 110.0, pnl_top + 14.0),
                egui::Align2::LEFT_CENTER,
                &format!("Unr {:+.0}", unr),
                egui::FontId::monospace(10.0),
                unr_color,
            );
            // Zero line
            let zero_y = pnl_top + pnl_h / 2.0;
            painter.line_segment(
                [egui::pos2(rect.left(), zero_y), egui::pos2(rect.left() + cw, zero_y)],
                egui::Stroke::new(0.3, t.dim.gamma_multiply(0.2)));
        } else {
            painter.text(
                egui::pos2(pnl_rect.center().x, pnl_rect.center().y),
                egui::Align2::CENTER_CENTER,
                "P&L — no IB data",
                egui::FontId::monospace(9.0),
                t.dim.gamma_multiply(0.4),
            );
        }
    }

    span_begin("pane_chrome");
    // ── Chart-area top-left badge strip (TF + OV) ──────────────────────────
    {
        let pad = 6.0_f32;
        let bar_h = 18.0_f32;
        let y = rect.top() + pt + pad;
        let mut x = rect.left() + pad;
        let p = ui.painter_at(rect);
        // Timeframe pill
        if !chart.timeframe.is_empty() {
            let tf = chart.timeframe.to_uppercase();
            let font = egui::FontId::monospace(10.0);
            let g = p.layout_no_wrap(tf.clone(), font.clone(), t.text);
            let w = g.size().x + 10.0;
            let r = egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(w, bar_h));
            p.rect_filled(r, 3.0, t.bg.gamma_multiply(0.4));
            p.text(r.center(), egui::Align2::CENTER_CENTER, &tf, font, t.text);
            x += w + 4.0;
        }
        // OV button (overlay editor toggle)
        {
            let has_overlays = !chart.symbol_overlays.is_empty();
            let ov_w = 32.0_f32;
            let ov_rect = egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(ov_w, bar_h));
            let ov_resp = ui.allocate_rect(ov_rect, egui::Sense::click());
            let active = chart.overlay_editing || has_overlays;
            let (bg_col, fg_col) = if chart.overlay_editing {
                (color_alpha(t.accent, 60), t.accent)
            } else if ov_resp.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                (t.bg.gamma_multiply(0.55), if active { t.accent } else { t.text })
            } else if has_overlays {
                (t.bg.gamma_multiply(0.4), t.dim)
            } else {
                (t.bg.gamma_multiply(0.4), t.dim.gamma_multiply(0.85))
            };
            p.rect_filled(ov_rect, 3.0, bg_col);
            p.rect_stroke(ov_rect, 3.0,
                egui::Stroke::new(0.5, t.toolbar_border),
                egui::StrokeKind::Inside);
            p.text(ov_rect.center(), egui::Align2::CENTER_CENTER, "OV",
                egui::FontId::monospace(10.0), fg_col);
            if ov_resp.clicked() {
                chart.overlay_editing = !chart.overlay_editing;
                if chart.overlay_editing { chart.overlay_editing_idx = None; }
            }
            x += ov_w + 4.0;
        }
        // MARK_BARS_PROTOCOL — Last|Mark segmented toggle (option panes only).
        if chart.is_option {
            let seg_h = bar_h;
            let font = egui::FontId::monospace(10.0);
            let parts = [("LAST", false), ("MARK", true)];
            let part_w = 36.0_f32;
            let total_w = part_w * parts.len() as f32;
            let outer = egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(total_w, seg_h));
            // Subtle frame
            p.rect_filled(outer, 3.0, t.bg.gamma_multiply(0.4));
            p.rect_stroke(outer, 3.0, egui::Stroke::new(0.5, t.toolbar_border), egui::StrokeKind::Inside);
            let mark_color = t.bear; // red-ish accent for MARK
            for (idx, (label, is_mark)) in parts.iter().enumerate() {
                let r = egui::Rect::from_min_size(
                    egui::pos2(x + part_w * idx as f32, y), egui::vec2(part_w, seg_h));
                let resp = ui.allocate_rect(r, egui::Sense::click());
                let active = chart.bar_source_mark == *is_mark;
                let hovered = resp.hovered();
                let bg_col = if active {
                    if *is_mark { color_alpha(mark_color, 70) } else { color_alpha(t.accent, 60) }
                } else if hovered {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    t.bg.gamma_multiply(0.55)
                } else {
                    egui::Color32::TRANSPARENT
                };
                let fg_col = if active {
                    if *is_mark { mark_color } else { t.accent }
                } else { t.dim.gamma_multiply(0.95) };
                if bg_col != egui::Color32::TRANSPARENT { p.rect_filled(r, 3.0, bg_col); }
                p.text(r.center(), egui::Align2::CENTER_CENTER, *label, font.clone(), fg_col);
                if resp.clicked() && chart.bar_source_mark != *is_mark {
                    // Toggle source: clear bars, swap WS subs, refetch history.
                    chart.bar_source_mark = *is_mark;
                    chart.bars.clear();
                    chart.timestamps.clear();
                    chart.indicator_bar_count = 0;
                    chart.vol_analytics_computed = 0;
                    chart.history_exhausted = false;
                    let occ = chart.option_contract.clone();
                    let display_sym = chart.symbol.clone();
                    let tf = chart.timeframe.clone();
                    let new_mark = chart.bar_source_mark;
                    if !occ.is_empty() && crate::apex_data::is_enabled() {
                        // fetch_option_bars_background flips the WS subs internally.
                        fetch_option_bars_background(occ, display_sym, tf, new_mark);
                    }
                }
            }
            x += total_w + 4.0;
            // Persistent MARK indicator badge (per spec §"Visual hint") — visible
            // even when the segmented control is offscreen / scrolled.
            if chart.bar_source_mark {
                let badge_text = "MARK";
                let g = p.layout_no_wrap(badge_text.into(), font.clone(), mark_color);
                let bw = g.size().x + 10.0;
                let br = egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(bw, seg_h));
                p.rect_filled(br, 3.0, color_alpha(mark_color, 35));
                p.rect_stroke(br, 3.0, egui::Stroke::new(0.5, mark_color), egui::StrokeKind::Inside);
                p.text(br.center(), egui::Align2::CENTER_CENTER, badge_text, font, mark_color);
                let _ = x; // suppress unused-after warning if more pills appended later
            }
        }
    }

    span_end(); // pane_render

    // ══════════════════════════════════════════════════════════════════════
    // UNIFIED INTERACTION DISPATCH
    //
    // Design: ONE allocate_rect for the whole pane interaction area.
    // Pointer events are classified into zones, then dispatched through a
    // strict priority chain. No overlapping rects, no event stealing.
    //
    // Zone layout:
    //   ┌──────────────────┬────┐
    //   │                  │ Y  │  YAxis: right price strip (pr wide)
    //   │   ChartBody      │Axis│
    //   │                  │    │
    //   ├──────────────────┤    │
    //   │     XAxis        │    │  XAxis: bottom 18px of chart body
    //   └──────────────────┴────┘
    //
    // Priority (highest first):
    //   1. Active drags (drawing, order, xaxis, yaxis) — always finish
    //   2. Modal tools (measure, zoom-select, trigger-pick)
    //   3. Drawing tools (hline, trendline, hzone, barmarker)
    //   4. New drag detection (order line → drawing → xaxis → yaxis → pan)
    //   5. Click dispatch (submit buttons, drawing select/deselect)
    //   6. Scroll zoom
    //   7. Hover cursors
    // ══════════════════════════════════════════════════════════════════════
    span_begin("interaction");

    // Single interaction rect covering chart body + axis strips
    // Shrink all edges that border another pane to avoid stealing drag from divider
    let has_right_neighbor = visible_count > 1 && pane_rects.iter().any(|r| (r.left() - pane_rect.right()).abs() < 5.0);
    let has_left_neighbor = visible_count > 1 && pane_rects.iter().any(|r| (r.right() - pane_rect.left()).abs() < 5.0);
    let has_bottom_neighbor = visible_count > 1 && pane_rects.iter().any(|r| (r.top() - pane_rect.bottom()).abs() < 5.0);
    let has_top_neighbor = visible_count > 1 && pane_rects.iter().any(|r| (r.bottom() - pane_rect.top()).abs() < 5.0);
    // Side-by-side: left pane gives 5px on right (axis side), right pane gives 30px on left (chart side)
    // Stacked: top pane gives 5px on bottom (axis side), bottom pane gives 30px on top (chart side)
    let shrink_left = if has_left_neighbor { 15.0_f32 } else { 0.0 };  // right pane: grab area on left
    let shrink_right = if has_right_neighbor { 5.0 } else { 0.0 };     // left pane: small on axis side
    let shrink_top = if has_top_neighbor { 28.0 } else { 0.0 };        // bottom pane: header (18px) + divider grab (10px)
    let shrink_bottom = 0.0_f32;                                        // top pane: no shrink (axis stays free)
    let interact_rect = egui::Rect::from_min_size(
        egui::pos2(rect.left() + shrink_left, rect.top() + pt + shrink_top),
        egui::vec2(cw + pr - shrink_left - shrink_right, ch - shrink_top - shrink_bottom),
    );
    // Skip chart interaction when pointer is over a floating window (order panel, DOM, etc.)
    let pointer_over_window = ctx.memory(|m| m.any_popup_open())
        || (watchlist.order_entry_open && ui.input(|i| i.pointer.hover_pos()).map_or(false, |p| {
            let abs_pos = if chart.order_panel_pos.y < 0.0 {
                egui::pos2(rect.left() + chart.order_panel_pos.x, rect.top() + pt + ch + chart.order_panel_pos.y)
            } else {
                egui::pos2(rect.left() + chart.order_panel_pos.x, rect.top() + pt + chart.order_panel_pos.y)
            };
            let panel_w = if chart.order_advanced { 300.0 } else { 230.0 };
            egui::Rect::from_min_size(abs_pos, egui::vec2(panel_w, 300.0)).contains(p)
        }));
    let chart_sense = if pointer_over_window { egui::Sense::hover() } else { egui::Sense::click_and_drag() };
    let resp = ui.allocate_rect(interact_rect, chart_sense);

    // Zone geometry (no allocate_rect — just math)
    let xaxis_y_top = rect.top() + pt + ch - 18.0;
    let yaxis_x_left = rect.left() + cw;

    // Classify pointer zone
    #[derive(PartialEq, Clone, Copy)]
    enum Zone { ChartBody, XAxis, YAxis }
    let pointer_zone = |pos: egui::Pos2| -> Zone {
        if pos.x >= yaxis_x_left { Zone::YAxis }
        else if pos.y >= xaxis_y_top { Zone::XAxis }
        else { Zone::ChartBody }
    };

    // Read pointer state once — avoid redundant input reads
    let hover_pos = ui.input(|i| i.pointer.hover_pos());
    let current_zone = hover_pos.map(|p| pointer_zone(p));
    let in_chart_body = current_zone == Some(Zone::ChartBody);
    let in_xaxis = current_zone == Some(Zone::XAxis);
    let in_yaxis = current_zone == Some(Zone::YAxis);
    let shift_held = ui.input(|i| i.modifiers.shift);

    // Activate pane on any interaction
    if visible_count > 1 && (resp.clicked() || resp.drag_started()) {
        *active_pane = pane_idx;
    }

    let pos_to_bar = |pos: egui::Pos2| -> f32 { (pos.x - rect.left() + off - bs*0.5) / bs + vs };
    let pos_to_price = |pos: egui::Pos2| -> f32 { min_p + (max_p-min_p) * (1.0 - (pos.y - rect.top() - pt) / ch) };

    // Hit-test helpers (used for hover cursors, click selection, drag initiation)
    let ts_ref = &chart.timestamps;
    let hit_drawing = |px: f32, py_pos: f32, drawings: &[Drawing]| -> Option<(String, i32)> {
        for d in drawings.iter().rev() {
            match &d.kind {
                DrawingKind::HLine{price} => {
                    if (py_pos - py(*price)).abs() < 12.0 { return Some((d.id.clone(), -1)); }
                }
                DrawingKind::TrendLine{price0,time0,price1,time1} => {
                    let p0 = egui::pos2(bx(SignalDrawing::time_to_bar(*time0, ts_ref)), py(*price0));
                    let p1 = egui::pos2(bx(SignalDrawing::time_to_bar(*time1, ts_ref)), py(*price1));
                    if p0.distance(egui::pos2(px, py_pos)) < 14.0 { return Some((d.id.clone(), 0)); }
                    if p1.distance(egui::pos2(px, py_pos)) < 14.0 { return Some((d.id.clone(), 1)); }
                    let dx = p1.x-p0.x; let dy = p1.y-p0.y; let len2 = dx*dx+dy*dy;
                    if len2 > 0.0 {
                        let t = ((px-p0.x)*dx+(py_pos-p0.y)*dy)/len2;
                        let t = t.max(0.0).min(1.0);
                        if egui::pos2(p0.x+t*dx, p0.y+t*dy).distance(egui::pos2(px, py_pos)) < 10.0 { return Some((d.id.clone(), -1)); }
                    }
                }
                DrawingKind::HZone{price0,price1} => {
                    if (py_pos - py(*price0)).abs() < 10.0 { return Some((d.id.clone(), 0)); }
                    if (py_pos - py(*price1)).abs() < 10.0 { return Some((d.id.clone(), 1)); }
                }
                DrawingKind::BarMarker{time,price,..} => {
                    if egui::pos2(bx(SignalDrawing::time_to_bar(*time, ts_ref)),py(*price)).distance(egui::pos2(px,py_pos)) < 12.0 { return Some((d.id.clone(), -1)); }
                }
                DrawingKind::Fibonacci{price0,time0,price1,time1} => {
                    // Hit on anchor points (ep 0, 1) or any fib/extension level line (ep -1)
                    let p0 = egui::pos2(bx(SignalDrawing::time_to_bar(*time0, ts_ref)), py(*price0));
                    let p1 = egui::pos2(bx(SignalDrawing::time_to_bar(*time1, ts_ref)), py(*price1));
                    if p0.distance(egui::pos2(px, py_pos)) < 14.0 { return Some((d.id.clone(), 0)); }
                    if p1.distance(egui::pos2(px, py_pos)) < 14.0 { return Some((d.id.clone(), 1)); }
                    let range = *price1 - *price0;
                    let xl = p0.x.min(p1.x); let xr = p0.x.max(p1.x);
                    if px >= xl - 5.0 && px <= xr + 5.0 {
                        let all_levels = [0.0_f32, 0.236, 0.382, 0.5, 0.618, 0.786, 1.0,
                            -0.272, -0.618, 1.272, 1.414, 1.618, 2.0, 2.618, 3.146];
                        for &lv in &all_levels {
                            if (py_pos - py(*price0 + range * lv)).abs() < 8.0 { return Some((d.id.clone(), -1)); }
                        }
                    }
                }
                DrawingKind::Channel{price0,time0,price1,time1,offset} | DrawingKind::FibChannel{price0,time0,price1,time1,offset} => {
                    // Hit on base endpoints (0,1), or on either line body (-1), or offset handle (2)
                    let p0 = egui::pos2(bx(SignalDrawing::time_to_bar(*time0, ts_ref)), py(*price0));
                    let p1 = egui::pos2(bx(SignalDrawing::time_to_bar(*time1, ts_ref)), py(*price1));
                    if p0.distance(egui::pos2(px, py_pos)) < 14.0 { return Some((d.id.clone(), 0)); }
                    if p1.distance(egui::pos2(px, py_pos)) < 14.0 { return Some((d.id.clone(), 1)); }
                    // Offset handle (midpoint of parallel line)
                    let qm = egui::pos2((p0.x+p1.x)/2.0, (py(*price0 + *offset) + py(*price1 + *offset))/2.0);
                    if qm.distance(egui::pos2(px, py_pos)) < 14.0 { return Some((d.id.clone(), 2)); }
                    // Line proximity (base line)
                    let dx = p1.x-p0.x; let dy = p1.y-p0.y; let len2 = dx*dx+dy*dy;
                    if len2 > 0.0 {
                        let t = ((px-p0.x)*dx+(py_pos-p0.y)*dy)/len2;
                        let t = t.max(0.0).min(1.0);
                        if egui::pos2(p0.x+t*dx,p0.y+t*dy).distance(egui::pos2(px,py_pos)) < 10.0 { return Some((d.id.clone(),-1)); }
                    }
                    // Parallel line proximity
                    let q0 = egui::pos2(p0.x, py(*price0 + *offset));
                    let q1 = egui::pos2(p1.x, py(*price1 + *offset));
                    let dx2 = q1.x-q0.x; let dy2 = q1.y-q0.y; let len22 = dx2*dx2+dy2*dy2;
                    if len22 > 0.0 {
                        let t = ((px-q0.x)*dx2+(py_pos-q0.y)*dy2)/len22;
                        let t = t.max(0.0).min(1.0);
                        if egui::pos2(q0.x+t*dx2,q0.y+t*dy2).distance(egui::pos2(px,py_pos)) < 10.0 { return Some((d.id.clone(),-1)); }
                    }
                }
                DrawingKind::Pitchfork{price0,time0,price1,time1,price2,time2} => {
                    let p0 = egui::pos2(bx(SignalDrawing::time_to_bar(*time0, ts_ref)), py(*price0));
                    let p1 = egui::pos2(bx(SignalDrawing::time_to_bar(*time1, ts_ref)), py(*price1));
                    let p2 = egui::pos2(bx(SignalDrawing::time_to_bar(*time2, ts_ref)), py(*price2));
                    let c = egui::pos2(px, py_pos);
                    if p0.distance(c) < 14.0 { return Some((d.id.clone(), 0)); }
                    if p1.distance(c) < 14.0 { return Some((d.id.clone(), 1)); }
                    if p2.distance(c) < 14.0 { return Some((d.id.clone(), 2)); }
                    let mid = egui::pos2((p1.x+p2.x)/2.0, (p1.y+p2.y)/2.0);
                    let mdx = mid.x-p0.x; let mdy = mid.y-p0.y; let mlen2 = mdx*mdx+mdy*mdy;
                    if mlen2 > 0.0 { let t = ((px-p0.x)*mdx+(py_pos-p0.y)*mdy)/mlen2; let t = t.max(0.0).min(1.0);
                        if egui::pos2(p0.x+t*mdx,p0.y+t*mdy).distance(c) < 10.0 { return Some((d.id.clone(),-1)); } }
                }
                DrawingKind::GannFan{price0,time0,..} => {
                    let p0 = egui::pos2(bx(SignalDrawing::time_to_bar(*time0, ts_ref)), py(*price0));
                    if p0.distance(egui::pos2(px, py_pos)) < 14.0 { return Some((d.id.clone(), 0)); }
                }
                DrawingKind::RegressionChannel{time0,time1} => {
                    let x0 = bx(SignalDrawing::time_to_bar(*time0, ts_ref));
                    let x1 = bx(SignalDrawing::time_to_bar(*time1, ts_ref));
                    if (px - x0).abs() < 10.0 { return Some((d.id.clone(), 0)); }
                    if (px - x1).abs() < 10.0 { return Some((d.id.clone(), 1)); }
                    if px >= x0.min(x1) - 5.0 && px <= x0.max(x1) + 5.0 { return Some((d.id.clone(), -1)); }
                }
                DrawingKind::XABCD{points} => {
                    let c = egui::pos2(px, py_pos);
                    for (i, &(t, p)) in points.iter().enumerate() {
                        let sp = egui::pos2(bx(SignalDrawing::time_to_bar(t, ts_ref)), py(p));
                        if sp.distance(c) < 14.0 { return Some((d.id.clone(), i as i32)); }
                    }
                }
                DrawingKind::ElliottWave{points,..} => {
                    let c = egui::pos2(px, py_pos);
                    for (i, &(t, p)) in points.iter().enumerate() {
                        let sp = egui::pos2(bx(SignalDrawing::time_to_bar(t, ts_ref)), py(p));
                        if sp.distance(c) < 14.0 { return Some((d.id.clone(), i as i32)); }
                    }
                }
                DrawingKind::AnchoredVWAP{time} => {
                    let ax = bx(SignalDrawing::time_to_bar(*time, ts_ref));
                    if (px - ax).abs() < 12.0 { return Some((d.id.clone(), 0)); }
                }
                DrawingKind::PriceRange{price0,time0,price1,time1} => {
                    let p0 = egui::pos2(bx(SignalDrawing::time_to_bar(*time0, ts_ref)), py(*price0));
                    let p1 = egui::pos2(bx(SignalDrawing::time_to_bar(*time1, ts_ref)), py(*price1));
                    if p0.distance(egui::pos2(px, py_pos)) < 14.0 { return Some((d.id.clone(), 0)); }
                    if p1.distance(egui::pos2(px, py_pos)) < 14.0 { return Some((d.id.clone(), 1)); }
                    let xl = p0.x.min(p1.x); let xr = p0.x.max(p1.x);
                    let yt = p0.y.min(p1.y); let yb = p0.y.max(p1.y);
                    if px >= xl && px <= xr && py_pos >= yt && py_pos <= yb { return Some((d.id.clone(), -1)); }
                }
                DrawingKind::RiskReward{entry_price,entry_time,..} => {
                    let ex = bx(SignalDrawing::time_to_bar(*entry_time, ts_ref));
                    let ey = py(*entry_price);
                    if egui::pos2(ex, ey).distance(egui::pos2(px, py_pos)) < 14.0 { return Some((d.id.clone(), 0)); }
                    if (py_pos - ey).abs() < 12.0 && px >= ex - 20.0 { return Some((d.id.clone(), -1)); }
                }
                DrawingKind::VerticalLine{time} => {
                    let x = bx(SignalDrawing::time_to_bar(*time, ts_ref));
                    if (px - x).abs() < 10.0 { return Some((d.id.clone(), 0)); }
                }
                DrawingKind::Ray{price0,time0,price1,time1} => {
                    let p0 = egui::pos2(bx(SignalDrawing::time_to_bar(*time0, ts_ref)), py(*price0));
                    let p1 = egui::pos2(bx(SignalDrawing::time_to_bar(*time1, ts_ref)), py(*price1));
                    if p0.distance(egui::pos2(px, py_pos)) < 14.0 { return Some((d.id.clone(), 0)); }
                    if p1.distance(egui::pos2(px, py_pos)) < 14.0 { return Some((d.id.clone(), 1)); }
                    let dx = p1.x-p0.x; let dy = p1.y-p0.y; let len2 = dx*dx+dy*dy;
                    if len2 > 0.0 {
                        let t = ((px-p0.x)*dx+(py_pos-p0.y)*dy)/len2;
                        let t = t.max(0.0);
                        if egui::pos2(p0.x+t*dx, p0.y+t*dy).distance(egui::pos2(px, py_pos)) < 10.0 { return Some((d.id.clone(), -1)); }
                    }
                }
                DrawingKind::FibExtension{price0,time0,price1,time1,price2,time2} => {
                    let p0 = egui::pos2(bx(SignalDrawing::time_to_bar(*time0, ts_ref)), py(*price0));
                    let p1 = egui::pos2(bx(SignalDrawing::time_to_bar(*time1, ts_ref)), py(*price1));
                    let p2 = egui::pos2(bx(SignalDrawing::time_to_bar(*time2, ts_ref)), py(*price2));
                    let c = egui::pos2(px, py_pos);
                    if p0.distance(c) < 14.0 { return Some((d.id.clone(), 0)); }
                    if p1.distance(c) < 14.0 { return Some((d.id.clone(), 1)); }
                    if p2.distance(c) < 14.0 { return Some((d.id.clone(), 2)); }
                    if (py_pos - p2.y).abs() < 8.0 && px >= p2.x { return Some((d.id.clone(), -1)); }
                }
                DrawingKind::FibTimeZone{time} => {
                    let x = bx(SignalDrawing::time_to_bar(*time, ts_ref));
                    if (px - x).abs() < 10.0 { return Some((d.id.clone(), 0)); }
                }
                DrawingKind::FibArc{price0,time0,price1,time1} => {
                    let p0 = egui::pos2(bx(SignalDrawing::time_to_bar(*time0, ts_ref)), py(*price0));
                    let p1 = egui::pos2(bx(SignalDrawing::time_to_bar(*time1, ts_ref)), py(*price1));
                    if p0.distance(egui::pos2(px, py_pos)) < 14.0 { return Some((d.id.clone(), 0)); }
                    if p1.distance(egui::pos2(px, py_pos)) < 14.0 { return Some((d.id.clone(), 1)); }
                    let dist = p0.distance(p1);
                    for &ratio in &[0.236_f32, 0.382, 0.5, 0.618, 0.786, 1.0] {
                        let r = dist * ratio;
                        let d_to_center = egui::pos2(px, py_pos).distance(p1);
                        if (d_to_center - r).abs() < 8.0 { return Some((d.id.clone(), -1)); }
                    }
                }
                DrawingKind::GannBox{price0,time0,price1,time1} => {
                    let p0 = egui::pos2(bx(SignalDrawing::time_to_bar(*time0, ts_ref)), py(*price0));
                    let p1 = egui::pos2(bx(SignalDrawing::time_to_bar(*time1, ts_ref)), py(*price1));
                    if p0.distance(egui::pos2(px, py_pos)) < 14.0 { return Some((d.id.clone(), 0)); }
                    if p1.distance(egui::pos2(px, py_pos)) < 14.0 { return Some((d.id.clone(), 1)); }
                    let xl = p0.x.min(p1.x); let xr = p0.x.max(p1.x);
                    let yt = p0.y.min(p1.y); let yb = p0.y.max(p1.y);
                    if px >= xl && px <= xr && py_pos >= yt && py_pos <= yb { return Some((d.id.clone(), -1)); }
                }
                DrawingKind::TextNote{price,time,text,font_size} => {
                    let x = bx(SignalDrawing::time_to_bar(*time, ts_ref));
                    let y = py(*price);
                    let w = text.len() as f32 * font_size * 0.5;
                    let h = font_size * 1.3;
                    if px >= x - 5.0 && px <= x + w + 5.0 && py_pos >= y - 5.0 && py_pos <= y + h + 5.0 {
                        return Some((d.id.clone(), -1));
                    }
                }
            }
        }
        None
    };
    // Hit test order line — EXCLUDE badge area so badge buttons get priority
    let hit_order_line = |pos: egui::Pos2, orders: &[OrderLevel]| -> Option<u32> {
        for order in orders {
            if order.status == OrderStatus::Cancelled || order.status == OrderStatus::Executed { continue; }
            let oy = py(order.price);
            if (pos.y - oy).abs() < 14.0 && pos.x < yaxis_x_left {
                // Compute approximate badge bounds
                let qty_s = format!("{}", order.qty);
                let not_s = fmt_notional(order.notional());
                let is_d = order.status == OrderStatus::Draft;
                let tw = 20.0 + qty_s.len() as f32 * 9.0 + 12.0 + not_s.len() as f32 * 9.0 + 12.0
                    + (if is_d { "DRAFT" } else { "LIVE" }).len() as f32 * 6.0 + 8.0
                    + if is_d { 38.0 } else { 0.0 } + 22.0 + 4.0;
                let bx = rect.left() + cw * 0.60 - tw * 0.5;
                // Only start drag outside the badge
                if pos.x < bx || pos.x > bx + tw {
                    return Some(order.id);
                }
            }
        }
        None
    };

    // Pre-compute hover hit once per frame (avoids redundant linear scans)
    let hover_hit: Option<(String, i32)> = if in_chart_body {
        hover_pos.and_then(|pos| hit_drawing(pos.x, pos.y, &chart.drawings))
    } else { None };
    let hover_order: Option<u32> = if in_chart_body {
        hover_pos.and_then(|pos| hit_order_line(pos, &chart.orders))
    } else { None };

    // Hit test for play lines (same tolerance as orders)
    let hit_play_line = |pos: egui::Pos2, lines: &[crate::chart_renderer::PlayLine]| -> Option<u32> {
        for pl in lines {
            let oy = py(pl.price);
            if (pos.y - oy).abs() < 14.0 && pos.x < yaxis_x_left {
                return Some(pl.id);
            }
        }
        None
    };
    let hover_play_line: Option<u32> = if in_chart_body {
        hover_pos.and_then(|pos| hit_play_line(pos, &chart.play_lines))
    } else { None };

    // Safety valve: clear stale drag state if pointer button is not held
    // AND egui doesn't think a drag is active. This catches lost-focus edge
    // cases without interfering with normal drag_stopped() handling.
    let any_button_down = ui.input(|i| i.pointer.button_down(egui::PointerButton::Primary));
    let egui_dragging = resp.dragged() || resp.drag_stopped();
    if !any_button_down && !egui_dragging {
        if chart.axis_drag_mode != 0 { chart.axis_drag_mode = 0; }
        if chart.dragging_order.is_some() { chart.dragging_order = None; }
        if chart.dragging_alert.is_some() { chart.dragging_alert = None; }
        if chart.dragging_play_line.is_some() { chart.dragging_play_line = None; }
        if chart.dragging_drawing.is_some() {
            if let Some((ref did, _)) = chart.dragging_drawing {
                if let Some(d) = chart.drawings.iter().find(|d| d.id == *did) {
                    crate::drawing_db::save(&drawing_to_db(d, &drawing_persist_key(chart), &chart.timeframe));
                    if let Some(snap) = chart.drag_drawing_snapshot.take() {
                        if chart.undo_stack.len() >= 50 { chart.undo_stack.remove(0); }
                        chart.undo_stack.push(DrawingAction::Modify(did.clone(), snap));
                        chart.redo_stack.clear();
                    }
                }
            }
            chart.dragging_drawing = None;
        }
        if chart.measuring { chart.measuring = false; chart.measure_start = None; chart.measure_active = false; }
    }

    // ── PRIORITY 0: Alert badge PLACE/X click handling (overlay on top of everything) ──
    let mut event_consumed = false;
    {
        let hits: Vec<AlertBadgeHit> = ALERT_BADGE_HITS.with(|h| h.borrow().clone());
        let primary_clicked = ui.input(|i| i.pointer.button_clicked(egui::PointerButton::Primary));
        for hit in &hits {
            if let Some(p) = hover_pos {
                if hit.is_draft && hit.place_rect.contains(p) {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    if primary_clicked {
                        if let Some(a) = chart.price_alerts.iter_mut().find(|a| a.id == hit.alert_id) {
                            a.draft = false;
                        }
                        event_consumed = true;
                    }
                } else if hit.x_rect.contains(p) {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    if primary_clicked {
                        chart.price_alerts.retain(|a| a.id != hit.alert_id);
                        event_consumed = true;
                    }
                }
            }
        }
    }

    // ── PRIORITY 0: Strikes overlay O button click (equity only) ──
    if !chart.is_option { if let Some(pos) = hover_pos {
        if egui::pos2(ovl_chart_x, ovl_chart_y).distance(pos) < 12.0 {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            if ui.input(|i| i.pointer.button_released(egui::PointerButton::Primary)) {
                chart.show_strikes_overlay = !chart.show_strikes_overlay;
                if chart.show_strikes_overlay && !chart.overlay_chain_loading {
                    let needs_fetch = chart.overlay_chain_symbol != chart.symbol
                        || (chart.overlay_calls.is_empty() && chart.overlay_puts.is_empty());
                    if needs_fetch {
                        chart.overlay_chain_loading = true;
                        let sym = chart.symbol.clone();
                        let price = chart.bars.last().map(|b| b.close).unwrap_or(0.0);
                        fetch_overlay_chain_background(sym, price);
                    }
                }
                event_consumed = true;
            }
        }
    }}

    // ── PRIORITY 1: Active drags (always finish, never interrupted) ──────

    // 1a: Order line drag (in progress)
    if let Some(order_id) = chart.dragging_order {
        event_consumed = true;
        if resp.dragged_by(egui::PointerButton::Primary) {
            if let Some(pos) = hover_pos {
                let new_price = pos_to_price(pos);
                crate::chart_renderer::trading::order_manager::modify_order_price(order_id as u64, new_price);
                if let Some(o) = chart.orders.iter_mut().find(|o| o.id == order_id) {
                    o.price = new_price;
                }
            }
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
        }
        if resp.drag_stopped() { chart.dragging_order = None; }
    }

    // 1a-play: Play line drag (in progress)
    if let Some(pl_id) = chart.dragging_play_line {
        event_consumed = true;
        if resp.dragged_by(egui::PointerButton::Primary) {
            if let Some(pos) = hover_pos {
                let new_price = pos_to_price(pos);
                if let Some(pl) = chart.play_lines.iter_mut().find(|p| p.id == pl_id) {
                    pl.price = new_price;
                }
            }
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
        }
        if resp.drag_stopped() { chart.dragging_play_line = None; }
    }

    // 1a-bis: Alert line drag (in progress)
    if let Some(alert_id) = chart.dragging_alert {
        event_consumed = true;
        if resp.dragged_by(egui::PointerButton::Primary) {
            if let Some(pos) = hover_pos {
                let new_price = pos_to_price(pos);
                let current_close = chart.bars.last().map(|b| b.close).unwrap_or(0.0);
                if let Some(a) = chart.price_alerts.iter_mut().find(|a| a.id == alert_id) {
                    a.price = new_price;
                    a.above = new_price > current_close;
                }
            }
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
        }
        if resp.drag_stopped() { chart.dragging_alert = None; }
    }

    // 1b: Drawing drag (in progress)
    if !event_consumed {
        if let Some((ref id, ep)) = chart.dragging_drawing.clone() {
            event_consumed = true;
            if resp.dragged_by(egui::PointerButton::Primary) {
                if let Some(pos) = hover_pos {
                    // For endpoint drags (ep >= 0), use magnet snap if available
                    let new_p = if ep >= 0 { snap_price.unwrap_or_else(|| pos_to_price(pos)) } else { pos_to_price(pos) };
                    let new_b = if ep >= 0 { snap_bar.unwrap_or_else(|| pos_to_bar(pos)) } else { pos_to_bar(pos) };
                    let dp = new_p - chart.drag_start_price;
                    let db = new_b - chart.drag_start_bar;
                    if let Some(d) = chart.drawings.iter_mut().find(|d| d.id == *id) {
                        match &mut d.kind {
                            DrawingKind::HLine{price} => *price += dp,
                            DrawingKind::TrendLine{price0,time0,price1,time1} => match ep {
                                0 => { *price0 = new_p; *time0 = bar_to_time(new_b, &chart.timestamps); }
                                1 => { *price1 = new_p; *time1 = bar_to_time(new_b, &chart.timestamps); }
                                _ => {
                                    *price0 += dp; *price1 += dp;
                                    let b0 = SignalDrawing::time_to_bar(*time0, &chart.timestamps) + db;
                                    let b1 = SignalDrawing::time_to_bar(*time1, &chart.timestamps) + db;
                                    *time0 = bar_to_time(b0, &chart.timestamps);
                                    *time1 = bar_to_time(b1, &chart.timestamps);
                                }
                            },
                            DrawingKind::HZone{price0,price1} => match ep {
                                0 => *price0 = new_p,
                                1 => *price1 = new_p,
                                _ => { *price0 += dp; *price1 += dp; }
                            },
                            DrawingKind::BarMarker{time,price,..} => {
                                let b = SignalDrawing::time_to_bar(*time, &chart.timestamps) + db;
                                *time = bar_to_time(b, &chart.timestamps);
                                *price += dp;
                            },
                            DrawingKind::Fibonacci{price0,time0,price1,time1} => match ep {
                                0 => { *price0 = new_p; *time0 = bar_to_time(new_b, &chart.timestamps); }
                                1 => { *price1 = new_p; *time1 = bar_to_time(new_b, &chart.timestamps); }
                                _ => {
                                    *price0 += dp; *price1 += dp;
                                    let b0 = SignalDrawing::time_to_bar(*time0, &chart.timestamps) + db;
                                    let b1 = SignalDrawing::time_to_bar(*time1, &chart.timestamps) + db;
                                    *time0 = bar_to_time(b0, &chart.timestamps);
                                    *time1 = bar_to_time(b1, &chart.timestamps);
                                }
                            },
                            DrawingKind::Channel{price0,time0,price1,time1,offset} | DrawingKind::FibChannel{price0,time0,price1,time1,offset} => match ep {
                                0 => { *price0 = new_p; *time0 = bar_to_time(new_b, &chart.timestamps); }
                                1 => { *price1 = new_p; *time1 = bar_to_time(new_b, &chart.timestamps); }
                                2 => { *offset += dp; } // drag offset handle
                                _ => {
                                    *price0 += dp; *price1 += dp;
                                    let b0 = SignalDrawing::time_to_bar(*time0, &chart.timestamps) + db;
                                    let b1 = SignalDrawing::time_to_bar(*time1, &chart.timestamps) + db;
                                    *time0 = bar_to_time(b0, &chart.timestamps);
                                    *time1 = bar_to_time(b1, &chart.timestamps);
                                }
                            },
                            DrawingKind::Pitchfork{price0,time0,price1,time1,price2,time2} => match ep {
                                0 => { *price0 = new_p; *time0 = bar_to_time(new_b, &chart.timestamps); }
                                1 => { *price1 = new_p; *time1 = bar_to_time(new_b, &chart.timestamps); }
                                2 => { *price2 = new_p; *time2 = bar_to_time(new_b, &chart.timestamps); }
                                _ => {
                                    *price0 += dp; *price1 += dp; *price2 += dp;
                                    let b0 = SignalDrawing::time_to_bar(*time0, &chart.timestamps) + db;
                                    let b1 = SignalDrawing::time_to_bar(*time1, &chart.timestamps) + db;
                                    let b2 = SignalDrawing::time_to_bar(*time2, &chart.timestamps) + db;
                                    *time0 = bar_to_time(b0, &chart.timestamps);
                                    *time1 = bar_to_time(b1, &chart.timestamps);
                                    *time2 = bar_to_time(b2, &chart.timestamps);
                                }
                            },
                            DrawingKind::GannFan{price0,time0,price1,time1} => match ep {
                                0 => { *price0 = new_p; *time0 = bar_to_time(new_b, &chart.timestamps); }
                                1 => { *price1 = new_p; *time1 = bar_to_time(new_b, &chart.timestamps); }
                                _ => {
                                    *price0 += dp; *price1 += dp;
                                    let b0 = SignalDrawing::time_to_bar(*time0, &chart.timestamps) + db;
                                    let b1 = SignalDrawing::time_to_bar(*time1, &chart.timestamps) + db;
                                    *time0 = bar_to_time(b0, &chart.timestamps);
                                    *time1 = bar_to_time(b1, &chart.timestamps);
                                }
                            },
                            DrawingKind::RegressionChannel{time0,time1} => {
                                let b0 = SignalDrawing::time_to_bar(*time0, &chart.timestamps) + db;
                                let b1 = SignalDrawing::time_to_bar(*time1, &chart.timestamps) + db;
                                *time0 = bar_to_time(b0, &chart.timestamps);
                                *time1 = bar_to_time(b1, &chart.timestamps);
                            }
                            DrawingKind::XABCD{points} => {
                                if ep >= 0 && (ep as usize) < points.len() {
                                    let i = ep as usize;
                                    points[i].0 = bar_to_time(new_b, &chart.timestamps);
                                    points[i].1 = new_p;
                                } else {
                                    for (t, p) in points.iter_mut() {
                                        *p += dp;
                                        let b = SignalDrawing::time_to_bar(*t, &chart.timestamps) + db;
                                        *t = bar_to_time(b, &chart.timestamps);
                                    }
                                }
                            }
                            DrawingKind::ElliottWave{points,..} => {
                                if ep >= 0 && (ep as usize) < points.len() {
                                    let i = ep as usize;
                                    points[i].0 = bar_to_time(new_b, &chart.timestamps);
                                    points[i].1 = new_p;
                                } else {
                                    for (t, p) in points.iter_mut() {
                                        *p += dp;
                                        let b = SignalDrawing::time_to_bar(*t, &chart.timestamps) + db;
                                        *t = bar_to_time(b, &chart.timestamps);
                                    }
                                }
                            }
                            DrawingKind::AnchoredVWAP{time} => {
                                let b = SignalDrawing::time_to_bar(*time, &chart.timestamps) + db;
                                *time = bar_to_time(b, &chart.timestamps);
                            }
                            DrawingKind::PriceRange{price0,time0,price1,time1} => match ep {
                                0 => { *price0 = new_p; *time0 = bar_to_time(new_b, &chart.timestamps); }
                                1 => { *price1 = new_p; *time1 = bar_to_time(new_b, &chart.timestamps); }
                                _ => {
                                    *price0 += dp; *price1 += dp;
                                    let b0 = SignalDrawing::time_to_bar(*time0, &chart.timestamps) + db;
                                    let b1 = SignalDrawing::time_to_bar(*time1, &chart.timestamps) + db;
                                    *time0 = bar_to_time(b0, &chart.timestamps);
                                    *time1 = bar_to_time(b1, &chart.timestamps);
                                }
                            },
                            DrawingKind::RiskReward{entry_price,entry_time,stop_price,target_price} => {
                                *entry_price += dp; *stop_price += dp; *target_price += dp;
                                let b = SignalDrawing::time_to_bar(*entry_time, &chart.timestamps) + db;
                                *entry_time = bar_to_time(b, &chart.timestamps);
                            }
                            DrawingKind::VerticalLine{time} => {
                                let b = SignalDrawing::time_to_bar(*time, &chart.timestamps) + db;
                                *time = bar_to_time(b, &chart.timestamps);
                            }
                            DrawingKind::Ray{price0,time0,price1,time1} => match ep {
                                0 => { *price0 = new_p; *time0 = bar_to_time(new_b, &chart.timestamps); }
                                1 => { *price1 = new_p; *time1 = bar_to_time(new_b, &chart.timestamps); }
                                _ => {
                                    *price0 += dp; *price1 += dp;
                                    let b0 = SignalDrawing::time_to_bar(*time0, &chart.timestamps) + db;
                                    let b1 = SignalDrawing::time_to_bar(*time1, &chart.timestamps) + db;
                                    *time0 = bar_to_time(b0, &chart.timestamps);
                                    *time1 = bar_to_time(b1, &chart.timestamps);
                                }
                            },
                            DrawingKind::FibExtension{price0,time0,price1,time1,price2,time2} => match ep {
                                0 => { *price0 = new_p; *time0 = bar_to_time(new_b, &chart.timestamps); }
                                1 => { *price1 = new_p; *time1 = bar_to_time(new_b, &chart.timestamps); }
                                2 => { *price2 = new_p; *time2 = bar_to_time(new_b, &chart.timestamps); }
                                _ => {
                                    *price0 += dp; *price1 += dp; *price2 += dp;
                                    let b0 = SignalDrawing::time_to_bar(*time0, &chart.timestamps) + db;
                                    let b1 = SignalDrawing::time_to_bar(*time1, &chart.timestamps) + db;
                                    let b2 = SignalDrawing::time_to_bar(*time2, &chart.timestamps) + db;
                                    *time0 = bar_to_time(b0, &chart.timestamps);
                                    *time1 = bar_to_time(b1, &chart.timestamps);
                                    *time2 = bar_to_time(b2, &chart.timestamps);
                                }
                            },
                            DrawingKind::FibTimeZone{time} => {
                                let b = SignalDrawing::time_to_bar(*time, &chart.timestamps) + db;
                                *time = bar_to_time(b, &chart.timestamps);
                            }
                            DrawingKind::FibArc{price0,time0,price1,time1} => match ep {
                                0 => { *price0 = new_p; *time0 = bar_to_time(new_b, &chart.timestamps); }
                                1 => { *price1 = new_p; *time1 = bar_to_time(new_b, &chart.timestamps); }
                                _ => {
                                    *price0 += dp; *price1 += dp;
                                    let b0 = SignalDrawing::time_to_bar(*time0, &chart.timestamps) + db;
                                    let b1 = SignalDrawing::time_to_bar(*time1, &chart.timestamps) + db;
                                    *time0 = bar_to_time(b0, &chart.timestamps);
                                    *time1 = bar_to_time(b1, &chart.timestamps);
                                }
                            },
                            DrawingKind::GannBox{price0,time0,price1,time1} => match ep {
                                0 => { *price0 = new_p; *time0 = bar_to_time(new_b, &chart.timestamps); }
                                1 => { *price1 = new_p; *time1 = bar_to_time(new_b, &chart.timestamps); }
                                _ => {
                                    *price0 += dp; *price1 += dp;
                                    let b0 = SignalDrawing::time_to_bar(*time0, &chart.timestamps) + db;
                                    let b1 = SignalDrawing::time_to_bar(*time1, &chart.timestamps) + db;
                                    *time0 = bar_to_time(b0, &chart.timestamps);
                                    *time1 = bar_to_time(b1, &chart.timestamps);
                                }
                            },
                            DrawingKind::TextNote{price,time,..} => {
                                let b = SignalDrawing::time_to_bar(*time, &chart.timestamps) + db;
                                *time = bar_to_time(b, &chart.timestamps);
                                *price += dp;
                            }
                        }
                    }
                    chart.drag_start_price = new_p;
                    chart.drag_start_bar = new_b;
                }
                ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
            }
            if resp.drag_stopped() {
                if let Some((ref did, _)) = chart.dragging_drawing.clone() {
                    if let Some(d) = chart.drawings.iter().find(|d| d.id == *did) {
                        crate::drawing_db::save(&drawing_to_db(d, &drawing_persist_key(chart), &chart.timeframe));
                        if let Some(snap) = chart.drag_drawing_snapshot.take() {
                            if chart.undo_stack.len() >= 50 { chart.undo_stack.remove(0); }
                            chart.undo_stack.push(DrawingAction::Modify(did.clone(), snap));
                            chart.redo_stack.clear();
                        }
                    }
                }
                chart.dragging_drawing = None;
            }
        }
    }

    // 1c: X-axis drag (in progress) — detected via drag_start zone
    if !event_consumed && chart.axis_drag_mode == 1 {
        event_consumed = true;
        if resp.dragged_by(egui::PointerButton::Primary) {
            let dx = resp.drag_delta().x;
            if dx.abs() > 1.0 {
                let f = if dx > 0.0 { 1.05_f32 } else { 0.95 };
                let old = chart.vc;
                let nw = ((old as f32*f).round() as u32).max(20).min(n as u32);
                let d = (old as i32 - nw as i32) / 2;
                chart.vc = nw; chart.vc_target = nw; chart.vs = (chart.vs + d as f32).max(0.0);
                chart.auto_scroll = false; chart.last_input = std::time::Instant::now();
            }
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
        }
        if resp.drag_stopped() { chart.axis_drag_mode = 0; }
    }

    // 1d: Y-axis drag (in progress)
    if !event_consumed && chart.axis_drag_mode == 2 {
        event_consumed = true;
        if resp.dragged_by(egui::PointerButton::Primary) {
            let dy = resp.drag_delta().y;
            if dy.abs() > 1.0 {
                let f = if dy > 0.0 { 1.05_f32 } else { 0.95 };
                let (lo, hi) = chart.price_range();
                let center = (lo + hi) / 2.0;
                let half = ((hi - lo) / 2.0) * f;
                chart.price_lock = Some((center - half, center + half));
                chart.auto_scroll = false; chart.last_input = std::time::Instant::now();
            }
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
        }
        if resp.drag_stopped() { chart.axis_drag_mode = 0; }
    }

    // ── PRIORITY 2: Modal tools ─────────────────────────────────────────

    // 2a: Measure tool (shift+drag or context menu)
    if !event_consumed && (shift_held || chart.measure_active) && chart.draw_tool.is_empty() {
        // Set cursor unconditionally whenever measure is armed — even if pointer hasn't entered pane yet
        ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
        if let Some(pos) = hover_pos {
            let bar_f = pos_to_bar(pos);
            let price_f = pos_to_price(pos);

            if ui.input(|i| i.pointer.button_pressed(egui::PointerButton::Primary)) && in_chart_body {
                chart.measure_start = Some((bar_f, price_f));
                chart.measuring = true;
            }

            if chart.measuring {
                event_consumed = true;
                if let Some((sb, sp)) = chart.measure_start {
                    let start_pos = egui::pos2(bx(sb), py(sp));
                    let end_pos = egui::pos2(bx(bar_f), py(price_f));

                    // Draw semi-transparent rectangle from start to current cursor
                    let measure_rect = egui::Rect::from_two_pos(start_pos, end_pos);
                    let price_diff = price_f - sp;
                    let fill_color = if price_diff >= 0.0 {
                        egui::Color32::from_rgba_unmultiplied(t.bull.r(), t.bull.g(), t.bull.b(), 20)
                    } else {
                        egui::Color32::from_rgba_unmultiplied(t.bear.r(), t.bear.g(), t.bear.b(), 20)
                    };
                    let stroke_color = if price_diff >= 0.0 { color_alpha(t.bull, 120) } else { color_alpha(t.bear, 120) };
                    painter.rect_filled(measure_rect, 0.0, fill_color);
                    painter.rect_stroke(measure_rect, 0.0, egui::Stroke::new(1.0, stroke_color), egui::StrokeKind::Outside);

                    // Corner dots
                    painter.circle_filled(start_pos, 3.0, t.accent);
                    painter.circle_filled(end_pos, 3.0, t.accent);

                    // Dashed diagonal line
                    let dir = end_pos - start_pos;
                    let len = dir.length();
                    if len > 2.0 {
                        let norm = dir / len;
                        let mut dd = 0.0;
                        while dd < len {
                            let a = start_pos + norm * dd;
                            let b_pt = start_pos + norm * (dd + 4.0).min(len);
                            painter.line_segment([a, b_pt], egui::Stroke::new(1.0, color_alpha(t.accent, 150)));
                            dd += 7.0;
                        }
                    }

                    let bar_diff = (bar_f - sb).abs();
                    let pct = if sp != 0.0 { (price_diff / sp) * 100.0 } else { 0.0 };
                    let candle_sec = if chart.timestamps.len() > 1 { (chart.timestamps[1] - chart.timestamps[0]).max(60) } else { 300 };
                    let time_secs = (bar_diff * candle_sec as f32) as i64;
                    let time_str = if time_secs >= 86400 { format!("{}d {}h", time_secs / 86400, (time_secs % 86400) / 3600) }
                        else if time_secs >= 3600 { format!("{}h {}m", time_secs / 3600, (time_secs % 3600) / 60) }
                        else { format!("{}m", time_secs / 60) };

                    let label = format!("{:+.2} ({:+.2}%)  {} bars  {}", price_diff, pct, bar_diff.round() as i32, time_str);
                    let label_pos = egui::pos2(
                        (start_pos.x + end_pos.x) / 2.0,
                        measure_rect.top() - 14.0,
                    );
                    let label_color = if price_diff >= 0.0 { t.bull } else { t.bear };
                    let galley = painter.layout_no_wrap(label.clone(), egui::FontId::monospace(10.0), label_color);
                    let label_rect = egui::Rect::from_center_size(label_pos, galley.size() + egui::vec2(8.0, 4.0));
                    painter.rect_filled(label_rect, 3.0, egui::Color32::from_rgba_unmultiplied(t.toolbar_bg.r(), t.toolbar_bg.g(), t.toolbar_bg.b(), 220));
                    painter.text(label_pos, egui::Align2::CENTER_CENTER, &label, egui::FontId::monospace(10.0), label_color);
                }

                if ui.input(|i| i.pointer.button_released(egui::PointerButton::Primary)) {
                    chart.measuring = false;
                    chart.measure_start = None;
                    chart.measure_active = false;
                }
            }
        }
    }

    // 2b: Zoom selection
    if !event_consumed && chart.zoom_selecting {
        event_consumed = true;
        let has_start = chart.zoom_start != egui::Pos2::ZERO;
        // Set magnifier cursor unconditionally while zoom tool is armed
        ui.ctx().set_cursor_icon(egui::CursorIcon::ZoomIn);

        if !has_start {
            if resp.clicked() {
                if let Some(pos) = resp.interact_pointer_pos() { chart.zoom_start = pos; }
            }
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) { chart.zoom_selecting = false; }
        } else {
            if let Some(pos) = hover_pos {
                let zr = egui::Rect::from_two_pos(chart.zoom_start, pos);
                painter.rect_filled(zr, 0.0, color_alpha(t.accent, 20));
                painter.rect_stroke(zr, 0.0, egui::Stroke::new(1.0, color_alpha(t.accent, 180)), egui::StrokeKind::Outside);
            }
            if resp.clicked() || resp.drag_stopped() {
                if let Some(pos) = hover_pos {
                    let sx = chart.zoom_start.x; let sy = chart.zoom_start.y;
                    if (pos.x-sx).abs() > 10.0 && (pos.y-sy).abs() > 10.0 {
                        let b_left = pos_to_bar(egui::pos2(sx.min(pos.x), 0.0));
                        let b_right = pos_to_bar(egui::pos2(sx.max(pos.x), 0.0));
                        let p_top = pos_to_price(egui::pos2(0.0, sy.min(pos.y)));
                        let p_bot = pos_to_price(egui::pos2(0.0, sy.max(pos.y)));
                        chart.vs = b_left.max(0.0);
                        chart.vc = ((b_right-b_left).ceil() as u32).max(5);
                        chart.vc_target = chart.vc;
                        chart.price_lock = Some((p_bot.min(p_top), p_bot.max(p_top)));
                        chart.auto_scroll = false; chart.last_input = std::time::Instant::now();
                    }
                }
                chart.zoom_selecting = false;
            }
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) { chart.zoom_selecting = false; }
        }
    }

    // 2c: Trigger order crosshair mode
    if !event_consumed && chart.trigger_setup.phase == TriggerPhase::Picking {
        event_consumed = true;
        ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
        if let Some(mouse) = hover_pos {
            if in_chart_body {
                let price_at_mouse = py_inv(mouse.y);
                let is_buy = chart.trigger_setup.pending_side == OrderSide::Buy;
                let line_color = if is_buy { t.bull } else { t.bear };
                let side_label = if is_buy { "BUY" } else { "SELL" };
                let opt_label = &chart.trigger_setup.option_type;
                painter.line_segment(
                    [egui::pos2(rect.left(), mouse.y), egui::pos2(rect.left() + cw, mouse.y)],
                    egui::Stroke::new(1.5, color_alpha(line_color, 200)));
                painter.text(egui::pos2(rect.left() + cw - 120.0, mouse.y - 14.0), egui::Align2::LEFT_BOTTOM,
                    &format!("{} {} {} @ {:.2}", Icon::LIGHTNING, side_label, opt_label, price_at_mouse),
                    egui::FontId::monospace(10.0), line_color);
                if resp.clicked() {
                    let id = chart.next_trigger_id; chart.next_trigger_id += 1;
                    let above = price_at_mouse > last_price;
                    chart.trigger_levels.push(TriggerLevel {
                        id, side: chart.trigger_setup.pending_side.clone(),
                        trigger_price: price_at_mouse, above,
                        symbol: chart.symbol.clone(),
                        option_type: chart.trigger_setup.option_type.clone(),
                        strike: chart.trigger_setup.strike,
                        expiry: chart.trigger_setup.expiry.clone(),
                        qty: chart.trigger_setup.qty,
                        submitted: false,
                    });
                    chart.trigger_setup.phase = TriggerPhase::Idle;
                }
            }
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                chart.trigger_setup.phase = TriggerPhase::Idle;
            }
        }
    }

    // ── PRIORITY 3: Drawing tools (click to place) ──────────────────────
    if !event_consumed && !chart.draw_tool.is_empty() {
        if resp.clicked() && in_chart_body {
            if let Some(pos) = resp.interact_pointer_pos() {
                // Use magnet-snapped coordinates if available, otherwise raw
                let bar = snap_bar.unwrap_or_else(|| pos_to_bar(pos));
                let price = snap_price.unwrap_or_else(|| pos_to_price(pos));
                let sym = drawing_persist_key(chart);
                let tf = chart.timeframe.clone();
                match chart.draw_tool.as_str() {
                    "hline" => {
                        let mut d = Drawing::new(new_uuid(), DrawingKind::HLine { price });
                        d.color = chart.draw_color.clone(); d.line_style = LineStyle::Dashed;
                        crate::drawing_db::save(&drawing_to_db(&d, &sym, &tf));
                        if chart.undo_stack.len() >= 50 { chart.undo_stack.remove(0); }
                        chart.undo_stack.push(DrawingAction::Add(d.clone())); chart.redo_stack.clear();
                        chart.drawings.push(d); chart.draw_tool.clear();
                    }
                    "trendline" => {
                        if let Some((b0, p0)) = chart.pending_pt {
                            let t0 = bar_to_time(b0, &chart.timestamps);
                            let t1 = bar_to_time(bar, &chart.timestamps);
                            let mut d = Drawing::new(new_uuid(), DrawingKind::TrendLine { price0: p0, time0: t0, price1: price, time1: t1 });
                            d.color = chart.draw_color.clone();
                            crate::drawing_db::save(&drawing_to_db(&d, &sym, &tf));
                            if chart.undo_stack.len() >= 50 { chart.undo_stack.remove(0); }
                            chart.undo_stack.push(DrawingAction::Add(d.clone())); chart.redo_stack.clear();
                            chart.drawings.push(d); chart.pending_pt = None; chart.draw_tool.clear();
                        } else { chart.pending_pt = Some((bar, price)); }
                    }
                    "hzone" => {
                        if let Some((_b0, p0)) = chart.pending_pt {
                            let mut d = Drawing::new(new_uuid(), DrawingKind::HZone { price0: p0, price1: price });
                            d.color = chart.draw_color.clone();
                            crate::drawing_db::save(&drawing_to_db(&d, &sym, &tf));
                            if chart.undo_stack.len() >= 50 { chart.undo_stack.remove(0); }
                            chart.undo_stack.push(DrawingAction::Add(d.clone())); chart.redo_stack.clear();
                            chart.drawings.push(d); chart.pending_pt = None; chart.draw_tool.clear();
                        } else { chart.pending_pt = Some((bar, price)); }
                    }
                    "barmarker" => {
                        let bar_idx = bar.round() as usize;
                        if let Some(b) = chart.bars.get(bar_idx) {
                            let mid = (b.open + b.close) / 2.0;
                            let up = price > mid;
                            let snap_price = if up { b.high } else { b.low };
                            let ts = chart.timestamps.get(bar_idx).copied().unwrap_or(0);
                            let mut d = Drawing::new(new_uuid(), DrawingKind::BarMarker { time: ts, price: snap_price, up });
                            d.color = chart.draw_color.clone();
                            crate::drawing_db::save(&drawing_to_db(&d, &sym, &tf));
                            if chart.undo_stack.len() >= 50 { chart.undo_stack.remove(0); }
                            chart.undo_stack.push(DrawingAction::Add(d.clone())); chart.redo_stack.clear();
                            chart.drawings.push(d); chart.draw_tool.clear();
                        }
                    }
                    "fibonacci" => {
                        // 2-click: first click = anchor, second = end
                        if let Some((b0, p0)) = chart.pending_pt {
                            let t0 = bar_to_time(b0, &chart.timestamps);
                            let t1 = bar_to_time(bar, &chart.timestamps);
                            let mut d = Drawing::new(new_uuid(), DrawingKind::Fibonacci { price0: p0, time0: t0, price1: price, time1: t1 });
                            d.color = "#ffc125".into(); // gold
                            crate::drawing_db::save(&drawing_to_db(&d, &sym, &tf));
                            if chart.undo_stack.len() >= 50 { chart.undo_stack.remove(0); }
                            chart.undo_stack.push(DrawingAction::Add(d.clone())); chart.redo_stack.clear();
                            chart.drawings.push(d); chart.pending_pt = None; chart.draw_tool.clear();
                        } else { chart.pending_pt = Some((bar, price)); }
                    }
                    "channel" => {
                        // 3-click: first two define trendline, third defines offset
                        if let Some((b0, p0)) = chart.pending_pt {
                            if let Some((b1, p1)) = chart.pending_pt2 {
                                // Third click: offset from base midpoint
                                let base_mid = (p0 + p1) / 2.0;
                                let offset = price - base_mid;
                                let t0 = bar_to_time(b0, &chart.timestamps);
                                let t1 = bar_to_time(b1, &chart.timestamps);
                                let mut d = Drawing::new(new_uuid(), DrawingKind::Channel { price0: p0, time0: t0, price1: p1, time1: t1, offset });
                                d.color = "#82dcb4".into(); // teal
                                crate::drawing_db::save(&drawing_to_db(&d, &sym, &tf));
                                if chart.undo_stack.len() >= 50 { chart.undo_stack.remove(0); }
                                chart.undo_stack.push(DrawingAction::Add(d.clone())); chart.redo_stack.clear();
                                chart.drawings.push(d); chart.pending_pt = None; chart.pending_pt2 = None; chart.draw_tool.clear();
                            } else {
                                // Second click: end of base trendline
                                chart.pending_pt2 = Some((bar, price));
                            }
                        } else { chart.pending_pt = Some((bar, price)); }
                    }
                    "fibchannel" => {
                        // 3-click: same as channel but creates FibChannel
                        if let Some((b0, p0)) = chart.pending_pt {
                            if let Some((b1, p1)) = chart.pending_pt2 {
                                let base_mid = (p0 + p1) / 2.0;
                                let offset = price - base_mid;
                                let t0 = bar_to_time(b0, &chart.timestamps);
                                let t1 = bar_to_time(b1, &chart.timestamps);
                                let mut d = Drawing::new(new_uuid(), DrawingKind::FibChannel { price0: p0, time0: t0, price1: p1, time1: t1, offset });
                                d.color = "#c4a35a".into(); // warm gold
                                crate::drawing_db::save(&drawing_to_db(&d, &sym, &tf));
                                if chart.undo_stack.len() >= 50 { chart.undo_stack.remove(0); }
                                chart.undo_stack.push(DrawingAction::Add(d.clone())); chart.redo_stack.clear();
                                chart.drawings.push(d); chart.pending_pt = None; chart.pending_pt2 = None; chart.pending_pts.clear(); chart.draw_tool.clear();
                            } else {
                                chart.pending_pt2 = Some((bar, price));
                            }
                        } else { chart.pending_pt = Some((bar, price)); }
                    }
                    "pitchfork" => {
                        // 3-click: pivot, then upper reaction, then lower reaction
                        chart.pending_pts.push((bar, price));
                        if chart.pending_pts.len() == 3 {
                            let (b0, p0) = chart.pending_pts[0];
                            let (b1, p1) = chart.pending_pts[1];
                            let (b2, p2) = chart.pending_pts[2];
                            let mut d = Drawing::new(new_uuid(), DrawingKind::Pitchfork {
                                price0: p0, time0: bar_to_time(b0, &chart.timestamps),
                                price1: p1, time1: bar_to_time(b1, &chart.timestamps),
                                price2: p2, time2: bar_to_time(b2, &chart.timestamps),
                            });
                            d.color = "#7ecfcf".into(); // teal
                            crate::drawing_db::save(&drawing_to_db(&d, &sym, &tf));
                            if chart.undo_stack.len() >= 50 { chart.undo_stack.remove(0); }
                            chart.undo_stack.push(DrawingAction::Add(d.clone())); chart.redo_stack.clear();
                            chart.drawings.push(d);
                            chart.pending_pt = None; chart.pending_pt2 = None; chart.pending_pts.clear(); chart.draw_tool.clear();
                        }
                    }
                    "gannfan" => {
                        if let Some((b0, p0)) = chart.pending_pt {
                            let mut d = Drawing::new(new_uuid(), DrawingKind::GannFan {
                                price0: p0, time0: bar_to_time(b0, &chart.timestamps),
                                price1: price, time1: bar_to_time(bar, &chart.timestamps),
                            });
                            d.color = "#e8c96b".into(); // gold
                            crate::drawing_db::save(&drawing_to_db(&d, &sym, &tf));
                            if chart.undo_stack.len() >= 50 { chart.undo_stack.remove(0); }
                            chart.undo_stack.push(DrawingAction::Add(d.clone())); chart.redo_stack.clear();
                            chart.drawings.push(d); chart.pending_pt = None; chart.pending_pts.clear(); chart.draw_tool.clear();
                        } else { chart.pending_pt = Some((bar, price)); }
                    }
                    "regression" => {
                        if let Some((b0, _p0)) = chart.pending_pt {
                            let t0 = bar_to_time(b0, &chart.timestamps);
                            let t1 = bar_to_time(bar, &chart.timestamps);
                            let mut d = Drawing::new(new_uuid(), DrawingKind::RegressionChannel { time0: t0, time1: t1 });
                            d.color = "#b480e8".into(); // purple
                            crate::drawing_db::save(&drawing_to_db(&d, &sym, &tf));
                            if chart.undo_stack.len() >= 50 { chart.undo_stack.remove(0); }
                            chart.undo_stack.push(DrawingAction::Add(d.clone())); chart.redo_stack.clear();
                            chart.drawings.push(d); chart.pending_pt = None; chart.pending_pts.clear(); chart.draw_tool.clear();
                        } else { chart.pending_pt = Some((bar, price)); }
                    }
                    "xabcd" => {
                        // 5-click: X A B C D
                        chart.pending_pts.push((bar, price));
                        if chart.pending_pts.len() == 5 {
                            let points: Vec<(i64, f32)> = chart.pending_pts.iter()
                                .map(|&(b, p)| (bar_to_time(b, &chart.timestamps), p)).collect();
                            let mut d = Drawing::new(new_uuid(), DrawingKind::XABCD { points });
                            d.color = "#ff9f43".into(); // orange
                            crate::drawing_db::save(&drawing_to_db(&d, &sym, &tf));
                            if chart.undo_stack.len() >= 50 { chart.undo_stack.remove(0); }
                            chart.undo_stack.push(DrawingAction::Add(d.clone())); chart.redo_stack.clear();
                            chart.drawings.push(d);
                            chart.pending_pt = None; chart.pending_pt2 = None; chart.pending_pts.clear(); chart.draw_tool.clear();
                        }
                    }
                    "elliott_impulse" => {
                        // 5-click: waves 1-5
                        chart.pending_pts.push((bar, price));
                        if chart.pending_pts.len() == 5 {
                            let points: Vec<(i64, f32)> = chart.pending_pts.iter()
                                .map(|&(b, p)| (bar_to_time(b, &chart.timestamps), p)).collect();
                            let mut d = Drawing::new(new_uuid(), DrawingKind::ElliottWave { points, wave_type: 0 });
                            d.color = "#4ecdc4".into();
                            crate::drawing_db::save(&drawing_to_db(&d, &sym, &tf));
                            if chart.undo_stack.len() >= 50 { chart.undo_stack.remove(0); }
                            chart.undo_stack.push(DrawingAction::Add(d.clone())); chart.redo_stack.clear();
                            chart.drawings.push(d);
                            chart.pending_pt = None; chart.pending_pt2 = None; chart.pending_pts.clear(); chart.draw_tool.clear();
                        }
                    }
                    "elliott_corrective" => {
                        // 3-click: A B C
                        chart.pending_pts.push((bar, price));
                        if chart.pending_pts.len() == 3 {
                            let points: Vec<(i64, f32)> = chart.pending_pts.iter()
                                .map(|&(b, p)| (bar_to_time(b, &chart.timestamps), p)).collect();
                            let mut d = Drawing::new(new_uuid(), DrawingKind::ElliottWave { points, wave_type: 1 });
                            d.color = "#4ecdc4".into();
                            crate::drawing_db::save(&drawing_to_db(&d, &sym, &tf));
                            if chart.undo_stack.len() >= 50 { chart.undo_stack.remove(0); }
                            chart.undo_stack.push(DrawingAction::Add(d.clone())); chart.redo_stack.clear();
                            chart.drawings.push(d);
                            chart.pending_pt = None; chart.pending_pt2 = None; chart.pending_pts.clear(); chart.draw_tool.clear();
                        }
                    }
                    "elliott_wxy" => {
                        chart.pending_pts.push((bar, price));
                        if chart.pending_pts.len() == 3 {
                            let points: Vec<(i64, f32)> = chart.pending_pts.iter().map(|&(b, p)| (bar_to_time(b, &chart.timestamps), p)).collect();
                            let mut d = Drawing::new(new_uuid(), DrawingKind::ElliottWave { points, wave_type: 2 });
                            d.color = "#4ecdc4".into();
                            crate::drawing_db::save(&drawing_to_db(&d, &sym, &tf));
                            if chart.undo_stack.len() >= 50 { chart.undo_stack.remove(0); }
                            chart.undo_stack.push(DrawingAction::Add(d.clone())); chart.redo_stack.clear();
                            chart.drawings.push(d);
                            chart.pending_pt = None; chart.pending_pt2 = None; chart.pending_pts.clear(); chart.draw_tool.clear();
                        }
                    }
                    "elliott_wxyxz" => {
                        chart.pending_pts.push((bar, price));
                        if chart.pending_pts.len() == 5 {
                            let points: Vec<(i64, f32)> = chart.pending_pts.iter().map(|&(b, p)| (bar_to_time(b, &chart.timestamps), p)).collect();
                            let mut d = Drawing::new(new_uuid(), DrawingKind::ElliottWave { points, wave_type: 3 });
                            d.color = "#4ecdc4".into();
                            crate::drawing_db::save(&drawing_to_db(&d, &sym, &tf));
                            if chart.undo_stack.len() >= 50 { chart.undo_stack.remove(0); }
                            chart.undo_stack.push(DrawingAction::Add(d.clone())); chart.redo_stack.clear();
                            chart.drawings.push(d);
                            chart.pending_pt = None; chart.pending_pt2 = None; chart.pending_pts.clear(); chart.draw_tool.clear();
                        }
                    }
                    "elliott_sub_impulse" => {
                        chart.pending_pts.push((bar, price));
                        if chart.pending_pts.len() == 5 {
                            let points: Vec<(i64, f32)> = chart.pending_pts.iter().map(|&(b, p)| (bar_to_time(b, &chart.timestamps), p)).collect();
                            let mut d = Drawing::new(new_uuid(), DrawingKind::ElliottWave { points, wave_type: 4 });
                            d.color = "#4ecdc4".into();
                            crate::drawing_db::save(&drawing_to_db(&d, &sym, &tf));
                            if chart.undo_stack.len() >= 50 { chart.undo_stack.remove(0); }
                            chart.undo_stack.push(DrawingAction::Add(d.clone())); chart.redo_stack.clear();
                            chart.drawings.push(d);
                            chart.pending_pt = None; chart.pending_pt2 = None; chart.pending_pts.clear(); chart.draw_tool.clear();
                        }
                    }
                    "elliott_sub_corrective" => {
                        chart.pending_pts.push((bar, price));
                        if chart.pending_pts.len() == 3 {
                            let points: Vec<(i64, f32)> = chart.pending_pts.iter().map(|&(b, p)| (bar_to_time(b, &chart.timestamps), p)).collect();
                            let mut d = Drawing::new(new_uuid(), DrawingKind::ElliottWave { points, wave_type: 5 });
                            d.color = "#4ecdc4".into();
                            crate::drawing_db::save(&drawing_to_db(&d, &sym, &tf));
                            if chart.undo_stack.len() >= 50 { chart.undo_stack.remove(0); }
                            chart.undo_stack.push(DrawingAction::Add(d.clone())); chart.redo_stack.clear();
                            chart.drawings.push(d);
                            chart.pending_pt = None; chart.pending_pt2 = None; chart.pending_pts.clear(); chart.draw_tool.clear();
                        }
                    }
                    "vline" => {
                        let t = bar_to_time(bar, &chart.timestamps);
                        let mut d = Drawing::new(new_uuid(), DrawingKind::VerticalLine { time: t });
                        d.color = chart.draw_color.clone(); d.line_style = LineStyle::Dashed;
                        crate::drawing_db::save(&drawing_to_db(&d, &sym, &tf));
                        if chart.undo_stack.len() >= 50 { chart.undo_stack.remove(0); }
                        chart.undo_stack.push(DrawingAction::Add(d.clone())); chart.redo_stack.clear();
                        chart.drawings.push(d); chart.pending_pt = None; chart.pending_pts.clear(); chart.draw_tool.clear();
                    }
                    "ray" => {
                        if let Some((b0, p0)) = chart.pending_pt {
                            let t0 = bar_to_time(b0, &chart.timestamps);
                            let t1 = bar_to_time(bar, &chart.timestamps);
                            let mut d = Drawing::new(new_uuid(), DrawingKind::Ray { price0: p0, time0: t0, price1: price, time1: t1 });
                            d.color = chart.draw_color.clone();
                            d.extend_right = true;
                            crate::drawing_db::save(&drawing_to_db(&d, &sym, &tf));
                            if chart.undo_stack.len() >= 50 { chart.undo_stack.remove(0); }
                            chart.undo_stack.push(DrawingAction::Add(d.clone())); chart.redo_stack.clear();
                            chart.drawings.push(d); chart.pending_pt = None; chart.pending_pts.clear(); chart.draw_tool.clear();
                        } else { chart.pending_pt = Some((bar, price)); }
                    }
                    "fibext" => {
                        // 3-click: A B C
                        chart.pending_pts.push((bar, price));
                        if chart.pending_pts.len() == 3 {
                            let (b0, p0) = chart.pending_pts[0];
                            let (b1, p1) = chart.pending_pts[1];
                            let (b2, p2) = chart.pending_pts[2];
                            let mut d = Drawing::new(new_uuid(), DrawingKind::FibExtension {
                                price0: p0, time0: bar_to_time(b0, &chart.timestamps),
                                price1: p1, time1: bar_to_time(b1, &chart.timestamps),
                                price2: p2, time2: bar_to_time(b2, &chart.timestamps),
                            });
                            d.color = "#ffd700".into(); // gold
                            crate::drawing_db::save(&drawing_to_db(&d, &sym, &tf));
                            if chart.undo_stack.len() >= 50 { chart.undo_stack.remove(0); }
                            chart.undo_stack.push(DrawingAction::Add(d.clone())); chart.redo_stack.clear();
                            chart.drawings.push(d);
                            chart.pending_pt = None; chart.pending_pt2 = None; chart.pending_pts.clear(); chart.draw_tool.clear();
                        }
                    }
                    "fibtimezone" => {
                        let t = bar_to_time(bar, &chart.timestamps);
                        let mut d = Drawing::new(new_uuid(), DrawingKind::FibTimeZone { time: t });
                        d.color = "#ffc125".into();
                        crate::drawing_db::save(&drawing_to_db(&d, &sym, &tf));
                        if chart.undo_stack.len() >= 50 { chart.undo_stack.remove(0); }
                        chart.undo_stack.push(DrawingAction::Add(d.clone())); chart.redo_stack.clear();
                        chart.drawings.push(d); chart.pending_pt = None; chart.pending_pts.clear(); chart.draw_tool.clear();
                    }
                    "fibarc" => {
                        if let Some((b0, p0)) = chart.pending_pt {
                            let t0 = bar_to_time(b0, &chart.timestamps);
                            let t1 = bar_to_time(bar, &chart.timestamps);
                            let mut d = Drawing::new(new_uuid(), DrawingKind::FibArc { price0: p0, time0: t0, price1: price, time1: t1 });
                            d.color = "#ffc125".into();
                            crate::drawing_db::save(&drawing_to_db(&d, &sym, &tf));
                            if chart.undo_stack.len() >= 50 { chart.undo_stack.remove(0); }
                            chart.undo_stack.push(DrawingAction::Add(d.clone())); chart.redo_stack.clear();
                            chart.drawings.push(d); chart.pending_pt = None; chart.pending_pts.clear(); chart.draw_tool.clear();
                        } else { chart.pending_pt = Some((bar, price)); }
                    }
                    "gannbox" => {
                        if let Some((b0, p0)) = chart.pending_pt {
                            let t0 = bar_to_time(b0, &chart.timestamps);
                            let t1 = bar_to_time(bar, &chart.timestamps);
                            let mut d = Drawing::new(new_uuid(), DrawingKind::GannBox { price0: p0, time0: t0, price1: price, time1: t1 });
                            d.color = "#e8c96b".into(); // gold
                            crate::drawing_db::save(&drawing_to_db(&d, &sym, &tf));
                            if chart.undo_stack.len() >= 50 { chart.undo_stack.remove(0); }
                            chart.undo_stack.push(DrawingAction::Add(d.clone())); chart.redo_stack.clear();
                            chart.drawings.push(d); chart.pending_pt = None; chart.pending_pts.clear(); chart.draw_tool.clear();
                        } else { chart.pending_pt = Some((bar, price)); }
                    }
                    "avwap" => {
                        // 1-click: anchor
                        let t = bar_to_time(bar, &chart.timestamps);
                        let mut d = Drawing::new(new_uuid(), DrawingKind::AnchoredVWAP { time: t });
                        d.color = "#b480e8".into();
                        crate::drawing_db::save(&drawing_to_db(&d, &sym, &tf));
                        if chart.undo_stack.len() >= 50 { chart.undo_stack.remove(0); }
                        chart.undo_stack.push(DrawingAction::Add(d.clone())); chart.redo_stack.clear();
                        chart.drawings.push(d); chart.pending_pt = None; chart.pending_pts.clear(); chart.draw_tool.clear();
                    }
                    "pricerange" => {
                        if let Some((b0, p0)) = chart.pending_pt {
                            let t0 = bar_to_time(b0, &chart.timestamps);
                            let t1 = bar_to_time(bar, &chart.timestamps);
                            let mut d = Drawing::new(new_uuid(), DrawingKind::PriceRange { price0: p0, time0: t0, price1: price, time1: t1 });
                            d.color = "#74b9ff".into();
                            crate::drawing_db::save(&drawing_to_db(&d, &sym, &tf));
                            if chart.undo_stack.len() >= 50 { chart.undo_stack.remove(0); }
                            chart.undo_stack.push(DrawingAction::Add(d.clone())); chart.redo_stack.clear();
                            chart.drawings.push(d); chart.pending_pt = None; chart.pending_pts.clear(); chart.draw_tool.clear();
                        } else { chart.pending_pt = Some((bar, price)); }
                    }
                    "riskreward" => {
                        // 3-click: entry, stop, target
                        chart.pending_pts.push((bar, price));
                        if chart.pending_pts.len() == 3 {
                            let entry_price = chart.pending_pts[0].1;
                            let stop_price  = chart.pending_pts[1].1;
                            let target_price = chart.pending_pts[2].1;
                            let entry_time = bar_to_time(chart.pending_pts[0].0, &chart.timestamps);
                            let mut d = Drawing::new(new_uuid(), DrawingKind::RiskReward { entry_price, entry_time, stop_price, target_price });
                            d.color = "#2ecc71".into();
                            crate::drawing_db::save(&drawing_to_db(&d, &sym, &tf));
                            if chart.undo_stack.len() >= 50 { chart.undo_stack.remove(0); }
                            chart.undo_stack.push(DrawingAction::Add(d.clone())); chart.redo_stack.clear();
                            chart.drawings.push(d);
                            chart.pending_pt = None; chart.pending_pt2 = None; chart.pending_pts.clear(); chart.draw_tool.clear();
                        }
                    }
                    "textnote" => {
                        let t = bar_to_time(bar, &chart.timestamps);
                        let mut d = Drawing::new(new_uuid(), DrawingKind::TextNote { price, time: t, text: String::new(), font_size: 13.0 });
                        d.color = chart.draw_color.clone();
                        chart.text_edit_id = Some(d.id.clone());
                        chart.text_edit_buf = String::new();
                        chart.drawings.push(d);
                        chart.draw_tool.clear();
                    }
                    _ => {}
                }
            }
        }
        event_consumed = true; // drawing tool active = block everything below
    }

    // ── PRIORITY 4: New drag detection ──────────────────────────────────
    if !event_consumed && chart.draw_tool.is_empty() && resp.drag_started_by(egui::PointerButton::Primary) {
        if let Some(pos) = resp.interact_pointer_pos() {
            let zone = pointer_zone(pos);
            match zone {
                Zone::XAxis if !watchlist.pane_divider_dragging => {
                    chart.axis_drag_mode = 1; // x-axis zoom drag
                    event_consumed = true;
                }
                Zone::YAxis if !watchlist.pane_divider_dragging => {
                    chart.axis_drag_mode = 2; // y-axis zoom drag
                    event_consumed = true;
                }
                Zone::ChartBody => {
                    // Priority: alert lines > order lines > drawings > pan
                    // Check if pointer started drag near an alert line (within 10px vertically — generous hit area)
                    let hover_alert: Option<u32> = chart.price_alerts.iter()
                        .filter(|a| !a.triggered && a.symbol == chart.symbol)
                        .find(|a| (py(a.price) - pos.y).abs() <= 10.0)
                        .map(|a| a.id);
                    if let Some(aid) = hover_alert {
                        chart.dragging_alert = Some(aid);
                        event_consumed = true;
                    } else if let Some(plid) = hover_play_line {
                        chart.dragging_play_line = Some(plid);
                        event_consumed = true;
                    } else if let Some(oid) = hover_order {
                        chart.dragging_order = Some(oid);
                        event_consumed = true;
                    } else if let Some((ref id, ep)) = hover_hit {
                        let is_locked = chart.drawings.iter().find(|d| d.id == *id).map_or(false, |d| d.locked);
                        if !is_locked {
                            let ctrl = ui.input(|i| i.modifiers.command);
                            if ctrl && ep < 0 {
                                // Parallel copy: clone the drawing, drag the copy
                                if let Some(src) = chart.drawings.iter().find(|d| d.id == *id).cloned() {
                                    let mut copy = src;
                                    copy.id = new_uuid();
                                    crate::drawing_db::save(&drawing_to_db(&copy, &drawing_persist_key(chart), &chart.timeframe));
                                    if chart.undo_stack.len() >= 50 { chart.undo_stack.remove(0); }
                                    chart.undo_stack.push(DrawingAction::Add(copy.clone()));
                                    chart.redo_stack.clear();
                                    chart.drag_start_price = pos_to_price(pos);
                                    chart.drag_start_bar = pos_to_bar(pos);
                                    chart.drag_drawing_snapshot = Some(copy.clone());
                                    chart.dragging_drawing = Some((copy.id.clone(), ep));
                                    chart.drawings.push(copy);
                                }
                            } else {
                                chart.dragging_drawing = Some((id.clone(), ep));
                                chart.drag_start_price = pos_to_price(pos);
                                chart.drag_start_bar = pos_to_bar(pos);
                                chart.drag_drawing_snapshot = chart.drawings.iter().find(|d| d.id == *id).cloned();
                            }
                            event_consumed = true;
                        }
                    }
                    // else: fall through to pan (handled below)
                }
                _ => {} // divider dragging — ignore axis zones
            }
        }
    }

    // Pan chart (no tool, no active drag, dragging in chart body)
    if !event_consumed && chart.draw_tool.is_empty() && chart.dragging_drawing.is_none()
        && chart.dragging_order.is_none() && chart.axis_drag_mode == 0
        && resp.dragged_by(egui::PointerButton::Primary) {
        let d = resp.drag_delta();
        // Horizontal pan
        chart.vs = (chart.vs - d.x/bs).max(0.0).min(n as f32 + 200.0);
        // Vertical pan — shift price range (only when vertical movement dominates)
        if d.y.abs() > 1.0 && d.y.abs() > d.x.abs() * 1.5 {
            let (lo, hi) = chart.price_range();
            let price_per_px = (hi - lo) / ch;
            let shift = d.y * price_per_px;
            chart.price_lock = Some((lo + shift, hi + shift));
        }
        chart.auto_scroll = false;
        chart.last_input = std::time::Instant::now();
    }

    // ── PRIORITY 4.5: Play click-to-set-price ──────────────────────────
    if !event_consumed {
        if let Some(kind) = chart.play_click_to_set.take() {
            if resp.clicked() {
                if let Some(pos) = resp.interact_pointer_pos() {
                    let price = pos_to_price(pos);
                    if let Some(pl) = chart.play_lines.iter_mut().find(|p| p.kind == kind) {
                        pl.price = price;
                    }
                    event_consumed = true;
                }
            } else {
                // Not clicked yet, keep waiting (put it back)
                chart.play_click_to_set = Some(kind);
                // Show crosshair cursor while in click-to-set mode
                ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
            }
        }
    }

    // ── PRIORITY 5: Click dispatch ──────────────────────────────────────
    if !event_consumed && chart.draw_tool.is_empty() && resp.clicked() {
        if let Some(pos) = resp.interact_pointer_pos() {
            let zone = pointer_zone(pos);
            if zone == Zone::ChartBody {
                // 5a: Check badge button clicks (SEND, X cancel) — matches new badge layout
                let mut submitted = Vec::new();
                let mut cancelled_badge = Vec::new();
                let mut badge_clicked = false;
                for order in &chart.orders {
                    if order.status == OrderStatus::Cancelled || order.status == OrderStatus::Executed { continue; }
                    let oy = py(order.price);
                    let is_draft_o = order.status == OrderStatus::Draft;
                    let badge_h = 24.0_f32;
                    // Recompute badge geometry to match rendering
                    let qty_s = format!("{}", order.qty);
                    let not_s = fmt_notional(order.notional());
                    let status_s = if is_draft_o { "DRAFT" } else { "LIVE" };
                    let side_w = 20.0_f32; let qty_w = qty_s.len() as f32 * 9.0 + 12.0;
                    let not_w = not_s.len() as f32 * 9.0 + 12.0;
                    let status_w = status_s.len() as f32 * 6.0 + 8.0;
                    let send_w = if is_draft_o { 38.0_f32 } else { 0.0 };
                    let x_btn_w = 22.0_f32;
                    let total_w = side_w + qty_w + not_w + status_w + send_w + x_btn_w + 4.0;
                    let bx = rect.left() + cw * 0.60 - total_w * 0.5;
                    let by = oy - badge_h * 0.5;

                    // X cancel button (rightmost)
                    let x_start = bx + total_w - x_btn_w;
                    let x_rect = egui::Rect::from_min_size(egui::pos2(x_start, by), egui::vec2(x_btn_w, badge_h));
                    if x_rect.contains(pos) { cancelled_badge.push(order.id); badge_clicked = true; }

                    // SEND button (drafts only)
                    if is_draft_o {
                        let send_start = x_start - send_w;
                        let send_rect = egui::Rect::from_min_size(egui::pos2(send_start, by), egui::vec2(send_w, badge_h));
                        if send_rect.contains(pos) { submitted.push(order.id); badge_clicked = true; }
                    }

                    // Double-click anywhere on badge opens editor (handled in double-click section)
                    // Single click on badge body = consume event (don't pass to drawings)
                    let full_badge = egui::Rect::from_min_size(egui::pos2(bx, by), egui::vec2(total_w, badge_h));
                    if full_badge.contains(pos) { badge_clicked = true; }
                }
                if !cancelled_badge.is_empty() {
                    for id in &cancelled_badge {
                        crate::chart_renderer::trading::order_manager::cancel_order(*id as u64);
                        cancel_order_with_pair(&mut chart.orders, *id);
                    }
                    chart.pending_confirms.retain(|(id, _)| !cancelled_badge.contains(id));
                } else if !submitted.is_empty() {
                    for id in &submitted {
                        crate::chart_renderer::trading::order_manager::confirm_order(*id as u64);
                        if let Some(o) = chart.orders.iter_mut().find(|o| o.id == *id) {
                            o.status = OrderStatus::Placed;
                            if let Some(pid) = o.pair_id {
                                crate::chart_renderer::trading::order_manager::confirm_order(pid as u64);
                                if let Some(p) = chart.orders.iter_mut().find(|o| o.id == pid && o.status == OrderStatus::Draft) {
                                    p.status = OrderStatus::Placed;
                                }
                            }
                        }
                    }
                } else if !badge_clicked {
                    // 5b: Drawing selection / deselection (use cached hover_hit)
                    let shift = shift_held;
                    if let Some((ref id, _)) = hover_hit {
                        if shift {
                            if chart.selected_ids.contains(id) { chart.selected_ids.retain(|x| x != id); }
                            else { chart.selected_ids.push(id.clone()); }
                        } else {
                            chart.selected_ids = vec![id.clone()];
                        }
                        chart.selected_id = Some(id.clone());
                    } else {
                        chart.selected_id = None;
                        chart.selected_ids.clear();
                    }
                }
            } else if zone == Zone::YAxis && watchlist.order_entry_open {
                // ── Click-on-price: set limit order price from Y-axis click ──
                let clicked_price = py_inv(pos.y);
                chart.order_limit_price = format!("{:.2}", clicked_price);
                chart.order_market = false;
            }
        }
    }

    // ── PRIORITY 6: Scroll zoom (smooth via vc_target) ────────────────
    let scroll = ui.input(|i| i.raw_scroll_delta.y);
    if scroll != 0.0 && resp.hovered() && in_chart_body {
        let f = if scroll > 0.0 { 0.9 } else { 1.1 };
        let old_target = chart.vc_target;
        let new_vc = ((old_target as f32 * f).round() as u32).max(20).min(n as u32);
        chart.vc_target = new_vc;
        chart.auto_scroll = false;
        chart.last_input = std::time::Instant::now();
    }

    // ── Double-click dispatch ────────────────────────────────────────────
    if resp.double_clicked() && chart.draw_tool.is_empty() {
        if let Some(pos) = resp.interact_pointer_pos() {
            let zone = pointer_zone(pos);
            if zone == Zone::YAxis {
                // Double-click Y-axis to reset price zoom
                chart.price_lock = None;
            } else if zone == Zone::ChartBody && chart.editing_order.is_none() {
                // Double-click TextNote to edit
                if let Some((ref id, _)) = hover_hit.clone() {
                    if let Some(d) = chart.drawings.iter().find(|d| d.id == *id) {
                        if let DrawingKind::TextNote { text, .. } = &d.kind {
                            chart.text_edit_id = Some(id.clone());
                            chart.text_edit_buf = text.clone();
                        }
                    }
                }
                // Double-click order line to edit
                let mut found_order = false;
                for order in &chart.orders {
                    if order.status == OrderStatus::Cancelled || order.status == OrderStatus::Executed { continue; }
                    if (pos.y - py(order.price)).abs() < 18.0 && pos.x < yaxis_x_left {
                        chart.editing_order = Some(order.id);
                        chart.edit_order_price = format!("{:.2}", order.price);
                        chart.edit_order_qty = format!("{}", order.qty);
                        found_order = true;
                        break;
                    }
                }
                // Double-click indicator line to edit
                if !found_order {
                    let mut found_indicator = false;
                    for ind in &chart.indicators {
                        if !ind.visible || ind.kind.category() != IndicatorCategory::Overlay { continue; }
                        let bar_i = ((pos.x - rect.left() + off - bs * 0.5) / bs + vs) as usize;
                        for di in 0..7 {
                            let idx = match di { 0 => bar_i, 1 => bar_i.saturating_sub(1), 2 => bar_i + 1, 3 => bar_i.saturating_sub(2), 4 => bar_i + 2, 5 => bar_i.saturating_sub(3), _ => bar_i + 3 };
                            if let Some(&v) = ind.values.get(idx) {
                                if !v.is_nan() && (pos.y - py(v)).abs() < 18.0 {
                                    chart.editing_indicator = Some(ind.id);
                                    found_indicator = true;
                                    break;
                                }
                            }
                            if let Some(&v2) = ind.values2.get(idx) {
                                if !v2.is_nan() && (pos.y - py(v2)).abs() < 18.0 {
                                    chart.editing_indicator = Some(ind.id);
                                    found_indicator = true;
                                    break;
                                }
                            }
                            if let Some(&v3) = ind.values3.get(idx) {
                                if !v3.is_nan() && (pos.y - py(v3)).abs() < 18.0 {
                                    chart.editing_indicator = Some(ind.id);
                                    found_indicator = true;
                                    break;
                                }
                            }
                        }
                        if found_indicator { break; }
                    }
                    // Double-click empty chart body = toggle maximize pane
                    if !found_indicator && hover_hit.is_none() {
                        if watchlist.maximized_pane.is_some() {
                            watchlist.maximized_pane = None;
                        } else {
                            watchlist.maximized_pane = Some(pane_idx);
                        }
                    }
                }
            }
        }
    }

    // ── PRIORITY 7: Hover cursors (uses cached hover_hit / hover_order) ──
    if !event_consumed && pointer_in_pane && chart.draw_tool.is_empty()
        && !chart.measure_active && !chart.zoom_selecting {
        if in_xaxis {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
        } else if in_yaxis {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
        } else if in_chart_body {
            // Alert hover gets vertical resize cursor
            let near_alert = hover_pos.map_or(false, |p| {
                chart.price_alerts.iter()
                    .filter(|a| !a.triggered && a.symbol == chart.symbol)
                    .any(|a| (py(a.price) - p.y).abs() <= 10.0)
            });
            if near_alert {
                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
            } else if let Some((_, ep)) = &hover_hit {
                ui.ctx().set_cursor_icon(if *ep >= 0 { egui::CursorIcon::Grab } else { egui::CursorIcon::Move });
            } else if hover_order.is_some() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
            }
        }
    }

    // ── Chart widgets (floating info cards) ──
    {
        let widget_rect = egui::Rect::from_min_size(
            egui::pos2(rect.left(), rect.top() + pt),
            egui::vec2(cw, ch));
        crate::chart_renderer::ui::chart_widgets::draw_widgets(ui, chart, widget_rect, t);
    }

    // ── FINAL cursor override: modal tools paint their own cursor via phosphor icons ──
    // winit on Windows doesn't support ZoomIn cursor (falls back to arrow) and Crosshair
    // is just a hairline +. We hide the system cursor and paint a phosphor icon at the
    // pointer position for a proper visual signal of the active tool.
    if chart.measure_active || chart.measuring || chart.zoom_selecting {
        ui.ctx().set_cursor_icon(egui::CursorIcon::None);
        if let Some(p) = hover_pos {
            if in_chart_body || chart.measure_active || chart.zoom_selecting {
                let (icon, icon_color) = if chart.zoom_selecting {
                    (Icon::MAGNIFYING_GLASS_PLUS, t.accent)
                } else {
                    (Icon::RULER, t.accent)
                };
                let font = egui::FontId::proportional(22.0);
                // Shadow pass for contrast against any background
                painter.text(p + egui::vec2(1.0, 1.0), egui::Align2::CENTER_CENTER,
                    icon, font.clone(), egui::Color32::from_black_alpha(180));
                painter.text(p, egui::Align2::CENTER_CENTER, icon, font, icon_color);
            }
        }
    }

    // ── Drawing significance tooltip on hover ──────────────────────────
    if let Some((ref hovered_id, _)) = hover_hit {
        if let Some(drawing) = chart.drawings.iter_mut().find(|d| d.id == *hovered_id) {
            // Lazy-compute significance if not yet set
            if drawing.significance.is_none() {
                if matches!(drawing.kind, DrawingKind::TrendLine { .. } | DrawingKind::HLine { .. }
                    | DrawingKind::Ray { .. } | DrawingKind::HZone { .. }) {
                    drawing.significance = DrawingSignificance::estimate(&drawing.kind, &chart.timestamps, &chart.bars);
                }
            }
            if let Some(ref sig) = drawing.significance {
                // Render tooltip near the cursor
                if let Some(ptr) = hover_pos {
                    let tip_x = ptr.x + 16.0;
                    let tip_y = ptr.y + 16.0;
                    let tip_w = 170.0;
                    let tip_h = 120.0;
                    let tip_rect = egui::Rect::from_min_size(egui::pos2(tip_x, tip_y), egui::vec2(tip_w, tip_h));

                    // Background + shadow
                    paint_tooltip_shadow(&painter, tip_rect, 4.0);
                    painter.rect_filled(tip_rect, 4.0, t.toolbar_bg);
                    painter.rect_stroke(tip_rect, 4.0, egui::Stroke::new(0.5, color_alpha(t.toolbar_border, 80)), egui::StrokeKind::Outside);

                    let lx = tip_x + 8.0;
                    let mut ly = tip_y + 10.0;
                    let label_font = egui::FontId::monospace(8.0);
                    let value_font = egui::FontId::monospace(9.0);
                    let dim = t.dim.gamma_multiply(0.5);

                    // Score bar
                    let score_color = if sig.score >= 7.0 { COLOR_AMBER }
                        else if sig.score >= 5.0 { t.bull }
                        else if sig.score >= 3.0 { t.accent }
                        else { t.dim.gamma_multiply(0.6) };
                    painter.text(egui::pos2(lx, ly), egui::Align2::LEFT_CENTER, "Score", label_font.clone(), dim);
                    let bar_x = lx + 42.0;
                    let bar_w = 80.0;
                    let bar_rect = egui::Rect::from_min_size(egui::pos2(bar_x, ly - 4.0), egui::vec2(bar_w, 8.0));
                    painter.rect_filled(bar_rect, 2.0, color_alpha(t.toolbar_border, 40));
                    let fill_w = (sig.score / 10.0).clamp(0.0, 1.0) * bar_w;
                    painter.rect_filled(egui::Rect::from_min_size(bar_rect.min, egui::vec2(fill_w, 8.0)), 2.0, score_color);
                    painter.text(egui::pos2(bar_x + bar_w + 6.0, ly), egui::Align2::LEFT_CENTER, &format!("{:.1}", sig.score), value_font.clone(), score_color);
                    ly += 14.0;

                    // Strength badge
                    let str_color = match sig.strength.as_str() {
                        "CRITICAL" => t.bear,
                        "STRONG" => COLOR_AMBER,
                        "MODERATE" => t.bull,
                        _ => t.dim,
                    };
                    painter.text(egui::pos2(lx, ly), egui::Align2::LEFT_CENTER, "Strength", label_font.clone(), dim);
                    painter.text(egui::pos2(lx + 60.0, ly), egui::Align2::LEFT_CENTER, &sig.strength, value_font.clone(), str_color);
                    ly += 13.0;

                    // Touches
                    painter.text(egui::pos2(lx, ly), egui::Align2::LEFT_CENTER, "Touches", label_font.clone(), dim);
                    painter.text(egui::pos2(lx + 60.0, ly), egui::Align2::LEFT_CENTER, &format!("{}", sig.touches), value_font.clone(), t.dim);
                    ly += 13.0;

                    // Volume Index
                    painter.text(egui::pos2(lx, ly), egui::Align2::LEFT_CENTER, "Vol Idx", label_font.clone(), dim);
                    let vi_color = if sig.volume_index > 1.5 { COLOR_AMBER } else { t.dim };
                    painter.text(egui::pos2(lx + 60.0, ly), egui::Align2::LEFT_CENTER, &format!("{:.1}x", sig.volume_index), value_font.clone(), vi_color);
                    ly += 13.0;

                    // Last Tested
                    painter.text(egui::pos2(lx, ly), egui::Align2::LEFT_CENTER, "Tested", label_font.clone(), dim);
                    let test_str = if sig.last_tested_bars == 0 { "now".to_string() } else { format!("{} bars ago", sig.last_tested_bars) };
                    painter.text(egui::pos2(lx + 60.0, ly), egui::Align2::LEFT_CENTER, &test_str, value_font.clone(), t.dim);
                    ly += 13.0;

                    // Age
                    painter.text(egui::pos2(lx, ly), egui::Align2::LEFT_CENTER, "Age", label_font.clone(), dim);
                    let age_str = if sig.age_days == 0 { "< 1 day".to_string() } else { format!("{} days", sig.age_days) };
                    painter.text(egui::pos2(lx + 60.0, ly), egui::Align2::LEFT_CENTER, &age_str, value_font.clone(), t.dim);

                    // Timeframe (if set by backend)
                    if !sig.timeframe.is_empty() {
                        ly += 13.0;
                        painter.text(egui::pos2(lx, ly), egui::Align2::LEFT_CENTER, "TF", label_font.clone(), dim);
                        painter.text(egui::pos2(lx + 60.0, ly), egui::Align2::LEFT_CENTER, &sig.timeframe, value_font.clone(), t.accent);
                    }
                }
            }
        }
    }

    // ── Keyboard shortcuts ───────────────────────────────────────────────
    if ui.input(|i| i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace)) {
        if !chart.selected_ids.is_empty() {
            for id in &chart.selected_ids {
                if let Some(d) = chart.drawings.iter().find(|d| d.id == *id) {
                    if chart.undo_stack.len() >= 50 { chart.undo_stack.remove(0); }
                    chart.undo_stack.push(DrawingAction::Remove(d.clone()));
                }
                crate::drawing_db::remove(id);
            }
            let ids = chart.selected_ids.clone();
            chart.drawings.retain(|d| !ids.contains(&d.id));
            chart.redo_stack.clear();
            chart.selected_ids.clear();
            chart.selected_id = None;
        } else if let Some(id) = chart.selected_id.take() {
            if let Some(d) = chart.drawings.iter().find(|d| d.id == id) {
                if chart.undo_stack.len() >= 50 { chart.undo_stack.remove(0); }
                chart.undo_stack.push(DrawingAction::Remove(d.clone()));
            }
            crate::drawing_db::remove(&id);
            chart.drawings.retain(|d| d.id != id);
            chart.redo_stack.clear();
        }
    }
    // Ctrl+Z: Undo
    if ui.input(|i| i.modifiers.command && i.key_pressed(egui::Key::Z) && !i.modifiers.shift) {
        if let Some(action) = chart.undo_stack.pop() {
            let toast_desc = match &action {
                DrawingAction::Add(d) => format!("Undone: Added {}", drawing_kind_short(&d.kind)),
                DrawingAction::Remove(d) => format!("Undone: Removed {}", drawing_kind_short(&d.kind)),
                DrawingAction::Modify(_, d) => format!("Undone: Modified {}", drawing_kind_short(&d.kind)),
            };
            let redo_action = match &action {
                DrawingAction::Add(d) => {
                    chart.drawings.retain(|x| x.id != d.id);
                    crate::drawing_db::remove(&d.id);
                    DrawingAction::Remove(d.clone())
                }
                DrawingAction::Remove(d) => {
                    crate::drawing_db::save(&drawing_to_db(d, &drawing_persist_key(chart), &chart.timeframe));
                    chart.drawings.push(d.clone());
                    DrawingAction::Add(d.clone())
                }
                DrawingAction::Modify(id, old) => {
                    let current = chart.drawings.iter().find(|d| d.id == *id).cloned();
                    let pkey = drawing_persist_key(chart);
                    let tf = chart.timeframe.clone();
                    if let Some(d) = chart.drawings.iter_mut().find(|d| d.id == *id) {
                        *d = old.clone();
                        crate::drawing_db::save(&drawing_to_db(d, &pkey, &tf));
                    }
                    DrawingAction::Modify(id.clone(), current.unwrap_or_else(|| old.clone()))
                }
            };
            if chart.redo_stack.len() >= 50 { chart.redo_stack.remove(0); }
            chart.redo_stack.push(redo_action);
            PENDING_TOASTS.with(|ts| ts.borrow_mut().push((toast_desc, 0.0, true)));
        }
    }
    // Ctrl+Shift+Z or Ctrl+Y: Redo
    if ui.input(|i| (i.modifiers.command && i.modifiers.shift && i.key_pressed(egui::Key::Z)) || (i.modifiers.command && i.key_pressed(egui::Key::Y))) {
        if let Some(action) = chart.redo_stack.pop() {
            let toast_desc = match &action {
                DrawingAction::Add(d) => format!("Redone: Added {}", drawing_kind_short(&d.kind)),
                DrawingAction::Remove(d) => format!("Redone: Removed {}", drawing_kind_short(&d.kind)),
                DrawingAction::Modify(_, d) => format!("Redone: Modified {}", drawing_kind_short(&d.kind)),
            };
            let undo_action = match &action {
                DrawingAction::Add(d) => {
                    crate::drawing_db::save(&drawing_to_db(d, &drawing_persist_key(chart), &chart.timeframe));
                    chart.drawings.push(d.clone());
                    DrawingAction::Remove(d.clone())
                }
                DrawingAction::Remove(d) => {
                    chart.drawings.retain(|x| x.id != d.id);
                    crate::drawing_db::remove(&d.id);
                    DrawingAction::Add(d.clone())
                }
                DrawingAction::Modify(id, restored) => {
                    let current = chart.drawings.iter().find(|d| d.id == *id).cloned();
                    let pkey = drawing_persist_key(chart);
                    let tf = chart.timeframe.clone();
                    if let Some(d) = chart.drawings.iter_mut().find(|d| d.id == *id) {
                        *d = restored.clone();
                        crate::drawing_db::save(&drawing_to_db(d, &pkey, &tf));
                    }
                    DrawingAction::Modify(id.clone(), current.unwrap_or_else(|| restored.clone()))
                }
            };
            if chart.undo_stack.len() >= 50 { chart.undo_stack.remove(0); }
            chart.undo_stack.push(undo_action);
            PENDING_TOASTS.with(|ts| ts.borrow_mut().push((toast_desc, 0.0, true)));
        }
    }
    // Ctrl+D: Duplicate selected drawing
    if ui.input(|i| i.modifiers.command && i.key_pressed(egui::Key::D)) {
        if let Some(ref sel_id) = chart.selected_id.clone() {
            if let Some(src) = chart.drawings.iter().find(|d| d.id == *sel_id).cloned() {
                let mut dup = src;
                dup.id = new_uuid();
                let bar_shift = if chart.timestamps.len() > 1 { (chart.timestamps[1] - chart.timestamps[0]) * 5 } else { 1500 };
                shift_drawing_time(&mut dup.kind, bar_shift);
                crate::drawing_db::save(&drawing_to_db(&dup, &drawing_persist_key(chart), &chart.timeframe));
                if chart.undo_stack.len() >= 50 { chart.undo_stack.remove(0); }
                chart.undo_stack.push(DrawingAction::Add(dup.clone()));
                chart.redo_stack.clear();
                chart.selected_id = Some(dup.id.clone());
                chart.selected_ids = vec![dup.id.clone()];
                chart.drawings.push(dup);
            }
        }
    }

    // Ctrl+Shift+S: Screenshot — save metadata + open Windows Snip tool
    if ui.input(|i| i.modifiers.command && i.modifiers.shift && i.key_pressed(egui::Key::S)) {
        // Save screenshot metadata to library
        let ss_entry = crate::chart_renderer::ui::panels::screenshot_panel::save_screenshot(&chart.symbol, &chart.timeframe, chart.vs, chart.vc);
        watchlist.screenshot_entries.insert(0, ss_entry);
        watchlist.screenshot_entries.truncate(200);
        PENDING_TOASTS.with(|ts| ts.borrow_mut().push(("Screenshot saved".into(), 0.0, true)));
        #[cfg(target_os = "windows")]
        {
            let _ = std::process::Command::new("cmd")
                .args(["/C", "start", "ms-screenclip:"])
                .creation_flags(0x08000000)
                .spawn();
        }
    }

    // Context menu (right-click)
    resp.context_menu(|ui| {
        let click_price = ui.input(|i| i.pointer.latest_pos()).map(|p| pos_to_price(p)).unwrap_or(0.0);
        let click_pos = ui.input(|i| i.pointer.latest_pos());

        // ── View controls (top) ──
        if ui.button(format!("{} Reset View", Icon::ARROW_COUNTER_CLOCKWISE)).clicked() {
            chart.auto_scroll = true; chart.price_lock = None;
            chart.vs = (n as f32 - chart.vc as f32 + 8.0).max(0.0);
            ui.close_menu();
        }
        if ui.button(format!("{} Drag Zoom", Icon::MAGNIFYING_GLASS_PLUS)).clicked() {
            chart.zoom_selecting = true; chart.zoom_start = egui::Pos2::ZERO;
            ui.close_menu();
        }
        if ui.button(format!("{} Measure (Shift+Drag)", Icon::RULER)).clicked() {
            chart.measure_active = true; chart.measure_start = None;
            ui.close_menu();
        }
        ui.separator();

        ui.label(egui::RichText::new(format!("ORDERS @ {:.2}", click_price)).small().color(t.dim));
        if ui.button(egui::RichText::new(format!("{} Buy Order", Icon::ARROW_FAT_UP)).color(t.bull)).clicked() {
            use crate::chart_renderer::trading::order_manager::*;
            if let Some(id) = submit_and_get_id(OrderIntent {
                symbol: chart.symbol.clone(), side: OrderSide::Buy,
                order_type: ManagedOrderType::Limit, price: click_price, qty: chart.order_qty,
                source: OrderSource::ChartClick, pair_with: None, option_symbol: None, option_con_id: None, stop_price: 0.0, trail_amount: None, trail_percent: None, last_price: 0.0, tif: 0, outside_rth: false,
            }) {
                chart.orders.push(OrderLevel { id: id as u32, side: OrderSide::Buy, price: click_price, qty: chart.order_qty, status: OrderStatus::Draft, pair_id: None, option_symbol: None, option_con_id: None, trail_amount: None, trail_percent: None });
            }
            ui.close_menu();
        }
        if ui.button(egui::RichText::new(format!("{} Sell Order", Icon::ARROW_FAT_DOWN)).color(t.bear)).clicked() {
            use crate::chart_renderer::trading::order_manager::*;
            if let Some(id) = submit_and_get_id(OrderIntent {
                symbol: chart.symbol.clone(), side: OrderSide::Sell,
                order_type: ManagedOrderType::Limit, price: click_price, qty: chart.order_qty,
                source: OrderSource::ChartClick, pair_with: None, option_symbol: None, option_con_id: None, stop_price: 0.0, trail_amount: None, trail_percent: None, last_price: 0.0, tif: 0, outside_rth: false,
            }) {
                chart.orders.push(OrderLevel { id: id as u32, side: OrderSide::Sell, price: click_price, qty: chart.order_qty, status: OrderStatus::Draft, pair_id: None, option_symbol: None, option_con_id: None, trail_amount: None, trail_percent: None });
            }
            ui.close_menu();
        }
        if ui.button(egui::RichText::new(format!("{} Stop Loss", Icon::SHIELD_WARNING)).color(t.bear)).clicked() {
            use crate::chart_renderer::trading::order_manager::*;
            if let Some(id) = submit_and_get_id(OrderIntent {
                symbol: chart.symbol.clone(), side: OrderSide::Stop,
                order_type: ManagedOrderType::Stop, price: click_price, qty: chart.order_qty,
                source: OrderSource::ChartClick, pair_with: None, option_symbol: None, option_con_id: None, stop_price: 0.0, trail_amount: None, trail_percent: None, last_price: 0.0, tif: 0, outside_rth: false,
            }) {
                chart.orders.push(OrderLevel { id: id as u32, side: OrderSide::Stop, price: click_price, qty: chart.order_qty, status: OrderStatus::Draft, pair_id: None, option_symbol: None, option_con_id: None, trail_amount: None, trail_percent: None });
            }
            ui.close_menu();
        }
        // OCO Bracket (simple) — routed through IB native OCO API
        if ui.button(egui::RichText::new(format!("\u{21C5} OCO Bracket")).color(t.accent)).clicked() {
            use crate::chart_renderer::trading::order_manager::*;
            let target_price = click_price * 1.01;
            let stop_price = click_price * 0.99;
            let results = submit_oco_order(vec![
                OrderIntent {
                    symbol: chart.symbol.clone(), side: OrderSide::OcoTarget,
                    order_type: ManagedOrderType::Limit, price: target_price, stop_price: 0.0, qty: chart.order_qty,
                    source: OrderSource::Oco, pair_with: None, option_symbol: None, option_con_id: None,
                    trail_amount: None, trail_percent: None, last_price: 0.0, tif: 0, outside_rth: false,
                },
                OrderIntent {
                    symbol: chart.symbol.clone(), side: OrderSide::OcoStop,
                    order_type: ManagedOrderType::Stop, price: stop_price, stop_price: stop_price, qty: chart.order_qty,
                    source: OrderSource::Oco, pair_with: None, option_symbol: None, option_con_id: None,
                    trail_amount: None, trail_percent: None, last_price: 0.0, tif: 0, outside_rth: false,
                },
            ]);
            let mut ids: Vec<u64> = Vec::new();
            for r in &results {
                match r {
                    OrderResult::Accepted(id) | OrderResult::NeedsConfirmation(id) => ids.push(*id),
                    _ => {}
                }
            }
            if ids.len() >= 2 {
                chart.orders.push(OrderLevel { id: ids[0] as u32, side: OrderSide::OcoTarget, price: target_price, qty: chart.order_qty, status: OrderStatus::Draft, pair_id: Some(ids[1] as u32), option_symbol: None, option_con_id: None, trail_amount: None, trail_percent: None });
                chart.orders.push(OrderLevel { id: ids[1] as u32, side: OrderSide::OcoStop, price: stop_price, qty: chart.order_qty, status: OrderStatus::Draft, pair_id: Some(ids[0] as u32), option_symbol: None, option_con_id: None, trail_amount: None, trail_percent: None });
            }
            ui.close_menu();
        }
        // Bracket presets submenu
        ui.menu_button(egui::RichText::new(format!("\u{21C5} Bracket Presets \u{25BA}")).color(t.accent), |ui| {
            let templates = chart.bracket_templates.clone();
            let mut delete_idx: Option<usize> = None;
            for (ti, tmpl) in templates.iter().enumerate() {
                ui.horizontal(|ui| {
                    if ui.button(egui::RichText::new(format!("{} (+{}% / -{}%)", tmpl.name, tmpl.target_pct, tmpl.stop_pct)).monospace().size(9.0)).clicked() {
                        use crate::chart_renderer::trading::order_manager::*;
                        let target_price = click_price * (1.0 + tmpl.target_pct / 100.0);
                        let stop_price   = click_price * (1.0 - tmpl.stop_pct  / 100.0);
                        let results = submit_oco_order(vec![
                            OrderIntent {
                                symbol: chart.symbol.clone(), side: OrderSide::OcoTarget,
                                order_type: ManagedOrderType::Limit, price: target_price, stop_price: 0.0, qty: chart.order_qty,
                                source: OrderSource::Oco, pair_with: None, option_symbol: None, option_con_id: None,
                                trail_amount: None, trail_percent: None, last_price: 0.0, tif: 0, outside_rth: false,
                            },
                            OrderIntent {
                                symbol: chart.symbol.clone(), side: OrderSide::OcoStop,
                                order_type: ManagedOrderType::Stop, price: stop_price, stop_price: stop_price, qty: chart.order_qty,
                                source: OrderSource::Oco, pair_with: None, option_symbol: None, option_con_id: None,
                                trail_amount: None, trail_percent: None, last_price: 0.0, tif: 0, outside_rth: false,
                            },
                        ]);
                        let mut ids: Vec<u64> = Vec::new();
                        for r in &results {
                            match r { OrderResult::Accepted(id) | OrderResult::NeedsConfirmation(id) => ids.push(*id), _ => {} }
                        }
                        if ids.len() >= 2 {
                            chart.orders.push(OrderLevel { id: ids[0] as u32, side: OrderSide::OcoTarget, price: target_price, qty: chart.order_qty, status: OrderStatus::Draft, pair_id: Some(ids[1] as u32), option_symbol: None, option_con_id: None, trail_amount: None, trail_percent: None });
                            chart.orders.push(OrderLevel { id: ids[1] as u32, side: OrderSide::OcoStop,   price: stop_price,   qty: chart.order_qty, status: OrderStatus::Draft, pair_id: Some(ids[0] as u32), option_symbol: None, option_con_id: None, trail_amount: None, trail_percent: None });
                        }
                        ui.close_menu();
                    }
                    if ui.add(egui::Button::new(egui::RichText::new(Icon::X).size(8.0).color(t.dim)).frame(false)).clicked() {
                        delete_idx = Some(ti);
                    }
                });
            }
            if let Some(idx) = delete_idx { chart.bracket_templates.remove(idx); }
            ui.separator();
            // Create new preset inline
            ui.label(egui::RichText::new("NEW PRESET").monospace().size(8.0).color(t.dim));
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Name").monospace().size(9.0).color(t.dim));
                ui.add(egui::TextEdit::singleline(&mut chart.new_bracket_name).desired_width(60.0).font(egui::FontId::monospace(9.0)));
            });
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Target %").monospace().size(9.0).color(t.dim));
                ui.add(egui::TextEdit::singleline(&mut chart.new_bracket_target).desired_width(40.0).font(egui::FontId::monospace(9.0)));
                ui.label(egui::RichText::new("Stop %").monospace().size(9.0).color(t.dim));
                ui.add(egui::TextEdit::singleline(&mut chart.new_bracket_stop).desired_width(40.0).font(egui::FontId::monospace(9.0)));
            });
            let can_create = !chart.new_bracket_name.trim().is_empty()
                && chart.new_bracket_target.parse::<f32>().is_ok()
                && chart.new_bracket_stop.parse::<f32>().is_ok();
            if ui.add_enabled(can_create, egui::Button::new(egui::RichText::new(format!("{} Create", Icon::PLUS)).monospace().size(9.0).color(t.accent))).clicked() {
                chart.bracket_templates.push(BracketTemplate {
                    name: chart.new_bracket_name.trim().to_string(),
                    target_pct: chart.new_bracket_target.parse().unwrap_or(1.0),
                    stop_pct: chart.new_bracket_stop.parse().unwrap_or(0.5),
                });
                chart.new_bracket_name.clear();
                chart.new_bracket_target.clear();
                chart.new_bracket_stop.clear();
            }
        });
        if ui.button(egui::RichText::new(format!("\u{27F2} Trigger Order")).color(t.accent)).clicked() {
            use crate::chart_renderer::trading::order_manager::*;
            let target_price = click_price * 1.02;
            if let Some(id1) = submit_and_get_id(OrderIntent {
                symbol: chart.symbol.clone(), side: OrderSide::TriggerBuy,
                order_type: ManagedOrderType::Limit, price: click_price, qty: chart.order_qty,
                source: OrderSource::Trigger, pair_with: None, option_symbol: None, option_con_id: None, stop_price: 0.0, trail_amount: None, trail_percent: None, last_price: 0.0, tif: 0, outside_rth: false,
            }) {
                if let Some(id2) = submit_and_get_id(OrderIntent {
                    symbol: chart.symbol.clone(), side: OrderSide::TriggerSell,
                    order_type: ManagedOrderType::Limit, price: target_price, qty: chart.order_qty,
                    source: OrderSource::Trigger, pair_with: Some(id1), option_symbol: None, option_con_id: None, stop_price: 0.0, trail_amount: None, trail_percent: None, last_price: 0.0, tif: 0, outside_rth: false,
                }) {
                    chart.orders.push(OrderLevel { id: id1 as u32, side: OrderSide::TriggerBuy, price: click_price, qty: chart.order_qty, status: OrderStatus::Draft, pair_id: Some(id2 as u32), option_symbol: None, option_con_id: None, trail_amount: None, trail_percent: None });
                    chart.orders.push(OrderLevel { id: id2 as u32, side: OrderSide::TriggerSell, price: target_price, qty: chart.order_qty, status: OrderStatus::Draft, pair_id: Some(id1 as u32), option_symbol: None, option_con_id: None, trail_amount: None, trail_percent: None });
                }
            }
            ui.close_menu();
        }
        if !chart.orders.is_empty() {
            if ui.button(egui::RichText::new(format!("{} Cancel All Orders", Icon::TRASH)).color(t.bear)).clicked() {
                crate::chart_renderer::trading::order_manager::cancel_all_orders(&chart.symbol);
                chart.orders.clear(); ui.close_menu();
            }
        }
        // ── Play lines (when editor active) ──
        if !chart.play_lines.is_empty() {
            ui.separator();
            ui.label(egui::RichText::new("PLAY LEVELS").small().color(t.accent));
            if ui.button(format!("\u{2295} Set Entry @ {:.2}", click_price)).clicked() {
                if let Some(pl) = chart.play_lines.iter_mut().find(|l| l.kind == crate::chart_renderer::PlayLineKind::Entry) {
                    pl.price = click_price;
                }
                ui.close_menu();
            }
            if ui.button(format!("\u{2295} Set Target @ {:.2}", click_price)).clicked() {
                if let Some(pl) = chart.play_lines.iter_mut().find(|l| l.kind == crate::chart_renderer::PlayLineKind::Target) {
                    pl.price = click_price;
                }
                ui.close_menu();
            }
            if chart.play_lines.iter().any(|l| l.kind == crate::chart_renderer::PlayLineKind::Stop) {
                if ui.button(format!("\u{2295} Set Stop @ {:.2}", click_price)).clicked() {
                    if let Some(pl) = chart.play_lines.iter_mut().find(|l| l.kind == crate::chart_renderer::PlayLineKind::Stop) {
                        pl.price = click_price;
                    }
                    ui.close_menu();
                }
            }
        }
        ui.separator();
        ui.label(egui::RichText::new(format!("ALERTS @ {:.2}", click_price)).small().color(t.dim));
        // Context-menu alerts are created as DRAFTS — user must Place them from the alerts panel
        // (same pattern as orders: draft → placed → active)
        if ui.button(format!("{} Alert Above {:.2}", Icon::ARROW_FAT_UP, click_price)).clicked() {
            let id = chart.next_alert_id; chart.next_alert_id += 1;
            chart.price_alerts.push(PriceAlert { id, price: click_price, above: true, triggered: false, draft: true, symbol: chart.symbol.clone() });
            ui.close_menu();
        }
        if ui.button(format!("{} Alert Below {:.2}", Icon::ARROW_FAT_DOWN, click_price)).clicked() {
            let id = chart.next_alert_id; chart.next_alert_id += 1;
            chart.price_alerts.push(PriceAlert { id, price: click_price, above: false, triggered: false, draft: true, symbol: chart.symbol.clone() });
            ui.close_menu();
        }
        ui.separator();
        ui.label(egui::RichText::new("DRAWING TOOLS").small().color(t.dim));
        {
            let dtm_out = crate::chart_renderer::ui::widgets::drawing::show_drawing_tool_menu(ui, chart, watchlist);
            if let Some(tool) = dtm_out.new_tool {
                chart.draw_tool = tool;
                chart.pending_pt = None; chart.pending_pt2 = None; chart.pending_pts.clear();
            }
        }
        ui.separator();

        // ══════════════════════════════════════════════════════
        // ── TEMPLATES section ──
        ui.separator();
        {
            let tmpl_out = crate::chart_renderer::ui::widgets::drawing::show_template_menu(ui, chart, watchlist);
            // Deferred apply: happens after the menu_button closure releases borrows
            if let Some(i) = tmpl_out.apply_tmpl {
                let tmpl = watchlist.pane_templates[i].1.clone();
                let gb = |key: &str, def: bool| -> bool { tmpl.get(key).and_then(|v| v.as_bool()).unwrap_or(def) };
                chart.show_volume = gb("show_volume", true);
                chart.show_oscillators = gb("show_oscillators", true);
                chart.ohlc_tooltip = gb("ohlc_tooltip", true);
                chart.magnet = gb("magnet", true);
                chart.log_scale = gb("log_scale", false);
                chart.show_vwap_bands = gb("show_vwap_bands", false);
                chart.show_cvd = gb("show_cvd", false);
                chart.show_delta_volume = gb("show_delta_volume", false);
                chart.show_rvol = gb("show_rvol", true);
                chart.show_ma_ribbon = gb("show_ma_ribbon", false);
                chart.show_prev_close = gb("show_prev_close", true);
                chart.show_auto_sr = gb("show_auto_sr", false);
                chart.show_auto_fib = gb("show_auto_fib", false);
                chart.show_footprint = gb("show_footprint", false);
                chart.show_gamma = gb("show_gamma", false);
                chart.show_darkpool = gb("show_darkpool", false);
                chart.show_events = gb("show_events", false);
                chart.hit_highlight = gb("hit_highlight", false);
                chart.show_pnl_curve = gb("show_pnl_curve", false);
                chart.show_pattern_labels = gb("show_pattern_labels", true);
                chart.candle_mode = match tmpl.get("candle_mode").and_then(|v| v.as_str()).unwrap_or("std") {
                    "vln" => CandleMode::Violin, "grd" => CandleMode::Gradient, "vg" => CandleMode::ViolinGradient,
                    "ha" => CandleMode::HeikinAshi, "line" => CandleMode::Line, "area" => CandleMode::Area,
                    "rnk" => CandleMode::Renko, "rng" => CandleMode::RangeBar, "tck" => CandleMode::TickBar,
                    _ => CandleMode::Standard,
                };
                if let Some(inds) = tmpl.get("indicators").and_then(|v| v.as_array()) {
                    chart.indicators.clear();
                    for (idx, ind_json) in inds.iter().enumerate() {
                        let kind_label = ind_json.get("kind").and_then(|v| v.as_str()).unwrap_or("SMA");
                        let kind = IndicatorType::all().iter().find(|t| t.label() == kind_label).copied().unwrap_or(IndicatorType::SMA);
                        let period = ind_json.get("period").and_then(|v| v.as_u64()).unwrap_or(20) as usize;
                        let color = ind_json.get("color").and_then(|v| v.as_str()).unwrap_or(INDICATOR_COLORS[idx % INDICATOR_COLORS.len()]);
                        let id = chart.next_indicator_id; chart.next_indicator_id += 1;
                        let mut ind = Indicator::new(id, kind, period, color);
                        ind.visible = ind_json.get("visible").and_then(|v| v.as_bool()).unwrap_or(true);
                        ind.thickness = ind_json.get("thickness").and_then(|v| v.as_f64()).unwrap_or(1.5) as f32;
                        ind.param2 = ind_json.get("param2").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                        ind.param3 = ind_json.get("param3").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                        ind.param4 = ind_json.get("param4").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                        ind.upper_color = ind_json.get("upper_color").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        ind.lower_color = ind_json.get("lower_color").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        ind.fill_color_hex = ind_json.get("fill_color_hex").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        ind.upper_thickness = ind_json.get("upper_thickness").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                        ind.lower_thickness = ind_json.get("lower_thickness").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                        ind.line_style = match ind_json.get("line_style").and_then(|v| v.as_str()).unwrap_or("solid") {
                            "dashed" => LineStyle::Dashed, "dotted" => LineStyle::Dotted, _ => LineStyle::Solid,
                        };
                        chart.indicators.push(ind);
                    }
                    chart.indicator_bar_count = 0;
                }
            }
            // Save current pane as template (button rendered by show_template_menu above)
            if tmpl_out.save_as_template {
                let name = format!("Template {}", watchlist.pane_templates.len() + 1);
                let indicators: Vec<serde_json::Value> = chart.indicators.iter().map(|ind| serde_json::json!({
                    "kind": ind.kind.label(), "period": ind.period, "color": ind.color,
                    "visible": ind.visible, "thickness": ind.thickness,
                    "param2": ind.param2, "param3": ind.param3, "param4": ind.param4,
                    "source": ind.source, "offset": ind.offset,
                    "ob_level": ind.ob_level, "os_level": ind.os_level,
                    "source_tf": ind.source_tf,
                    "line_style": match ind.line_style { LineStyle::Solid => "solid", LineStyle::Dashed => "dashed", LineStyle::Dotted => "dotted" },
                    "upper_color": ind.upper_color, "lower_color": ind.lower_color,
                    "fill_color_hex": ind.fill_color_hex,
                    "upper_thickness": ind.upper_thickness, "lower_thickness": ind.lower_thickness,
                })).collect();
                let tmpl = serde_json::json!({
                    "show_volume": chart.show_volume, "show_oscillators": chart.show_oscillators,
                    "ohlc_tooltip": chart.ohlc_tooltip, "magnet": chart.magnet, "log_scale": chart.log_scale,
                    "show_vwap_bands": chart.show_vwap_bands, "show_cvd": chart.show_cvd,
                    "show_delta_volume": chart.show_delta_volume, "show_rvol": chart.show_rvol,
                    "show_ma_ribbon": chart.show_ma_ribbon, "show_prev_close": chart.show_prev_close,
                    "show_auto_sr": chart.show_auto_sr, "show_auto_fib": chart.show_auto_fib,
                    "show_footprint": chart.show_footprint, "show_gamma": chart.show_gamma,
                    "show_darkpool": chart.show_darkpool, "show_events": chart.show_events,
                    "hit_highlight": chart.hit_highlight, "show_pnl_curve": chart.show_pnl_curve,
                    "show_pattern_labels": chart.show_pattern_labels,
                    "candle_mode": match chart.candle_mode {
                        CandleMode::Standard => "std", CandleMode::Violin => "vln",
                        CandleMode::Gradient => "grd", CandleMode::ViolinGradient => "vg",
                        CandleMode::HeikinAshi => "ha", CandleMode::Line => "line", CandleMode::Area => "area",
                        CandleMode::Renko => "rnk", CandleMode::RangeBar => "rng", CandleMode::TickBar => "tck",
                    },
                    "indicators": indicators,
                });
                watchlist.pane_templates.push((name, tmpl));
                save_templates(&watchlist.pane_templates);
            }
        }

        // ── HIDE section ──
        // ══════════════════════════════════════════════════════
        let everything_hidden = chart.hide_all_drawings && chart.hide_all_indicators && chart.hide_signal_drawings;
        let hide_all_label = if everything_hidden { "Show All" } else { "Hide All" };
        let hide_all_icon = if everything_hidden { Icon::EYE } else { Icon::EYE_SLASH };
        if ui.button(format!("{} {}", hide_all_icon, hide_all_label)).clicked() {
            let target = !everything_hidden;
            chart.hide_all_drawings    = target;
            chart.hide_all_indicators  = target;
            chart.hide_signal_drawings = target;
            ui.close_menu();
        }
        ui.menu_button(format!("{} Hide / Show \u{25BA}", Icon::EYE), |ui| {
            // Drawings
            ui.label(egui::RichText::new("DRAWINGS").small().color(t.dim));
            {
                let icon = if chart.hide_all_drawings { Icon::EYE_SLASH } else { Icon::EYE };
                let lbl  = if chart.hide_all_drawings { "Show All Drawings" } else { "Hide All Drawings" };
                if ui.button(format!("{} {}", icon, lbl)).clicked() {
                    chart.hide_all_drawings = !chart.hide_all_drawings;
                    ui.close_menu();
                }
            }
            // By drawing group
            for g in chart.groups.clone() {
                let count = chart.drawings.iter().filter(|d| d.group_id == g.id).count();
                if count == 0 { continue; }
                let hidden = chart.hidden_groups.contains(&g.id);
                let icon = if hidden { Icon::EYE_SLASH } else { Icon::EYE };
                let label = format!("  {} {} ({})", icon, g.name, count);
                if ui.button(label).clicked() {
                    if hidden { chart.hidden_groups.retain(|x| x != &g.id); }
                    else      { chart.hidden_groups.push(g.id.clone()); }
                    ui.close_menu();
                }
            }
            ui.separator();

            // Indicators
            ui.label(egui::RichText::new("INDICATORS").small().color(t.dim));
            {
                let icon = if chart.hide_all_indicators { Icon::EYE_SLASH } else { Icon::EYE };
                let lbl  = if chart.hide_all_indicators { "Show All Indicators" } else { "Hide All Indicators" };
                if ui.button(format!("{} {}", icon, lbl)).clicked() {
                    chart.hide_all_indicators = !chart.hide_all_indicators;
                    ui.close_menu();
                }
            }
            let ind_snapshot: Vec<(u32, String, bool)> = chart.indicators.iter()
                .map(|i| (i.id, i.display_name(), i.visible)).collect();
            for (id, name, visible) in &ind_snapshot {
                let icon = if *visible { Icon::EYE } else { Icon::EYE_SLASH };
                let label = format!("  {} {}", icon, name);
                if ui.button(label).clicked() {
                    if let Some(ind) = chart.indicators.iter_mut().find(|i| i.id == *id) {
                        ind.visible = !ind.visible;
                    }
                    ui.close_menu();
                }
            }
            ui.separator();

            // Signals
            ui.label(egui::RichText::new("SIGNALS").small().color(t.dim));
            {
                let icon = if chart.hide_signal_drawings { Icon::EYE_SLASH } else { Icon::EYE };
                let lbl  = if chart.hide_signal_drawings { "Show Signal Lines" } else { "Hide Signal Lines" };
                if ui.button(format!("{} {}", icon, lbl)).clicked() {
                    chart.hide_signal_drawings = !chart.hide_signal_drawings;
                    ui.close_menu();
                }
            }
            {
                let icon = if chart.show_pattern_labels { Icon::EYE } else { Icon::EYE_SLASH };
                let lbl  = if chart.show_pattern_labels { "Hide Pattern Labels" } else { "Show Pattern Labels" };
                if ui.button(format!("{} {}", icon, lbl)).clicked() {
                    chart.show_pattern_labels = !chart.show_pattern_labels;
                    ui.close_menu();
                }
            }
        });

        ui.separator();

        // ══════════════════════════════════════════════════════
        // ── DELETE section ──
        // ══════════════════════════════════════════════════════
        if !chart.selected_ids.is_empty() {
            if ui.button(egui::RichText::new(format!("{} Delete Selected ({})", Icon::TRASH, chart.selected_ids.len())).color(t.bear)).clicked() {
                let ids = chart.selected_ids.clone();
                for d in chart.drawings.iter().filter(|d| ids.contains(&d.id)) {
                    if chart.undo_stack.len() >= 50 { chart.undo_stack.remove(0); }
                    chart.undo_stack.push(DrawingAction::Remove(d.clone()));
                }
                chart.redo_stack.clear();
                for id in &ids { crate::drawing_db::remove(id); }
                chart.drawings.retain(|d| !ids.contains(&d.id));
                chart.selected_ids.clear(); chart.selected_id = None;
                ui.close_menu();
            }
        }
        if !chart.drawings.is_empty() {
            if ui.button(egui::RichText::new(format!("{} Delete All Drawings", Icon::TRASH)).color(t.bear)).clicked() {
                for d in &chart.drawings {
                    if chart.undo_stack.len() >= 50 { chart.undo_stack.remove(0); }
                    chart.undo_stack.push(DrawingAction::Remove(d.clone()));
                }
                chart.redo_stack.clear();
                for d in &chart.drawings { crate::drawing_db::remove(&d.id); }
                chart.drawings.clear();
                chart.selected_ids.clear(); chart.selected_id = None;
                ui.close_menu();
            }
        }
        let temp_count = chart.drawings.iter().filter(|d| d.group_id == "default").count();
        if temp_count > 0 {
            if ui.button(egui::RichText::new(format!("{} Delete Temp Drawings ({})", Icon::TRASH, temp_count)).color(t.bear)).clicked() {
                let to_remove: Vec<String> = chart.drawings.iter().filter(|d| d.group_id == "default").map(|d| d.id.clone()).collect();
                for id in &to_remove { crate::drawing_db::remove(id); }
                chart.drawings.retain(|d| d.group_id != "default");
                chart.selected_ids.clear(); chart.selected_id = None;
                ui.close_menu();
            }
        }
        ui.menu_button(format!("{} Delete \u{25BA}", Icon::TRASH), |ui| {
            let red = t.bear;

            // Drawings
            ui.label(egui::RichText::new("DRAWINGS").small().color(t.dim));
            if !chart.drawings.is_empty() {
                if ui.button(egui::RichText::new(format!("{} All Drawings", Icon::TRASH)).color(red)).clicked() {
                    for d in &chart.drawings {
                        if chart.undo_stack.len() >= 50 { chart.undo_stack.remove(0); }
                        chart.undo_stack.push(DrawingAction::Remove(d.clone()));
                    }
                    chart.redo_stack.clear();
                    for d in &chart.drawings { crate::drawing_db::remove(&d.id); }
                    chart.drawings.clear();
                    chart.selected_ids.clear(); chart.selected_id = None;
                    ui.close_menu();
                }
            }
            // By group — deletes drawings in THIS chart belonging to that group (not the group itself)
            for g in chart.groups.clone() {
                let count = chart.drawings.iter().filter(|d| d.group_id == g.id).count();
                if count == 0 { continue; }
                let label = format!("  {} {} ({})", Icon::TRASH, g.name, count);
                if ui.button(egui::RichText::new(&label).color(red)).clicked() {
                    let gid = g.id.clone();
                    let ids: Vec<String> = chart.drawings.iter().filter(|d| d.group_id == gid).map(|d| d.id.clone()).collect();
                    for d in chart.drawings.iter().filter(|d| d.group_id == gid) {
                        if chart.undo_stack.len() >= 50 { chart.undo_stack.remove(0); }
                        chart.undo_stack.push(DrawingAction::Remove(d.clone()));
                    }
                    chart.redo_stack.clear();
                    for id in &ids { crate::drawing_db::remove(id); }
                    chart.drawings.retain(|d| d.group_id != gid);
                    chart.selected_ids.retain(|sid| !ids.contains(sid));
                    if let Some(ref s) = chart.selected_id { if ids.contains(s) { chart.selected_id = None; } }
                    ui.close_menu();
                }
            }
            ui.separator();

            // Indicators
            ui.label(egui::RichText::new("INDICATORS").small().color(t.dim));
            if !chart.indicators.is_empty() {
                if ui.button(egui::RichText::new(format!("{} All Indicators", Icon::TRASH)).color(red)).clicked() {
                    chart.indicators.clear();
                    chart.indicator_bar_count = 0;
                    ui.close_menu();
                }
            }
            let ind_snapshot: Vec<(u32, String)> = chart.indicators.iter()
                .map(|i| (i.id, i.display_name())).collect();
            for (id, name) in &ind_snapshot {
                let label = format!("  {} {}", Icon::TRASH, name);
                if ui.button(egui::RichText::new(&label).color(red)).clicked() {
                    chart.indicators.retain(|i| i.id != *id);
                    ui.close_menu();
                }
            }
            ui.separator();

            // Signals
            ui.label(egui::RichText::new("SIGNALS").small().color(t.dim));
            if !chart.signal_drawings.is_empty() {
                if ui.button(egui::RichText::new(format!("{} Signal Drawings ({})", Icon::TRASH, chart.signal_drawings.len())).color(red)).clicked() {
                    chart.signal_drawings.clear();
                    ui.close_menu();
                }
            }
            if !chart.pattern_labels.is_empty() {
                if ui.button(egui::RichText::new(format!("{} Pattern Labels ({})", Icon::TRASH, chart.pattern_labels.len())).color(red)).clicked() {
                    chart.pattern_labels.clear();
                    ui.close_menu();
                }
            }
        });
    });

    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        chart.draw_tool.clear(); chart.pending_pt = None; chart.pending_pt2 = None; chart.pending_pts.clear();
        chart.selected_id = None; chart.editing_indicator = None; chart.editing_order = None;
        if let Some(ref edit_id) = chart.text_edit_id.clone() {
            chart.drawings.retain(|d| d.id != *edit_id);
            crate::drawing_db::remove(edit_id);
            chart.text_edit_id = None; chart.text_edit_buf.clear();
        }
    }

    // ── Drawing properties bar (horizontal, top-center of chart) ──────────
    // Close the filter dialog when a drawing is selected to avoid two overlapping panels
    // (Old drawing list panel removed — consolidated into object_tree.rs)

    if chart.selected_id.is_some() { watchlist.trendline_filter_open = false; }
    if chart.selected_id.is_some() {
        let bar_y = rect.top() + pt + 4.0;
        let est_w = 520.0;
        let bar_x = (rect.left() + cw / 2.0 - est_w / 2.0).max(rect.left() + 4.0);
        let props_out = egui::Area::new(egui::Id::new(format!("draw_props_{}", pane_idx)))
            .fixed_pos(egui::pos2(bar_x, bar_y))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                // Theme the combo box popups to match the chart
                ui.style_mut().visuals.widgets.inactive.bg_fill = t.toolbar_bg;
                ui.style_mut().visuals.widgets.hovered.bg_fill = color_alpha(t.toolbar_border, 80);
                ui.style_mut().visuals.widgets.active.bg_fill = color_alpha(t.accent, 40);
                ui.style_mut().visuals.selection.bg_fill = color_alpha(t.accent, 50);
                ui.style_mut().visuals.popup_shadow = egui::epaint::Shadow::NONE;
                ui.style_mut().visuals.window_fill = t.toolbar_bg;
                ui.style_mut().visuals.extreme_bg_color = t.toolbar_bg;
                egui::Frame::popup(&ctx.style())
                    .fill(t.toolbar_bg)
                    .stroke(egui::Stroke::new(0.5, t.toolbar_border))
                    .inner_margin(egui::Margin { left: 8, right: 8, top: 4, bottom: 4 })
                    .corner_radius(4.0)
                    .show(ui, |ui| {
                        crate::chart_renderer::ui::widgets::drawing::properties_bar::show_drawing_properties_bar_ui(
                            ui, ctx, t, chart, pane_idx,
                        )
                    }).inner
            }).inner;
        if props_out.delete_sel {
            chart.selected_id = None;
            chart.selected_ids.clear();
        }
        if props_out.open_group_manager {
            chart.group_manager_open = true;
        }
    }

    // ── Text note editing ─────────────────────────────────────────────────
    if let Some(ref edit_id) = chart.text_edit_id.clone() {
        let draw_info = chart.drawings.iter().find(|d| d.id == *edit_id).and_then(|d| {
            if let DrawingKind::TextNote { price, time, font_size, .. } = &d.kind {
                Some((*price, *time, *font_size))
            } else { None }
        });
        if let Some((price, time, font_size)) = draw_info {
            let x = bx(SignalDrawing::time_to_bar(time, &chart.timestamps));
            let y = py(price);
            let tn_out = crate::chart_renderer::ui::widgets::drawing::show_text_note_editor(
                crate::chart_renderer::ui::widgets::drawing::text_note_editor::TextNoteCtx {
                    ctx, x, y,
                    pane_idx,
                    text_buf: &mut chart.text_edit_buf,
                    font_size,
                },
            );
            if tn_out.discard {
                chart.drawings.retain(|d| d.id != *edit_id);
                crate::drawing_db::remove(edit_id);
                chart.text_edit_id = None; chart.text_edit_buf.clear();
            } else if let Some(text) = tn_out.commit {
                let pkey = drawing_persist_key(chart);
                let tf_local = chart.timeframe.clone();
                if let Some(d) = chart.drawings.iter_mut().find(|d| d.id == *edit_id) {
                    if let DrawingKind::TextNote { text: ref mut t, .. } = &mut d.kind { *t = text; }
                    if chart.undo_stack.len() >= 50 { chart.undo_stack.remove(0); }
                    chart.undo_stack.push(DrawingAction::Add(d.clone()));
                    chart.redo_stack.clear();
                    crate::drawing_db::save(&drawing_to_db(d, &pkey, &tf_local));
                }
                chart.text_edit_id = None; chart.text_edit_buf.clear();
            }
        }
    }

    // M key toggles magnet mode
    if ui.input(|i| i.key_pressed(egui::Key::M)) && !ctx.wants_keyboard_input() {
        chart.magnet = !chart.magnet;
    }

    // Replay mode keyboard controls
    if chart.replay_mode && !ctx.wants_keyboard_input() {
        // Space: toggle play/pause
        if ui.input(|i| i.key_pressed(egui::Key::Space)) {
            chart.replay_playing = !chart.replay_playing;
            if chart.replay_playing { chart.replay_last_step = None; }
        }
        // Right arrow: step forward 1 bar (only when paused)
        if !chart.replay_playing && ui.input(|i| i.key_pressed(egui::Key::ArrowRight)) {
            chart.replay_bar_count = (chart.replay_bar_count + 1).min(chart.bars.len());
            chart.indicator_bar_count = 0;
            chart.vs = (chart.replay_bar_count as f32 - chart.vc as f32 + CHART_RIGHT_PAD as f32).max(0.0);
        }
        // Left arrow: step back 1 bar (only when paused)
        if !chart.replay_playing && ui.input(|i| i.key_pressed(egui::Key::ArrowLeft)) {
            chart.replay_bar_count = chart.replay_bar_count.saturating_sub(1).max(1);
            chart.indicator_bar_count = 0;
            chart.vs = (chart.replay_bar_count as f32 - chart.vc as f32 + CHART_RIGHT_PAD as f32).max(0.0);
        }
        // Home: jump to start
        if ui.input(|i| i.key_pressed(egui::Key::Home)) {
            chart.replay_bar_count = 1;
            chart.replay_playing = false;
            chart.indicator_bar_count = 0;
            chart.vs = 0.0;
        }
        // End: jump to end (exit replay)
        if ui.input(|i| i.key_pressed(egui::Key::End)) {
            chart.replay_bar_count = chart.bars.len();
            chart.replay_playing = false;
            chart.indicator_bar_count = 0;
            chart.vs = (chart.replay_bar_count as f32 - chart.vc as f32 + CHART_RIGHT_PAD as f32).max(0.0);
        }
    }

    // ── Keyboard shortcuts for drawing tools ─────────────────────────────
    // Single-key activates tools instantly (only when no tool active and no text input)
    if !ctx.wants_keyboard_input() && chart.draw_tool.is_empty() {
        let new_tool: Option<&str> = ui.input(|i| {
            if i.key_pressed(egui::Key::T) { Some("trendline") }
            else if i.key_pressed(egui::Key::H) { Some("hline") }
            else if i.key_pressed(egui::Key::F) { Some("fibonacci") }
            else if i.key_pressed(egui::Key::C) && !i.modifiers.command { Some("channel") }
            else if i.key_pressed(egui::Key::V) && !i.modifiers.command { Some("vline") }
            else if i.key_pressed(egui::Key::R) { Some("ray") }
            // Z is now drag-zoom (handled separately), not hzone
            else if i.key_pressed(egui::Key::P) { Some("pitchfork") }
            else if i.key_pressed(egui::Key::G) { Some("gannfan") }
            else if i.key_pressed(egui::Key::X) { Some("fibext") }
            else if i.key_pressed(egui::Key::N) { Some("textnote") }
            else { None }
        });
        if let Some(tool) = new_tool {
            chart.draw_tool = tool.into();
            chart.pending_pt = None; chart.pending_pt2 = None; chart.pending_pts.clear();
        }
    }

    // ── Trading hotkeys ───────────────────────────────────────────────────
    if !ctx.wants_keyboard_input() {
        // Ctrl+B: Buy market at last price
        if ui.input(|i| i.modifiers.command && i.key_pressed(egui::Key::B) && !i.modifiers.shift) {
            use crate::chart_renderer::trading::order_manager::*;
            let price = chart.bars.last().map(|b| b.close).unwrap_or(0.0);
            let result = submit_order(OrderIntent {
                symbol: chart.symbol.clone(), side: OrderSide::Buy,
                order_type: ManagedOrderType::Market, price, qty: chart.order_qty,
                source: OrderSource::Hotkey, pair_with: None, option_symbol: None, option_con_id: None, stop_price: 0.0, trail_amount: None, trail_percent: None, last_price: 0.0, tif: 0, outside_rth: false,
            });
            if let OrderResult::Accepted(id) = result {
                chart.orders.push(OrderLevel { id: id as u32, side: OrderSide::Buy, price, qty: chart.order_qty, status: OrderStatus::Placed, pair_id: None, option_symbol: None, option_con_id: None, trail_amount: None, trail_percent: None });
            }
        }
        // Ctrl+Shift+B: Sell market at last price
        if ui.input(|i| i.modifiers.command && i.modifiers.shift && i.key_pressed(egui::Key::B)) {
            use crate::chart_renderer::trading::order_manager::*;
            let price = chart.bars.last().map(|b| b.close).unwrap_or(0.0);
            let result = submit_order(OrderIntent {
                symbol: chart.symbol.clone(), side: OrderSide::Sell,
                order_type: ManagedOrderType::Market, price, qty: chart.order_qty,
                source: OrderSource::Hotkey, pair_with: None, option_symbol: None, option_con_id: None, stop_price: 0.0, trail_amount: None, trail_percent: None, last_price: 0.0, tif: 0, outside_rth: false,
            });
            if let OrderResult::Accepted(id) = result {
                chart.orders.push(OrderLevel { id: id as u32, side: OrderSide::Sell, price, qty: chart.order_qty, status: OrderStatus::Placed, pair_id: None, option_symbol: None, option_con_id: None, trail_amount: None, trail_percent: None });
            }
        }
        // Ctrl+Shift+Q: Cancel all orders (local + IB)
        if ui.input(|i| i.modifiers.command && i.modifiers.shift && i.key_pressed(egui::Key::Q)) {
            crate::chart_renderer::trading::order_manager::cancel_all_orders("");
            chart.orders.clear();
            // Cancel all IB orders too
            std::thread::spawn(|| {
                let _ = reqwest::blocking::Client::new()
                    .delete(format!("{}/orders", APEXIB_URL))
                    .timeout(std::time::Duration::from_secs(5))
                    .send();
            });
        }
        // Ctrl+Shift+F: Flatten all positions (IB)
        if ui.input(|i| i.modifiers.command && i.modifiers.shift && i.key_pressed(egui::Key::F)) {
            crate::chart_renderer::trading::order_manager::cancel_all_orders("");
            chart.orders.retain(|o| o.status == OrderStatus::Executed);
            std::thread::spawn(|| {
                let _ = reqwest::blocking::Client::new()
                    .post(format!("{}/risk/flatten", APEXIB_URL))
                    .timeout(std::time::Duration::from_secs(5))
                    .send();
            });
        }
        // Ctrl+Shift+K: Kill Switch — cancel all orders + flatten all positions
        if ui.input(|i| i.modifiers.command && i.modifiers.shift && i.key_pressed(egui::Key::K)) {
            crate::chart_renderer::trading::order_manager::kill_switch();
            chart.orders.clear();
            PENDING_TOASTS.with(|ts| ts.borrow_mut().push(("KILL SWITCH ACTIVATED".into(), 0.0, false)));
        }
        // Ctrl+Shift+H: Halt trading
        if ui.input(|i| i.modifiers.command && i.modifiers.shift && i.key_pressed(egui::Key::H)) {
            crate::chart_renderer::trading::order_manager::halt_trading();
            PENDING_TOASTS.with(|ts| ts.borrow_mut().push(("Trading HALTED".into(), 0.0, false)));
        }
        // Ctrl+Shift+R: Resume trading
        if ui.input(|i| i.modifiers.command && i.modifiers.shift && i.key_pressed(egui::Key::R)) {
            crate::chart_renderer::trading::order_manager::resume_trading();
            PENDING_TOASTS.with(|ts| ts.borrow_mut().push(("Trading RESUMED".into(), 0.0, true)));
        }
    }

    // ── Replay control bar — bottom of chart pane ──
    if chart.replay_mode && !chart.bars.is_empty() {
        use crate::ui_kit::icons::Icon;
        let replay_h = 28.0_f32;
        let replay_rect = egui::Rect::from_min_size(
            egui::pos2(rect.left(), rect.top() + pt + ch - replay_h),
            egui::vec2(cw, replay_h),
        );
        ui.painter().rect_filled(replay_rect, 0.0, color_alpha(t.toolbar_bg, 230));
        ui.painter().line_segment(
            [egui::pos2(replay_rect.left(), replay_rect.top()), egui::pos2(replay_rect.right(), replay_rect.top())],
            egui::Stroke::new(1.0, color_alpha(t.toolbar_border, 120)),
        );

        let total_bars = chart.bars.len();
        let cur_bar = chart.replay_bar_count;
        let btn_size = egui::vec2(22.0, 20.0);
        let btn_y = replay_rect.top() + (replay_h - btn_size.y) * 0.5;
        let mut cx = replay_rect.left() + 8.0;

        let mut replay_btn = |ui: &mut egui::Ui, cx: &mut f32, icon: &str, tooltip: &str| -> bool {
            let r = egui::Rect::from_min_size(egui::pos2(*cx, btn_y), btn_size);
            *cx += btn_size.x + 2.0;
            let resp = ui.allocate_rect(r, egui::Sense::click());
            let hovered = resp.hovered();
            let col = if hovered { t.text } else { t.dim.gamma_multiply(0.8) };
            if hovered {
                ui.painter().rect_filled(r, 3.0, color_alpha(t.toolbar_border, ALPHA_DIM));
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            ui.painter().text(r.center(), egui::Align2::CENTER_CENTER,
                icon, egui::FontId::proportional(12.0), col);
            if !tooltip.is_empty() { resp.clone().on_hover_text(tooltip); }
            resp.clicked()
        };

        if replay_btn(ui, &mut cx, Icon::SKIP_BACK, "Jump to start (Home)") {
            chart.replay_bar_count = 1;
            chart.replay_playing = false;
            chart.indicator_bar_count = 0;
            chart.vs = 0.0;
        }
        if replay_btn(ui, &mut cx, Icon::CARET_LEFT, "Step back (Left)") {
            chart.replay_bar_count = chart.replay_bar_count.saturating_sub(1).max(1);
            chart.indicator_bar_count = 0;
            chart.vs = (chart.replay_bar_count as f32 - chart.vc as f32 + CHART_RIGHT_PAD as f32).max(0.0);
        }
        let play_icon = if chart.replay_playing { Icon::PAUSE } else { Icon::PLAY };
        let play_tip = if chart.replay_playing { "Pause (Space)" } else { "Play (Space)" };
        if replay_btn(ui, &mut cx, play_icon, play_tip) {
            chart.replay_playing = !chart.replay_playing;
            if chart.replay_playing { chart.replay_last_step = None; }
        }
        if replay_btn(ui, &mut cx, Icon::CARET_RIGHT, "Step forward (Right)") {
            chart.replay_bar_count = (chart.replay_bar_count + 1).min(total_bars);
            chart.indicator_bar_count = 0;
            chart.vs = (chart.replay_bar_count as f32 - chart.vc as f32 + CHART_RIGHT_PAD as f32).max(0.0);
        }
        if replay_btn(ui, &mut cx, Icon::SKIP_FORWARD, "Jump to end (End)") {
            chart.replay_bar_count = total_bars;
            chart.replay_playing = false;
            chart.indicator_bar_count = 0;
            chart.vs = (chart.replay_bar_count as f32 - chart.vc as f32 + CHART_RIGHT_PAD as f32).max(0.0);
        }

        cx += 6.0;
        let speeds: &[(f32, &str)] = &[(0.5, "0.5x"), (1.0, "1x"), (2.0, "2x"), (5.0, "5x"), (10.0, "10x")];
        for &(spd, label) in speeds {
            let lw = label.len() as f32 * 6.0 + 8.0;
            let sr = egui::Rect::from_min_size(egui::pos2(cx, btn_y), egui::vec2(lw, btn_size.y));
            cx += lw + 2.0;
            let resp = ui.allocate_rect(sr, egui::Sense::click());
            let is_active = (chart.replay_speed - spd).abs() < 0.01;
            let col = if is_active { t.bull } else if resp.hovered() { t.dim } else { t.dim.gamma_multiply(0.6) };
            if resp.hovered() {
                ui.painter().rect_filled(sr, 3.0, color_alpha(t.toolbar_border, 40));
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            if is_active {
                ui.painter().rect_filled(sr, 3.0, color_alpha(t.bull, 30));
            }
            ui.painter().text(sr.center(), egui::Align2::CENTER_CENTER,
                label, egui::FontId::monospace(9.0), col);
            if resp.clicked() {
                chart.replay_speed = spd;
                chart.replay_last_step = None;
            }
        }

        cx += 8.0;
        let counter_text = format!("Bar {} / {}", cur_bar, total_bars);
        ui.painter().text(
            egui::pos2(cx, replay_rect.center().y),
            egui::Align2::LEFT_CENTER,
            &counter_text,
            egui::FontId::monospace(9.5),
            t.dim.gamma_multiply(0.9),
        );
        cx += counter_text.len() as f32 * 6.0 + 16.0;

        let progress_w = (replay_rect.right() - cx - 12.0).max(40.0);
        let progress_h = 6.0_f32;
        let progress_rect = egui::Rect::from_min_size(
            egui::pos2(cx, replay_rect.center().y - progress_h * 0.5),
            egui::vec2(progress_w, progress_h),
        );
        ui.painter().rect_filled(progress_rect, 3.0, color_alpha(t.dim, 40));
        let frac_done = if total_bars > 0 { cur_bar as f32 / total_bars as f32 } else { 0.0 };
        let filled_w = progress_w * frac_done;
        if filled_w > 0.5 {
            let filled_rect = egui::Rect::from_min_size(
                progress_rect.min,
                egui::vec2(filled_w, progress_h),
            );
            ui.painter().rect_filled(filled_rect, 3.0, color_alpha(t.bull, 180));
        }
        let dot_x = progress_rect.left() + filled_w;
        ui.painter().circle_filled(
            egui::pos2(dot_x, progress_rect.center().y),
            4.0,
            t.text,
        );
        let progress_sense_rect = progress_rect.expand2(egui::vec2(0.0, 6.0));
        let prog_resp = ui.allocate_rect(progress_sense_rect, egui::Sense::click_and_drag());
        if prog_resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        if prog_resp.clicked() || prog_resp.dragged() {
            if let Some(pos) = ui.input(|i| i.pointer.hover_pos()) {
                let frac = ((pos.x - progress_rect.left()) / progress_w).clamp(0.0, 1.0);
                let new_bar = ((frac * total_bars as f32) as usize).max(1).min(total_bars);
                chart.replay_bar_count = new_bar;
                chart.indicator_bar_count = 0;
                chart.vs = (chart.replay_bar_count as f32 - chart.vc as f32 + CHART_RIGHT_PAD as f32).max(0.0);
            }
        }
    }

    // ── Restore button (when pane is maximized) — drawn last so it's on top ──
    if watchlist.maximized_pane.is_some() {
        let btn_w = 28.0;
        let btn_h = 22.0;
        let btn_rect = egui::Rect::from_min_size(
            egui::pos2(pane_rect.right() - btn_w - 8.0, pane_rect.top() + 4.0),
            egui::vec2(btn_w, btn_h));
        // Background pill
        ui.painter().rect_filled(btn_rect, 4.0, color_alpha(t.toolbar_bg, 230));
        ui.painter().rect_stroke(btn_rect, 4.0, egui::Stroke::new(STROKE_STD, color_alpha(t.toolbar_border, ALPHA_ACTIVE)), egui::StrokeKind::Outside);
        // Restore icon — overlapping squares
        let c = btn_rect.center();
        let s = 4.0;
        let icon_col = t.dim.gamma_multiply(0.7);
        ui.painter().rect_stroke(egui::Rect::from_min_size(egui::pos2(c.x - s + 1.5, c.y - s - 0.5), egui::vec2(s * 1.5, s * 1.5)), 1.0, egui::Stroke::new(1.2, icon_col), egui::StrokeKind::Outside);
        ui.painter().rect_stroke(egui::Rect::from_min_size(egui::pos2(c.x - s - 0.5, c.y - s + 1.5), egui::vec2(s * 1.5, s * 1.5)), 1.0, egui::Stroke::new(1.2, icon_col), egui::StrokeKind::Outside);
        // Click detection
        let btn_resp = ui.allocate_rect(btn_rect, egui::Sense::click());
        if btn_resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            ui.painter().rect_filled(btn_rect, 4.0, color_alpha(t.toolbar_border, ALPHA_DIM));
            // Redraw icon brighter on hover
            let hc = t.dim;
            ui.painter().rect_stroke(egui::Rect::from_min_size(egui::pos2(c.x - s + 1.5, c.y - s - 0.5), egui::vec2(s * 1.5, s * 1.5)), 1.0, egui::Stroke::new(1.2, hc), egui::StrokeKind::Outside);
            ui.painter().rect_stroke(egui::Rect::from_min_size(egui::pos2(c.x - s - 0.5, c.y - s + 1.5), egui::vec2(s * 1.5, s * 1.5)), 1.0, egui::Stroke::new(1.2, hc), egui::StrokeKind::Outside);
        }
        if btn_resp.clicked() { watchlist.maximized_pane = None; }
    }

    // ── Drawing-tool picker (opened by 2nd middle-click) ────────────────────
    if chart.draw_picker_open {
        let pos = chart.draw_picker_pos;
        let picker_out = crate::chart_renderer::ui::widgets::drawing::show_drawing_tool_picker(
            ctx, t, chart, watchlist, pane_idx, pos,
        );
        if let Some(tool) = picker_out.chosen { apply_draw_tool(&tool, chart); }
        if let Some(tool) = picker_out.star_toggle {
            if let Some(p) = watchlist.draw_favorites.iter().position(|f| f == &tool) {
                watchlist.draw_favorites.remove(p);
            } else {
                watchlist.draw_favorites.push(tool);
            }
        }
        if picker_out.close { chart.draw_picker_open = false; }
    }

    span_end(); // interaction
}

/// Phosphor icon glyph for a draw-tool name.
fn drawing_icon(tool: &str) -> &'static str {
    use crate::ui_kit::icons::Icon;
    match tool {
        "trendline"          => Icon::LINE_SEGMENT,
        "hline"              => Icon::MINUS,
        "vline"              => Icon::DOTS_SIX_VERTICAL,
        "hzone"              => Icon::RECTANGLE,
        "ray"                => Icon::ARROW_FAT_UP,
        "channel"            => Icon::GIT_DIFF,
        "fibonacci"          => Icon::CHART_LINE,
        "fibext"             => Icon::CHART_LINE,
        "fibarc"             => Icon::CIRCLE,
        "fibchannel"         => Icon::GIT_DIFF,
        "fibtimezone"        => Icon::LIST,
        "pitchfork"          => Icon::GIT_DIFF,
        "gannbox"            => Icon::SQUARE,
        "gannfan"            => Icon::SPARKLE,
        "regression"         => Icon::PULSE,
        "avwap"              => Icon::CHART_LINE,
        "pricerange"         => Icon::ARROWS_OUT,
        "riskreward"         => Icon::CHART_BAR,
        "barmarker"          => Icon::MAP_PIN,
        "xabcd"              => Icon::CHART_LINE,
        "elliott_corrective" => Icon::CHART_LINE,
        "elliott_sub_corrective" => Icon::CHART_LINE,
        "elliott_wxyxz"      => Icon::CHART_LINE,
        "magnifier"          => Icon::MAGNIFYING_GLASS_PLUS,
        "measure"            => Icon::RULER,
        _                    => Icon::PENCIL_LINE,
    }
}

/// Display label for a draw-tool name.
fn drawing_label(tool: &str) -> &'static str {
    match tool {
        "trendline" => "Trend Line",
        "hline" => "Horizontal",
        "vline" => "Vertical",
        "hzone" => "H-Zone",
        "ray" => "Ray",
        "channel" => "Channel",
        "fibonacci" => "Fibonacci",
        "fibext" => "Fib Extension",
        "fibarc" => "Fib Arc",
        "fibchannel" => "Fib Channel",
        "fibtimezone" => "Fib Time Zone",
        "pitchfork" => "Pitchfork",
        "gannbox" => "Gann Box",
        "gannfan" => "Gann Fan",
        "regression" => "Regression",
        "avwap" => "Anchored VWAP",
        "pricerange" => "Price Range",
        "riskreward" => "Risk/Reward",
        "barmarker" => "Bar Marker",
        "xabcd" => "XABCD",
        "elliott_corrective" => "Elliott Corrective",
        "elliott_sub_corrective" => "Elliott Sub-Corr.",
        "elliott_wxyxz" => "Elliott WXYXZ",
        "magnifier" => "Magnifier (zoom)",
        "measure" => "Measure",
        _ => "Tool",
    }
}

/// Returns true when a "permanent" (toggle) tool is currently active for `tool`.
fn drawing_is_active(tool: &str, chart: &Chart) -> bool {
    match tool {
        "magnifier" => chart.zoom_selecting,
        "measure" => chart.measure_active,
        _ => false,
    }
}

/// Apply a draw-tool selection. Drawing tools set `draw_tool`; the two
/// "permanent" toggles flip their respective booleans.
fn apply_draw_tool(tool: &str, chart: &mut Chart) {
    chart.pending_pt = None; chart.pending_pt2 = None; chart.pending_pts.clear();
    match tool {
        "magnifier" => {
            chart.zoom_selecting = !chart.zoom_selecting;
            chart.draw_tool.clear();
        }
        "measure" => {
            chart.measure_active = !chart.measure_active;
            chart.draw_tool.clear();
        }
        _ => { chart.draw_tool = tool.to_string(); }
    }
}

/// Categorized draw-tool list for the picker's ALL TOOLS section.
const DRAW_CATEGORIES: &[(&str, &[(&str, &str)])] = &[
    ("LINES", &[
        ("trendline", "Trend Line"),
        ("hline", "Horizontal"),
        ("vline", "Vertical"),
        ("ray", "Ray"),
        ("channel", "Channel"),
        ("regression", "Regression"),
    ]),
    ("ZONES", &[
        ("hzone", "Horizontal Zone"),
        ("pricerange", "Price Range"),
    ]),
    ("FIBONACCI", &[
        ("fibonacci", "Retracement"),
        ("fibext", "Extension"),
        ("fibarc", "Arc"),
        ("fibchannel", "Channel"),
        ("fibtimezone", "Time Zone"),
    ]),
    ("GANN / PITCHFORK", &[
        ("pitchfork", "Pitchfork"),
        ("gannbox", "Gann Box"),
        ("gannfan", "Gann Fan"),
    ]),
    ("HARMONIC", &[
        ("xabcd", "XABCD"),
        ("elliott_corrective", "Elliott Corrective"),
        ("elliott_sub_corrective", "Elliott Sub-Corrective"),
        ("elliott_wxyxz", "Elliott WXYXZ"),
    ]),
    ("UTILITY", &[
        ("magnifier", "Magnifier (zoom)"),
        ("measure", "Measure"),
        ("avwap", "Anchored VWAP"),
        ("riskreward", "Risk / Reward"),
        ("barmarker", "Bar Marker"),
    ]),
];

/// Phase 8: Handle deferred actions (option chart open, underlying orders, repaint).
fn handle_deferred(
    ctx: &egui::Context,
    panes: &mut Vec<Chart>,
    active_pane: &mut usize,
    layout: &mut Layout,
    watchlist: &mut Watchlist,
) {
    // ── Handle deferred option chart open ──
    // Replaces the CURRENT (active) pane with the option chart
    if let Some((sym, strike, is_call, expiry)) = watchlist.pending_opt_chart.take() {
        let ap = *active_pane;
        let raw_occ = watchlist.pending_opt_chart_contract.take().unwrap_or_default();
        crate::apex_log!("option.click", "sym={sym} strike={strike} is_call={is_call} expiry='{expiry}' raw_occ='{raw_occ}'");
        let occ = if raw_occ.starts_with("O:") {
            raw_occ
        } else {
            let o = synthesize_occ(&sym, strike, is_call, &expiry);
            crate::apex_log!("option.occ", "synthesized OCC: {o}");
            o
        };
        let strike_str = if (strike - strike.round()).abs() < 0.005 { format!("{:.0}", strike) } else { format!("{:.1}", strike) };
        let opt_sym = format!("{} {}{} {}", sym, strike_str, if is_call { "C" } else { "P" }, expiry);
        crate::apex_log!("option.open", "occ={occ} display_sym='{opt_sym}'");
        // Always open the contract in the active pane. The user expects clicks
        // on the chain to land where they're focused, not in some other pane.
        let target = ap.min(panes.len().saturating_sub(1));
        panes[target].symbol = opt_sym.clone();
        panes[target].is_option = true;
        panes[target].underlying = sym.clone();
        panes[target].option_type = if is_call { "C".into() } else { "P".into() };
        panes[target].option_strike = strike;
        panes[target].option_expiry = expiry;
        panes[target].option_contract = occ.clone();

        let tf = panes[target].timeframe.clone();

        // Clear bars — we only want real data. The fetcher will populate via
        // ChartCommand::LoadBars on success and subscribe the WS for live ticks.
        panes[target].bars.clear();
        panes[target].timestamps.clear();

        if occ.is_empty() {
            eprintln!("[option-chart] No OCC contract ticker — cannot fetch bars for {}", opt_sym);
        } else if !crate::apex_data::is_enabled() {
            eprintln!("[option-chart] ApexData disabled — cannot fetch bars for {}", occ);
        } else {
            fetch_option_bars_background(occ.clone(), opt_sym, tf.clone(), panes[target].bar_source_mark);
        }
        panes[target].vs = (panes[target].bars.len() as f32 - panes[target].vc as f32 + CHART_RIGHT_PAD as f32).max(0.0);
        panes[target].auto_scroll = true;
        panes[target].indicator_bar_count = 0;
        *active_pane = target;
    }

    // ── Handle deferred underlying order actions ──
    // Check if any option pane requested to place an order on its underlying
    let mut und_action: Option<(usize, OrderSide, String, String, f32, String, u32)> = None;
    for (pi, pane) in panes.iter_mut().enumerate() {
        if let Some(side) = pane.pending_und_order.take() {
            und_action = Some((pi, side, pane.underlying.clone(), pane.option_type.clone(), pane.option_strike, pane.option_expiry.clone(), pane.order_qty));
        }
    }
    if let Some((source_pi, side, underlying, opt_type, strike, expiry, qty)) = und_action {
        let opt_sym = panes[source_pi].symbol.clone();
        let source_sym = panes[source_pi].symbol.clone();
        let tf = panes[0].timeframe.clone();
        let theme = panes[0].theme_idx;

        // Find or create the underlying pane
        let und_pane = panes.iter().position(|p| p.symbol == underlying && !p.is_option);
        let target_pi = if let Some(pi) = und_pane {
            pi
        } else if panes.len() <= 1 {
            *layout = Layout::TwoH;
            let mut p = Chart::new_with(&underlying, &tf);
            p.theme_idx = theme;
            p.pending_symbol_change = Some(underlying.clone());
            panes.push(p);
            panes.len() - 1
        } else {
            let other = panes.iter().position(|p| !p.is_option && p.symbol != source_sym);
            let pi = other.unwrap_or((source_pi + 1) % panes.len());
            panes[pi].pending_symbol_change = Some(underlying.clone());
            panes[pi].is_option = false;
            pi
        };

        // Place a draft order level on the underlying pane — same as regular orders
        let last = panes[target_pi].bars.last().map(|b| b.close).unwrap_or(0.0);
        {
            use crate::chart_renderer::trading::order_manager::*;
            let result = submit_order(OrderIntent {
                symbol: panes[target_pi].symbol.clone(), side,
                order_type: ManagedOrderType::Limit, price: last, qty,
                source: OrderSource::Trigger, pair_with: None,
                option_symbol: Some(opt_sym.clone()), option_con_id: None, stop_price: 0.0, trail_amount: None, trail_percent: None, last_price: 0.0, tif: 0, outside_rth: false,
            });
            match result {
                OrderResult::Accepted(id) => {
                    panes[target_pi].orders.push(OrderLevel {
                        id: id as u32, side, price: last, qty, status: OrderStatus::Placed, pair_id: None,
                        option_symbol: Some(opt_sym), option_con_id: None, trail_amount: None, trail_percent: None,
                    });
                }
                OrderResult::NeedsConfirmation(id) => {
                    panes[target_pi].orders.push(OrderLevel {
                        id: id as u32, side, price: last, qty, status: OrderStatus::Draft, pair_id: None,
                        option_symbol: Some(opt_sym), option_con_id: None, trail_amount: None, trail_percent: None,
                    });
                    panes[target_pi].pending_confirms.push((id as u32, std::time::Instant::now()));
                }
                _ => {}
            }
        }
        *active_pane = target_pi;
    }

    ctx.request_repaint();
}

pub(crate) fn draw_chart(ctx: &egui::Context, panes: &mut Vec<Chart>, active_pane: &mut usize, layout: &mut Layout, watchlist: &mut Watchlist, toasts: &[(String, f32, std::time::Instant, bool)], conn_panel_open: &mut bool, rx: &mpsc::Receiver<ChartCommand>) {
    use crate::monitoring::{span_begin, span_end};

    // Publish the active style id for `style::current()` so widget primitives
    // can pick the right corners / borders / serifs / button treatment.
    crate::chart_renderer::ui::style::set_active_style(style_id(watchlist));

    // ── Watchlist divider drag (handled at top level to avoid panel interference) ──
    if watchlist.divider_y > 0.0 && watchlist.options_visible {
        let pointer_pos = ctx.input(|i| i.pointer.latest_pos());
        let primary_pressed = ctx.input(|i| i.pointer.primary_pressed());
        let primary_down = ctx.input(|i| i.pointer.primary_down());
        let primary_released = ctx.input(|i| i.pointer.primary_released());

        // Start drag on press near divider
        if primary_pressed {
            if let Some(pos) = pointer_pos {
                if (pos.y - watchlist.divider_y).abs() < 10.0 {
                    watchlist.divider_dragging = true;
                }
            }
        }
        // During drag, compute split from absolute Y position
        if watchlist.divider_dragging && primary_down {
            if let Some(pos) = pointer_pos {
                ctx.set_cursor_icon(egui::CursorIcon::ResizeVertical);
                // divider_y is the absolute screen Y of the divider
                // divider_total_h is the total height available for stocks+options
                // The stocks area starts at divider_y - stocks_h and ends at divider_y
                // We want: new divider_y = pos.y, solve for split
                let stocks_start_y = watchlist.divider_y - watchlist.divider_total_h * watchlist.options_split;
                let new_split = (pos.y - stocks_start_y) / watchlist.divider_total_h;
                watchlist.options_split = new_split.clamp(0.15, 0.85);
            }
        }
        // End drag
        if primary_released && watchlist.divider_dragging {
            watchlist.divider_dragging = false;
        }
    }

    // Window drag is now handled directly inside the toolbar panel via
    // ui.interact(..., Sense::click_and_drag()) — see earlier in this file.

    // Clear TB_BTN_CLICKED for next frame — MUST be after the drag handler above reads it
    TB_BTN_CLICKED.with(|f| f.set(false));

    route_commands(rx, panes, active_pane, watchlist);

    // Keep ApexData's snapshot poller's watched set synced with visible symbols.
    // Union: active pane symbols + every item in the watchlist.
    if crate::apex_data::is_enabled() {
        // For watchlist items that are options, the feed key is the OCC ticker
        // synthesized from underlying/strike/side/expiry — NOT the display label.
        let item_feed_key = |it: &WatchlistItem| -> String {
            if it.is_option && !it.underlying.is_empty() {
                synthesize_occ(&it.underlying, it.strike, it.option_type == "C", &it.expiry)
            } else {
                it.symbol.clone()
            }
        };

        let mut watched: std::collections::HashSet<String> = panes.iter()
            .map(|p| {
                // Option panes feed via the OCC ticker, not the display label.
                if p.is_option && !p.option_contract.is_empty() { p.option_contract.clone() }
                else { p.symbol.clone() }
            })
            .filter(|s| !s.is_empty() && !crate::data::is_crypto(s))
            .collect();
        for sec in &watchlist.sections {
            for it in &sec.items {
                let key = item_feed_key(it);
                if !key.is_empty() && !crate::data::is_crypto(&key) {
                    watched.insert(key);
                }
            }
        }
        let watched_list: Vec<String> = watched.iter().cloned().collect();
        crate::apex_data::live_state::set_watched_symbols(watched_list.clone());

        // Push snapshot data into watchlist items so rows render live prices.
        for sym in &watched_list {
            if let Some(snap) = crate::apex_data::live_state::get_snapshot(sym) {
                watchlist.set_price(sym, snap.last as f32);
                watchlist.set_prev_close(sym, snap.day_open as f32);
            }
        }

        // Push live NBBO quotes into watchlist items (option rows look up via
        // their synthesized OCC; equity rows by symbol).
        for sec in &mut watchlist.sections {
            for it in &mut sec.items {
                if it.symbol.is_empty() { continue; }
                let key = item_feed_key(it);
                if let Some(q) = crate::apex_data::live_state::get_quote(&key) {
                    it.bid = q.bid as f32;
                    it.ask = q.ask as f32;
                }
                // Pull last price from snapshots into the option row's `price`
                // field so the watchlist mark column updates from the feed.
                if it.is_option {
                    if let Some(snap) = crate::apex_data::live_state::get_snapshot(&key) {
                        if snap.last > 0.0 { it.price = snap.last as f32; }
                    }
                }
            }
        }

        // Chain-delta cache → watchlist.chain_0dte/chain_far refresh.
        // chain_delta arrives every 5s; re-derive the displayed grids from the
        // local cache (spec §5.4.d bootstrap pattern: REST once, WS merge forever).
        {
            let sym = watchlist.chain_symbol.clone();
            if !sym.is_empty() {
                // If cache is empty for the selected chain symbol, kick off a fetch
                // (debounced via chain_last_fetch). Covers SPX/SPXW/NDX/etc. that
                // weren't pre-fetched at startup.
                let cached = crate::apex_data::live_state::get_chain(&sym);
                if cached.is_empty() {
                    let stale = watchlist.chain_last_fetch
                        .map(|t| t.elapsed() > std::time::Duration::from_secs(3))
                        .unwrap_or(true);
                    if stale {
                        watchlist.chain_last_fetch = Some(std::time::Instant::now());
                        watchlist.chain_loading = true;
                        let dte = 0;
                        let hint = crate::apex_data::live_state::get_snapshot(&sym)
                            .map(|s| s.last as f32).unwrap_or(0.0);
                        crate::apex_log!("chain.refresh", "{}: cache empty — kicking fetch", sym);
                        fetch_chain_background(sym.clone(), watchlist.chain_num_strikes, dte, hint);
                    }
                }
                if !cached.is_empty() {
                    // Pass num_strikes=0 (sentinel = no trim) so the watchlist UI
                    // gets every strike for the chosen expiry. The render_block
                    // then handles its own windowing (count / pct / sigma) and the
                    // prev/next-strike buttons can walk the full ladder.
                    let ns = 0usize;
                    let hint = if watchlist.chain_underlying_price > 0.0 {
                        watchlist.chain_underlying_price
                    } else {
                        crate::apex_data::live_state::get_snapshot(&sym)
                            .map(|s| s.last as f32).unwrap_or(0.0)
                    };
                    let far_dte = watchlist.chain_far_dte;
                    // 0DTE expiry rule: pick the most recent trading day whose options
                    // are "active" — today during/after pre-market on a weekday, else
                    // the previous trading day (Sat→Fri, Sun→Fri, weekday<4amET→prev).
                    let zero_dte_str = active_zero_dte_date().format("%Y-%m-%d").to_string();
                    let cached_today: Vec<_> = cached.iter()
                        .filter(|r| r.expiry == zero_dte_str).cloned().collect();
                    let (c0, p0, spot0) = apex_data_chain_to_tuples(&cached_today, 0, ns, hint);
                    let (cf, pf, _)     = apex_data_chain_to_tuples(&cached, far_dte, ns, hint);
                    let to_rows = |tuples: Vec<(f32,f32,f32,f32,i32,i32,f32,bool,String)>| -> Vec<OptionRow> {
                        tuples.into_iter().map(|(strike,last,bid,ask,vol,oi,iv,itm,contract)| OptionRow {
                            strike, last, bid, ask, volume: vol, oi, iv, itm, contract,
                        }).collect()
                    };
                    watchlist.chain_0dte = (to_rows(c0), to_rows(p0));
                    watchlist.chain_far  = (to_rows(cf), to_rows(pf));
                    // Throttled log: emit only when the summary changes or
                    // ≥5s elapsed. Was firing every frame at 60+ Hz.
                    {
                        use std::sync::{OnceLock, Mutex};
                        static LAST: OnceLock<Mutex<(String, std::time::Instant)>> = OnceLock::new();
                        let m = LAST.get_or_init(|| Mutex::new((String::new(), std::time::Instant::now() - std::time::Duration::from_secs(60))));
                        let key = format!("{}:{}c/{}p:{}c/{}p:{:.2}:{}",
                            sym,
                            watchlist.chain_0dte.0.len(), watchlist.chain_0dte.1.len(),
                            watchlist.chain_far.0.len(), watchlist.chain_far.1.len(),
                            spot0, cached.len());
                        let mut g = m.lock().unwrap();
                        if g.0 != key || g.1.elapsed() >= std::time::Duration::from_secs(5) {
                            crate::apex_log!("chain.refresh",
                                "{}: 0DTE={}c/{}p, far(dte={})={}c/{}p, spot={:.2}, cache={} rows",
                                sym, watchlist.chain_0dte.0.len(), watchlist.chain_0dte.1.len(),
                                far_dte, watchlist.chain_far.0.len(), watchlist.chain_far.1.len(),
                                spot0, cached.len());
                            g.0 = key; g.1 = std::time::Instant::now();
                        }
                    }
                    if spot0 > 0.0 { watchlist.chain_underlying_price = spot0; }
                    watchlist.chain_loading = false;
                }
            }
        }

        // Register contracts for greeks polling. Three sources:
        //   • Each pane's own option_contract (so an open 285C polls its greeks).
        //   • ATM 0DTE call per pane (drives the greeks ribbon for the underlying).
        //   • Spread builder legs when the panel is open.
        // The OptionGreeks widget + spread/chain tabs read via
        // live_state::get_greeks(contract).
        let mut contracts: std::collections::HashSet<String> = std::collections::HashSet::new();
        for p in panes.iter() {
            if p.is_option && p.option_contract.starts_with("O:") {
                contracts.insert(p.option_contract.clone());
            }
            if p.symbol.is_empty() { continue; }
            let spot = watchlist.chain_underlying_price;
            let calls = if !watchlist.chain_0dte.0.is_empty() { &watchlist.chain_0dte.0 }
                        else { &watchlist.chain_far.0 };
            if let Some(atm) = calls.iter()
                .min_by(|a, b| (a.strike - spot).abs()
                    .partial_cmp(&((b.strike - spot).abs()))
                    .unwrap_or(std::cmp::Ordering::Equal))
            {
                if !atm.contract.is_empty() && atm.contract.starts_with("O:") {
                    contracts.insert(atm.contract.clone());
                }
            }
        }
        // Watchlist option items — poll greeks too so the rows can show greek
        // columns when the user toggles them.
        for sec in &watchlist.sections {
            for it in &sec.items {
                if it.is_option && !it.underlying.is_empty() {
                    let occ = synthesize_occ(
                        &it.underlying, it.strike, it.option_type == "C", &it.expiry);
                    if occ.starts_with("O:") { contracts.insert(occ); }
                }
            }
        }
        // Spread builder legs.
        if watchlist.spread_open {
            let underlying = if panes[*active_pane].is_option {
                panes[*active_pane].underlying.clone()
            } else { panes[*active_pane].symbol.clone() };
            if !underlying.is_empty() && !crate::data::is_crypto(&underlying) {
                for leg in &watchlist.spread_state.legs {
                    let occ = synthesize_occ(
                        &underlying, leg.strike, leg.option_type == "CALL", &leg.expiry);
                    if occ.starts_with("O:") { contracts.insert(occ); }
                }
            }
        }
        crate::apex_data::live_state::set_watched_contracts(contracts);

        // Tape & quote WS subscriptions follow the open panels. Tape panel
        // streams trades for the active pane — for option panes we use the
        // OCC ticker, NOT the display label, so the server actually streams.
        let tape_syms: Vec<String> = if watchlist.tape_open {
            let ap = &panes[*active_pane];
            let key = if ap.is_option && !ap.option_contract.is_empty() {
                ap.option_contract.clone()
            } else { ap.symbol.clone() };
            if key.is_empty() || crate::data::is_crypto(&key) { vec![] } else { vec![key] }
        } else { vec![] };
        crate::apex_data::ws::set_tape(&tape_syms);

        // Quote subscriptions: every watched item (already includes option OCCs
        // from the watched_list build above) PLUS every open option pane's OCC.
        // Drives DOM sidebar, order entry NBBO, ladder mid-price, options
        // overlay along the price axis, and any FMV piggyback (§6.1).
        let mut quote_set: std::collections::HashSet<String> = watched_list.iter()
            .filter(|s| !crate::data::is_crypto(s))
            .cloned().collect();
        for p in panes.iter() {
            if p.is_option && !p.option_contract.is_empty() {
                quote_set.insert(p.option_contract.clone());
            }
            // Strikes overlay (the "O" / circle toggle) renders pills for each
            // overlay_calls/overlay_puts row — subscribe their OCCs so each
            // pill's bid/ask/IV is live, not stale from the seed fetch.
            if p.show_strikes_overlay {
                for r in p.overlay_calls.iter().chain(p.overlay_puts.iter()) {
                    if r.contract.starts_with("O:") {
                        quote_set.insert(r.contract.clone());
                    }
                }
            }
            // Floating order tickets / DOM sidebar / ladder all read live data
            // for the pane's option contract; already covered by `option_contract`
            // above.
        }
        // Spread builder legs — when the panel is open, every leg's OCC needs
        // a live quote so the BUY/SELL prices and net debit/credit are real.
        if watchlist.spread_open {
            let underlying = panes[*active_pane].symbol.clone();
            let underlying = if panes[*active_pane].is_option {
                panes[*active_pane].underlying.clone()
            } else { underlying };
            if !underlying.is_empty() && !crate::data::is_crypto(&underlying) {
                for leg in &watchlist.spread_state.legs {
                    let occ = synthesize_occ(
                        &underlying, leg.strike, leg.option_type == "CALL", &leg.expiry,
                    );
                    if occ.starts_with("O:") { quote_set.insert(occ); }
                }
            }
        }
        let quote_syms: Vec<String> = quote_set.into_iter().collect();
        crate::apex_data::ws::set_quotes(&quote_syms);
    }

    check_history_pagination(panes, *active_pane);

    update_simulation(panes);

    let (theme_idx, account_data_cached, win_ref) = setup_theme(ctx, panes, *active_pane, watchlist);
    let _t_owned = get_theme(theme_idx);
    let t = &_t_owned;

    render_toolbar(ctx, panes, active_pane, layout, watchlist, t, theme_idx, &account_data_cached, win_ref, conn_panel_open, toasts);

    span_begin("chart_panes");
    CROSSHAIR_SYNC_TIME.with(|t| t.set(0));


    egui::CentralPanel::default().frame(egui::Frame::NONE.fill(t.bg)).show(ctx, |ui| {
        let full_rect = ui.available_rect_before_wrap();
        let actual_count = layout.max_panes().min(panes.len());
        let (visible_count, pane_rects) = if let Some(max_idx) = watchlist.maximized_pane {
            if max_idx < actual_count {
                // Maximized: show only one pane fullscreen
                (1, vec![full_rect])
            } else {
                watchlist.maximized_pane = None;
                (actual_count, layout.pane_rects(full_rect, actual_count, watchlist.pane_split_h, watchlist.pane_split_v, watchlist.pane_split_h2, watchlist.pane_split_v2))
            }
        } else {
            (actual_count, layout.pane_rects(full_rect, actual_count, watchlist.pane_split_h, watchlist.pane_split_v, watchlist.pane_split_h2, watchlist.pane_split_v2))
        };

        // Compute max pane header height (tabs make headers taller)
        let any_pane_has_tabs = panes.iter().take(visible_count).any(|c| c.tab_symbols.len() > 1);
        let max_header_h = if any_pane_has_tabs { 28.0_f32 } else if visible_count > 1 { 18.0_f32 } else { 0.0_f32 };

        // ── Pane gap fill (gutter color between panes) ────────────────────────
        // pane_gap_alpha > 0 paints the full_rect underneath all pane tiles so
        // the gap/gutter area shows a distinct tint rather than raw background.
        if visible_count > 1 {
            let st_gap = crate::chart_renderer::ui::style::current();
            if st_gap.pane_gap_alpha > 0 || st_gap.pane_gap_color.is_some() {
                let gap_col = match st_gap.pane_gap_color {
                    Some(c) => crate::chart_renderer::ui::style::color_alpha(c, st_gap.pane_gap_alpha),
                    None    => crate::chart_renderer::ui::style::color_alpha(t.toolbar_border, st_gap.pane_gap_alpha),
                };
                ui.painter().rect_filled(full_rect, 0.0, gap_col);
            }
        }

        // ── Pane divider drag handles (geometry-based, works for all layouts) ──
        if visible_count > 1 {
            // Find unique vertical divider X positions (between side-by-side panes)
            let mut v_dividers: Vec<f32> = Vec::new();
            // Find unique horizontal divider Y positions (between stacked panes)
            let mut h_dividers: Vec<f32> = Vec::new();
            for i in 0..visible_count {
                for j in (i+1)..visible_count {
                    let r0 = &pane_rects[i];
                    let r1 = &pane_rects[j];
                    // Side by side: r0.right ≈ r1.left (vertical divider)
                    if (r0.right() - r1.left()).abs() < 5.0
                        && r0.top().max(r1.top()) < r0.bottom().min(r1.bottom()) {
                        let x = (r0.right() + r1.left()) / 2.0;
                        if !v_dividers.iter().any(|&vx| (vx - x).abs() < 3.0) { v_dividers.push(x); }
                    }
                    // Stacked: r0.bottom ≈ r1.top (horizontal divider)
                    if (r0.bottom() - r1.top()).abs() < 5.0
                        && r0.left().max(r1.left()) < r0.right().min(r1.right()) {
                        let y = (r0.bottom() + r1.top()) / 2.0;
                        if !h_dividers.iter().any(|&hy| (hy - y).abs() < 3.0) { h_dividers.push(y); }
                    }
                }
            }
            // Vertical dividers (drag left/right to adjust column widths)
            for (di, &div_x) in v_dividers.iter().enumerate() {
                // Hit area: 5px left, 15px right = 20px total (smaller vertical)
                let div_rect = egui::Rect::from_min_size(
                    egui::pos2(div_x - 5.0, full_rect.top() + max_header_h),
                    egui::vec2(20.0, full_rect.height() - max_header_h));
                let div_resp = ui.interact(div_rect, egui::Id::new(("pane_div_h", di)), egui::Sense::drag());
                if div_resp.hovered() || div_resp.dragged() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
                    let alpha = if div_resp.dragged() { 30u8 } else { 15 };
                    ui.painter().rect_filled(div_rect, 0.0, color_alpha(t.accent, alpha));
                    ui.painter().line_segment(
                        [egui::pos2(div_x, full_rect.top()), egui::pos2(div_x, full_rect.bottom())],
                        egui::Stroke::new(1.0, color_alpha(t.accent, if div_resp.dragged() { 120 } else { 50 })));
                }
                if div_resp.dragged() {
                    let dx = div_resp.drag_delta().x;
                    let ratio = dx / full_rect.width();
                    // First vertical divider → split_h, second → split_h2
                    if di == 0 {
                        watchlist.pane_split_h = (watchlist.pane_split_h + ratio).clamp(0.15, 0.85);
                    } else {
                        watchlist.pane_split_h2 = (watchlist.pane_split_h2 + ratio).clamp(0.15, 0.85);
                    }
                    watchlist.pane_divider_dragging = true;
                }
                if div_resp.drag_stopped() { watchlist.pane_divider_dragging = false; }
            }
            // Horizontal dividers (drag up/down to adjust row heights)
            for (di, &div_y) in h_dividers.iter().enumerate() {
                // Hit area: nothing above, 10px below the pane header (starts below divider)
                let div_rect = egui::Rect::from_min_size(
                    egui::pos2(full_rect.left(), div_y + max_header_h),
                    egui::vec2(full_rect.width(), 10.0));
                let div_resp = ui.interact(div_rect, egui::Id::new(("pane_div_v", di)), egui::Sense::drag());
                if div_resp.hovered() || div_resp.dragged() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
                    let alpha = if div_resp.dragged() { 30u8 } else { 15 };
                    ui.painter().rect_filled(div_rect, 0.0, color_alpha(t.accent, alpha));
                    ui.painter().line_segment(
                        [egui::pos2(full_rect.left(), div_y), egui::pos2(full_rect.right(), div_y)],
                        egui::Stroke::new(1.0, color_alpha(t.accent, if div_resp.dragged() { 120 } else { 50 })));
                }
                if div_resp.dragged() {
                    let dy = div_resp.drag_delta().y;
                    let ratio = dy / full_rect.height();
                    // First horizontal divider → split_v, second → split_v2
                    if di == 0 {
                        watchlist.pane_split_v = (watchlist.pane_split_v + ratio).clamp(0.15, 0.85);
                    } else {
                        watchlist.pane_split_v2 = (watchlist.pane_split_v2 + ratio).clamp(0.15, 0.85);
                    }
                    watchlist.pane_divider_dragging = true;
                }
                if div_resp.drag_stopped() { watchlist.pane_divider_dragging = false; }
            }
        }

        for render_i in 0..visible_count {
        // When maximized, render the maximized pane using rect index 0
        let pane_idx = if let Some(max_idx) = watchlist.maximized_pane { max_idx } else { render_i };
        // All pane types go through render_chart_pane which renders the header/tabs first,
        // then dispatches to the type-specific content
        render_chart_pane(ui, ctx, panes, pane_idx, active_pane, visible_count, &pane_rects, theme_idx, watchlist, &account_data_cached);
        } // end for pane_idx

        // ── Cross-pane tab drag: ghost rendering + drop handling ──
        if let Some(drag) = watchlist.dragging_tab.clone() {
            let pointer = ui.input(|i| i.pointer.hover_pos()).or(Some(drag.current_pos)).unwrap();
            let pointer_released = ui.input(|i| i.pointer.primary_released());
            let pointer_down = ui.input(|i| i.pointer.primary_down());

            // Find which pane the pointer is over (for drop target highlight + drop target)
            let drop_pane: Option<usize> = pane_rects.iter().enumerate()
                .find(|(_, r)| r.contains(pointer))
                .map(|(i, _)| i);

            // Highlight drop target header
            if let Some(dst) = drop_pane {
                if dst != drag.source_pane {
                    if let Some(pane_r) = pane_rects.get(dst) {
                        let hsize = watchlist.pane_header_size.tabs_header_h();
                        let drop_rect = egui::Rect::from_min_size(pane_r.min, egui::vec2(pane_r.width(), hsize));
                        let _tref_owned = get_theme(theme_idx);
                        let t_ref = &_tref_owned;
                        ui.painter().rect_filled(drop_rect, 0.0, color_alpha(t_ref.accent, 40));
                        ui.painter().rect_stroke(drop_rect, 0.0,
                            egui::Stroke::new(2.0, color_alpha(t_ref.accent, 200)), egui::StrokeKind::Inside);
                    }
                }
            }

            // Paint ghost tab at cursor
            {
                let _tref_owned = get_theme(theme_idx);
                let t_ref = &_tref_owned;
                let ghost_text = format!("{} {}", drag.symbol, drag.timeframe);
                let font = egui::FontId::monospace(10.0);
                let galley = ui.painter().layout_no_wrap(ghost_text.clone(), font.clone(), TEXT_PRIMARY);
                let gw = galley.size().x + 20.0;
                let gh = galley.size().y + 8.0;
                let ghost_rect = egui::Rect::from_min_size(
                    egui::pos2(pointer.x + 8.0, pointer.y + 8.0),
                    egui::vec2(gw, gh));
                ui.painter().rect_filled(ghost_rect, 4.0, color_alpha(t_ref.toolbar_bg, 230));
                ui.painter().rect_stroke(ghost_rect, 4.0,
                    egui::Stroke::new(1.0, color_alpha(t_ref.accent, ALPHA_ACTIVE)),
                    egui::StrokeKind::Outside);
                ui.painter().text(
                    egui::pos2(ghost_rect.left() + 10.0, ghost_rect.center().y),
                    egui::Align2::LEFT_CENTER, &ghost_text, font, TEXT_PRIMARY);
                ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
            }

            // On release: perform the move if dropped on a different pane
            if pointer_released {
                if let Some(dst) = drop_pane {
                    if dst != drag.source_pane
                        && drag.source_pane < panes.len()
                        && dst < panes.len()
                        && drag.tab_idx < panes[drag.source_pane].tab_symbols.len()
                    {
                        let src = drag.source_pane;
                        let ti = drag.tab_idx;
                        // Remove from source
                        panes[src].tab_symbols.remove(ti);
                        panes[src].tab_timeframes.remove(ti);
                        if ti < panes[src].tab_changes.len() { panes[src].tab_changes.remove(ti); }
                        if ti < panes[src].tab_prices.len() { panes[src].tab_prices.remove(ti); }
                        if panes[src].tab_active >= panes[src].tab_symbols.len() {
                            panes[src].tab_active = panes[src].tab_symbols.len().saturating_sub(1);
                        } else if panes[src].tab_active > ti {
                            panes[src].tab_active -= 1;
                        }
                        // If source had exactly one tab (now zero), keep its current symbol
                        // rendering via the non-tab header. tab_symbols is allowed to be empty.

                        // Append to destination — initialize dest tabs if needed
                        if panes[dst].tab_symbols.is_empty() {
                            // Seed dest with its current symbol as tab 0
                            let dst_sym = panes[dst].symbol.clone();
                            let dst_tf = panes[dst].timeframe.clone();
                            let dst_px = panes[dst].bars.last().map(|b| b.close).unwrap_or(0.0);
                            panes[dst].tab_symbols.push(dst_sym);
                            panes[dst].tab_timeframes.push(dst_tf);
                            panes[dst].tab_changes.push(0.0);
                            panes[dst].tab_prices.push(dst_px);
                        }
                        panes[dst].tab_symbols.push(drag.symbol.clone());
                        panes[dst].tab_timeframes.push(drag.timeframe.clone());
                        panes[dst].tab_changes.push(drag.change);
                        panes[dst].tab_prices.push(drag.price);
                        panes[dst].tab_active = panes[dst].tab_symbols.len() - 1;
                        // Load the dragged symbol on dest
                        panes[dst].pending_symbol_change = Some(drag.symbol.clone());
                        panes[dst].pending_timeframe_change = Some(drag.timeframe.clone());
                        *active_pane = dst;
                    }
                }
                watchlist.dragging_tab = None;
            } else if !pointer_down {
                // Safety: clear drag state if mouse button got released outside our notice
                watchlist.dragging_tab = None;
            }
        }
    });
    span_end(); // chart_panes

    // ── Design Mode — full inspector with Style/Theme/Preview/click-to-select ──
    #[cfg(feature = "design-mode")]
    if crate::design_tokens::is_active() {
        // F12 toggles the new inspector. Defaults open on first activation.
        let f12 = ctx.input(|i| i.key_pressed(egui::Key::F12));
        DESIGN_INSPECTOR.with(|cell| {
            let mut cell = cell.borrow_mut();
            if cell.is_none() {
                *cell = Some(crate::design_inspector::Inspector::new(
                    std::path::PathBuf::from("design.toml"),
                ));
            }
            if let Some(inspector) = cell.as_mut() {
                if f12 { inspector.toggle(); }
                if let Some(tokens_lock) = crate::design_tokens::get_lock() {
                    if let Ok(mut tokens) = tokens_lock.write() {
                        let _changed = inspector.show(ctx, &mut *tokens);
                    }
                }
            }
        });

        // Design-mode style-editor panel (Ctrl+Shift+D) — extracted to widget.
        crate::chart_renderer::ui::widgets::design_mode_panel::show(ctx);
    }

    // ── Perf HUD (Ctrl+Shift+P) ─────────────────────────────────────────────
    {
        use std::sync::atomic::{AtomicBool, Ordering};
        static PERF_HUD_OPEN: AtomicBool = AtomicBool::new(false);
        if ctx.input(|i| i.modifiers.command && i.modifiers.shift && i.key_pressed(egui::Key::P)) {
            let was = PERF_HUD_OPEN.load(Ordering::Relaxed);
            PERF_HUD_OPEN.store(!was, Ordering::Relaxed);
        }
        let mut open = PERF_HUD_OPEN.load(Ordering::Relaxed);
        crate::chart_renderer::ui::widgets::perf_hud::show(ctx, &mut open);
        PERF_HUD_OPEN.store(open, Ordering::Relaxed);
    }

    handle_deferred(ctx, panes, active_pane, layout, watchlist);
}
