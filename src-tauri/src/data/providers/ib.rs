//! Adapter: `MarketDataProvider` over `feeds::ib_ws`.
//!
//! The existing IB integration is Tauri-state-driven (`ib_ws_send` command) —
//! frontend code sends subscribe/unsubscribe JSON over the WS, and ticks are
//! emitted as `ib-tick` Tauri events. There is no per-symbol Rust-side stream
//! API today, and historical bars come from yfinance/Yahoo paths, not IB.
//!
//! Wave-2 adapter: advertise honestly via `capabilities()` and return
//! `NotSupported` everywhere. Wired through the registry so the connection
//! state surfaces in the UI uniformly. A proper IB stream API is follow-up
//! work — flagged in the report.

use super::provider::{
    BarStream, ChainSnapshot, ChainStream, MarketDataProvider, ProviderCapabilities,
    QuoteStream, TradeStream,
};
use crate::data::feeds::ib_ws;
use crate::data::connectivity::{
    ApiError, Connection, ConnectionMetrics, ConnectionState,
};
use crate::data::feeds::apex_data::types::BarWire;

pub struct IbProvider;

impl IbProvider {
    pub fn new() -> Self { Self }
}

impl Default for IbProvider {
    fn default() -> Self { Self::new() }
}

impl Connection for IbProvider {
    fn name(&self) -> &str { "ib_ws" }
    fn state(&self) -> ConnectionState {
        use std::sync::atomic::Ordering;
        // Wave 7E: surface the watchdog stall through ConnectionState so the
        // UI status dot can flip amber the moment we lose tick liveness.
        if crate::data::feeds::ib_ws::FORCE_RECONNECT.load(Ordering::Relaxed) {
            return ConnectionState::Backoff {
                until: std::time::Instant::now() + std::time::Duration::from_secs(1),
                attempt: crate::data::feeds::ib_ws::RECONNECT_COUNT.load(Ordering::Relaxed),
                reason: "tick_stalled".into(),
            };
        }
        ConnectionState::Idle
    }
    fn subscribe_state(&self) -> Option<tokio::sync::broadcast::Receiver<ConnectionState>> {
        // Wave 11c: push stream backed by `feeds::ib_ws::state_tx()`.
        Some(crate::data::feeds::ib_ws::state_tx().subscribe())
    }
    fn metrics(&self) -> ConnectionMetrics {
        feed_metrics_snapshot(
            &crate::data::feeds::ib_ws::MESSAGES_IN,
            &crate::data::feeds::ib_ws::PARSE_ERRORS,
            &crate::data::feeds::ib_ws::RECONNECT_COUNT,
            &crate::data::feeds::ib_ws::LAST_MESSAGE_AT_MS,
        )
    }
}

/// Build a `ConnectionMetrics` from a feed's atomic counters. Shared across
/// the ib / crypto / signals adapters since they all use the same shape.
pub(super) fn feed_metrics_snapshot(
    msgs_in:  &std::sync::atomic::AtomicU64,
    parse_e:  &std::sync::atomic::AtomicU64,
    recon:    &std::sync::atomic::AtomicU32,
    last_ms:  &std::sync::atomic::AtomicI64,
) -> ConnectionMetrics {
    use std::sync::atomic::Ordering;
    let last = last_ms.load(Ordering::Relaxed);
    let last_message_at = if last > 0 {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(last);
        let age = (now_ms - last).max(0) as u64;
        std::time::Instant::now().checked_sub(std::time::Duration::from_millis(age))
    } else {
        None
    };
    ConnectionMetrics {
        messages_in:     msgs_in.load(Ordering::Relaxed),
        messages_out:    0,
        parse_errors:    parse_e.load(Ordering::Relaxed),
        reconnect_count: recon.load(Ordering::Relaxed),
        last_message_at,
        queue_depth:     None,
    }
}

#[async_trait::async_trait]
impl MarketDataProvider for IbProvider {
    async fn bars(&self, _: &str, _: &str, _: i64, _: i64, _: Option<usize>) -> Result<Vec<BarWire>, ApiError> {
        Err(ApiError::NotSupported("ib: historical bars not exposed".into()))
    }
    // Wave 12a: bridge to the per-symbol fanout hubs in `feeds::ib_ws`.
    // The hubs are populated by `ws_loop`'s tick decoder — each decoded
    // tick is fanned to bar / trade hubs unconditionally and to the
    // quote hub when bid/ask are present in the payload.
    //
    // IB ticks are trade-print shaped, so the bar stream synthesizes a
    // degenerate 1m OHLC (open=high=low=close=tick price). A real bar
    // aggregator is deferred — callers that need true bars should
    // resample downstream or use `MarketDataProvider::bars` for history.
    fn subscribe_bars(&self, sym: &str, tf: &str) -> Result<BarStream, ApiError> {
        if tf != "1m" {
            return Err(ApiError::NotSupported(format!(
                "ib: stream synthesizes 1m only, not {tf}"
            )));
        }
        Ok(ib_ws::hub_subscribe(ib_ws::bar_hub(), sym))
    }
    fn unsubscribe_bars(&self, _: &str, _: &str) {
        // Receiver-drop is sufficient — `hub_fanout` prunes the dead
        // sender on the next tick that touches the symbol.
    }
    fn subscribe_quotes(&self, sym: &str) -> Result<QuoteStream, ApiError> {
        Ok(ib_ws::hub_subscribe(ib_ws::quote_hub(), sym))
    }
    fn unsubscribe_quotes(&self, _: &str) {}
    fn subscribe_trades(&self, sym: &str) -> Result<TradeStream, ApiError> {
        Ok(ib_ws::hub_subscribe(ib_ws::trade_hub(), sym))
    }
    fn unsubscribe_trades(&self, _: &str) {}
    async fn chain_snapshot(&self, _: &str) -> Result<ChainSnapshot, ApiError> {
        Err(ApiError::NotSupported("ib: chain not implemented".into()))
    }
    fn subscribe_chain(&self, _: &str) -> Result<ChainStream, ApiError> {
        Err(ApiError::NotSupported("ib: chain not implemented".into()))
    }
    fn unsubscribe_chain(&self, _: &str) {}
    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            // Wave 12a: bars/quotes/trades stream via per-symbol fanout hubs
            // in `feeds::ib_ws`. Chain handling is its own beast and stays
            // off until a dedicated wave.
            bars: true, quotes: true, trades: true, chain: false,
            crypto_only: false, historical: false, realtime: true,
            fundamentals: false, news: false, earnings: false, corporate_actions: false,
        }
    }
}
