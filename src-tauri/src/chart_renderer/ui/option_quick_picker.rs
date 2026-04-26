//! Option quick-picker — compact floating popup opened when clicking an
//! already-active options tab. Lets the user switch to a different strike
//! or expiry without leaving the chart pane.
//!
//! Data source: reuses `watchlist.chain_0dte` / `chain_far_dte` (the same
//! data the Watchlist CHAIN tab uses), fetched via `fetch_chain_background`.

use egui;
use super::style::*;
use super::super::gpu::*;
use crate::ui_kit::icons::Icon;

const DTE_LIST: &[i32] = &[0, 1, 2, 3, 7, 14, 30, 60];

fn dte_label(dte: i32) -> String {
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
}

pub(crate) fn draw(
    ctx: &egui::Context,
    watchlist: &mut Watchlist,
    panes: &mut [Chart],
    _ap: usize,
    t: &Theme,
) {
    // Iterate through panes; any with option_quick_open renders its own popup
    for pi in 0..panes.len() {
        if !panes[pi].option_quick_open { continue; }
        let underlying = panes[pi].underlying.clone();
        if underlying.is_empty() {
            panes[pi].option_quick_open = false;
            continue;
        }

        let pos = panes[pi].option_quick_pos;
        let mut close_picker = false;
        let mut pending_load: Option<(f32, bool)> = None; // (strike, is_call)
        let dte_idx = panes[pi].option_quick_dte_idx.min(DTE_LIST.len() - 1);
        let current_dte = DTE_LIST[dte_idx];

        // Ensure we always see fresh data for the current DTE
        let spot = panes[pi].bars.last().map(|b| b.close).unwrap_or(0.0);
        // Current option metadata (for prev/next strike buttons)
        let cur_strike = panes[pi].option_strike;
        let cur_is_call = panes[pi].option_type == "C";

        let window_resp = egui::Area::new(egui::Id::new(("opt_quick_picker", pi)))
            .order(egui::Order::Foreground)
            .fixed_pos(pos)
            .show(ctx, |ui| {
                egui::Frame::popup(&ctx.style())
                    .fill(t.toolbar_bg)
                    .stroke(egui::Stroke::new(STROKE_STD, color_alpha(t.toolbar_border, ALPHA_HEAVY)))
                    .inner_margin(egui::Margin::same(GAP_LG as i8))
                    .corner_radius(RADIUS_LG)
                    .shadow(egui::epaint::Shadow {
                        offset: [0, 4], blur: 14, spread: 0,
                        color: egui::Color32::from_black_alpha(80),
                    })
                    .show(ui, |ui| {
                        ui.set_width(260.0);

                        // ── Header: underlying symbol + close ──
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(&underlying)
                                .monospace().size(FONT_LG).strong().color(t.accent));
                            ui.label(egui::RichText::new(format!("@ {:.2}", spot))
                                .monospace().size(FONT_SM).color(t.dim));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if close_button(ui, t.dim) { close_picker = true; }
                            });
                        });
                        ui.add_space(GAP_SM);
                        separator(ui, color_alpha(t.toolbar_border, ALPHA_MUTED));
                        ui.add_space(GAP_SM);

                        // ── Expiry nav: < [DTE] > ──
                        ui.horizontal(|ui| {
                            ui.add_space(GAP_MD);
                            // Back arrow
                            let can_back = dte_idx > 0;
                            let back_col = if can_back { t.accent } else { t.dim.gamma_multiply(0.3) };
                            if icon_btn(ui, Icon::CARET_LEFT, back_col, FONT_LG).clicked() && can_back {
                                panes[pi].option_quick_dte_idx = dte_idx - 1;
                                let new_dte = DTE_LIST[dte_idx - 1];
                                fetch_chain_background(underlying.clone(), 15, new_dte, spot);
                            }
                            // DTE label (centered)
                            ui.vertical_centered(|ui| {
                                ui.label(egui::RichText::new(dte_label(current_dte))
                                    .monospace().size(FONT_LG).strong().color(TEXT_PRIMARY));
                            });
                            // Forward arrow
                            let can_fwd = dte_idx < DTE_LIST.len() - 1;
                            let fwd_col = if can_fwd { t.accent } else { t.dim.gamma_multiply(0.3) };
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.add_space(GAP_MD);
                                if icon_btn(ui, Icon::CARET_RIGHT, fwd_col, FONT_LG).clicked() && can_fwd {
                                    panes[pi].option_quick_dte_idx = dte_idx + 1;
                                    let new_dte = DTE_LIST[dte_idx + 1];
                                    fetch_chain_background(underlying.clone(), 15, new_dte, spot);
                                }
                            });
                        });
                        ui.add_space(GAP_SM);

                        // ── Quick strike navigation: < Prev Strike    Next Strike > ──
                        // Only show when we know the current strike (is_option tab)
                        if cur_strike > 0.0 {
                            ui.add_space(GAP_XS);
                            ui.horizontal(|ui| {
                                ui.add_space(GAP_SM);
                                let half_w = 120.0;
                                // Prev strike
                                let (prev_rect, prev_resp) = ui.allocate_exact_size(
                                    egui::vec2(half_w, 22.0), egui::Sense::click());
                                let prev_bg = if prev_resp.hovered() {
                                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                    color_alpha(t.accent, ALPHA_TINT)
                                } else { color_alpha(t.toolbar_border, ALPHA_SUBTLE) };
                                ui.painter().rect_filled(prev_rect, RADIUS_MD, prev_bg);
                                ui.painter().rect_stroke(prev_rect, RADIUS_MD,
                                    egui::Stroke::new(STROKE_THIN, color_alpha(t.accent, ALPHA_LINE)),
                                    egui::StrokeKind::Inside);
                                ui.painter().text(
                                    prev_rect.center(), egui::Align2::CENTER_CENTER,
                                    format!("{} Prev Strike", Icon::CARET_LEFT),
                                    egui::FontId::monospace(FONT_SM), TEXT_PRIMARY);
                                if prev_resp.clicked() {
                                    // Find the next-lower strike in the current type's chain
                                    let rows = if cur_is_call { &watchlist.chain_0dte.0 } else { &watchlist.chain_0dte.1 };
                                    let rows = if current_dte == 0 { rows }
                                        else if cur_is_call { &watchlist.chain_far.0 } else { &watchlist.chain_far.1 };
                                    let mut sorted: Vec<f32> = rows.iter().map(|r| r.strike).collect();
                                    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
                                    if let Some(&lower) = sorted.iter().rev().find(|&&s| s < cur_strike) {
                                        pending_load = Some((lower, cur_is_call));
                                    }
                                }
                                ui.add_space(GAP_SM);
                                // Next strike
                                let (next_rect, next_resp) = ui.allocate_exact_size(
                                    egui::vec2(half_w, 22.0), egui::Sense::click());
                                let next_bg = if next_resp.hovered() {
                                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                    color_alpha(t.accent, ALPHA_TINT)
                                } else { color_alpha(t.toolbar_border, ALPHA_SUBTLE) };
                                ui.painter().rect_filled(next_rect, RADIUS_MD, next_bg);
                                ui.painter().rect_stroke(next_rect, RADIUS_MD,
                                    egui::Stroke::new(STROKE_THIN, color_alpha(t.accent, ALPHA_LINE)),
                                    egui::StrokeKind::Inside);
                                ui.painter().text(
                                    next_rect.center(), egui::Align2::CENTER_CENTER,
                                    format!("Next Strike {}", Icon::CARET_RIGHT),
                                    egui::FontId::monospace(FONT_SM), TEXT_PRIMARY);
                                if next_resp.clicked() {
                                    let rows = if cur_is_call { &watchlist.chain_0dte.0 } else { &watchlist.chain_0dte.1 };
                                    let rows = if current_dte == 0 { rows }
                                        else if cur_is_call { &watchlist.chain_far.0 } else { &watchlist.chain_far.1 };
                                    let mut sorted: Vec<f32> = rows.iter().map(|r| r.strike).collect();
                                    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
                                    if let Some(&higher) = sorted.iter().find(|&&s| s > cur_strike) {
                                        pending_load = Some((higher, cur_is_call));
                                    }
                                }
                            });
                            ui.add_space(GAP_SM);
                            separator(ui, color_alpha(t.toolbar_border, ALPHA_MUTED));
                            ui.add_space(GAP_SM);
                        }

                        // Column headers: CALL | STRIKE | PUT
                        ui.horizontal(|ui| {
                            let cw = 250.0 / 3.0;
                            col_header(ui, "CALL",   cw, t.dim.gamma_multiply(0.5), false);
                            col_header(ui, "STRIKE", cw, t.dim.gamma_multiply(0.5), false);
                            col_header(ui, "PUT",    cw, t.dim.gamma_multiply(0.5), false);
                        });
                        separator(ui, color_alpha(t.toolbar_border, ALPHA_MUTED));

                        // ── Chain rows ──
                        // Source: watchlist.chain_0dte when current_dte == 0, else chain_far
                        if current_dte != 0 && watchlist.chain_far_dte != current_dte {
                            watchlist.chain_far_dte = current_dte;
                            fetch_chain_background(underlying.clone(), 15, current_dte, spot);
                        }
                        let chain_ref = if current_dte == 0 {
                            &watchlist.chain_0dte
                        } else {
                            &watchlist.chain_far
                        };
                        let (calls, puts) = (&chain_ref.0, &chain_ref.1);

                        if calls.is_empty() && puts.is_empty() {
                            ui.add_space(GAP_LG);
                            ui.vertical_centered(|ui| {
                                ui.label(egui::RichText::new("Loading chain…")
                                    .monospace().size(FONT_SM).color(t.dim));
                            });
                            ui.add_space(GAP_LG);
                        } else {
                            // Build a sorted list of unique strikes
                            let mut strikes: Vec<f32> = calls.iter().map(|r| r.strike)
                                .chain(puts.iter().map(|r| r.strike))
                                .collect();
                            strikes.sort_by(|a, b| a.partial_cmp(b).unwrap());
                            strikes.dedup_by(|a, b| (*a - *b).abs() < 0.01);

                            egui::ScrollArea::vertical()
                                .id_salt(("opt_quick_scroll", pi))
                                .max_height(260.0)
                                .show(ui, |ui| {
                                    for strike in &strikes {
                                        let call_row = calls.iter().find(|r| (r.strike - strike).abs() < 0.01);
                                        let put_row  = puts.iter().find(|r| (r.strike - strike).abs() < 0.01);
                                        let is_atm = (strike - spot).abs() < (spot * 0.005).max(0.5);
                                        ui.horizontal(|ui| {
                                            let cw = 86.0;
                                            // CALL cell
                                            let call_text = call_row.map(|r| format!("{:.2}", r.bid))
                                                .unwrap_or_else(|| "-".into());
                                            let (crect, cresp) = ui.allocate_exact_size(egui::vec2(cw, 20.0), egui::Sense::click());
                                            if cresp.hovered() {
                                                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                                ui.painter().rect_filled(crect, RADIUS_SM, color_alpha(t.bull, ALPHA_GHOST));
                                            }
                                            ui.painter().text(crect.center(), egui::Align2::CENTER_CENTER,
                                                &call_text, egui::FontId::monospace(FONT_SM),
                                                if call_row.is_some() { t.bull } else { t.dim.gamma_multiply(0.4) });
                                            if cresp.clicked() && call_row.is_some() {
                                                pending_load = Some((*strike, true));
                                            }
                                            // STRIKE cell
                                            let (srect, _) = ui.allocate_exact_size(egui::vec2(cw, 20.0), egui::Sense::hover());
                                            let strike_col = if is_atm { t.accent } else { TEXT_PRIMARY };
                                            let strike_txt = if (strike - strike.round()).abs() < 0.005 {
                                                format!("{:.0}", strike)
                                            } else {
                                                format!("{:.1}", strike)
                                            };
                                            ui.painter().text(srect.center(), egui::Align2::CENTER_CENTER,
                                                strike_txt,
                                                egui::FontId::monospace(FONT_SM),
                                                strike_col);
                                            // PUT cell
                                            let put_text = put_row.map(|r| format!("{:.2}", r.bid))
                                                .unwrap_or_else(|| "-".into());
                                            let (prect, presp) = ui.allocate_exact_size(egui::vec2(cw, 20.0), egui::Sense::click());
                                            if presp.hovered() {
                                                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                                ui.painter().rect_filled(prect, RADIUS_SM, color_alpha(t.bear, ALPHA_GHOST));
                                            }
                                            ui.painter().text(prect.center(), egui::Align2::CENTER_CENTER,
                                                &put_text, egui::FontId::monospace(FONT_SM),
                                                if put_row.is_some() { t.bear } else { t.dim.gamma_multiply(0.4) });
                                            if presp.clicked() && put_row.is_some() {
                                                pending_load = Some((*strike, false));
                                            }
                                        });
                                    }
                                });
                        }
                    });
            });

        // Close on click outside
        if !close_picker {
            let picker_rect = window_resp.response.rect;
            if ctx.input(|i| i.pointer.any_pressed()) {
                if let Some(p) = ctx.input(|i| i.pointer.interact_pos()) {
                    if !picker_rect.contains(p) {
                        close_picker = true;
                    }
                }
            }
        }

        if close_picker {
            panes[pi].option_quick_open = false;
        }

        // Handle strike click → swap the contract on this pane (not a split).
        if let Some((strike, is_call)) = pending_load {
            let rows = if current_dte == 0 {
                if is_call { &watchlist.chain_0dte.0 } else { &watchlist.chain_0dte.1 }
            } else {
                if is_call { &watchlist.chain_far.0 } else { &watchlist.chain_far.1 }
            };
            let occ = rows.iter()
                .find(|r| (r.strike - strike).abs() < 0.01)
                .map(|r| r.contract.clone())
                .unwrap_or_default();
            // If the user is already on an option pane, replace the contract in
            // place instead of routing through pending_opt_chart (which splits).
            if panes[pi].is_option {
                let occ_final = if occ.starts_with("O:") {
                    occ.clone()
                } else {
                    crate::chart_renderer::gpu::synthesize_occ(&underlying, strike, is_call, "")
                };
                let strike_str = if (strike - strike.round()).abs() < 0.005 {
                    format!("{:.0}", strike)
                } else { format!("{:.1}", strike) };
                let opt_sym = format!("{} {}{}", underlying, strike_str, if is_call { "C" } else { "P" });
                panes[pi].symbol = opt_sym.clone();
                panes[pi].option_type = if is_call { "C".into() } else { "P".into() };
                panes[pi].option_strike = strike;
                panes[pi].option_contract = occ_final.clone();
                panes[pi].bars.clear();
                panes[pi].timestamps.clear();
                let tf = panes[pi].timeframe.clone();
                if !occ_final.is_empty() && crate::apex_data::is_enabled() {
                    crate::chart_renderer::gpu::fetch_option_bars_background(occ_final, opt_sym, tf);
                }
            } else {
                watchlist.pending_opt_chart = Some((underlying.clone(), strike, is_call, String::new()));
                watchlist.pending_opt_chart_contract = Some(occ);
            }
            panes[pi].option_quick_open = false;
        }
    }
}
