//! Typed timestamp with explicit time source.
//!
//! Different connectivity layers reported time in different units (REST in
//! seconds, WS in milliseconds, ManagedOrder in milliseconds, `std::time::Instant`
//! for latency measurements). Conversions were scattered `time / 1000` calls.
//! `Timestamp` enforces the unit at the type level and `TimeSource` records
//! provenance so a UI/journal consumer can show "exchange UTC" vs "local
//! wall clock" without guessing.

use serde::{Deserialize, Serialize};

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash,
         Serialize, Deserialize)]
pub enum TimeSource {
    /// Canonical exchange UTC (bars, quotes, trades).
    ExchangeUtc,
    /// Wall-clock local time (user input, journal).
    Local,
    /// `std::time::Instant`-equivalent (latency measurements). Not meaningful
    /// across processes; used only for in-process duration math.
    Monotonic,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash,
         Serialize, Deserialize)]
pub struct Timestamp {
    pub unix_ms: i64,
    pub source: TimeSource,
}

impl Timestamp {
    /// Build from epoch seconds. Multiplies into milliseconds at the boundary.
    pub fn from_seconds(s: i64, source: TimeSource) -> Self {
        Self { unix_ms: s.saturating_mul(1000), source }
    }

    /// Build from epoch milliseconds.
    pub fn from_millis(ms: i64, source: TimeSource) -> Self {
        Self { unix_ms: ms, source }
    }

    /// Current wall-clock UTC. Source is recorded as `Local` because the
    /// system clock is owned by the host OS, not the exchange.
    pub fn now_utc() -> Self {
        let ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        Self { unix_ms: ms, source: TimeSource::Local }
    }

    /// Epoch seconds (truncating toward zero). Lossy.
    pub fn seconds(self) -> i64 { self.unix_ms / 1000 }

    /// Epoch milliseconds (canonical).
    pub fn millis(self) -> i64 { self.unix_ms }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_seconds_roundtrip() {
        let t = Timestamp::from_seconds(1_700_000_000, TimeSource::ExchangeUtc);
        assert_eq!(t.unix_ms, 1_700_000_000_000);
        assert_eq!(t.seconds(), 1_700_000_000);
        assert_eq!(t.source, TimeSource::ExchangeUtc);
    }

    #[test]
    fn from_millis_roundtrip() {
        let t = Timestamp::from_millis(1_700_000_123, TimeSource::Local);
        assert_eq!(t.millis(), 1_700_000_123);
        assert_eq!(t.seconds(), 1_700_000); // truncated
    }

    #[test]
    fn now_utc_is_positive_and_local() {
        let t = Timestamp::now_utc();
        assert!(t.unix_ms > 0);
        assert_eq!(t.source, TimeSource::Local);
    }

    #[test]
    fn seconds_to_millis_does_not_truncate_low_bits() {
        // The whole point: seconds × 1000 must be exact, not the result of an
        // intermediate float conversion that loses bits.
        let t = Timestamp::from_seconds(2_500_000_000, TimeSource::ExchangeUtc);
        assert_eq!(t.unix_ms, 2_500_000_000_000);
    }

    #[test]
    fn ordering_is_by_unix_ms() {
        let earlier = Timestamp::from_millis(100, TimeSource::Local);
        let later   = Timestamp::from_millis(200, TimeSource::Local);
        assert!(earlier < later);
    }
}
