//! Signals-as-datatype consumer (apex-data `/ws/v2` `signals` topic).
//!
//! ApexSignals publishes every emitted signal to the shared `signals:events`
//! Redis stream, which apex-data surfaces as the `signals` topic on the unified
//! `/ws/v2` socket (raw serialized `Signal` structs — `{type, symbol, ts,
//! data}`). This feed tails that topic and surfaces a curated set of families as
//! alert-badge notifications, the same way `intercepts_feed` does for
//! interceptions.
//!
//! It is intentionally ADDITIVE and does NOT replace `signals_feed` (which
//! consumes ApexSignals' purpose-built channel frames for pattern/trendline
//! *rendering*). This one is for the broader signal universe the rendering
//! pipeline doesn't handle — surfaced as notifications, not chart geometry.
//!
//! Double-gated so it can never surprise a live session:
//!   - only runs when the unified socket is opted into (`APEX_WS_V2=1`), and
//!   - only badges families in `SIGNALS_V2_FAMILIES` (a small actionable default),
//!     and only events newer than startup (so the WS replay doesn't spam).
//!
//! Transport (connect / LAN-resolve / backoff / shutdown) is the shared
//! [`resilient_ws`] harness.

use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{Arc, OnceLock};

use crate::chart_renderer::ui::components::toolbar::alert_feed;
use crate::chart_renderer::ui::tools::notification::AlertKind;
use crate::data::feeds::resilient_ws::{self, WsConfig};

static STARTED: OnceLock<()> = OnceLock::new();
/// Only surface signals newer than this (epoch ms), set at startup — so the WS
/// replay of recent history and reconnect replays don't re-badge old events.
static SINCE_MS: AtomicI64 = AtomicI64::new(0);

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Signal families (the snake_case `Signal` variant tag) to surface as badges.
/// Conservative default — the full universe is ~100 families and most are
/// chart/analytics detail, not notification-worthy. Override via
/// `SIGNALS_V2_FAMILIES` (csv); empty disables badging entirely.
fn surfaced_families() -> Vec<String> {
    std::env::var("SIGNALS_V2_FAMILIES")
        .ok()
        .map(|v| v.split(',').map(|s| s.trim().to_lowercase()).filter(|s| !s.is_empty()).collect())
        .unwrap_or_else(|| {
            ["combined", "market_regime", "unusual_activity", "earnings"]
                .iter()
                .map(|s| s.to_string())
                .collect()
        })
}

/// Spawn the feed. Idempotent. Only active when the unified socket is opted into
/// (`APEX_WS_V2=1`); otherwise inert. Hard opt-out via `SIGNALS_V2_FEED_ENABLED=0`.
pub fn start() {
    if !resilient_ws::v2_enabled() {
        return; // signals topic lives on /ws/v2 only
    }
    if std::env::var("SIGNALS_V2_FEED_ENABLED").map(|v| v == "0").unwrap_or(false) {
        return;
    }
    if STARTED.set(()).is_err() {
        return;
    }
    SINCE_MS.store(now_ms(), Ordering::SeqCst);
    let surfaced = surfaced_families();
    resilient_ws::spawn(WsConfig {
        name: "signals_v2_feed",
        // Fixed global URL — account-wide signals, no per-symbol dialing.
        reconnect: Arc::new(AtomicBool::new(false)),
        idle_timeout: None, // signals can be legitimately quiet
        url_provider: Box::new(|| Some(resilient_ws::v2_url())),
        subscribe_msg: Some(resilient_ws::v2_subscribe(&["signals"], "*")),
        on_text: Box::new(move |text| on_text(text, &surfaced)),
    });
}

fn on_text(text: &str, surfaced: &[String]) {
    // The signals topic is delivered as a /ws/v2 Frame::Event envelope; the inner
    // event is `{type, symbol, ts, data}` (the raw serialized Signal).
    let Some((dt, ev)) = resilient_ws::unwrap_v2_event(text) else { return };
    if dt != "signals" {
        return;
    }
    let family = ev.get("type").and_then(|v| v.as_str()).unwrap_or("").to_lowercase();
    if !surfaced.iter().any(|f| f == &family) {
        return;
    }
    let ts = ev.get("ts").and_then(|v| v.as_i64()).unwrap_or(0);
    if ts <= SINCE_MS.load(Ordering::SeqCst) {
        return; // historical replay / already past — don't badge
    }
    let symbol = ev.get("symbol").and_then(|v| v.as_str()).unwrap_or("?").to_string();

    // Best-effort context from the raw signal body — most engines expose one of
    // these. Absent → just the family + symbol.
    let body = ev.get("data");
    let ctx = body
        .and_then(|d| {
            ["direction", "bias", "signal", "tier", "regime", "state", "label"]
                .iter()
                .find_map(|k| d.get(*k).and_then(|v| v.as_str()))
        })
        .map(|s| format!(" — {s}"))
        .unwrap_or_default();
    let score = body
        .and_then(|d| d.get("score").or_else(|| d.get("confidence")).and_then(|v| v.as_f64()))
        .map(|v| format!(" ({v:.2})"))
        .unwrap_or_default();

    let label = pretty_family(&family);
    let msg = format!("{label}{ctx}{score}");
    alert_feed::push(AlertKind::Signal, Some(symbol), msg);
    crate::wake_native_ui();
}

/// `unusual_activity` → `Unusual Activity`.
fn pretty_family(family: &str) -> String {
    family
        .split('_')
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}
