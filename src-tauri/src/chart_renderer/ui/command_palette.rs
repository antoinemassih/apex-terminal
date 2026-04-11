//! Command Palette UI component.

use egui;
use super::style::*;
use super::super::gpu::*;
use crate::ui_kit::icons::Icon;
use crate::chart_renderer::gpu::fetch_bars_background;
use crate::chart_renderer::trading::OrderStatus;

pub(crate) fn draw(ctx: &egui::Context, watchlist: &mut Watchlist, panes: &mut [Chart], ap: usize, t: &Theme) {
// ── Command palette (Ctrl+Space) ────────────────────────────────────────
if !ctx.wants_keyboard_input() {
    if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::Space)) {
        watchlist.cmd_palette_open = !watchlist.cmd_palette_open;
        if watchlist.cmd_palette_open { watchlist.cmd_palette_query.clear(); watchlist.cmd_palette_results.clear(); watchlist.cmd_palette_sel = 0; }
    }
}
if watchlist.cmd_palette_open {
    // Escape closes
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) { watchlist.cmd_palette_open = false; }

    let screen = ctx.screen_rect();
    let pal_w = 500.0_f32;
    let pal_x = (screen.width() - pal_w) / 2.0;
    let pal_y = screen.height() * 0.2;

    // Dimmed background overlay
    egui::Area::new(egui::Id::new("cmd_palette_bg"))
        .fixed_pos(egui::pos2(0.0, 0.0))
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            ui.painter().rect_filled(screen, 0.0, egui::Color32::from_black_alpha(120));
        });

    let cmd_pal_resp = egui::Window::new("cmd_palette")
        .fixed_pos(egui::pos2(pal_x, pal_y))
        .fixed_size(egui::vec2(pal_w, 0.0))
        .title_bar(false)
        .frame(egui::Frame::popup(&ctx.style())
            .fill(color_alpha(t.toolbar_bg, 250))
            .inner_margin(egui::Margin::same(8))
            .stroke(egui::Stroke::new(1.5, color_alpha(t.accent, 80)))
            .corner_radius(8.0)
)
        .show(ctx, |ui| {
            // Search field
            let te = ui.add(egui::TextEdit::singleline(&mut watchlist.cmd_palette_query)
                .desired_width(pal_w - 16.0)
                .font(egui::FontId::monospace(14.0))
                .hint_text("Symbol or command...")
                .frame(false));
            te.request_focus();

            // Search on query change
            let query = watchlist.cmd_palette_query.trim().to_uppercase();
            if !query.is_empty() {
                // Built-in commands
                let mut results: Vec<(String, String, String)> = Vec::new();
                if "> flatten".starts_with(&query.to_lowercase()) || "flatten".starts_with(&query.to_lowercase()) {
                    results.push(("> flatten".into(), "Flatten all positions".into(), "Command".into()));
                }
                if "> cancel".starts_with(&query.to_lowercase()) || "cancel all".starts_with(&query.to_lowercase()) {
                    results.push(("> cancel".into(), "Cancel all open orders".into(), "Command".into()));
                }

                // Symbol search (static)
                let sym_results = crate::ui_kit::symbols::search_symbols(&query, 10);
                for si in &sym_results {
                    results.push((si.symbol.to_string(), si.name.to_string(), "Symbol".into()));
                }
                watchlist.cmd_palette_results = results;
            } else {
                // Show recent symbols when empty
                watchlist.cmd_palette_results.clear();
            }

            // Arrow keys navigate, Enter selects
            if ui.input(|i| i.key_pressed(egui::Key::ArrowDown)) {
                watchlist.cmd_palette_sel = (watchlist.cmd_palette_sel + 1).min(watchlist.cmd_palette_results.len() as i32 - 1);
            }
            if ui.input(|i| i.key_pressed(egui::Key::ArrowUp)) {
                watchlist.cmd_palette_sel = (watchlist.cmd_palette_sel - 1).max(0);
            }

            let mut execute_idx: Option<usize> = None;
            if ui.input(|i| i.key_pressed(egui::Key::Enter)) && !watchlist.cmd_palette_results.is_empty() {
                execute_idx = Some(watchlist.cmd_palette_sel.max(0) as usize);
            }

            // Results list
            if !watchlist.cmd_palette_results.is_empty() {
                ui.add_space(4.0);
                ui.add(egui::Separator::default().spacing(2.0));
                ui.add_space(4.0);
                for (ri, (sym, name, rtype)) in watchlist.cmd_palette_results.iter().enumerate() {
                    let is_sel = ri as i32 == watchlist.cmd_palette_sel;
                    let bg = if is_sel { color_alpha(t.accent, 30) } else { egui::Color32::TRANSPARENT };
                    let resp = ui.horizontal(|ui| {
                        let full_rect = egui::Rect::from_min_size(ui.cursor().min, egui::vec2(pal_w - 16.0, 24.0));
                        ui.painter().rect_filled(full_rect, 4.0, bg);
                        ui.add_space(8.0);
                        let type_col = if rtype == "Command" { t.accent } else { t.dim.gamma_multiply(0.4) };
                        ui.label(egui::RichText::new(rtype).monospace().size(8.0).color(type_col));
                        ui.add_space(4.0);
                        let sym_col = if is_sel { egui::Color32::WHITE } else { egui::Color32::from_rgb(220, 220, 230) };
                        ui.label(egui::RichText::new(sym).monospace().size(12.0).strong().color(sym_col));
                        ui.label(egui::RichText::new(name).monospace().size(10.0).color(t.dim.gamma_multiply(0.5)));
                        ui.allocate_space(egui::vec2(0.0, 24.0)); // ensure row height
                    });
                    if resp.response.interact(egui::Sense::click()).clicked() {
                        execute_idx = Some(ri);
                    }
                }
            }

            // Execute selected command/symbol
            if let Some(idx) = execute_idx {
                if let Some((sym, _, rtype)) = watchlist.cmd_palette_results.get(idx) {
                    if rtype == "Command" {
                        match sym.as_str() {
                            "> flatten" => {
                                for chart in panes.iter_mut() { chart.orders.retain(|o| o.status == OrderStatus::Executed); }
                                std::thread::spawn(|| {
                                    let _ = reqwest::blocking::Client::new()
                                        .post(format!("{}/risk/flatten", APEXIB_URL))
                                        .timeout(std::time::Duration::from_secs(5)).send();
                                });
                            }
                            "> cancel" => {
                                for chart in panes.iter_mut() { chart.orders.clear(); }
                                std::thread::spawn(|| {
                                    let _ = reqwest::blocking::Client::new()
                                        .delete(format!("{}/orders", APEXIB_URL))
                                        .timeout(std::time::Duration::from_secs(5)).send();
                                });
                            }
                            _ => {}
                        }
                    } else {
                        // Symbol — load in active pane
                        let sym_str = sym.clone();
                        let tf = panes[ap].timeframe.clone();
                        panes[ap].symbol = sym_str.clone();
                        fetch_bars_background(sym_str, tf);
                    }
                    watchlist.cmd_palette_open = false;
                }
            }
        });

    // Click-away closes command palette
    if let Some(wr) = &cmd_pal_resp {
        let pal_rect = wr.response.rect;
        if ctx.input(|i| i.pointer.any_pressed()) {
            if let Some(pos) = ctx.input(|i| i.pointer.interact_pos()) {
                if !pal_rect.contains(pos) {
                    watchlist.cmd_palette_open = false;
                }
            }
        }
    }
}


}
