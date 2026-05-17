//! Refcounted, deduplicated subscription multiplexer.
//!
//! Sits in front of a `MarketDataProvider` and folds N callers asking for the
//! same (symbol, stream-kind) into a single underlying subscription. The
//! first caller triggers `provider.subscribe_*`, the last drop triggers
//! `provider.unsubscribe_*`. Each caller gets a fresh `broadcast::Receiver`.
//!
//! - **Dedup / refcount**: AAPL bars asked for by 3 panes → one upstream sub.
//! - **TTL**: subs with no upstream activity for >30s log a warning so we know
//!   when a feed silently went stale.
//! - **Gap fill**: on upstream reconnect, `gap_fill_on_reconnect` replays
//!   bars from the last seen timestamp via `provider.bars(...)` and forwards
//!   them through the existing fanout.

use super::provider::MarketDataProvider;
use crate::data::connectivity::ApiError;
use crate::data::feeds::apex_data::types::{BarWire, ChainDelta, Quote, Trade};
use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tokio::sync::broadcast;

const FANOUT_CAP: usize = 1024;
const STALE_TTL_SECS: u64 = 30;

struct BarSub {
    refcount: AtomicUsize,
    last_seen_ts: AtomicI64,
    last_activity: Mutex<Instant>,
    fanout: broadcast::Sender<BarWire>,
}

struct QuoteSub {
    refcount: AtomicUsize,
    last_activity: Mutex<Instant>,
    fanout: broadcast::Sender<Quote>,
}

struct TradeSub {
    refcount: AtomicUsize,
    last_activity: Mutex<Instant>,
    fanout: broadcast::Sender<Trade>,
}

struct ChainSub {
    refcount: AtomicUsize,
    last_activity: Mutex<Instant>,
    fanout: broadcast::Sender<ChainDelta>,
}

pub struct SubscriptionManager {
    provider: Arc<dyn MarketDataProvider>,
    bars: Mutex<HashMap<(String, String), Arc<BarSub>>>,
    quotes: Mutex<HashMap<String, Arc<QuoteSub>>>,
    trades: Mutex<HashMap<String, Arc<TradeSub>>>,
    chain: Mutex<HashMap<String, Arc<ChainSub>>>,
}

impl SubscriptionManager {
    pub fn new(provider: Arc<dyn MarketDataProvider>) -> Self {
        Self {
            provider,
            bars: Mutex::new(HashMap::new()),
            quotes: Mutex::new(HashMap::new()),
            trades: Mutex::new(HashMap::new()),
            chain: Mutex::new(HashMap::new()),
        }
    }

    /// First subscriber spins up the upstream pump; subsequent subscribers
    /// just get a fresh `broadcast::Receiver` on the existing fanout.
    #[tracing::instrument(skip(self), level = "debug", fields(symbol, timeframe))]
    pub fn subscribe_bars(
        &self,
        symbol: &str,
        timeframe: &str,
    ) -> Result<broadcast::Receiver<BarWire>, ApiError> {
        let key = (symbol.to_string(), timeframe.to_string());
        let mut map = self.bars.lock().expect("bars map");
        if let Some(sub) = map.get(&key) {
            sub.refcount.fetch_add(1, Ordering::SeqCst);
            return Ok(sub.fanout.subscribe());
        }
        let mut rx = self.provider.subscribe_bars(symbol, timeframe)?;
        let (tx, rx_out) = broadcast::channel::<BarWire>(FANOUT_CAP);
        let sub = Arc::new(BarSub {
            refcount: AtomicUsize::new(1),
            last_seen_ts: AtomicI64::new(0),
            last_activity: Mutex::new(Instant::now()),
            fanout: tx.clone(),
        });
        map.insert(key.clone(), sub.clone());
        drop(map);
        let _ = rx_out; // returned below via tx.subscribe()
        let recv_out = tx.subscribe();
        tokio::spawn({
            let sub = sub.clone();
            async move {
                while let Some(bar) = rx.recv().await {
                    sub.last_seen_ts.store(bar.time, Ordering::Relaxed);
                    if let Ok(mut g) = sub.last_activity.lock() {
                        *g = Instant::now();
                    }
                    if sub.fanout.send(bar).is_err() {
                        // No live receivers; keep pumping anyway so refcount
                        // logic stays the sole authority on lifecycle.
                    }
                }
            }
        });
        Ok(recv_out)
    }

    #[tracing::instrument(skip(self), level = "debug", fields(symbol, timeframe))]
    pub fn unsubscribe_bars(&self, symbol: &str, timeframe: &str) {
        let key = (symbol.to_string(), timeframe.to_string());
        let mut map = self.bars.lock().expect("bars map");
        if let Some(sub) = map.get(&key) {
            if sub.refcount.fetch_sub(1, Ordering::SeqCst) == 1 {
                map.remove(&key);
                self.provider.unsubscribe_bars(symbol, timeframe);
            }
        }
    }

    #[tracing::instrument(skip(self), level = "debug", fields(symbol))]
    pub fn subscribe_quotes(
        &self,
        symbol: &str,
    ) -> Result<broadcast::Receiver<Quote>, ApiError> {
        let mut map = self.quotes.lock().expect("quotes map");
        if let Some(sub) = map.get(symbol) {
            sub.refcount.fetch_add(1, Ordering::SeqCst);
            return Ok(sub.fanout.subscribe());
        }
        let mut rx = self.provider.subscribe_quotes(symbol)?;
        let (tx, _) = broadcast::channel::<Quote>(FANOUT_CAP);
        let sub = Arc::new(QuoteSub {
            refcount: AtomicUsize::new(1),
            last_activity: Mutex::new(Instant::now()),
            fanout: tx.clone(),
        });
        map.insert(symbol.to_string(), sub.clone());
        drop(map);
        let recv_out = tx.subscribe();
        tokio::spawn(async move {
            while let Some(q) = rx.recv().await {
                if let Ok(mut g) = sub.last_activity.lock() {
                    *g = Instant::now();
                }
                let _ = sub.fanout.send(q);
            }
        });
        Ok(recv_out)
    }

    #[tracing::instrument(skip(self), level = "debug", fields(symbol))]
    pub fn unsubscribe_quotes(&self, symbol: &str) {
        let mut map = self.quotes.lock().expect("quotes map");
        if let Some(sub) = map.get(symbol) {
            if sub.refcount.fetch_sub(1, Ordering::SeqCst) == 1 {
                map.remove(symbol);
                self.provider.unsubscribe_quotes(symbol);
            }
        }
    }

    #[tracing::instrument(skip(self), level = "debug", fields(symbol))]
    pub fn subscribe_trades(
        &self,
        symbol: &str,
    ) -> Result<broadcast::Receiver<Trade>, ApiError> {
        let mut map = self.trades.lock().expect("trades map");
        if let Some(sub) = map.get(symbol) {
            sub.refcount.fetch_add(1, Ordering::SeqCst);
            return Ok(sub.fanout.subscribe());
        }
        let mut rx = self.provider.subscribe_trades(symbol)?;
        let (tx, _) = broadcast::channel::<Trade>(FANOUT_CAP);
        let sub = Arc::new(TradeSub {
            refcount: AtomicUsize::new(1),
            last_activity: Mutex::new(Instant::now()),
            fanout: tx.clone(),
        });
        map.insert(symbol.to_string(), sub.clone());
        drop(map);
        let recv_out = tx.subscribe();
        tokio::spawn(async move {
            while let Some(t) = rx.recv().await {
                if let Ok(mut g) = sub.last_activity.lock() {
                    *g = Instant::now();
                }
                let _ = sub.fanout.send(t);
            }
        });
        Ok(recv_out)
    }

    #[tracing::instrument(skip(self), level = "debug", fields(symbol))]
    pub fn unsubscribe_trades(&self, symbol: &str) {
        let mut map = self.trades.lock().expect("trades map");
        if let Some(sub) = map.get(symbol) {
            if sub.refcount.fetch_sub(1, Ordering::SeqCst) == 1 {
                map.remove(symbol);
                self.provider.unsubscribe_trades(symbol);
            }
        }
    }

    #[tracing::instrument(skip(self), level = "debug", fields(underlying))]
    pub fn subscribe_chain(
        &self,
        underlying: &str,
    ) -> Result<broadcast::Receiver<ChainDelta>, ApiError> {
        let mut map = self.chain.lock().expect("chain map");
        if let Some(sub) = map.get(underlying) {
            sub.refcount.fetch_add(1, Ordering::SeqCst);
            return Ok(sub.fanout.subscribe());
        }
        let mut rx = self.provider.subscribe_chain(underlying)?;
        let (tx, _) = broadcast::channel::<ChainDelta>(FANOUT_CAP);
        let sub = Arc::new(ChainSub {
            refcount: AtomicUsize::new(1),
            last_activity: Mutex::new(Instant::now()),
            fanout: tx.clone(),
        });
        map.insert(underlying.to_string(), sub.clone());
        drop(map);
        let recv_out = tx.subscribe();
        tokio::spawn(async move {
            while let Some(d) = rx.recv().await {
                if let Ok(mut g) = sub.last_activity.lock() {
                    *g = Instant::now();
                }
                let _ = sub.fanout.send(d);
            }
        });
        Ok(recv_out)
    }

    #[tracing::instrument(skip(self), level = "debug", fields(underlying))]
    pub fn unsubscribe_chain(&self, underlying: &str) {
        let mut map = self.chain.lock().expect("chain map");
        if let Some(sub) = map.get(underlying) {
            if sub.refcount.fetch_sub(1, Ordering::SeqCst) == 1 {
                map.remove(underlying);
                self.provider.unsubscribe_chain(underlying);
            }
        }
    }

    /// Replay bars from the last-seen timestamp through the existing fanout.
    /// Call when the upstream provider signals a reconnect.
    #[tracing::instrument(skip(self), level = "debug", fields(symbol, timeframe))]
    pub async fn gap_fill_on_reconnect(
        &self,
        symbol: &str,
        timeframe: &str,
    ) -> Result<usize, ApiError> {
        let key = (symbol.to_string(), timeframe.to_string());
        let (last_ts, fanout) = {
            let map = self.bars.lock().expect("bars map");
            match map.get(&key) {
                Some(sub) => (
                    sub.last_seen_ts.load(Ordering::Relaxed),
                    sub.fanout.clone(),
                ),
                None => return Ok(0),
            }
        };
        if last_ts == 0 {
            return Ok(0);
        }
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        let bars = self
            .provider
            .bars(symbol, timeframe, last_ts, now_ms, None)
            .await?;
        let n = bars.len();
        for b in bars {
            let _ = fanout.send(b);
        }
        Ok(n)
    }

    /// Walk every cached subscription; warn (via `tracing`) when activity is
    /// older than `STALE_TTL_SECS`. Call from a periodic supervisor.
    pub fn check_stale(&self) {
        let now = Instant::now();
        let check = |kind: &str, key: &str, activity: &Mutex<Instant>| {
            if let Ok(g) = activity.lock() {
                let age = now.saturating_duration_since(*g);
                if age.as_secs() > STALE_TTL_SECS {
                    tracing::warn!(
                        target: "providers.sub_mgr",
                        kind,
                        key,
                        age_secs = age.as_secs(),
                        "subscription stale — no upstream activity"
                    );
                }
            }
        };
        if let Ok(map) = self.bars.lock() {
            for ((s, tf), sub) in map.iter() {
                check("bars", &format!("{s}:{tf}"), &sub.last_activity);
            }
        }
        if let Ok(map) = self.quotes.lock() {
            for (s, sub) in map.iter() {
                check("quotes", s, &sub.last_activity);
            }
        }
        if let Ok(map) = self.trades.lock() {
            for (s, sub) in map.iter() {
                check("trades", s, &sub.last_activity);
            }
        }
        if let Ok(map) = self.chain.lock() {
            for (s, sub) in map.iter() {
                check("chain", s, &sub.last_activity);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::connectivity::{Connection, ConnectionMetrics, ConnectionState};
    use crate::data::feeds::apex_data::types::AssetClass;
    use crate::data::providers::provider::{
        BarStream, ChainStream, MarketDataProvider, ProviderCapabilities, QuoteStream,
        TradeStream,
    };
    use std::sync::atomic::AtomicU32;

    struct MockProvider {
        subs: Mutex<HashMap<String, AtomicU32>>,
    }

    impl MockProvider {
        fn new() -> Self {
            Self {
                subs: Mutex::new(HashMap::new()),
            }
        }
        fn live_subs(&self, key: &str) -> u32 {
            self.subs
                .lock()
                .unwrap()
                .get(key)
                .map(|a| a.load(Ordering::SeqCst))
                .unwrap_or(0)
        }
    }

    impl Connection for MockProvider {
        fn name(&self) -> &str { "mock" }
        fn state(&self) -> ConnectionState { ConnectionState::Idle }
        fn metrics(&self) -> ConnectionMetrics { ConnectionMetrics::default() }
    }

    #[async_trait::async_trait]
    impl MarketDataProvider for MockProvider {
        async fn bars(&self, _s: &str, _tf: &str, _a: i64, _b: i64, _l: Option<usize>) -> Result<Vec<BarWire>, ApiError> { Ok(vec![]) }
        fn subscribe_bars(&self, s: &str, tf: &str) -> Result<BarStream, ApiError> {
            let key = format!("{s}:{tf}");
            self.subs.lock().unwrap().entry(key).or_insert_with(|| AtomicU32::new(0)).fetch_add(1, Ordering::SeqCst);
            let (_tx, rx) = tokio::sync::mpsc::unbounded_channel();
            Ok(rx)
        }
        fn unsubscribe_bars(&self, s: &str, tf: &str) {
            let key = format!("{s}:{tf}");
            if let Some(a) = self.subs.lock().unwrap().get(&key) { a.fetch_sub(1, Ordering::SeqCst); }
        }
        fn subscribe_quotes(&self, s: &str) -> Result<QuoteStream, ApiError> {
            self.subs.lock().unwrap().entry(s.to_string()).or_insert_with(|| AtomicU32::new(0)).fetch_add(1, Ordering::SeqCst);
            let (_tx, rx) = tokio::sync::mpsc::unbounded_channel(); Ok(rx)
        }
        fn unsubscribe_quotes(&self, s: &str) {
            if let Some(a) = self.subs.lock().unwrap().get(s) { a.fetch_sub(1, Ordering::SeqCst); }
        }
        fn subscribe_trades(&self, _s: &str) -> Result<TradeStream, ApiError> {
            let (_tx, rx) = tokio::sync::mpsc::unbounded_channel(); Ok(rx)
        }
        fn unsubscribe_trades(&self, _s: &str) {}
        async fn chain_snapshot(&self, _u: &str) -> Result<super::super::provider::ChainSnapshot, ApiError> {
            Ok(super::super::provider::ChainSnapshot { underlying: String::new(), contracts: 0, total_in_cache: 0, filters: Default::default(), rows: vec![] })
        }
        fn subscribe_chain(&self, _u: &str) -> Result<ChainStream, ApiError> { Err(ApiError::NotSupported("chain".into())) }
        fn unsubscribe_chain(&self, _u: &str) {}
        fn capabilities(&self) -> ProviderCapabilities { ProviderCapabilities { bars: true, quotes: true, trades: true, ..Default::default() } }
    }

    // Suppress unused warning when not used.
    #[allow(dead_code)]
    fn _wire_asset_class() -> AssetClass { AssetClass::Stock }

    #[tokio::test]
    async fn refcount_dedups_subscriptions() {
        let prov = Arc::new(MockProvider::new());
        let mgr = SubscriptionManager::new(prov.clone());
        let _r1 = mgr.subscribe_bars("AAPL", "5m").unwrap();
        let _r2 = mgr.subscribe_bars("AAPL", "5m").unwrap();
        let _r3 = mgr.subscribe_bars("AAPL", "5m").unwrap();
        assert_eq!(prov.live_subs("AAPL:5m"), 1, "underlying sub should be 1");
        mgr.unsubscribe_bars("AAPL", "5m");
        mgr.unsubscribe_bars("AAPL", "5m");
        assert_eq!(prov.live_subs("AAPL:5m"), 1, "still active");
        mgr.unsubscribe_bars("AAPL", "5m");
        assert_eq!(prov.live_subs("AAPL:5m"), 0, "evicted at zero");
    }

    #[tokio::test]
    async fn quotes_refcount_independent() {
        let prov = Arc::new(MockProvider::new());
        let mgr = SubscriptionManager::new(prov.clone());
        let _a = mgr.subscribe_quotes("TSLA").unwrap();
        let _b = mgr.subscribe_quotes("TSLA").unwrap();
        assert_eq!(prov.live_subs("TSLA"), 1);
        mgr.unsubscribe_quotes("TSLA");
        assert_eq!(prov.live_subs("TSLA"), 1);
        mgr.unsubscribe_quotes("TSLA");
        assert_eq!(prov.live_subs("TSLA"), 0);
    }
}
