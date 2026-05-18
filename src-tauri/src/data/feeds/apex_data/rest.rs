//! Blocking REST client for ApexData.
//!
//! All calls are blocking so they can be invoked from background threads
//! (`std::thread::spawn`). The caller is responsible for not blocking the
//! render thread — spawn a thread and deliver results via a channel.
//!
//! Wave 7C: typed `Result<T, ApiError>` surface. Callers can still recover
//! the legacy `Option<T>` shape via `.ok()` while migration progresses.

use super::config::{apex_url, apex_token, is_enabled};
use super::types::*;
use crate::data::connectivity::error::{ApiError, AuthError};
use reqwest::blocking::Client;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

/// Simple circuit breaker: after `TRIP_THRESHOLD` consecutive failures,
/// shortcut all REST calls to None for `COOLDOWN` before probing again.
const TRIP_THRESHOLD: u32 = 3;
const COOLDOWN: Duration = Duration::from_secs(30);

struct Breaker { fails: u32, opened_at: Option<Instant> }

static BREAKER: OnceLock<Mutex<Breaker>> = OnceLock::new();
fn breaker() -> &'static Mutex<Breaker> {
    BREAKER.get_or_init(|| Mutex::new(Breaker { fails: 0, opened_at: None }))
}

// ── REST stats (for diagnostics panel) ─────────────────────────────────────

#[derive(Clone, Debug)]
pub struct RestCall {
    pub path: String,
    pub status: u16,          // 0 = network error, 1 = breaker-open, >= 200 = HTTP
    pub outcome: &'static str, // "ok" | "http" | "err" | "parse" | "skip"
    pub ms: u128,
    pub at: std::time::SystemTime,
}

pub struct RestStats {
    pub total_ok: u64,
    pub total_http_err: u64,
    pub total_net_err: u64,
    pub total_parse_err: u64,
    pub total_skipped: u64,
    pub recent: std::collections::VecDeque<RestCall>,
}

impl RestStats {
    pub fn new() -> Self { Self { total_ok: 0, total_http_err: 0, total_net_err: 0, total_parse_err: 0, total_skipped: 0, recent: std::collections::VecDeque::with_capacity(40) } }
}

static STATS: OnceLock<Mutex<RestStats>> = OnceLock::new();
fn stats() -> &'static Mutex<RestStats> {
    STATS.get_or_init(|| Mutex::new(RestStats::new()))
}

fn record(call: RestCall) {
    if let Ok(mut s) = stats().lock() {
        match call.outcome {
            "ok"    => s.total_ok += 1,
            "http"  => s.total_http_err += 1,
            "err"   => s.total_net_err += 1,
            "parse" => s.total_parse_err += 1,
            "skip"  => s.total_skipped += 1,
            _ => {}
        }
        s.recent.push_back(call);
        while s.recent.len() > 40 { s.recent.pop_front(); }
    }
}

/// Snapshot the current REST stats (for the diagnostics panel).
pub fn stats_snapshot() -> (u64, u64, u64, u64, u64, Vec<RestCall>) {
    stats().lock().ok().map(|s| {
        (s.total_ok, s.total_http_err, s.total_net_err, s.total_parse_err, s.total_skipped,
         s.recent.iter().cloned().collect())
    }).unwrap_or((0, 0, 0, 0, 0, vec![]))
}

/// Breaker state for the diagnostics panel.
pub fn breaker_snapshot() -> (u32, Option<Duration>) {
    breaker().lock().ok().map(|b| {
        let remaining = b.opened_at.map(|t| COOLDOWN.saturating_sub(t.elapsed()));
        (b.fails, remaining)
    }).unwrap_or((0, None))
}
fn breaker_is_open() -> bool {
    if let Ok(b) = breaker().lock() {
        if let Some(t) = b.opened_at { return t.elapsed() < COOLDOWN; }
    }
    false
}
fn breaker_note_success() {
    if let Ok(mut b) = breaker().lock() { b.fails = 0; b.opened_at = None; }
}
/// Manually clear the breaker (used after settings changes that may have
/// fixed the underlying connectivity issue).
pub fn reset_breaker() {
    if let Ok(mut b) = breaker().lock() { b.fails = 0; b.opened_at = None; }
}
fn breaker_note_failure() {
    if let Ok(mut b) = breaker().lock() {
        b.fails += 1;
        if b.fails >= TRIP_THRESHOLD { b.opened_at = Some(Instant::now()); }
    }
}

fn client() -> Client {
    let mut b = Client::builder()
        .timeout(Duration::from_secs(3))
        .connect_timeout(Duration::from_secs(1))
        .user_agent("apex-terminal/0.9");
    // LAN override: when configured, resolve the apex-data hostname to the
    // homelab Traefik IP directly (bypasses public DNS that returns an
    // un-routable WAN IP). Host header stays untouched so ingress routing works.
    if let (Some(ip), Some((host, port))) = (super::config::apex_lan_ip(), super::config::apex_host_port()) {
        if let Ok(ip_parsed) = ip.parse::<std::net::IpAddr>() {
            b = b.resolve(&host, std::net::SocketAddr::new(ip_parsed, port));
            crate::apex_log!("rest.cfg", "LAN override: {host}:{port} → {ip}");
        }
    }
    b.build().unwrap_or_else(|_| Client::new())
}

/// Typed GET. Maps HTTP status / network / parse failures to `ApiError`.
///
/// - 200 → `Ok(T)` after deserialization
/// - 401 → `ApiError::Auth(AuthError::TokenExpired)`
/// - other 4xx / 5xx → `ApiError::Http { status, body }`
/// - network error → `ApiError::Network(...)`
/// - parse error → `ApiError::Parse(...)`
/// - circuit open → `ApiError::CircuitOpen`
/// - apex-data disabled → `ApiError::NotSupported("apex_data disabled")`
fn get<T: serde::de::DeserializeOwned>(path: &str) -> Result<T, ApiError> {
    if !is_enabled() {
        crate::apex_log!("rest.skip", "disabled: {path}");
        record(RestCall { path: path.into(), status: 0, outcome: "skip", ms: 0, at: std::time::SystemTime::now() });
        return Err(ApiError::NotSupported("apex_data disabled".into()));
    }
    if breaker_is_open() {
        crate::apex_log!("rest.skip", "breaker open: {path}");
        record(RestCall { path: path.into(), status: 1, outcome: "skip", ms: 0, at: std::time::SystemTime::now() });
        return Err(ApiError::CircuitOpen);
    }
    let url = format!("{}{path}", apex_url());
    crate::apex_log!("rest.req", "GET {url}");
    let t0 = Instant::now();
    let mut req = client().get(&url);
    if let Some(tok) = apex_token() { req = req.bearer_auth(tok); }
    match req.send() {
        Ok(r) if r.status().is_success() => {
            let status = r.status();
            match r.json::<T>() {
                Ok(v) => {
                    crate::apex_log!("rest.ok", "{path} → {} ({:?})", status, t0.elapsed());
                    breaker_note_success();
                    record(RestCall { path: path.into(), status: status.as_u16(), outcome: "ok", ms: t0.elapsed().as_millis(), at: std::time::SystemTime::now() });
                    Ok(v)
                }
                Err(e) => {
                    crate::apex_log!("rest.parse", "{path} → {} body parse failed: {e}", status);
                    record(RestCall { path: path.into(), status: status.as_u16(), outcome: "parse", ms: t0.elapsed().as_millis(), at: std::time::SystemTime::now() });
                    Err(ApiError::Parse(e.to_string()))
                }
            }
        }
        Ok(r) => {
            let status = r.status();
            let code = status.as_u16();
            // Read body for diagnostics (trimmed by caller as needed).
            let body = r.text().unwrap_or_default();
            crate::apex_log!("rest.http", "{path} → {} ({:?})", status, t0.elapsed());
            record(RestCall { path: path.into(), status: code, outcome: "http", ms: t0.elapsed().as_millis(), at: std::time::SystemTime::now() });
            if code == 401 {
                Err(ApiError::Auth(AuthError::TokenExpired))
            } else {
                let trimmed: String = body.chars().take(512).collect();
                Err(ApiError::Http { status: code, body: trimmed })
            }
        }
        Err(e) => {
            crate::apex_log!("rest.err", "{path} network error ({:?}): {e}", t0.elapsed());
            breaker_note_failure();
            record(RestCall { path: path.into(), status: 0, outcome: "err", ms: t0.elapsed().as_millis(), at: std::time::SystemTime::now() });
            Err(ApiError::Network(e.to_string()))
        }
    }
}

// ── Auth-retry wrapper ─────────────────────────────────────────────────────
//
// Wraps a blocking REST closure with the typed auth-retry helper. The closure
// runs inside `tokio::task::spawn_blocking` so the async `with_auth_retry`
// surface stays compatible with our blocking `reqwest` client. We keep the
// blocking-client surface because migrating every caller (chart fetch, watchlist
// refresh, diagnostics) to async would dwarf this wave.
//
// Most callers today invoke REST functions directly from a worker thread, where
// a tokio runtime is NOT available. To preserve the blocking entry points we
// expose a pair: the raw `get_*` functions (synchronous, no auth retry) and
// the `*_with_auth_retry` async variants for code paths that have a runtime.

async fn with_apex_auth_retry<T, F, Fut>(f: F) -> Result<T, ApiError>
where
    T: Send + 'static,
    F: Fn(String) -> Fut + Send + Sync,
    Fut: std::future::Future<Output = Result<T, ApiError>> + Send,
{
    let auth = super::config::ApexDataAuth;
    crate::data::connectivity::auth::with_auth_retry(&auth, f).await
}

/// Run a blocking REST closure inside `with_auth_retry`. The closure receives
/// the current bearer (already injected by `with_auth_retry`) and is executed
/// via `spawn_blocking` so it doesn't stall the tokio reactor.
///
/// Callers that already have an `&tokio::Runtime` handle can use this directly;
/// the existing synchronous `get_*` functions remain available for non-async
/// contexts (chart background threads).
pub async fn with_auth_retry_blocking<T, F>(f: F) -> Result<T, ApiError>
where
    T: Send + 'static,
    F: Fn(String) -> Result<T, ApiError> + Send + Sync + Clone + 'static,
{
    with_apex_auth_retry(move |_token| {
        let f = f.clone();
        async move {
            // The current token is already read inside `with_auth_retry`; we
            // re-read inside the blocking closure to avoid a `Send` capture of
            // String across thread boundaries beyond what's necessary.
            let token = _token.clone();
            tokio::task::spawn_blocking(move || f(token))
                .await
                .map_err(|e| ApiError::Network(format!("spawn_blocking join: {e}")))?
        }
    })
    .await
}

// ── §5.3 bars ──────────────────────────────────────────────────────────────

/// `GET /api/bars/:class/:symbol/:tf[?source=last|mark]` — MARK_BARS_PROTOCOL §REST.
/// `source=last` is the default (trade-print bars). `source=mark` returns NBBO-mid bars
/// (volume=0). Stock callers should always pass `BarSource::Last`.
pub fn get_bars(class: AssetClass, symbol: &str, tf: &str, source: BarSource) -> Result<Vec<ChartBar>, ApiError> {
    // Omit ?source=last to keep URLs identical to pre-MARK behavior (back-compat).
    match source {
        BarSource::Last => get(&format!("/api/bars/{}/{}/{}", class.path(), symbol, tf)),
        BarSource::Mark => get(&format!("/api/bars/{}/{}/{}?source=mark", class.path(), symbol, tf)),
    }
}

/// `GET /api/replay/...[&source=last|mark]` — cursor-paginated QuestDB replay.
pub fn get_replay(class: AssetClass, symbol: &str, tf: &str, from_ms: i64, to_ms: i64, cursor: Option<i64>, limit: Option<u32>, source: BarSource) -> Result<ReplayResponse, ApiError> {
    let mut q = format!("from={from_ms}&to={to_ms}");
    if let Some(c) = cursor { q.push_str(&format!("&cursor={c}")); }
    if let Some(l) = limit  { q.push_str(&format!("&limit={l}")); }
    if matches!(source, BarSource::Mark) { q.push_str("&source=mark"); }
    get(&format!("/api/replay/{}/{}/{}?{q}", class.path(), symbol, tf))
}

// ── §5.2 snapshot / quote / price ──────────────────────────────────────────

pub fn get_snapshot(class: AssetClass, symbol: &str) -> Result<Snapshot, ApiError> {
    get(&format!("/api/snap/{}/{}", class.path(), symbol))
}

pub fn get_quote(symbol: &str) -> Result<Quote, ApiError> {
    get(&format!("/api/quote/{symbol}"))
}

pub fn get_all_quotes() -> Result<Vec<Quote>, ApiError> {
    get("/api/quote")
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct PriceResponse {
    pub symbol: String,
    pub asset_class: AssetClass,
    pub price: f64,
}

pub fn get_price(symbol: &str) -> Result<PriceResponse, ApiError> {
    get(&format!("/api/price/{symbol}"))
}

pub fn get_symbols() -> Result<SymbolsResponse, ApiError> {
    get("/api/symbols")
}

// ── §5.4 options ───────────────────────────────────────────────────────────

pub fn get_chain(underlying: &str) -> Result<ChainResponse, ApiError> {
    get_chain_with(underlying, &ChainQuery::default())
}

/// §5.4.c — query-parameterized chain fetch. Defaults: `dte_max=14`,
/// `strike_window_pct=10.0`. Pass `ChainQuery { all: true, .. }` to bypass.
pub fn get_chain_with(underlying: &str, q: &ChainQuery) -> Result<ChainResponse, ApiError> {
    let qs = q.to_query_string();
    get(&format!("/api/chain/{underlying}{qs}"))
}

pub fn get_greeks(contract: &str) -> Result<GreeksRow, ApiError> {
    get(&format!("/api/greeks/{contract}"))
}

pub fn get_indicators(class: AssetClass, symbol: &str, tf: &str) -> Result<IndicatorsResponse, ApiError> {
    get(&format!("/api/indicators/{}/{}/{}", class.path(), symbol, tf))
}

// ── §5.1 health / ops ──────────────────────────────────────────────────────

/// `GET /api/health/ready` — returns a `HealthReady` for both 200 and 503.
///
/// Special-cased because the 503 response carries a useful payload (the same
/// `HealthReady` schema as 200). Network / parse failures still bubble up
/// as `ApiError`.
pub fn get_health_ready() -> Result<HealthReady, ApiError> {
    let url = format!("{}/api/health/ready", apex_url());
    let mut req = client().get(&url);
    if let Some(tok) = apex_token() { req = req.bearer_auth(tok); }
    let resp = req.send().map_err(|e| ApiError::Network(e.to_string()))?;
    // Both 200 and 503 carry a HealthReady body
    resp.json::<HealthReady>().map_err(|e| ApiError::Parse(e.to_string()))
}

pub fn get_feeds() -> Result<FeedsResponse, ApiError> {
    get("/api/feeds")
}

// ── §5.5 fund holdings (ETF / index constituents) ────────────────────────

/// `GET /api/holdings/:ticker` — Polygon-backed ETF/index holdings.
/// Returns `(symbol, weight)` pairs in the order ApexData returns them
/// (typically weight-descending).
pub fn fetch_holdings(ticker: &str) -> Result<Vec<(String, Option<f32>)>, ApiError> {
    #[derive(serde::Deserialize)]
    struct HoldingRow {
        symbol: String,
        #[serde(default)]
        weight: Option<f32>,
    }
    #[derive(serde::Deserialize)]
    struct HoldingsResp {
        #[serde(default)]
        holdings: Vec<HoldingRow>,
        #[serde(default)]
        error: Option<String>,
    }
    let resp: HoldingsResp = get(&format!("/api/holdings/{ticker}"))?;
    if let Some(e) = resp.error.as_ref() {
        crate::apex_log!("rest.holdings", "{ticker} server error: {e}");
        return Err(ApiError::Http { status: 200, body: format!("server error: {e}") });
    }
    Ok(resp.holdings.into_iter().map(|h| (h.symbol, h.weight)).collect())
}

/// Liveness — text "ok". Returns true on HTTP 200.
pub fn is_live() -> bool {
    let url = format!("{}/api/health/live", apex_url());
    client().get(&url).send().map(|r| r.status().is_success()).unwrap_or(false)
}

// ── Async wrappers with auth-retry ─────────────────────────────────────────
//
// These variants wrap the blocking `get_*` functions inside `with_auth_retry`
// (via `spawn_blocking`). Use them from `async fn` contexts that have a tokio
// runtime — they will refresh + retry once on a 401, then surface the
// underlying `ApiError`. The synchronous `get_*` functions remain available
// for code paths still on `std::thread::spawn` workers.

/// `get_chain` with auth-retry. Spawn-blocking + one refresh on 401.
pub async fn get_chain_async(underlying: &str) -> Result<ChainResponse, ApiError> {
    let u = underlying.to_string();
    with_auth_retry_blocking(move |_tok| {
        let u = u.clone();
        get_chain(&u)
    })
    .await
}

/// `get_snapshot` with auth-retry.
pub async fn get_snapshot_async(class: AssetClass, symbol: &str) -> Result<Snapshot, ApiError> {
    let s = symbol.to_string();
    with_auth_retry_blocking(move |_tok| {
        let s = s.clone();
        get_snapshot(class, &s)
    })
    .await
}

/// `get_health_ready` with auth-retry.
pub async fn get_health_ready_async() -> Result<HealthReady, ApiError> {
    with_auth_retry_blocking(|_tok| get_health_ready()).await
}

/// `get_greeks` with auth-retry.
pub async fn get_greeks_async(contract: &str) -> Result<GreeksRow, ApiError> {
    let c = contract.to_string();
    with_auth_retry_blocking(move |_tok| {
        let c = c.clone();
        get_greeks(&c)
    })
    .await
}
