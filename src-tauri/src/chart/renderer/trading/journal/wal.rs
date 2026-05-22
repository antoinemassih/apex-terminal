//! Append-only Write-Ahead Log file at `{state_dir}/orders.wal`.
//!
//! Each event is one line of JSON, fsync'd after write. On hitting 10 MB, the
//! file is rotated to `orders.wal.1` (one rotation kept). This module is
//! intentionally minimal — the goal is durability of intent, not transactional
//! correctness.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

use super::events::JournalEvent;
use crate::data::connectivity::errors_sink::{report, ErrorLevel};

const ROTATE_BYTES: u64 = 10 * 1024 * 1024;

/// Serialize WAL writes so concurrent threads don't interleave bytes.
static WAL_LOCK: Mutex<()> = Mutex::new(());

/// Resolve the WAL file path.
///
/// If the `APEX_WAL_PATH` environment variable is set, its value is used
/// directly (parent directory is created if missing). This is intended for
/// tests so they can point at a `tempfile::TempDir` instead of trampling the
/// developer machine's `state/orders.wal`.
///
/// Otherwise falls back to `{exe_dir}/state/orders.wal` — the production
/// default that existed before env-var threading.
pub(crate) fn wal_path() -> PathBuf {
    if let Ok(override_path) = std::env::var("APEX_WAL_PATH") {
        let p = PathBuf::from(override_path);
        if let Some(parent) = p.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        return p;
    }
    let dir = std::env::current_exe().ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."));
    let state = dir.join("state");
    let _ = std::fs::create_dir_all(&state);
    state.join("orders.wal")
}

fn rotated_path() -> PathBuf {
    let mut p = wal_path();
    p.set_file_name("orders.wal.1");
    p
}

/// Append a single event line and fsync. Errors logged to stderr only — never
/// block the calling order operation.
///
/// # Wave 8 Medium — WAL_LOCK critical section
///
/// `WAL_LOCK` is held across the full open → write_all → sync_data sequence.
/// An fsync on a spinning disk or a busy NFS mount can take tens of milliseconds,
/// during which no other thread can append to the WAL. This is the accepted
/// trade-off for this pass: WAL writes are infrequent (one per order event) and
/// the ORDER_MANAGER mutex is NOT held here (CC1 ensures all journal::append
/// calls happen after the ORDER_MANAGER guard drops), so the only contention is
/// between concurrent WAL writers.
///
/// **Remaining risk**: multiple background threads (reconciler, broker watchdog,
/// spawned submit threads) can queue up on WAL_LOCK simultaneously during a busy
/// reconcile sweep.  A dedicated WAL writer thread fed by a channel would
/// eliminate this bottleneck, but is deferred to a future pass because it
/// requires a graceful-shutdown drain to guarantee durability.
///
/// Mitigation applied here:
///   - JSON serialization is done BEFORE acquiring WAL_LOCK so the lock is held
///     only for the I/O operations (previously it was also held during
///     serde_json::to_string).
///   - The lock is released immediately on any error path (no unwinding under lock).
pub(crate) fn append(event: &JournalEvent) {
    // Serialize BEFORE acquiring the lock — reduces critical section by the
    // serde work (Wave 8 Medium partial mitigation).
    let mut line = match serde_json::to_string(event) {
        Ok(s) => s,
        Err(e) => { report(ErrorLevel::Error, "wal", "serialize_failed", e.to_string()); return; }
    };
    line.push('\n');

    // KNOWN RISK (Wave 8 Medium): WAL_LOCK is held across open + write_all +
    // sync_data. An fsync stall blocks concurrent WAL writers for its duration.
    // ORDER_MANAGER is NOT held here (CC1 fix ensures that), so order operations
    // are unaffected. See doc comment above for the full analysis.
    let _g = WAL_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let path = wal_path();

    // Rotate if the existing file exceeds the threshold.
    if let Ok(meta) = std::fs::metadata(&path) {
        if meta.len() > ROTATE_BYTES {
            let _ = std::fs::remove_file(rotated_path());
            let _ = std::fs::rename(&path, rotated_path());
        }
    }

    let mut f = match OpenOptions::new().append(true).create(true).open(&path) {
        Ok(f) => f,
        Err(e) => { report(ErrorLevel::Error, "wal", "open_failed", e.to_string()); return; }
    };
    if let Err(e) = f.write_all(line.as_bytes()) {
        report(ErrorLevel::Error, "wal", "write_failed", e.to_string());
        return;
    }
    if let Err(e) = f.sync_data() {
        report(ErrorLevel::Error, "wal", "fsync_failed", e.to_string());
    }
}

/// Read all events from the active WAL and the rotated WAL (`orders.wal.1`),
/// returning them merged in chronological order (rotated first, active second).
/// Used at startup for orphan-attempt detection. Best effort — bad lines are
/// skipped.
///
/// Rationale: when the WAL rolls over to `orders.wal.1`, the Attempt that
/// created an orphan may be in the old file while the matching Ack/Fail is
/// still in the new file (or vice versa).  Reading both files prevents false
/// positives where an acknowledged order looks orphaned just because the Ack
/// was written to the current file after rotation.
pub(crate) fn read_all() -> Vec<JournalEvent> {
    fn parse_file(path: &std::path::Path) -> Vec<JournalEvent> {
        let bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(_) => return Vec::new(),
        };
        let text = match std::str::from_utf8(&bytes) {
            Ok(t) => t,
            Err(_) => return Vec::new(),
        };
        text.lines()
            .filter(|l| !l.trim().is_empty())
            .filter_map(|l| serde_json::from_str::<JournalEvent>(l).ok())
            .collect()
    }

    // Rotated file is older — prepend it so events are in chronological order.
    let mut events = parse_file(&rotated_path());
    events.extend(parse_file(&wal_path()));
    events
}
