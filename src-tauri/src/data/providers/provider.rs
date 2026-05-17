//! Wave-2 `MarketDataProvider` trait — unified surface every feed implements.
//!
//! Replaces the bespoke per-feed APIs (`apex_data::ws::add_bar_sub`,
//! `crypto_feed::run_feed`, `ib_ws::ib_ws_send`, …) with a single typed
//! contract so higher-level code (subscription manager, fallback chains,
//! cached wrappers, replay) can talk to any backend uniformly.
//!
//! All streams are `tokio::sync::mpsc::UnboundedReceiver<T>` — the codebase
//! already standardizes on tokio mpsc for the ApexData WS, so we keep that
//! everywhere for symmetry. Dropping the receiver implicitly cancels the
//! subscription on the manager side.

use crate::data::connectivity::{ApiError, Connection};
use crate::data::feeds::apex_data::types::{
    BarWire, ChainDelta, ChainResponse, Quote, Trade,
};

/// Snapshot of an options chain for an underlying.
///
/// Re-exports the existing `ChainResponse` type — providers do not yet need a
/// dedicated `ChainSnapshot` struct.
pub type ChainSnapshot = ChainResponse;

pub type BarStream = tokio::sync::mpsc::UnboundedReceiver<BarWire>;
pub type QuoteStream = tokio::sync::mpsc::UnboundedReceiver<Quote>;
pub type TradeStream = tokio::sync::mpsc::UnboundedReceiver<Trade>;
pub type ChainStream = tokio::sync::mpsc::UnboundedReceiver<ChainDelta>;

/// Honest self-description: providers advertise what they can actually do.
///
/// Callers (`FallbackProvider`, registries, the UI) consult this rather than
/// blindly probing methods and treating `NotSupported` as a soft error.
#[derive(Debug, Clone, Copy, Default)]
pub struct ProviderCapabilities {
    pub bars: bool,
    pub quotes: bool,
    pub trades: bool,
    pub chain: bool,
    pub crypto_only: bool,
    /// Can serve historical bars via `bars(...)`.
    pub historical: bool,
    /// Can serve live streams via `subscribe_*`.
    pub realtime: bool,
}

/// Unified market-data surface.
///
/// Every concrete impl is `Arc<dyn MarketDataProvider>` so the manager,
/// fallback chain, and cached wrapper can compose them without owning each
/// other's lifetimes.
#[async_trait::async_trait]
pub trait MarketDataProvider: Connection {
    /// Historical bars — pulled from REST, replay, or cache.
    async fn bars(
        &self,
        symbol: &str,
        timeframe: &str,
        start_ms: i64,
        end_ms: i64,
        limit: Option<usize>,
    ) -> Result<Vec<BarWire>, ApiError>;

    /// Live bar stream. Drop the receiver to cancel.
    fn subscribe_bars(&self, symbol: &str, timeframe: &str) -> Result<BarStream, ApiError>;
    fn unsubscribe_bars(&self, symbol: &str, timeframe: &str);

    /// NBBO quotes.
    fn subscribe_quotes(&self, symbol: &str) -> Result<QuoteStream, ApiError>;
    fn unsubscribe_quotes(&self, symbol: &str);

    /// Tape (prints).
    fn subscribe_trades(&self, symbol: &str) -> Result<TradeStream, ApiError>;
    fn unsubscribe_trades(&self, symbol: &str);

    /// Options chain — snapshot + delta stream.
    async fn chain_snapshot(&self, underlying: &str) -> Result<ChainSnapshot, ApiError>;
    fn subscribe_chain(&self, underlying: &str) -> Result<ChainStream, ApiError>;
    fn unsubscribe_chain(&self, underlying: &str);

    /// What does this provider actually support? Callers should inspect
    /// before composing a fallback chain — `NotSupported` errors are still
    /// returned at call time as a safety net.
    fn capabilities(&self) -> ProviderCapabilities;
}
