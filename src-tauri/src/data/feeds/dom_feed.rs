//! Live DOM (L2 depth-of-market) feed.
//!
//! Connects to apex-data `/ws/dom?symbol=X&rows=20` and pushes a merged,
//! price-keyed ladder into the chart renderer via `ChartCommand::DomLevels`.
//!
//! The DOM endpoint is per-symbol (the symbol lives in the URL), so this feed
//! re-dials whenever the active chart symbol changes. Call `set_symbol()` on
//! every symbol switch (and once on startup); the connect loop reconnects.
//!
//! Transport (connect / LAN-resolve / backoff / shutdown / chart-send) is
//! handled by [`resilient_ws`]; this file is the URL + the `dom` ladder parse.

use std::sync::{Arc, OnceLock};
use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::chart_renderer::{ChartCommand, ui::panels::dom_panel::DomLevel};
use crate::data::feeds::resilient_ws::{self, WsConfig};

static ACTIVE_SYMBOL: OnceLock<Mutex<Option<String>>> = OnceLock::new();
static RECONNECT: OnceLock<Arc<AtomicBool>> = OnceLock::new();
static STARTED: OnceLock<()> = OnceLock::new();

fn active() -> &'static Mutex<Option<String>> {
    ACTIVE_SYMBOL.get_or_init(|| Mutex::new(None))
}

fn reconnect_flag() -> &'static Arc<AtomicBool> {
    RECONNECT.get_or_init(|| Arc::new(AtomicBool::new(false)))
}

/// Epoch milliseconds. Shared with the renderer's live-vs-mock gate so the
/// staleness window uses one clock.
pub fn now_ms() -> i64 {
    crate::foundation::time::now_ms()
}

/// Point the DOM feed at `symbol` (the chart symbol). Trips a reconnect only if
/// it actually changed. The connect loop re-dials the new symbol's `/ws/dom`.
pub fn set_symbol(symbol: &str) {
    let mut g = active().lock();
    if g.as_deref() == Some(symbol) {
        return;
    }
    *g = Some(symbol.to_string());
    reconnect_flag().store(true, Ordering::SeqCst);
}

/// Build the `/ws/dom` URL from the shared apex-data base. `apex_ws_url()`
/// returns `ws(s)://host/ws?format=json`; we keep the scheme+host and swap the
/// path so DOM honours the same `APEX_DATA_URL` override as the main feed.
fn dom_ws_url(symbol: &str) -> String {
    let base = crate::data::feeds::apex_data::config::apex_ws_url();
    let host = base
        .split("/ws")
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or("wss://apex-data-v2-dev.xllio.com");
    // Strip the F: futures class tag → backend wants the bare root (F:ES → ES).
    let symbol = symbol.strip_prefix("F:").unwrap_or(symbol);
    // rows is clamped 1..20 server-side; ask for the max.
    format!("{host}/ws/dom?symbol={symbol}&rows=20")
}

/// Spawn the feed. Idempotent — safe to call once at startup.
pub fn start() {
    if STARTED.set(()).is_err() {
        return;
    }
    // Opt-in: use the unified /ws/v2 `depth` topic (one socket + a per-symbol
    // subscribe) instead of the bespoke per-symbol /ws/dom URL. Both reconnect on
    // symbol change; on_text is dual-shape. Bespoke stays aliased server-side.
    let v2 = resilient_ws::v2_enabled();
    resilient_ws::spawn(WsConfig {
        name: "dom_feed",
        reconnect: reconnect_flag().clone(),
        // A book can be legitimately quiet (illiquid / off-hours), so no idle
        // watchdog — only reconnect on symbol change or a dropped socket.
        idle_timeout: None,
        url_provider: Box::new(move || {
            // Idle (None) until a symbol is set; in v2 the symbol rides the
            // subscribe, so the URL is the global socket.
            active().lock().as_ref().map(|s| if v2 { resilient_ws::v2_url() } else { dom_ws_url(s) })
        }),
        subscribe_provider: Box::new(move || {
            if !v2 { return None; }
            active().lock().as_ref().map(|s| {
                let root = s.strip_prefix("F:").unwrap_or(s);
                resilient_ws::v2_subscribe(&["depth"], root)
            })
        }),
        on_text: Box::new(on_text),
    });
}

/// Parse one text frame and route it. Reads the current symbol for the
/// `DomLevels` command's symbol field.
fn on_text(text: &str) {
    let Some(symbol) = active().lock().clone() else { return };
    let v: serde_json::Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(_) => return,
    };
    match v.get("type").and_then(|t| t.as_str()) {
        Some("dom") => {
            // /ws/v2 (Frame::Dom) nests bids/asks under `data`; the bespoke
            // /ws/dom carries them at the top level. Parse whichever has them.
            let payload = match v.get("data") {
                Some(d) if d.get("bids").is_some() || d.get("asks").is_some() => d,
                _ => &v,
            };
            let levels = parse_dom(payload);
            resilient_ws::send_to_charts(ChartCommand::DomLevels { symbol, levels });
        }
        Some("error") => tracing::warn!(target: "dom_feed", "server error: {v}"),
        _ => {}
    }
}

/// Merge the `bids` + `asks` arrays into a single price-keyed ladder, sorted
/// high → low (the order the DOM panel renders). L2 depth carries no traded
/// volume, so `volume` stays 0 and `delta` is the resting bid/ask imbalance.
fn parse_dom(v: &serde_json::Value) -> Vec<DomLevel> {
    use std::collections::BTreeMap;

    // Key by price*1000 (integer) so floats sort deterministically.
    let mut book: BTreeMap<i64, DomLevel> = BTreeMap::new();
    let empty = Vec::new();

    let read_side = |side: &str| -> Vec<serde_json::Value> {
        v.get(side)
            .and_then(|a| a.as_array())
            .cloned()
            .unwrap_or_else(|| empty.clone())
    };

    for lvl in read_side("bids") {
        let p = lvl.get("price").and_then(|x| x.as_f64()).unwrap_or(0.0);
        // Sizes arrive as JSON floats (e.g. `12.0`); as_u64() returns None on
        // those, so read as f64 and round.
        let s = lvl.get("size").and_then(|x| x.as_f64()).unwrap_or(0.0).round() as u32;
        let e = book.entry((p * 1000.0) as i64).or_insert(DomLevel {
            price: p as f32, bid_size: 0, ask_size: 0, volume: 0, delta: 0, absorbed: false, pulled: false, big_print: false, buy_vol: 0, sell_vol: 0,
        });
        e.bid_size = s;
    }
    for lvl in read_side("asks") {
        let p = lvl.get("price").and_then(|x| x.as_f64()).unwrap_or(0.0);
        let s = lvl.get("size").and_then(|x| x.as_f64()).unwrap_or(0.0).round() as u32;
        let e = book.entry((p * 1000.0) as i64).or_insert(DomLevel {
            price: p as f32, bid_size: 0, ask_size: 0, volume: 0, delta: 0, absorbed: false, pulled: false, big_print: false, buy_vol: 0, sell_vol: 0,
        });
        e.ask_size = s;
    }

    // BTreeMap iterates ascending; the ladder draws high → low, so reverse.
    book.into_values()
        .rev()
        .map(|mut l| {
            l.delta = l.bid_size as i64 - l.ask_size as i64;
            l
        })
        .collect()
}
