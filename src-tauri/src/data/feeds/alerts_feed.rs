//! Live playbook-alert feed (B2 fix).
//!
//! Connects to ApexData `/ws/alerts` and surfaces fired playbook alerts in
//! the terminal's alert-badge feed AND the alerts panel's "PLAYBOOK ALERTS"
//! section.
//!
//! ## Frame shape
//! The `/ws/alerts` endpoint emits raw `AlertEvent` JSON (no versioned
//! envelope — it predates the unified `/ws/v2`):
//! ```json
//! {
//!   "id":        "evt-uuid",
//!   "rule_id":   "rule-uuid",
//!   "rule_name": "01 Rotation Under Pin",
//!   "kind":      "composite",
//!   "symbol":    "QQQ",
//!   "op":        "crosses_above",
//!   "threshold": 0.0,
//!   "value":     1.0,
//!   "severity":  "warning",
//!   "message":   "Rotation detected …",
//!   "ts_ms":     1720600000000
//! }
//! ```
//! The `/ws/v2` path wraps this in a `{"type":"event","data":{"datatype":"alerts",
//! "data":{…}}}` envelope; [`resilient_ws::unwrap_v2_event`] handles that.
//!
//! ## Feature gate
//! Set `ALERTS_FEED_ENABLED=0` to suppress the feed entirely (no error spam
//! when ApexData is not running). Default is enabled.
//!
//! ## URL
//! Derived from the shared ApexData base URL (`apex_data::config::apex_ws_url`),
//! same as `intercepts_feed`. No additional env var required; the base URL
//! `APEX_DATA_URL` already governs this.

use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{Arc, OnceLock, Mutex};

use serde::Deserialize;

use crate::chart_renderer::ui::components::toolbar::alert_feed;
use crate::chart_renderer::ui::tools::notification::AlertKind;
use crate::data::feeds::resilient_ws::{self, WsConfig};

// ── Startup guard ──────────────────────────────────────────────────────────────

static STARTED: OnceLock<()> = OnceLock::new();

/// Epoch-ms stamp set at startup; frames older than this are historical replay
/// replayed on connect — we badge them as "recent fired" rather than suppressing
/// them completely (the panel history is useful), but we filter the badge strip
/// to only show truly new events that fire AFTER we connected.
static SINCE_MS: AtomicI64 = AtomicI64::new(0);

fn now_ms() -> i64 {
    crate::foundation::time::now_ms()
}

// ── Shared queue for the alerts panel ─────────────────────────────────────────

/// A received playbook alert that the alerts panel can render.
#[derive(Clone, Debug)]
pub struct PlaybookAlert {
    pub rule_name: String,
    pub symbol:    String,
    pub severity:  String,
    pub message:   String,
    pub ts_ms:     i64,
}

/// Global ring of recently fired playbook alerts for the alerts panel section.
/// Capped at 50 entries (oldest evicted first).
static PLAYBOOK_ALERTS: OnceLock<Mutex<Vec<PlaybookAlert>>> = OnceLock::new();

fn playbook_queue() -> &'static Mutex<Vec<PlaybookAlert>> {
    PLAYBOOK_ALERTS.get_or_init(|| Mutex::new(Vec::with_capacity(50)))
}

/// Read a snapshot of recent playbook alerts (panel rendering, frame-safe).
pub fn snapshot() -> Vec<PlaybookAlert> {
    playbook_queue().lock().map(|g| g.clone()).unwrap_or_default()
}

fn push_to_queue(alert: PlaybookAlert) {
    if let Ok(mut g) = playbook_queue().lock() {
        if g.len() >= 50 {
            g.remove(0);
        }
        g.push(alert);
    }
}

// ── Wire URL ───────────────────────────────────────────────────────────────────

fn alerts_ws_url() -> String {
    let base = crate::data::feeds::apex_data::config::apex_ws_url();
    // Strip the `/ws?format=json` suffix and replace with `/ws/alerts`
    let host = base
        .split("/ws")
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or("wss://apex-data-v2-dev.xllio.com");
    format!("{host}/ws/alerts")
}

// ── Public start() ─────────────────────────────────────────────────────────────

/// Spawn the alerts feed. Idempotent — safe to call once at startup.
/// Opt-out via `ALERTS_FEED_ENABLED=0`.
pub fn start() {
    if std::env::var("ALERTS_FEED_ENABLED")
        .map(|v| v == "0")
        .unwrap_or(false)
    {
        return;
    }
    if STARTED.set(()).is_err() {
        return;
    }
    SINCE_MS.store(now_ms(), Ordering::SeqCst);

    let v2 = resilient_ws::v2_enabled();
    resilient_ws::spawn(WsConfig {
        name: "alerts_feed",
        reconnect: Arc::new(AtomicBool::new(false)),
        // Alerts come in bursts when rules fire; long quiet periods are normal.
        idle_timeout: None,
        url_provider: Box::new(move || {
            Some(if v2 { resilient_ws::v2_url() } else { alerts_ws_url() })
        }),
        subscribe_provider: Box::new(move || {
            v2.then(|| resilient_ws::v2_subscribe(&["alerts"], "*"))
        }),
        on_text: Box::new(on_text),
    });
}

// ── Parse + route ──────────────────────────────────────────────────────────────

/// Minimal serde shape for AlertEvent from `/ws/alerts`.
/// Only the fields we display — unknown fields are ignored.
#[derive(Deserialize)]
struct AlertEvent {
    rule_name: String,
    #[serde(default)]
    symbol:   String,
    severity: String,
    message:  String,
    ts_ms:    i64,
}

fn on_text(text: &str) {
    // /ws/v2 wraps in a Frame::Event envelope; unwrap to the inner object.
    let val: serde_json::Value = match resilient_ws::unwrap_v2_event(text) {
        Some((dt, inner)) if dt == "alerts" => inner,
        Some(_) => return, // different datatype on the shared v2 socket
        None => match serde_json::from_str(text) {
            Ok(v) => v,
            Err(_) => return,
        },
    };

    let ev: AlertEvent = match serde_json::from_value(val) {
        Ok(e) => e,
        Err(_) => return,
    };

    let ts = ev.ts_ms;

    // Push to the panel queue unconditionally (panel shows history including
    // the initial 50-event replay on connect).
    let alert = PlaybookAlert {
        rule_name: ev.rule_name.clone(),
        symbol:    ev.symbol.clone(),
        severity:  ev.severity.clone(),
        message:   ev.message.clone(),
        ts_ms:     ts,
    };
    push_to_queue(alert);

    // Badge strip: only show events that fired after we connected (suppresses
    // the historical replay spam).
    if ts > SINCE_MS.load(Ordering::SeqCst) {
        let kind = severity_to_kind(&ev.severity);
        let sym = if ev.symbol.is_empty() { None } else { Some(ev.symbol.clone()) };
        let msg = format!("PLAYBOOK {}: {}", ev.rule_name, ev.message);
        alert_feed::push(kind, sym, msg);
        crate::wake_native_ui();
    }
}

fn severity_to_kind(severity: &str) -> AlertKind {
    match severity.to_lowercase().as_str() {
        "critical" | "error" => AlertKind::Warning,
        "warning" | "warn"   => AlertKind::Warning,
        _                    => AlertKind::Signal,
    }
}

// ── Screen-entry alert source (T-HEAT, Wave S2 — additive) ───────────────────
//
// `panels/screener_heatmap.rs` calls `push_screen_entry` when a symbol enters
// an active screen. Events are pushed to the same `PLAYBOOK_ALERTS` ring used
// by playbook alerts so `alerts_panel.rs` surfaces them under a "SCREEN ENTRIES"
// section without any refactor of the existing PLAYBOOK ALERTS rendering path.
//
// The `rule_name` is prefixed with `"screen:"` so `alerts_panel.rs` can
// optionally filter/group screen-entry events separately (SCR-6 §5.1 naming
// convention).

/// Push a screen-entry event into the shared `PLAYBOOK_ALERTS` ring.
///
/// Called exclusively from `panels/screener_heatmap.rs::route_screen_entry_alert`.
/// NOT called from `on_text` — this is an additive second source alongside the
/// `/ws/alerts` WebSocket path.
pub fn push_screen_entry(symbol: &str, screen_name: &str, score: f64, severity: &str) {
    let alert = PlaybookAlert {
        rule_name: format!("screen:{}", screen_name),
        symbol:    symbol.to_string(),
        severity:  severity.to_string(),
        message:   format!("{} entered screen '{}' (score {:.1})", symbol, screen_name, score),
        ts_ms:     now_ms(),
    };
    push_to_queue(alert);
}
