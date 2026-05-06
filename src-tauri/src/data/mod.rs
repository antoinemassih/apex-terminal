pub mod bar_cache;
pub mod feeds;

// Re-export everything from foundation::data_types so `crate::data::Bar` etc. still work
pub use crate::foundation::data_types::*;

// Re-export sub-feeds at crate::data level for backward compat
pub use feeds::apex_data;
pub use feeds::crypto_feed;
pub use feeds::signals_feed;
pub use feeds::discord;

// ib_ws is pub(crate) / mod only — keep internal
pub(crate) use feeds::ib_ws;
