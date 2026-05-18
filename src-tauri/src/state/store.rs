//! `Store<T>` — typed, lock-protected, versioned, debounce-persisted state container.
//!
//! Wraps a `Persistable` aggregate (e.g., `UiSettings`, `TradingDefaults`) with:
//! - `parking_lot::RwLock` interior — cheap, no syscall when uncontested
//! - `AtomicU64` version counter — render loop dirty-check
//! - debounced persistence — at most one disk write per 200ms per store
//!
//! Reading from a store is constant-time:
//! ```ignore
//! let cached_version = ...; // stored on the Watchlist or local
//! if store.version() != cached_version {
//!     let snapshot = store.read().clone();
//!     // ... use snapshot
//!     cached_version = store.version();
//! }
//! ```
//!
//! Writing is single-mutation:
//! ```ignore
//! store.update(|s| { s.field = new_value; });
//! ```
//!
//! After `update()` returns, the version counter bumps and the debounce clock
//! starts. The background `persist_supervisor` thread (spawned at startup)
//! checks every 50ms and persists any store whose `needs_persist()` returns true.
//!
//! ## Anti-patterns (audit-flagged)
//!
//! - **No callbacks inside `update()`** that mutate other stores. Use the
//!   `SubscriptionBus` (`PaneEvent`) to fan out cross-store changes — never
//!   take a write lock on store B while holding a write lock on store A.
//! - **No `ctx.request_repaint()` inside stores.** egui paints every frame;
//!   the render loop dirty-checks via `version()`.
//! - **No `Store<AppState>` god-object.** Stores are flat and typed by concern.

use parking_lot::{Mutex, RwLock, RwLockReadGuard};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// Default debounce window: at most one disk write per 200ms per store.
pub const DEBOUNCE_MS: u64 = 200;

pub struct Store<T: Clone + Send + Sync + 'static> {
    inner: RwLock<T>,
    version: AtomicU64,
    pub(super) persist_path: Option<PathBuf>,
    last_mutated: Mutex<Option<Instant>>,
    key: &'static str,  // for telemetry / errors_sink
}

impl<T: Clone + Send + Sync + 'static> Store<T> {
    pub fn new(key: &'static str, value: T, persist_path: Option<PathBuf>) -> Arc<Self> {
        Arc::new(Self {
            inner: RwLock::new(value),
            version: AtomicU64::new(0),
            persist_path,
            last_mutated: Mutex::new(None),
            key,
        })
    }

    pub fn key(&self) -> &'static str { self.key }

    pub fn version(&self) -> u64 { self.version.load(Ordering::Acquire) }

    pub fn read(&self) -> RwLockReadGuard<T> { self.inner.read() }

    /// Single-mutation API. Bumps the version counter and sets the debounce clock.
    /// Does NOT persist synchronously — that's the supervisor's job.
    pub fn update(&self, f: impl FnOnce(&mut T)) {
        {
            let mut guard = self.inner.write();
            f(&mut *guard);
        }
        self.version.fetch_add(1, Ordering::Release);
        *self.last_mutated.lock() = Some(Instant::now());
    }

    /// True iff the debounce window has elapsed since the last `update()`.
    pub fn needs_persist(&self) -> bool {
        self.last_mutated
            .lock()
            .map(|t| t.elapsed() >= Duration::from_millis(DEBOUNCE_MS))
            .unwrap_or(false)
    }

    pub fn mark_persisted(&self) {
        *self.last_mutated.lock() = None;
    }

    /// Read the inner value and clone it (convenience for flush implementations).
    pub(super) fn read_clone(&self) -> T {
        self.inner.read().clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[derive(Clone, Debug, PartialEq)]
    struct Counter {
        value: u32,
    }

    fn make_store(v: u32) -> Arc<Store<Counter>> {
        Store::new("test_counter", Counter { value: v }, None)
    }

    #[test]
    fn update_bumps_version() {
        let store = make_store(0);
        assert_eq!(store.version(), 0);
        store.update(|c| c.value += 1);
        assert_eq!(store.version(), 1);
        store.update(|c| c.value += 1);
        assert_eq!(store.version(), 2);
    }

    #[test]
    fn read_returns_updated_value() {
        let store = make_store(42);
        assert_eq!(store.read().value, 42);
        store.update(|c| c.value = 99);
        assert_eq!(store.read().value, 99);
    }

    #[test]
    fn needs_persist_false_immediately_after_update() {
        let store = make_store(0);
        store.update(|c| c.value = 1);
        // Immediately after update, debounce window has NOT elapsed.
        assert!(!store.needs_persist());
    }

    #[test]
    fn needs_persist_false_before_any_update() {
        let store = make_store(0);
        assert!(!store.needs_persist());
    }

    #[test]
    fn needs_persist_true_after_debounce_window() {
        let store = make_store(0);
        store.update(|c| c.value = 1);
        // Sleep past the debounce window.
        thread::sleep(Duration::from_millis(DEBOUNCE_MS + 50));
        assert!(store.needs_persist());
    }

    #[test]
    fn mark_persisted_clears_pending() {
        let store = make_store(0);
        store.update(|c| c.value = 1);
        thread::sleep(Duration::from_millis(DEBOUNCE_MS + 50));
        assert!(store.needs_persist());
        store.mark_persisted();
        assert!(!store.needs_persist());
    }

    #[test]
    fn key_returns_static_str() {
        let store = make_store(0);
        assert_eq!(store.key(), "test_counter");
    }

    #[test]
    fn persist_path_none_when_not_set() {
        let store = make_store(0);
        assert!(store.persist_path.is_none());
    }

    #[test]
    fn persist_path_some_when_set() {
        let path = PathBuf::from("/tmp/test.json");
        let store: Arc<Store<Counter>> = Store::new("k", Counter { value: 0 }, Some(path.clone()));
        assert_eq!(store.persist_path.as_deref(), Some(path.as_path()));
    }
}
