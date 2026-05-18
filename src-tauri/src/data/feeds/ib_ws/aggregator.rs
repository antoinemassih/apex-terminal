//! Wave 13b: time-bucket OHLC aggregator for IB tick stream.
//!
//! Wave 12a wired IB ticks into `bar_hub()` but emitted a degenerate per-tick
//! bar (O=H=L=C=tick, volume=tick_volume). This module replaces that with a
//! proper time-bucket aggregator that maintains a rolling OHLC bucket per
//! `(symbol, timeframe)` pair and emits a `closed` bar when a tick crosses
//! into a new bucket.
//!
//! Bucket math is currently hard-coded to 1-minute buckets
//! (`bucket_start_ms = (ts_ms / 60_000) * 60_000`). Other timeframes still
//! return a `BarBucket` with `timeframe` set to the requested string but
//! bucketed on a 1-minute boundary — see `bucket_start_for` for the
//! single point of customization when adding 5m/1h/etc. The aggregator is
//! intentionally not generic over timeframe yet because the IB tick path
//! only feeds the 1m series today; widening the API surface before there's
//! a second caller would be premature.
//!
//! All field widths use `f64` (matches `BarWire` on the wire) — the task
//! spec sketched `f32`, but every consumer downstream is already `f64`, so
//! using `f32` here would force a cast on every fanout.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

/// A single OHLC bucket for one `(symbol, timeframe)` slot.
#[derive(Debug, Clone, PartialEq)]
pub struct BarBucket {
    pub symbol: String,
    pub timeframe: String,
    pub bucket_start_ms: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
    pub tick_count: u32,
}

/// Outcome of feeding one tick into the aggregator.
///
/// `current` is always populated — it is the bucket the tick was applied to
/// (either a freshly opened one, or the existing bucket extended in place).
///
/// `closed` is `Some(prev)` only on the tick that crossed into a new bucket.
/// Consumers that want a "closed bar" signal should fan `prev` out with
/// `closed = true` BEFORE the live `current` bar (or after — order is up to
/// the caller; `bar_hub` consumers in `ws_loop` emit live-then-closed today).
#[derive(Debug, Clone, PartialEq)]
pub struct ProcessResult {
    pub closed: Option<BarBucket>,
    pub current: BarBucket,
}

pub struct OhlcAggregator {
    buckets: Mutex<HashMap<(String, String), BarBucket>>,
}

impl OhlcAggregator {
    pub fn new() -> Self {
        Self { buckets: Mutex::new(HashMap::new()) }
    }

    /// Fold one tick into the bucket for `(symbol, timeframe)`.
    ///
    /// Lock scope is exactly one HashMap lookup + a small struct mutation —
    /// the hot path holds the mutex for nanoseconds. Symbol counts are
    /// bounded (active subscriptions), so contention is negligible.
    pub fn process_tick(
        &self,
        symbol: &str,
        tf: &str,
        ts_ms: i64,
        price: f64,
        volume: f64,
    ) -> ProcessResult {
        let bucket_start = bucket_start_for(tf, ts_ms);
        let key = (symbol.to_string(), tf.to_string());
        let mut map = self.buckets.lock().expect("ohlc aggregator poisoned");

        let entry = map.get(&key).cloned();
        match entry {
            Some(cur) if cur.bucket_start_ms == bucket_start => {
                // Extend in place — same bucket.
                let mut new = cur;
                if price > new.high { new.high = price; }
                if price < new.low  { new.low  = price; }
                new.close = price;
                new.volume += volume;
                new.tick_count += 1;
                map.insert(key, new.clone());
                ProcessResult { closed: None, current: new }
            }
            Some(prev) => {
                // Tick crossed into a new bucket; prev closes, open a new one.
                debug_assert!(bucket_start > prev.bucket_start_ms,
                    "ticks must arrive in non-decreasing time order");
                let fresh = BarBucket {
                    symbol: symbol.to_string(),
                    timeframe: tf.to_string(),
                    bucket_start_ms: bucket_start,
                    open: price, high: price, low: price, close: price,
                    volume,
                    tick_count: 1,
                };
                map.insert(key, fresh.clone());
                ProcessResult { closed: Some(prev), current: fresh }
            }
            None => {
                // First tick for this slot.
                let fresh = BarBucket {
                    symbol: symbol.to_string(),
                    timeframe: tf.to_string(),
                    bucket_start_ms: bucket_start,
                    open: price, high: price, low: price, close: price,
                    volume,
                    tick_count: 1,
                };
                map.insert(key, fresh.clone());
                ProcessResult { closed: None, current: fresh }
            }
        }
    }

    /// Test-only: wipe the global state.
    #[cfg(test)]
    pub fn reset(&self) {
        self.buckets.lock().unwrap().clear();
    }
}

impl Default for OhlcAggregator {
    fn default() -> Self { Self::new() }
}

/// Bucket boundary for a given timeframe. Currently only `"1m"` is
/// honored; anything else falls back to 1-minute buckets so we never
/// silently drop a tick. Extend here when adding 5m/1h/etc.
fn bucket_start_for(tf: &str, ts_ms: i64) -> i64 {
    let width_ms = match tf {
        "1m" => 60_000,
        _    => 60_000, // fallback — see module doc
    };
    (ts_ms / width_ms) * width_ms
}

static AGG: OnceLock<OhlcAggregator> = OnceLock::new();
pub fn aggregator() -> &'static OhlcAggregator {
    AGG.get_or_init(OhlcAggregator::new)
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // Use per-test unique symbols so the singleton state from `aggregator()`
    // can't leak across tests when they run in parallel. The unit tests below
    // construct fresh aggregators directly — the singleton is only exercised
    // by the ws_loop integration test in `mod.rs`.

    #[test]
    fn process_first_tick_creates_bucket() {
        let a = OhlcAggregator::new();
        let r = a.process_tick("AAPL", "1m", 60_000, 100.0, 5.0);
        assert!(r.closed.is_none());
        assert_eq!(r.current.bucket_start_ms, 60_000);
        assert_eq!(r.current.open, 100.0);
        assert_eq!(r.current.high, 100.0);
        assert_eq!(r.current.low, 100.0);
        assert_eq!(r.current.close, 100.0);
        assert_eq!(r.current.volume, 5.0);
        assert_eq!(r.current.tick_count, 1);
        assert_eq!(r.current.symbol, "AAPL");
        assert_eq!(r.current.timeframe, "1m");
    }

    #[test]
    fn tick_within_same_minute_extends_bucket() {
        let a = OhlcAggregator::new();
        a.process_tick("AAPL", "1m", 60_000, 100.0, 5.0);
        a.process_tick("AAPL", "1m", 60_500, 105.0, 2.0); // new high
        let r = a.process_tick("AAPL", "1m", 59_999 + 30_000, 98.0, 3.0); // ts 89_999 -> same bucket 60_000
        // (89_999 / 60_000) * 60_000 = 60_000
        assert!(r.closed.is_none());
        assert_eq!(r.current.bucket_start_ms, 60_000);
        assert_eq!(r.current.open, 100.0);
        assert_eq!(r.current.high, 105.0);
        assert_eq!(r.current.low, 98.0);
        assert_eq!(r.current.close, 98.0);
        assert_eq!(r.current.volume, 10.0);
        assert_eq!(r.current.tick_count, 3);
    }

    #[test]
    fn tick_crosses_minute_boundary_closes_previous_and_starts_new() {
        let a = OhlcAggregator::new();
        a.process_tick("AAPL", "1m", 60_000, 100.0, 5.0);
        a.process_tick("AAPL", "1m", 90_000, 110.0, 4.0);
        let r = a.process_tick("AAPL", "1m", 120_000, 95.0, 7.0); // bucket 120_000 (new)

        let closed = r.closed.expect("expected previous bucket to close");
        assert_eq!(closed.bucket_start_ms, 60_000);
        assert_eq!(closed.open, 100.0);
        assert_eq!(closed.high, 110.0);
        assert_eq!(closed.low, 100.0);
        assert_eq!(closed.close, 110.0);
        assert_eq!(closed.volume, 9.0);
        assert_eq!(closed.tick_count, 2);

        assert_eq!(r.current.bucket_start_ms, 120_000);
        assert_eq!(r.current.open, 95.0);
        assert_eq!(r.current.high, 95.0);
        assert_eq!(r.current.low, 95.0);
        assert_eq!(r.current.close, 95.0);
        assert_eq!(r.current.volume, 7.0);
        assert_eq!(r.current.tick_count, 1);
    }

    #[test]
    fn independent_symbols_have_independent_buckets() {
        let a = OhlcAggregator::new();
        a.process_tick("AAPL", "1m", 60_000, 100.0, 5.0);
        a.process_tick("MSFT", "1m", 60_000, 300.0, 1.0);
        // Crossing minute only on AAPL must NOT close MSFT.
        let r_aapl = a.process_tick("AAPL", "1m", 120_000, 101.0, 2.0);
        assert!(r_aapl.closed.is_some(), "AAPL bucket should have closed");

        let r_msft = a.process_tick("MSFT", "1m", 90_000, 305.0, 4.0);
        assert!(r_msft.closed.is_none(), "MSFT same-minute tick must not close");
        assert_eq!(r_msft.current.open, 300.0);
        assert_eq!(r_msft.current.close, 305.0);
        assert_eq!(r_msft.current.volume, 5.0);
        assert_eq!(r_msft.current.tick_count, 2);
    }

    #[test]
    fn bucket_start_alignment_matches_floor_division() {
        assert_eq!(bucket_start_for("1m", 0),       0);
        assert_eq!(bucket_start_for("1m", 59_999),  0);
        assert_eq!(bucket_start_for("1m", 60_000),  60_000);
        assert_eq!(bucket_start_for("1m", 60_001),  60_000);
        assert_eq!(bucket_start_for("1m", 1_700_000_123_456),
                   (1_700_000_123_456_i64 / 60_000) * 60_000);
    }
}
