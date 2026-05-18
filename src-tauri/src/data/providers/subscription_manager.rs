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

/// Which series a bar subscription targets. `Last` is the default
/// trade-print stream; `Mark` is the parallel NBBO-mid stream
/// (MARK_BARS_PROTOCOL). Subscriptions for the same `(symbol, timeframe)`
/// with different `BarSource` values are independent — they refcount
/// separately and each maps to a distinct underlying feed call.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub enum BarSource { Last, Mark }

impl BarSource {
    pub fn as_str(self) -> &'static str {
        match self { BarSource::Last => "last", BarSource::Mark => "mark" }
    }
    pub fn from_str(s: &str) -> Self {
        match s { "mark" => BarSource::Mark, _ => BarSource::Last }
    }
}

struct BarSub {
    refcount: AtomicUsize,
    last_seen_ts: AtomicI64,
    last_activity: Mutex<Instant>,
    fanout: broadcast::Sender<BarWire>,
}

struct QuoteSub {
    refcount: AtomicUsize,
    last_seen_ts: AtomicI64,
    last_activity: Mutex<Instant>,
    fanout: broadcast::Sender<Quote>,
}

struct TradeSub {
    refcount: AtomicUsize,
    last_seen_ts: AtomicI64,
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
    bars: Mutex<HashMap<(String, String, BarSource), Arc<BarSub>>>,
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
    ///
    /// Defaults to `BarSource::Last` — the trade-print stream. For mark-source
    /// (NBBO-mid) bars, use [`subscribe_bars_with_source`].
    #[tracing::instrument(skip(self), level = "debug", fields(symbol, timeframe))]
    pub fn subscribe_bars(
        &self,
        symbol: &str,
        timeframe: &str,
    ) -> Result<broadcast::Receiver<BarWire>, ApiError> {
        self.subscribe_bars_with_source(symbol, timeframe, BarSource::Last)
    }

    #[tracing::instrument(skip(self), level = "debug", fields(symbol, timeframe))]
    pub fn unsubscribe_bars(&self, symbol: &str, timeframe: &str) {
        self.unsubscribe_bars_with_source(symbol, timeframe, BarSource::Last)
    }

    /// Source-aware subscription. `(symbol, timeframe, source)` keys are
    /// independent — Last and Mark subs for the same symbol/timeframe pair
    /// each refcount + bump separately.
    ///
    /// For `BarSource::Last` this routes through the underlying
    /// `MarketDataProvider` trait (`provider.subscribe_bars`). For
    /// `BarSource::Mark` there is no trait surface yet, so we only track
    /// refcount + fanout state — callers remain responsible for the actual
    /// upstream feed call (e.g. `ws::add_mark_bar_sub`). The returned
    /// receiver is still valid (callers may ignore it); `last_seen_ts` is
    /// kept current via [`bump_last_seen_bar`].
    #[tracing::instrument(skip(self), level = "debug", fields(symbol, timeframe, source = ?source))]
    pub fn subscribe_bars_with_source(
        &self,
        symbol: &str,
        timeframe: &str,
        source: BarSource,
    ) -> Result<broadcast::Receiver<BarWire>, ApiError> {
        let key = (symbol.to_string(), timeframe.to_string(), source);
        let mut map = self.bars.lock().expect("bars map");
        if let Some(sub) = map.get(&key) {
            sub.refcount.fetch_add(1, Ordering::SeqCst);
            return Ok(sub.fanout.subscribe());
        }
        // Only Last has a trait-side pump. Mark gets a state-only entry; its
        // bumper is driven externally from the feed adapter.
        let upstream = if source == BarSource::Last {
            Some(self.provider.subscribe_bars(symbol, timeframe)?)
        } else {
            None
        };
        let (tx, _rx_out) = broadcast::channel::<BarWire>(FANOUT_CAP);
        let sub = Arc::new(BarSub {
            refcount: AtomicUsize::new(1),
            last_seen_ts: AtomicI64::new(0),
            last_activity: Mutex::new(Instant::now()),
            fanout: tx.clone(),
        });
        map.insert(key.clone(), sub.clone());
        drop(map);
        let recv_out = tx.subscribe();
        if let Some(mut rx) = upstream {
            tokio::spawn({
                let sub = sub.clone();
                async move {
                    while let Some(bar) = rx.recv().await {
                        if bar.time > 0 {
                            let prev = sub.last_seen_ts.load(Ordering::Relaxed);
                            if bar.time > prev {
                                sub.last_seen_ts.store(bar.time, Ordering::Relaxed);
                            }
                        }
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
        }
        Ok(recv_out)
    }

    #[tracing::instrument(skip(self), level = "debug", fields(symbol, timeframe, source = ?source))]
    pub fn unsubscribe_bars_with_source(&self, symbol: &str, timeframe: &str, source: BarSource) {
        let key = (symbol.to_string(), timeframe.to_string(), source);
        let mut map = self.bars.lock().expect("bars map");
        if let Some(sub) = map.get(&key) {
            if sub.refcount.fetch_sub(1, Ordering::SeqCst) == 1 {
                map.remove(&key);
                if source == BarSource::Last {
                    self.provider.unsubscribe_bars(symbol, timeframe);
                }
                // Mark-source upstream unsub stays the caller's responsibility.
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
            last_seen_ts: AtomicI64::new(0),
            last_activity: Mutex::new(Instant::now()),
            fanout: tx.clone(),
        });
        map.insert(symbol.to_string(), sub.clone());
        drop(map);
        let recv_out = tx.subscribe();
        tokio::spawn(async move {
            while let Some(q) = rx.recv().await {
                if q.time > 0 {
                    let prev = sub.last_seen_ts.load(Ordering::Relaxed);
                    if q.time > prev {
                        sub.last_seen_ts.store(q.time, Ordering::Relaxed);
                    }
                }
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
            last_seen_ts: AtomicI64::new(0),
            last_activity: Mutex::new(Instant::now()),
            fanout: tx.clone(),
        });
        map.insert(symbol.to_string(), sub.clone());
        drop(map);
        let recv_out = tx.subscribe();
        tokio::spawn(async move {
            while let Some(t) = rx.recv().await {
                if t.time > 0 {
                    let prev = sub.last_seen_ts.load(Ordering::Relaxed);
                    if t.time > prev {
                        sub.last_seen_ts.store(t.time, Ordering::Relaxed);
                    }
                }
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

    // ────────────────────────────────────────────────────────────────────────
    // Wave 8 — request-side wrappers and external `last_seen_ts` bumpers.
    //
    // The data-delivery path remains unchanged (feeds fan frames into
    // `NATIVE_CHART_TXS`). These helpers exist so callers can route their
    // subscription REQUESTS through SubscriptionManager — that gives
    // `gap_fill_on_reconnect_all()` a populated set to walk. Callers may
    // ignore the returned receiver; the broadcast Sender is held internally
    // and the pump still bumps `last_seen_ts` from the trait-side stream.
    // ────────────────────────────────────────────────────────────────────────

    /// Bump the bar `last_seen_ts` from outside the trait pump.
    ///
    /// Useful when the actual frame stream reaches the UI via a path that
    /// does NOT flow through `SubscriptionManager.subscribe_bars` (e.g. the
    /// legacy `NATIVE_CHART_TXS` route). Monotonic: only the max wins.
    pub fn bump_last_seen_bar(&self, symbol: &str, timeframe: &str, source: BarSource, ts_ms: i64) {
        if ts_ms <= 0 { return; }
        let key = (symbol.to_string(), timeframe.to_string(), source);
        if let Ok(map) = self.bars.lock() {
            if let Some(sub) = map.get(&key) {
                let prev = sub.last_seen_ts.load(Ordering::Relaxed);
                if ts_ms > prev {
                    sub.last_seen_ts.store(ts_ms, Ordering::Relaxed);
                }
                if let Ok(mut g) = sub.last_activity.lock() {
                    *g = Instant::now();
                }
            }
        }
    }

    pub fn bump_last_seen_quote(&self, symbol: &str, ts_ms: i64) {
        if ts_ms <= 0 { return; }
        if let Ok(map) = self.quotes.lock() {
            if let Some(sub) = map.get(symbol) {
                let prev = sub.last_seen_ts.load(Ordering::Relaxed);
                if ts_ms > prev {
                    sub.last_seen_ts.store(ts_ms, Ordering::Relaxed);
                }
                if let Ok(mut g) = sub.last_activity.lock() {
                    *g = Instant::now();
                }
            }
        }
    }

    pub fn bump_last_seen_trade(&self, symbol: &str, ts_ms: i64) {
        if ts_ms <= 0 { return; }
        if let Ok(map) = self.trades.lock() {
            if let Some(sub) = map.get(symbol) {
                let prev = sub.last_seen_ts.load(Ordering::Relaxed);
                if ts_ms > prev {
                    sub.last_seen_ts.store(ts_ms, Ordering::Relaxed);
                }
                if let Ok(mut g) = sub.last_activity.lock() {
                    *g = Instant::now();
                }
            }
        }
    }

    /// Read-only inspection of current bar `last_seen_ts` (for tests / debug).
    /// Defaults to `BarSource::Last`.
    pub fn last_seen_bar(&self, symbol: &str, timeframe: &str) -> i64 {
        self.last_seen_bar_with_source(symbol, timeframe, BarSource::Last)
    }

    pub fn last_seen_bar_with_source(&self, symbol: &str, timeframe: &str, source: BarSource) -> i64 {
        let key = (symbol.to_string(), timeframe.to_string(), source);
        self.bars.lock().ok()
            .and_then(|m| m.get(&key).map(|s| s.last_seen_ts.load(Ordering::Relaxed)))
            .unwrap_or(0)
    }

    pub fn last_seen_quote(&self, symbol: &str) -> i64 {
        self.quotes.lock().ok()
            .and_then(|m| m.get(symbol).map(|s| s.last_seen_ts.load(Ordering::Relaxed)))
            .unwrap_or(0)
    }

    pub fn last_seen_trade(&self, symbol: &str) -> i64 {
        self.trades.lock().ok()
            .and_then(|m| m.get(symbol).map(|s| s.last_seen_ts.load(Ordering::Relaxed)))
            .unwrap_or(0)
    }

    // ────────────────────────────────────────────────────────────────────────
    // Wave 9e — test-only introspection helpers.
    //
    // The concurrency stress tests need to assert on (a) the set of currently
    // active bar subscriptions and (b) the refcount on a specific key. The
    // production API doesn't expose these; we add them gated to `cfg(test)`
    // so they don't expand the public surface.
    // ────────────────────────────────────────────────────────────────────────

    /// Snapshot of currently-tracked bar subscriptions as
    /// `(symbol, timeframe, source)` triples. Test-only.
    #[cfg(test)]
    pub fn active_bar_subs(&self) -> Vec<(String, String, BarSource)> {
        self.bars.lock().ok()
            .map(|m| m.keys().cloned().collect())
            .unwrap_or_default()
    }

    /// Current refcount for the bar key `(symbol, timeframe, Last)`.
    /// Returns 0 if no entry exists. Test-only.
    #[cfg(test)]
    pub fn refcount_for_bar(&self, symbol: &str, timeframe: &str) -> usize {
        let key = (symbol.to_string(), timeframe.to_string(), BarSource::Last);
        self.bars.lock().ok()
            .and_then(|m| m.get(&key).map(|s| s.refcount.load(Ordering::SeqCst)))
            .unwrap_or(0)
    }

    /// Replace the active quote-subscription set, computing the diff against
    /// current state and calling `subscribe_quotes` / `unsubscribe_quotes`
    /// for each delta. Mirrors the `ws::set_quotes` replace-set semantics so
    /// callers that thought in those terms can migrate one-for-one.
    ///
    /// Receivers from `subscribe_quotes` are dropped; the broadcast Sender
    /// stays alive (so refcount logic remains the lifecycle authority) and
    /// future `subscribe_quotes` calls fan onto the same channel.
    pub fn set_quotes(&self, symbols: &[String]) {
        use std::collections::HashSet;
        let want: HashSet<String> = symbols.iter().cloned().collect();
        let have: HashSet<String> = self.quotes.lock().ok()
            .map(|m| m.keys().cloned().collect())
            .unwrap_or_default();
        for sym in want.difference(&have) {
            let _ = self.subscribe_quotes(sym);
        }
        for sym in have.difference(&want) {
            // Refcount-aware: only the SubscriptionManager-owned ref is
            // dropped here. Other callers holding refs still keep the sub.
            self.unsubscribe_quotes(sym);
        }
    }

    pub fn set_trades(&self, symbols: &[String]) {
        use std::collections::HashSet;
        let want: HashSet<String> = symbols.iter().cloned().collect();
        let have: HashSet<String> = self.trades.lock().ok()
            .map(|m| m.keys().cloned().collect())
            .unwrap_or_default();
        for sym in want.difference(&have) {
            let _ = self.subscribe_trades(sym);
        }
        for sym in have.difference(&want) {
            self.unsubscribe_trades(sym);
        }
    }

    pub fn set_chain(&self, underlyings: &[String]) {
        use std::collections::HashSet;
        let want: HashSet<String> = underlyings.iter().cloned().collect();
        let have: HashSet<String> = self.chain.lock().ok()
            .map(|m| m.keys().cloned().collect())
            .unwrap_or_default();
        for u in want.difference(&have) {
            let _ = self.subscribe_chain(u);
        }
        for u in have.difference(&want) {
            self.unsubscribe_chain(u);
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
        self.gap_fill_on_reconnect_with_source(symbol, timeframe, BarSource::Last).await
    }

    pub async fn gap_fill_on_reconnect_with_source(
        &self,
        symbol: &str,
        timeframe: &str,
        source: BarSource,
    ) -> Result<usize, ApiError> {
        let key = (symbol.to_string(), timeframe.to_string(), source);
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
            tracing::info!(
                target: "providers.sub_mgr",
                symbol, timeframe,
                "gap_fill_on_reconnect: no last_seen_ts yet — skipping"
            );
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
        tracing::info!(
            target: "providers.sub_mgr",
            symbol, timeframe, last_ts, now_ms, replayed = n,
            "gap_fill_on_reconnect: replayed bars through fanout"
        );
        for b in bars {
            let _ = fanout.send(b);
        }
        Ok(n)
    }

    /// Walk every active bar subscription and trigger a gap-fill replay.
    ///
    /// Wave 7E entry point — called from a WS feed's reconnect-success hook.
    /// Returns total bars replayed across all subs. Errors per-sub are logged
    /// and swallowed so one bad symbol can't block the others.
    pub async fn gap_fill_on_reconnect_all(&self) -> usize {
        let keys: Vec<(String, String, BarSource)> = match self.bars.lock() {
            Ok(g) => g.keys().cloned().collect(),
            Err(_) => return 0,
        };
        let mut total = 0usize;
        for (sym, tf, src) in keys {
            // Mark-source subs have no provider trait surface for historical
            // bars yet, so gap-fill replay only runs for Last. Skip Mark
            // entries silently — their `last_seen_ts` is still tracked for
            // future use.
            if src != BarSource::Last { continue; }
            match self.gap_fill_on_reconnect_with_source(&sym, &tf, src).await {
                Ok(n) => total = total.saturating_add(n),
                Err(e) => {
                    tracing::warn!(
                        target: "providers.sub_mgr",
                        symbol = %sym, timeframe = %tf, source = ?src, err = %e,
                        "gap_fill_on_reconnect_all: per-sub error swallowed"
                    );
                }
            }
        }
        total
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
            for ((s, tf, src), sub) in map.iter() {
                check("bars", &format!("{s}:{tf}:{}", src.as_str()), &sub.last_activity);
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
    async fn subscribe_bars_calls_underlying_feed_once_per_key() {
        let prov = Arc::new(MockProvider::new());
        let mgr = SubscriptionManager::new(prov.clone());
        // 4 panes ask for the same (sym, tf).
        let _a = mgr.subscribe_bars("MSFT", "1m").unwrap();
        let _b = mgr.subscribe_bars("MSFT", "1m").unwrap();
        let _c = mgr.subscribe_bars("MSFT", "1m").unwrap();
        let _d = mgr.subscribe_bars("MSFT", "1m").unwrap();
        // Underlying feed should have been touched exactly once.
        assert_eq!(prov.live_subs("MSFT:1m"), 1);
    }

    #[tokio::test]
    async fn unsubscribe_bars_calls_underlying_only_when_refcount_zero() {
        let prov = Arc::new(MockProvider::new());
        let mgr = SubscriptionManager::new(prov.clone());
        for _ in 0..5 { let _ = mgr.subscribe_bars("NVDA", "5m").unwrap(); }
        assert_eq!(prov.live_subs("NVDA:5m"), 1);
        for _ in 0..4 { mgr.unsubscribe_bars("NVDA", "5m"); }
        // 4 of 5 unsubs — still live.
        assert_eq!(prov.live_subs("NVDA:5m"), 1);
        // 5th unsub — last one out, underlying call fires.
        mgr.unsubscribe_bars("NVDA", "5m");
        assert_eq!(prov.live_subs("NVDA:5m"), 0);
    }

    #[tokio::test]
    async fn bump_last_seen_advances_timestamp_monotonically() {
        let prov = Arc::new(MockProvider::new());
        let mgr = SubscriptionManager::new(prov.clone());
        let _r = mgr.subscribe_bars("AMD", "1m").unwrap();
        assert_eq!(mgr.last_seen_bar("AMD", "1m"), 0);
        mgr.bump_last_seen_bar("AMD", "1m", BarSource::Last,100);
        assert_eq!(mgr.last_seen_bar("AMD", "1m"), 100);
        mgr.bump_last_seen_bar("AMD", "1m", BarSource::Last,200);
        assert_eq!(mgr.last_seen_bar("AMD", "1m"), 200);
        // Going backwards is ignored.
        mgr.bump_last_seen_bar("AMD", "1m", BarSource::Last,150);
        assert_eq!(mgr.last_seen_bar("AMD", "1m"), 200);
        // Unknown key is a no-op (doesn't insert).
        mgr.bump_last_seen_bar("GHOST", "1m", BarSource::Last, 999);
        assert_eq!(mgr.last_seen_bar("GHOST", "1m"), 0);
    }

    #[tokio::test]
    async fn set_quotes_diffs_against_existing() {
        let prov = Arc::new(MockProvider::new());
        let mgr = SubscriptionManager::new(prov.clone());
        mgr.set_quotes(&["AAPL".into(), "MSFT".into(), "TSLA".into()]);
        assert_eq!(prov.live_subs("AAPL"), 1);
        assert_eq!(prov.live_subs("MSFT"), 1);
        assert_eq!(prov.live_subs("TSLA"), 1);
        // Replace TSLA with NVDA — TSLA should drop, NVDA should add.
        mgr.set_quotes(&["AAPL".into(), "MSFT".into(), "NVDA".into()]);
        assert_eq!(prov.live_subs("AAPL"), 1);
        assert_eq!(prov.live_subs("MSFT"), 1);
        assert_eq!(prov.live_subs("TSLA"), 0);
        assert_eq!(prov.live_subs("NVDA"), 1);
        // Clear all.
        mgr.set_quotes(&[]);
        assert_eq!(prov.live_subs("AAPL"), 0);
        assert_eq!(prov.live_subs("MSFT"), 0);
        assert_eq!(prov.live_subs("NVDA"), 0);
    }

    #[tokio::test]
    async fn subscribe_bars_with_source_distinguishes_sources() {
        let prov = Arc::new(MockProvider::new());
        let mgr = SubscriptionManager::new(prov.clone());
        // Same (sym, tf) with different sources → independent refcounts.
        let _last_a = mgr.subscribe_bars_with_source("AAPL", "5m", BarSource::Last).unwrap();
        let _last_b = mgr.subscribe_bars_with_source("AAPL", "5m", BarSource::Last).unwrap();
        let _mark_a = mgr.subscribe_bars_with_source("AAPL", "5m", BarSource::Mark).unwrap();
        // Provider should be touched once (Last only — Mark has no trait surface).
        assert_eq!(prov.live_subs("AAPL:5m"), 1, "Last underlying sub fired once");
        // bump_last_seen for Last only — Mark stays at 0.
        mgr.bump_last_seen_bar("AAPL", "5m", BarSource::Last, 1_000);
        mgr.bump_last_seen_bar("AAPL", "5m", BarSource::Mark, 2_000);
        assert_eq!(mgr.last_seen_bar_with_source("AAPL", "5m", BarSource::Last), 1_000);
        assert_eq!(mgr.last_seen_bar_with_source("AAPL", "5m", BarSource::Mark), 2_000);
        // Drop Last twice → underlying unsub fires.
        mgr.unsubscribe_bars_with_source("AAPL", "5m", BarSource::Last);
        assert_eq!(prov.live_subs("AAPL:5m"), 1, "still one Last ref");
        mgr.unsubscribe_bars_with_source("AAPL", "5m", BarSource::Last);
        assert_eq!(prov.live_subs("AAPL:5m"), 0, "Last evicted at zero");
        // Mark sub still alive — independent.
        assert_eq!(mgr.last_seen_bar_with_source("AAPL", "5m", BarSource::Mark), 2_000);
        mgr.unsubscribe_bars_with_source("AAPL", "5m", BarSource::Mark);
        assert_eq!(mgr.last_seen_bar_with_source("AAPL", "5m", BarSource::Mark), 0);
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

    // ── Wave 9e: concurrent subscribe/unsubscribe stress ───────────────────
    //
    // The existing tests cover only single-threaded behaviour. The
    // `Mutex<HashMap>` + `AtomicUsize` pattern inside SubscriptionManager
    // has been correct on paper but never exercised under contention. These
    // tests fan many threads through the same keys to catch any race in the
    // get-or-insert / refcount / evict-on-zero sequence.
    //
    // A multi-threaded tokio runtime is required because the first
    // subscriber on each key spawns an upstream-pump task via `tokio::spawn`
    // — and our worker threads need a runtime handle to enter so the spawn
    // doesn't panic. `std::thread::spawn` then runs those workers in
    // parallel under that runtime handle.

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn concurrent_subscribe_unsubscribe_stress() {
        use std::thread;
        let handle = tokio::runtime::Handle::current();
        let prov = Arc::new(MockProvider::new());
        let mgr = Arc::new(SubscriptionManager::new(prov.clone()));

        // 16 threads × 100 iterations = 1600 subscribe/unsubscribe pairs on
        // the same (symbol, timeframe). Net effect must be zero refcount.
        let mut handles = vec![];
        for _ in 0..16 {
            let mgr = mgr.clone();
            let h = handle.clone();
            handles.push(thread::spawn(move || {
                let _guard = h.enter();
                for _ in 0..100 {
                    let _ = mgr.subscribe_bars("AAPL", "1m");
                    mgr.unsubscribe_bars("AAPL", "1m");
                }
            }));
        }
        for h in handles { h.join().unwrap(); }

        // After all subscribe/unsubscribe pairs complete, the entry must be
        // evicted (refcount fell to zero on the last unsubscribe).
        let active = mgr.active_bar_subs();
        assert!(
            active.is_empty() || !active.iter().any(|(s, _, _)| s == "AAPL"),
            "expected AAPL bars to be evicted after balanced subscribe/unsubscribe, \
             got active = {:?}", active
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn concurrent_mixed_subscribes_eventually_consistent() {
        use std::thread;
        let handle = tokio::runtime::Handle::current();
        let prov = Arc::new(MockProvider::new());
        let mgr = Arc::new(SubscriptionManager::new(prov.clone()));

        // 16 threads, 4 distinct symbols × 2 timeframes:
        //   - threads 0..8 subscribe + leave subscribed (50 subs each)
        //   - threads 8..16 subscribe + immediately unsubscribe
        let mut handles = vec![];
        for i in 0..16 {
            let mgr = mgr.clone();
            let h = handle.clone();
            handles.push(thread::spawn(move || {
                let _guard = h.enter();
                for j in 0..50 {
                    let sym = format!("SYM{}", i % 4);
                    let tf = if j % 2 == 0 { "1m" } else { "5m" };
                    let _ = mgr.subscribe_bars(&sym, tf);
                    if i >= 8 {
                        mgr.unsubscribe_bars(&sym, tf);
                    }
                }
            }));
        }
        for h in handles { h.join().unwrap(); }

        // Net: at most 8 active subscriptions (4 symbols × 2 timeframes).
        let active = mgr.active_bar_subs();
        assert!(
            active.len() <= 8,
            "expected ≤8 active subs after stress, got {} ({:?})",
            active.len(),
            active
        );

        // No deadlocks, no panics, and every surviving entry has a positive
        // refcount (we never crossed zero on the wrong side).
        for (sym, tf, _) in active {
            let count = mgr.refcount_for_bar(&sym, &tf);
            assert!(count > 0, "{sym}:{tf} has zero refcount but is in active list");
        }
    }
}
