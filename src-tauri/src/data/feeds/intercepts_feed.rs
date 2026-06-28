//! Live drawing-interception feed.
//!
//! Connects to apex-data `/ws/intercepts` and surfaces backend-computed
//! interception events (price breaking / retesting an auto-drawn line) in the
//! terminal's alert-badge feed. The backend (`InterceptEngine`) computes
//! interceptions across the whole universe — independent of whether a chart is
//! open — and streams them; here we render the significant ones as notifications.
//!
//! Global socket (fixed URL, no per-symbol dialing). Noisy phases
//! (approach/touch/bounce/cross) are filtered out by default — only `break` and
//! `retest` reach the badge feed (override via `INTERCEPTS_FEED_EVENTS`).
//!
//! Transport (connect / LAN-resolve / backoff / shutdown) is handled by
//! [`resilient_ws`]; this file is the URL + the event filter/parse.

use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{Arc, OnceLock};

use crate::chart_renderer::ui::components::toolbar::alert_feed;
use crate::chart_renderer::ui::tools::notification::AlertKind;
use crate::data::feeds::resilient_ws::{self, WsConfig};

static STARTED: OnceLock<()> = OnceLock::new();
/// Only surface interceptions newer than this (epoch ms) — set at startup so the
/// WS replay of recent history doesn't spam the feed, and reconnects don't
/// re-push already-seen events.
static SINCE_MS: AtomicI64 = AtomicI64::new(0);

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Which interception events to surface as badges (default: the actionable ones).
fn surfaced_events() -> Vec<String> {
    std::env::var("INTERCEPTS_FEED_EVENTS")
        .ok()
        .map(|v| v.split(',').map(|s| s.trim().to_lowercase()).filter(|s| !s.is_empty()).collect())
        .unwrap_or_else(|| vec!["break".into(), "retest".into()])
}

fn intercepts_ws_url() -> String {
    let base = crate::data::feeds::apex_data::config::apex_ws_url();
    let host = base
        .split("/ws")
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or("wss://apex-data-v2-dev.xllio.com");
    format!("{host}/ws/intercepts")
}

/// Spawn the feed. Idempotent — safe to call once at startup. Opt-out via
/// `INTERCEPTS_FEED_ENABLED=0`.
pub fn start() {
    if std::env::var("INTERCEPTS_FEED_ENABLED").map(|v| v == "0").unwrap_or(false) {
        return;
    }
    if STARTED.set(()).is_err() {
        return;
    }
    SINCE_MS.store(now_ms(), Ordering::SeqCst);
    let surfaced = surfaced_events();
    // Opt-in: consume the unified /ws/v2 `intercepts` topic instead of the
    // bespoke /ws/intercepts endpoint (which stays aliased server-side).
    let v2 = resilient_ws::v2_enabled();
    resilient_ws::spawn(WsConfig {
        name: "intercepts_feed",
        // Fixed global URL — no target changes, so the reconnect signal is unused.
        reconnect: Arc::new(AtomicBool::new(false)),
        idle_timeout: None, // interceptions are sporadic; quiet is normal
        url_provider: Box::new(move || Some(if v2 { resilient_ws::v2_url() } else { intercepts_ws_url() })),
        subscribe_msg: if v2 { Some(resilient_ws::v2_subscribe(&["intercepts"], "*")) } else { None },
        on_text: Box::new(move |text| on_text(text, &surfaced)),
    });
}

fn on_text(text: &str, surfaced: &[String]) {
    // /ws/v2 wraps the InterceptEvent in a Frame::Event envelope; unwrap to the
    // inner event. Otherwise treat the text as a raw /ws/intercepts event.
    let ev: serde_json::Value = match resilient_ws::unwrap_v2_event(text) {
        Some((dt, inner)) if dt == "intercepts" => inner,
        Some(_) => return, // a different datatype sharing the socket — ignore
        None => {
            let Ok(v) = serde_json::from_str::<serde_json::Value>(text) else { return };
            v
        }
    };
    let event = ev.get("event").and_then(|v| v.as_str()).unwrap_or("");
    if !surfaced.iter().any(|e| e == &event.to_lowercase()) {
        return;
    }
    let ts = ev.get("ts_ms").and_then(|v| v.as_i64()).unwrap_or(0);
    if ts <= SINCE_MS.load(Ordering::SeqCst) {
        return; // historical replay / already past — don't badge
    }
    let symbol = ev.get("symbol").and_then(|v| v.as_str()).unwrap_or("?").to_string();
    let dtype = ev.get("dtype").and_then(|v| v.as_str()).unwrap_or("line");
    let tf = ev.get("tf").and_then(|v| v.as_str()).unwrap_or("");
    let price = ev.get("price").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let kind = match event {
        "break" => AlertKind::Warning,
        _ => AlertKind::Signal,
    };
    let msg = format!("{event} {dtype} {tf} @ {price:.2}");
    alert_feed::push(kind, Some(symbol), msg);
    crate::wake_native_ui();
}
