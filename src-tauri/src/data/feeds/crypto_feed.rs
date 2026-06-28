//! Real-time crypto feed — ApexCrypto WebSocket for live bar updates.
//!
//! Subscribes to chart timeframes + 1s for price tracking + tape, pushing
//! UpdateLastBar / AppendBar / WatchlistPrice / TapeEntry to the chart renderer.
//!
//! Transport (connect / subscribe / conditional heartbeat / tick-watchdog +
//! stall toast / jittered backoff / shutdown / ConnectionState + metrics) is
//! handled by [`resilient_ws::spawn_subscribed`]; this file keeps its
//! `state_tx()` + metric statics (the `crypto` provider subscribes to them for
//! the connection panel) and the bar/tape parse.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU32, AtomicU64, Ordering};
use std::time::Duration;

use crate::chart_renderer::{Bar, ChartCommand};
use crate::data::connectivity::ConnectionState;
use crate::data::feeds::resilient_ws::{self, FeedMetrics, SubscribedWsConfig};

// ── Per-feed metrics (read by the `crypto` provider / diagnostics) ───────────
pub(crate) static MESSAGES_IN:        AtomicU64 = AtomicU64::new(0);
pub(crate) static PARSE_ERRORS:       AtomicU64 = AtomicU64::new(0);
pub(crate) static RECONNECT_COUNT:    AtomicU32 = AtomicU32::new(0);
pub(crate) static LAST_MESSAGE_AT_MS: AtomicI64 = AtomicI64::new(0);
/// Vestigial: staleness/reconnect now owned by the `resilient_ws` harness. Kept
/// (always false) because the `crypto` provider's `state()` fallback reads it;
/// authoritative state flows via `state_tx()`.
pub(crate) static FORCE_RECONNECT: AtomicBool = AtomicBool::new(false);

// ── ConnectionState push stream (the `crypto` provider subscribes here) ──────
static STATE_TX: OnceLock<tokio::sync::broadcast::Sender<ConnectionState>> = OnceLock::new();
pub fn state_tx() -> &'static tokio::sync::broadcast::Sender<ConnectionState> {
    STATE_TX.get_or_init(|| tokio::sync::broadcast::channel(64).0)
}
pub(crate) fn publish_state(s: ConnectionState) {
    let _ = state_tx().send(s);
}

static STARTED: OnceLock<()> = OnceLock::new();

const APEX_CRYPTO_WS: &str = "ws://192.168.1.56:30840/ws";

pub fn start() {
    if STARTED.set(()).is_err() {
        return;
    }
    resilient_ws::spawn_subscribed(SubscribedWsConfig {
        name: "crypto_feed",
        url: Box::new(|| APEX_CRYPTO_WS.to_string()),
        subscribe_msg: serde_json::json!({
            "subscribe": ["*:1s", "*:1m", "*:5m", "*:15m", "*:30m", "*:1h", "*:4h", "*:1d"],
            "tape": ["*"]
        }).to_string(),
        // Wildcard subs — surface 1 so the panel shows green-with-load.
        subscribed_count: 1,
        // Crypto carries steady-state ticks; only ping during quiet stretches.
        heartbeat: Some(Duration::from_secs(30)),
        conditional_heartbeat: true,
        stale_timeout: Some(Duration::from_secs(30)),
        gap_fill_on_reconnect: true,
        metrics: FeedMetrics {
            messages_in: &MESSAGES_IN,
            reconnect_count: &RECONNECT_COUNT,
            last_message_at_ms: &LAST_MESSAGE_AT_MS,
        },
        publish_state: Box::new(publish_state),
        on_text: Box::new(on_text),
    });
}

/// Parse one crypto frame: trade → tape, bar → chart/watchlist.
fn on_text(text: &str) {
    let json: serde_json::Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(_) => { PARSE_ERRORS.fetch_add(1, Ordering::Relaxed); return; }
    };

    if let Some(trade) = json.get("trade") {
        let symbol = trade["symbol"].as_str().unwrap_or("").to_string();
        let price = trade["price"].as_f64().unwrap_or(0.0) as f32;
        let qty = trade["qty"].as_f64().unwrap_or(0.0) as f32;
        let time = trade["time"].as_i64().unwrap_or(0);
        let is_buy = trade["side"].as_str() == Some("buy");
        resilient_ws::send_to_charts(ChartCommand::TapeEntry { symbol, price, qty, time, is_buy });
        return;
    }

    let Ok(update) = serde_json::from_value::<BarUpdateMsg>(json) else { return };
    let bar = &update.bar;
    let is_1s = bar.timeframe == "1s";

    // Bump SubscriptionManager so gap-fill on reconnect has accurate replay
    // anchors for crypto symbols too (no-op when no one subscribed to this
    // (sym, tf)). Crypto bars are always last-source.
    crate::data::providers::registry::subscription_manager()
        .bump_last_seen_bar(
            &bar.symbol,
            &bar.timeframe,
            crate::data::providers::subscription_manager::BarSource::Last,
            bar.time,
        );

    if is_1s {
        // 1s bars → watchlist price only (don't push to the chart).
        resilient_ws::send_to_charts(ChartCommand::WatchlistPrice {
            symbol: bar.symbol.clone(),
            price: bar.close as f32,
            prev_close: bar.open as f32,
            day_close: 0.0, // crypto: no regular-session close concept
        });
        return;
    }

    // >= 1m bars → chart updates.
    let gpu_bar = Bar {
        open: bar.open as f32,
        high: bar.high as f32,
        low: bar.low as f32,
        close: bar.close as f32,
        volume: bar.volume as f32,
        _pad: 0.0,
    };
    let time_sec = bar.time / 1000;

    if update.is_closed {
        resilient_ws::send_to_charts(ChartCommand::AppendBar {
            symbol: bar.symbol.clone(),
            timeframe: bar.timeframe.clone(),
            bar: gpu_bar,
            timestamp: time_sec,
            mark: false,
        });
    } else {
        let mut tick_bar = gpu_bar;
        tick_bar.volume = 0.0;
        resilient_ws::send_to_charts(ChartCommand::UpdateLastBar {
            symbol: bar.symbol.clone(),
            timeframe: bar.timeframe.clone(),
            bar: tick_bar,
            timestamp: time_sec,
            mark: false,
            cumulative: false, // incremental tick model (unchanged)
        });
    }
}

#[derive(serde::Deserialize)]
struct BarUpdateMsg {
    bar: CryptoBar,
    is_closed: bool,
}

#[derive(serde::Deserialize)]
struct CryptoBar {
    symbol: String,
    timeframe: String,
    time: i64,
    open: f64,
    high: f64,
    low: f64,
    close: f64,
    volume: f64,
}
