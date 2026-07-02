//! Order WAL — append-only journal of order intents and state transitions.
//!
//! Public API:
//!   - `append(event)`  — write one event, fsync'd
//!   - `replay()`       — read every event in the active WAL
//!   - `find_orphans()` — Attempts with no matching Ack/Fail (for startup audit)

pub(crate) mod events;
pub(crate) mod wal;

pub(crate) use events::{AttemptKind, ControlKind, JournalEvent};

use std::collections::{HashSet, VecDeque};
use std::sync::{Mutex, OnceLock};
use crate::data::connectivity::errors_sink::{report, ErrorLevel};

/// In-memory ring of recent journal events for UI tail. Capacity is generous —
/// the order ledger panel reads up to ~200 at a time.
const RING_CAPACITY: usize = 1024;

fn ring() -> &'static Mutex<VecDeque<JournalEvent>> {
    static RING: OnceLock<Mutex<VecDeque<JournalEvent>>> = OnceLock::new();
    RING.get_or_init(|| {
        // Seed the ring from the WAL on first access so the panel shows
        // history immediately after a restart.
        let mut dq: VecDeque<JournalEvent> = VecDeque::with_capacity(RING_CAPACITY);
        let recent = wal::read_all();
        let start = recent.len().saturating_sub(RING_CAPACITY);
        for ev in recent.into_iter().skip(start) {
            dq.push_back(ev);
        }
        Mutex::new(dq)
    })
}

/// Append a journal event. Fire-and-forget — failures log to stderr only.
pub(crate) fn append(event: JournalEvent) {
    wal::append(&event);
    if let Ok(mut g) = ring().lock() {
        if g.len() >= RING_CAPACITY { g.pop_front(); }
        g.push_back(event);
    }
}

/// Snapshot of the most recent `n` journal events (oldest first).
/// Reads from the in-memory ring buffer; never touches disk.
pub(crate) fn tail(n: usize) -> Vec<JournalEvent> {
    let g = match ring().lock() { Ok(g) => g, Err(p) => p.into_inner() };
    let len = g.len();
    let start = len.saturating_sub(n);
    g.iter().skip(start).cloned().collect()
}

/// Read every event in the active WAL (oldest first).
pub(crate) fn replay() -> Vec<JournalEvent> {
    wal::read_all()
}

/// True if the most recent event in the WAL is a `Shutdown` marker.
/// Used by startup recovery to skip orphan reconciliation when the previous
/// run exited cleanly (Drop got to write the marker).
pub(crate) fn last_event_was_shutdown() -> bool {
    let events = replay();
    matches!(events.last(), Some(JournalEvent::Shutdown { .. }))
}

/// Return all Attempts with no matching Ack/Fail.
/// Each tuple is `(client_id, kind, ts_ms, payload)` so the caller can
/// reconcile against the broker (Wave 6a recovery path).
pub(crate) fn find_orphan_attempts() -> Vec<(String, AttemptKind, u64, serde_json::Value)> {
    let events = replay();
    let mut resolved: HashSet<String> = HashSet::new();
    for ev in &events {
        match ev {
            JournalEvent::Ack { client_id, .. } | JournalEvent::Fail { client_id, .. } => {
                resolved.insert(client_id.clone());
            }
            _ => {}
        }
    }
    let mut orphans: Vec<(String, AttemptKind, u64, serde_json::Value)> = Vec::new();
    for ev in &events {
        if let JournalEvent::Attempt { client_id, kind, ts_ms, payload } = ev {
            if !resolved.contains(client_id) {
                orphans.push((client_id.clone(), *kind, *ts_ms, payload.clone()));
            }
        }
    }
    orphans
}

/// Spawn a background thread that snapshots the WAL hourly.
/// State layout: state/wal_backup/orders-2026-05-10T14.wal (one per hour, plus
/// orders.wal.1 rotation if present). Keeps last 24 backups; older are deleted.
///
/// Idempotent — repeated calls are no-ops thanks to the OnceLock guard.
/// Does NOT acquire `wal::WAL_LOCK`, so the append path is never blocked.
pub(crate) fn start_wal_backup_thread() {
    static STARTED: OnceLock<()> = OnceLock::new();
    if STARTED.get().is_some() { return; }
    let _ = STARTED.set(());
    crate::foundation::guard::spawn_guarded("mod", || loop {
        std::thread::sleep(std::time::Duration::from_secs(3600)); // 1 hour
        match snapshot_wal() {
            Ok(path) => report(ErrorLevel::Info, "wal_backup", "snapshot_saved", path),
            Err(e) => report(ErrorLevel::Error, "wal_backup", "snapshot_failed", e),
        }
        prune_old_backups(24);
    });
}

/// Trigger a snapshot synchronously. Exposed so a future "Backup now" button
/// in the Order Ledger panel can call it. Returns the destination path on
/// success.
#[allow(dead_code)]
pub(crate) fn snapshot_wal_now() -> Result<String, String> {
    snapshot_wal()
}

fn snapshot_wal() -> Result<String, String> {
    use std::fs;
    let src = wal::wal_path();
    if !src.exists() { return Err("no wal yet".into()); }
    let dir = backup_dir()?;
    fs::create_dir_all(&dir).map_err(|e| format!("mkdir: {e}"))?;
    let now = chrono::Utc::now().format("%Y-%m-%dT%H").to_string();
    let dst = dir.join(format!("orders-{}.wal", now));
    fs::copy(&src, &dst).map_err(|e| format!("copy: {e}"))?;
    // Also snapshot the rotated WAL if present, so the audit trail across the
    // 10 MB rotation boundary survives a corruption event.
    let rotated_src = src.with_file_name("orders.wal.1");
    if rotated_src.exists() {
        let rotated_dst = dir.join(format!("orders-{}.wal.1", now));
        let _ = fs::copy(&rotated_src, &rotated_dst);
    }
    Ok(dst.to_string_lossy().into_owned())
}

fn prune_old_backups(keep: usize) {
    let dir = match backup_dir() { Ok(d) => d, Err(_) => return };
    let mut entries: Vec<_> = match std::fs::read_dir(&dir) {
        Ok(it) => it.flatten().collect(),
        Err(_) => return,
    };
    // Newest first by filename — timestamp prefix sorts lexicographically.
    entries.sort_by_key(|e| std::cmp::Reverse(e.file_name()));
    for old in entries.iter().skip(keep) {
        let _ = std::fs::remove_file(old.path());
    }
}

fn backup_dir() -> Result<std::path::PathBuf, String> {
    Ok(wal::wal_path().parent().ok_or("no parent")?.join("wal_backup"))
}

/// Scan the WAL and report Attempts that never received an Ack or Fail.
/// Logs each to stderr; returns count.
#[allow(dead_code)]
pub(crate) fn report_orphans_to_stderr() -> usize {
    let events = replay();
    let mut resolved: HashSet<String> = HashSet::new();
    for ev in &events {
        match ev {
            JournalEvent::Ack { client_id, .. } | JournalEvent::Fail { client_id, .. } => {
                resolved.insert(client_id.clone());
            }
            _ => {}
        }
    }
    let mut orphans = 0usize;
    for ev in &events {
        if let JournalEvent::Attempt { client_id, kind, ts_ms, .. } = ev {
            if !resolved.contains(client_id) {
                report(ErrorLevel::Warn, "wal", "orphan_attempt", format!("client_id={} kind={:?} ts={}", client_id, kind, ts_ms));
                orphans += 1;
            }
        }
    }
    orphans
}
