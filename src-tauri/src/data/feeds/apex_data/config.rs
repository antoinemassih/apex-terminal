//! Runtime configuration for the ApexData client.
//!
//! Precedence (highest → lowest):
//!   1. Runtime override via `set_apex_url` / `set_apex_token`
//!   2. Env vars `APEX_DATA_URL` / `APEX_DATA_TOKEN`
//!   3. Compiled defaults (prod URL)

use std::sync::{RwLock, OnceLock};

// Cut over to the ApexDatav2 parallel stack (2026-06-14): apex-data-v2-dev
// serves the same shared QuestDB/Redis via the v2 gateway (full apex-data
// binary) PLUS the unified /ws/v2, /api/instrument, /api/replay and
// /api/health/data surfaces. The host-match ingress routes every path
// (/ws, /ws/dom, /ws/futures, /api/bars, /api/chain, …) to the gateway.
// Revert to `http://apex-data-dev.xllio.com` via Settings → Trading → APEX DATA
// or the `APEX_DATA_URL` env if you need the legacy stack.
const DEFAULT_URL: &str = "http://apex-data-v2-dev.xllio.com";

/// LAN IP of the K3s Traefik ingress. When set, reqwest resolves the
/// apex-data hostname to this IP directly, bypassing public DNS (which
/// returns the homelab's public IP that isn't routable from the LAN due to
/// missing split-horizon DNS). Set to `None` to use normal DNS.
///
/// Override via `APEX_DATA_LAN_IP` env or `set_apex_lan_ip()`.
///
/// ⚠ A LIST, not a single host. Every K3s node runs `svclb-traefik`, so any of
/// them can serve the ingress — and a single hard-coded IP put one machine in
/// front of a money path. When it stops answering, the chain fetch fails and
/// `fetch.rs` falls back to a SYNTHESIZED Black-Scholes chain, i.e. the axis
/// pills quietly switch from real NBBO to fabricated bids/asks. Failing over to
/// another node is strictly better than failing over to invented prices.
///
/// Verified serving the ingress on 2026-08-01 (Host header + `/api/chain/SPY`
/// → HTTP 200). 192.168.1.79 is deliberately EXCLUDED — it was not answering.
const DEFAULT_LAN_IPS: &[&str] = &[
    "192.168.1.71",
    "192.168.1.177",
    "192.168.1.55",
    "192.168.1.56",
    "192.168.1.80",
];

static URL:     OnceLock<RwLock<String>>          = OnceLock::new();
static TOKEN:   OnceLock<RwLock<Option<String>>>  = OnceLock::new();
static ENABLED: OnceLock<RwLock<bool>>            = OnceLock::new();
static LAN_IP:  OnceLock<RwLock<Option<String>>>  = OnceLock::new();

fn url_cell()     -> &'static RwLock<String>         { URL.get_or_init(|| RwLock::new(load_initial_url())) }
fn token_cell()   -> &'static RwLock<Option<String>> { TOKEN.get_or_init(|| RwLock::new(load_initial_token())) }
fn enabled_cell() -> &'static RwLock<bool>           { ENABLED.get_or_init(|| RwLock::new(load_initial_enabled())) }
fn lan_ip_cell()  -> &'static RwLock<Option<String>> { LAN_IP.get_or_init(|| RwLock::new(load_initial_lan_ip())) }

fn load_initial_lan_ip() -> Option<String> {
    // `APEX_DATA_LAN_IP` accepts a comma-separated list; a single value keeps
    // working exactly as before.
    std::env::var("APEX_DATA_LAN_IP").ok().filter(|s| !s.is_empty())
        .or_else(|| Some(DEFAULT_LAN_IPS.join(",")))
}

fn load_initial_url() -> String {
    std::env::var("APEX_DATA_URL").unwrap_or_else(|_| DEFAULT_URL.into())
}
fn load_initial_token() -> Option<String> {
    std::env::var("APEX_DATA_TOKEN").ok().filter(|s| !s.is_empty())
}
fn load_initial_enabled() -> bool {
    std::env::var("APEX_DATA_ENABLED").map(|v| v == "1" || v.eq_ignore_ascii_case("true")).unwrap_or(true)
}

pub fn apex_url() -> String {
    url_cell().read().map(|g| g.clone()).unwrap_or_else(|_| DEFAULT_URL.into())
}

pub fn apex_ws_url() -> String {
    let base = apex_url();
    let ws = if let Some(rest) = base.strip_prefix("https://") {
        format!("wss://{rest}")
    } else if let Some(rest) = base.strip_prefix("http://") {
        format!("ws://{rest}")
    } else {
        format!("ws://{base}")
    };
    format!("{ws}/ws?format=json")
}

pub fn apex_token() -> Option<String> {
    token_cell().read().ok().and_then(|g| g.clone())
}

pub fn is_enabled() -> bool {
    enabled_cell().read().map(|g| *g).unwrap_or(true)
}

pub fn set_apex_url(url: impl Into<String>) {
    if let Ok(mut g) = url_cell().write() { *g = url.into(); }
}
pub fn set_apex_token(token: Option<String>) {
    if let Ok(mut g) = token_cell().write() { *g = token.filter(|s| !s.is_empty()); }
}
pub fn set_enabled(on: bool) {
    if let Ok(mut g) = enabled_cell().write() { *g = on; }
}

/// First configured LAN IP. For call sites that can only take one address
/// (the WS connect path) and for diagnostics display.
pub fn apex_lan_ip() -> Option<String> {
    apex_lan_ips().into_iter().next()
}

/// Every configured LAN IP, in preference order.
///
/// The REST client hands the whole list to reqwest's `resolve_to_addrs`, which
/// tries them in turn — so losing one node degrades to a slower connect rather
/// than to fabricated option prices.
pub fn apex_lan_ips() -> Vec<String> {
    lan_ip_cell().read().ok()
        .and_then(|g| g.clone())
        .map(|raw| parse_lan_ips(&raw))
        .unwrap_or_default()
}

/// Split a configured LAN-IP string into candidates, in preference order.
///
/// Pure so it can be tested without touching the process-wide config cell —
/// two tests mutating that global raced under the parallel test runner.
fn parse_lan_ips(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}
pub fn set_apex_lan_ip(ip: Option<String>) {
    if let Ok(mut g) = lan_ip_cell().write() { *g = ip.filter(|s| !s.is_empty()); }
}

/// Parse the host[:port] out of the configured base URL. Used by the REST
/// and WS layers to bind the LAN-IP override only to this host.
/// Wave 1: typed `Authenticated` adapter over the free-function token store.
///
/// ApexData does NOT currently expose a `/auth/refresh` endpoint, so
/// `refresh_token()` returns `AuthError::RefreshFailed("not supported")`.
/// Callers using `with_auth_retry` will see the original 401 propagate after
/// one no-op refresh attempt — same effective behavior as today, but the
/// typed surface is now in place so a server-side refresh flow can be wired
/// without touching every call site.
///
/// TODO(wave-2): wire to the real refresh endpoint once it lands in ApexData.
pub struct ApexDataAuth;

#[async_trait::async_trait]
impl crate::data::connectivity::Authenticated for ApexDataAuth {
    async fn refresh_token(&self) -> Result<String, crate::data::connectivity::AuthError> {
        Err(crate::data::connectivity::AuthError::RefreshFailed(
            "apex_data: refresh endpoint not implemented server-side".into(),
        ))
    }
    fn current_token(&self) -> Option<String> {
        apex_token()
    }
}

/// Redis URL for the bar cache. Reads `APEX_REDIS_URL` env var.
///
/// The bar cache is OPTIONAL. If the env var is unset, this warns once and
/// returns a localhost sentinel that fails gracefully at connection time —
/// it must never brick startup, and no homelab address is baked into the
/// binary. Set `APEX_REDIS_URL` to point at a real Redis.
///
/// Never log the returned value — it may carry a password.
pub fn apex_redis_url() -> String {
    match std::env::var("APEX_REDIS_URL").ok().filter(|s| !s.is_empty()) {
        Some(url) => url,
        None => {
            // F2: route to the unified sink so the misconfig shows as a
            // persistent in-app indicator (+ Prometheus count), not one-time
            // stderr. Stable `code` dedups across repeat calls.
            crate::data::connectivity::errors_sink::report(
                crate::data::connectivity::errors_sink::ErrorLevel::Warn,
                "config", "redis_url_unset",
                "APEX_REDIS_URL not set — bar cache disabled. \
                 Set it (e.g. redis://:password@host:6379/) to enable.",
            );
            "redis://127.0.0.1:6379/".to_string()
        }
    }
}

/// PostgreSQL URL for the drawings / watchlist DB. Reads `APEX_PG_URL` env var.
///
/// DB persistence is OPTIONAL — local JSON is the fallback. If the env var is
/// unset, this warns once and returns a localhost sentinel that fails
/// gracefully at connection time; it must never brick startup, and no homelab
/// address is baked into the binary. Set `APEX_PG_URL` for a real database.
///
/// Never log the returned value — it may carry a password.
pub fn apex_pg_url() -> String {
    match std::env::var("APEX_PG_URL").ok().filter(|s| !s.is_empty()) {
        Some(url) => url,
        None => {
            // F2: persistent in-app indicator (+ Prometheus count) instead of
            // one-time stderr. Stable `code` dedups across repeat calls.
            crate::data::connectivity::errors_sink::report(
                crate::data::connectivity::errors_sink::ErrorLevel::Warn,
                "config", "pg_url_unset",
                "APEX_PG_URL not set — Postgres persistence disabled \
                 (local JSON fallback). Set it (e.g. postgresql://user:pw@host:5432/db).",
            );
            "postgresql://postgres@127.0.0.1:5432/apex".to_string()
        }
    }
}

pub fn apex_host_port() -> Option<(String, u16)> {
    let url = apex_url();
    let rest = url.strip_prefix("http://").or_else(|| url.strip_prefix("https://")).unwrap_or(&url);
    let host_port = rest.split('/').next().unwrap_or("");
    let (host, port) = match host_port.rsplit_once(':') {
        Some((h, p)) => (h.to_string(), p.parse().unwrap_or(80)),
        None => (host_port.to_string(), if url.starts_with("https://") { 443 } else { 80 }),
    };
    if host.is_empty() { None } else { Some((host, port)) }
}

#[cfg(test)]
mod lan_failover_tests {
    use super::*;

    #[test]
    fn the_default_is_more_than_one_node() {
        // The regression this guards: a single hard-coded IP in front of the
        // chain fetch. When it stops answering, the terminal does not show an
        // error — it shows SYNTHESIZED option prices. More than one candidate
        // is the difference between degrading and fabricating.
        assert!(DEFAULT_LAN_IPS.len() >= 2,
            "one LAN IP means one machine can silently turn the pills synthetic");
    }

    #[test]
    fn every_default_is_a_parseable_address() {
        for ip in DEFAULT_LAN_IPS {
            assert!(ip.parse::<std::net::IpAddr>().is_ok(), "unparseable default LAN IP: {ip}");
        }
    }

    #[test]
    fn the_known_dead_node_is_not_a_candidate() {
        // 192.168.1.79 did not answer the ingress on 2026-08-01. Listing a node
        // that never responds just buys a connect timeout on every request.
        assert!(!DEFAULT_LAN_IPS.contains(&"192.168.1.79"),
            "192.168.1.79 was not serving the ingress — do not make requests wait on it");
    }

    #[test]
    fn a_comma_separated_override_parses_into_candidates() {
        assert_eq!(parse_lan_ips("10.0.0.1, 10.0.0.2 ,10.0.0.3"),
                   vec!["10.0.0.1", "10.0.0.2", "10.0.0.3"]);
    }

    #[test]
    fn a_single_value_override_still_works_unchanged() {
        // Backwards compatibility: APEX_DATA_LAN_IP was a single IP before.
        assert_eq!(parse_lan_ips("10.9.9.9"), vec!["10.9.9.9"]);
    }

    #[test]
    fn an_empty_or_ragged_override_yields_no_bogus_candidates() {
        // An empty entry would become an unparseable "address" that costs a
        // failed resolve on every request.
        assert!(parse_lan_ips("").is_empty());
        assert!(parse_lan_ips("  ").is_empty());
        assert_eq!(parse_lan_ips("10.0.0.1,,  ,10.0.0.2"), vec!["10.0.0.1", "10.0.0.2"]);
    }

    #[test]
    fn the_packed_default_round_trips_back_to_the_candidate_list() {
        // load_initial_lan_ip() joins DEFAULT_LAN_IPS with commas; parsing it
        // back must reproduce the list exactly, or the default silently
        // degrades to one node again.
        assert_eq!(parse_lan_ips(&DEFAULT_LAN_IPS.join(",")), DEFAULT_LAN_IPS.to_vec());
    }
}
