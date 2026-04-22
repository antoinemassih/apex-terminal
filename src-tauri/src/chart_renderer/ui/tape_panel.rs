//! Time & Sales panel — real-time trade tape display.

use egui;
use super::style::*;
use super::super::gpu::{Watchlist, TapeRow, Theme};

const fn rgb(r: u8, g: u8, b: u8) -> egui::Color32 { egui::Color32::from_rgb(r, g, b) }

/// Draw the T&S content into `ui` (used by analysis_panel as a tab).
pub(crate) fn draw_content(ui: &mut egui::Ui, watchlist: &mut Watchlist, active_symbol: &str, t: &Theme) {
    let panel_w = ui.available_width();

    ui.label(egui::RichText::new(format!("TIME & SALES  {}", active_symbol)).monospace().size(9.0).strong().color(t.accent));
    ui.add_space(2.0);

    // Column headers
    ui.horizontal(|ui| {
        ui.add_space(4.0);
        let hw = (panel_w - 12.0) / 3.0;
        let hdr_color = t.dim.gamma_multiply(0.5);
        col_header(ui, "TIME",  hw, hdr_color, false);
        col_header(ui, "PRICE", hw, hdr_color, true);
        col_header(ui, "SIZE",  hw, hdr_color, true);
    });
    separator(ui, t.toolbar_border);

    // Trade rows
    let row_h = 14.0;
    egui::ScrollArea::vertical()
        .id_salt("tape_scroll")
        .stick_to_bottom(true)
        .show(ui, |ui| {
            ui.set_min_width(panel_w - 4.0);
            let entries: Vec<&TapeRow> = watchlist.tape_entries.iter()
                .filter(|e| e.symbol == active_symbol)
                .collect();

            if entries.is_empty() {
                ui.add_space(20.0);
                ui.label(egui::RichText::new("Waiting for trades...").monospace().size(9.0).color(t.dim.gamma_multiply(0.4)));
                if !crate::data::is_crypto(active_symbol) {
                    ui.label(egui::RichText::new("T&S available for crypto symbols").monospace().size(8.0).color(t.dim.gamma_multiply(0.3)));
                }
            }

            let col_w = (panel_w - 12.0) / 3.0;
            for entry in entries.iter().rev().take(200).collect::<Vec<_>>().into_iter().rev() {
                let (rect, _) = ui.allocate_exact_size(egui::vec2(panel_w - 4.0, row_h), egui::Sense::hover());

                let bg = if entry.is_buy {
                    color_alpha(rgb(46, 204, 113), 12)
                } else {
                    color_alpha(rgb(231, 76, 60), 12)
                };
                ui.painter().rect_filled(rect, 0.0, bg);

                let side_color = if entry.is_buy { rgb(46, 204, 113) } else { rgb(231, 76, 60) };
                let font = egui::FontId::monospace(8.5);

                // Time
                let secs = entry.time / 1000;
                let h = (secs / 3600) % 24;
                let m = (secs / 60) % 60;
                let s = secs % 60;
                let time_str = format!("{:02}:{:02}:{:02}", h, m, s);
                ui.painter().text(
                    egui::pos2(rect.left() + 4.0, rect.center().y),
                    egui::Align2::LEFT_CENTER, &time_str, font.clone(),
                    t.dim.gamma_multiply(0.6),
                );

                // Price
                let price_str = if entry.price >= 100.0 {
                    format!("{:.2}", entry.price)
                } else if entry.price >= 1.0 {
                    format!("{:.4}", entry.price)
                } else {
                    format!("{:.6}", entry.price)
                };
                ui.painter().text(
                    egui::pos2(rect.left() + 4.0 + col_w, rect.center().y),
                    egui::Align2::LEFT_CENTER, &price_str, font.clone(),
                    side_color,
                );

                // Size
                let qty_str = if entry.qty >= 1.0 {
                    format!("{:.4}", entry.qty)
                } else {
                    format!("{:.6}", entry.qty)
                };
                ui.painter().text(
                    egui::pos2(rect.right() - 4.0, rect.center().y),
                    egui::Align2::RIGHT_CENTER, &qty_str, font,
                    egui::Color32::from_gray(180),
                );
            }
        });
}

pub(crate) fn draw(ctx: &egui::Context, watchlist: &mut Watchlist, active_symbol: &str, t: &Theme) {
    if !watchlist.tape_open { return; }

    egui::SidePanel::right("time_and_sales")
        .default_width(220.0)
        .min_width(180.0)
        .max_width(350.0)
        .resizable(true)
        .frame(panel_frame_compact(t.toolbar_bg, t.toolbar_border))
        .show(ctx, |ui| {
            if panel_header_sub(ui, "TIME & SALES", Some(active_symbol), t.accent, t.dim) {
                watchlist.tape_open = false;
            }
            ui.add_space(2.0);
            draw_content(ui, watchlist, active_symbol, t);
        });
}
