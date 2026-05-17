//! Process-wide canonical provider chain.
//!
//! Lazy singleton: first access builds the `FallbackProvider` chain that
//! replaces the old 6-tier `if-else` ladder in
//! `chart/renderer/io/fetch.rs::fetch_bars_background`:
//!
//! 1. `CryptoProvider`         — short-circuits crypto symbols (ApexCrypto REST)
//! 2. `CachedProvider(ApexDataProvider)` — primary source w/ Redis cache
//! 3. `HttpFallbackProvider`   — OCOCO (InfluxDB cache)
//! 4. `HttpFallbackProvider`   — yfinance sidecar
//! 5. `HttpFallbackProvider`   — Yahoo Finance v8
//!
//! Anything past step 1 that doesn't satisfy `crypto_only` capabilities still
//! gets consulted for stocks; the crypto provider returns `NotSupported` for
//! non-crypto symbols so it cleanly steps aside.

use super::{
    apex_data::ApexDataProvider,
    cached::CachedProvider,
    crypto::CryptoProvider,
    fallback::FallbackProvider,
    http_fallback::{HttpFallbackProvider, Mode},
    MarketDataProvider,
};
use std::sync::{Arc, OnceLock};

static BAR_CHAIN: OnceLock<Arc<dyn MarketDataProvider>> = OnceLock::new();

/// The canonical bar-fetch chain. First call builds and caches; subsequent
/// calls return the same `Arc`.
pub fn bar_chain() -> Arc<dyn MarketDataProvider> {
    BAR_CHAIN
        .get_or_init(|| {
            let chain = FallbackProvider::new(
                "bar_chain",
                vec![
                    Arc::new(CryptoProvider::new()),
                    Arc::new(CachedProvider::new(Arc::new(ApexDataProvider::new()))),
                    Arc::new(HttpFallbackProvider::new(
                        "ococo",
                        "http://192.168.1.60:30300/api/bars?symbol={symbol}&interval={interval}&limit=500",
                        Mode::BarsJson,
                    ).with_timeout_ms(2000)),
                    Arc::new(HttpFallbackProvider::new(
                        "yfinance-sidecar",
                        "http://127.0.0.1:8777/bars?symbol={symbol}&interval={interval}&period={range}",
                        Mode::BarsJson,
                    ).with_timeout_ms(3000)),
                    Arc::new(HttpFallbackProvider::new(
                        "yahoo-v8",
                        "https://query1.finance.yahoo.com/v8/finance/chart/{symbol}?interval={interval}&range={range}",
                        Mode::YahooV8,
                    ).with_timeout_ms(5000)),
                ],
            );
            let arc: Arc<dyn MarketDataProvider> = Arc::new(chain);
            arc
        })
        .clone()
}
