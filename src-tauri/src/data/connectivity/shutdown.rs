//! Cooperative shutdown.
//!
//! Wave-1 wires the plumbing only — real `drain` bodies land per-feed in
//! later waves. The contract is:
//!
//! - At startup, every long-lived connection / pool calls
//!   `register("name", arc)` once.
//! - At exit, the entry point calls `drain_all(deadline)`.
//! - `drain` should: stop accepting new work, flush in-flight, close
//!   transports. It MUST return within the deadline — `drain_all` enforces a
//!   global cap via `tokio::time::timeout` over the joined set.

use async_trait::async_trait;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

#[async_trait]
pub trait Shutdown: Send + Sync {
    /// Stop accepting work, finish in-flight, close resources.
    /// `Ok` = drained cleanly within deadline. `Err(reason)` = caller may log.
    async fn drain(&self, deadline: Duration) -> Result<(), String>;
}

struct Entry {
    name: &'static str,
    handle: Arc<dyn Shutdown>,
}

fn registry() -> &'static Mutex<Vec<Entry>> {
    static R: std::sync::OnceLock<Mutex<Vec<Entry>>> = std::sync::OnceLock::new();
    R.get_or_init(|| Mutex::new(Vec::new()))
}

/// Register a connection / pool / worker for orderly shutdown.
///
/// Safe to call multiple times — entries are not deduplicated, so callers
/// should register exactly once during their setup phase.
pub fn register(name: &'static str, handle: Arc<dyn Shutdown>) {
    if let Ok(mut g) = registry().lock() {
        g.push(Entry { name, handle });
    }
}

/// Drain all registered handles concurrently, bounded by `deadline`.
///
/// Logs (via tracing) the outcome of each handle. Does NOT propagate
/// errors — shutdown is best-effort by design.
pub async fn drain_all(deadline: Duration) {
    let entries: Vec<Entry> = {
        let mut g = match registry().lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        // Take ownership so we don't hold the lock across awaits.
        std::mem::take(&mut *g)
    };

    if entries.is_empty() {
        return;
    }

    tracing::info!(target: "shutdown", count = entries.len(), "draining connections");

    let mut handles = Vec::with_capacity(entries.len());
    for Entry { name, handle } in entries {
        handles.push(tokio::spawn(async move {
            match tokio::time::timeout(deadline, handle.drain(deadline)).await {
                Ok(Ok(())) => tracing::info!(target: "shutdown", connection = name, "drained"),
                Ok(Err(e)) => tracing::warn!(target: "shutdown", connection = name, error = %e, "drain returned error"),
                Err(_)     => tracing::warn!(target: "shutdown", connection = name, "drain timed out"),
            }
        }));
    }

    for h in handles {
        let _ = h.await;
    }
}

/// No-op `Shutdown` impl — useful for early registrations where the real
/// drain logic will land in a later wave. Brief sleep ensures any in-flight
/// task at least gets a yield point before the process exits.
pub struct NoopShutdown {
    pub name: &'static str,
}

#[async_trait]
impl Shutdown for NoopShutdown {
    async fn drain(&self, _deadline: Duration) -> Result<(), String> {
        tokio::time::sleep(Duration::from_millis(10)).await;
        tracing::debug!(target: "shutdown", connection = self.name, "noop drain");
        Ok(())
    }
}
