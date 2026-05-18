//! Price in micro-units (1/1_000_000 of a dollar).
//! $145.32 = Price(145_320_000).
//!
//! Replaces f32 prices throughout the order/trade hot path. Avoids float drift
//! on accumulated fills and eliminates the rounding-collision dedup bug
//! (`(price * 100.0).round() as i64` collapses $145.999 and $146.001 onto the
//! same integer-cents bucket; comparing `Price(i64)` micro-units does not).
//!
//! Wire format: a bare integer (i64) via `#[serde(transparent)]`. For
//! backwards compatibility with persisted f32 prices, callers that load older
//! state should use `Price::from_f32` at the boundary — Wave 3 introduced this
//! type and any data written before then was f32.

use serde::{Deserialize, Serialize};

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Default,
         Serialize, Deserialize)]
#[serde(transparent)]
pub struct Price(pub i64);

impl Price {
    pub const ZERO: Price = Price(0);
    pub const MICRO_PER_DOLLAR: i64 = 1_000_000;

    /// Construct from a dollar amount expressed as f64. Rounds to the nearest
    /// micro-unit.
    pub fn from_dollars(d: f64) -> Self {
        Price((d * Self::MICRO_PER_DOLLAR as f64).round() as i64)
    }

    /// Construct from a dollar amount expressed as f32. Widens to f64 before
    /// scaling so the round-trip through `to_f32` is stable for typical
    /// equity / option price magnitudes (< $1M).
    pub fn from_f32(d: f32) -> Self {
        Price((d as f64 * Self::MICRO_PER_DOLLAR as f64).round() as i64)
    }

    /// Convert back to dollars as f64. Lossless for any value the constructors
    /// accept (i64 mantissa easily fits in f64 for any realistic price).
    pub fn to_dollars(self) -> f64 {
        self.0 as f64 / Self::MICRO_PER_DOLLAR as f64
    }

    /// Convert back to dollars as f32. Lossy for very large or very precise
    /// values; fine for display + broker wire format.
    pub fn to_f32(self) -> f32 {
        self.to_dollars() as f32
    }

    /// Cents (1/100 dollar). Useful for legacy systems that store integer
    /// cents and for display formatting at lower precision than micro-units.
    pub fn cents(self) -> i64 {
        self.0 / 10_000
    }

    /// True iff this is the zero / "no price" sentinel (market orders).
    pub fn is_zero(self) -> bool { self.0 == 0 }
}

impl std::fmt::Display for Price {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "${:.2}", self.to_dollars())
    }
}

impl std::ops::Add for Price {
    type Output = Price;
    fn add(self, rhs: Price) -> Price { Price(self.0 + rhs.0) }
}

impl std::ops::Sub for Price {
    type Output = Price;
    fn sub(self, rhs: Price) -> Price { Price(self.0 - rhs.0) }
}

impl std::ops::Mul<i64> for Price {
    type Output = Price;
    /// `Price * size` — total notional in micro-dollars.
    fn mul(self, rhs: i64) -> Price { Price(self.0 * rhs) }
}

impl std::ops::Div<i64> for Price {
    type Output = Price;
    /// `Price / n` — dollar-cost-average style. Integer division on micro
    /// units; remainder is dropped (sub-micro precision is meaningless).
    fn div(self, rhs: i64) -> Price { Price(self.0 / rhs) }
}

// ── Wire-compat serde adapters ──────────────────────────────────────────────
//
// `Price`'s native serde shape is a bare i64 (transparent). For structs that
// persist to disk in an older f32 format (e.g. `orders.json` predates this
// type), use `#[serde(with = "price_serde::as_f32")]` on the field to keep
// the wire shape as f32 while the in-memory type is `Price`.
//
// `as_f32_opt` is the `Option<Price>` variant.

pub mod price_serde {
    use super::Price;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub mod as_f32 {
        use super::*;
        pub fn serialize<S: Serializer>(p: &Price, s: S) -> Result<S::Ok, S::Error> {
            p.to_f32().serialize(s)
        }
        pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Price, D::Error> {
            f32::deserialize(d).map(Price::from_f32)
        }
    }

    pub mod as_f32_opt {
        use super::*;
        pub fn serialize<S: Serializer>(p: &Option<Price>, s: S) -> Result<S::Ok, S::Error> {
            p.map(|x| x.to_f32()).serialize(s)
        }
        pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Price>, D::Error> {
            Option::<f32>::deserialize(d).map(|o| o.map(Price::from_f32))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_from_to_dollars() {
        let p = Price::from_dollars(145.32);
        assert_eq!(p.0, 145_320_000);
        assert!((p.to_dollars() - 145.32).abs() < 1e-9);
    }

    #[test]
    fn roundtrip_from_to_f32() {
        let p = Price::from_f32(145.32);
        // f32 precision at this magnitude is ~6 digits; allow 1e-3 wobble.
        assert!((p.to_f32() - 145.32).abs() < 1e-3);
    }

    #[test]
    fn no_drift_after_repeated_roundtrip() {
        // The whole reason this type exists: prices must NOT accumulate drift
        // when bounced through f64.
        let mut p = Price::from_dollars(145.32);
        for _ in 0..1_000 {
            p = Price::from_dollars(p.to_dollars());
        }
        assert_eq!(p, Price::from_dollars(145.32));
    }

    #[test]
    fn dedup_signature_distinguishes_neighbouring_prices() {
        // The legacy signature did `(price * 100.0).round() as i64` which
        // bucketed $145.999 and $146.001 together (both round to 14600 cents).
        // With Price(i64) micro-units, the two values must be DIFFERENT.
        let a = Price::from_dollars(145.999);
        let b = Price::from_dollars(146.001);
        assert_ne!(a, b, "neighbouring sub-cent prices must NOT collide");
        // Spell the old broken behaviour out so a regression is obvious:
        assert_ne!(a.0, b.0);
        assert_eq!((a.to_dollars() * 100.0).round() as i64,
                   (b.to_dollars() * 100.0).round() as i64,
                   "legacy *100-rounding bucket would still collide them");
    }

    #[test]
    fn arithmetic_add_sub() {
        let a = Price::from_dollars(10.50);
        let b = Price::from_dollars(0.25);
        assert_eq!((a + b).to_dollars(), 10.75);
        assert_eq!((a - b).to_dollars(), 10.25);
    }

    #[test]
    fn arithmetic_mul_size() {
        let p = Price::from_dollars(2.50);
        // 2.50 * 4 shares = $10.00 notional, in micro-units that's 10_000_000.
        assert_eq!((p * 4i64).0, 10_000_000);
    }

    #[test]
    fn arithmetic_div_average() {
        let total = Price::from_dollars(10.00);
        let avg = total / 4;
        assert_eq!(avg.to_dollars(), 2.50);
    }

    #[test]
    fn cents_accessor() {
        let p = Price::from_dollars(145.32);
        assert_eq!(p.cents(), 14_532);
    }

    #[test]
    fn display_uses_dollar_format() {
        let p = Price::from_dollars(145.32);
        assert_eq!(format!("{}", p), "$145.32");
    }

    #[test]
    fn zero_const() {
        assert_eq!(Price::ZERO, Price(0));
        assert!(Price::ZERO.is_zero());
        assert!(!Price::from_dollars(0.01).is_zero());
    }

    #[test]
    fn serde_transparent_emits_bare_integer() {
        let p = Price::from_dollars(145.32);
        let s = serde_json::to_string(&p).unwrap();
        assert_eq!(s, "145320000");
        let back: Price = serde_json::from_str(&s).unwrap();
        assert_eq!(back, p);
    }
}

#[cfg(test)]
mod proptests {
    //! Property-based tests for `Price`.
    //!
    //! Wave 9e: proptest covers the invariants the hand-written suite spot-
    //! checks — micro-unit round-trip, add/sub inverse, ordering, sub-cent
    //! distinctness. proptest runs 256 cases per `#[test]` by default; the
    //! whole block finishes well under a second.
    use super::*;
    use proptest::prelude::*;

    proptest! {
        /// `from_dollars` ↔ `to_dollars` must round-trip within micro-unit
        /// precision across the entire "printable" price range (±$1M).
        #[test]
        fn from_f64_to_f64_lossless_in_printable_range(d in -1_000_000.0f64..=1_000_000.0) {
            let p = Price::from_dollars(d);
            let back = p.to_dollars();
            prop_assert!(
                (back - d).abs() < 0.0000005,
                "drift exceeded: {} → {} → {}", d, p.0, back
            );
        }

        /// `(a + b) - b == a` for any pair of micro-unit prices within a
        /// range that cannot overflow i64.
        #[test]
        fn add_sub_inverse(
            a in -100_000_000i64..=100_000_000,
            b in -100_000_000i64..=100_000_000,
        ) {
            let pa = Price(a);
            let pb = Price(b);
            prop_assert_eq!((pa + pb) - pb, pa);
        }

        /// Derived ordering must match the underlying `i64` ordering.
        #[test]
        fn ordering_matches_underlying_i64(a in any::<i64>(), b in any::<i64>()) {
            prop_assert_eq!(Price(a).cmp(&Price(b)), a.cmp(&b));
        }

        /// Two prices 2 cents apart must NEVER hash/compare equal — the
        /// legacy `(price*100).round()` bucketing bug would collapse them.
        #[test]
        fn neighboring_prices_distinct(d in 0.000001f64..=10.0) {
            let a = Price::from_dollars(d);
            let b = Price::from_dollars(d + 0.002); // 2 cents = 20_000 micro-units
            prop_assert_ne!(a, b);
        }
    }
}
