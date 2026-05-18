//! Corporate actions affecting a symbol's price history / continuity.
//!
//! Wave 10c: typed domain model for `MarketDataProvider::corporate_actions`.
//! Covers splits, dividends, spinoffs, mergers, ticker renames, plus a
//! catch-all `Other` for vendor-specific kinds we don't model yet.

use serde::{Deserialize, Serialize};
use super::{Price, Timestamp};

/// What kind of corporate action this row represents. Carries action-
/// specific payload inline so callers don't need a side table.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CorporateActionKind {
    /// Stock split. `2:1` is `ratio_num=2, ratio_den=1`; a 1-for-10
    /// reverse split is `ratio_num=1, ratio_den=10`.
    Split { ratio_num: u32, ratio_den: u32 },
    /// Cash dividend. `amount` is per-share in the listed currency.
    Dividend { amount: Price, currency: String },
    /// Spinoff issuing shares in `child_symbol`.
    Spinoff { child_symbol: String },
    /// Merger / acquisition. `exchange_ratio` is shares of the
    /// acquirer received per share of this symbol.
    Merger { acquirer_symbol: String, exchange_ratio: f32 },
    /// Ticker rename (e.g. `FB` → `META`).
    NameChange { new_symbol: String },
    /// Provider-specific action we don't model — carries the raw
    /// vendor label so downstream code can decide what to do.
    Other(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CorporateAction {
    pub symbol: String,
    pub action: CorporateActionKind,
    /// When the action takes effect (ex-date).
    pub effective_date: Timestamp,
    /// When the action was announced.
    pub announced_date: Timestamp,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::foundation::types::TimeSource;

    fn ts(ms: i64) -> Timestamp { Timestamp::from_millis(ms, TimeSource::ExchangeUtc) }

    #[test]
    fn serde_round_trip_split() {
        let a = CorporateAction {
            symbol: "AAPL".into(),
            action: CorporateActionKind::Split { ratio_num: 4, ratio_den: 1 },
            effective_date: ts(1_700_000_000_000),
            announced_date: ts(1_699_000_000_000),
        };
        let json = serde_json::to_string(&a).unwrap();
        let back: CorporateAction = serde_json::from_str(&json).unwrap();
        assert_eq!(a, back);
    }

    #[test]
    fn serde_round_trip_dividend() {
        let a = CorporateAction {
            symbol: "MSFT".into(),
            action: CorporateActionKind::Dividend {
                amount: Price::from_dollars(0.83),
                currency: "USD".into(),
            },
            effective_date: ts(1_700_000_000_000),
            announced_date: ts(1_699_000_000_000),
        };
        let json = serde_json::to_string(&a).unwrap();
        let back: CorporateAction = serde_json::from_str(&json).unwrap();
        assert_eq!(a, back);
    }

    #[test]
    fn serde_round_trip_name_change() {
        let a = CorporateAction {
            symbol: "FB".into(),
            action: CorporateActionKind::NameChange { new_symbol: "META".into() },
            effective_date: ts(1_660_000_000_000),
            announced_date: ts(1_640_000_000_000),
        };
        let json = serde_json::to_string(&a).unwrap();
        let back: CorporateAction = serde_json::from_str(&json).unwrap();
        assert_eq!(a, back);
    }

    #[test]
    fn serde_round_trip_other() {
        let a = CorporateAction {
            symbol: "X".into(),
            action: CorporateActionKind::Other("rights_offering".into()),
            effective_date: ts(1),
            announced_date: ts(0),
        };
        let json = serde_json::to_string(&a).unwrap();
        let back: CorporateAction = serde_json::from_str(&json).unwrap();
        assert_eq!(a, back);
    }
}
