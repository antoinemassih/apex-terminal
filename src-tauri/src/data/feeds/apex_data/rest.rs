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
    // Security: log only the path, not the full URL (which may contain host + query secrets).
    crate::apex_log!("rest.req", "GET {path}");
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

/// POST helper that mirrors `get` — sends a JSON body, parses a JSON response,
/// and routes through the same breaker / stats pipeline. Returns `None` on any
/// non-2xx HTTP, network error, body-parse failure, or breaker-open shortcut.
///
/// Kept private; new endpoints add typed wrappers (see `start_replay`,
/// `replay_pause`, etc.) so callers never touch reqwest directly.
fn post_json<B: serde::Serialize, T: serde::de::DeserializeOwned>(path: &str, body: &B) -> Option<T> {
    if !is_enabled() {
        crate::apex_log!("rest.skip", "disabled: POST {path}");
        record(RestCall { path: format!("POST {path}"), status: 0, outcome: "skip", ms: 0, at: std::time::SystemTime::now() });
        return None;
    }
    if breaker_is_open() {
        crate::apex_log!("rest.skip", "breaker open: POST {path}");
        record(RestCall { path: format!("POST {path}"), status: 1, outcome: "skip", ms: 0, at: std::time::SystemTime::now() });
        return None;
    }
    let url = format!("{}{path}", apex_url());
    // Security: log only the path, not the full URL (which may contain host + query secrets).
    crate::apex_log!("rest.req", "POST {path}");
    let t0 = Instant::now();
    let mut req = client().post(&url).json(body);
    if let Some(tok) = apex_token() { req = req.bearer_auth(tok); }
    match req.send() {
        Ok(r) if r.status().is_success() => {
            let status = r.status();
            match r.json::<T>() {
                Ok(v) => {
                    crate::apex_log!("rest.ok", "POST {path} → {} ({:?})", status, t0.elapsed());
                    breaker_note_success();
                    record(RestCall { path: format!("POST {path}"), status: status.as_u16(), outcome: "ok", ms: t0.elapsed().as_millis(), at: std::time::SystemTime::now() });
                    Some(v)
                }
                Err(e) => {
                    crate::apex_log!("rest.parse", "POST {path} → {} body parse failed: {e}", status);
                    record(RestCall { path: format!("POST {path}"), status: status.as_u16(), outcome: "parse", ms: t0.elapsed().as_millis(), at: std::time::SystemTime::now() });
                    None
                }
            }
        }
        Ok(r) => {
            crate::apex_log!("rest.http", "POST {path} → {} ({:?})", r.status(), t0.elapsed());
            record(RestCall { path: format!("POST {path}"), status: r.status().as_u16(), outcome: "http", ms: t0.elapsed().as_millis(), at: std::time::SystemTime::now() });
            None
        }
        Err(e) => {
            crate::apex_log!("rest.err", "POST {path} network error ({:?}): {e}", t0.elapsed());
            breaker_note_failure();
            record(RestCall { path: format!("POST {path}"), status: 0, outcome: "err", ms: t0.elapsed().as_millis(), at: std::time::SystemTime::now() });
            None
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

/// Synchronous auth-retry: run `f` once; on 401/Auth error attempt a token
/// refresh (via a one-shot tokio runtime or the current handle if one exists),
/// then retry `f` exactly once more. Any other error propagates immediately.
///
/// This is the sync sibling of `with_auth_retry_blocking`. Use it for blocking
/// functions that cannot be made async without migrating every call site.
/// Token expiry is therefore caught and retried rather than silently failing.
fn with_auth_retry_sync<T, F>(f: F) -> Result<T, ApiError>
where
    F: Fn() -> Result<T, ApiError>,
{
    fn is_auth(e: &ApiError) -> bool {
        matches!(e, ApiError::Auth(_) | ApiError::Http { status: 401, .. })
    }

    match f() {
        Ok(v) => Ok(v),
        Err(e) if is_auth(&e) => {
            // Attempt token refresh. ApexDataAuth::refresh_token is currently a
            // no-op (server-side refresh not yet wired), but the call ensures we
            // will pick up an externally-updated token if the caller has rotated
            // it out-of-band (e.g. via set_apex_token). On RefreshFailed we
            // still retry once — a fresh read of apex_token() may succeed.
            let refresh_result: Result<String, _> = {
                let auth = super::config::ApexDataAuth;
                // Try to use an existing tokio handle; fall back to a one-shot runtime.
                match tokio::runtime::Handle::try_current() {
                    Ok(handle) => std::thread::spawn(move || {
                        handle.block_on(
                            crate::data::connectivity::auth::Authenticated::refresh_token(&auth)
                        )
                    })
                    .join()
                    .unwrap_or(Err(crate::data::connectivity::AuthError::RefreshFailed(
                        "join error".into(),
                    ))),
                    Err(_) => match tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                    {
                        Ok(rt) => rt.block_on(
                            crate::data::connectivity::auth::Authenticated::refresh_token(&auth)
                        ),
                        Err(build_err) => Err(crate::data::connectivity::AuthError::RefreshFailed(
                            build_err.to_string(),
                        )),
                    },
                }
            };
            if let Err(ref re) = refresh_result {
                crate::apex_log!("rest.auth", "token refresh failed: {re}; retrying once anyway");
            }
            // Always retry once — the token may have been refreshed externally
            // even if our refresh call failed.
            f()
        }
        Err(other) => Err(other),
    }
}

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
/// as `ApiError`. On 401 the call is retried once via `with_auth_retry_sync`
/// (token expiry should not silently block health checks forever).
pub fn get_health_ready() -> Result<HealthReady, ApiError> {
    with_auth_retry_sync(|| {
        let url = format!("{}/api/health/ready", apex_url());
        let mut req = client().get(&url);
        if let Some(tok) = apex_token() { req = req.bearer_auth(tok); }
        let resp = req.send().map_err(|e| ApiError::Network(e.to_string()))?;
        let status = resp.status();
        if status.is_success() || status.as_u16() == 503 {
            // Both 200 and 503 carry a HealthReady body — parse normally.
            resp.json::<HealthReady>().map_err(|e| ApiError::Parse(e.to_string()))
        } else if status.as_u16() == 401 {
            Err(ApiError::Auth(crate::data::connectivity::AuthError::TokenExpired))
        } else {
            let body = resp.text().unwrap_or_default();
            Err(ApiError::Http { status: status.as_u16(), body })
        }
    })
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

// ── §5.6 stocks bulk / movers / grouped ────────────────────────────────────

/// `GET /api/stocks/snap/bulk?tickers=AAPL,MSFT,...`
/// Returns the full Polygon snapshot envelope for each ticker. Empty list →
/// empty `Vec`; callers should *not* pass more than a few hundred tickers
/// (URL length limit at the gateway).
pub fn snap_bulk(tickers: &[String]) -> Option<Vec<StockSnapshot>> {
    if tickers.is_empty() { return Some(Vec::new()); }
    // Upper-case and dedup defensively — backend is strict about ticker shape.
    let mut seen = std::collections::HashSet::new();
    let joined: String = tickers.iter()
        .map(|s| s.to_uppercase())
        .filter(|s| seen.insert(s.clone()))
        .collect::<Vec<_>>()
        .join(",");
    // URL-encode the joined ticker list so commas/special chars are safe.
    let encoded = urlencoding::encode(&joined);
    get(&format!("/api/stocks/snap/bulk?tickers={encoded}")).ok()
}

/// One symbol-search hit from `GET /api/search?q=`. Polygon ticker-reference
/// shaped; only the fields the terminal's search box needs are parsed.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct SearchHit {
    pub ticker: String,
    #[serde(default)] pub name: String,
    /// "stocks", "crypto", "fx", "indices", "otc" — per Polygon.
    #[serde(default)] pub market: String,
    /// "CS", "ETF", "ADRC", … (instrument type). May be absent.
    #[serde(default, rename = "type")] pub kind: String,
    #[serde(default)] pub primary_exchange: String,
    #[serde(default = "default_true")] pub active: bool,
}

fn default_true() -> bool { true }

#[derive(Debug, Clone, serde::Deserialize)]
pub struct SearchResponse {
    #[serde(default)] pub results: Vec<SearchHit>,
}

/// `GET /api/search?q=<query>` — Polygon-backed symbol search (stocks, ETFs,
/// indices, crypto). Prefix + fuzzy match. This is the stock-capable search
/// that replaces the (options-only, frequently-down) ApexIB `/search/` path.
pub fn search(query: &str) -> Option<Vec<SearchHit>> {
    let q = query.trim();
    if q.is_empty() { return Some(Vec::new()); }
    let encoded = urlencoding::encode(q);
    let resp: SearchResponse = get(&format!("/api/search?q={encoded}")).ok()?;
    Some(resp.results)
}

/// `GET /api/stocks/movers?direction=gainers|losers` — Polygon top-20 movers.
/// `direction` must be "gainers" or "losers"; anything else is rejected by
/// the backend (and short-circuited here to `None`).
pub fn stocks_movers(direction: &str) -> Option<Vec<StockSnapshot>> {
    let dir = direction.trim().to_lowercase();
    if dir != "gainers" && dir != "losers" { return None; }
    get(&format!("/api/stocks/movers?direction={dir}")).ok()
}

/// `GET /api/stocks/grouped/:date` — full-market 1d grouped bars for the
/// given YYYY-MM-DD session date. Returns `None` if the backend has no data
/// (weekend / holiday / future date / not-yet-published).
pub fn stocks_grouped_daily(date: &str) -> Option<Vec<GroupedDailyBar>> {
    get(&format!("/api/stocks/grouped/{date}")).ok()
}

// ── Wave 10 projector outputs (sector rotation / breadth / movers) ─────────
//
// These mirror the new ApexData projector routes. The projectors are merged
// upstream but the HTTP routes may not be exposed yet on every deployment —
// the breaker + None return handles missing routes gracefully.
//
// TODO(wire-route): confirm `/api/stocks/sector_rotation`, `/api/stocks/breadth/:index`,
// and `/api/stocks/movers/:kind` are exposed in `apex-data`'s router crate.
// If not, a Redis bridge keyed `projector:sector_rotation` / `projector:breadth:<idx>` /
// `projector:movers:<kind>` may be the only access path until the routes ship.

/// `GET /api/stocks/sector_rotation` — 11 SPDR sector ETF RRG snapshot.
pub fn get_sector_rotation() -> Option<SectorRotationReading> {
    get("/api/stocks/sector_rotation").ok()
}

/// `GET /api/stocks/breadth/:index` — `index` ∈ {spx, ndx, compq, rut, us}.
pub fn get_breadth(index: &str) -> Option<BreadthReading> {
    let idx = index.trim().to_lowercase();
    if idx.is_empty() { return None; }
    get(&format!("/api/stocks/breadth/{idx}")).ok()
}

/// `GET /api/stocks/movers/:kind` — Wave 10 projector. Distinct from the
/// legacy `/api/stocks/movers?direction=` endpoint which only supports
/// gainers/losers; this one covers gainers/losers/active/rvol_leaders/gappers.
pub fn get_movers(kind: MoverKind) -> Option<MoversReading> {
    get(&format!("/api/stocks/movers/{}", kind.as_str())).ok()
}

// ── Wave 10 projector REST endpoints ─────────────────────────────────────

/// `GET /api/stocks/news/:ticker?since=<epoch_ms>` — cached `news_sentiment`
/// projector output. Backend may return `404` when no articles cached yet;
/// we surface that as `None` so callers can fall back to empty state.
pub fn get_news(ticker: &str, since_ms: Option<i64>) -> Option<NewsResponse> {
    let path = match since_ms {
        Some(s) => format!("/api/stocks/news/{ticker}?since={s}"),
        None    => format!("/api/stocks/news/{ticker}"),
    };
    get(&path).ok()
}

/// `GET /api/stocks/iv_rank/:underlying?lookback=<sessions>` — projector
/// reading from the `vol_surface` migration. Defaults to 252-session lookback
/// when `lookback` is `None`.
pub fn get_iv_rank(underlying: &str, lookback: Option<u32>) -> Option<IvRankV2> {
    let path = match lookback {
        Some(l) => format!("/api/stocks/iv_rank/{underlying}?lookback={l}"),
        None    => format!("/api/stocks/iv_rank/{underlying}"),
    };
    get(&path).ok()
}

/// `GET /api/stocks/iiv/:etf` — `etf_iiv` projector reading (market vs NAV).
pub fn get_etf_iiv(etf: &str) -> Option<EtfIivReading> {
    get(&format!("/api/stocks/iiv/{etf}")).ok()
}

/// `GET /api/stocks/corporate_actions/:ticker` — `corporate_actions`
/// projector reading (halts, dividends, splits, earnings).
pub fn get_corp_actions(ticker: &str) -> Option<CorporateActionsReading> {
    get(&format!("/api/stocks/corporate_actions/{ticker}")).ok()
}

/// Liveness — text "ok". Returns `true` on HTTP 200, `false` on any error.
///
/// Non-critical (health probe only). Internally uses `with_auth_retry_sync` so
/// a 401 caused by token expiry triggers one refresh attempt + retry before
/// giving up — token expiry must not cause permanent `false` results silently.
pub fn is_live() -> bool {
    let result: Result<bool, ApiError> = with_auth_retry_sync(|| {
        let url = format!("{}/api/health/live", apex_url());
        let resp = client().get(&url).send().map_err(|e| ApiError::Network(e.to_string()))?;
        let status = resp.status();
        if status.as_u16() == 401 {
            Err(ApiError::Auth(crate::data::connectivity::AuthError::TokenExpired))
        } else {
            Ok(status.is_success())
        }
    });
    result.unwrap_or(false)
}

// ── SOTA UX §3.1 — Provenance ──────────────────────────────────────────────

/// Outcome of a `get_provenance` call. We distinguish the 404 "aged out"
/// case from a network/parse error so the ProvenancePane can show the
/// spec-mandated message: "Provenance log doesn't have this entry. It
/// may have aged out of the 24h hot window."
#[derive(Debug)]
pub enum ProvenanceError {
    /// Lineage not found. Server returned 404. Per spec §4.1, render the
    /// "aged out of the 24h hot window" message.
    NotFound,
    /// Disabled, breaker open, network failure, parse error, or any other
    /// non-200/non-404 outcome. Carries a human-readable cause.
    Other(String),
}

impl std::fmt::Display for ProvenanceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound => write!(
                f,
                "Provenance log doesn't have this entry. It may have aged \
                 out of the 24h hot window."
            ),
            Self::Other(s) => write!(f, "{s}"),
        }
    }
}

impl std::error::Error for ProvenanceError {}

/// `GET /api/provenance/:lineage_id?format=tree|dag&depth=N`
///
/// Per SOTA §3.1 + §4.1. Blocking — call from a background thread.
/// `depth` is clamped to `1..=10`. Returns `NotFound` for 404 (caller
/// renders the aged-out message); `Other` for any other failure.
///
/// Note: the public name is `get_provenance` per the spec's wiring
/// summary (§5). The Result is sync-friendly since the REST client here
/// is `reqwest::blocking`; callers wrap it in `std::thread::spawn` so the
/// render thread never blocks.
pub fn get_provenance(
    lineage_id: &str,
    depth: u8,
    mode: ProvenanceMode,
) -> Result<ProvenanceTreeNode, ProvenanceError> {
    if !is_enabled() {
        return Err(ProvenanceError::Other("apex_data disabled".into()));
    }
    if breaker_is_open() {
        return Err(ProvenanceError::Other("rest breaker open".into()));
    }
    let d = depth.clamp(1, 10);
    let lineage_id = lineage_id.to_string();
    let mode_str = mode.as_str().to_string();

    // Internal helper: one attempt, returns ApiError on 401 so with_auth_retry_sync
    // can catch it and retry after a token refresh. NotFound (404) is surfaced
    // separately so callers render the "aged out of 24h hot window" message.
    let inner = move || -> Result<ProvenanceTreeNode, ApiError> {
        let path = format!("/api/provenance/{lineage_id}?format={mode_str}&depth={d}");
        let url = format!("{}{path}", apex_url());
        crate::apex_log!("rest.req", "GET {url}");
        let t0 = Instant::now();
        let mut req = client().get(&url);
        if let Some(tok) = apex_token() { req = req.bearer_auth(tok); }
        match req.send() {
            Ok(r) if r.status() == reqwest::StatusCode::NOT_FOUND => {
                record(RestCall {
                    path: path.clone(),
                    status: 404,
                    outcome: "http",
                    ms: t0.elapsed().as_millis(),
                    at: std::time::SystemTime::now(),
                });
                // Sentinel: map 404 through a recognizable Http error so the outer
                // converter can translate it to ProvenanceError::NotFound.
                Err(ApiError::Http { status: 404, body: String::new() })
            }
            Ok(r) if r.status().is_success() => {
                let status = r.status().as_u16();
                match r.json::<ProvenanceTreeNode>() {
                    Ok(node) => {
                        breaker_note_success();
                        record(RestCall {
                            path,
                            status,
                            outcome: "ok",
                            ms: t0.elapsed().as_millis(),
                            at: std::time::SystemTime::now(),
                        });
                        Ok(node)
                    }
                    Err(e) => {
                        record(RestCall {
                            path,
                            status,
                            outcome: "parse",
                            ms: t0.elapsed().as_millis(),
                            at: std::time::SystemTime::now(),
                        });
                        Err(ApiError::Parse(e.to_string()))
                    }
                }
            }
            Ok(r) => {
                let status = r.status().as_u16();
                record(RestCall {
                    path,
                    status,
                    outcome: "http",
                    ms: t0.elapsed().as_millis(),
                    at: std::time::SystemTime::now(),
                });
                if status == 401 {
                    Err(ApiError::Auth(crate::data::connectivity::AuthError::TokenExpired))
                } else {
                    Err(ApiError::Http { status, body: String::new() })
                }
            }
            Err(e) => {
                breaker_note_failure();
                record(RestCall {
                    path,
                    status: 0,
                    outcome: "err",
                    ms: t0.elapsed().as_millis(),
                    at: std::time::SystemTime::now(),
                });
                Err(ApiError::Network(e.to_string()))
            }
        }
    };

    // Run through auth-retry so a 401 triggers refresh + one retry before failing.
    match with_auth_retry_sync(inner) {
        Ok(node) => Ok(node),
        Err(ApiError::Http { status: 404, .. }) => Err(ProvenanceError::NotFound),
        Err(ApiError::Auth(_)) => Err(ProvenanceError::Other(
            "authentication failed (token expired or missing)".into(),
        )),
        Err(e) => Err(ProvenanceError::Other(e.to_string())),
    }
}

// ── §3.1 / SOTA §4.2 — replay session control ──────────────────────────────

/// `POST /api/replay/start` — kick off a new replay session.
/// On success returns the server-assigned `replay_id` (and any extra metadata).
/// Pair with `crate::apex_data::ws::subscribe_replay(replay_id, ...)` to start
/// receiving frames.
pub fn start_replay(cfg: &ReplayConfig) -> Option<ReplayStartResponse> {
    // Use a borrow-friendly wire struct so we can serialize the enum mode
    // with its snake_case string form without changing ReplayConfig's
    // serde plumbing.
    #[derive(serde::Serialize)]
    struct Wire<'a> {
        from_ts: i64,
        to_ts: i64,
        symbols: &'a [String],
        speed_multiplier: f32,
        asset_class: AssetClass,
        source: &'a str,
        mode: &'a str,
    }
    let body = Wire {
        from_ts: cfg.from_ts,
        to_ts: cfg.to_ts,
        symbols: &cfg.symbols,
        speed_multiplier: cfg.speed_multiplier,
        asset_class: cfg.asset_class,
        source: &cfg.source,
        mode: cfg.mode.as_str(),
    };
    post_json("/api/replay/start", &body)
}

/// `GET /api/replay/:id/status` — current frames-emitted / progress / state.
/// Returns `None` only on transport/HTTP failure; a 200 with a `Failed`
/// `state` still comes back as `Some(...)` so the UI can render the error.
pub fn replay_status(id: &str) -> Option<ReplayStatus> {
    get(&format!("/api/replay/{id}/status")).ok()
}

/// `POST /api/replay/:id/pause` — server returns `{ ok, state }`.
/// Only the new `state` is surfaced; the `ok` flag is implicit in `Some(_)`.
pub fn replay_pause(id: &str) -> Option<ReplayState> {
    replay_control(id, "pause")
}

/// `POST /api/replay/:id/resume`.
pub fn replay_resume(id: &str) -> Option<ReplayState> {
    replay_control(id, "resume")
}

/// `POST /api/replay/:id/stop`.
pub fn replay_stop(id: &str) -> Option<ReplayState> {
    replay_control(id, "stop")
}

/// Internal: the three control endpoints share the same `{ ok, state }`
/// response shape, so we collapse them through one helper.
fn replay_control(id: &str, action: &str) -> Option<ReplayState> {
    #[derive(serde::Deserialize)]
    struct Ctrl {
        #[serde(default)] ok: bool,
        #[serde(default)] state: ReplayState,
    }
    // Body is empty — the action lives in the URL path.
    let resp: Ctrl = post_json(&format!("/api/replay/{id}/{action}"), &serde_json::json!({}))?;
    if !resp.ok { crate::apex_log!("replay.ctrl", "{id}/{action} returned ok=false"); }
    Some(resp.state)
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

// ── P1.8 tests — auth-retry coverage for is_live / get_provenance / get_health_ready ──
//
// Tests use mockito to stand up a local HTTP server and repoint the apex URL
// at it via `set_apex_url`. `serial_test` prevents URL-override races between
// parallel test runners (the config is process-global).

#[cfg(test)]
mod p1_8_auth_retry_tests {
    use super::*;
    use mockito::Server;
    use serial_test::serial;

    /// Helper: override the apex URL and clear the breaker so prior test state
    /// doesn't bleed through.
    fn setup(url: &str) {
        super::super::config::set_apex_url(url);
        super::super::config::set_enabled(true);
        super::reset_breaker();
    }

    // ── is_live ───────────────────────────────────────────────────────────────

    /// `is_live` must return `false` when the server keeps returning 401 even
    /// after the (no-op) token refresh. This proves that the auth-retry wrapper
    /// is engaged and propagates the final auth failure as `false` rather than
    /// panicking or hanging.
    #[test]
    #[serial]
    fn is_live_returns_false_on_persistent_auth_failure() {
        let mut server = Server::new();
        // Always 401 — simulates a token that can't be refreshed.
        let _m = server
            .mock("GET", "/api/health/live")
            .with_status(401)
            .with_body("Unauthorized")
            .expect(2) // first attempt + one retry
            .create();

        setup(&server.url());
        // Token must be set so the bearer header is sent (otherwise the server
        // may return 200 on the no-auth path in some deployments).
        super::super::config::set_apex_token(Some("stale-token".into()));

        let result = is_live();
        assert!(!result, "is_live should return false on persistent 401");

        _m.assert();
    }

    /// Happy path: 200 → `true`.
    #[test]
    #[serial]
    fn is_live_returns_true_on_200() {
        let mut server = Server::new();
        let _m = server
            .mock("GET", "/api/health/live")
            .with_status(200)
            .with_body("ok")
            .create();

        setup(&server.url());
        super::super::config::set_apex_token(None);

        assert!(is_live());
        _m.assert();
    }

    // ── get_provenance ────────────────────────────────────────────────────────

    /// First call returns 401; after the (no-op) refresh the second call
    /// returns 200 with a valid `ProvenanceTreeNode`. The final result should
    /// be `Ok(node)`.
    #[test]
    #[serial]
    fn get_provenance_retries_on_401_then_succeeds() {
        let mut server = Server::new();
        // First call: 401
        let _m401 = server
            .mock("GET", mockito::Matcher::Regex(r"^/api/provenance/".to_string()))
            .with_status(401)
            .with_body("Unauthorized")
            .expect(1)
            .create();
        // Second call (after retry): 200 with a minimal node
        let node_json = r#"{"lineage_id":"abc-123","source_engine":"test","symbol":"AAPL","t_ms":0,"kind":"signal","children":[]}"#;
        let _m200 = server
            .mock("GET", mockito::Matcher::Regex(r"^/api/provenance/".to_string()))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(node_json)
            .expect(1)
            .create();

        setup(&server.url());
        super::super::config::set_apex_token(Some("stale-token".into()));

        let result = get_provenance("abc-123", 1, ProvenanceMode::Tree);
        assert!(result.is_ok(), "Expected Ok after retry: {:?}", result);
        let node = result.unwrap();
        assert_eq!(node.lineage_id, "abc-123");

        _m401.assert();
        _m200.assert();
    }

    /// 404 → `ProvenanceError::NotFound` (aged-out message).
    #[test]
    #[serial]
    fn get_provenance_404_returns_not_found() {
        let mut server = Server::new();
        let _m = server
            .mock("GET", mockito::Matcher::Regex(r"^/api/provenance/".to_string()))
            .with_status(404)
            .with_body("Not Found")
            .create();

        setup(&server.url());
        super::super::config::set_apex_token(None);

        let result = get_provenance("gone-id", 1, ProvenanceMode::Tree);
        assert!(
            matches!(result, Err(ProvenanceError::NotFound)),
            "Expected NotFound, got {:?}",
            result
        );
        _m.assert();
    }

    // ── get_health_ready ──────────────────────────────────────────────────────

    /// 503 with a `HealthReady` body → `Ok(HealthReady { ready: false, .. })`.
    /// This verifies that the 503 special-case survives the auth-retry refactor.
    #[test]
    #[serial]
    fn get_health_ready_503_returns_not_ready() {
        let mut server = Server::new();
        let body = r#"{"ready":false,"tick_age_ms":0,"tick_fresh":false,"redis":false,"questdb":false,"feeds_connected":0,"feeds_total":0}"#;
        let _m = server
            .mock("GET", "/api/health/ready")
            .with_status(503)
            .with_header("content-type", "application/json")
            .with_body(body)
            .create();

        setup(&server.url());
        super::super::config::set_apex_token(None);

        let result = get_health_ready();
        assert!(result.is_ok(), "Expected Ok for 503 path: {:?}", result);
        assert!(!result.unwrap().ready);
        _m.assert();
    }

    /// First call returns 401; after refresh the second call returns 200 with
    /// a `HealthReady { ready: true }`. Verifies auth-retry is wired.
    #[test]
    #[serial]
    fn get_health_ready_401_retries_via_auth() {
        let mut server = Server::new();
        let _m401 = server
            .mock("GET", "/api/health/ready")
            .with_status(401)
            .with_body("Unauthorized")
            .expect(1)
            .create();
        let body = r#"{"ready":true,"tick_age_ms":5,"tick_fresh":true,"redis":true,"questdb":true,"feeds_connected":2,"feeds_total":2}"#;
        let _m200 = server
            .mock("GET", "/api/health/ready")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(body)
            .expect(1)
            .create();

        setup(&server.url());
        super::super::config::set_apex_token(Some("stale-token".into()));

        let result = get_health_ready();
        assert!(result.is_ok(), "Expected Ok after retry: {:?}", result);
        assert!(result.unwrap().ready);

        _m401.assert();
        _m200.assert();
    }
}
