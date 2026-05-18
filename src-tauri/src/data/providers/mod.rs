//! Wave-2 unified market-data provider layer.
//!
//! Sits between the existing per-feed modules (`feeds/apex_data`,
//! `feeds/ib_ws`, `feeds/crypto_feed`, `feeds/signals_feed`) and the
//! consumers that historically called those modules directly. Goal: one trait
//! (`MarketDataProvider`), one subscription manager (refcounted + deduped),
//! composable adapters for fallback / cache / replay.
//!
//! This wave keeps the existing UI delivery path (`NATIVE_CHART_TXS` broadcast
//! out of the feeds) intact — the new provider streams are a NEW surface.
//! Migration of UI consumers is Wave 5.

pub mod provider;
pub mod subscription_manager;
pub mod fallback;
pub mod cached;
pub mod replay;
pub mod apex_data;
pub mod ib;
pub mod crypto;
pub mod signals;
pub mod http_fallback;
pub mod registry;
pub mod mock;

pub use provider::{
    BarStream, ChainSnapshot, ChainStream, MarketDataProvider, ProviderCapabilities,
    QuoteStream, TradeStream,
};
pub use subscription_manager::SubscriptionManager;
pub use fallback::FallbackProvider;
pub use cached::CachedProvider;
pub use replay::ReplayProvider;
pub use http_fallback::HttpFallbackProvider;
pub use mock::{MockFrame, MockMarketDataProvider};
