//! Live futures bar feed.
//!
//! Futures are an IBKR source (not on the Polygon `/ws` firehose), so their
//! live candles come from ApexIB's 5-second `reqRealTimeBars`, proxied by
//! apex-data at `/ws/futures?symbol=ES`. This client folds each 5s print into
//! the chart's current candle (`UpdateLastBar`) and updates the watchlist
//! (`WatchlistPrice`).
//!
//! Activates ONLY for `F:`-tagged futures symbols; any non-futures target
//! deactivates it. Re-dials on symbol change, like `dom_feed`.
//!
//! Transport (connect / LAN-resolve / backoff / idle-watchdog / shutdown /
//! chart-send) is handled by [`resilient_ws`]; this file is just the URL +
//! the `bar`-frame parse.

use std::sync::{Arc, OnceLock};
use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crate::chart_renderer::{Bar, ChartCommand};
use crate::data::feeds::resilient_ws::{self, WsConfig};

/// Active target: `(canonical_symbol_with_F_tag, timeframe)`. `None` = idle.
static TARGET: OnceLock<Mutex<Option<(String, String)>>> = OnceLock::new();
static RECONNECT: OnceLock<Arc<AtomicBool>> = OnceLock::new();
static STARTED: OnceLock<()> = OnceLock::new();

fn target() -> &'static Mutex<Option<(String, String)>> { TARGET.get_or_init(|| Mutex::new(None)) }
fn reconnect_flag() -> &'static Arc<AtomicBool> { RECONNECT.get_or_init(|| Arc::new(AtomicBool::new(false))) }

/// Point the futures feed at `(symbol, timeframe)`. Only `F:`-tagged symbols
/// activate the feed; anything else deactivates it (stocks/options stream via
/// the Polygon `/ws`). Idempotent. Call on every chart symbol/timeframe change.
pub fn set_target(symbol: &str, timeframe: &str) {
    let mut g = target().lock();
    let next = if symbol.starts_with("F:") {
        Some((symbol.to_string(), timeframe.to_string()))
    } else {
        None
    };
    if *g == next {
        return;
    }
    *g = next;
    reconnect_flag().store(true, Ordering::SeqCst);
}

/// Build the `/ws/futures` URL from the shared apex-data base. Strips the `F:`
/// class tag → the backend wants the bare root (`F:ES` → `ES`).
fn futures_ws_url(symbol: &str) -> String {
    let base = crate::data::feeds::apex_data::config::apex_ws_url();
    let host = base
        .split("/ws")
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or("wss://apex-data-v2-dev.xllio.com");
    let root = symbol.strip_prefix("F:").unwrap_or(symbol);
    format!("{host}/ws/futures?symbol={root}")
}

/// Spawn the feed. Idempotent — safe to call once at startup.
pub fn start() {
    if STARTED.set(()).is_err() {
        return;
    }
    resilient_ws::spawn(WsConfig {
        name: "futures_feed",
        reconnect: reconnect_flag().clone(),
        // 5s bars; a 30s gap means the stream is stale → reconnect.
        idle_timeout: Some(Duration::from_secs(30)),
        url_provider: Box::new(|| {
            target().lock().as_ref().map(|(sym, _tf)| futures_ws_url(sym))
        }),
        on_text: Box::new(on_text),
    });
}

/// Parse one text frame and route it. Reads the current target for the symbol /
/// timeframe context the chart commands need.
fn on_text(text: &str) {
    let Some((symbol, timeframe)) = target().lock().clone() else { return };
    let v: serde_json::Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(_) => return,
    };
    match v.get("type").and_then(|t| t.as_str()) {
        Some("bar") => emit_bar(&v, &symbol, &timeframe),
        Some("error") => tracing::warn!(target: "futures_feed", "server error: {v}"),
        _ => {}
    }
}

/// Fold a 5s `bar` frame into the chart's current candle + the watchlist.
fn emit_bar(v: &serde_json::Value, symbol: &str, timeframe: &str) {
    let f = |k: &str| v.get(k).and_then(|x| x.as_f64()).unwrap_or(0.0);
    let (open, high, low, close, volume) = (f("open"), f("high"), f("low"), f("close"), f("volume"));
    if close <= 0.0 {
        return;
    }
    let time_sec = v.get("time").and_then(|x| x.as_i64()).unwrap_or(0);

    // Watchlist price (keyed by the canonical F: symbol the watchlist stores).
    resilient_ws::send_to_charts(ChartCommand::WatchlistPrice {
        symbol: symbol.to_string(),
        price: close as f32,
        prev_close: open as f32,
        day_close: 0.0, // futures: 24h, no equity-style regular close
        change_perc: None, // futures: panel computes from prev_close
    });

    // Fold into the current chart candle. `cumulative: false` = incremental
    // tick model (volume +=, high/low via close) — the IB/crypto path.
    let bar = Bar {
        open: open as f32,
        high: high as f32,
        low: low as f32,
        close: close as f32,
        volume: volume as f32,
        _pad: 0.0,
    };
    resilient_ws::send_to_charts(ChartCommand::UpdateLastBar {
        symbol: symbol.to_string(),
        timeframe: timeframe.to_string(),
        bar,
        timestamp: time_sec,
        mark: false,
        cumulative: false,
    });
}
