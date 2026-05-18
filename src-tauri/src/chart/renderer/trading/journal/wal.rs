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
pub(crate) fn append(event: &JournalEvent) {
    let _g = WAL_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let path = wal_path();

    // Rotate if the existing file exceeds the threshold.
    if let Ok(meta) = std::fs::metadata(&path) {
        if meta.len() > ROTATE_BYTES {
            let _ = std::fs::remove_file(rotated_path());
            let _ = std::fs::rename(&path, rotated_path());
        }
    }

    let mut line = match serde_json::to_string(event) {
        Ok(s) => s,
        Err(e) => { eprintln!("[wal] serialize failed: {e}"); return; }
    };
    line.push('\n');

    let mut f = match OpenOptions::new().append(true).create(true).open(&path) {
        Ok(f) => f,
        Err(e) => { eprintln!("[wal] open failed: {e}"); return; }
    };
    if let Err(e) = f.write_all(line.as_bytes()) {
        eprintln!("[wal] write failed: {e}");
        return;
    }
    if let Err(e) = f.sync_data() {
        eprintln!("[wal] fsync failed: {e}");
    }
}

/// Read all events from the active WAL (oldest first). Used at startup for
/// orphan-attempt detection. Best effort — bad lines are skipped.
pub(crate) fn read_all() -> Vec<JournalEvent> {
    let path = wal_path();
    let bytes = match std::fs::read(&path) {
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
