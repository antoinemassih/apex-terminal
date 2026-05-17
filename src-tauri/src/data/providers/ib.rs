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
        // No connection-state hook in feeds::ib_ws yet — return Idle. Wave-3
        // can plumb the connect/disconnect events through here.
        ConnectionState::Idle
    }
    fn metrics(&self) -> ConnectionMetrics { ConnectionMetrics::default() }
}

#[async_trait::async_trait]
impl MarketDataProvider for IbProvider {
    async fn bars(&self, _: &str, _: &str, _: i64, _: i64, _: Option<usize>) -> Result<Vec<BarWire>, ApiError> {
        Err(ApiError::NotSupported("ib: historical bars not exposed".into()))
    }
    fn subscribe_bars(&self, _: &str, _: &str) -> Result<BarStream, ApiError> {
        Err(ApiError::NotSupported("ib: bar stream — use Tauri ib-tick events".into()))
    }
    fn unsubscribe_bars(&self, _: &str, _: &str) {}
    fn subscribe_quotes(&self, _: &str) -> Result<QuoteStream, ApiError> {
        Err(ApiError::NotSupported("ib: quotes — use Tauri ib-tick events".into()))
    }
    fn unsubscribe_quotes(&self, _: &str) {}
    fn subscribe_trades(&self, _: &str) -> Result<TradeStream, ApiError> {
        Err(ApiError::NotSupported("ib: trades — use Tauri ib-tick events".into()))
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
            // Honest: the trait surface isn't wired yet for IB.
            bars: false, quotes: false, trades: false, chain: false,
            crypto_only: false, historical: false, realtime: false,
        }
    }
}
