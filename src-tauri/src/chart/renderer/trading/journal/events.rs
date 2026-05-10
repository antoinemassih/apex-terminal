//! Journal event variants — append-only WAL records.

use crate::chart::renderer::trading::order_manager::OrderState;

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub(crate) enum AttemptKind { Submit, Cancel, CancelAll, Modify }

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub(crate) enum ControlKind { Kill, ReleaseKill, Halt, Resume }

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub(crate) enum JournalEvent {
    Attempt {
        client_id: String,
        kind: AttemptKind,
        ts_ms: u64,
        payload: serde_json::Value,
    },
    Ack {
        client_id: String,
        backend_id: Option<String>,
        ts_ms: u64,
    },
    Fail {
        client_id: String,
        reason: String,
        ts_ms: u64,
    },
    StateChg {
        client_id: String,
        from: OrderState,
        to: OrderState,
        ts_ms: u64,
    },
    Reconcile {
        client_id: String,
        local: OrderState,
        broker: String,
        resolution: String,
        ts_ms: u64,
    },
    Control {
        kind: ControlKind,
        ts_ms: u64,
    },
    /// Marker emitted on graceful shutdown via `OrderManager::Drop`.
    /// On replay, if the last event is `Shutdown` the previous run exited cleanly
    /// and we can skip orphan-attempt recovery.
    Shutdown {
        ts_ms: u64,
    },
}

impl JournalEvent {
    pub(crate) fn client_id(&self) -> &str {
        match self {
            Self::Attempt { client_id, .. }
            | Self::Ack { client_id, .. }
            | Self::Fail { client_id, .. }
            | Self::StateChg { client_id, .. }
            | Self::Reconcile { client_id, .. } => client_id,
            Self::Control { .. } => "<control>",
            Self::Shutdown { .. } => "<shutdown>",
        }
    }
}
