//! Watchlist persistence — phase (c) of the watchlist refactor.
//!
//! Schema lives in `migrations/002_watchlist_state.sql`. Sync-facing API
//! is in `crate::persistence::watchlist_db`. The async sqlx round-trip
//! lives in `codec::db`.

pub mod codec;
pub mod refresh;
