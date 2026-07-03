//! Single source of truth for wall-clock epoch time (audit D1).
//!
//! There were 11 hand-rolled `now_ms()` / `epoch_ms()` copies scattered across
//! feeds, panels, and the trading manager — split between `-> i64` and `-> u64`
//! return types, which invites sign/overflow bugs at call boundaries. This
//! module owns the one clock read; the former local functions are now thin
//! wrappers over these so no call site had to change.
//!
//! Both accessors read the same monotonic-ish system clock; the u64 variant
//! exists only because the trading/journal path stores `ts_ms: u64`.

use std::time::{SystemTime, UNIX_EPOCH};

/// Milliseconds since the Unix epoch as `i64` (the majority type — feeds,
/// panels, drawings). Saturates to 0 before 1970 (clock skew) rather than
/// panicking.
#[inline]
pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Milliseconds since the Unix epoch as `u64` — used by the trading/journal
/// path whose `ts_ms` fields are `u64`. Same clock read as [`now_ms`].
#[inline]
pub fn now_ms_u64() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
