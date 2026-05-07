//! Time & Sales panel — real-time trade tape display.

use egui;
use super::super::style::*;
use super::super::super::gpu::{Watchlist, TapeRow, Theme};
use super::super::widgets::frames::CompactPanelFrame;
use super::super::widgets::text::MonospaceCode;
use super::super::widgets::rows::ListRow;
use super::super::widgets::headers::PanelHeaderWithClose;


/// Draw the T&S content into `ui` (used by analysis_panel as a tab).
pub(crate) fn draw_content(ui: &mut egui::Ui, watchlist: &mut Watchlist, active_symbol: &str, t: &Theme) {
    let panel_w = ui.available_width();

    ui.add(MonospaceCode::new(&format!("TIME & SALES  {}", active_symbol)).size_px(9.0).strong(true).color(t.accent));
    ui.add_space(4.0);

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
                ui.add(MonospaceCode::new("Waiting for trades...").size_px(9.0).color(t.dim).gamma(0.4));
                if !crate::data::is_crypto(active_symbol) && !crate::apex_data::is_enabled() {
                    ui.add(MonospaceCode::new("Enable ApexData in settings for stock T&S").size_px(8.0).color(t.dim).gamma(0.3));
                }
            }

            let col_w = (panel_w - 12.0) / 3.0;
            for entry in entries.iter().rev().take(200).collect::<Vec<_>>().into_iter().rev() {
                let side_color = if entry.is_buy { egui::Color32::from_rgb(46, 204, 113) } else { egui::Color32::from_rgb(231, 76, 60) };

                // Build strings before the closure to avoid borrow issues.
                let secs = entry.time / 1000;
                let h = (secs / 3600) % 24;
                let m = (secs / 60) % 60;
                let s = secs % 60;
                let time_str = format!("{:02}:{:02}:{:02}", h, m, s);
                let price_str = if entry.price >= 100.0 {
                    format!("{:.2}", entry.price)
                } else if entry.price >= 1.0 {
                    format!("{:.4}", entry.price)
                } else {
                    format!("{:.6}", entry.price)
                };
                let qty_str = if entry.qty >= 1.0 {
                    format!("{:.4}", entry.qty)
                } else {
                    format!("{:.6}", entry.qty)
                };

                let dim_color = t.dim.gamma_multiply(0.6);
                let qty_color = egui::Color32::from_gray(180);
                let cw = col_w;

                ListRow::new(row_h)
                    .hover_enabled(false)
                    .row_tint(side_color, 12)
                    .body(move |ui| {
                        let rect = ui.max_rect();
                        let font = egui::FontId::monospace(11.0);
                        ui.painter().text(
                            egui::pos2(rect.left(), rect.center().y),
                            egui::Align2::LEFT_CENTER, &time_str, font.clone(),
                            dim_color,
                        );
                        ui.painter().text(
                            egui::pos2(rect.left() + cw, rect.center().y),
                            egui::Align2::LEFT_CENTER, &price_str, font.clone(),
                            side_color,
                        );
                        ui.painter().text(
                            egui::pos2(rect.right(), rect.center().y),
                            egui::Align2::RIGHT_CENTER, &qty_str, font,
                            qty_color,
                        );
                    })
                    .show(ui);
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
        .frame(CompactPanelFrame::new(t.toolbar_bg, t.toolbar_border).build())
        .show(ctx, |ui| {
            if PanelHeaderWithClose::new("TIME & SALES").subtitle(active_symbol).theme(t).show(ui) {
                watchlist.tape_open = false;
            }
            ui.add_space(4.0);
            draw_content(ui, watchlist, active_symbol, t);
        });
}
