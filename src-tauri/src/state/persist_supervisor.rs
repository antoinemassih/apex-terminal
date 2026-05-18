//! Background thread that walks the `StoreRegistry` every 50ms and persists
//! any store whose debounce window has elapsed.
//!
//! Failures are reported through `errors_sink::report(Warn, ...)` — never
//! panicked, never swallowed silently.

use super::store_registry::StoreRegistry;
use crate::data::connectivity::errors_sink::{report, ErrorLevel};
use std::sync::Arc;
use std::time::Duration;

/// Interval between supervisor walk ticks.
pub const TICK_MS: u64 = 50;

/// Spawn the persist supervisor. Returns the `JoinHandle` so the caller can
/// keep the thread alive (typically stored on the app root struct).
///
/// The thread runs until the process exits. There is intentionally no
/// shutdown channel — the OS will clean it up at process exit, and adding a
/// shutdown path would complicate startup wiring for no user benefit.
pub fn spawn(registry: Arc<StoreRegistry>) -> std::thread::JoinHandle<()> {
    std::thread::Builder::new()
        .name("persist_supervisor".into())
        .spawn(move || {
            loop {
                std::thread::sleep(Duration::from_millis(TICK_MS));
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
}
