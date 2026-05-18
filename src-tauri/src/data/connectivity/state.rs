//! Typed connection lifecycle state.
//!
//! Replaces ad-hoc boolean flags (`is_connected`, `is_authenticated`, …) with
//! a single enum that captures every meaningful phase including transient
//! backoff. The `Connection` trait gives the UI / observability layer a
//! uniform handle over every feed (ApexData WS, IB WS, Crypto, Signals, …).
//!
//! ## Polling vs subscription
//!
//! Consumers have two ways to observe lifecycle changes:
//!
//! - **Polling — `state()`**: cheap one-shot read. Right pattern for a per-frame
//!   status dot, a `status_for_metrics()` snapshot, or anywhere you don't
//!   actually need to react to *transitions*.
//! - **Subscription — `subscribe_state()`**: returns
//!   `Option<broadcast::Receiver<ConnectionState>>`. Each receiver is its own
//!   independent cursor over the state-transition stream (tokio broadcast
//!   semantics — every subscriber sees every value, with a per-receiver lag
//!   counter when the queue overflows). Right pattern for a side-panel that
//!   wants to flash on reconnect, a diagnostics tab that logs every
//!   transition, or a toast on terminal `Failed`.
//!
//! Adapters that don't have a real state machine return `None` from
//! `subscribe_state()` — callers must fall back to polling. Wave 11c wires
//! push notification into `ApexDataProvider`, `IbProvider`, `CryptoProvider`,
//! `SignalsProvider`, and `MockMarketDataProvider`. `ReplayProvider` is
//! intentionally polling-only (no real lifecycle).

use std::time::Instant;

use super::error::ConnectionError;

/// Lifecycle of any long-lived data connection.
///
/// Designed so a single dot in the status bar can render unambiguously:
/// `Authenticated`/`Subscribed` → green, `Connecting`/`Backoff` → amber,
/// `Failed` → red, `Idle`/`ShuttingDown` → dim.
#[derive(Debug, Clone)]
pub enum ConnectionState {
    /// Nothing in flight — initial state / after clean shutdown.
    Idle,
    /// TCP/WS dial in progress. `attempt` starts at 1.
    Connecting { attempt: u32 },
    /// Handshake + auth complete, no subscriptions yet.
    Authenticated,
    /// At least one subscription active. `count` is the live sub count.
    Subscribed { count: usize },
    /// Waiting before next reconnect attempt.
    Backoff { until: Instant, attempt: u32, reason: String },
    /// Terminal failure — caller must intervene (refresh token, fix config…).
    Failed { reason: ConnectionError },
    /// Drain in progress; no new work accepted.
    ShuttingDown,
}

/// Per-connection counters surfaced to the UI / metrics.
///
/// Cheap to construct each call — connection impls usually load atomics into
/// the struct on demand rather than holding it long-term.
#[derive(Debug, Clone, Default)]
pub struct ConnectionMetrics {
    pub messages_in: u64,
    pub messages_out: u64,
    pub parse_errors: u64,
    pub reconnect_count: u32,
    pub last_message_at: Option<Instant>,
    /// Outbound send queue depth, if the transport exposes one.
    pub queue_depth: Option<usize>,
}

/// Common surface every long-lived feed connection implements.
///
/// Implementations are typically `Arc<Self>` so the registry / UI can hold
/// observer handles without owning the feed.
pub trait Connection: Send + Sync {
    /// Stable identifier, e.g. `"apex_data"`, `"ib_ws"`, `"crypto"`, `"signals"`.
    fn name(&self) -> &str;
    fn state(&self) -> ConnectionState;
    fn metrics(&self) -> ConnectionMetrics;

    /// Optional: subscribe to state-change events. Providers that support
    /// push notification return `Some(receiver)` — others return `None` and
    /// callers fall back to polling [`state`](Self::state).
    ///
    /// Each subscription gets its own `broadcast::Receiver` cursor. Cheap to
    /// hold, cheap to clone (each call returns a fresh receiver). Receivers
    /// that fall behind surface a `RecvError::Lagged` so callers can decide
    /// whether to re-sync via `state()` or just drop the stale snapshot.
    fn subscribe_state(&self) -> Option<tokio::sync::broadcast::Receiver<ConnectionState>> {
        None
    }
}
