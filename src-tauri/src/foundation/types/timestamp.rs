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

// ── Wire-compat serde adapters ──────────────────────────────────────────────
//
// Persisted older formats store ms as bare `u64`. Use
// `#[serde(with = "timestamp_serde::as_u64_local")]` on the field to keep the
// wire shape as a bare u64 (ms) while the in-memory type is `Timestamp`.
// The reconstituted `Timestamp` is tagged with `TimeSource::Local` since
// disk-persisted manager state is wall-clock UTC ms.

pub mod timestamp_serde {
    use super::*;

    pub mod as_u64_local {
        use super::*;
        use serde::{Deserialize, Deserializer, Serialize, Serializer};
        pub fn serialize<S: Serializer>(t: &Timestamp, s: S) -> Result<S::Ok, S::Error> {
            (t.unix_ms as u64).serialize(s)
        }
        pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Timestamp, D::Error> {
            let ms = u64::deserialize(d)? as i64;
            Ok(Timestamp::from_millis(ms, TimeSource::Local))
        }
    }

    /// State-history field: `Vec<(OrderState, u64)>` is the legacy wire
    /// shape. We keep the wire as `(_, u64)` and convert the second element
    /// to a `Timestamp` on the way in. Caller still picks an enum type for
    /// the first element (untouched by this adapter).
    pub mod as_pairs_u64_local {
        use super::*;
        use serde::{Deserialize, Deserializer, Serialize, Serializer};
        use serde::de::DeserializeOwned;
        pub fn serialize<S, T>(v: &Vec<(T, Timestamp)>, s: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
            T: Serialize + Clone,
        {
            let raw: Vec<(T, u64)> = v.iter().map(|(t, ts)| (t.clone(), ts.unix_ms as u64)).collect();
            raw.serialize(s)
        }
        pub fn deserialize<'de, D, T>(d: D) -> Result<Vec<(T, Timestamp)>, D::Error>
        where
            D: Deserializer<'de>,
            T: DeserializeOwned,
        {
            let raw: Vec<(T, u64)> = Vec::deserialize(d)?;
            Ok(raw.into_iter()
                .map(|(t, ms)| (t, Timestamp::from_millis(ms as i64, TimeSource::Local)))
                .collect())
        }
    }
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

#[cfg(test)]
mod proptests {
    //! Property-based tests for `Timestamp`.
    //!
    //! Wave 9e: proptest covers ms / seconds round-trips across the full
    //! realistic epoch range. 256 cases per generator by default.
    use super::*;
    use proptest::prelude::*;

    proptest! {
        /// `from_millis` ↔ `millis()` must round-trip exactly across the
        /// realistic epoch-ms range (±10^13).
        #[test]
        fn ms_round_trip(ms in -10_000_000_000_000i64..=10_000_000_000_000) {
            let t = Timestamp::from_millis(ms, TimeSource::ExchangeUtc);
            prop_assert_eq!(t.millis(), ms);
        }

        /// `from_seconds` ↔ `seconds()` must round-trip; and the underlying
        /// `unix_ms` must be exactly `s * 1000` (no precision loss from any
        /// intermediate float).
        #[test]
        fn seconds_round_trip(s in -10_000_000_000i64..=10_000_000_000) {
            let t = Timestamp::from_seconds(s, TimeSource::ExchangeUtc);
            prop_assert_eq!(t.seconds(), s);
            prop_assert_eq!(t.millis(), s * 1000);
        }
    }
}
