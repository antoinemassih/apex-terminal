//! Company / instrument fundamentals snapshot.
//!
//! Wave 10c: extracted as a typed domain model so `MarketDataProvider`
//! has a real `fundamentals()` surface (replacing the orphaned ad-hoc
//! placeholder shapes in `chart/renderer/gpu.rs`). Providers without a
//! real upstream return `ApiError::NotSupported` from the trait default.

use serde::{Deserialize, Serialize};

/// Snapshot of company-level fundamentals for one symbol.
///
/// All numeric fields are optional because different providers cover
/// different surfaces (a crypto venue has none of these; a US equity
/// provider has most). `market_cap` is in whole dollars, `eps` /
/// `dividend_yield` / `pe_ratio` follow the conventional units used in
/// market-data terminals (yield in percent: 1.25 == 1.25%).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Fundamentals {
    pub symbol: String,
    /// Market capitalization in whole dollars (no smaller unit).
    pub market_cap: Option<i64>,
    /// Trailing P/E ratio.
    pub pe_ratio: Option<f32>,
    /// EPS (trailing twelve months) in dollars.
    pub eps: Option<f32>,
    /// Forward dividend yield expressed as percent (e.g. `1.25` = 1.25%).
    pub dividend_yield: Option<f32>,
    pub sector: Option<String>,
    pub industry: Option<String>,
    /// Free-text company description, usually one paragraph.
    pub description: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serde_round_trip() {
        let f = Fundamentals {
            symbol: "AAPL".into(),
            market_cap: Some(3_500_000_000_000),
            pe_ratio: Some(29.4),
            eps: Some(6.42),
            dividend_yield: Some(0.45),
            sector: Some("Technology".into()),
            industry: Some("Consumer Electronics".into()),
            description: Some("Designs consumer electronics".into()),
        };
        let json = serde_json::to_string(&f).unwrap();
        let back: Fundamentals = serde_json::from_str(&json).unwrap();
        assert_eq!(f, back);
    }

    #[test]
    fn all_optional_fields_serialize_as_null() {
        let f = Fundamentals {
            symbol: "X".into(),
            market_cap: None, pe_ratio: None, eps: None, dividend_yield: None,
            sector: None, industry: None, description: None,
        };
        let json = serde_json::to_string(&f).unwrap();
        let back: Fundamentals = serde_json::from_str(&json).unwrap();
        assert_eq!(f, back);
    }
}
