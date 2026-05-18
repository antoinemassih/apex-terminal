//! Programmable `MarketDataProvider` for tests.
//!
//! Wave 6: lets integration tests drive the connectivity stack without standing
//! up real WS / REST services. Each stream is scripted as a `VecDeque<MockFrame>`
//! — the test pushes frames, the provider drains them on `subscribe_*` and
//! drives a `tokio::sync::mpsc::UnboundedSender` from a background task. Frames
//! can also force disconnects, reconnects, parse errors, or sleeps so tests
//! can assert behavior in adverse network conditions.
//!
//! Not wired into the production registry — pulled in by `tests/` and by
//! Rust-side `#[cfg(test)]` modules that need a deterministic feed.

use super::provider::{
    BarStream, ChainSnapshot, ChainStream, MarketDataProvider, ProviderCapabilities,
    QuoteStream, TradeStream,
};
use crate::data::connectivity::{
    ApiError, Connection, ConnectionMetrics, ConnectionState,
};
use crate::data::feeds::apex_data::types::{BarWire, ChainDelta, Quote, Trade};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::mpsc;

/// A scripted unit of mock-feed behavior. Drained in order by the background
/// stream task.
#[derive(Debug, Clone)]
pub enum MockFrame {
    Bar(BarWire),
    Quote(Quote),
    Trade(Trade),
    /// Force the provider's `ConnectionState` to `Backoff` mid-stream — useful
    /// for testing reconnect behavior in the SubscriptionManager.
    Disconnect,
    /// Force the provider's `ConnectionState` back to `Subscribed { count: 1 }`.
    Reconnect,
    /// Emit a parse-error event (bumps `parse_errors`).
    Error(String),
    /// Pause `dur` before delivering the next frame.
    Sleep(Duration),
}

pub struct MockMarketDataProvider {
    name: String,
    state: Arc<Mutex<ConnectionState>>,
    metrics: Arc<Mutex<ConnectionMetrics>>,
    bar_script: Arc<Mutex<VecDeque<MockFrame>>>,
    quote_script: Arc<Mutex<VecDeque<MockFrame>>>,
    trade_script: Arc<Mutex<VecDeque<MockFrame>>>,
    historical_bars: Arc<Mutex<Vec<BarWire>>>,
    /// When `true`, `bars()` returns an error to simulate a REST failure.
    historical_error: Arc<Mutex<Option<String>>>,
    capabilities: ProviderCapabilities,
}

impl MockMarketDataProvider {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            state: Arc::new(Mutex::new(ConnectionState::Idle)),
            metrics: Arc::new(Mutex::new(ConnectionMetrics::default())),
            bar_script: Arc::new(Mutex::new(VecDeque::new())),
            quote_script: Arc::new(Mutex::new(VecDeque::new())),
            trade_script: Arc::new(Mutex::new(VecDeque::new())),
            historical_bars: Arc::new(Mutex::new(Vec::new())),
            historical_error: Arc::new(Mutex::new(None)),
            capabilities: ProviderCapabilities {
                bars: true, quotes: true, trades: true, chain: false,
                crypto_only: false, historical: true, realtime: true,
                fundamentals: false, news: false, earnings: false, corporate_actions: false,
            },
        }
    }

    pub fn script_bars(self, frames: Vec<MockFrame>) -> Self {
        *self.bar_script.lock().unwrap() = VecDeque::from(frames);
        self
    }

    pub fn script_quotes(self, frames: Vec<MockFrame>) -> Self {
        *self.quote_script.lock().unwrap() = VecDeque::from(frames);
        self
    }

    pub fn script_trades(self, frames: Vec<MockFrame>) -> Self {
        *self.trade_script.lock().unwrap() = VecDeque::from(frames);
        self
    }

    pub fn historical_bars(self, bars: Vec<BarWire>) -> Self {
        *self.historical_bars.lock().unwrap() = bars;
        self
    }

    pub fn historical_error(self, msg: impl Into<String>) -> Self {
        *self.historical_error.lock().unwrap() = Some(msg.into());
        self
    }

    pub fn with_capabilities(mut self, caps: ProviderCapabilities) -> Self {
        self.capabilities = caps;
        self
    }

    pub fn metrics_snapshot(&self) -> ConnectionMetrics {
        self.metrics.lock().map(|g| g.clone()).unwrap_or_default()
    }

    pub fn state_snapshot(&self) -> ConnectionState {
        self.state.lock().map(|g| g.clone()).unwrap_or(ConnectionState::Idle)
    }

    pub fn set_state(&self, state: ConnectionState) {
        if let Ok(mut g) = self.state.lock() { *g = state; }
    }

    /// Push an additional frame onto the bar script. Useful when tests want to
    /// interleave subscribe + later injection.
    pub fn push_bar_frame(&self, frame: MockFrame) {
        if let Ok(mut q) = self.bar_script.lock() { q.push_back(frame); }
    }
    pub fn push_quote_frame(&self, frame: MockFrame) {
        if let Ok(mut q) = self.quote_script.lock() { q.push_back(frame); }
    }
}

impl Connection for MockMarketDataProvider {
    fn name(&self) -> &str { &self.name }
    fn state(&self) -> ConnectionState {
        self.state.lock().map(|g| g.clone()).unwrap_or(ConnectionState::Idle)
    }
    fn metrics(&self) -> ConnectionMetrics {
        self.metrics.lock().map(|g| g.clone()).unwrap_or_default()
    }
}

fn spawn_stream<T: Send + 'static, F>(
    script: Arc<Mutex<VecDeque<MockFrame>>>,
    metrics: Arc<Mutex<ConnectionMetrics>>,
    state: Arc<Mutex<ConnectionState>>,
    tx: mpsc::UnboundedSender<T>,
    extract: F,
)
where
    F: Fn(MockFrame) -> Option<T> + Send + 'static,
{
    tokio::spawn(async move {
        loop {
            let next = match script.lock() {
                Ok(mut q) => q.pop_front(),
                Err(_) => break,
            };
            let frame = match next {
                Some(f) => f,
                None => {
                    // Idle wait — tests can push more frames via push_*_frame.
                    tokio::time::sleep(Duration::from_millis(10)).await;
                    if tx.is_closed() { break; }
                    continue;
                }
            };
            match frame {
                MockFrame::Sleep(d) => tokio::time::sleep(d).await,
                MockFrame::Disconnect => {
                    if let Ok(mut s) = state.lock() {
                        *s = ConnectionState::Backoff {
                            until: std::time::Instant::now() + Duration::from_millis(50),
                            attempt: 1,
                            reason: "mock disconnect".into(),
                        };
                    }
                }
                MockFrame::Reconnect => {
                    if let Ok(mut s) = state.lock() {
                        *s = ConnectionState::Subscribed { count: 1 };
                    }
                    if let Ok(mut m) = metrics.lock() { m.reconnect_count += 1; }
                }
                MockFrame::Error(_) => {
                    if let Ok(mut m) = metrics.lock() { m.parse_errors += 1; }
                }
                other => {
                    if let Some(t) = extract(other) {
                        if tx.send(t).is_err() { break; }
                        if let Ok(mut m) = metrics.lock() {
                            m.messages_in += 1;
                            m.last_message_at = Some(std::time::Instant::now());
                        }
                    }
                }
            }
            if tx.is_closed() { break; }
        }
    });
}

#[async_trait::async_trait]
impl MarketDataProvider for MockMarketDataProvider {
    async fn bars(
        &self,
        _symbol: &str,
        _timeframe: &str,
        _start_ms: i64,
        _end_ms: i64,
        _limit: Option<usize>,
    ) -> Result<Vec<BarWire>, ApiError> {
        if let Some(err) = self.historical_error.lock().ok().and_then(|g| g.clone()) {
            return Err(ApiError::Network(err));
        }
        Ok(self.historical_bars.lock().map(|g| g.clone()).unwrap_or_default())
    }

    fn subscribe_bars(&self, _symbol: &str, _timeframe: &str) -> Result<BarStream, ApiError> {
        let (tx, rx) = mpsc::unbounded_channel::<BarWire>();
        self.set_state(ConnectionState::Subscribed { count: 1 });
        spawn_stream(
            self.bar_script.clone(),
            self.metrics.clone(),
            self.state.clone(),
            tx,
            |f| match f { MockFrame::Bar(b) => Some(b), _ => None },
        );
        Ok(rx)
    }
    fn unsubscribe_bars(&self, _: &str, _: &str) {}

    fn subscribe_quotes(&self, _symbol: &str) -> Result<QuoteStream, ApiError> {
        let (tx, rx) = mpsc::unbounded_channel::<Quote>();
        self.set_state(ConnectionState::Subscribed { count: 1 });
        spawn_stream(
            self.quote_script.clone(),
            self.metrics.clone(),
            self.state.clone(),
            tx,
            |f| match f { MockFrame::Quote(q) => Some(q), _ => None },
        );
        Ok(rx)
    }
    fn unsubscribe_quotes(&self, _: &str) {}

    fn subscribe_trades(&self, _symbol: &str) -> Result<TradeStream, ApiError> {
        let (tx, rx) = mpsc::unbounded_channel::<Trade>();
        self.set_state(ConnectionState::Subscribed { count: 1 });
        spawn_stream(
            self.trade_script.clone(),
            self.metrics.clone(),
            self.state.clone(),
            tx,
            |f| match f { MockFrame::Trade(t) => Some(t), _ => None },
        );
        Ok(rx)
    }
    fn unsubscribe_trades(&self, _: &str) {}

    async fn chain_snapshot(&self, _u: &str) -> Result<ChainSnapshot, ApiError> {
        Err(ApiError::NotSupported("mock: chain".into()))
    }
    fn subscribe_chain(&self, _: &str) -> Result<ChainStream, ApiError> {
        let (_tx, rx) = mpsc::unbounded_channel::<ChainDelta>();
        Ok(rx)
    }
    fn unsubscribe_chain(&self, _: &str) {}

    fn capabilities(&self) -> ProviderCapabilities { self.capabilities }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::feeds::apex_data::types::AssetClass;

    fn bw(t: i64, c: f64) -> BarWire {
        BarWire {
            symbol: "X".into(), asset_class: AssetClass::Stock, timeframe: "1m".into(),
            time: t, open: c, high: c, low: c, close: c, volume: 0.0,
            vwap: 0.0, trades: 0, closed: true,
        }
    }

    #[tokio::test]
    async fn scripted_bars_flow_through() {
        let p = MockMarketDataProvider::new("m")
            .script_bars(vec![MockFrame::Bar(bw(1, 1.0)), MockFrame::Bar(bw(2, 2.0))]);
        let mut rx = p.subscribe_bars("X", "1m").unwrap();
        let b1 = tokio::time::timeout(Duration::from_millis(200), rx.recv()).await.unwrap().unwrap();
        let b2 = tokio::time::timeout(Duration::from_millis(200), rx.recv()).await.unwrap().unwrap();
        assert_eq!(b1.time, 1);
        assert_eq!(b2.time, 2);
        let m = p.metrics();
        assert!(m.messages_in >= 2);
    }

    #[tokio::test]
    async fn historical_bars_returns_canned_data() {
        let p = MockMarketDataProvider::new("m")
            .historical_bars(vec![bw(10, 10.0), bw(20, 20.0)]);
        let bars = p.bars("X", "1m", 0, 0, None).await.unwrap();
        assert_eq!(bars.len(), 2);
    }

    #[tokio::test]
    async fn historical_error_propagates() {
        let p = MockMarketDataProvider::new("m").historical_error("boom");
        let err = p.bars("X", "1m", 0, 0, None).await.unwrap_err();
        assert!(format!("{}", err).contains("boom"));
    }

    #[tokio::test]
    async fn disconnect_reconnect_toggles_state() {
        let p = MockMarketDataProvider::new("m").script_bars(vec![
            MockFrame::Bar(bw(1, 1.0)),
            MockFrame::Disconnect,
            MockFrame::Sleep(Duration::from_millis(20)),
            MockFrame::Reconnect,
            MockFrame::Bar(bw(2, 2.0)),
        ]);
        let mut rx = p.subscribe_bars("X", "1m").unwrap();
        let _ = tokio::time::timeout(Duration::from_millis(200), rx.recv()).await.unwrap().unwrap();
        // Drain script - eventually we should hit Reconnect and emit bar 2.
        let b2 = tokio::time::timeout(Duration::from_millis(500), rx.recv()).await.unwrap().unwrap();
        assert_eq!(b2.time, 2);
        assert!(matches!(p.state(), ConnectionState::Subscribed { .. }));
        assert!(p.metrics().reconnect_count >= 1);
    }
}
