//! Wraps a `MarketDataProvider` with a Redis-backed bar cache.
//!
//! On `bars(...)`: check `crate::data::bar_cache` first; on miss, fetch from
//! the inner provider and populate the cache. Subscribe paths pass through
//! unchanged — live streams are not cached.

use super::provider::{
    BarStream, ChainSnapshot, ChainStream, MarketDataProvider, ProviderCapabilities,
    QuoteStream, TradeStream,
};
use crate::data::connectivity::{
    ApiError, Connection, ConnectionMetrics, ConnectionState,
};
use crate::data::feeds::apex_data::types::{AssetClass, BarWire};
use std::sync::Arc;

pub struct CachedProvider {
    inner: Arc<dyn MarketDataProvider>,
    name: String,
}

impl CachedProvider {
    pub fn new(inner: Arc<dyn MarketDataProvider>) -> Self {
        let name = format!("cached:{}", inner.name());
        Self { inner, name }
    }
}

impl Connection for CachedProvider {
    fn name(&self) -> &str { &self.name }
    fn state(&self) -> ConnectionState { self.inner.state() }
    fn metrics(&self) -> ConnectionMetrics { self.inner.metrics() }
}

fn to_bar_wires(bars: Vec<crate::data::Bar>, sym: &str, tf: &str) -> Vec<BarWire> {
    let class = AssetClass::from_symbol(sym);
    bars.into_iter()
        .map(|b| BarWire {
            symbol: sym.to_string(),
            asset_class: class,
            timeframe: tf.to_string(),
            time: b.time * 1000, // bar_cache stores seconds; BarWire wants ms
            open: b.open, high: b.high, low: b.low, close: b.close, volume: b.volume,
            vwap: 0.0, trades: 0, closed: true,
        })
        .collect()
}

fn to_internal_bars(bars: &[BarWire]) -> Vec<crate::data::Bar> {
    bars.iter()
        .map(|b| crate::data::Bar {
            time: b.time / 1000,
            open: b.open, high: b.high, low: b.low, close: b.close, volume: b.volume,
        })
        .collect()
}

#[async_trait::async_trait]
impl MarketDataProvider for CachedProvider {
    #[tracing::instrument(skip(self), level = "debug", fields(provider = %self.name, symbol, timeframe, limit))]
    async fn bars(
        &self,
        symbol: &str,
        timeframe: &str,
        start_ms: i64,
        end_ms: i64,
        limit: Option<usize>,
    ) -> Result<Vec<BarWire>, ApiError> {
        // Cache hit?
        if let Some(cached) = crate::data::bar_cache::get(symbol, timeframe) {
            if !cached.is_empty() {
                return Ok(to_bar_wires(cached, symbol, timeframe));
            }
        }
        let bars = self.inner.bars(symbol, timeframe, start_ms, end_ms, limit).await?;
        if !bars.is_empty() {
            // WS-H #42: only cache CLOSED bars. The still-forming current bar
            // (typically bars.last(), closed=false) would otherwise be served
            // stale from the cache within its TTL to another pane loading the
            // same symbol/timeframe — a stale price under an order-entry context.
            // The caller still receives ALL bars (including the live current one).
            let closed: Vec<BarWire> = bars.iter().filter(|b| b.closed).cloned().collect();
            if !closed.is_empty() {
                let internal = to_internal_bars(&closed);
                crate::data::bar_cache::set(symbol, timeframe, &internal);
            }
        }
        Ok(bars)
    }

    fn subscribe_bars(&self, s: &str, tf: &str) -> Result<BarStream, ApiError> { self.inner.subscribe_bars(s, tf) }
    fn unsubscribe_bars(&self, s: &str, tf: &str) { self.inner.unsubscribe_bars(s, tf) }
    fn subscribe_quotes(&self, s: &str) -> Result<QuoteStream, ApiError> { self.inner.subscribe_quotes(s) }
    fn unsubscribe_quotes(&self, s: &str) { self.inner.unsubscribe_quotes(s) }
    fn subscribe_trades(&self, s: &str) -> Result<TradeStream, ApiError> { self.inner.subscribe_trades(s) }
    fn unsubscribe_trades(&self, s: &str) { self.inner.unsubscribe_trades(s) }
    async fn chain_snapshot(&self, u: &str) -> Result<ChainSnapshot, ApiError> { self.inner.chain_snapshot(u).await }
    fn subscribe_chain(&self, u: &str) -> Result<ChainStream, ApiError> { self.inner.subscribe_chain(u) }
    fn unsubscribe_chain(&self, u: &str) { self.inner.unsubscribe_chain(u) }
    fn capabilities(&self) -> ProviderCapabilities { self.inner.capabilities() }
}
