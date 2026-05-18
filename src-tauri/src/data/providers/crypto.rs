//! Adapter: `MarketDataProvider` over `feeds::crypto_feed`.
//!
//! The existing crypto feed is a singleton WS pump that broadcasts bar/trade
//! frames straight into `NATIVE_CHART_TXS` for the chart renderer. It does
//! NOT expose a per-symbol stream API yet. This adapter implements the
//! historical `bars(...)` path (calls the ApexCrypto REST endpoint that the
//! fetch fallback chain used to call inline) and returns `NotSupported` for
//! the per-symbol stream methods — wired through the existing global pump in
//! `crypto_feed::start()`.

use super::provider::{
    BarStream, ChainSnapshot, ChainStream, MarketDataProvider, ProviderCapabilities,
    QuoteStream, TradeStream,
};
use crate::data::connectivity::{
    ApiError, Connection, ConnectionMetrics, ConnectionState,
};
use crate::data::feeds::apex_data::types::{AssetClass, BarWire};

const APEX_CRYPTO_BARS: &str = "http://192.168.1.56:30840/api/bars";

pub struct CryptoProvider;

impl CryptoProvider {
    pub fn new() -> Self { Self }
}

impl Default for CryptoProvider {
    fn default() -> Self { Self::new() }
}

impl Connection for CryptoProvider {
    fn name(&self) -> &str { "crypto" }
    fn state(&self) -> ConnectionState {
        use std::sync::atomic::Ordering;
        if crate::data::feeds::crypto_feed::FORCE_RECONNECT.load(Ordering::Relaxed) {
            return ConnectionState::Backoff {
                until: std::time::Instant::now() + std::time::Duration::from_secs(1),
                attempt: crate::data::feeds::crypto_feed::RECONNECT_COUNT.load(Ordering::Relaxed),
                reason: "tick_stalled".into(),
            };
        }
        ConnectionState::Idle
    }
    fn metrics(&self) -> ConnectionMetrics {
        super::ib::feed_metrics_snapshot(
            &crate::data::feeds::crypto_feed::MESSAGES_IN,
            &crate::data::feeds::crypto_feed::PARSE_ERRORS,
            &crate::data::feeds::crypto_feed::RECONNECT_COUNT,
            &crate::data::feeds::crypto_feed::LAST_MESSAGE_AT_MS,
        )
    }
}

#[async_trait::async_trait]
impl MarketDataProvider for CryptoProvider {
    async fn bars(
        &self,
        symbol: &str,
        timeframe: &str,
        _start_ms: i64,
        _end_ms: i64,
        _limit: Option<usize>,
    ) -> Result<Vec<BarWire>, ApiError> {
        // Crypto only — gate so non-crypto symbols cleanly fall through to the
        // next provider in a `FallbackProvider` chain.
        if !crate::data::is_crypto(symbol) {
            return Err(ApiError::NotSupported("crypto: non-crypto symbol".into()));
        }
        let sym = symbol.to_string();
        let tf = timeframe.to_string();
        let url = format!("{}/{}/{}", APEX_CRYPTO_BARS, sym, tf);
        let res: Result<Vec<crate::data::Bar>, ApiError> = tokio::task::spawn_blocking(move || {
            let client = reqwest::blocking::Client::builder()
                .user_agent("apex-terminal/crypto")
                .build()
                .map_err(|e| ApiError::Network(e.to_string()))?;
            let resp = client
                .get(&url)
                .timeout(std::time::Duration::from_secs(5))
                .send()
                .map_err(|e| ApiError::Network(e.to_string()))?;
            if !resp.status().is_success() {
                return Err(ApiError::Http { status: resp.status().as_u16(), body: "ApexCrypto".into() });
            }
            resp.json::<Vec<crate::data::Bar>>()
                .map_err(|e| ApiError::Parse(e.to_string()))
        })
        .await
        .map_err(|e| ApiError::Network(format!("join: {e}")))?;

        let bars = res?;
        let class = AssetClass::from_symbol(&sym);
        Ok(bars
            .into_iter()
            .map(|b| BarWire {
                symbol: sym.clone(),
                asset_class: class,
                timeframe: tf.clone(),
                time: b.time * 1000,
                open: b.open, high: b.high, low: b.low, close: b.close, volume: b.volume,
                vwap: 0.0, trades: 0, closed: true,
            })
            .collect())
    }

    fn subscribe_bars(&self, _: &str, _: &str) -> Result<BarStream, ApiError> {
        Err(ApiError::NotSupported("crypto: per-symbol stream not yet exposed".into()))
    }
    fn unsubscribe_bars(&self, _: &str, _: &str) {}
    fn subscribe_quotes(&self, _: &str) -> Result<QuoteStream, ApiError> {
        Err(ApiError::NotSupported("crypto: quotes not exposed".into()))
    }
    fn unsubscribe_quotes(&self, _: &str) {}
    fn subscribe_trades(&self, _: &str) -> Result<TradeStream, ApiError> {
        Err(ApiError::NotSupported("crypto: trades — broadcast via NATIVE_CHART_TXS".into()))
    }
    fn unsubscribe_trades(&self, _: &str) {}
    async fn chain_snapshot(&self, _: &str) -> Result<ChainSnapshot, ApiError> {
        Err(ApiError::NotSupported("crypto: no options".into()))
    }
    fn subscribe_chain(&self, _: &str) -> Result<ChainStream, ApiError> {
        Err(ApiError::NotSupported("crypto: no options".into()))
    }
    fn unsubscribe_chain(&self, _: &str) {}
    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            bars: true, quotes: false, trades: false, chain: false,
            crypto_only: true, historical: true, realtime: false,
            fundamentals: false, news: false, earnings: false, corporate_actions: false,
        }
    }
}
