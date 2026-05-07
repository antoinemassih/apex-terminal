//! Overlay Manager UI component.

use egui;
use super::super::style::*;
use super::super::super::gpu::*;
use super::super::widgets::buttons::IconBtn;
use super::super::widgets::text::MonospaceCode;
use crate::ui_kit::icons::Icon;
use super::super::widgets::frames::PopupFrame;

pub(crate) fn draw(ctx: &egui::Context, watchlist: &mut Watchlist, panes: &mut [Chart], ap: usize, t: &Theme) {
// ── Overlay management pane ─────────────────────────────────────────────
if panes[ap].overlay_editing {
    let mut close_ov = false;
    let mut delete_idx: Option<usize> = None;
    egui::Window::new("overlay_mgr")
        .default_pos(egui::pos2(200.0, 80.0))
        .default_size(egui::vec2(260.0, 0.0))
        .resizable(false)
        .movable(true)
        .title_bar(false)
        .frame(PopupFrame::new().theme(t).ctx(ctx).build())
        .show(ctx, |ui| {
            let m = 8.0;
            // Header
            if dialog_header(ui, "SYMBOL OVERLAYS", t.dim) { close_ov = true; }
            ui.add_space(8.0);

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
                    ui.add_space(12.0);
                    let status = if ov_loading { " ..." } else if ov_empty { " (no data)" } else { "" };
                    let ov_label = format!("{}{}", ov_sym, status);
                    ui.add(MonospaceCode::new(&ov_label).size_px(font_sm()).color(oc));
                    // Color cycle (click to cycle through colors)
                    let (cr, cresp) = ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::click());
                    ui.painter().circle_filled(cr.center(), 5.0, oc);
                    if cresp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
                    if cresp.clicked() {
                        let all_colors: Vec<&str> = OVERLAY_COLORS.iter().chain(INDICATOR_COLORS.iter().filter(|c| !OVERLAY_COLORS.contains(c))).copied().collect();
                        let cur_idx = all_colors.iter().position(|&c| c == ov_color).unwrap_or(0);
                        panes[ap].symbol_overlays[oi].color = all_colors[(cur_idx + 1) % all_colors.len()].to_string();
                    }
                    // Candle toggle
                    let candle_icon = if ov_candles { Icon::CHART_BAR } else { Icon::CHART_LINE };
                    let candle_col = if ov_candles { t.accent } else { t.dim.gamma_multiply(0.5) };
                    if ui.add(IconBtn::new(candle_icon).size(font_sm()).color(candle_col)).clicked() {
                        panes[ap].symbol_overlays[oi].show_candles = !panes[ap].symbol_overlays[oi].show_candles;
                    }
                    // Delete
                    if ui.add(IconBtn::new(Icon::X).size(font_sm()).color(t.bear.gamma_multiply(0.5))).clicked() {
                        delete_idx = Some(oi);
                    }
                });
                ui.add_space(4.0);
            }

            if n_ov > 0 {
                ui.add_space(4.0);
                dialog_separator_shadow(ui, m, color_alpha(t.toolbar_border, alpha_muted()));
                ui.add_space(4.0);
            }

            // ── Add new overlay ──
            dialog_section(ui, "ADD OVERLAY", m, t.dim.gamma_multiply(0.5));
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.add_space(m);
                super::super::widgets::inputs::TextInput::new(&mut panes[ap].overlay_input)
                    .placeholder("Symbol...")
                    .width(240.0 - m * 2.0)
                    .font_size(font_sm())
                    .show(ui);
            });
            let query = panes[ap].overlay_input.trim().to_uppercase();
            if !query.is_empty() {
                ui.add_space(4.0);
                let results = crate::ui_kit::symbols::search_symbols(&query, 5);
                for si in &results {
                    ui.horizontal(|ui| {
                        ui.add_space(m);
                        let search_label = format!("{} — {}", si.symbol, si.name);
                        if ui.add(super::super::widgets::buttons::SimpleBtn::new(&search_label).color(t.dim).min_width(230.0)).clicked() {
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

            if ui.input(|i| i.key_pressed(egui::Key::Escape)) { close_ov = true; }
            ui.add_space(8.0);
        });
    if let Some(di) = delete_idx { panes[ap].symbol_overlays.remove(di); }
    if close_ov { panes[ap].overlay_editing = false; panes[ap].overlay_editing_idx = None; panes[ap].overlay_input.clear(); }
}



}
