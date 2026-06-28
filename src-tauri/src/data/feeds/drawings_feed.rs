//! Live auto-drawing feed trigger.
//!
//! Subscribes to apex-data `/ws/drawings` (a global add/update/remove stream).
//! When a drawing changes for the *active* chart symbol/timeframe AND the user
//! has enabled the unified feed (`AutoDrawConfig.live_feed`), this re-pulls the
//! full drawing set via `fetch_apexsignals_drawings` (which, in live_feed mode,
//! reads apex-data `/api/drawings`). So the chart's auto-drawings refresh live
//! as the backend recomputes them at bar close.
//!
//! When `live_feed` is off (default), this is inert — the tuned per-chart
//! ApexSignals fetch owns the drawings and honours the AUTO-CHARTING panel knobs.
//!
//! Refreshes are debounced and only fire for the active symbol. Transport
//! (connect / LAN-resolve / backoff / shutdown) is handled by [`resilient_ws`].

use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{Arc, OnceLock};

use parking_lot::Mutex;

use crate::data::feeds::resilient_ws::{self, WsConfig};

static STARTED: OnceLock<()> = OnceLock::new();
static ACTIVE: OnceLock<Mutex<Option<(String, String)>>> = OnceLock::new();
static LAST_REFRESH_MS: AtomicI64 = AtomicI64::new(0);

const DEBOUNCE_MS: i64 = 1500;

fn active() -> &'static Mutex<Option<(String, String)>> {
    ACTIVE.get_or_init(|| Mutex::new(None))
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Point the live-drawing trigger at the active chart symbol/timeframe. Cheap;
/// just updates the filter the WS loop matches against.
pub fn set_target(symbol: &str, timeframe: &str) {
    let sym = symbol.strip_prefix("F:").unwrap_or(symbol).to_string();
    *active().lock() = Some((sym, timeframe.to_string()));
}

fn drawings_ws_url() -> String {
    let base = crate::data::feeds::apex_data::config::apex_ws_url();
    let host = base
        .split("/ws")
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or("wss://apex-data-v2-dev.xllio.com");
    format!("{host}/ws/drawings")
}

/// Spawn the feed. Idempotent. Opt-out via `DRAWINGS_FEED_ENABLED=0`.
pub fn start() {
    if std::env::var("DRAWINGS_FEED_ENABLED").map(|v| v == "0").unwrap_or(false) {
        return;
    }
    if STARTED.set(()).is_err() {
        return;
    }
    resilient_ws::spawn(WsConfig {
        name: "drawings_feed",
        // Fixed global URL — no target changes, so the reconnect signal is unused.
        reconnect: Arc::new(AtomicBool::new(false)),
        idle_timeout: None, // drawing events are sporadic; quiet is normal
        url_provider: Box::new(|| Some(drawings_ws_url())),
        on_text: Box::new(on_text),
    });
}

fn on_text(text: &str) {
    let Ok(ev) = serde_json::from_str::<serde_json::Value>(text) else { return };
    let sym = ev.get("symbol").and_then(|v| v.as_str()).unwrap_or("");
    let tf = ev.get("tf").and_then(|v| v.as_str()).unwrap_or("");

    // Only react to the active symbol/tf, and only when the unified feed is on.
    let matches_active = active().lock().as_ref().map(|(s, t)| s == sym && t == tf).unwrap_or(false);
    if !matches_active { return; }
    if !crate::chart_renderer::gpu::auto_draw_config().live_feed { return; }

    // Debounce: a recompute cycle emits many events; coalesce into one re-pull.
    let now = now_ms();
    if now - LAST_REFRESH_MS.load(Ordering::Relaxed) < DEBOUNCE_MS { return; }
    LAST_REFRESH_MS.store(now, Ordering::Relaxed);

    // Re-pull the full set (fetch spawns its own worker + wakes the UI).
    crate::chart_renderer::gpu::fetch_apexsignals_drawings(sym.to_string(), tf.to_string());
}
