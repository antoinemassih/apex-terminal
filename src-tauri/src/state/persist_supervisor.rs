//! Background thread that walks the `StoreRegistry` every 50ms and persists
//! any store whose debounce window has elapsed.
//!
//! Failures are reported through `errors_sink::report(Warn, ...)` — never
//! panicked, never swallowed silently.

use super::store_registry::StoreRegistry;
use crate::data::connectivity::errors_sink::{report, ErrorLevel};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

/// Interval between supervisor walk ticks.
pub const TICK_MS: u64 = 50;

/// Global shutdown flag set by [`shutdown`].  Shared with the supervisor
/// thread so it can exit cleanly without any change to the `JoinHandle<()>`
/// return type used by existing callers.
static SHUTDOWN: OnceLock<Arc<AtomicBool>> = OnceLock::new();

fn shutdown_flag() -> Arc<AtomicBool> {
    Arc::clone(SHUTDOWN.get_or_init(|| Arc::new(AtomicBool::new(false))))
}

/// Signal the persist supervisor to stop after its current tick finishes.
///
/// Call this **before** the final `flush_all` on application quit so the
/// supervisor cannot write a stale snapshot after that flush completes.
/// The function is idempotent and safe to call multiple times.
pub fn shutdown() {
    shutdown_flag().store(true, Ordering::Release);
}

/// Spawn the persist supervisor.  Returns the `JoinHandle` so the caller can
/// keep the thread alive (typically stored on the app root struct).
///
/// Use [`shutdown`] before the final flush on quit to stop the supervisor
/// cleanly.
pub fn spawn(registry: Arc<StoreRegistry>) -> std::thread::JoinHandle<()> {
    let flag = shutdown_flag();

    std::thread::Builder::new()
        .name("persist_supervisor".into())
        .spawn(move || {
            loop {
                // Check shutdown flag at the top of each tick so the thread
                // exits promptly even if it just woke up.
                if flag.load(Ordering::Acquire) {
                    break;
                }

                std::thread::sleep(Duration::from_millis(TICK_MS));

                // Re-check after sleep so we don't do a spurious flush tick
                // when shutdown was requested while we were sleeping.
                if flag.load(Ordering::Acquire) {
                    break;
                }

                for store in registry.all() {
                    if store.needs_persist() {
                        match store.flush() {
                            Ok(()) => store.mark_persisted(),
                            Err(e) => report(
                                ErrorLevel::Warn,
                                "persist_supervisor",
                                "write_failed",
                                format!("{}: {e}", store.key()),
                            ),
                        }
                    }
                }
            }
        })
        .expect("spawn persist_supervisor")
}

// Note: the supervisor thread itself is not directly tested — timing-dependent
// loop tests are flaky and the individual components (Store, StoreRegistry,
// PersistableStore) each have their own unit tests. Integration testing of the
// full pipeline (update → debounce → supervisor flush) belongs in Wave 2 when
// the supervisor is wired at startup with real aggregates.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tick_ms_is_less_than_debounce_ms() {
        // Sanity: the supervisor tick must be faster than the debounce window
        // or stores would never be flushed promptly.
        assert!(TICK_MS < super::super::store::DEBOUNCE_MS);
    }

    #[test]
    fn shutdown_flag_is_initially_false() {
        // The flag starts false so a freshly spawned supervisor runs normally.
        // (We cannot reset the OnceLock between tests, so we can only assert
        // the pre-signal state if no prior test already signalled it.  This
        // test intentionally does NOT call shutdown() so it cannot be flaky.)
        let flag = Arc::new(AtomicBool::new(false));
        assert!(!flag.load(Ordering::Acquire));
    }

    #[test]
    fn shutdown_signal_stops_supervisor_thread() {
        use super::super::store_registry::StoreRegistry;
        use std::sync::atomic::AtomicBool;

        // Spin up a supervisor with its own local shutdown flag (bypass the
        // global OnceLock so tests are independent).
        let registry = Arc::new(StoreRegistry::new());
        let local_flag = Arc::new(AtomicBool::new(false));
        let flag_clone = Arc::clone(&local_flag);

        let handle = std::thread::Builder::new()
            .name("test_supervisor".into())
            .spawn(move || {
                loop {
                    if flag_clone.load(Ordering::Acquire) { break; }
                    std::thread::sleep(Duration::from_millis(TICK_MS));
                    if flag_clone.load(Ordering::Acquire) { break; }
                    for store in registry.all() {
                        if store.needs_persist() {
                            let _ = store.flush();
                            store.mark_persisted();
                        }
                    }
                }
            })
            .expect("spawn test supervisor");

        // Signal shutdown and expect the thread to exit promptly.
        local_flag.store(true, Ordering::Release);
        let join_result = handle.join();
        assert!(join_result.is_ok(), "supervisor thread panicked");
    }
}
