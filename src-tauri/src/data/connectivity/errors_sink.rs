//! Unified error sink.
//!
//! Replaces scattered `eprintln!` calls + bespoke toast pushes with a single
//! choke point. Every `report()` call:
//!
//! 1. Pushes a `ReportedError` into a bounded ring buffer (cap 200, FIFO).
//! 2. Emits a `tracing` event at the matching level.
//! 3. Forwards `Warn`/`Error`/`Critical` to `apex_data::live_state::push_toast_with_severity`
//!    so the toast UX receives severity-coded messages (Warn=\x01, Error=\x02, Critical=\x03)
//!    without each call site needing to know about it.
//!
//! The ring buffer is drained by the UI status panel via `drain_recent()`.
//! Draining returns and clears the current buffer, so successive calls see
//! only new errors.

use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;
use std::time::SystemTime;

const CAP: usize = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorLevel {
    Info,
    Warn,
    Error,
    Critical,
}

#[derive(Debug, Clone)]
pub struct ReportedError {
    pub level: ErrorLevel,
    pub source: String,
    pub code: String,
    pub message: String,
    pub at: SystemTime,
}

fn buffer() -> &'static Mutex<VecDeque<ReportedError>> {
    static B: std::sync::OnceLock<Mutex<VecDeque<ReportedError>>> = std::sync::OnceLock::new();
    B.get_or_init(|| Mutex::new(VecDeque::with_capacity(CAP)))
}

/// Monotonic per-(level, source, code) counters (audit F1). Every `report()`
/// bumps one — so broker rejects (source="order_manager"), feed failures
/// (source="apex_data"/…), cache errors, etc. all become alertable Prometheus
/// series via `counts_snapshot()` → `monitoring::format_prometheus`. Cardinality
/// is bounded (a fixed vocabulary of source/code pairs), unlike the ring buffer
/// which is drained; these counters only ever increase, which is what
/// rate()/increase() alerting needs.
fn counters() -> &'static Mutex<HashMap<(&'static str, String, String), u64>> {
    static C: std::sync::OnceLock<Mutex<HashMap<(&'static str, String, String), u64>>> = std::sync::OnceLock::new();
    C.get_or_init(|| Mutex::new(HashMap::new()))
}

fn level_str(level: ErrorLevel) -> &'static str {
    match level {
        ErrorLevel::Info => "info",
        ErrorLevel::Warn => "warn",
        ErrorLevel::Error => "error",
        ErrorLevel::Critical => "critical",
    }
}

/// Snapshot of the (level, source, code, count) counters for Prometheus export.
pub fn counts_snapshot() -> Vec<(&'static str, String, String, u64)> {
    counters()
        .lock()
        .map(|g| g.iter().map(|((l, s, c), n)| (*l, s.clone(), c.clone(), *n)).collect())
        .unwrap_or_default()
}

/// Push an error onto the global sink.
///
/// Side effects: emits a tracing event and (for level >= Warn) routes a
/// toast through `apex_data::live_state::push_toast` to preserve current UX.
/// F1: bump the monotonic per-(level, source, code) Prometheus counter WITHOUT
/// pushing to the UI error ring. Use for high-frequency metrics (e.g. broker
/// order submits) that would otherwise flood the 200-entry tail the operator
/// reads. `report()` funnels its own count through here too, so every reported
/// event is still counted.
pub fn count(level: ErrorLevel, source: &str, code: &str) {
    if let Ok(mut c) = counters().lock() {
        let key = (level_str(level), source.to_string(), code.to_string());
        *c.entry(key).or_insert(0) += 1;
    }
}

pub fn report(level: ErrorLevel, source: &str, code: &str, message: impl Into<String>) {
    let message = message.into();
    let rec = ReportedError {
        level,
        source: source.to_string(),
        code: code.to_string(),
        message: message.clone(),
        at: SystemTime::now(),
    };

    // Ring-buffer push — drop oldest when full so we keep the most recent
    // 200 entries (the UI almost always wants the tail).
    if let Ok(mut g) = buffer().lock() {
        if g.len() == CAP {
            g.pop_front();
        }
        g.push_back(rec);
    }

    // F1: bump the monotonic Prometheus counter for this (level, source, code)
    // BEFORE the Info early-return below, so info-level feed reconnects are
    // observable too.
    count(level, source, code);

    // Mirror to tracing so file/stderr logs capture the same event.
    match level {
        ErrorLevel::Info     => tracing::info!(target: "errors_sink", source, code, "{}", message),
        ErrorLevel::Warn     => tracing::warn!(target: "errors_sink", source, code, "{}", message),
        ErrorLevel::Error    => tracing::error!(target: "errors_sink", source, code, "{}", message),
        ErrorLevel::Critical => tracing::error!(target: "errors_sink", source, code, critical = true, "{}", message),
    }

    // Surface to the existing toast pipeline for anything actionable.
    // Info-level events stay silent — they're for log scrubbing, not the user.
    // Severity mapping:
    //   Warn     → \x01  (yellow warning)
    //   Error    → \x02  (red danger)
    //   Critical → \x03  (red critical with pulse animation — distinct from plain Error)
    let sev = match level {
        ErrorLevel::Info     => return, // silent
        ErrorLevel::Warn     => 1u8,    // yellow warning (\x01 prefix)
        ErrorLevel::Error    => 2u8,    // red danger (\x02 prefix)
        ErrorLevel::Critical => 3u8,    // red critical (\x03 prefix — stronger + pulse)
    };
    let toast = format!("{source}: {message}");
    crate::data::apex_data::live_state::push_toast_with_severity(toast, sev);
}

/// Drain and return all buffered errors. Subsequent calls see only new events.
pub fn drain_recent() -> Vec<ReportedError> {
    if let Ok(mut g) = buffer().lock() {
        g.drain(..).collect()
    } else {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // The tests below rely on the global sink, so we serialize them via a
    // single mutex to keep them from clobbering each other when cargo runs
    // tests in parallel.
    fn lock_tests() -> std::sync::MutexGuard<'static, ()> {
        static M: std::sync::OnceLock<Mutex<()>> = std::sync::OnceLock::new();
        M.get_or_init(|| Mutex::new(())).lock().unwrap()
    }

    #[test]
    fn counters_increment_per_level_source_code() {
        let _g = lock_tests();
        // Unique code so this test is independent of the global monotonic map.
        let code = "f1_counter_probe";
        let before = counts_snapshot().into_iter()
            .find(|(l, s, c, _)| *l == "warn" && s == "feedX" && c == code)
            .map(|(_, _, _, n)| n).unwrap_or(0);
        report(ErrorLevel::Warn, "feedX", code, "boom");
        report(ErrorLevel::Warn, "feedX", code, "boom again");
        let after = counts_snapshot().into_iter()
            .find(|(l, s, c, _)| *l == "warn" && s == "feedX" && c == code)
            .map(|(_, _, _, n)| n).unwrap_or(0);
        assert_eq!(after - before, 2, "counter must bump once per report()");
    }

    #[test]
    fn info_level_is_counted_even_though_silent() {
        let _g = lock_tests();
        // Info returns before the toast but must still be counted (F1).
        let code = "f1_info_probe";
        let before = counts_snapshot().into_iter()
            .find(|(l, _, c, _)| *l == "info" && c == code)
            .map(|(_, _, _, n)| n).unwrap_or(0);
        report(ErrorLevel::Info, "srcInfo", code, "silent but counted");
        let after = counts_snapshot().into_iter()
            .find(|(l, _, c, _)| *l == "info" && c == code)
            .map(|(_, _, _, n)| n).unwrap_or(0);
        assert_eq!(after - before, 1);
    }

    #[test]
    fn fifo_order_preserved() {
        let _g = lock_tests();
        let _ = drain_recent();
        report(ErrorLevel::Info, "src", "a", "1");
        report(ErrorLevel::Info, "src", "b", "2");
        report(ErrorLevel::Info, "src", "c", "3");
        let out = drain_recent();
        assert_eq!(out.len(), 3);
        assert_eq!(out[0].code, "a");
        assert_eq!(out[1].code, "b");
        assert_eq!(out[2].code, "c");
    }

    #[test]
    fn capped_at_200_drops_oldest() {
        let _g = lock_tests();
        let _ = drain_recent();
        for i in 0..250 {
            report(ErrorLevel::Info, "src", "k", format!("{i}"));
        }
        let out = drain_recent();
        assert_eq!(out.len(), CAP);
        // First retained item should be the 50th (250 - 200 = 50).
        assert_eq!(out[0].message, "50");
        assert_eq!(out.last().unwrap().message, "249");
    }

    #[test]
    fn multiple_sources_interleave() {
        let _g = lock_tests();
        let _ = drain_recent();
        report(ErrorLevel::Info, "apex_data", "x", "ax");
        report(ErrorLevel::Info, "ib_ws", "y", "iy");
        report(ErrorLevel::Info, "apex_data", "z", "az");
        let out = drain_recent();
        assert_eq!(out.len(), 3);
        assert_eq!(out[0].source, "apex_data");
        assert_eq!(out[1].source, "ib_ws");
        assert_eq!(out[2].source, "apex_data");
    }

    #[test]
    fn drain_clears_buffer() {
        let _g = lock_tests();
        let _ = drain_recent();
        report(ErrorLevel::Info, "s", "c", "msg");
        assert_eq!(drain_recent().len(), 1);
        assert_eq!(drain_recent().len(), 0);
    }
}
