//! `ReplayProvider` — replays a recorded session from disk for deterministic
//! UI testing.
//!
//! Wave-2 stub: skeleton + capability flags only. The actual record / playback
//! machinery lands in Wave 6 (see `docs/CONNECTIVITY_REFACTOR.md`).

use super::provider::{
    BarStream, ChainSnapshot, ChainStream, MarketDataProvider, ProviderCapabilities,
    QuoteStream, TradeStream,
};
use crate::data::connectivity::{
    ApiError, Connection, ConnectionMetrics, ConnectionState,
};
use crate::data::feeds::apex_data::types::BarWire;
use std::path::PathBuf;

pub struct ReplayProvider {
    name: String,
    #[allow(dead_code)]
    recording_path: PathBuf,
    #[allow(dead_code)]
    rate: f32,
}

impl ReplayProvider {
    pub fn new(name: impl Into<String>, recording_path: PathBuf, rate: f32) -> Self {
        Self { name: name.into(), recording_path, rate }
    }
}

impl Connection for ReplayProvider {
    fn name(&self) -> &str { &self.name }
    fn state(&self) -> ConnectionState { ConnectionState::Idle }
    fn metrics(&self) -> ConnectionMetrics { ConnectionMetrics::default() }
}

// TODO(wave-6): real recording parser + scheduled stream pump.
#[async_trait::async_trait]
impl MarketDataProvider for ReplayProvider {
    async fn bars(&self, _: &str, _: &str, _: i64, _: i64, _: Option<usize>) -> Result<Vec<BarWire>, ApiError> {
        Err(ApiError::NotSupported("replay: bars not yet implemented".into()))
    }
    fn subscribe_bars(&self, _: &str, _: &str) -> Result<BarStream, ApiError> {
        Err(ApiError::NotSupported("replay: streams not yet implemented".into()))
    }
    fn unsubscribe_bars(&self, _: &str, _: &str) {}
    fn subscribe_quotes(&self, _: &str) -> Result<QuoteStream, ApiError> {
        Err(ApiError::NotSupported("replay: streams not yet implemented".into()))
    }
    fn unsubscribe_quotes(&self, _: &str) {}
    fn subscribe_trades(&self, _: &str) -> Result<TradeStream, ApiError> {
        Err(ApiError::NotSupported("replay: streams not yet implemented".into()))
    }
    fn unsubscribe_trades(&self, _: &str) {}
    async fn chain_snapshot(&self, _: &str) -> Result<ChainSnapshot, ApiError> {
        Err(ApiError::NotSupported("replay: chain not yet implemented".into()))
    }
    fn subscribe_chain(&self, _: &str) -> Result<ChainStream, ApiError> {
        Err(ApiError::NotSupported("replay: streams not yet implemented".into()))
    }
    fn unsubscribe_chain(&self, _: &str) {}
    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities { historical: false, realtime: false, ..Default::default() }
    }
}
