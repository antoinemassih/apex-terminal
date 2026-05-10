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
                eprintln!("[wal] orphan attempt: client_id={} kind={:?} ts={}", client_id, kind, ts_ms);
                orphans += 1;
            }
        }
    }
    orphans
}
