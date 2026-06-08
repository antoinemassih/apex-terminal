//! Per-symbol live state cache fed by the ApexData WebSocket + REST pollers.
//!
//! The UI reads from here each frame (cheap HashMap lookups). Writers are the
//! WS frame dispatcher on the tokio thread + two poller threads (health + snap).
//! All access is mutex-guarded.

use super::types::{
    Quote, Trade, Snapshot, HealthReady, FeedsResponse, GreeksRow, ChainRow,
    CombinedSignalV2, RegimeFrame, RegimeTransition,
    TradePlanV2, SpikeExplanation,
    SectorRotationReading, BreadthReading, MoversReading, MoverKind, HaltReading, HaltKind,
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
    toasts: Mutex<Vec<String>>,                    // pending toasts (severity prefix: \x01=warn \x02=danger \x03=critical \x04=success; no prefix=info)
    /// Per-underlying chain cache (§5.4.d): `HashMap<underlying, HashMap<ticker, row>>`.
    /// Seeded from REST, merged from `chain_delta` frames.
    chains: Mutex<HashMap<String, HashMap<String, ChainRow>>>,
    chain_touched: Mutex<HashMap<String, Instant>>, // last update per underlying
    connected: Mutex<bool>,
    health: Mutex<Option<HealthReady>>,
    feeds:  Mutex<Option<FeedsResponse>>,
    // ── SOTA UX state (Agent A) ────────────────────────────────────────────
    /// Latest full 4-axis regime frame from `Signal::Regime` (§4.3).
    /// `RwLock` not available here without a new dep — the existing module
    /// uses `Mutex` uniformly, so we follow suit for consistency.
    latest_regime: Mutex<Option<RegimeFrame>>,
    /// Per-symbol latest `CombinedSignalV2` from `Signal::Combined` (§4.6).
    latest_combined: Mutex<HashMap<String, CombinedSignalV2>>,
    /// Last 20 axis transitions today, for the optional tape transitions strip.
    regime_transitions: Mutex<VecDeque<RegimeTransition>>,
    // ── SOTA §4.4 — TradePlan v2 (latest per symbol) ─────────────────────
    latest_trade_plan: Mutex<HashMap<String, TradePlanV2>>,
    // ── SOTA §4.5 — Spike explanations (capped ring) ─────────────────────
    /// Bounded ring of the most recent N spike explanations. Newest at the
    /// back; capacity SPIKE_HISTORY_CAP.
    recent_spike_explanations: Mutex<VecDeque<SpikeExplanation>>,
    /// Dedup set — popup component reads this to skip re-pushed ids it
    /// already dismissed. Server may republish on reconnect; we don't want
    /// the toast to re-fire.
    dismissed_spikes: Mutex<HashSet<String>>,
    // Wave 10 projector caches
    sector_rotation: Mutex<Option<(SectorRotationReading, Instant)>>,
    breadth: Mutex<HashMap<String, (BreadthReading, Instant)>>,  // keyed by lower-case index
    movers: Mutex<HashMap<String, (MoversReading, Instant)>>,    // keyed by kind.as_str()
    recent_halts: Mutex<VecDeque<HaltReading>>,                  // capped at 50
    /// Sequenced halt history for the toast overlay — we keep cleared events so
    /// the UI can dismiss the matching active toast.
    halt_events: Mutex<VecDeque<HaltReading>>,                   // capped at 100
}

/// Max spikes kept in the ring (per §4.5 — recent activity, not history).
const SPIKE_HISTORY_CAP: usize = 10;
/// Maximum size of the dismissed-spike dedup set before trimming. When
/// exceeded, the set is pruned down to this cap by clearing entries whose
/// IDs no longer appear in the live spike ring (those are the "true" stale
/// ones). If the set is still over the cap after that pass (i.e. all live
/// ring IDs were in the set), old entries are cleared entirely so the set
/// cannot grow without bound across a long session.
const DISMISSED_SPIKES_CAP: usize = 500;

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
        latest_regime: Mutex::new(None),
        latest_combined: Mutex::new(HashMap::new()),
        regime_transitions: Mutex::new(VecDeque::with_capacity(20)),
        latest_trade_plan: Mutex::new(HashMap::new()),
        recent_spike_explanations: Mutex::new(VecDeque::with_capacity(SPIKE_HISTORY_CAP)),
        dismissed_spikes: Mutex::new(HashSet::new()),
        sector_rotation: Mutex::new(None),
        breadth: Mutex::new(HashMap::new()),
        movers: Mutex::new(HashMap::new()),
        recent_halts: Mutex::new(VecDeque::with_capacity(50)),
        halt_events: Mutex::new(VecDeque::with_capacity(100)),
    })
}

// ── Background pollers ─────────────────────────────────────────────────────

/// Start the health + snapshot pollers. Safe to call multiple times.
pub fn start_pollers() {
    static STARTED: std::sync::Once = std::sync::Once::new();
    STARTED.call_once(|| {
        // Health + feeds poller: 1s health, 5s feeds.
        std::thread::Builder::new().name("apex-health".into()).spawn(|| {
            use crate::data::connectivity::errors_sink::{report, ErrorLevel};
            use crate::data::connectivity::error::ApiError;
            let mut last_feeds = Instant::now() - Duration::from_secs(10);
            loop {
                match super::rest::get_health_ready() {
                    Ok(h) => { if let Ok(mut g) = state().health.lock() { *g = Some(h); } }
                    // Circuit-open is expected once tripped; don't spam the sink.
                    Err(ApiError::CircuitOpen) => {}
                    Err(e) => {
                        report(ErrorLevel::Warn, "apex_data.rest", "health_poll", e.to_string());
                    }
                }
                if last_feeds.elapsed() >= Duration::from_secs(5) {
                    match super::rest::get_feeds() {
                        Ok(f) => { if let Ok(mut g) = state().feeds.lock() { *g = Some(f); } }
                        Err(ApiError::CircuitOpen) => {}
                        Err(e) => {
                            report(ErrorLevel::Warn, "apex_data.rest", "feeds_poll", e.to_string());
                        }
                    }
                    last_feeds = Instant::now();
                }
                std::thread::sleep(Duration::from_secs(1));
            }
        }).ok();

        // Snapshot poller: 1Hz per watched symbol.
        // Audit fix Wave 3: also evicts symbols that left the watch set every
        // 30s to prevent quotes/snapshots/fmv/tape_by_symbol/latest_combined/
        // latest_trade_plan from growing with every symbol ever seen.
        // tape_by_symbol is additionally hard-capped at 500 keys.
        std::thread::Builder::new().name("apex-snap".into()).spawn(|| {
            use crate::data::connectivity::errors_sink::{report, ErrorLevel};
            use crate::data::connectivity::error::ApiError;
            const TAPE_BY_SYMBOL_KEY_CAP: usize = 500;
            let mut evict_tick: u32 = 0;
            loop {
                let watched: Vec<String> = state().snap_watch.lock().ok()
                    .map(|g| g.iter().cloned().collect()).unwrap_or_default();
                for sym in &watched {
                    let class = super::types::AssetClass::from_symbol(sym);
                    match super::rest::get_snapshot(class, sym) {
                        Ok(s) => {
                            if let Ok(mut g) = state().snapshots.lock() {
                                g.insert(sym.clone(), (s, Instant::now()));
                            }
                        }
                        Err(ApiError::CircuitOpen) => {}
                        // Route not exposed on this deployment (e.g. per-symbol
                        // `/api/snap/*` is missing — bulk snapshot + chain still
                        // work). A polled 404 is a standing condition, not a
                        // per-symbol-per-second user toast, so skip it silently
                        // like CircuitOpen. Real errors (5xx/timeout/parse) below
                        // still surface.
                        Err(ApiError::Http { status: 404, .. }) => {}
                        Err(e) => {
                            report(ErrorLevel::Warn, "apex_data.rest", "snapshot_poll",
                                format!("{sym}: {e}"));
                        }
                    }
                }
                evict_tick = evict_tick.wrapping_add(1);
                // Every 30 iterations (~30s): evict symbols no longer in watch set.
                if evict_tick % 30 == 0 {
                    let watch_set: HashSet<String> = watched.iter().cloned().collect();
                    if let Ok(mut g) = state().snapshots.lock() {
                        g.retain(|k, _| watch_set.contains(k));
                    }
                    if let Ok(mut g) = state().quotes.lock() {
                        g.retain(|k, _| watch_set.contains(k));
                    }
                    if let Ok(mut g) = state().fmv.lock() {
                        g.retain(|k, _| watch_set.contains(k));
                    }
                    if let Ok(mut g) = state().latest_combined.lock() {
                        g.retain(|k, _| watch_set.contains(k));
                    }
                    if let Ok(mut g) = state().latest_trade_plan.lock() {
                        g.retain(|k, _| watch_set.contains(k));
                    }
                    // tape_by_symbol: evict unwatched keys then hard-cap at
                    // TAPE_BY_SYMBOL_KEY_CAP in case watched set is very large.
                    if let Ok(mut g) = state().tape_by_symbol.lock() {
                        g.retain(|k, _| watch_set.contains(k));
                        if g.len() > TAPE_BY_SYMBOL_KEY_CAP {
                            // Remove excess keys (arbitrary — HashMap has no order).
                            let remove: Vec<String> = g.keys()
                                .filter(|k| !watch_set.contains(*k))
                                .take(g.len() - TAPE_BY_SYMBOL_KEY_CAP)
                                .cloned()
                                .collect();
                            for k in remove { g.remove(&k); }
                        }
                    }
                }
                std::thread::sleep(Duration::from_secs(1));
            }
        }).ok();

        // Wave 10 projector poller: sector rotation (15s), breadth (5s), movers (5s).
        // Cheap: 3 GETs every 5s. The REST circuit breaker silences any missing
        // routes — panels fall back to a "loading" state when reading absent caches.
        std::thread::Builder::new().name("apex-projectors".into()).spawn(|| {
            let mut last_rotation = Instant::now() - Duration::from_secs(60);
            loop {
                // Movers — one bucket per tick, round-robin to spread load.
                for kind in super::types::MoverKind::all() {
                    if let Some(m) = super::rest::get_movers(kind) {
                        set_movers(kind, m);
                    }
                }
                // Breadth — fetch the default index (`us`); panels that need a
                // specific index can call get_breadth(idx) directly through fetch helpers.
                if let Some(b) = super::rest::get_breadth("us") {
                    set_breadth("us", b);
                }
                // Sector rotation — every 15s (slower-changing).
                if last_rotation.elapsed() >= Duration::from_secs(15) {
                    if let Some(r) = super::rest::get_sector_rotation() {
                        set_sector_rotation(r);
                    }
                    last_rotation = Instant::now();
                }
                std::thread::sleep(Duration::from_secs(5));
            }
        }).ok();

        // Greeks poller: 1 fetch every 2s per watched contract.
        // Audit fix Wave 3: evicts contracts that left greeks_watch every ~60s.
        std::thread::Builder::new().name("apex-greeks".into()).spawn(|| {
            use crate::data::connectivity::errors_sink::{report, ErrorLevel};
            use crate::data::connectivity::error::ApiError;
            let mut evict_tick: u32 = 0;
            loop {
                let watched: Vec<String> = state().greeks_watch.lock().ok()
                    .map(|g| g.iter().cloned().collect()).unwrap_or_default();
                for contract in &watched {
                    match super::rest::get_greeks(contract) {
                        Ok(gr) => {
                            if let Ok(mut g) = state().greeks.lock() {
                                g.insert(contract.clone(), (gr, Instant::now()));
                            }
                        }
                        Err(ApiError::CircuitOpen) => {}
                        // Missing route on this deployment — skip silently (see
                        // the snapshot poller note) rather than toast every 2s.
                        Err(ApiError::Http { status: 404, .. }) => {}
                        Err(e) => {
                            report(ErrorLevel::Warn, "apex_data.rest", "greeks_poll",
                                format!("{contract}: {e}"));
                        }
                    }
                }
                evict_tick = evict_tick.wrapping_add(1);
                // Every 30 iterations (~60s at 2s sleep): remove stale contracts.
                if evict_tick % 30 == 0 {
                    let watch_set: HashSet<String> = watched.iter().cloned().collect();
                    if let Ok(mut g) = state().greeks.lock() {
                        g.retain(|k, _| watch_set.contains(k));
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
// detached task fetches the projector reading and writes it back. Multiple
// concurrent callers may briefly race, but the cost is one duplicate REST
// hit — acceptable for our cardinality (≤ ~200 tickers).
//
// Audit fix Wave 3: each projector fetch now uses `tokio::task::spawn_blocking`
// on the shared `ws::runtime()` instead of spawning a new OS thread per call.
// The REST functions are still blocking (reqwest blocking client), so
// spawn_blocking is the right primitive. The inflight guard remains in place
// to prevent concurrent duplicate fetches for the same key.
fn proj_spawn<F: FnOnce() + Send + 'static>(task: F) {
    // Use the already-running apex-data-ws tokio runtime so we don't
    // allocate a new OS thread per projector fetch.
    //
    // We call `Runtime::spawn_blocking` directly on the runtime handle
    // rather than the free function `tokio::task::spawn_blocking(...)`
    // wrapped in `.spawn(...)`. The free function requires an *ambient*
    // Tokio runtime context — render-thread call sites don't have one,
    // so it panics with "there is no reactor running" (e.g. clicking
    // Watchlist would crash). `Runtime::spawn_blocking` is a method on
    // the runtime itself and brings its own context.
    super::ws::runtime().spawn_blocking(task);
}

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
    proj_spawn(move || {
        let resp = super::rest::get_news(&owned, None).unwrap_or_default();
        if let Ok(mut g) = proj().news.lock() {
            g.insert(owned.clone(), (resp, Instant::now()));
        }
        release_inflight(&claim_key);
    });
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
    proj_spawn(move || {
        let resp = super::rest::get_iv_rank(&owned, lookback).unwrap_or_default();
        if let Ok(mut g) = proj().iv_rank.lock() {
            g.insert(owned.clone(), (resp, Instant::now()));
        }
        release_inflight(&claim_key);
    });
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
    proj_spawn(move || {
        let resp = super::rest::get_etf_iiv(&owned).unwrap_or_default();
        if let Ok(mut g) = proj().iiv.lock() {
            g.insert(owned.clone(), (resp, Instant::now()));
        }
        release_inflight(&claim_key);
    });
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
    proj_spawn(move || {
        let resp = super::rest::get_corp_actions(&owned).unwrap_or_default();
        if let Ok(mut g) = proj().corp.lock() {
            g.insert(owned.clone(), (resp, Instant::now()));
        }
        release_inflight(&claim_key);
    });
}

/// Push a toast with explicit severity encoded as a leading control byte.
///
/// Severity byte vocabulary (stripped by `top_nav.rs` before display):
///   `0x00` / no prefix → Info   (accent tint)
///   `0x01`             → Warning (warn tint)
///   `0x02`             → Danger  (bear tint, opaque)
///   `0x03`             → Critical (bear tint, stronger + pulse animation)
///   `0x04`             → Success  (bull tint)
///
/// Other values push the message with no prefix (falls back to Info rendering).
pub fn push_toast_with_severity(msg: impl Into<String>, sev: u8) {
    let raw = msg.into();
    let encoded = match sev {
        1 => format!("\x01{raw}"),
        2 => format!("\x02{raw}"),
        3 => format!("\x03{raw}"),
        4 => format!("\x04{raw}"),
        _ => raw,
    };
    if let Ok(mut g) = state().toasts.lock() { g.push(encoded); }
}
/// Push a plain toast (no severity prefix). Kept for call-site compatibility.
pub fn push_toast(msg: impl Into<String>) {
    if let Ok(mut g) = state().toasts.lock() { g.push(msg.into()); }
}
/// Drain all pending toasts. Messages may have a leading control-byte severity prefix.
pub fn drain_toasts() -> Vec<String> {
    state().toasts.lock().ok().map(|mut g| std::mem::take(&mut *g)).unwrap_or_default()
}

// ── SOTA UX (Agent A) ───────────────────────────────────────────────────────

/// Replace the latest regime frame. Called from `ws::dispatch` when a
/// `regime` frame arrives. Also pushes a transition entry when the regime
/// key differs from the previous one so the optional tape sub-strip has
/// something to render.
pub fn push_regime(frame: RegimeFrame) {
    let prev_axes = state().latest_regime.lock().ok().and_then(|g| g.clone());
    if let Ok(mut g) = state().latest_regime.lock() {
        *g = Some(frame.clone());
    }
    if let Some(prev) = prev_axes {
        let now = &frame.regime;
        let old = &prev.regime;
        let t_ms = frame.t_ms;
        let mut push = |axis: &str, from: &str, to: &str| {
            if from != to {
                if let Ok(mut q) = state().regime_transitions.lock() {
                    q.push_back(RegimeTransition {
                        axis: axis.into(), from: from.into(),
                        to: to.into(), t_ms,
                    });
                    while q.len() > 20 { q.pop_front(); }
                }
            }
        };
        push("intraday", &old.intraday.value, &now.intraday.value);
        push("multiday", &old.multiday.value, &now.multiday.value);
        push("vol",      &old.vol.value,      &now.vol.value);
        push("sector",   &old.sector.value,   &now.sector.value);
    }
}

pub fn get_regime() -> Option<RegimeFrame> {
    state().latest_regime.lock().ok().and_then(|g| g.clone())
}

pub fn get_regime_transitions() -> Vec<RegimeTransition> {
    state().regime_transitions.lock().ok()
        .map(|g| g.iter().cloned().collect()).unwrap_or_default()
}

/// Replace the latest combined signal for a symbol. Called from `ws::dispatch`
/// when a `combined` frame arrives.
pub fn push_combined(signal: CombinedSignalV2) {
    if let Ok(mut g) = state().latest_combined.lock() {
        g.insert(signal.symbol.clone(), signal);
    }
}

pub fn get_combined(symbol: &str) -> Option<CombinedSignalV2> {
    state().latest_combined.lock().ok()?.get(symbol).cloned()
}

/// All cached combined signals, cloned. Sorted by score descending so the
/// SignalsPanel can render the top N directly.
///
/// The lock is held only long enough to clone out the values; sorting happens
/// after the guard is dropped so the hot WS writer is not blocked during sort.
pub fn all_combined_sorted() -> Vec<CombinedSignalV2> {
    let mut v: Vec<CombinedSignalV2> = {
        let g = match state().latest_combined.lock() { Ok(g) => g, Err(_) => return vec![] };
        g.values().cloned().collect()
        // guard dropped here
    };
    v.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    v
}

// ── SOTA §4.4 — Trade plan v2 (latest per symbol) ──────────────────────────

/// Upsert the latest TradePlan v2 for a symbol. Idempotent. Older plans for
/// the same symbol are replaced wholesale — the panel only ever shows the
/// most recent one.
pub fn push_trade_plan(plan: TradePlanV2) {
    if let Ok(mut g) = state().latest_trade_plan.lock() {
        g.insert(plan.symbol.clone(), plan);
    }
}

/// Read the latest plan for a symbol. Returns `None` if no plan has arrived
/// for this symbol yet (or the lock is poisoned).
pub fn get_trade_plan(symbol: &str) -> Option<TradePlanV2> {
    state().latest_trade_plan.lock().ok()?.get(symbol).cloned()
}

/// Snapshot all current trade plans (cheap clone of the inner HashMap). Used
/// by the trade-plan panel which renders one row per active symbol.
pub fn all_trade_plans() -> HashMap<String, TradePlanV2> {
    state().latest_trade_plan.lock().ok().map(|g| g.clone()).unwrap_or_default()
}

// ── SOTA §4.5 — Spike explanations ─────────────────────────────────────────

/// Push a freshly arrived spike explanation into the recent-ring. Drops the
/// oldest if at cap. The dedup set is *not* mutated here — it's the popup
/// component's responsibility to call `dismiss_spike(id)` when the user
/// closes a toast. We deliberately keep "the server pushed it again" and
/// "the user dismissed it" as orthogonal state.
pub fn push_spike(spike: SpikeExplanation) {
    if let Ok(mut g) = state().recent_spike_explanations.lock() {
        // Idempotent: if the same id already lives in the ring, replace it in
        // place rather than appending a duplicate. Lets the server republish
        // freely (e.g. on reconnect) without bloating the ring.
        if let Some(idx) = g.iter().position(|s| s.id == spike.id) {
            g[idx] = spike;
            return;
        }
        g.push_back(spike);
        while g.len() > SPIKE_HISTORY_CAP { g.pop_front(); }
    }
}

/// Snapshot of all spikes currently in the ring, newest-first. Cheap clone —
/// SPIKE_HISTORY_CAP is small (10).
pub fn iter_spikes() -> Vec<SpikeExplanation> {
    state().recent_spike_explanations.lock().ok()
        .map(|g| g.iter().rev().cloned().collect())
        .unwrap_or_default()
}

/// Mark a spike id as dismissed. The popup component checks this set before
/// re-rendering an old toast that the server republished.
/// Bounded at DISMISSED_SPIKES_CAP: when the set exceeds the cap, IDs that
/// no longer appear in the live spike ring are removed first; if still over
/// cap the entire set is cleared (worst case: a previously-dismissed toast
/// briefly reappears once on reconnect, far better than unbounded growth).
pub fn dismiss_spike(id: &str) {
    if let Ok(mut g) = state().dismissed_spikes.lock() {
        g.insert(id.to_string());
        if g.len() > DISMISSED_SPIKES_CAP {
            // First pass: drop IDs not in the live ring (truly stale dismissals).
            let live_ids: HashSet<String> = state().recent_spike_explanations
                .lock()
                .ok()
                .map(|ring| ring.iter().map(|s| s.id.clone()).collect())
                .unwrap_or_default();
            g.retain(|k| live_ids.contains(k));
            // Second pass: if still over cap (all live IDs were in the set),
            // clear entirely so the set stays bounded at all times.
            if g.len() > DISMISSED_SPIKES_CAP {
                g.clear();
            }
        }
    }
}

/// Test: has this spike id been dismissed? Used by the popup state machine.
pub fn is_spike_dismissed(id: &str) -> bool {
    state().dismissed_spikes.lock().ok().map(|g| g.contains(id)).unwrap_or(false)
}

#[cfg(test)]
pub fn reset_spike_state_for_tests() {
    if let Ok(mut g) = state().recent_spike_explanations.lock() { g.clear(); }
    if let Ok(mut g) = state().dismissed_spikes.lock() { g.clear(); }
    if let Ok(mut g) = state().latest_trade_plan.lock() { g.clear(); }
}

// ── Wave 10 projector caches ───────────────────────────────────────────────

/// Cache the latest sector-rotation projector reading.
pub fn set_sector_rotation(r: SectorRotationReading) {
    if let Ok(mut g) = state().sector_rotation.lock() { *g = Some((r, Instant::now())); }
}
/// Read the cached sector rotation (cloned). `None` if never fetched.
pub fn get_sector_rotation() -> Option<SectorRotationReading> {
    state().sector_rotation.lock().ok()?.as_ref().map(|(r, _)| r.clone())
}
/// Age of the cached reading in seconds — `u64::MAX` if absent.
pub fn sector_rotation_age_secs() -> u64 {
    state().sector_rotation.lock().ok()
        .and_then(|g| g.as_ref().map(|(_, t)| t.elapsed().as_secs()))
        .unwrap_or(u64::MAX)
}

pub fn set_breadth(index: &str, r: BreadthReading) {
    if let Ok(mut g) = state().breadth.lock() {
        g.insert(index.to_lowercase(), (r, Instant::now()));
    }
}
pub fn get_breadth(index: &str) -> Option<BreadthReading> {
    state().breadth.lock().ok()?.get(&index.to_lowercase()).map(|(r, _)| r.clone())
}

pub fn set_movers(kind: MoverKind, r: MoversReading) {
    if let Ok(mut g) = state().movers.lock() {
        g.insert(kind.as_str().to_string(), (r, Instant::now()));
    }
}
pub fn get_movers(kind: MoverKind) -> Option<MoversReading> {
    state().movers.lock().ok()?.get(kind.as_str()).map(|(r, _)| r.clone())
}
pub fn movers_age_secs(kind: MoverKind) -> u64 {
    state().movers.lock().ok()
        .and_then(|g| g.get(kind.as_str()).map(|(_, t)| t.elapsed().as_secs()))
        .unwrap_or(u64::MAX)
}

// ── Halts (WS `Frame::Halt`) ───────────────────────────────────────────────

/// Push a halt event into the cap-50 recent-halts ring. `HaltCleared` removes
/// the matching `HaltActive` for the same symbol from `recent_halts` (so the
/// "currently halted" set stays accurate) but always lands in `halt_events`.
pub fn push_halt(h: HaltReading) {
    if let Ok(mut g) = state().halt_events.lock() {
        g.push_back(h.clone());
        while g.len() > 100 { g.pop_front(); }
    }
    if let Ok(mut g) = state().recent_halts.lock() {
        match h.kind {
            HaltKind::HaltCleared => {
                // Remove any active halt for this symbol.
                g.retain(|x| !(x.symbol.eq_ignore_ascii_case(&h.symbol)
                            && matches!(x.kind, HaltKind::HaltActive)));
                // Still record the clear so toast logic can pair them.
                g.push_back(h);
            }
            _ => {
                g.push_back(h);
            }
        }
        while g.len() > 50 { g.pop_front(); }
    }
}

/// Snapshot of recent halts (most-recent last).
pub fn recent_halts() -> Vec<HaltReading> {
    state().recent_halts.lock().ok().map(|g| g.iter().cloned().collect()).unwrap_or_default()
}

/// Drain the halt-event queue for the toast renderer. Empties the queue.
pub fn drain_halt_events() -> Vec<HaltReading> {
    state().halt_events.lock().ok().map(|mut g| {
        let out: Vec<_> = g.iter().cloned().collect();
        g.clear();
        out
    }).unwrap_or_default()
}

#[cfg(test)]
mod halt_tests {
    use super::*;

    #[test]
    fn halt_cleared_removes_matching_active() {
        // Use a fresh state by exercising the global; safe because each test
        // pushes uniquely-named symbols.
        push_halt(HaltReading {
            symbol: "ZZTEST1".into(), kind: HaltKind::HaltActive,
            reason: "LULD".into(), price: 10.0, time_ms: 1, resumes_at_ms: 0,
        });
        assert!(recent_halts().iter().any(|h|
            h.symbol == "ZZTEST1" && matches!(h.kind, HaltKind::HaltActive)));
        push_halt(HaltReading {
            symbol: "ZZTEST1".into(), kind: HaltKind::HaltCleared,
            reason: "LULD".into(), price: 10.5, time_ms: 2, resumes_at_ms: 0,
        });
        assert!(!recent_halts().iter().any(|h|
            h.symbol == "ZZTEST1" && matches!(h.kind, HaltKind::HaltActive)),
            "HaltActive should be removed after HaltCleared");
    }

    #[test]
    fn drain_halt_events_empties_queue() {
        push_halt(HaltReading {
            symbol: "ZZTEST2".into(), kind: HaltKind::NearLuldUp,
            reason: "".into(), price: 1.0, time_ms: 1, resumes_at_ms: 0,
        });
        let events = drain_halt_events();
        assert!(events.iter().any(|h| h.symbol == "ZZTEST2"));
        // Second drain should not re-yield the same event.
        let events2 = drain_halt_events();
        assert!(!events2.iter().any(|h| h.symbol == "ZZTEST2"));
    }
}
