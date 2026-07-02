//! Panic isolation (audit A4).
//!
//! Two gaps this closes for RELEASE builds:
//!  1. There was no release panic hook — the only hook lived in `dev_inspector`
//!     (debug-only), so release panics died silently to stderr.
//!  2. Worker threads (`std::thread::spawn` for fetches/feeds/broker ops) had no
//!     `catch_unwind` — a panic in one worker poisoned any shared `Mutex` it
//!     held and cascade-killed consumers. `spawn_guarded` contains the blast
//!     radius to the one worker and surfaces the panic through `errors_sink`.
//!
//! Note: the OrderManager path already recovers from poisoned locks
//! (`with_mgr` extracts inner data on poison), so containment + observability —
//! not lock hygiene — is the remaining risk this module addresses.

use std::panic::{self, AssertUnwindSafe};

/// Install a process-wide panic hook that reports through tracing +
/// errors_sink in ALL builds (release included), then delegates to the
/// previous hook (so the debug dev_inspector hook still fires).
pub fn install_release_panic_hook() {
    let previous = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        let msg = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "non-string panic payload".to_string());
        let loc = info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_else(|| "unknown".to_string());
        let thread = std::thread::current().name().unwrap_or("unnamed").to_string();
        tracing::error!(target: "panic", thread = %thread, location = %loc, "PANIC: {msg}");
        crate::data::connectivity::errors_sink::report(
            crate::data::connectivity::errors_sink::ErrorLevel::Error,
            "panic",
            "thread_panic",
            format!("[{thread}] {msg} @ {loc}"),
        );
        previous(info);
    }));
}

/// Spawn a fire-and-forget worker whose panics are CONTAINED: the closure runs
/// under `catch_unwind`, so a panic kills only this worker (reported via the
/// panic hook + an errors_sink entry) instead of unwinding into poisoned-lock
/// cascades. Use for every background fetch/feed/broker worker.
pub fn spawn_guarded<F>(name: &'static str, f: F) -> std::thread::JoinHandle<()>
where
    F: FnOnce() + Send + 'static,
{
    std::thread::Builder::new()
        .name(name.to_string())
        .spawn(move || {
            if panic::catch_unwind(AssertUnwindSafe(f)).is_err() {
                // The hook already logged the panic itself; add the containment note.
                crate::data::connectivity::errors_sink::report(
                    crate::data::connectivity::errors_sink::ErrorLevel::Error,
                    "panic",
                    "worker_contained",
                    format!("worker '{name}' died from a panic (contained; app unaffected)"),
                );
            }
        })
        .unwrap_or_else(|e| panic!("failed to spawn guarded thread '{name}': {e}"))
}
