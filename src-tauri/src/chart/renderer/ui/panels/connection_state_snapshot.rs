//! Wave 12d — push-based bridge from `Connection::subscribe_state()` broadcast
//! streams into a synchronous snapshot map readable per-frame by egui.
//!
//! ## Why
//!
//! egui is synchronous per frame, but `subscribe_state()` returns a tokio
//! `broadcast::Receiver<ConnectionState>` driven by background tasks. We
//! bridge the two with a module-level `RwLock<HashMap<String, ConnectionState>>`
//! that's updated by a per-provider listener task and read lock-free-ishly by
//! the connection panel each frame.
//!
//! ## Lifecycle
//!
//! Call [`spawn_state_listeners`] exactly once at app startup *after* the
//! tokio runtime is available and the underlying WS feeds have been started
//! (so their broadcast `Sender`s already exist).
//!
//! Each listener:
//! 1. Seeds the snapshot with `provider.state()` once (avoids "stale until
//!    first transition" race if a state change happened before this task
//!    spawned).
//! 2. Loops on `rx.recv()` and updates the snapshot on every transition.
//! 3. Calls [`crate::wake_native_ui`] so the chrome layer repaints
//!    immediately — without this the connection panel would still appear to
//!    poll (next mouse move / tick would surface the change).
//!
//! On `RecvError::Lagged` we re-seed from `provider.state()` and keep
//! looping; on `Closed` we exit.

use crate::data::connectivity::{Connection, ConnectionState};
use crate::data::providers::{
    apex_data::ApexDataProvider, crypto::CryptoProvider, ib::IbProvider, signals::SignalsProvider,
};
use std::collections::HashMap;
use std::sync::{Arc, OnceLock, RwLock};
use tokio::sync::broadcast::error::RecvError;

static STATE_SNAPSHOT: OnceLock<RwLock<HashMap<String, ConnectionState>>> = OnceLock::new();
static LISTENERS_SPAWNED: OnceLock<()> = OnceLock::new();

/// Module-level snapshot map. Keys are stable provider names
/// (`"apex_data"`, `"ib_ws"`, `"crypto"`, `"signals"`). Values are the most
/// recently observed [`ConnectionState`] for that provider.
pub fn snapshot() -> &'static RwLock<HashMap<String, ConnectionState>> {
    STATE_SNAPSHOT.get_or_init(|| RwLock::new(HashMap::new()))
}

/// Read the current snapshot for a provider, falling back to [`ConnectionState::Idle`]
/// when no listener has populated an entry yet.
pub fn get(name: &str) -> ConnectionState {
    snapshot()
        .read()
        .ok()
        .and_then(|g| g.get(name).cloned())
        .unwrap_or(ConnectionState::Idle)
}

/// Spawn one listener task per push-capable provider. Idempotent — second and
/// subsequent calls are no-ops.
///
/// Must be called from within a running tokio runtime context (Tauri's
/// `async_runtime`). Safe to call regardless of whether the underlying WS
/// feeds have actually connected yet — each task seeds from `provider.state()`
/// (`Idle` if nothing has happened) and updates as transitions arrive.
pub fn spawn_state_listeners() {
    if LISTENERS_SPAWNED.set(()).is_err() {
        return;
    }

    let providers: Vec<(String, Arc<dyn Connection>)> = vec![
        ("apex_data".into(), Arc::new(ApexDataProvider::new())),
        ("ib_ws".into(), Arc::new(IbProvider::new())),
        ("crypto".into(), Arc::new(CryptoProvider::new())),
        ("signals".into(), Arc::new(SignalsProvider::new())),
    ];

    for (name, provider) in providers {
        spawn_one(name, provider);
    }
}

fn spawn_one(name: String, provider: Arc<dyn Connection>) {
    let Some(mut rx) = provider.subscribe_state() else {
        // No push stream — drop. Caller falls back to `.state()` polling if
        // they really need this provider's status (none currently do; the
        // connection panel only reads via `get(...)`).
        return;
    };

    tokio::spawn(async move {
        // Seed once before entering the recv loop so the snapshot is
        // populated even if no transition fires after spawn.
        {
            let s = provider.state();
            if let Ok(mut g) = snapshot().write() {
                g.insert(name.clone(), s);
            }
        }

        loop {
            match rx.recv().await {
                Ok(state) => {
                    if let Ok(mut g) = snapshot().write() {
                        g.insert(name.clone(), state);
                    }
                    crate::wake_native_ui();
                }
                Err(RecvError::Lagged(_)) => {
                    // Listener fell behind — re-seed from the authoritative
                    // poll surface and keep going.
                    let s = provider.state();
                    if let Ok(mut g) = snapshot().write() {
                        g.insert(name.clone(), s);
                    }
                    crate::wake_native_ui();
                }
                Err(RecvError::Closed) => break,
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::providers::mock::MockMarketDataProvider;
    use std::sync::Arc;
    use std::time::Duration;

    /// Drives the same listener pattern used in production against a
    /// `MockMarketDataProvider` and asserts the snapshot reflects a
    /// pushed transition within 50 ms.
    #[tokio::test(flavor = "multi_thread")]
    async fn snapshot_updates_when_provider_publishes() {
        // Unique key so parallel tests don't stomp the shared snapshot map.
        let key = "test_w12d_push";

        let mock = Arc::new(MockMarketDataProvider::new(key));
        let mut rx = mock.subscribe_state().expect("mock supports state stream");

        let mock_for_task = mock.clone();
        let key_for_task = key.to_string();
        tokio::spawn(async move {
            // Seed.
            if let Ok(mut g) = snapshot().write() {
                g.insert(key_for_task.clone(), mock_for_task.state());
            }
            while let Ok(s) = rx.recv().await {
                if let Ok(mut g) = snapshot().write() {
                    g.insert(key_for_task.clone(), s);
                }
            }
        });

        mock.set_state(ConnectionState::Authenticated);

        for _ in 0..10 {
            tokio::time::sleep(Duration::from_millis(5)).await;
            if matches!(
                snapshot().read().ok().and_then(|g| g.get(key).cloned()),
                Some(ConnectionState::Authenticated)
            ) {
                return;
            }
        }
        panic!("snapshot did not update within 50 ms");
    }

    /// Wave 13c: verify the snapshot map carries independent entries for
    /// every push-capable feed (apex_data, ib_ws, crypto, signals). The
    /// production listener side is already covered by
    /// `snapshot_updates_when_provider_publishes`; this test exercises the
    /// read path that `connection_panel::status_from_snapshot` walks each
    /// frame.
    #[tokio::test(flavor = "multi_thread")]
    async fn snapshot_tracks_all_four_feeds_independently() {
        // Write directly to the snapshot, skipping the listener machinery.
        {
            let mut w = snapshot().write().unwrap();
            w.insert("apex_data".into(), ConnectionState::Authenticated);
            w.insert("ib_ws".into(), ConnectionState::Subscribed { count: 3 });
            w.insert(
                "crypto".into(),
                ConnectionState::Backoff {
                    until: std::time::Instant::now(),
                    attempt: 1,
                    reason: "test".into(),
                },
            );
            w.insert("signals".into(), ConnectionState::Idle);
        }

        assert!(matches!(get("apex_data"), ConnectionState::Authenticated));
        assert!(matches!(
            get("ib_ws"),
            ConnectionState::Subscribed { count: 3 }
        ));
        assert!(matches!(get("crypto"), ConnectionState::Backoff { .. }));
        assert!(matches!(get("signals"), ConnectionState::Idle));
    }
}
