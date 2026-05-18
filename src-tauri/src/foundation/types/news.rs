//! News items for the news/feed panel.
//!
//! Wave 10c: typed domain model for `MarketDataProvider::news`. Replaces
//! the inline `NewsItem` literal struct in `chart/renderer/gpu.rs` for
//! the data-layer surface (the GPU struct stays put — UI plumbing is a
//! separate wave).

use serde::{Deserialize, Serialize};
use super::Timestamp;

/// A single news headline + metadata. Ordering is by `published_at`
/// (most recent first) when returned in a list.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NewsItem {
    /// Stable provider-specific id (used for dedup across refreshes).
    pub id: String,
    /// Primary symbol the item is associated with. `None` for
    /// macro/economic headlines.
    pub symbol: Option<String>,
    pub headline: String,
    pub source: String,
    pub url: Option<String>,
    pub published_at: Timestamp,
    /// Short summary / lede when the provider supplies one.
    pub summary: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::foundation::types::TimeSource;

    #[test]
    fn serde_round_trip() {
        let n = NewsItem {
            id: "bberg-12345".into(),
            symbol: Some("NVDA".into()),
            headline: "NVDA Beats Earnings Estimates".into(),
            source: "Bloomberg".into(),
            url: Some("https://example.com/n/12345".into()),
            published_at: Timestamp::from_millis(1_700_000_000_000, TimeSource::ExchangeUtc),
            summary: Some("Q3 EPS topped consensus.".into()),
        };
        let json = serde_json::to_string(&n).unwrap();
        let back: NewsItem = serde_json::from_str(&json).unwrap();
        assert_eq!(n, back);
    }
}
