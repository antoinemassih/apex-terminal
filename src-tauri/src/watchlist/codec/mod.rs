//! Postgres codec for watchlists and symbol universes.
//!
//! See `migrations/002_watchlist_state.sql` for the schema. The renderer
//! never calls these functions directly — it goes through the sync
//! `crate::persistence::watchlist_db` worker.

pub mod db;
