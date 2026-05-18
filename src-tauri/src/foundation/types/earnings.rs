//! Earnings calendar entries.
//!
//! Wave 10c: typed domain model for `MarketDataProvider::earnings`.
//! Covers both upcoming reports (estimates only) and historical
//! reports (estimate + actual).

use serde::{Deserialize, Serialize};
use super::Timestamp;

/// When during the trading day the earnings report is released.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EarningsWhen {
    BeforeMarket,
    AfterMarket,
    DuringHours,
    Unknown,
}

/// A single earnings calendar item — upcoming or historical.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EarningsItem {
    pub symbol: String,
    /// Scheduled / actual report time.
    pub reports_at: Timestamp,
    pub eps_estimate: Option<f32>,
    pub eps_actual: Option<f32>,
    /// Revenue in whole dollars.
    pub revenue_estimate: Option<i64>,
    pub revenue_actual: Option<i64>,
    pub when: EarningsWhen,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::foundation::types::TimeSource;

    #[test]
    fn serde_round_trip() {
        let e = EarningsItem {
            symbol: "AAPL".into(),
            reports_at: Timestamp::from_millis(1_700_000_000_000, TimeSource::ExchangeUtc),
            eps_estimate: Some(1.25),
            eps_actual: Some(1.31),
            revenue_estimate: Some(120_000_000_000),
            revenue_actual: Some(123_500_000_000),
            when: EarningsWhen::AfterMarket,
        };
        let json = serde_json::to_string(&e).unwrap();
        let back: EarningsItem = serde_json::from_str(&json).unwrap();
        assert_eq!(e, back);
    }

    #[test]
    fn when_variants_round_trip() {
        for w in [EarningsWhen::BeforeMarket, EarningsWhen::AfterMarket,
                  EarningsWhen::DuringHours, EarningsWhen::Unknown] {
            let json = serde_json::to_string(&w).unwrap();
            let back: EarningsWhen = serde_json::from_str(&json).unwrap();
            assert_eq!(w, back);
        }
    }
}
