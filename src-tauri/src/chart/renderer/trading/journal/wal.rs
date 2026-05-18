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

pub(crate) fn wal_path() -> PathBuf {
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
        Err(e) => { report(ErrorLevel::Error, "wal", "serialize_failed", e.to_string()); return; }
    };
    line.push('\n');

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
