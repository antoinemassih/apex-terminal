//! Unified market-data provider layer.
//!
//! One trait (`MarketDataProvider`), one refcounted/deduped `SubscriptionManager`,
//! and composable adapters for fallback / cache / replay.
//!
//! ## What this layer owns (and what it deliberately does NOT)
//!
//! It is the authority for:
//!  1. **Historical / fallback bar fetch** — `registry::bar_chain()` is a
//!     `FallbackProvider` (crypto → cached(apex_data) → OCOCO → yfinance →
//!     Yahoo); `fetch_bars_background` drains the first non-empty source.
//!  2. **Live upstream subscription** — `subscribe_bars()` routes to
//!     `provider.subscribe_bars()`, which for apex_data calls
//!     `ws::add_bar_sub(symbol, timeframe)` — i.e. this is what actually tells
//!     the firehose to stream a symbol's bars.
//!  3. **Gap-fill anchoring** — `apex_data`'s frame listener + each subscription's
//!     pump keep `last_seen_ts` current so `gap_fill_on_reconnect_all()` (called
//!     by every WS feed on reconnect) knows where to replay from.
//!
//! It does NOT deliver live bars to the chart UI. That intentionally flows
//! through the single `NATIVE_CHART_TXS` hot path (the `lib.rs` frame listener →
//! `ChartCommand`), so there is exactly one delivery route and no per-frame
//! fanout duplication. Consequently the `broadcast::Receiver` returned by the
//! live `subscribe_*` calls is intentionally dropped at the call site — the
//! *subscription* (items 2 + 3) is the point, not the receiver. This split is
//! deliberate; do not "finish wiring" the receivers into the UI (it would create
//! a redundant second delivery path) nor delete the subscribe calls (it would
//! stop upstream subscription + break gap-fill).

pub mod provider;
pub mod subscription_manager;
pub mod fallback;
pub mod cached;
pub mod apex_data;
pub mod ib;
pub mod crypto;
pub mod signals;
pub mod http_fallback;
pub mod registry;
pub mod mock;

pub use provider::{
    BarStream, ChainSnapshot, ChainStream, MarketDataProvider, ProviderCapabilities,
    QuoteStream, TradeStream,
};
pub use subscription_manager::SubscriptionManager;
pub use fallback::FallbackProvider;
pub use cached::CachedProvider;
pub use http_fallback::HttpFallbackProvider;
pub use mock::{MockFrame, MockMarketDataProvider};
