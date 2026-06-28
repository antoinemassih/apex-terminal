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
    subscription_manager::SubscriptionManager,
    MarketDataProvider,
};
use std::sync::{Arc, OnceLock};

static BAR_CHAIN: OnceLock<Arc<dyn MarketDataProvider>> = OnceLock::new();
static SUBSCRIPTION_MANAGER: OnceLock<Arc<SubscriptionManager>> = OnceLock::new();

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
                        &format!("{}/api/bars?symbol={{symbol}}&interval={{interval}}&limit=500",
                            crate::data::endpoints::ococo_http()),
                        Mode::BarsJson,
                    ).with_timeout_ms(2000)),
                    Arc::new(HttpFallbackProvider::new(
                        "yfinance-sidecar",
                        &format!("{}/bars?symbol={{symbol}}&interval={{interval}}&period={{range}}",
                            crate::data::endpoints::yfin_sidecar()),
                        Mode::BarsJson,
                    ).with_timeout_ms(3000)),
                    Arc::new(HttpFallbackProvider::new(
                        "yahoo-v8",
                        &format!("{}/v8/finance/chart/{{symbol}}?interval={{interval}}&range={{range}}",
                            crate::data::endpoints::yahoo_chart()),
                        Mode::YahooV8,
                    ).with_timeout_ms(5000)),
                ],
            );
            let arc: Arc<dyn MarketDataProvider> = Arc::new(chain);
            arc
        })
        .clone()
}

/// Process-wide `SubscriptionManager` wrapping `bar_chain()`.
///
/// Wave 7E: instantiated so the WS feeds can call
/// `subscription_manager().gap_fill_on_reconnect_all()` from their reconnect
/// hooks. This is currently the SUM of (a) the live registry-anchored
/// dedup/refcount surface and (b) the gap-fill replay surface — the latter is
/// what Wave 7E needs. UI consumers do not yet route their subscribes through
/// here; that bigger migration is tracked separately. See subscription_manager.rs.
pub fn subscription_manager() -> Arc<SubscriptionManager> {
    SUBSCRIPTION_MANAGER
        .get_or_init(|| Arc::new(SubscriptionManager::new(bar_chain())))
        .clone()
}
