//! Connection Panel UI component.

use egui;
use super::style::*;
use super::super::gpu::*;
use crate::ui_kit::icons::Icon;
use crate::chart_renderer::gpu::APEXIB_URL;
use crate::chart_renderer::trading::{AccountSummary, Position, IbOrder, read_account_data};
const fn rgb(r: u8, g: u8, b: u8) -> egui::Color32 { egui::Color32::from_rgb(r, g, b) }

pub(crate) fn draw(ctx: &egui::Context, watchlist: &mut Watchlist, panes: &mut [Chart], ap: usize, t: &Theme, conn_panel_open: &mut bool) {
// ── Connection panel popup ──────────────────────────────────────────────
if *conn_panel_open {
    dialog_window_themed(ctx, "conn_panel", egui::pos2(ctx.screen_rect().right() - 260.0, 40.0), 240.0, t.toolbar_bg, t.toolbar_border, None)
        .show(ctx, |ui| {
            if dialog_header(ui, "CONNECTIONS", t.dim) { *conn_panel_open = false; }
            ui.add_space(8.0);
            let m = 10.0;

            dialog_section(ui, "SERVICES", m, t.dim.gamma_multiply(0.5));
            let svc_row = |ui: &mut egui::Ui, name: &str, status: &str, ok: bool, detail: &str| {
                ui.horizontal(|ui| {
                    ui.add_space(m);
                    let dot_color = if ok { rgb(46,204,113) } else { rgb(231,76,60) };
                    ui.painter().circle_filled(egui::pos2(ui.cursor().min.x + 4.0, ui.cursor().min.y + 7.0), 3.5, dot_color);
                    ui.add_space(12.0);
                    ui.label(egui::RichText::new(name).monospace().size(10.0).strong().color(egui::Color32::from_rgb(200,200,210)));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(m);
                        status_badge(ui, status, if ok { t.bull } else { t.bear });
                    });
                });
                ui.horizontal(|ui| {
                    ui.add_space(m + 12.0);
                    ui.label(egui::RichText::new(detail).monospace().size(8.0).color(t.dim.gamma_multiply(0.45)));
                });
                ui.add_space(3.0);
            };

            let redis_ok = crate::bar_cache::get("__ping_test", "").is_none();
            let ib_ok = read_account_data().as_ref().map(|(a, _, _)| a.connected).unwrap_or(false);
            svc_row(ui, "ApexIB", if ib_ok { "OK" } else { "OFF" }, ib_ok, APEXIB_URL);
            svc_row(ui, "Redis Cache", if redis_ok { "OK" } else { "OFF" }, redis_ok, "192.168.1.89:6379");
            svc_row(ui, "GPU Engine", "DX12", true, "wgpu + egui");
            svc_row(ui, "Data Feed", "OK", true, "query1.finance.yahoo.com");
            svc_row(ui, "OCOCO", "OK", true, "192.168.1.60:30300");

            ui.add_space(4.0);
            dialog_separator_shadow(ui, m, color_alpha(t.toolbar_border, 40));
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.add_space(m);
                ui.label(egui::RichText::new("apexib:5000 \u{00B7} redis:6379 \u{00B7} ococo:30300 \u{00B7} yahoo").monospace().size(8.0).color(t.dim.gamma_multiply(0.3)));
            });
            ui.add_space(8.0);
        });
}


}
