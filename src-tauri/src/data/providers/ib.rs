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
        let mut m = feed_metrics_snapshot(
            &crate::data::feeds::ib_ws::MESSAGES_IN,
            &crate::data::feeds::ib_ws::PARSE_ERRORS,
            &crate::data::feeds::ib_ws::RECONNECT_COUNT,
            &crate::data::feeds::ib_ws::LAST_MESSAGE_AT_MS,
        );
        // P1.18: surface broadcast fanout queue depth so slow IB subscribers
        // can be detected before RecvError::Lagged is triggered.
        m.queue_depth = Some(super::registry::subscription_manager().total_queue_depth());
        m
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
    // Wave 12a/13b/14b: bridge to the per-(symbol, tf) bar fanout hub in
    // `feeds::ib_ws`. The hub is populated by `ws_loop`'s tick decoder via
    // the `OhlcAggregator`, which keeps a rolling OHLC bucket per active
    // `(sym, tf)` slot and emits live + closed bars on each tick. Quotes
    // and trades use the symbol-keyed hubs (no TF concept on a tick).
    //
    // Wave 14b lifted the 1m-only restriction: any standard timeframe
    // string the aggregator recognizes ("1s"…"1d") is now valid, and the
    // active-TF set is driven by `bar_hub_subscribe` so the tick path
    // knows which buckets to fold each tick into. Unknown TFs fall back
    // to 1m buckets inside the aggregator (with a one-shot warn) rather
    // than failing here — keeps the provider permissive while the
    // bucketing layer owns TF validation.
    fn subscribe_bars(&self, sym: &str, tf: &str) -> Result<BarStream, ApiError> {
        Ok(ib_ws::bar_hub_subscribe(sym, tf))
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

// ── Wave 14b: subscribe_bars TF acceptance tests ─────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::providers::provider::MarketDataProvider;

    #[test]
    fn subscribe_bars_accepts_standard_timeframes() {
        let p = IbProvider::new();
        // All standard TFs should produce a live receiver — no NotSupported.
        for tf in ["1m", "5m", "15m", "1h", "1d"] {
            let sym = format!("TEST_IBP_{}", tf);
            let rx = p.subscribe_bars(&sym, tf)
                .unwrap_or_else(|e| panic!("subscribe_bars({tf}) failed: {e:?}"));
            // Drop the receiver — next tick fanout (if any) will prune it.
            drop(rx);
        }
    }

    #[test]
    fn subscribe_bars_accepts_unknown_tf_via_1m_fallback() {
        // Wave 14b made the provider permissive: unknown TFs are routed
        // through the aggregator's 1m fallback (with a one-shot warn) rather
        // than rejected at the provider boundary.
        let p = IbProvider::new();
        let _rx = p.subscribe_bars("TEST_IBP_UNKNOWN", "7m")
            .expect("unknown TF should not error at provider");
    }
}
