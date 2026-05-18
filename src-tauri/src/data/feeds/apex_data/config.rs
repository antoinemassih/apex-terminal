//! Runtime configuration for the ApexData client.
//!
//! Precedence (highest → lowest):
//!   1. Runtime override via `set_apex_url` / `set_apex_token`
//!   2. Env vars `APEX_DATA_URL` / `APEX_DATA_TOKEN`
//!   3. Compiled defaults (prod URL)

use std::sync::{RwLock, OnceLock};

// Dev deployment is live at apex-data-dev.xllio.com (K3s ingress). Prod URL
// (`http://apex-data.xllio.com`) is documented in the spec but not yet up.
// Override at runtime via Settings → Trading → APEX DATA or `APEX_DATA_URL` env.
const DEFAULT_URL: &str = "http://apex-data-dev.xllio.com";

/// LAN IP of the K3s Traefik ingress. When set, reqwest resolves the
/// apex-data hostname to this IP directly, bypassing public DNS (which
/// returns the homelab's public IP that isn't routable from the LAN due to
/// missing split-horizon DNS). Set to `None` to use normal DNS.
///
/// Override via `APEX_DATA_LAN_IP` env or `set_apex_lan_ip()`.
const DEFAULT_LAN_IP: Option<&str> = Some("192.168.1.71");

static URL:     OnceLock<RwLock<String>>          = OnceLock::new();
static TOKEN:   OnceLock<RwLock<Option<String>>>  = OnceLock::new();
static ENABLED: OnceLock<RwLock<bool>>            = OnceLock::new();
static LAN_IP:  OnceLock<RwLock<Option<String>>>  = OnceLock::new();

fn url_cell()     -> &'static RwLock<String>         { URL.get_or_init(|| RwLock::new(load_initial_url())) }
fn token_cell()   -> &'static RwLock<Option<String>> { TOKEN.get_or_init(|| RwLock::new(load_initial_token())) }
fn enabled_cell() -> &'static RwLock<bool>           { ENABLED.get_or_init(|| RwLock::new(load_initial_enabled())) }
fn lan_ip_cell()  -> &'static RwLock<Option<String>> { LAN_IP.get_or_init(|| RwLock::new(load_initial_lan_ip())) }

fn load_initial_lan_ip() -> Option<String> {
    std::env::var("APEX_DATA_LAN_IP").ok().filter(|s| !s.is_empty())
        .or_else(|| DEFAULT_LAN_IP.map(|s| s.to_string()))
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

pub fn apex_lan_ip() -> Option<String> {
    lan_ip_cell().read().ok().and_then(|g| g.clone())
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
/// If the env var is not set, logs a Critical error and returns a sentinel
/// URL that will fail at connection time. The homelab host/port fallback is
/// kept (without a password) so the failure message is actionable. Set
/// `APEX_REDIS_URL` in your environment to configure the credential.
///
/// Never log the returned value — it may carry a password.
pub fn apex_redis_url() -> String {
    match std::env::var("APEX_REDIS_URL").ok().filter(|s| !s.is_empty()) {
        Some(url) => url,
        None => {
            crate::data::connectivity::errors_sink::report(
                crate::data::connectivity::errors_sink::ErrorLevel::Critical,
                "config",
                "missing_credentials",
                "APEX_REDIS_URL not set; bar cache unavailable — set the env var with the Redis URL including credentials",
            );
            "redis://192.168.1.89:6379/".into()
        }
    }
}

/// PostgreSQL URL for the drawings / watchlist DB. Reads `APEX_PG_URL` env var.
///
/// If the env var is not set, logs a Critical error and returns a sentinel
/// URL that will fail at connection time. The homelab host/port fallback is
/// kept (without a password) so the failure message is actionable. Set
/// `APEX_PG_URL` in your environment to configure the credential.
///
/// Never log the returned value — it may carry a password.
pub fn apex_pg_url() -> String {
    match std::env::var("APEX_PG_URL").ok().filter(|s| !s.is_empty()) {
        Some(url) => url,
        None => {
            crate::data::connectivity::errors_sink::report(
                crate::data::connectivity::errors_sink::ErrorLevel::Critical,
                "config",
                "missing_credentials",
                "APEX_PG_URL not set; drawings/watchlist DB unavailable — set the env var with the PostgreSQL URL including credentials",
            );
            "postgresql://postgres@192.168.1.143:5432/ococo".into()
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
