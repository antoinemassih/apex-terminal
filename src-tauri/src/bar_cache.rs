//! Redis-backed bar data cache.
//!
//! Stores OHLCV bars in Redis keyed by `apex:bars:{SYMBOL}:{timeframe}`.
//! TTL varies by timeframe: intraday data expires faster than daily.
//! Uses a persistent connection (Mutex-guarded) to avoid per-call overhead.
//! Falls back gracefully — if Redis is unreachable, all ops return None/Ok.

use crate::data::Bar;
use redis::{Client, Connection};
use std::sync::{Mutex, OnceLock};

static CONN: OnceLock<Mutex<Option<Connection>>> = OnceLock::new();

/// Initialize the Redis connection (call once at startup).
/// If Redis is unreachable, caching is silently disabled.
pub fn init() {
    CONN.get_or_init(|| {
        let conn = Client::open("redis://:monkeyxx@192.168.1.89:6379/")
            .ok()
            .and_then(|c| c.get_connection().ok());
        match &conn {
            Some(_) => eprintln!("[bar-cache] Redis connected at 192.168.1.89:6379"),
            None    => eprintln!("[bar-cache] Redis unreachable — caching disabled"),
        }
        Mutex::new(conn)
    });
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
    // Reconnect
    *guard = Client::open("redis://:monkeyxx@192.168.1.89:6379/")
        .ok()
        .and_then(|c| c.get_connection().ok());
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
