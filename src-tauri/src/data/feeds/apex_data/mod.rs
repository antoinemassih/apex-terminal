//! ApexData integration — real-time and historical market data client.
//!
//! Mirrors the contract in `ApexData/docs/FRONTEND_INTEGRATION.md` (spec v1).
//!
//! Layout:
//! - `config`  — base URL + auth token, runtime-overridable
//! - `types`   — data models (§4 of the spec)
//! - `rest`    — blocking REST client (§5 endpoints)
//! - `ws`      — WebSocket subscription manager (§6 protocol)
//!
//! The module is transport-agnostic for callers: give it a `(symbol, timeframe)`
//! and you get bars (once) + optional live updates routed through
//! `ChartCommand::AppendBar` / `UpdateLastBar`.

pub mod config;
pub mod types;
pub mod rest;
pub mod ws;
pub mod live_state;
pub mod debug_log;

pub use config::{apex_url, apex_ws_url, apex_token, set_apex_url, set_apex_token, is_enabled, set_enabled};
pub use types::*;
