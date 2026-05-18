//! Wave 13b/14b: time-bucket OHLC aggregator for IB tick stream.
//!
//! Wave 12a wired IB ticks into `bar_hub()` but emitted a degenerate per-tick
//! bar (O=H=L=C=tick, volume=tick_volume). Wave 13b replaced that with a
//! proper time-bucket aggregator keyed by `(symbol, timeframe)`. Wave 14b
//! extended bucket math to recognize the full set of standard timeframe
//! strings ("1s"…"1d") so consumers can subscribe to any TF, not just "1m".
//!
//! Bucket boundaries are pure epoch-millisecond floor division: for any TF
//! whose width divides evenly into 24h, the daily boundary lands at 00:00
//! UTC. Market-hours-aware sessions (e.g. NYSE-RTH 9:30–16:00) need a
//! different scheme — out of scope here; flagged as follow-up.
//!
//! Unknown TF strings fall back to a 1-minute bucket and log a single
//! `tracing::warn!` per `(symbol, tf)` pair (don't spam every tick).
//!
//! All field widths use `f64` (matches `BarWire` on the wire) — the task
//! spec sketched `f32`, but every consumer downstream is already `f64`, so
//! using `f32` here would force a cast on every fanout.

use std::collections::{HashMap, HashSet};
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
        let bucket_start = bucket_start_for_logged(symbol, tf, ts_ms);
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

/// Bucket width in milliseconds for a given standard timeframe string.
///
/// Recognized: 1s/5s/15s/30s, 1m/2m/3m/5m/10m/15m/30m, 1h/2h/4h/6h/12h, 1d.
/// Any other input falls back to 60_000 ms (1m) — callers that route
/// through [`bucket_start_for`] also emit a one-shot warning per
/// `(symbol, tf)` so unknown timeframes surface in logs without spamming.
pub(crate) fn bucket_size_ms(tf: &str) -> i64 {
    match tf {
        "1s"  => 1_000,
        "5s"  => 5_000,
        "15s" => 15_000,
        "30s" => 30_000,
        "1m"  => 60_000,
        "2m"  => 120_000,
        "3m"  => 180_000,
        "5m"  => 300_000,
        "10m" => 600_000,
        "15m" => 900_000,
        "30m" => 1_800_000,
        "1h"  => 3_600_000,
        "2h"  => 7_200_000,
        "4h"  => 14_400_000,
        "6h"  => 21_600_000,
        "12h" => 43_200_000,
        "1d"  => 86_400_000,
        _     => 60_000, // fallback — see module doc + UNKNOWN_TF_WARNED
    }
}

/// Returns true when `tf` is one of the recognized standard timeframes.
fn is_known_tf(tf: &str) -> bool {
    matches!(tf,
        "1s" | "5s" | "15s" | "30s" |
        "1m" | "2m" | "3m" | "5m" | "10m" | "15m" | "30m" |
        "1h" | "2h" | "4h" | "6h" | "12h" |
        "1d"
    )
}

/// Per-(symbol, tf) dedupe set for the unknown-TF warning. Keeps the log
/// signal-to-noise high: one warn per pair, not one per tick.
static UNKNOWN_TF_WARNED: OnceLock<Mutex<HashSet<(String, String)>>> = OnceLock::new();

fn warn_unknown_tf_once(symbol: &str, tf: &str) {
    let set = UNKNOWN_TF_WARNED.get_or_init(|| Mutex::new(HashSet::new()));
    let key = (symbol.to_string(), tf.to_string());
    let mut guard = set.lock().expect("unknown-tf warn set poisoned");
    if guard.insert(key) {
        tracing::warn!(
            target: "ib_ws::aggregator",
            symbol = %symbol,
            tf = %tf,
            "unknown timeframe; falling back to 1m buckets"
        );
    }
}

/// Bucket start (epoch ms) for `ts_ms` under timeframe `tf`. Pure
/// floor-division — for any width that divides 24h evenly the daily
/// boundary lands at 00:00 UTC.
pub(crate) fn bucket_start_for(tf: &str, ts_ms: i64) -> i64 {
    let size = bucket_size_ms(tf);
    (ts_ms / size) * size
}

/// Same as [`bucket_start_for`] but additionally emits a one-shot
/// `tracing::warn!` per (symbol, tf) if `tf` is not a recognized standard
/// timeframe. Called from the hot tick path so the symbol context is in
/// scope; pure callers (and tests) can use [`bucket_start_for`] directly.
pub(crate) fn bucket_start_for_logged(symbol: &str, tf: &str, ts_ms: i64) -> i64 {
    if !is_known_tf(tf) {
        warn_unknown_tf_once(symbol, tf);
    }
    bucket_start_for(tf, ts_ms)
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

    // ── Wave 14b: multi-TF bucket boundary tests ─────────────────────────────

    #[test]
    fn process_tick_5m_buckets_at_5min_boundaries() {
        // 5m bucket width = 300_000 ms. ts=300_001 → bucket_start=300_000.
        assert_eq!(bucket_start_for("5m", 0),         0);
        assert_eq!(bucket_start_for("5m", 299_999),   0);
        assert_eq!(bucket_start_for("5m", 300_000),   300_000);
        assert_eq!(bucket_start_for("5m", 300_001),   300_000);
        assert_eq!(bucket_start_for("5m", 600_000),   600_000);

        let a = OhlcAggregator::new();
        let r1 = a.process_tick("AAPL", "5m", 0,        100.0, 1.0);
        assert_eq!(r1.current.bucket_start_ms, 0);
        let r2 = a.process_tick("AAPL", "5m", 299_999,  101.0, 1.0);
        assert!(r2.closed.is_none());
        assert_eq!(r2.current.bucket_start_ms, 0);
        let r3 = a.process_tick("AAPL", "5m", 300_001,  102.0, 1.0);
        let closed = r3.closed.expect("expected close on 5m boundary cross");
        assert_eq!(closed.bucket_start_ms, 0);
        assert_eq!(r3.current.bucket_start_ms, 300_000);
    }

    #[test]
    fn one_min_and_five_min_buckets_coexist_for_same_symbol() {
        // (sym, "1m") and (sym, "5m") are independent map keys — a tick
        // crossing the 1m boundary must NOT close the 5m bucket.
        let a = OhlcAggregator::new();
        a.process_tick("AAPL", "1m", 60_000,  100.0, 1.0);
        a.process_tick("AAPL", "5m", 60_000,  100.0, 1.0);

        // Tick at 120_000 ms crosses 1m boundary (60_000 → 120_000) but is
        // still inside the 5m bucket [0, 300_000).
        let r1 = a.process_tick("AAPL", "1m", 120_000, 102.0, 1.0);
        assert!(r1.closed.is_some(), "1m bucket must close on minute rollover");

        let r5 = a.process_tick("AAPL", "5m", 120_000, 102.0, 1.0);
        assert!(r5.closed.is_none(), "5m bucket must NOT close on minute rollover");
        assert_eq!(r5.current.bucket_start_ms, 0);
        assert_eq!(r5.current.open,  100.0);
        assert_eq!(r5.current.close, 102.0);
        assert_eq!(r5.current.tick_count, 2);
    }

    #[test]
    fn one_hour_bucket_closes_on_hour_boundary() {
        let a = OhlcAggregator::new();
        // bucket starts at hour 1 → ts = 3_600_000
        let h1 = 3_600_000_i64;
        let h2 = 2 * 3_600_000_i64;
        a.process_tick("AAPL", "1h", h1 + 100,         100.0, 1.0);
        let inside = a.process_tick("AAPL", "1h", h1 + 1_000_000, 110.0, 2.0);
        assert!(inside.closed.is_none());
        assert_eq!(inside.current.bucket_start_ms, h1);

        let cross = a.process_tick("AAPL", "1h", h2 + 1, 105.0, 4.0);
        let closed = cross.closed.expect("expected close on hour boundary");
        assert_eq!(closed.bucket_start_ms, h1);
        assert_eq!(closed.tick_count, 2);
        assert_eq!(cross.current.bucket_start_ms, h2);
    }

    #[test]
    fn one_day_bucket_closes_at_utc_midnight() {
        // Epoch-ms floor division by 86_400_000 gives 00:00 UTC.
        let a = OhlcAggregator::new();
        let day_size = 86_400_000_i64;
        // 2024-01-01 00:00:00 UTC = 1_704_067_200_000 ms (divisible by day_size).
        let d1 = 1_704_067_200_000_i64;
        let d2 = d1 + day_size; // 2024-01-02 00:00:00 UTC
        assert_eq!(d1 % day_size, 0, "sanity: d1 is on UTC midnight");

        a.process_tick("AAPL", "1d", d1 + 1,                100.0, 1.0);
        let noon = a.process_tick("AAPL", "1d", d1 + 12 * 3_600_000, 110.0, 1.0);
        assert!(noon.closed.is_none());
        assert_eq!(noon.current.bucket_start_ms, d1);

        let next = a.process_tick("AAPL", "1d", d2 + 30_000, 105.0, 1.0);
        let closed = next.closed.expect("expected close at UTC midnight");
        assert_eq!(closed.bucket_start_ms, d1);
        assert_eq!(next.current.bucket_start_ms, d2);
    }

    #[test]
    fn unknown_tf_falls_back_to_1m_and_does_not_panic() {
        // The warning is OnceLock-deduped per (symbol, tf); we can't easily
        // assert the warn count without wiring a tracing subscriber, but we
        // can assert the function returns 1m-aligned buckets and survives.
        assert_eq!(bucket_start_for("7m",   60_001),  60_000);
        assert_eq!(bucket_start_for("",     60_001),  60_000);
        assert_eq!(bucket_start_for("xyzz", 120_500), 120_000);

        // bucket_start_for_logged should also return the same value and
        // be safe to call repeatedly (idempotent dedupe).
        let v1 = bucket_start_for_logged("AAPL", "7m", 60_001);
        let v2 = bucket_start_for_logged("AAPL", "7m", 60_500);
        let v3 = bucket_start_for_logged("AAPL", "7m", 60_999);
        assert_eq!(v1, 60_000);
        assert_eq!(v2, 60_000);
        assert_eq!(v3, 60_000);

        // And the aggregator itself folds correctly under the 1m fallback.
        let a = OhlcAggregator::new();
        let r1 = a.process_tick("AAPL", "7m", 60_001, 100.0, 1.0);
        assert_eq!(r1.current.bucket_start_ms, 60_000);
        let r2 = a.process_tick("AAPL", "7m", 120_001, 101.0, 1.0);
        assert!(r2.closed.is_some(), "fallback 1m bucket must still close on minute rollover");
    }

    #[test]
    fn bucket_size_ms_table_is_complete() {
        // Defensive: catch typos in the match arms.
        assert_eq!(bucket_size_ms("1s"),  1_000);
        assert_eq!(bucket_size_ms("5s"),  5_000);
        assert_eq!(bucket_size_ms("15s"), 15_000);
        assert_eq!(bucket_size_ms("30s"), 30_000);
        assert_eq!(bucket_size_ms("1m"),  60_000);
        assert_eq!(bucket_size_ms("2m"),  120_000);
        assert_eq!(bucket_size_ms("3m"),  180_000);
        assert_eq!(bucket_size_ms("5m"),  300_000);
        assert_eq!(bucket_size_ms("10m"), 600_000);
        assert_eq!(bucket_size_ms("15m"), 900_000);
        assert_eq!(bucket_size_ms("30m"), 1_800_000);
        assert_eq!(bucket_size_ms("1h"),  3_600_000);
        assert_eq!(bucket_size_ms("2h"),  7_200_000);
        assert_eq!(bucket_size_ms("4h"),  14_400_000);
        assert_eq!(bucket_size_ms("6h"),  21_600_000);
        assert_eq!(bucket_size_ms("12h"), 43_200_000);
        assert_eq!(bucket_size_ms("1d"),  86_400_000);
        assert_eq!(bucket_size_ms("nonsense"), 60_000); // fallback
    }
}
