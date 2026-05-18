//! Adapter: `MarketDataProvider` over the existing `feeds::apex_data` module.
//!
//! Thin — it forwards subscribe/unsubscribe to `apex_data::ws::*` and
//! historical bar fetches to `apex_data::rest::*`. The underlying WS code is
//! unchanged; this adapter just gives it the new trait surface so callers
//! (subscription manager, fallback chain) can talk to it uniformly.
//!
//! Per-stream demultiplexing: ApexData WS has a single global frame listener
//! that fans out every frame. We register one listener per `ApexDataProvider`
//! instance, then route inbound bars/quotes/trades/chain-deltas to whichever
//! mpsc senders are currently registered for that key.

use super::provider::{
    BarStream, ChainSnapshot, ChainStream, MarketDataProvider, ProviderCapabilities,
    QuoteStream, TradeStream,
};
use crate::data::connectivity::{
    ApiError, Connection, ConnectionMetrics, ConnectionState,
};
use crate::data::feeds::apex_data::{self, ws, rest};
use crate::data::feeds::apex_data::types::{AssetClass, BarWire, ChainDelta, Quote, Trade};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use tokio::sync::mpsc;

type BarSender = mpsc::UnboundedSender<BarWire>;
type QuoteSender = mpsc::UnboundedSender<Quote>;
type TradeSender = mpsc::UnboundedSender<Trade>;
type ChainSender = mpsc::UnboundedSender<ChainDelta>;

struct Routes {
    bars: HashMap<(String, String), Vec<BarSender>>,
    quotes: HashMap<String, Vec<QuoteSender>>,
    trades: HashMap<String, Vec<TradeSender>>,
    chain: HashMap<String, Vec<ChainSender>>,
}

impl Routes {
    fn new() -> Self {
        Self {
            bars: HashMap::new(),
            quotes: HashMap::new(),
            trades: HashMap::new(),
            chain: HashMap::new(),
        }
    }
}

static ROUTES: OnceLock<Mutex<Routes>> = OnceLock::new();

fn routes() -> &'static Mutex<Routes> {
    ROUTES.get_or_init(|| Mutex::new(Routes::new()))
}

fn ensure_listener() {
    static REGISTERED: OnceLock<()> = OnceLock::new();
    REGISTERED.get_or_init(|| {
        ws::subscribe_to_frames(|frame| {
            let mut r = match routes().lock() { Ok(g) => g, Err(_) => return };
            match frame {
                ws::Frame::Bar(b) => {
                    let key = (b.bar.symbol.clone(), b.bar.timeframe.clone());
                    if let Some(senders) = r.bars.get_mut(&key) {
                        senders.retain(|s| s.send(b.bar.clone()).is_ok());
                    }
                }
                ws::Frame::Snapshot { bar, .. } => {
                    let key = (bar.bar.symbol.clone(), bar.bar.timeframe.clone());
                    if let Some(senders) = r.bars.get_mut(&key) {
                        senders.retain(|s| s.send(bar.bar.clone()).is_ok());
                    }
                }
                ws::Frame::Quote(q) => {
                    if let Some(senders) = r.quotes.get_mut(&q.symbol) {
                        senders.retain(|s| s.send(q.clone()).is_ok());
                    }
                }
                ws::Frame::Trade(t) => {
                    if let Some(senders) = r.trades.get_mut(&t.symbol) {
                        senders.retain(|s| s.send(t.clone()).is_ok());
                    }
                }
                ws::Frame::ChainDelta(d) => {
                    if let Some(senders) = r.chain.get_mut(&d.underlying) {
                        senders.retain(|s| s.send(d.clone()).is_ok());
                    }
                }
                _ => {}
            }
        });
    });
}

pub struct ApexDataProvider;

impl ApexDataProvider {
    pub fn new() -> Self {
        ensure_listener();
        Self
    }
}

impl Default for ApexDataProvider {
    fn default() -> Self { Self::new() }
}

impl Connection for ApexDataProvider {
    fn name(&self) -> &str { "apex_data" }

    fn state(&self) -> ConnectionState {
        // Best-effort mapping: real Backoff / Failed states ship in a later
        // cleanup. For now, mirror the existing connected flag honestly.
        if ws::is_connected() {
            let count = {
                let r = match routes().lock() { Ok(g) => g, Err(_) => return ConnectionState::Authenticated };
                r.bars.len() + r.quotes.len() + r.trades.len() + r.chain.len()
            };
            if count == 0 {
                ConnectionState::Authenticated
            } else {
                ConnectionState::Subscribed { count }
            }
        } else {
            ConnectionState::Idle
        }
    }

    fn metrics(&self) -> ConnectionMetrics {
        use std::sync::atomic::Ordering;
        let last_ms = ws::LAST_MESSAGE_AT_MS.load(Ordering::Relaxed);
        let last_message_at = if last_ms > 0 {
            // Best-effort: derive an `Instant` from the recorded unix-ms by
            // subtracting "ms since the last message" from `Instant::now()`.
            let now_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as i64)
                .unwrap_or(last_ms);
            let age = (now_ms - last_ms).max(0) as u64;
            std::time::Instant::now().checked_sub(std::time::Duration::from_millis(age))
        } else {
            None
        };
        ConnectionMetrics {
            messages_in:    ws::MESSAGES_IN.load(Ordering::Relaxed),
            messages_out:   0,
            parse_errors:   ws::PARSE_ERRORS.load(Ordering::Relaxed),
            reconnect_count: ws::RECONNECT_COUNT.load(Ordering::Relaxed),
            last_message_at,
            queue_depth:    None,
        }
    }
}

#[async_trait::async_trait]
impl MarketDataProvider for ApexDataProvider {
    #[tracing::instrument(skip(self), level = "debug", fields(symbol, timeframe))]
    async fn bars(
        &self,
        symbol: &str,
        timeframe: &str,
        _start_ms: i64,
        _end_ms: i64,
        _limit: Option<usize>,
    ) -> Result<Vec<BarWire>, ApiError> {
        if !apex_data::is_enabled() {
            return Err(ApiError::NotSupported("apex_data: disabled".into()));
        }
        let sym = symbol.to_string();
        let tf = timeframe.to_string();
        let bars = tokio::task::spawn_blocking(move || {
            let class = AssetClass::from_symbol(&sym);
            rest::get_bars(class, &sym, &tf, crate::apex_data::BarSource::Last)
                .map(|chart_bars| {
                    let cls = class;
                    chart_bars
                        .into_iter()
                        .map(|b| BarWire {
                            symbol: sym.clone(),
                            asset_class: cls,
                            timeframe: tf.clone(),
                            time: b.time * 1000,
                            open: b.open, high: b.high, low: b.low, close: b.close, volume: b.volume,
                            vwap: 0.0, trades: 0, closed: true,
                        })
                        .collect::<Vec<_>>()
                })
        })
        .await
        .map_err(|e| ApiError::Network(format!("join: {e}")))?
        .unwrap_or_default();
        if bars.is_empty() {
            Err(ApiError::Network("apex_data: empty/none".into()))
        } else {
            Ok(bars)
        }
    }

    #[tracing::instrument(skip(self), level = "debug", fields(symbol, timeframe))]
    fn subscribe_bars(&self, symbol: &str, timeframe: &str) -> Result<BarStream, ApiError> {
        let (tx, rx) = mpsc::unbounded_channel::<BarWire>();
        {
            let mut r = routes().lock().map_err(|_| ApiError::Network("route lock".into()))?;
            r.bars.entry((symbol.to_string(), timeframe.to_string()))
                .or_insert_with(Vec::new)
                .push(tx);
        }
        ws::add_bar_sub(symbol, timeframe);
        Ok(rx)
    }
    fn unsubscribe_bars(&self, symbol: &str, timeframe: &str) {
        if let Ok(mut r) = routes().lock() {
            r.bars.remove(&(symbol.to_string(), timeframe.to_string()));
        }
        ws::remove_bar_sub(symbol, timeframe);
    }

    #[tracing::instrument(skip(self), level = "debug", fields(symbol))]
    fn subscribe_quotes(&self, symbol: &str) -> Result<QuoteStream, ApiError> {
        let (tx, rx) = mpsc::unbounded_channel::<Quote>();
        {
            let mut r = routes().lock().map_err(|_| ApiError::Network("route lock".into()))?;
            r.quotes.entry(symbol.to_string()).or_insert_with(Vec::new).push(tx);
            // Re-push the active set since ApexData WS uses replace-set semantics.
            let syms: Vec<String> = r.quotes.keys().cloned().collect();
            ws::set_quotes(&syms);
        }
        Ok(rx)
    }
    fn unsubscribe_quotes(&self, symbol: &str) {
        if let Ok(mut r) = routes().lock() {
            r.quotes.remove(symbol);
            let syms: Vec<String> = r.quotes.keys().cloned().collect();
            ws::set_quotes(&syms);
        }
    }

    #[tracing::instrument(skip(self), level = "debug", fields(symbol))]
    fn subscribe_trades(&self, symbol: &str) -> Result<TradeStream, ApiError> {
        let (tx, rx) = mpsc::unbounded_channel::<Trade>();
        {
            let mut r = routes().lock().map_err(|_| ApiError::Network("route lock".into()))?;
            r.trades.entry(symbol.to_string()).or_insert_with(Vec::new).push(tx);
            let syms: Vec<String> = r.trades.keys().cloned().collect();
            ws::set_tape(&syms);
        }
        Ok(rx)
    }
    fn unsubscribe_trades(&self, symbol: &str) {
        if let Ok(mut r) = routes().lock() {
            r.trades.remove(symbol);
            let syms: Vec<String> = r.trades.keys().cloned().collect();
            ws::set_tape(&syms);
        }
    }

    #[tracing::instrument(skip(self), level = "debug", fields(underlying))]
    async fn chain_snapshot(&self, underlying: &str) -> Result<ChainSnapshot, ApiError> {
        if !apex_data::is_enabled() {
            return Err(ApiError::NotSupported("apex_data: disabled".into()));
        }
        // Routed through `with_auth_retry` so a 401 from ApexData triggers
        // a single refresh + retry before bubbling up. (Refresh is a no-op
        // today — ApexData has no `/auth/refresh` endpoint — but the wiring
        // is in place so a server-side flow lands transparently.)
        rest::get_chain_async(underlying).await
    }

    fn subscribe_chain(&self, underlying: &str) -> Result<ChainStream, ApiError> {
        let (tx, rx) = mpsc::unbounded_channel::<ChainDelta>();
        {
            let mut r = routes().lock().map_err(|_| ApiError::Network("route lock".into()))?;
            r.chain.entry(underlying.to_string()).or_insert_with(Vec::new).push(tx);
            let uls: Vec<String> = r.chain.keys().cloned().collect();
            ws::set_chain(&uls);
        }
        Ok(rx)
    }
    fn unsubscribe_chain(&self, underlying: &str) {
        if let Ok(mut r) = routes().lock() {
            r.chain.remove(underlying);
            let uls: Vec<String> = r.chain.keys().cloned().collect();
            ws::set_chain(&uls);
        }
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            bars: true, quotes: true, trades: true, chain: true,
            crypto_only: false, historical: true, realtime: true,
        }
    }
}
