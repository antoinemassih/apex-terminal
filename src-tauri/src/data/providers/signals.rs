//! Adapter: `MarketDataProvider` over `feeds::signals_feed`.
//!
//! Signals is a publication feed (patterns / alerts / trendlines /
//! significance) — not market-data per se. It doesn't fit the
//! `MarketDataProvider` data shape cleanly, but exposing it as a
//! capability-zero provider lets the registry route its connection state /
//! shutdown through the same path as the other feeds.
//!
//! All methods return `NotSupported`. The signal publication path stays on
//! its existing global broadcast surface — no migration this wave.

use super::provider::{
    BarStream, ChainSnapshot, ChainStream, MarketDataProvider, ProviderCapabilities,
    QuoteStream, TradeStream,
};
use crate::data::connectivity::{
    ApiError, Connection, ConnectionMetrics, ConnectionState,
};
use crate::data::feeds::apex_data::types::BarWire;

pub struct SignalsProvider;

impl SignalsProvider {
    pub fn new() -> Self { Self }
}

impl Default for SignalsProvider {
    fn default() -> Self { Self::new() }
}

impl Connection for SignalsProvider {
    fn name(&self) -> &str { "signals" }
    fn state(&self) -> ConnectionState {
        use std::sync::atomic::Ordering;
        if crate::data::feeds::signals_feed::FORCE_RECONNECT.load(Ordering::Relaxed) {
            return ConnectionState::Backoff {
                until: std::time::Instant::now() + std::time::Duration::from_secs(1),
                attempt: crate::data::feeds::signals_feed::RECONNECT_COUNT.load(Ordering::Relaxed),
                reason: "tick_stalled".into(),
            };
        }
        ConnectionState::Idle
    }
    fn subscribe_state(&self) -> Option<tokio::sync::broadcast::Receiver<ConnectionState>> {
        // Wave 11c: push stream backed by `feeds::signals_feed::state_tx()`.
        Some(crate::data::feeds::signals_feed::state_tx().subscribe())
    }
    fn metrics(&self) -> ConnectionMetrics {
        let mut m = super::ib::feed_metrics_snapshot(
            &crate::data::feeds::signals_feed::MESSAGES_IN,
            &crate::data::feeds::signals_feed::PARSE_ERRORS,
            &crate::data::feeds::signals_feed::RECONNECT_COUNT,
            &crate::data::feeds::signals_feed::LAST_MESSAGE_AT_MS,
        );
        // P1.18: expose broadcast fanout queue depth (sum across all active subs).
        m.queue_depth = Some(super::registry::subscription_manager().total_queue_depth());
        m
    }
}

#[async_trait::async_trait]
impl MarketDataProvider for SignalsProvider {
    async fn bars(&self, _: &str, _: &str, _: i64, _: i64, _: Option<usize>) -> Result<Vec<BarWire>, ApiError> {
        Err(ApiError::NotSupported("signals: not a market-data source".into()))
    }
    fn subscribe_bars(&self, _: &str, _: &str) -> Result<BarStream, ApiError> { Err(ApiError::NotSupported("signals".into())) }
    fn unsubscribe_bars(&self, _: &str, _: &str) {}
    fn subscribe_quotes(&self, _: &str) -> Result<QuoteStream, ApiError> { Err(ApiError::NotSupported("signals".into())) }
    fn unsubscribe_quotes(&self, _: &str) {}
    fn subscribe_trades(&self, _: &str) -> Result<TradeStream, ApiError> { Err(ApiError::NotSupported("signals".into())) }
    fn unsubscribe_trades(&self, _: &str) {}
    async fn chain_snapshot(&self, _: &str) -> Result<ChainSnapshot, ApiError> { Err(ApiError::NotSupported("signals".into())) }
    fn subscribe_chain(&self, _: &str) -> Result<ChainStream, ApiError> { Err(ApiError::NotSupported("signals".into())) }
    fn unsubscribe_chain(&self, _: &str) {}
    fn capabilities(&self) -> ProviderCapabilities { ProviderCapabilities::default() }
}
