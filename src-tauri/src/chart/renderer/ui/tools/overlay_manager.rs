//! Overlay Manager UI component.

use egui;
use crate::ui_kit::sx::Tone;
use super::super::style::*;
use super::super::super::gpu::*;
use super::super::components::text::MonospaceCode;
use crate::ui_kit::icons::Icon;
use crate::ui_kit::widgets::{Button, Input, Tooltip};
use crate::ui_kit::widgets::tokens::{Variant, Size as KitSize};
use crate::ui_kit::widgets::icon_placement::IconPlacement;

pub(crate) fn draw(ctx: &egui::Context, watchlist: &mut Watchlist, panes: &mut [Chart], ap: usize, t: &Theme) {
// ── Overlay management pane ─────────────────────────────────────────────
// Always call show() (no `if open` gate) and drive it with `.open(flag)` so the
// overlay plays its fade-out; ToolOverlay stops rendering once fully hidden.
{
    let mut delete_idx: Option<usize> = None;
    // Migrated to ToolOverlay (2026-05-26) — consolidates header chrome.
    let portable_t = crate::chart_renderer::theme_impl::theme_to_portable(t);
    let modal_resp = crate::ui_kit::widgets::ToolOverlay::new("SYMBOL OVERLAYS")
        .id("overlay_mgr")
        .width(260.0)
        .open(panes[ap].overlay_editing)
        .show(ctx, &portable_t, |ui| {
            let m = 8.0;
            ui.add_space(gap_sm());

            // ── Existing overlays ──
            let n_ov = panes[ap].symbol_overlays.len();
            for oi in 0..n_ov {
                let ov_sym = panes[ap].symbol_overlays[oi].symbol.clone();
                let ov_color = panes[ap].symbol_overlays[oi].color.clone();
                let ov_loading = panes[ap].symbol_overlays[oi].loading;
                let ov_empty = panes[ap].symbol_overlays[oi].bars.is_empty();
                let ov_candles = panes[ap].symbol_overlays[oi].show_candles;
                let oc = hex_to_color(&ov_color, 1.0);
                ui.horizontal(|ui| {
                    ui.add_space(m);
                    ui.painter().circle_filled(egui::pos2(ui.cursor().min.x + 5.0, ui.cursor().min.y + 10.0), 4.0, oc);
                    ui.add_space(gap_md());
                    let status = if ov_loading { " ..." } else if ov_empty { " (no data)" } else { "" };
                    let ov_label = format!("{}{}", ov_sym, status);
                    ui.add(MonospaceCode::new(&ov_label).size_px(font_sm()).color(oc));
                    // Color cycle (click to cycle through colors)
                    let (cr, cresp) = ui.allocate_exact_size(egui::vec2(icon_sm(), icon_sm()), egui::Sense::click());
                    ui.painter().circle_filled(cr.center(), 5.0, oc);
                    crate::chart_renderer::ui::style::cursor::clickable(ui, &cresp);
                    cursor::focus_ring(ui, &cresp, t.accent);
                    if cresp.clicked() {
                        let all_colors: Vec<&str> = OVERLAY_COLORS.iter().chain(INDICATOR_COLORS.iter().filter(|c| !OVERLAY_COLORS.contains(c))).copied().collect();
                        let cur_idx = all_colors.iter().position(|&c| c == ov_color).unwrap_or(0);
                        panes[ap].symbol_overlays[oi].color = all_colors[(cur_idx + 1) % all_colors.len()].to_string();
                    }
                    // Candle toggle
                    let candle_icon = if ov_candles { Icon::CHART_BAR } else { Icon::CHART_LINE };
                    let candle_col = if ov_candles { t.accent } else { color_half(t.dim) };
                    let r = ui.add(Button::icon(candle_icon).variant(Variant::Ghost).glyph_color(candle_col).size(KitSize::Sm).placement(IconPlacement::ListRow));
                    Tooltip::new("Toggle candles / line").show(ui, &r, t);
                    if r.clicked() {
                        panes[ap].symbol_overlays[oi].show_candles = !panes[ap].symbol_overlays[oi].show_candles;
                    }
                    // Delete
                    let r = ui.add(Button::icon(Icon::X).variant(Variant::Ghost).size(KitSize::Sm).placement(IconPlacement::ListRow).tone_destructive());
                    Tooltip::new("Remove overlay").show(ui, &r, t);
                    if r.clicked() {
                        delete_idx = Some(oi);
                    }
                });
                ui.add_space(gap_xs());
            }

            if n_ov > 0 {
                ui.add_space(gap_xs());
                dialog_separator_shadow(ui, m, tint(t, Tone::Border, alpha_muted()));
                ui.add_space(gap_xs());
            }

            // ── Add new overlay ──
            dialog_section(ui, "ADD OVERLAY", m, color_half(t.dim));
            ui.add_space(gap_xs());
            ui.horizontal(|ui| {
                ui.add_space(m);
                Input::new(&mut panes[ap].overlay_input)
                    .placeholder("Symbol...")
                    .min_width(240.0 - m * 2.0)
                    .size(KitSize::Sm)
                    .show(ui, t);
            });
            let query = panes[ap].overlay_input.trim().to_uppercase();
            if !query.is_empty() {
                ui.add_space(gap_xs());
                let results = crate::ui_kit::symbols::search_symbols(&query, 5);
                for si in &results {
                    ui.horizontal(|ui| {
                        ui.add_space(m);
                        let search_label = format!("{} — {}", si.symbol, si.name);
                        if ui.add(Button::new(search_label.as_str()).variant(Variant::Secondary).simple_treatment(true).fg(t.dim).min_size(egui::vec2(230.0, 0.0))).clicked() {
                            let color = OVERLAY_COLORS[panes[ap].symbol_overlays.len() % OVERLAY_COLORS.len()].to_string();
                            panes[ap].symbol_overlays.push(SymbolOverlay {
                                symbol: si.symbol.to_string(), color, bars: vec![], timestamps: vec![], loading: true, show_candles: false, visible: true,
                            });
                            fetch_overlay_bars_background(si.symbol.to_string(), panes[ap].timeframe.clone());
                            panes[ap].overlay_input.clear();
                        }
                    });
                }
            }
            if ui.input(|i| i.key_pressed(egui::Key::Enter)) && !query.is_empty() {
                let color = OVERLAY_COLORS[panes[ap].symbol_overlays.len() % OVERLAY_COLORS.len()].to_string();
                panes[ap].symbol_overlays.push(SymbolOverlay {
                    symbol: query.clone(), color, bars: vec![], timestamps: vec![], loading: true, show_candles: false, visible: true,
                });
                fetch_overlay_bars_background(query, panes[ap].timeframe.clone());
                panes[ap].overlay_input.clear();
            }

            ui.add_space(gap_sm());
        });
    if let Some(di) = delete_idx { panes[ap].symbol_overlays.remove(di); }
    if modal_resp.closed { panes[ap].overlay_editing = false; panes[ap].overlay_editing_idx = None; panes[ap].overlay_input.clear(); }
}



}
