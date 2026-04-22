//! Connection Panel UI component.

use egui;
use super::style::*;
use super::super::gpu::*;
use crate::ui_kit::icons::Icon;
use crate::chart_renderer::gpu::APEXIB_URL;
use crate::chart_renderer::trading::{AccountSummary, Position, IbOrder, read_account_data};
const fn rgb(r: u8, g: u8, b: u8) -> egui::Color32 { egui::Color32::from_rgb(r, g, b) }

pub(crate) fn draw(_ctx: &egui::Context, _watchlist: &mut Watchlist, _panes: &mut [Chart], _ap: usize, t: &Theme, conn_panel_open: &mut bool) {
    if !*conn_panel_open { return; }

    // Use a simple egui::Window instead of dialog_window_themed to avoid potential panics
    let screen = _ctx.screen_rect();
    egui::Window::new("connections")
        .fixed_pos(egui::pos2(screen.right() - 240.0, 40.0))
        .fixed_size(egui::vec2(220.0, 0.0))
        .title_bar(false)
        .frame(egui::Frame::popup(&_ctx.style())
            .fill(t.toolbar_bg)
            .inner_margin(0.0)
            .stroke(egui::Stroke::new(1.0, color_alpha(t.toolbar_border, ALPHA_ACTIVE)))
            .corner_radius(RADIUS_LG))
        .show(_ctx, |ui| {
            if dialog_header(ui, "CONNECTIONS", t.dim) { *conn_panel_open = false; }
            ui.add_space(6.0);
            let m = 8.0;

            ui.horizontal(|ui| {
                ui.add_space(m);
                ui.label(egui::RichText::new("SERVICES").monospace().size(7.0).color(t.dim.gamma_multiply(0.5)));
            });
            ui.add_space(4.0);

            // Check connection status (non-blocking)
            let redis_ok = crate::bar_cache::is_connected();
            let ib_ok = read_account_data().map(|(a, _, _)| a.connected).unwrap_or(false);

            let services: &[(&str, &str, bool, &str)] = &[
                ("ApexIB", if ib_ok { "OK" } else { "OFF" }, ib_ok, APEXIB_URL),
                ("Redis", if redis_ok { "OK" } else { "OFF" }, redis_ok, "192.168.1.89:6379"),
                ("GPU", "DX12", true, "wgpu + egui"),
                ("Yahoo", "OK", true, "query1.finance.yahoo.com"),
            ];

            for (name, status, ok, detail) in services {
                ui.horizontal(|ui| {
                    ui.add_space(m);
                    let dot = if *ok { rgb(46, 204, 113) } else { rgb(231, 76, 60) };
                    ui.painter().circle_filled(egui::pos2(ui.cursor().min.x + 4.0, ui.cursor().min.y + 7.0), 3.5, dot);
                    ui.add_space(12.0);
                    ui.label(egui::RichText::new(*name).monospace().size(9.0).strong().color(t.text));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(m);
                        status_badge(ui, status, if *ok { t.bull } else { t.bear });
                    });
                });
                ui.horizontal(|ui| {
                    ui.add_space(m + 12.0);
                    ui.label(egui::RichText::new(*detail).monospace().size(8.0).color(t.dim.gamma_multiply(0.45)));
                });
                ui.add_space(3.0);
            }

            ui.add_space(6.0);
        });
}
