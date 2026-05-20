//! Connection Panel — floating modal listing service connections grouped by
//! role (Market Data, Trading, Infrastructure).
//!
//! Chrome: `Modal + HeaderStyle::Dialog` (the canonical floating-modal preset).
//! Body: `PanelSection` per group, `PanelListRow` per service.

use egui;
use super::super::style::*;
use super::super::super::gpu::*;
use super::connection_state_snapshot;
use crate::data::connectivity::ConnectionState;
use crate::ui_kit::widgets::{Indicator, IndicatorTone, PanelListRow, PanelSection};
use crate::chart_renderer::gpu::APEXIB_URL;
use crate::chart_renderer::trading::read_account_data;

/// Display tone for a connection-state badge. Maps onto `Theme` colors
/// (`bull` / `warn` / `bear` / `dim`) inside `service_row` so light themes
/// stay readable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StatusColor {
    /// Healthy — green dot, green badge text.
    Ok,
    /// Transient / recoverable — amber pulse, amber badge text.
    Warn,
    /// Terminal failure — red dot, red badge text.
    Bad,
    /// Idle / shutdown — muted dim badge.
    Dim,
}

/// Map a feed's `ConnectionState` (read from the push-driven snapshot map) into
/// a `(label, tone)` pair suitable for `service_row`.
///
/// `label` is the trailing badge text; `tone` decides the dot + badge color.
/// `Subscribed { count > 0 }` surfaces the live subscription count so a glance
/// at the panel reveals e.g. `OK (3)` for three active subs.
pub(crate) fn status_from_snapshot(name: &str) -> (String, StatusColor) {
    match connection_state_snapshot::get(name) {
        ConnectionState::Authenticated => ("OK".into(), StatusColor::Ok),
        ConnectionState::Subscribed { count } if count > 0 => {
            (format!("OK ({count})"), StatusColor::Ok)
        }
        ConnectionState::Subscribed { .. } => ("OK".into(), StatusColor::Ok),
        ConnectionState::Connecting { attempt } => {
            (format!("CONN {attempt}"), StatusColor::Warn)
        }
        ConnectionState::Backoff { attempt, .. } => {
            (format!("BACKOFF {attempt}"), StatusColor::Warn)
        }
        ConnectionState::Idle => ("IDLE".into(), StatusColor::Dim),
        ConnectionState::Failed { .. } => ("DOWN".into(), StatusColor::Bad),
        ConnectionState::ShuttingDown => ("STOP".into(), StatusColor::Dim),
    }
}

pub(crate) fn draw(_ctx: &egui::Context, _watchlist: &mut Watchlist, _panes: &mut [Chart], _ap: usize, t: &Theme, conn_panel_open: &mut bool) {
    if !*conn_panel_open { return; }

    use super::super::chrome::modal::{Modal, Anchor, HeaderStyle, FrameKind};
    let screen = _ctx.screen_rect();
    let custom_frame = egui::Frame::popup(&_ctx.style())
        .fill(t.toolbar_bg)
        .inner_margin(0.0)
        .stroke(egui::Stroke::new(stroke_std(), color_alpha(t.toolbar_border, alpha_active())))
        .corner_radius(r_lg_cr());

    // Wave 12d: ApexData status now derives from the push-based snapshot
    // populated by `connection_state_snapshot::spawn_state_listeners()` at
    // startup. The snapshot is updated synchronously from the broadcast
    // stream, so we no longer poll `ws::is_connected()` per frame here.
    // Redis and IB still come from their legacy flags — those feeds don't
    // expose a `ConnectionState` broadcast yet.
    let redis_ok = crate::bar_cache::is_connected();
    let ib_ok = read_account_data().map(|(a, _, _)| a.connected).unwrap_or(false);
    let apex_enabled = crate::apex_data::is_enabled();
    let apex_state = connection_state_snapshot::get("apex_data");
    let apex_health = crate::apex_data::live_state::get_health();
    let (apex_status, apex_ok) = if !apex_enabled {
        ("OFF", false)
    } else {
        match &apex_state {
            ConnectionState::Authenticated | ConnectionState::Subscribed { .. } => {
                // ApexData WS reports live, but the data-readiness gate
                // (live_state::get_health) is a separate signal — it only
                // flips `ready=true` after the first non-stale snapshot.
                // Keep treating "WS up but not yet ready" as AMBER.
                if apex_health.as_ref().map(|h| h.ready).unwrap_or(false) {
                    ("OK", true)
                } else {
                    ("AMBER", false)
                }
            }
            ConnectionState::Connecting { .. } | ConnectionState::Backoff { .. } => ("AMBER", false),
            ConnectionState::Idle | ConnectionState::ShuttingDown | ConnectionState::Failed { .. } => {
                ("DOWN", false)
            }
        }
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
                    // Wave 13c: crypto + signals feeds — both push their
                    // ConnectionState into `connection_state_snapshot` via the
                    // listeners spawned at startup.
                    let (crypto_status, crypto_tone) = status_from_snapshot("crypto");
                    service_row_tone(ui, t, "crypto", "Crypto",
                                     &crypto_status, crypto_tone, "crypto ws");
                    let (signals_status, signals_tone) = status_from_snapshot("signals");
                    service_row_tone(ui, t, "signals", "Signals",
                                     &signals_status, signals_tone, "signals ws");
                });
            if md_resp.action_clicked { open_diag = true; }

            // ── TRADING ────────────────────────────────────────────────────
            PanelSection::new("TRADING").show(ui, t, |ui, t| {
                service_row(ui, t, "apexib", "ApexIB",
                            if ib_ok { "OK" } else { "OFF" }, ib_ok, APEXIB_URL);
                // Wave 13c: surface the ib_ws WebSocket lifecycle from the
                // push-driven snapshot (separate from the REST account-data
                // flag above, which is `ib_ok`).
                let (ib_ws_status, ib_ws_tone) = status_from_snapshot("ib_ws");
                service_row_tone(ui, t, "ib_ws", "ApexIB WS",
                                 &ib_ws_status, ib_ws_tone, "ib websocket");
            });

            // ── INFRASTRUCTURE ─────────────────────────────────────────────
            PanelSection::new("INFRASTRUCTURE").rule(false).show(ui, t, |ui, t| {
                service_row(ui, t, "redis", "Redis",
                            if redis_ok { "OK" } else { "OFF" }, redis_ok, "192.168.1.89:6379");
                service_row(ui, t, "gpu", "GPU", "DX12", true, "wgpu + egui");
            });

            ui.add_space(gap_xs());
        });

    if open_diag { _watchlist.update_sidebar_state(|s| s.apex_diag_open = true); }
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

/// Variant of [`service_row`] that takes a typed [`StatusColor`] instead of
/// the legacy `(ok: bool, "AMBER"-sentinel)` pair. Used by Wave 13c rows that
/// derive their status from `status_from_snapshot`.
fn service_row_tone(
    ui: &mut egui::Ui,
    t: &Theme,
    id: &str,
    name: &str,
    status: &str,
    tone: StatusColor,
    detail: &str,
) {
    let status_owned = status.to_string();
    PanelListRow::new(id)
        .dense(false)
        .leading(move |ui, t| {
            let indicator = match tone {
                StatusColor::Warn => Indicator::pulsing().tone(IndicatorTone::Warn),
                StatusColor::Ok => Indicator::dot().tone(IndicatorTone::Bull),
                StatusColor::Bad => Indicator::dot().tone(IndicatorTone::Bear),
                StatusColor::Dim => Indicator::dot().tone(IndicatorTone::Neutral),
            };
            indicator.show(ui, t);
        })
        .primary(name)
        .secondary(detail)
        .trailing(move |ui, t| {
            let color = match tone {
                StatusColor::Ok => t.bull,
                StatusColor::Warn => t.warn,
                StatusColor::Bad => t.bear,
                StatusColor::Dim => t.dim,
            };
            status_badge(ui, &status_owned, color);
        })
        .show(ui, t);
}
