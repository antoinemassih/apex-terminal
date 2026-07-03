//! Shared HTTP clients (audit D2).
//!
//! Reqwest clients own a connection pool + TLS session cache and are explicitly
//! designed to be created ONCE and reused. The trading broker previously built
//! a fresh `reqwest::blocking::Client::new()` for every order op (submit /
//! cancel / modify / bracket / oco / …), paying a new TCP + TLS handshake each
//! time — measurable added latency on the live-money path under a burst.
//!
//! This module owns one pooled blocking client the whole app shares. Per-request
//! `.timeout(...)` calls still apply (the broker sets 5s per order call), so
//! behavior is unchanged apart from connection reuse.

use std::sync::OnceLock;
use std::time::Duration;

/// Process-wide pooled blocking HTTP client. No client-level timeout (matching
/// the previous per-op `Client::new()` behavior) — callers set per-request
/// timeouts. Falls back to a default client if the builder ever fails.
pub fn blocking_client() -> &'static reqwest::blocking::Client {
    static CLIENT: OnceLock<reqwest::blocking::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::blocking::Client::builder()
            .user_agent("apex-native")
            .pool_max_idle_per_host(8)
            .pool_idle_timeout(Duration::from_secs(90))
            .build()
            .unwrap_or_else(|_| reqwest::blocking::Client::new())
    })
}
