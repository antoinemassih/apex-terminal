//! Connection Panel — floating modal listing service connections grouped by
//! role (Market Data, Trading, Infrastructure).
//!
//! Chrome: `Modal + HeaderStyle::Dialog` (the canonical floating-modal preset).
//! Body: `PanelSection` per group, `PanelListRow` per service.

use egui;
use super::super::style::*;
use super::super::super::gpu::*;
use crate::ui_kit::widgets::{Indicator, IndicatorTone, PanelListRow, PanelSection};
use crate::chart_renderer::gpu::APEXIB_URL;
use crate::chart_renderer::trading::read_account_data;

pub(crate) fn draw(_ctx: &egui::Context, _watchlist: &mut Watchlist, _panes: &mut [Chart], _ap: usize, t: &Theme, conn_panel_open: &mut bool) {
    if !*conn_panel_open { return; }

    use super::super::widgets::modal::{Modal, Anchor, HeaderStyle, FrameKind};
    let screen = _ctx.screen_rect();
    let custom_frame = egui::Frame::popup(&_ctx.style())
        .fill(t.toolbar_bg)
        .inner_margin(0.0)
        .stroke(egui::Stroke::new(stroke_std(), color_alpha(t.toolbar_border, alpha_active())))
        .corner_radius(r_lg_cr());

    // Snapshot live status once per frame.
    let redis_ok = crate::bar_cache::is_connected();
    let ib_ok = read_account_data().map(|(a, _, _)| a.connected).unwrap_or(false);
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

    let mut open_diag = false;

    let resp = Modal::new("CONNECTIONS")
        .id("connections")
        .ctx(_ctx)
        .theme(t)
        .size(egui::vec2(260.0, 0.0))
        .anchor(Anchor::Window { pos: Some(egui::pos2(screen.right() - 280.0, 40.0)) })
        .header_style(HeaderStyle::Dialog)
        .frame_kind(FrameKind::Custom(custom_frame))
        .separator(false)
        .show(|ui| {
            ui.add_space(gap_sm());

            // ── MARKET DATA ────────────────────────────────────────────────
            let market_data: &[(&str, &str, &str, bool, &str)] = &[
                ("apexdata", "ApexData", apex_status, apex_ok, apex_url_owned.as_str()),
                ("yahoo",    "Yahoo",    "OK",        true,    "query1.finance.yahoo.com"),
            ];
            let md_resp = PanelSection::new("MARKET DATA")
                .action("diag", crate::ui_kit::widgets::PanelTone::Accent)
                .show(ui, t, |ui, t| {
                    for (id, name, status, ok, detail) in market_data {
                        service_row(ui, t, id, name, status, *ok, detail);
                    }
                });
            if md_resp.action_clicked { open_diag = true; }

            // ── TRADING ────────────────────────────────────────────────────
            PanelSection::new("TRADING").show(ui, t, |ui, t| {
                service_row(ui, t, "apexib", "ApexIB",
                            if ib_ok { "OK" } else { "OFF" }, ib_ok, APEXIB_URL);
            });

            // ── INFRASTRUCTURE ─────────────────────────────────────────────
            PanelSection::new("INFRASTRUCTURE").rule(false).show(ui, t, |ui, t| {
                service_row(ui, t, "redis", "Redis",
                            if redis_ok { "OK" } else { "OFF" }, redis_ok, "192.168.1.89:6379");
                service_row(ui, t, "gpu", "GPU", "DX12", true, "wgpu + egui");
            });

            ui.add_space(gap_xs());
        });

    if open_diag { _watchlist.apex_diag_open = true; }
    if resp.closed { *conn_panel_open = false; }
}

/// Single service connection — status dot leading, name + endpoint stacked,
/// status badge trailing.
fn service_row(ui: &mut egui::Ui, t: &Theme, id: &str, name: &str, status: &str, ok: bool, detail: &str) {
    let status_owned = status.to_string();
    let pulsing = status == "AMBER";
    PanelListRow::new(id)
        .dense(false)
        .leading(move |ui, t| {
            let indicator = if pulsing {
                Indicator::pulsing().tone(IndicatorTone::Warn)
            } else if ok {
                Indicator::dot().tone(IndicatorTone::Bull)
            } else {
                Indicator::dot().tone(IndicatorTone::Bear)
            };
            indicator.show(ui, t);
        })
        .primary(name)
        .secondary(detail)
        .trailing(move |ui, t| {
            let color = if pulsing { t.warn } else if ok { t.bull } else { t.bear };
            status_badge(ui, &status_owned, color);
        })
        .show(ui, t);
}
