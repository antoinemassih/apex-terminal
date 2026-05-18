//! Per-symbol live state cache fed by the ApexData WebSocket + REST pollers.
//!
//! The UI reads from here each frame (cheap HashMap lookups). Writers are the
//! WS frame dispatcher on the tokio thread + two poller threads (health + snap).
//! All access is mutex-guarded.

use super::types::{
    Quote, Trade, Snapshot, HealthReady, FeedsResponse, GreeksRow, ChainRow,
    NewsResponse, IvRankV2, EtfIivReading, CorporateActionsReading,
};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

/// Fair Market Value snapshot (§6.1 `fmv` frame).
#[derive(Debug, Clone)]
pub struct Fmv {
    pub symbol: String,
    pub fmv: f64,
    pub time_ms: i64,
}

struct State {
    quotes: Mutex<HashMap<String, Quote>>,         // keyed by symbol
    tape:   Mutex<VecDeque<Trade>>,                // global ring buffer (200 max)
    tape_by_symbol: Mutex<HashMap<String, VecDeque<Trade>>>, // per-symbol (100 max each)
    snapshots: Mutex<HashMap<String, (Snapshot, Instant)>>, // per-symbol with fetch time
    snap_watch: Mutex<HashSet<String>>,            // symbols the poller should keep fresh
    greeks: Mutex<HashMap<String, (GreeksRow, Instant)>>,   // per-contract (OCC ticker)
    greeks_watch: Mutex<HashSet<String>>,          // contracts the poller should keep fresh
    fmv: Mutex<HashMap<String, Fmv>>,              // per-symbol FMV (options)
    toasts: Mutex<Vec<String>>,                    // pending server-error toasts
    /// Per-underlying chain cache (§5.4.d): `HashMap<underlying, HashMap<ticker, row>>`.
    /// Seeded from REST, merged from `chain_delta` frames.
    chains: Mutex<HashMap<String, HashMap<String, ChainRow>>>,
    chain_touched: Mutex<HashMap<String, Instant>>, // last update per underlying
    connected: Mutex<bool>,
    health: Mutex<Option<HealthReady>>,
    feeds:  Mutex<Option<FeedsResponse>>,
}

static STATE: OnceLock<State> = OnceLock::new();

fn state() -> &'static State {
    STATE.get_or_init(|| State {
        quotes: Mutex::new(HashMap::new()),
        tape:   Mutex::new(VecDeque::with_capacity(200)),
        tape_by_symbol: Mutex::new(HashMap::new()),
        snapshots: Mutex::new(HashMap::new()),
        snap_watch: Mutex::new(HashSet::new()),
        greeks: Mutex::new(HashMap::new()),
        greeks_watch: Mutex::new(HashSet::new()),
        fmv: Mutex::new(HashMap::new()),
        toasts: Mutex::new(Vec::new()),
        chains: Mutex::new(HashMap::new()),
        chain_touched: Mutex::new(HashMap::new()),
        connected: Mutex::new(false),
        health: Mutex::new(None),
        feeds:  Mutex::new(None),
    })
}

// ── Background pollers ─────────────────────────────────────────────────────

/// Start the health + snapshot pollers. Safe to call multiple times.
pub fn start_pollers() {
    static STARTED: std::sync::Once = std::sync::Once::new();
    STARTED.call_once(|| {
        // Health + feeds poller: 1s health, 5s feeds.
        std::thread::Builder::new().name("apex-health".into()).spawn(|| {
            let mut last_feeds = Instant::now() - Duration::from_secs(10);
            loop {
                if let Some(h) = super::rest::get_health_ready() {
                    if let Ok(mut g) = state().health.lock() { *g = Some(h); }
                }
                if last_feeds.elapsed() >= Duration::from_secs(5) {
                    if let Some(f) = super::rest::get_feeds() {
                        if let Ok(mut g) = state().feeds.lock() { *g = Some(f); }
                    }
                    last_feeds = Instant::now();
                }
                std::thread::sleep(Duration::from_secs(1));
            }
        }).ok();

        // Snapshot poller: 1Hz per watched symbol.
        std::thread::Builder::new().name("apex-snap".into()).spawn(|| {
            loop {
                let watched: Vec<String> = state().snap_watch.lock().ok()
                    .map(|g| g.iter().cloned().collect()).unwrap_or_default();
                for sym in watched {
                    let class = super::types::AssetClass::from_symbol(&sym);
                    if let Some(s) = super::rest::get_snapshot(class, &sym) {
                        if let Ok(mut g) = state().snapshots.lock() {
                            g.insert(sym, (s, Instant::now()));
                        }
                    }
                }
                std::thread::sleep(Duration::from_secs(1));
            }
        }).ok();

        // Greeks poller: 1 fetch every 2s per watched contract.
        std::thread::Builder::new().name("apex-greeks".into()).spawn(|| {
            loop {
                let watched: Vec<String> = state().greeks_watch.lock().ok()
                    .map(|g| g.iter().cloned().collect()).unwrap_or_default();
                for contract in watched {
                    if let Some(gr) = super::rest::get_greeks(&contract) {
                        if let Ok(mut g) = state().greeks.lock() {
                            g.insert(contract, (gr, Instant::now()));
                        }
                    }
                }
                std::thread::sleep(Duration::from_secs(2));
            }
        }).ok();
    });
}

pub fn watch_symbol(symbol: &str) {
    if let Ok(mut g) = state().snap_watch.lock() { g.insert(symbol.to_string()); }
}
pub fn unwatch_symbol(symbol: &str) {
    if let Ok(mut g) = state().snap_watch.lock() { g.remove(symbol); }
}
/// Replace the watched-symbol set. Called by the watchlist panel each frame.
pub fn set_watched_symbols<I: IntoIterator<Item = String>>(syms: I) {
    if let Ok(mut g) = state().snap_watch.lock() {
        g.clear(); g.extend(syms);
    }
}

/// Replace the watched-contract set for greeks polling.
pub fn set_watched_contracts<I: IntoIterator<Item = String>>(contracts: I) {
    if let Ok(mut g) = state().greeks_watch.lock() {
        g.clear(); g.extend(contracts);
    }
}

pub fn get_greeks(contract: &str) -> Option<GreeksRow> {
    state().greeks.lock().ok()?.get(contract).map(|(g, _)| g.clone())
}

/// Look up cached greeks by underlying. Used by widgets that only know the chart
/// symbol; returns whichever contract happens to be watched for that underlying
/// (in practice, the pane's ATM 0DTE call registered by the frame hook).
pub fn get_greeks_for_underlying(underlying: &str) -> Option<GreeksRow> {
    let g = state().greeks.lock().ok()?;
    g.values()
        .find(|(row, _)| row.underlying.eq_ignore_ascii_case(underlying))
        .map(|(row, _)| row.clone())
}

// ── Writers ────────────────────────────────────────────────────────────────

pub fn push_quote(q: Quote) {
    if let Ok(mut g) = state().quotes.lock() { g.insert(q.symbol.clone(), q); }
}

pub fn push_trade(t: Trade) {
    if let Ok(mut g) = state().tape.lock() {
        g.push_back(t.clone());
        while g.len() > 200 { g.pop_front(); }
    }
    if let Ok(mut g) = state().tape_by_symbol.lock() {
        let entry = g.entry(t.symbol.clone()).or_insert_with(|| VecDeque::with_capacity(100));
        entry.push_back(t);
        while entry.len() > 100 { entry.pop_front(); }
    }
}

pub fn set_connected(on: bool) {
    if let Ok(mut g) = state().connected.lock() { *g = on; }
}

// ── Readers ────────────────────────────────────────────────────────────────

pub fn get_quote(symbol: &str) -> Option<Quote> {
    state().quotes.lock().ok()?.get(symbol).cloned()
}

pub fn tape_global(max: usize) -> Vec<Trade> {
    state().tape.lock().ok().map(|g| {
        let n = g.len().min(max);
        g.iter().rev().take(n).rev().cloned().collect()
    }).unwrap_or_default()
}

pub fn tape_for(symbol: &str, max: usize) -> Vec<Trade> {
    state().tape_by_symbol.lock().ok().and_then(|g| {
        g.get(symbol).map(|q| {
            let n = q.len().min(max);
            q.iter().rev().take(n).rev().cloned().collect()
        })
    }).unwrap_or_default()
}

pub fn is_connected() -> bool {
    state().connected.lock().map(|g| *g).unwrap_or(false)
}

pub fn get_snapshot(symbol: &str) -> Option<Snapshot> {
    state().snapshots.lock().ok()?.get(symbol).map(|(s, _)| s.clone())
}

pub fn get_health() -> Option<HealthReady> {
    state().health.lock().ok()?.clone()
}

pub fn get_feeds() -> Option<FeedsResponse> {
    state().feeds.lock().ok()?.clone()
}

// ── FMV ────────────────────────────────────────────────────────────────────

pub fn push_fmv(v: Fmv) {
    if let Ok(mut g) = state().fmv.lock() { g.insert(v.symbol.clone(), v); }
}
pub fn get_fmv(symbol: &str) -> Option<Fmv> {
    state().fmv.lock().ok()?.get(symbol).cloned()
}

// ── Chain cache (§5.4.d) ───────────────────────────────────────────────────

/// Bulk-seed the local chain cache from a REST response. Replaces any existing
/// rows for that underlying — use this on the initial `/api/chain/:ul` fetch.
pub fn seed_chain(underlying: &str, rows: &[ChainRow]) {
    if let Ok(mut g) = state().chains.lock() {
        let m = g.entry(underlying.to_uppercase()).or_insert_with(HashMap::new);
        m.clear();
        for r in rows { m.insert(r.ticker.clone(), r.clone()); }
    }
    if let Ok(mut g) = state().chain_touched.lock() {
        g.insert(underlying.to_uppercase(), Instant::now());
    }
}

/// Merge delta rows from a WS chain_delta frame (§5.4.d).
pub fn merge_chain_delta(underlying: &str, rows: &[ChainRow]) {
    if let Ok(mut g) = state().chains.lock() {
        let m = g.entry(underlying.to_uppercase()).or_insert_with(HashMap::new);
        for r in rows { m.insert(r.ticker.clone(), r.clone()); }
    }
    if let Ok(mut g) = state().chain_touched.lock() {
        g.insert(underlying.to_uppercase(), Instant::now());
    }
}

/// Snapshot the chain for an underlying (cloned out of the cache).
pub fn get_chain(underlying: &str) -> Vec<ChainRow> {
    state().chains.lock().ok().and_then(|g| {
        g.get(&underlying.to_uppercase()).map(|m| m.values().cloned().collect())
    }).unwrap_or_default()
}

pub fn clear_chain(underlying: &str) {
    if let Ok(mut g) = state().chains.lock() {
        g.remove(&underlying.to_uppercase());
    }
}

/// Summary snapshot of every cached chain for the diagnostics panel:
/// returns `Vec<(underlying, row_count, seconds_since_last_update)>`.
pub fn chain_summary() -> Vec<(String, usize, u64)> {
    let chains = match state().chains.lock() { Ok(g) => g.clone(), Err(_) => return vec![] };
    let touched = match state().chain_touched.lock() { Ok(g) => g.clone(), Err(_) => return vec![] };
    let mut out: Vec<_> = chains.iter().map(|(k, v)| {
        let age = touched.get(k).map(|t| t.elapsed().as_secs()).unwrap_or(9999);
        (k.clone(), v.len(), age)
    }).collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

// ── Toasts (cross-thread) ──────────────────────────────────────────────────

// ── Wave 10 projector caches (per-ticker, TTL-gated REST fetch) ──────────
//
// Each cache stores `(value, fetched_at)`. Callers ask `get_or_fetch_*` from
// any thread; if the entry is missing or stale beyond the TTL, a single
// detached thread fetches the projector reading and writes it back. Multiple
// concurrent callers may briefly race, but the cost is one duplicate REST
// hit — acceptable for our cardinality (≤ ~200 tickers).

use std::collections::HashMap as StdHashMap;

const NEWS_TTL:   Duration = Duration::from_secs(60);
const IVRANK_TTL: Duration = Duration::from_secs(60);
const IIV_TTL:    Duration = Duration::from_secs(10);
const CORP_TTL:   Duration = Duration::from_secs(300);

struct ProjectorCaches {
    news:    Mutex<StdHashMap<String, (NewsResponse, Instant)>>,
    iv_rank: Mutex<StdHashMap<String, (IvRankV2, Instant)>>,
    iiv:     Mutex<StdHashMap<String, (EtfIivReading, Instant)>>,
    corp:    Mutex<StdHashMap<String, (CorporateActionsReading, Instant)>>,
    /// Tickers currently being fetched (per cache). Prevents thread storms
    /// when many UI frames request the same key while the first is in flight.
    inflight: Mutex<std::collections::HashSet<String>>,
}

static PROJ: OnceLock<ProjectorCaches> = OnceLock::new();
fn proj() -> &'static ProjectorCaches {
    PROJ.get_or_init(|| ProjectorCaches {
        news:    Mutex::new(StdHashMap::new()),
        iv_rank: Mutex::new(StdHashMap::new()),
        iiv:     Mutex::new(StdHashMap::new()),
        corp:    Mutex::new(StdHashMap::new()),
        inflight: Mutex::new(std::collections::HashSet::new()),
    })
}

fn try_claim_inflight(key: String) -> bool {
    if let Ok(mut g) = proj().inflight.lock() { g.insert(key) } else { false }
}
fn release_inflight(key: &str) {
    if let Ok(mut g) = proj().inflight.lock() { g.remove(key); }
}

// News --------------------------------------------------------------------

pub fn get_news(ticker: &str) -> Option<NewsResponse> {
    proj().news.lock().ok()?.get(&ticker.to_uppercase()).map(|(v, _)| v.clone())
}

pub fn get_or_fetch_news(ticker: &str) {
    let key = ticker.to_uppercase();
    let needs_fetch = proj().news.lock().ok().and_then(|g| g.get(&key).map(|(_, t)| t.elapsed() >= NEWS_TTL))
        .unwrap_or(true);
    if !needs_fetch { return; }
    let claim_key = format!("news:{key}");
    if !try_claim_inflight(claim_key.clone()) { return; }
    let owned = key.clone();
    std::thread::Builder::new().name("apex-news".into()).spawn(move || {
        let resp = super::rest::get_news(&owned, None).unwrap_or_default();
        if let Ok(mut g) = proj().news.lock() {
            g.insert(owned.clone(), (resp, Instant::now()));
        }
        release_inflight(&claim_key);
    }).ok();
}

// IV rank -----------------------------------------------------------------

pub fn get_iv_rank(underlying: &str) -> Option<IvRankV2> {
    proj().iv_rank.lock().ok()?.get(&underlying.to_uppercase()).map(|(v, _)| v.clone())
}

pub fn get_or_fetch_iv_rank(underlying: &str, lookback: Option<u32>) {
    let key = underlying.to_uppercase();
    let needs_fetch = proj().iv_rank.lock().ok().and_then(|g| g.get(&key).map(|(_, t)| t.elapsed() >= IVRANK_TTL))
        .unwrap_or(true);
    if !needs_fetch { return; }
    let claim_key = format!("ivrank:{key}");
    if !try_claim_inflight(claim_key.clone()) { return; }
    let owned = key.clone();
    std::thread::Builder::new().name("apex-ivrank".into()).spawn(move || {
        let resp = super::rest::get_iv_rank(&owned, lookback).unwrap_or_default();
        if let Ok(mut g) = proj().iv_rank.lock() {
            g.insert(owned.clone(), (resp, Instant::now()));
        }
        release_inflight(&claim_key);
    }).ok();
}

// ETF IIV -----------------------------------------------------------------

pub fn get_etf_iiv(etf: &str) -> Option<EtfIivReading> {
    proj().iiv.lock().ok()?.get(&etf.to_uppercase()).map(|(v, _)| v.clone())
}

pub fn get_or_fetch_etf_iiv(etf: &str) {
    let key = etf.to_uppercase();
    let needs_fetch = proj().iiv.lock().ok().and_then(|g| g.get(&key).map(|(_, t)| t.elapsed() >= IIV_TTL))
        .unwrap_or(true);
    if !needs_fetch { return; }
    let claim_key = format!("iiv:{key}");
    if !try_claim_inflight(claim_key.clone()) { return; }
    let owned = key.clone();
    std::thread::Builder::new().name("apex-iiv".into()).spawn(move || {
        let resp = super::rest::get_etf_iiv(&owned).unwrap_or_default();
        if let Ok(mut g) = proj().iiv.lock() {
            g.insert(owned.clone(), (resp, Instant::now()));
        }
        release_inflight(&claim_key);
    }).ok();
}

// Corporate actions -------------------------------------------------------

pub fn get_corp_actions(ticker: &str) -> Option<CorporateActionsReading> {
    proj().corp.lock().ok()?.get(&ticker.to_uppercase()).map(|(v, _)| v.clone())
}

pub fn get_or_fetch_corp_actions(ticker: &str) {
    let key = ticker.to_uppercase();
    let needs_fetch = proj().corp.lock().ok().and_then(|g| g.get(&key).map(|(_, t)| t.elapsed() >= CORP_TTL))
        .unwrap_or(true);
    if !needs_fetch { return; }
    let claim_key = format!("corp:{key}");
    if !try_claim_inflight(claim_key.clone()) { return; }
    let owned = key.clone();
    std::thread::Builder::new().name("apex-corp".into()).spawn(move || {
        let resp = super::rest::get_corp_actions(&owned).unwrap_or_default();
        if let Ok(mut g) = proj().corp.lock() {
            g.insert(owned.clone(), (resp, Instant::now()));
        }
        release_inflight(&claim_key);
    }).ok();
}

pub fn push_toast(msg: impl Into<String>) {
    if let Ok(mut g) = state().toasts.lock() { g.push(msg.into()); }
}
pub fn drain_toasts() -> Vec<String> {
    state().toasts.lock().ok().map(|mut g| std::mem::take(&mut *g)).unwrap_or_default()
}
