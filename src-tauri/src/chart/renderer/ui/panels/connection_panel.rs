//! Connection Panel UI component.

use egui;
use super::super::style::*;
use super::super::super::gpu::*;
use super::super::widgets::buttons::SimpleBtn;
use super::super::widgets::text::{BodyLabel, SectionLabel};
use crate::ui_kit::icons::Icon;
use crate::ui_kit::widgets::Progress;
use crate::ui_kit::widgets::tokens::Size as KitSize;
use crate::chart_renderer::gpu::APEXIB_URL;
use crate::chart_renderer::trading::{AccountSummary, Position, IbOrder, read_account_data};

pub(crate) fn draw(_ctx: &egui::Context, _watchlist: &mut Watchlist, _panes: &mut [Chart], _ap: usize, t: &Theme, conn_panel_open: &mut bool) {
    if !*conn_panel_open { return; }

    use super::super::widgets::modal::{Modal, Anchor, HeaderStyle, FrameKind};
    let screen = _ctx.screen_rect();
    let custom_frame = egui::Frame::popup(&_ctx.style())
        .fill(t.toolbar_bg)
        .inner_margin(0.0)
        .stroke(egui::Stroke::new(1.0, color_alpha(t.toolbar_border, alpha_active())))
        .corner_radius(r_lg_cr());
    let resp = Modal::new("CONNECTIONS")
        .id("connections")
        .ctx(_ctx)
        .theme(t)
        .size(egui::vec2(220.0, 0.0))
        .anchor(Anchor::Window { pos: Some(egui::pos2(screen.right() - 240.0, 40.0)) })
        .header_style(HeaderStyle::Dialog)
        .frame_kind(FrameKind::Custom(custom_frame))
        .separator(false)
        .show(|ui| {
            ui.add_space(8.0);
            let m = 8.0;

            ui.horizontal(|ui| {
                ui.add_space(m);
                ui.add(SectionLabel::new("SERVICES").tiny().color(t.dim.gamma_multiply(0.5)));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add(SimpleBtn::new("diag").color(t.dim)).on_hover_text("ApexData diagnostics panel").clicked() {
                        _watchlist.apex_diag_open = true;
                    }
                });
            });
            ui.add_space(4.0);

            // Check connection status (non-blocking)
            let redis_ok = crate::bar_cache::is_connected();
            let ib_ok = read_account_data().map(|(a, _, _)| a.connected).unwrap_or(false);

            // ApexData status — REST health + WS connection.
            let apex_enabled = crate::apex_data::is_enabled();
            let apex_ws_ok = crate::apex_data::ws::is_connected();
            let apex_health = crate::apex_data::live_state::get_health();
            let (apex_status, apex_ok) = if !apex_enabled {
                ("OFF", false)
            } else if let Some(h) = apex_health.as_ref() {
                if h.ready && apex_ws_ok { ("OK", true) }
                else if apex_ws_ok       { ("AMBER", false) }
                else                     { ("DOWN", false) }
            } else {
                (if apex_ws_ok { "AMBER" } else { "DOWN" }, apex_ws_ok)
            };
            let apex_url_owned = crate::apex_data::apex_url();
            let apex_url_str: &str = apex_url_owned.as_str();

            let services: &[(&str, &str, bool, &str)] = &[
                ("ApexData", apex_status, apex_ok, apex_url_str),
                ("ApexIB", if ib_ok { "OK" } else { "OFF" }, ib_ok, APEXIB_URL),
                ("Redis", if redis_ok { "OK" } else { "OFF" }, redis_ok, "192.168.1.89:6379"),
                ("GPU", "DX12", true, "wgpu + egui"),
                ("Yahoo", "OK", true, "query1.finance.yahoo.com"),
            ];

            for (name, status, ok, detail) in services {
                ui.horizontal(|ui| {
                    ui.add_space(m);
                    let dot = if *ok { egui::Color32::from_rgb(46, 204, 113) } else { egui::Color32::from_rgb(231, 76, 60) };
                    ui.painter().circle_filled(egui::pos2(ui.cursor().min.x + 4.0, ui.cursor().min.y + 7.0), 3.5, dot);
                    ui.add_space(12.0);
                    ui.add(BodyLabel::new(*name).size(font_sm_tight()).monospace(true).strong(true).color(t.text));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(m);
                        status_badge(ui, status, if *ok { t.bull } else { t.bear });
                        // AMBER = transitional/connecting state — show indeterminate spinner.
                        if *status == "AMBER" {
                            ui.add_space(4.0);
                            Progress::circular_indeterminate().size(KitSize::Xs).show(ui, t);
                        }
                    });
                });
                ui.horizontal(|ui| {
                    ui.add_space(m + 12.0);
                    ui.add(BodyLabel::new(*detail).size(font_xs()).monospace(true).color(t.dim.gamma_multiply(0.45)));
                });
                ui.add_space(4.0);
            }

            ui.add_space(8.0);
        });
    if resp.closed { *conn_panel_open = false; }
}
