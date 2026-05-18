//! Redis-backed bar data cache.
//!
//! Stores OHLCV bars in Redis keyed by `apex:bars:{SYMBOL}:{timeframe}`.
//! TTL varies by timeframe: intraday data expires faster than daily.
//! Uses a persistent connection (Mutex-guarded) to avoid per-call overhead.
//! Falls back gracefully — if Redis is unreachable, all ops return None/Ok.
//!
//! ## Configuration
//!
//! The Redis URL is supplied by the caller (see
//! `crate::data::apex_data::config::apex_redis_url`, which reads the
//! `APEX_REDIS_URL` env var). The URL carries a password — do NOT log it,
//! print it in error messages, or surface it to the UI.

use crate::data::Bar;
use crate::data::connectivity::errors_sink::{report, ErrorLevel};
use redis::{Client, Connection};
use std::sync::{Mutex, OnceLock};

static CONN: OnceLock<Mutex<Option<Connection>>> = OnceLock::new();
static REDIS_URL: OnceLock<String> = OnceLock::new();

fn open_connection() -> Option<Connection> {
    let url = REDIS_URL.get()?;
    Client::open(url.as_str()).ok().and_then(|c| c.get_connection().ok())
}

/// Initialize the Redis connection (call once at startup).
/// If Redis is unreachable, caching is silently disabled.
///
/// `url` is consumed once and stored for reconnects. Never echoed back in
/// any log line (it contains the password).
pub fn init(url: &str) {
    let _ = REDIS_URL.set(url.to_string());
    CONN.get_or_init(|| {
        let conn = open_connection();
        match &conn {
            // Strip auth segment from URL before logging — never echo password.
            Some(_) => {
                let endpoint = host_port_for_log(url).unwrap_or_else(|| "redis".into());
                report(ErrorLevel::Info, "bar_cache", "redis_connected", format!("Redis connected at {endpoint}"));
            }
            None => report(ErrorLevel::Warn, "bar_cache", "redis_unreachable", "Redis unreachable — caching disabled"),
        }
        Mutex::new(conn)
    });
}

/// Strip user:pass out of a `redis://[user[:pass]@]host[:port][/db]` URL so
/// the host:port can be logged without leaking credentials.
fn host_port_for_log(url: &str) -> Option<String> {
    let rest = url.strip_prefix("redis://").or_else(|| url.strip_prefix("rediss://"))?;
    let after_auth = rest.rsplit_once('@').map(|(_, h)| h).unwrap_or(rest);
    let host_port = after_auth.split('/').next()?;
    Some(host_port.to_string())
}

fn key(symbol: &str, timeframe: &str) -> String {
    format!("apex:bars:{}:{}", symbol.to_uppercase(), timeframe)
}

/// TTL in seconds based on timeframe.
fn ttl_secs(timeframe: &str) -> u64 {
    match timeframe {
        "1m" | "2m" => 120,   // 2 min
        "5m"        => 300,   // 5 min
        "15m"       => 600,   // 10 min
        "30m"       => 900,   // 15 min
        "1h"        => 1800,  // 30 min
        "4h"        => 3600,  // 1 hour
        "1d"        => 14400, // 4 hours
        "1wk"       => 43200, // 12 hours
        _           => 300,
    }
}

/// Acquire the persistent connection, reconnecting if it dropped.
fn with_conn<T>(f: impl Fn(&mut Connection) -> redis::RedisResult<T>) -> Option<T> {
    let lock = CONN.get()?;
    let mut guard = lock.lock().ok()?;
    // Try the operation; if it fails (broken pipe, timeout), attempt one reconnect.
    if let Some(conn) = guard.as_mut() {
        if let Ok(v) = f(conn) { return Some(v); }
    }
    // Reconnect using the URL passed to `init`.
    *guard = open_connection();
    let conn = guard.as_mut()?;
    f(conn).ok()
}

/// Check if Redis connection was established (non-blocking).
pub fn is_connected() -> bool {
    CONN.get()
        .and_then(|m| m.lock().ok())
        .map(|guard| guard.is_some())
        .unwrap_or(false)
}

/// Get cached bars. Returns None if cache miss or Redis unavailable.
pub fn get(symbol: &str, timeframe: &str) -> Option<Vec<Bar>> {
    let k = key(symbol, timeframe);
    let data: String = with_conn(|c| redis::cmd("GET").arg(&k).query(c))?;
    serde_json::from_str(&data).ok()
}

/// Cache bars with appropriate TTL. Silently ignores errors.
pub fn set(symbol: &str, timeframe: &str, bars: &[Bar]) {
    let Ok(json) = serde_json::to_string(bars) else { return; };
    let k = key(symbol, timeframe);
    let ttl = ttl_secs(timeframe);
    let _: Option<()> = with_conn(|c| redis::cmd("SETEX").arg(&k).arg(ttl).arg(&json).query(c));
}
