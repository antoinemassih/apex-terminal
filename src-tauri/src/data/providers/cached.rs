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

/// Clip a cached bar series to the range/limit the caller actually asked for.
///
/// AUDIT 2026-08-02 (AT-118 / AT-002, P0): the bar cache is keyed on
/// `{symbol}:{timeframe}` with no range dimension, so a hit could serve any
/// stored span. Returning it unclipped is what let a small gap-fill window
/// receive the entire cached history. Pure fn so the boundary behaviour is
/// testable without Redis.
///
/// `start_ms`/`end_ms` of `<= 0` mean "unbounded" — several callers pass 0 for
/// "whatever you have". `limit` means "the most recent N".
fn window_cached(
    wires: Vec<BarWire>,
    start_ms: i64,
    end_ms: i64,
    limit: Option<usize>,
) -> Vec<BarWire> {
    let mut out: Vec<BarWire> = wires
        .into_iter()
        .filter(|b| (start_ms <= 0 || b.time >= start_ms)
                 && (end_ms   <= 0 || b.time <= end_ms))
        .collect();
    if let Some(n) = limit {
        if out.len() > n {
            let excess = out.len() - n;
            out.drain(..excess);
        }
    }
    out
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
        //
        // AUDIT 2026-08-02 (AT-118 / AT-002, P0): this used to return the cached
        // blob verbatim, ignoring start_ms, end_ms AND limit. The cache key is
        // `apex:bars:{sym}:{tf}` with no range dimension (bar_cache.rs:58), so
        // whatever range happened to be stored first was served for EVERY
        // subsequent request regardless of what was asked for.
        //
        // That is what made the gap-fill replay a P0: on reconnect,
        // `subscription_manager::gap_fill_on_reconnect` asks for
        // [last_seen_ts, now] — a small catch-up window — and got back the full
        // cached history (potentially years of daily bars). Every one of those
        // was then pushed through `send_to_native_chart(AppendBar)`, appending
        // stale bars after the current one and corrupting the series.
        //
        // Honour the requested window. The cache is a superset, so slicing it
        // keeps the cache benefit while respecting the contract. A `start`/`end`
        // of <= 0 means "unbounded" — several callers pass 0 for "whatever you
        // have" and must keep working.
        if let Some(cached) = crate::data::bar_cache::get(symbol, timeframe) {
            if !cached.is_empty() {
                let wires = to_bar_wires(cached, symbol, timeframe);
                let windowed = window_cached(wires, start_ms, end_ms, limit);
                // An empty slice is NOT a cache hit — it means the cache holds
                // nothing for the requested window. Fall through to the provider
                // rather than reporting "no bars exist".
                if !windowed.is_empty() {
                    return Ok(windowed);
                }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn wire(time_ms: i64) -> BarWire {
        BarWire {
            symbol: "SPY".into(),
            asset_class: AssetClass::from_symbol("SPY"),
            timeframe: "1m".into(),
            time: time_ms,
            open: 1.0, high: 1.0, low: 1.0, close: 1.0, volume: 1.0,
            vwap: 0.0, trades: 0, closed: true,
        }
    }

    /// AUDIT 2026-08-02 (AT-118 / AT-002, P0): the cache key has no range
    /// dimension, so a hit could serve any stored span. Returning it unclipped
    /// is what turned a small gap-fill catch-up window into a replay of the
    /// entire cached history straight down the live AppendBar path.
    #[test]
    fn cached_bars_are_clipped_to_the_requested_window() {
        // A cache holding a long history, minute bars from t=1000 to t=10000.
        let cached: Vec<BarWire> = (1..=10).map(|i| wire(i * 1000)).collect();

        // Gap-fill asks only for the catch-up window [8000, 10000].
        let got = window_cached(cached.clone(), 8000, 10000, None);

        assert_eq!(got.len(), 3, "only bars inside the window may be returned");
        assert_eq!(got.first().map(|b| b.time), Some(8000));
        assert_eq!(got.last().map(|b| b.time), Some(10000));
        assert!(got.iter().all(|b| b.time >= 8000 && b.time <= 10000),
            "a bar outside the requested window is exactly the P0: replayed \
             into the live append path it lands AFTER the current bar");
    }

    #[test]
    fn unbounded_start_and_end_are_honoured() {
        let cached: Vec<BarWire> = (1..=5).map(|i| wire(i * 1000)).collect();
        // Several callers pass 0 meaning "whatever you have" — must not clip.
        assert_eq!(window_cached(cached.clone(), 0, 0, None).len(), 5);
        assert_eq!(window_cached(cached.clone(), 0, 3000, None).len(), 3);
        assert_eq!(window_cached(cached, 3000, 0, None).len(), 3);
    }

    #[test]
    fn limit_keeps_the_most_recent_bars() {
        let cached: Vec<BarWire> = (1..=10).map(|i| wire(i * 1000)).collect();
        let got = window_cached(cached, 0, 0, Some(3));
        assert_eq!(got.len(), 3);
        assert_eq!(got.first().map(|b| b.time), Some(8000),
            "limit means the most RECENT n — trim from the front, not the back");
        assert_eq!(got.last().map(|b| b.time), Some(10000));
    }

    #[test]
    fn a_window_the_cache_cannot_cover_yields_empty_so_the_caller_refetches() {
        let cached: Vec<BarWire> = (1..=5).map(|i| wire(i * 1000)).collect();
        // Requested window is entirely newer than anything cached.
        let got = window_cached(cached, 50_000, 60_000, None);
        assert!(got.is_empty(),
            "an empty result must fall through to the provider rather than \
             being reported as 'no bars exist'");
    }
}
