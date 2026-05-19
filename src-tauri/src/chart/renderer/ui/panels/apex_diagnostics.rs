//! ApexData diagnostics panel — live view of config, connection health,
//! REST stats, WS subscriptions, chain cache, and recent request history.
//!
//! Opens via `watchlist.apex_diag_open`. Read-only; all data pulled from the
//! running apex_data module state.
//!
//! Chrome: `Modal + HeaderStyle::Dialog`. Body: `PanelSection` groups with
//! `PanelKeyValueRow` for tagged metric stacks (Bull/Bear/Warn tones).

use egui;
use super::super::style::*;
use super::super::super::gpu::{Watchlist, Theme};
use super::super::widgets::modal::{Modal, Anchor, HeaderStyle, FrameKind};
use crate::ui_kit::widgets::{Alert, PanelEmpty, PanelKeyValueRow, PanelSection, PanelTone, Progress};
use crate::ui_kit::widgets::tokens::Size as KitSize;

pub(crate) fn draw(ctx: &egui::Context, watchlist: &mut Watchlist, t: &Theme) {
    if !watchlist.apex_diag_open { return; }

    let screen = ctx.screen_rect();
    let w = 620.0_f32;
    let h = (screen.height() * 0.85).min(720.0);

    let frame = super::super::widgets::frames::PopupFrame::new()
        .colors(t.toolbar_bg, t.toolbar_border)
        .ctx(ctx)
        .build();

    let mut reset_breaker = false;

    let resp = Modal::new("APEX DATA DIAGNOSTICS")
        .id("apex_diagnostics")
        .ctx(ctx)
        .theme(t)
        .size(egui::vec2(w, h))
        .anchor(Anchor::Window { pos: Some(egui::pos2(screen.center().x - w / 2.0, 60.0)) })
        .frame_kind(FrameKind::Custom(frame))
        .header_style(HeaderStyle::Dialog)
        .separator(false)
        .show(|ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                let r = PanelSection::new("CONFIG")
                    .action("reset breaker", PanelTone::Warn)
                    .show(ui, t, section_config);
                if r.action_clicked { reset_breaker = true; }

                PanelSection::new("CONNECTION").show(ui, t, section_connection);
                PanelSection::new("REST STATS").show(ui, t, section_rest_stats);
                PanelSection::new("WS").show(ui, t, section_ws_subs);
                PanelSection::new("CHAIN CACHE").show(ui, t, section_chain_cache);
                PanelSection::new("RECENT REST CALLS").rule(false).show(ui, t, section_recent_calls);
            });
        });

    if reset_breaker { crate::apex_data::rest::reset_breaker(); }
    if resp.closed { watchlist.update_sidebar_state(|s| s.apex_diag_open = false); }
}

// ────────────────────────────────────────────────────────────────────────────

/// Inline status pill — small tinted rounded rectangle. Kept local since the
/// shared `Tag` widget renders too large for these dense diagnostics rows.
fn pill(ui: &mut egui::Ui, text: &str, color: egui::Color32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(72.0, 14.0), egui::Sense::hover());
    ui.painter().rect_filled(rect, current().r_md, color_alpha(color, 50));
    ui.painter().rect_stroke(rect, current().r_md, egui::Stroke::new(current().stroke_std, color), egui::StrokeKind::Inside);
    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER,
        text, egui::FontId::proportional(font_xs()), color);
}

// ────────────────────────────────────────────────────────────────────────────

fn section_config(ui: &mut egui::Ui, t: &Theme) {
    let url = crate::apex_data::apex_url();
    let lan = crate::apex_data::config::apex_lan_ip().unwrap_or_else(|| "—".into());
    let token = crate::apex_data::apex_token().map(|_| "set".to_string()).unwrap_or_else(|| "none".into());
    let enabled = crate::apex_data::is_enabled();

    PanelKeyValueRow::new("enabled", if enabled { "yes" } else { "no" })
        .tone(if enabled { PanelTone::Bull } else { PanelTone::Bear })
        .show(ui, t);
    PanelKeyValueRow::new("base URL", url).show(ui, t);
    PanelKeyValueRow::new("LAN IP",   lan).show(ui, t);
    PanelKeyValueRow::new("token",    token).show(ui, t);
    PanelKeyValueRow::new("log file", crate::apex_data::debug_log::path_string())
        .tone(PanelTone::Default)
        .show(ui, t);
}

fn section_connection(ui: &mut egui::Ui, t: &Theme) {
    let ws = crate::apex_data::ws::is_connected();
    let (fails, cooldown) = crate::apex_data::rest::breaker_snapshot();

    ui.horizontal(|ui| {
        ui.add_space(gap_md());
        ui.label(egui::RichText::new("WS").monospace().size(font_xs()).color(color_muted(t.dim)));
        ui.add_space(gap_sm());
        pill(ui, if ws { "connected" } else { "disconnected" },
             if ws { t.bull } else { t.bear });
    });
    ui.horizontal(|ui| {
        ui.add_space(gap_md());
        ui.label(egui::RichText::new("breaker").monospace().size(font_xs()).color(color_muted(t.dim)));
        ui.add_space(gap_sm());
        if cooldown.is_some() {
            pill(ui, "open", t.bear);
        } else {
            pill(ui, "closed", t.bull);
        }
        ui.add_space(gap_sm());
        ui.label(egui::RichText::new(format!("fails={fails}")).monospace().size(font_xs()).color(t.dim));
    });

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
                ui.add_space(gap_md());
                ui.label(egui::RichText::new("health").monospace().size(font_xs()).color(color_muted(t.dim)));
                ui.add_space(gap_sm());
                pill(ui, "ready", t.bull);
                ui.add_space(gap_sm());
                ui.label(egui::RichText::new(format!(
                    "tick age {}ms, redis={} questdb={} feeds {}/{}",
                    h.tick_age_ms, h.redis, h.questdb, h.feeds_connected, h.feeds_total
                )).monospace().size(font_xs()).color(t.dim));
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
        PanelKeyValueRow::new("health", "(no response yet)")
            .tone(PanelTone::Default)
            .show(ui, t);
    }
}

fn section_rest_stats(ui: &mut egui::Ui, t: &Theme) {
    let (ok, http_err, net_err, parse_err, skipped, _) = crate::apex_data::rest::stats_snapshot();
    let total = ok + http_err + net_err + parse_err + skipped;
    let pct = |n: u64| if total == 0 { 0.0 } else { 100.0 * n as f64 / total as f64 };

    PanelKeyValueRow::new("total", format!("{total}")).show(ui, t);
    PanelKeyValueRow::new("ok",    format!("{ok}"))       .meta(format!("{:.0}%", pct(ok)))       .tone(PanelTone::Bull).show(ui, t);
    PanelKeyValueRow::new("http",  format!("{http_err}")) .meta(format!("{:.0}%", pct(http_err))) .tone(PanelTone::Warn).show(ui, t);
    PanelKeyValueRow::new("net",   format!("{net_err}"))  .meta(format!("{:.0}%", pct(net_err)))  .tone(PanelTone::Bear).show(ui, t);
    PanelKeyValueRow::new("parse", format!("{parse_err}")).meta(format!("{:.0}%", pct(parse_err))).tone(PanelTone::Bear).show(ui, t);
    PanelKeyValueRow::new("skip",  format!("{skipped}"))  .meta(format!("{:.0}%", pct(skipped)))  .tone(PanelTone::Default).show(ui, t);

    if total > 0 {
        ui.add_space(gap_xs());
        ui.horizontal(|ui| {
            ui.add_space(gap_md());
            ui.label(egui::RichText::new("ok rate").monospace().size(font_xs()).color(color_muted(t.dim)));
            ui.add_space(gap_sm());
            Progress::linear((pct(ok) / 100.0) as f32).size(KitSize::Sm).show(ui, t);
        });
    }
}

fn section_ws_subs(ui: &mut egui::Ui, t: &Theme) {
    ui.horizontal(|ui| {
        ui.add_space(gap_md());
        ui.label(egui::RichText::new(
            "(subscription counts tracked client-side; see 'chain cache' below for live state)"
        ).monospace().size(font_xs()).color(color_muted(t.dim)));
    });
}

fn section_chain_cache(ui: &mut egui::Ui, t: &Theme) {
    let summary = crate::apex_data::live_state::chain_summary();
    if summary.is_empty() {
        PanelEmpty::new("No chains cached yet")
            .min_height(48.0)
            .show(ui, t);
        return;
    }
    for (ul, rows, age_s) in summary {
        let tone = if age_s < 10 { PanelTone::Bull }
                   else if age_s < 60 { PanelTone::Warn }
                   else { PanelTone::Bear };
        PanelKeyValueRow::new(&ul, format!("{age_s}s ago"))
            .meta(format!("{rows} rows"))
            .tone(tone)
            .show(ui, t);
    }
}

fn section_recent_calls(ui: &mut egui::Ui, t: &Theme) {
    let (_, _, _, _, _, recent) = crate::apex_data::rest::stats_snapshot();
    if recent.is_empty() {
        PanelEmpty::new("No recent calls")
            .min_height(48.0)
            .show(ui, t);
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
            ui.add_space(gap_md());
            let (pill_rect, _) = ui.allocate_exact_size(egui::vec2(62.0, 14.0), egui::Sense::hover());
            ui.painter().rect_filled(pill_rect, current().r_sm, color_alpha(color, 40));
            ui.painter().text(pill_rect.center(), egui::Align2::CENTER_CENTER,
                &label, egui::FontId::monospace(font_xs()), color);
            ui.add_space(gap_sm());
            ui.label(egui::RichText::new(format!("{}ms", call.ms)).monospace().size(font_xs()).color(t.dim));
            ui.add_space(gap_sm());
            ui.label(egui::RichText::new(&call.path).monospace().size(font_xs()).color(color_subtle(t.text)));
        });
    }
}
