//! Blocking REST client for ApexData.
//!
//! All calls are blocking so they can be invoked from background threads
//! (`std::thread::spawn`). The caller is responsible for not blocking the
//! render thread — spawn a thread and deliver results via a channel.

use super::config::{apex_url, apex_token, is_enabled};
use super::types::*;
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

fn get<T: serde::de::DeserializeOwned>(path: &str) -> Option<T> {
    if !is_enabled() {
        crate::apex_log!("rest.skip", "disabled: {path}");
        record(RestCall { path: path.into(), status: 0, outcome: "skip", ms: 0, at: std::time::SystemTime::now() });
        return None;
    }
    if breaker_is_open() {
        crate::apex_log!("rest.skip", "breaker open: {path}");
        record(RestCall { path: path.into(), status: 1, outcome: "skip", ms: 0, at: std::time::SystemTime::now() });
        return None;
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
                    Some(v)
                }
                Err(e) => {
                    crate::apex_log!("rest.parse", "{path} → {} body parse failed: {e}", status);
                    record(RestCall { path: path.into(), status: status.as_u16(), outcome: "parse", ms: t0.elapsed().as_millis(), at: std::time::SystemTime::now() });
                    None
                }
            }
        }
        Ok(r) => {
            crate::apex_log!("rest.http", "{path} → {} ({:?})", r.status(), t0.elapsed());
            record(RestCall { path: path.into(), status: r.status().as_u16(), outcome: "http", ms: t0.elapsed().as_millis(), at: std::time::SystemTime::now() });
            None
        }
        Err(e) => {
            crate::apex_log!("rest.err", "{path} network error ({:?}): {e}", t0.elapsed());
            breaker_note_failure();
            record(RestCall { path: path.into(), status: 0, outcome: "err", ms: t0.elapsed().as_millis(), at: std::time::SystemTime::now() });
            None
        }
    }
}

// ── §5.3 bars ──────────────────────────────────────────────────────────────

/// `GET /api/bars/:class/:symbol/:tf[?source=last|mark]` — MARK_BARS_PROTOCOL §REST.
/// `source=last` is the default (trade-print bars). `source=mark` returns NBBO-mid bars
/// (volume=0). Stock callers should always pass `BarSource::Last`.
pub fn get_bars(class: AssetClass, symbol: &str, tf: &str, source: BarSource) -> Option<Vec<ChartBar>> {
    // Omit ?source=last to keep URLs identical to pre-MARK behavior (back-compat).
    match source {
        BarSource::Last => get(&format!("/api/bars/{}/{}/{}", class.path(), symbol, tf)),
        BarSource::Mark => get(&format!("/api/bars/{}/{}/{}?source=mark", class.path(), symbol, tf)),
    }
}

/// `GET /api/replay/...[&source=last|mark]` — cursor-paginated QuestDB replay.
pub fn get_replay(class: AssetClass, symbol: &str, tf: &str, from_ms: i64, to_ms: i64, cursor: Option<i64>, limit: Option<u32>, source: BarSource) -> Option<ReplayResponse> {
    let mut q = format!("from={from_ms}&to={to_ms}");
    if let Some(c) = cursor { q.push_str(&format!("&cursor={c}")); }
    if let Some(l) = limit  { q.push_str(&format!("&limit={l}")); }
    if matches!(source, BarSource::Mark) { q.push_str("&source=mark"); }
    get(&format!("/api/replay/{}/{}/{}?{q}", class.path(), symbol, tf))
}

// ── §5.2 snapshot / quote / price ──────────────────────────────────────────

pub fn get_snapshot(class: AssetClass, symbol: &str) -> Option<Snapshot> {
    get(&format!("/api/snap/{}/{}", class.path(), symbol))
}

pub fn get_quote(symbol: &str) -> Option<Quote> {
    get(&format!("/api/quote/{symbol}"))
}

pub fn get_all_quotes() -> Option<Vec<Quote>> {
    get("/api/quote")
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct PriceResponse {
    pub symbol: String,
    pub asset_class: AssetClass,
    pub price: f64,
}

pub fn get_price(symbol: &str) -> Option<PriceResponse> {
    get(&format!("/api/price/{symbol}"))
}

pub fn get_symbols() -> Option<SymbolsResponse> {
    get("/api/symbols")
}

// ── §5.4 options ───────────────────────────────────────────────────────────

pub fn get_chain(underlying: &str) -> Option<ChainResponse> {
    get_chain_with(underlying, &ChainQuery::default())
}

/// §5.4.c — query-parameterized chain fetch. Defaults: `dte_max=14`,
/// `strike_window_pct=10.0`. Pass `ChainQuery { all: true, .. }` to bypass.
pub fn get_chain_with(underlying: &str, q: &ChainQuery) -> Option<ChainResponse> {
    let qs = q.to_query_string();
    get(&format!("/api/chain/{underlying}{qs}"))
}

pub fn get_greeks(contract: &str) -> Option<GreeksRow> {
    get(&format!("/api/greeks/{contract}"))
}

pub fn get_indicators(class: AssetClass, symbol: &str, tf: &str) -> Option<IndicatorsResponse> {
    get(&format!("/api/indicators/{}/{}/{}", class.path(), symbol, tf))
}

// ── §5.1 health / ops ──────────────────────────────────────────────────────

/// `GET /api/health/ready` — returns a `HealthReady` for both 200 and 503.
pub fn get_health_ready() -> Option<HealthReady> {
    let url = format!("{}/api/health/ready", apex_url());
    let mut req = client().get(&url);
    if let Some(tok) = apex_token() { req = req.bearer_auth(tok); }
    let resp = req.send().ok()?;
    // Both 200 and 503 carry a HealthReady body
    resp.json::<HealthReady>().ok()
}

pub fn get_feeds() -> Option<FeedsResponse> {
    get("/api/feeds")
}

/// Liveness — text "ok". Returns true on HTTP 200.
pub fn is_live() -> bool {
    let url = format!("{}/api/health/live", apex_url());
    client().get(&url).send().map(|r| r.status().is_success()).unwrap_or(false)
}

