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
use super::super::chrome::modal::{Modal, Anchor, HeaderStyle, FrameKind};
use crate::ui_kit::widgets::{Alert, PanelEmpty, PanelKeyValueRow, PanelSection, PanelTone, Progress};
use crate::ui_kit::widgets::tokens::Size as KitSize;
use crate::ui_kit::layout::{Align as FlexAlign, Item, Surface};
use crate::ui_kit::scale::Space;
use crate::ui_kit::style::measure_mono;

pub(crate) fn draw(ctx: &egui::Context, watchlist: &mut Watchlist, t: &Theme) {
    if !watchlist.apex_diag_open { return; }

    let screen = ctx.screen_rect();
    let w = 620.0_f32;
    let h = (screen.height() * 0.85).min(720.0);

    let mut reset_breaker = false;

    let resp = Modal::new("APEX DATA DIAGNOSTICS")
        .id("apex_diagnostics")
        .ctx(ctx)
        .theme(t)
        .size(egui::vec2(w, h))
        .anchor(Anchor::Window { pos: Some(egui::pos2(screen.center().x - w / 2.0, 60.0)) })
        .frame_kind(FrameKind::DialogWindow)
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

/// Status-pill geometry — unchanged from the hand-rolled rows this module used
/// to build; named so the flex spec and the painter agree on one number.
const PILL_W: f32 = 72.0;
const PILL_H: f32 = 14.0;

/// Inline status pill — small tinted rounded rectangle. Kept local since the
/// shared `Tag` widget renders too large for these dense diagnostics rows.
fn pill(ui: &mut egui::Ui, text: &str, color: egui::Color32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(PILL_W, PILL_H), egui::Sense::hover());
    ui.painter().rect_filled(rect, current().r_md, color_alpha(color, 50));
    ui.painter().rect_stroke(rect, current().r_md, egui::Stroke::new(current().stroke_std, color), egui::StrokeKind::Inside);
    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER,
        text, egui::FontId::proportional(font_xs()), color);
}

// ── Row layout ───────────────────────────────────────────────────────────────
//
// Every non-`PanelKeyValueRow` row in this panel used to be
// `ui.horizontal { add_space(gap_md); label; add_space(gap_sm); …; }` — a
// leading indent and a gutter, both hand-written, both drifting between
// sections. They are now `Surface` rows: the indent is the first item's
// `margin_start(Space::Md)` and every gutter is the container `gap(Space::Sm)`.
//
// Each row reserves its own strip first. `Surface` solves inside whatever box
// it is handed, and inside this modal's `ScrollArea` that box is the entire
// remaining panel height — so a `Center`-aligned row would float its children
// down the middle of the panel. Allocating the strip up front is the same thing
// `PanelSection`'s own header does, for the same reason.

/// Reserve a full-width strip `h` tall and return a `Ui` scoped to it.
fn strip(ui: &mut egui::Ui, h: f32) -> egui::Ui {
    let (_id, rect) = ui.allocate_space(egui::vec2(ui.available_width(), h));
    ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    )
}

/// A `mono_xs` cell rendered into its own solved rect. `Extend` because the
/// rect was sized from this exact galley — wrapping would be a rounding
/// artefact, and it reproduces `ui.horizontal`'s no-wrap behaviour.
fn cell(ui: &mut egui::Ui, text: &str, color: egui::Color32) {
    ui.add(
        egui::Label::new(
            egui::RichText::new(text).monospace().size(font_xs()).color(color),
        )
        .wrap_mode(egui::TextWrapMode::Extend),
    );
}

/// One `label · pill · note` status row.
fn status_row(
    ui: &mut egui::Ui,
    t: &Theme,
    label: &str,
    pill_text: &str,
    pill_color: egui::Color32,
    note: Option<String>,
) {
    let label_sz = measure_mono(ui, label, font_xs());
    let note_sz = note.as_deref().map(|n| measure_mono(ui, n, font_xs()));
    let h = label_sz.y.max(PILL_H).max(note_sz.map(|s| s.y).unwrap_or(0.0));
    let mut row = strip(ui, h);

    let mut s = Surface::row()
        .gap(Space::Sm)
        .align(FlexAlign::Center)
        .item(Item::fixed(label_sz.x).cross(label_sz.y).margin_start(Space::Md.px()))
        .item(Item::fixed(PILL_W).cross(PILL_H));
    if let Some(sz) = note_sz {
        s = s.item(Item::grow(1.0).cross(sz.y));
    }
    s.show(&mut row, t, |idx, ui| match idx {
        0 => cell(ui, label, color_muted(t.dim)),
        1 => pill(ui, pill_text, pill_color),
        _ => {
            if let Some(n) = note.as_deref() {
                cell(ui, n, t.dim);
            }
        }
    });
}

/// A single indented note line — the `gap_md` indent is the item's
/// `margin_start`, not an `add_space`.
fn note_row(ui: &mut egui::Ui, t: &Theme, text: &str) {
    let sz = measure_mono(ui, text, font_xs());
    let mut row = strip(ui, sz.y);
    Surface::row()
        .align(FlexAlign::Center)
        .item(Item::grow(1.0).cross(sz.y).margin_start(Space::Md.px()))
        .show(&mut row, t, |_idx, ui| cell(ui, text, color_muted(t.dim)));
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

    status_row(
        ui, t, "WS",
        if ws { "connected" } else { "disconnected" },
        if ws { t.bull } else { t.bear },
        None,
    );
    let (breaker_text, breaker_col) = if cooldown.is_some() {
        ("open", t.bear)
    } else {
        ("closed", t.bull)
    };
    status_row(ui, t, "breaker", breaker_text, breaker_col, Some(format!("fails={fails}")));

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
            status_row(ui, t, "health", "ready", t.bull, Some(format!(
                "tick age {}ms, redis={} questdb={} feeds {}/{}",
                h.tick_age_ms, h.redis, h.questdb, h.feeds_connected, h.feeds_total
            )));
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
        // `label · bar` — the bar takes the slack instead of being pushed by an
        // `add_space`. Progress::linear(Sm) paints a 4px-tall track.
        const BAR_H: f32 = 4.0;
        let label_sz = measure_mono(ui, "ok rate", font_xs());
        let mut row = strip(ui, label_sz.y.max(BAR_H));
        Surface::row()
            .gap(Space::Sm)
            .align(FlexAlign::Center)
            .item(Item::fixed(label_sz.x).cross(label_sz.y).margin_start(Space::Md.px()))
            .item(Item::grow(1.0).cross(BAR_H))
            .show(&mut row, t, |idx, ui| {
                if idx == 0 {
                    cell(ui, "ok rate", color_muted(t.dim));
                } else {
                    Progress::linear((pct(ok) / 100.0) as f32).size(KitSize::Sm).show(ui, t);
                }
            });
    }
}

fn section_ws_subs(ui: &mut egui::Ui, t: &Theme) {
    note_row(
        ui, t,
        "(subscription counts tracked client-side; see 'chain cache' below for live state)",
    );
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

/// NOT migrated to `Surface` on purpose: this is a repeating table body (up to
/// 25 rows, re-rendered every frame the modal is open), and the flex engine
/// builds and solves one `TaffyTree` per `show()`. `ui_kit::layout::flex`'s
/// module docs draw the line exactly here — flexbox is for panel chrome and
/// forms, not row loops. The row carries no cross-row alignment requirement
/// either: it is a painter-placed pill plus two trailing labels.
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
