//! Wave-1 connectivity foundation.
//!
//! Typed lifecycle/error surface for data feeds. Existing feeds (ApexData WS,
//! IB WS, Crypto, Signals) continue to work unchanged — this module only ADDS
//! the foundation. Migration is Wave 2's job.
//!
//! Submodules:
//! - `state`       — `ConnectionState`, `Connection`, `ConnectionMetrics`
//! - `error`       — `ApiError`, `AuthError`, `ConnectionError`
//! - `backoff`     — exponential backoff with jitter + max-attempts
//! - `errors_sink` — global error ring buffer + tracing + toast routing
//! - `telemetry`   — `init_tracing()` (file + stderr, non-blocking)
//! - `auth`        — `Authenticated` trait + `with_auth_retry` helper
//! - `shutdown`    — `Shutdown` trait + global drain registry

pub mod state;
pub mod error;
pub mod backoff;
pub mod errors_sink;
pub mod telemetry;
pub mod auth;
pub mod shutdown;

pub use state::{Connection, ConnectionMetrics, ConnectionState};
pub use error::{ApiError, AuthError, ConnectionError};
pub use backoff::Backoff;
pub use errors_sink::{drain_recent, report, ErrorLevel, ReportedError};
pub use telemetry::init_tracing;
pub use auth::{with_auth_retry, Authenticated};
pub use shutdown::{drain_all, register, Shutdown};
