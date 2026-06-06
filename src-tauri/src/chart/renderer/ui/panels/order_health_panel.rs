//! Order System Health — operator-facing observability for the order
//! subsystem. Surfaces submit latency, reject rates, top reject reasons,
//! `Unknown`-state spikes, active state counts, and broker contact age so
//! issues are visible without grepping logs.
//!
//! Data sources (all lock-free on the render path):
//!   - `journal::tail()` — recent WAL events (Attempt/Ack/Fail/StateChg/...)
//!   - `order_manager::orders_snapshot()` — published Arc, no manager lock
//!   - `order_manager::broker_contact_age_secs()` — single-mutex peek
//!
//! Metrics are recomputed at most once per second and cached so the panel
//! is cheap to render every frame.
//!
//! Chrome: `Modal + HeaderStyle::Dialog`. Body: `PanelSection` groups with
//! `PanelKeyValueRow` (kv pairs) and `PanelListRow::columns` (state rows).

use std::collections::HashMap;
use crate::ui_kit::sx::Tone;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

use egui;

use super::super::style::*;
use super::super::super::gpu::{Watchlist, Theme};
use crate::ui_kit::widgets::{
    PanelKeyValueRow, PanelListRow, PanelSection, PanelTone,
    modal::{Modal, Anchor, HeaderStyle, FrameKind},
};
use crate::ui_kit::widgets::panel_list_row::Column;
use super::super::components::text::MonospaceCode;
use crate::chart_renderer::trading::order_manager::{self, OrderState};
use crate::chart_renderer::trading::journal::{self, JournalEvent, AttemptKind};

const FIVE_MIN_MS: u64 = 5 * 60 * 1000;
const ONE_HOUR_MS: u64 = 60 * 60 * 1000;
const TAIL_N: usize = 1024;

/// Aggregated point-in-time metrics. Cheap to clone (small ints + a tiny vec).
#[derive(Clone, Default)]
struct Metrics {
    // Last 5 minutes
    avg_pending_ack_ms: Option<u64>, // None if no Attempt→Ack pairs in window
    submit_attempts_5m: u32,
    submit_fails_5m: u32,
    top_rejects: Vec<(String, u32)>, // top 5 reject reasons by count

    // Last hour
    unknown_terminal_1h: u32, // orders that ended in `Unknown`

    // Active counts (point in time)
    active_pending_submit: u32,
    active_working: u32,
    active_partial_fill: u32,
    active_pending_cancel: u32,
    active_pending_modify: u32,
    active_unknown: u32,

    // Broker contact (seconds since last successful call)
    broker_age_secs: Option<u64>,
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn compute_metrics() -> Metrics {
    let mut m = Metrics::default();
    let events = journal::tail(TAIL_N);
    let now = now_ms();
    let win5 = now.saturating_sub(FIVE_MIN_MS);
    let win1h = now.saturating_sub(ONE_HOUR_MS);

    // ── Submit metrics over the last 5 minutes ─────────────────────────
    // Pair Attempt(Submit) -> first Ack on the same client_id to compute
    // pending-ack durations. Track Fail counts separately for reject rate.
    let mut submit_attempt_ts: HashMap<&str, u64> = HashMap::new();
    let mut acked_durations: Vec<u64> = Vec::new();
    let mut reject_reasons: HashMap<String, u32> = HashMap::new();

    for ev in &events {
        match ev {
            JournalEvent::Attempt { kind: AttemptKind::Submit, ts_ms, client_id, .. }
                if *ts_ms >= win5 =>
            {
                m.submit_attempts_5m += 1;
                submit_attempt_ts.entry(client_id.as_str()).or_insert(*ts_ms);
            }
            JournalEvent::Ack { client_id, ts_ms, .. } if *ts_ms >= win5 => {
                if let Some(start) = submit_attempt_ts.get(client_id.as_str()) {
                    if *ts_ms >= *start {
                        acked_durations.push(*ts_ms - *start);
                    }
                }
            }
            JournalEvent::Fail { reason, ts_ms, .. } if *ts_ms >= win5 => {
                m.submit_fails_5m += 1;
                *reject_reasons.entry(reason.clone()).or_insert(0) += 1;
            }
            _ => {}
        }
    }

    if !acked_durations.is_empty() {
        let sum: u64 = acked_durations.iter().sum();
        m.avg_pending_ack_ms = Some(sum / acked_durations.len() as u64);
    }

    // Top 5 reject reasons by count.
    let mut rejects: Vec<(String, u32)> = reject_reasons.into_iter().collect();
    rejects.sort_by(|a, b| b.1.cmp(&a.1));
    rejects.truncate(5);
    m.top_rejects = rejects;

    // ── Orders ending in `Unknown` over the last hour ──────────────────
    // Count StateChg events whose `to` is `Unknown`. This double-counts
    // if a single client_id flips Unknown->Working->Unknown, but for an
    // operator-facing spike indicator that's acceptable (and uncommon).
    for ev in &events {
        if let JournalEvent::StateChg { to: OrderState::Unknown, ts_ms, .. } = ev {
            if *ts_ms >= win1h {
                m.unknown_terminal_1h += 1;
            }
        }
    }

    // ── Active state counts from the lock-free snapshot ────────────────
    let snap = order_manager::orders_snapshot();
    for o in &snap.orders {
        match o.state {
            OrderState::PendingSubmit => m.active_pending_submit += 1,
            OrderState::Working       => m.active_working += 1,
            OrderState::PartialFill   => m.active_partial_fill += 1,
            OrderState::PendingCancel => m.active_pending_cancel += 1,
            OrderState::PendingModify => m.active_pending_modify += 1,
            OrderState::Unknown       => m.active_unknown += 1,
            _ => {}
        }
    }

    // ── Broker contact age ─────────────────────────────────────────────
    m.broker_age_secs = order_manager::broker_contact_age_secs();

    m
}

/// 1s memoization wrapper — keeps the panel cheap even at 60fps.
fn cached_metrics() -> Metrics {
    static METRICS_CACHE: OnceLock<Mutex<(Option<Instant>, Metrics)>> = OnceLock::new();
    let cache = METRICS_CACHE.get_or_init(|| Mutex::new((None, Metrics::default())));
    let mut g = match cache.lock() { Ok(g) => g, Err(p) => p.into_inner() };
    let stale = match g.0 {
        None => true,
        Some(t) => t.elapsed().as_millis() > 1000,
    };
    if stale {
        g.1 = compute_metrics();
        g.0 = Some(Instant::now());
    }
    g.1.clone()
}

/// Top-level draw entry. Mirrors the shape of `order_ledger_panel::draw`.
/// Renders only when `watchlist.order_health_open`.
pub(crate) fn draw(ctx: &egui::Context, watchlist: &mut Watchlist, t: &Theme) {
    if !watchlist.order_health_open { return; }

    let metrics = cached_metrics();

    let resp = Modal::new("ORDER SYSTEM HEALTH")
        .id("order_system_health")
        .ctx(ctx)
        .theme(t)
        .size(egui::vec2(320.0, 480.0))
        .anchor(Anchor::Window { pos: None })
        .frame_kind(FrameKind::DialogWindow)
        .header_style(HeaderStyle::Dialog)
        .separator(true)
        .show(|ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {

                // ── Section 1: Active orders by state ──────────────────
                PanelSection::new("ACTIVE ORDERS").show(ui, t, |ui, t| {
                    render_state_row(ui, t, "PendingSubmit", metrics.active_pending_submit, t.warn);
                    render_state_row(ui, t, "Working",       metrics.active_working,        t.accent);
                    render_state_row(ui, t, "PartialFill",   metrics.active_partial_fill,   t.accent);
                    render_state_row(ui, t, "PendingCancel", metrics.active_pending_cancel, t.warn);
                    render_state_row(ui, t, "PendingModify", metrics.active_pending_modify, t.warn);
                    render_state_row(ui, t, "Unknown",       metrics.active_unknown,        t.bear);
                });

                // ── Section 2: Last 5 minutes ───────────────────────────
                PanelSection::new("LAST 5 MINUTES").show(ui, t, |ui, t| {
                    let avg_label = match metrics.avg_pending_ack_ms {
                        Some(ms) => format!("{} ms", ms),
                        None => "—".to_string(),
                    };
                    let avg_tone = match metrics.avg_pending_ack_ms {
                        Some(ms) if ms > 1000 => PanelTone::Bear,
                        Some(ms) if ms > 250  => PanelTone::Warn,
                        _ => PanelTone::Text,
                    };
                    PanelKeyValueRow::new("Avg pending ack", avg_label)
                        .tone(avg_tone)
                        .show(ui, t);

                    let (rate_label, rate_tone) = if metrics.submit_attempts_5m == 0 {
                        ("— (no submits)".to_string(), PanelTone::Default)
                    } else {
                        let pct = 100.0 * metrics.submit_fails_5m as f32
                            / metrics.submit_attempts_5m as f32;
                        let tone = if pct >= 10.0 {
                            PanelTone::Bear
                        } else if pct >= 1.0 {
                            PanelTone::Warn
                        } else {
                            PanelTone::Bull
                        };
                        (format!("{:.1}% ({}/{})", pct, metrics.submit_fails_5m, metrics.submit_attempts_5m), tone)
                    };
                    PanelKeyValueRow::new("Submit reject rate", rate_label)
                        .tone(rate_tone)
                        .show(ui, t);

                    ui.add_space(gap_xs());
                    ui.add(MonospaceCode::new("Top rejects").size_px(font_xs()).color(t.dim));
                    if metrics.top_rejects.is_empty() {
                        ui.indent("oh_no_rejects", |ui| {
                            ui.add(MonospaceCode::new("none").size_px(font_xs()).color(t.dim).gamma(0.5));
                        });
                    } else {
                        ui.indent("oh_rejects", |ui| {
                            for (reason, count) in &metrics.top_rejects {
                                let truncated: String = if reason.len() > 38 {
                                    format!("{}…", &reason[..37])
                                } else {
                                    reason.clone()
                                };
                                ui.horizontal(|ui| {
                                    ui.add(MonospaceCode::new(&format!("{:>3}\u{00d7}", count))
                                        .size_px(font_xs()).color(t.bear).strong(true));
                                    ui.add(MonospaceCode::new(&truncated).size_px(font_xs()).color(t.text));
                                });
                            }
                        });
                    }
                });

                // ── Section 3: Broker contact ───────────────────────────
                PanelSection::new("BROKER CONTACT").show(ui, t, |ui, t| {
                    let (broker_label, broker_tone) = match metrics.broker_age_secs {
                        None => ("never (cold start)".to_string(), PanelTone::Default),
                        Some(s) => {
                            let tone = if s > 10 {
                                PanelTone::Bear
                            } else if s > 5 {
                                PanelTone::Warn
                            } else {
                                PanelTone::Bull
                            };
                            (format!("{}s ago", s), tone)
                        }
                    };
                    PanelKeyValueRow::new("Last broker contact", broker_label)
                        .tone(broker_tone)
                        .show(ui, t);
                });

                // ── Section 4: Last hour ────────────────────────────────
                PanelSection::new("LAST HOUR").rule(false).show(ui, t, |ui, t| {
                    let unk_tone = if metrics.unknown_terminal_1h > 0 {
                        PanelTone::Bear
                    } else {
                        PanelTone::Bull
                    };
                    PanelKeyValueRow::new(
                        "Orders ending Unknown",
                        metrics.unknown_terminal_1h.to_string(),
                    )
                    .tone(unk_tone)
                    .show(ui, t);
                });

            });
        });

    if resp.closed {
        watchlist.update_sidebar_state(|s| s.order_health_open = false);
    }
}

// ── Row helpers ──────────────────────────────────────────────────────────────

/// Render one active-state row via `PanelListRow::columns`.
/// Label left, count right. Count dims to `tint(t, Tone::Dim, alpha_strong())`
/// when zero; uses the provided `color` otherwise.
fn render_state_row(
    ui: &mut egui::Ui,
    t: &Theme,
    label: &str,
    count: u32,
    color: egui::Color32,
) {
    let count_str = count.to_string();
    let count_color = if count == 0 { tint(t, Tone::Dim, alpha_strong()) } else { color };
    PanelListRow::new(label)
        .columns(&[
            Column::left(label).color(t.text),
            Column::right(&count_str).color(count_color),
        ])
        .hoverable(false)
        .show(ui, t);
}
