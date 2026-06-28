//! Centralized base URLs for the non-apex-data backends.
//!
//! The crypto feed, OCOCO bar/annotation service, the yfinance sidecar, Yahoo,
//! and the legacy local ibserver used to have their homelab IPs hardcoded at
//! ~15 call sites across providers / fetch / foundation. This collects them so
//! the addresses aren't baked into the binary: each honours an env override and
//! falls back to the compiled default. Mirrors `apex_data/config.rs`'s
//! precedence, minus the runtime setter (these have no settings-UI surface).
//!
//! Each function returns a scheme+host[:port] **base** (no trailing slash, no
//! path); call sites append their own path/query.

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key)
        .ok()
        .map(|s| s.trim().trim_end_matches('/').to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| default.to_string())
}

/// ApexCrypto WebSocket URL (full ws URL incl. `/ws`). Override: `APEX_CRYPTO_WS`.
pub fn crypto_ws() -> String { env_or("APEX_CRYPTO_WS", "ws://192.168.1.56:30840/ws") }

/// ApexCrypto REST base. Override: `APEX_CRYPTO_HTTP`.
pub fn crypto_http() -> String { env_or("APEX_CRYPTO_HTTP", "http://192.168.1.56:30840") }

/// OCOCO bar/annotation service base. Override: `OCOCO_URL`.
pub fn ococo_http() -> String { env_or("OCOCO_URL", "http://192.168.1.60:30300") }

/// yfinance sidecar base. Override: `YFIN_SIDECAR_URL`.
pub fn yfin_sidecar() -> String { env_or("YFIN_SIDECAR_URL", "http://127.0.0.1:8777") }

/// Yahoo Finance chart API base. Override: `YAHOO_CHART_URL`.
pub fn yahoo_chart() -> String { env_or("YAHOO_CHART_URL", "https://query1.finance.yahoo.com") }

/// Legacy local ibserver WS URL (full ws URL incl. `/ws`). Override:
/// `APEX_IBSERVER_WS`. Dormant unless `APEX_ENABLE_LOCAL_IBSERVER` is set.
pub fn ibserver_ws() -> String { env_or("APEX_IBSERVER_WS", "ws://127.0.0.1:5000/ws") }
