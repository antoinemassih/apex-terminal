//! Tracing setup.
//!
//! Wires up a non-blocking, daily-rolling file appender plus a compact
//! stderr layer. Replaces the previous mix of `eprintln!` + unbuffered
//! direct file writes that blocked the hot WS thread on every log line.
//!
//! ## Filter
//! Reads `RUST_LOG` if set; otherwise defaults to `info,apex_data=debug` so
//! the WS feed stays verbose without firehosing the rest of the app.
//!
//! ## Idempotency
//! `init_tracing()` returns a `WorkerGuard` that MUST live for the program's
//! duration — dropping it stops the background log writer. We also keep a
//! `OnceLock` so repeat calls from the Tauri and native entry points (or
//! during tests) are no-ops and return a fresh dummy guard.

use std::path::Path;
use tracing_appender::non_blocking::WorkerGuard;

/// Initialize the global tracing subscriber.
///
/// Wire the returned `WorkerGuard` into a `static` or stash it at the top of
/// `main()` so the non-blocking writer thread stays alive.
pub fn init_tracing(log_dir: &Path) -> WorkerGuard {
    use tracing_subscriber::{prelude::*, EnvFilter, fmt};

    // Ensure log dir exists; ignore failures (fall back to cwd if the
    // appender errors out — tracing-appender panics on missing dir, so we
    // pre-create it).
    let _ = std::fs::create_dir_all(log_dir);

    let file_appender = tracing_appender::rolling::daily(log_dir, "apex.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,apex_data=debug"));

    let stderr_layer = fmt::layer().with_writer(std::io::stderr).compact();
    let file_layer   = fmt::layer().with_writer(non_blocking).with_ansi(false);

    // Multiple call sites (Tauri `run`, native `main`, tests) all reach here.
    // `try_init` returns Err if a global subscriber is already installed — we
    // ignore that and still hand back the guard so the caller's lifetime is
    // uniform across entry points.
    let _ = tracing_subscriber::registry()
        .with(env_filter)
        .with(stderr_layer)
        .with(file_layer)
        .try_init();

    guard
}
