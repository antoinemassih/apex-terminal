//! `FallbackProvider` — tries providers in order; first non-error wins.
//!
//! Replaces the manual 6-tier `if-else` cascade in
//! `chart/renderer/io/fetch.rs::fetch_bars_background` with a composable
//! chain. Each call to `bars(...)` (the only method most fallbacks need to
//! cover) walks the provider list, per-provider-timeout-bounded, and returns
//! the first `Ok` it gets. Errors are merged into a single `ApiError` so the
//! caller sees a meaningful failure mode.
//!
//! Subscribe paths route to the FIRST provider whose `capabilities().realtime`
//! is true — fallback for streams doesn't make sense (you can't mix live
//! feeds halfway through a session) but we expose it for completeness.

use super::provider::{
    BarStream, ChainSnapshot, ChainStream, MarketDataProvider, ProviderCapabilities,
    QuoteStream, TradeStream,
};
use crate::data::connectivity::{
    ApiError, Connection, ConnectionMetrics, ConnectionState,
};
use crate::data::feeds::apex_data::types::BarWire;
use std::sync::Arc;
use std::time::Duration;

pub struct FallbackProvider {
    name: String,
    providers: Vec<Arc<dyn MarketDataProvider>>,
    per_call_timeout: Duration,
}

impl FallbackProvider {
    pub fn new(name: impl Into<String>, providers: Vec<Arc<dyn MarketDataProvider>>) -> Self {
        Self {
            name: name.into(),
            providers,
            per_call_timeout: Duration::from_secs(5),
        }
    }

    pub fn with_timeout(mut self, t: Duration) -> Self {
        self.per_call_timeout = t;
        self
    }

    fn first_realtime(&self) -> Option<&Arc<dyn MarketDataProvider>> {
        self.providers
            .iter()
            .find(|p| p.capabilities().realtime)
    }
}

impl Connection for FallbackProvider {
    fn name(&self) -> &str { &self.name }
    fn state(&self) -> ConnectionState {
        // Aggregate: if any inner provider is Subscribed, we're Subscribed.
        for p in &self.providers {
            if matches!(p.state(), ConnectionState::Subscribed { .. } | ConnectionState::Authenticated) {
                return p.state();
            }
        }
        ConnectionState::Idle
    }
    fn metrics(&self) -> ConnectionMetrics { ConnectionMetrics::default() }
}

#[async_trait::async_trait]
impl MarketDataProvider for FallbackProvider {
    async fn bars(
        &self,
        symbol: &str,
        timeframe: &str,
        start_ms: i64,
        end_ms: i64,
        limit: Option<usize>,
    ) -> Result<Vec<BarWire>, ApiError> {
        let mut errors: Vec<String> = Vec::new();
        for p in &self.providers {
            if !p.capabilities().bars {
                continue;
            }
            let fut = p.bars(symbol, timeframe, start_ms, end_ms, limit);
            match tokio::time::timeout(self.per_call_timeout, fut).await {
                Ok(Ok(bars)) if !bars.is_empty() => return Ok(bars),
                Ok(Ok(_)) => errors.push(format!("{}: empty", p.name())),
                Ok(Err(e)) => errors.push(format!("{}: {}", p.name(), e)),
                Err(_) => errors.push(format!("{}: timeout", p.name())),
            }
        }
        Err(ApiError::Network(format!(
            "all providers failed [{}]",
            errors.join(" | ")
        )))
    }

    fn subscribe_bars(&self, symbol: &str, timeframe: &str) -> Result<BarStream, ApiError> {
        self.first_realtime()
            .ok_or_else(|| ApiError::NotSupported("no realtime provider".into()))
            .and_then(|p| p.subscribe_bars(symbol, timeframe))
    }
    fn unsubscribe_bars(&self, symbol: &str, timeframe: &str) {
        if let Some(p) = self.first_realtime() {
            p.unsubscribe_bars(symbol, timeframe);
        }
    }
    fn subscribe_quotes(&self, symbol: &str) -> Result<QuoteStream, ApiError> {
        self.first_realtime()
            .ok_or_else(|| ApiError::NotSupported("no realtime provider".into()))
            .and_then(|p| p.subscribe_quotes(symbol))
    }
    fn unsubscribe_quotes(&self, symbol: &str) {
        if let Some(p) = self.first_realtime() {
            p.unsubscribe_quotes(symbol);
        }
    }
    fn subscribe_trades(&self, symbol: &str) -> Result<TradeStream, ApiError> {
        self.first_realtime()
            .ok_or_else(|| ApiError::NotSupported("no realtime provider".into()))
            .and_then(|p| p.subscribe_trades(symbol))
    }
    fn unsubscribe_trades(&self, symbol: &str) {
        if let Some(p) = self.first_realtime() {
            p.unsubscribe_trades(symbol);
        }
    }
    async fn chain_snapshot(&self, u: &str) -> Result<ChainSnapshot, ApiError> {
        let mut errors: Vec<String> = Vec::new();
        for p in &self.providers {
            if !p.capabilities().chain { continue; }
            match tokio::time::timeout(self.per_call_timeout, p.chain_snapshot(u)).await {
                Ok(Ok(s)) => return Ok(s),
                Ok(Err(e)) => errors.push(format!("{}: {}", p.name(), e)),
                Err(_) => errors.push(format!("{}: timeout", p.name())),
            }
        }
        Err(ApiError::Network(format!("no chain provider [{}]", errors.join(" | "))))
    }
    fn subscribe_chain(&self, u: &str) -> Result<ChainStream, ApiError> {
        for p in &self.providers {
            if p.capabilities().chain && p.capabilities().realtime {
                return p.subscribe_chain(u);
            }
        }
        Err(ApiError::NotSupported("no chain provider".into()))
    }
    fn unsubscribe_chain(&self, u: &str) {
        for p in &self.providers {
            if p.capabilities().chain && p.capabilities().realtime {
                p.unsubscribe_chain(u);
                return;
            }
        }
    }
    fn capabilities(&self) -> ProviderCapabilities {
        // Union over inner provider capabilities.
        let mut c = ProviderCapabilities::default();
        for p in &self.providers {
            let pc = p.capabilities();
            c.bars       |= pc.bars;
            c.quotes     |= pc.quotes;
            c.trades     |= pc.trades;
            c.chain      |= pc.chain;
            c.historical |= pc.historical;
            c.realtime   |= pc.realtime;
            // crypto_only is true only if EVERY provider is crypto_only.
            c.crypto_only = if c == ProviderCapabilities::default() {
                pc.crypto_only
            } else {
                c.crypto_only && pc.crypto_only
            };
        }
        c
    }
}

impl PartialEq for ProviderCapabilities {
    fn eq(&self, other: &Self) -> bool {
        self.bars == other.bars
            && self.quotes == other.quotes
            && self.trades == other.trades
            && self.chain == other.chain
            && self.crypto_only == other.crypto_only
            && self.historical == other.historical
            && self.realtime == other.realtime
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::feeds::apex_data::types::AssetClass;
    use crate::data::providers::provider::{
        BarStream, ChainStream, QuoteStream, TradeStream,
    };
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct StubProvider {
        name: &'static str,
        calls: AtomicUsize,
        return_ok: bool,
    }

    impl Connection for StubProvider {
        fn name(&self) -> &str { self.name }
        fn state(&self) -> ConnectionState { ConnectionState::Idle }
        fn metrics(&self) -> ConnectionMetrics { ConnectionMetrics::default() }
    }

    #[async_trait::async_trait]
    impl MarketDataProvider for StubProvider {
        async fn bars(&self, _s: &str, _tf: &str, _a: i64, _b: i64, _l: Option<usize>) -> Result<Vec<BarWire>, ApiError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            if self.return_ok {
                Ok(vec![BarWire {
                    symbol: "X".into(), asset_class: AssetClass::Stock, timeframe: "5m".into(),
                    time: 0, open: 1.0, high: 1.0, low: 1.0, close: 1.0, volume: 0.0,
                    vwap: 0.0, trades: 0, closed: true,
                }])
            } else {
                Err(ApiError::Network("nope".into()))
            }
        }
        fn subscribe_bars(&self, _: &str, _: &str) -> Result<BarStream, ApiError> { Err(ApiError::NotSupported("x".into())) }
        fn unsubscribe_bars(&self, _: &str, _: &str) {}
        fn subscribe_quotes(&self, _: &str) -> Result<QuoteStream, ApiError> { Err(ApiError::NotSupported("x".into())) }
        fn unsubscribe_quotes(&self, _: &str) {}
        fn subscribe_trades(&self, _: &str) -> Result<TradeStream, ApiError> { Err(ApiError::NotSupported("x".into())) }
        fn unsubscribe_trades(&self, _: &str) {}
        async fn chain_snapshot(&self, _: &str) -> Result<ChainSnapshot, ApiError> { Err(ApiError::NotSupported("x".into())) }
        fn subscribe_chain(&self, _: &str) -> Result<ChainStream, ApiError> { Err(ApiError::NotSupported("x".into())) }
        fn unsubscribe_chain(&self, _: &str) {}
        fn capabilities(&self) -> ProviderCapabilities {
            ProviderCapabilities { bars: true, historical: true, ..Default::default() }
        }
    }

    #[tokio::test]
    async fn returns_first_ok_skipping_failures() {
        let a = Arc::new(StubProvider { name: "a", calls: AtomicUsize::new(0), return_ok: false });
        let b = Arc::new(StubProvider { name: "b", calls: AtomicUsize::new(0), return_ok: true });
        let c = Arc::new(StubProvider { name: "c", calls: AtomicUsize::new(0), return_ok: true });
        let fb = FallbackProvider::new("test", vec![a.clone(), b.clone(), c.clone()]);
        let bars = fb.bars("X", "5m", 0, 0, None).await.unwrap();
        assert_eq!(bars.len(), 1);
        assert_eq!(a.calls.load(Ordering::SeqCst), 1);
        assert_eq!(b.calls.load(Ordering::SeqCst), 1);
        assert_eq!(c.calls.load(Ordering::SeqCst), 0, "third not consulted");
    }

    #[tokio::test]
    async fn errors_accumulate_when_all_fail() {
        let a = Arc::new(StubProvider { name: "a", calls: AtomicUsize::new(0), return_ok: false });
        let b = Arc::new(StubProvider { name: "b", calls: AtomicUsize::new(0), return_ok: false });
        let fb = FallbackProvider::new("test", vec![a, b]);
        let err = fb.bars("X", "5m", 0, 0, None).await.unwrap_err();
        let msg = format!("{}", err);
        assert!(msg.contains("a:"));
        assert!(msg.contains("b:"));
    }
}
