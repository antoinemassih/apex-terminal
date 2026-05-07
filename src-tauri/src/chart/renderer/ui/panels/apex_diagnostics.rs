//! ApexData diagnostics panel — live view of config, connection health,
//! REST stats, WS subscriptions, chain cache, and recent request history.
//!
//! Opens via `watchlist.apex_diag_open`. Read-only; all data pulled from the
//! running apex_data module state.

use egui;
use super::super::style::*;
use super::super::super::gpu::{Watchlist, Theme};
use super::super::widgets::headers::PanelHeaderWithClose;
use super::super::widgets::text::SectionLabelSize;
use crate::ui_kit::widgets::{Alert, Progress};
use crate::ui_kit::widgets::tokens::Size as KitSize;

pub(crate) fn draw(ctx: &egui::Context, watchlist: &mut Watchlist, t: &Theme) {
    if !watchlist.apex_diag_open { return; }

    let screen = ctx.screen_rect();
    let w = 620.0_f32;
    let h = (screen.height() * 0.85).min(720.0);

    egui::Window::new("apex_diagnostics")
        .fixed_pos(egui::pos2(screen.center().x - w / 2.0, 60.0))
        .fixed_size(egui::vec2(w, h))
        .title_bar(false)
        .frame(super::super::widgets::frames::PopupFrame::new().colors(t.toolbar_bg, t.toolbar_border).ctx(ctx).build())
        .show(ctx, |ui| {
            let closed = PanelHeaderWithClose::new("APEX DATA DIAGNOSTICS")
                .title_size(SectionLabelSize::Md)
                .theme(t)
                .show_with(ui, |ui| {
                    if ui.small_button("reset breaker").clicked() {
                        crate::apex_data::rest::reset_breaker();
                    }
                });
            if closed { watchlist.apex_diag_open = false; }
            ui.separator();

            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                section_config(ui, t);
                ui.add_space(gap_lg());
                section_connection(ui, t);
                ui.add_space(gap_lg());
                section_rest_stats(ui, t);
                ui.add_space(gap_lg());
                section_ws_subs(ui, t);
                ui.add_space(gap_lg());
                section_chain_cache(ui, t);
                ui.add_space(gap_lg());
                section_recent_calls(ui, t);
            });
        });
}

// ────────────────────────────────────────────────────────────────────────────

fn hdr(ui: &mut egui::Ui, label: &str, t: &Theme) {
    ui.add(super::super::widgets::text::MonospaceCode::new(label).sm().color(t.dim).gamma(0.7).strong(true));
    ui.separator();
}

fn kv(ui: &mut egui::Ui, k: &str, v: &str, t: &Theme, value_color: Option<egui::Color32>) {
    ui.horizontal(|ui| {
        ui.add(super::super::widgets::text::MonospaceCode::new(k).color(t.dim));
        ui.add_space(gap_xs());
        ui.add(super::super::widgets::text::MonospaceCode::new(v).sm().color(value_color.unwrap_or(t.text)).strong(true));
    });
}

fn pill(ui: &mut egui::Ui, text: &str, color: egui::Color32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(60.0, 14.0), egui::Sense::hover());
    ui.painter().rect_filled(rect, current().r_md, color_alpha(color, 50));
    ui.painter().rect_stroke(rect, current().r_md, egui::Stroke::new(current().stroke_std, color), egui::StrokeKind::Inside);
    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER,
        text, egui::FontId::proportional(font_sm()), color);
}

// ────────────────────────────────────────────────────────────────────────────

fn section_config(ui: &mut egui::Ui, t: &Theme) {
    hdr(ui, "CONFIG", t);
    let url = crate::apex_data::apex_url();
    let lan = crate::apex_data::config::apex_lan_ip().unwrap_or_else(|| "—".into());
    let token = crate::apex_data::apex_token().map(|_| "set".to_string()).unwrap_or_else(|| "none".into());
    let enabled = crate::apex_data::is_enabled();
    kv(ui, "enabled",  if enabled { "yes" } else { "no" }, t,
       Some(if enabled { t.bull } else { t.bear }));
    kv(ui, "base URL", &url, t, None);
    kv(ui, "LAN IP  ", &lan, t, None);
    kv(ui, "token   ", &token, t, None);
    kv(ui, "log file", &crate::apex_data::debug_log::path_string(), t, Some(t.dim));
}

fn section_connection(ui: &mut egui::Ui, t: &Theme) {
    hdr(ui, "CONNECTION", t);
    let ws = crate::apex_data::ws::is_connected();
    let (fails, cooldown) = crate::apex_data::rest::breaker_snapshot();
    ui.horizontal(|ui| {
        ui.add(super::super::widgets::text::MonospaceCode::new("WS").color(t.dim));
        pill(ui, if ws { "connected" } else { "disconnected" },
             if ws { t.bull } else { t.bear });
    });
    ui.horizontal(|ui| {
        ui.add(super::super::widgets::text::MonospaceCode::new("breaker").color(t.dim));
        if cooldown.is_some() {
            pill(ui, "open", t.bear);
        } else {
            pill(ui, "closed", t.bull);
        }
        ui.add(super::super::widgets::text::MonospaceCode::new(&format!("fails={fails}")).color(t.dim));
    });

    // When breaker is open, surface it as a banner with cooldown progress.
    // 30s is the COOLDOWN constant in apex_data::rest.
    if let Some(remaining) = cooldown {
        const COOLDOWN_SECS: f32 = 30.0;
        let remaining_s = remaining.as_secs_f32();
        let elapsed_frac = (1.0 - (remaining_s / COOLDOWN_SECS)).clamp(0.0, 1.0);
        Alert::warn(format!(
            "REST circuit-breaker open after {fails} consecutive failures. \
             Probing again in {}s.",
            remaining.as_secs()
        ))
        .title("Circuit Breaker Open")
        .show(ui, t);
        ui.add_space(gap_xs());
        Progress::linear(elapsed_frac).size(KitSize::Sm).show(ui, t);
    }

    if let Some(h) = crate::apex_data::live_state::get_health() {
        if h.ready {
            ui.horizontal(|ui| {
                ui.add(super::super::widgets::text::MonospaceCode::new("health").color(t.dim));
                pill(ui, "ready", t.bull);
                ui.add(super::super::widgets::text::MonospaceCode::new(&format!("tick age {}ms, redis={} questdb={} feeds {}/{}",
                    h.tick_age_ms, h.redis, h.questdb, h.feeds_connected, h.feeds_total)).color(t.dim));
            });
        } else {
            Alert::warn(format!(
                "tick age {}ms, redis={} questdb={} feeds {}/{}",
                h.tick_age_ms, h.redis, h.questdb, h.feeds_connected, h.feeds_total
            ))
            .title("Health Not Ready")
            .show(ui, t);
        }
    } else {
        kv(ui, "health ", "(no response yet)", t, Some(t.dim));
    }
}

fn section_rest_stats(ui: &mut egui::Ui, t: &Theme) {
    hdr(ui, "REST STATS", t);
    let (ok, http_err, net_err, parse_err, skipped, _) = crate::apex_data::rest::stats_snapshot();
    let total = ok + http_err + net_err + parse_err + skipped;
    let pct = |n: u64| if total == 0 { 0.0 } else { 100.0 * n as f64 / total as f64 };
    ui.horizontal_wrapped(|ui| {
        kv(ui, "total",  &format!("{total}"), t, None); ui.add_space(gap_lg());
        kv(ui, "ok",     &format!("{ok} ({:.0}%)",        pct(ok)), t, Some(t.bull)); ui.add_space(gap_lg());
        kv(ui, "http",   &format!("{http_err} ({:.0}%)",  pct(http_err)), t, Some(t.warn)); ui.add_space(gap_lg());
        kv(ui, "net",    &format!("{net_err} ({:.0}%)",   pct(net_err)), t, Some(t.bear)); ui.add_space(gap_lg());
        kv(ui, "parse",  &format!("{parse_err} ({:.0}%)", pct(parse_err)), t, Some(t.bear)); ui.add_space(gap_lg());
        kv(ui, "skip",   &format!("{skipped} ({:.0}%)",   pct(skipped)), t, Some(t.dim));
    });
    if total > 0 {
        ui.add_space(gap_xs());
        ui.horizontal(|ui| {
            ui.add(super::super::widgets::text::MonospaceCode::new("ok rate").color(t.dim));
            ui.add_space(gap_xs());
            Progress::linear((pct(ok) / 100.0) as f32).size(KitSize::Sm).show(ui, t);
        });
    }
}

fn section_ws_subs(ui: &mut egui::Ui, t: &Theme) {
    hdr(ui, "WS", t);
    ui.add(super::super::widgets::text::MonospaceCode::new("(subscription counts tracked client-side; see 'chain cache' below for live state)")
        .color(t.dim.gamma_multiply(0.6)));
}

fn section_chain_cache(ui: &mut egui::Ui, t: &Theme) {
    hdr(ui, "CHAIN CACHE", t);
    let summary = crate::apex_data::live_state::chain_summary();
    if summary.is_empty() {
        ui.add(super::super::widgets::text::MonospaceCode::new("  (empty — no chains cached yet)").color(t.dim));
        return;
    }
    ui.horizontal(|ui| {
        ui.add(super::super::widgets::text::MonospaceCode::new("underlying").color(t.dim.gamma_multiply(0.6)));
        ui.add_space(60.0);
        ui.add(super::super::widgets::text::MonospaceCode::new("rows").color(t.dim.gamma_multiply(0.6)));
        ui.add_space(gap_2xl());
        ui.add(super::super::widgets::text::MonospaceCode::new("last update").color(t.dim.gamma_multiply(0.6)));
    });
    for (ul, rows, age_s) in summary {
        let age_color = if age_s < 10 { t.bull }
                       else if age_s < 60 { t.warn }
                       else { t.bear };
        ui.horizontal(|ui| {
            ui.add(super::super::widgets::text::MonospaceCode::new(&ul).sm().color(t.text).strong(true));
            ui.add_space(120.0 - 10.0 * ul.len() as f32);
            ui.add(super::super::widgets::text::MonospaceCode::new(&format!("{rows}")).color(t.text));
            ui.add_space(24.0);
            ui.add(super::super::widgets::text::MonospaceCode::new(&format!("{age_s}s ago")).color(age_color));
        });
    }
}

fn section_recent_calls(ui: &mut egui::Ui, t: &Theme) {
    hdr(ui, "RECENT REST CALLS", t);
    let (_, _, _, _, _, recent) = crate::apex_data::rest::stats_snapshot();
    if recent.is_empty() {
        ui.add(super::super::widgets::text::MonospaceCode::new("  (none yet)").color(t.dim));
        return;
    }
    for call in recent.iter().rev().take(25) {
        let color = match call.outcome {
            "ok"    => t.bull,
            "http"  => t.warn,
            "parse" => t.bear,
            "err"   => t.bear,
            _       => t.dim,
        };
        let label = match call.outcome {
            "ok"    => format!("{} {}", call.status, call.outcome),
            "http"  => format!("{}", call.status),
            "err"   => "net err".into(),
            "parse" => "parse err".into(),
            "skip"  => if call.status == 1 { "breaker".into() } else { "skip".into() },
            _       => call.outcome.into(),
        };
        ui.horizontal(|ui| {
            let (pill_rect, _) = ui.allocate_exact_size(egui::vec2(62.0, 14.0), egui::Sense::hover());
            ui.painter().rect_filled(pill_rect, current().r_sm, color_alpha(color, 40));
            ui.painter().text(pill_rect.center(), egui::Align2::CENTER_CENTER,
                &label, egui::FontId::monospace(super::super::style::font_xs()), color);
            ui.add_space(gap_sm());
            ui.add(super::super::widgets::text::MonospaceCode::new(&format!("{}ms", call.ms)).color(t.dim));
            ui.add_space(gap_sm());
            ui.add(super::super::widgets::text::MonospaceCode::new(&call.path).color(t.text.gamma_multiply(0.85)));
        });
    }
}
