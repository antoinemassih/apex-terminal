//! Persistent perf logging — append-only jank events + per-session summaries.
//!
//! Logs land in a platform-appropriate data dir:
//!   macOS:   ~/Library/Application Support/apex-terminal/perf/
//!   Linux:   ~/.local/share/apex-terminal/perf/
//!   Windows: %APPDATA%\apex-terminal\perf\
//!
//! Files:
//!   jank.jsonl               — one JSON line per jank event (rotated at 10 MiB → jank.1.jsonl)
//!   sessions/<ts>-<sha>.json — per-process summary written on graceful exit or panic
//!
//! Reads APEX_GIT_SHA from the compile-time env (set by build.rs).

use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::sync::OnceLock;
use parking_lot::Mutex;

const ROTATE_BYTES: u64 = 10 * 1024 * 1024; // 10 MiB
const GIT_SHA: &str = env!("APEX_GIT_SHA");

static LOG_DIR: OnceLock<Option<PathBuf>> = OnceLock::new();
static JANK_WRITER: OnceLock<Option<Mutex<BufWriter<File>>>> = OnceLock::new();

fn log_dir() -> Option<&'static PathBuf> {
    LOG_DIR.get_or_init(|| {
        let base = dirs::data_dir()?;
        let dir = base.join("apex-terminal").join("perf");
        std::fs::create_dir_all(&dir).ok()?;
        let _ = std::fs::create_dir_all(dir.join("sessions"));
        Some(dir)
    }).as_ref()
}

fn jank_writer() -> Option<&'static Mutex<BufWriter<File>>> {
    let slot: &Option<Mutex<BufWriter<File>>> = JANK_WRITER.get_or_init(|| {
        let dir = log_dir()?;
        let path = dir.join("jank.jsonl");
        // Simple single-rotation: rename to .1 when file exceeds ROTATE_BYTES.
        if let Ok(meta) = std::fs::metadata(&path) {
            if meta.len() > ROTATE_BYTES {
                let _ = std::fs::rename(&path, dir.join("jank.1.jsonl"));
            }
        }
        let file = OpenOptions::new().create(true).append(true).open(&path).ok()?;
        Some(Mutex::new(BufWriter::new(file)))
    });
    slot.as_ref()
}

/// Append a single jank event JSON line to `jank.jsonl`.
///
/// Never blocks — drops the line silently if the writer lock is already held
/// (e.g., a flush running concurrently on another thread). Failures in the
/// underlying I/O are also silently dropped to keep the hot render path clean.
pub fn append_jank(json_line: &str) {
    let Some(writer) = jank_writer() else { return };
    // try_lock: skip rather than block the render thread
    let Some(mut guard) = writer.try_lock() else { return };
    let _ = writeln!(guard, "{}", json_line);
}

/// Flush the jank writer. Call from a periodic 5-second tick or from the
/// monitoring supervisor so data isn't lost on power failure.
pub fn flush_jank() {
    if let Some(writer) = jank_writer() {
        if let Some(mut guard) = writer.try_lock() {
            let _ = guard.flush();
        }
    }
}

/// Write a per-session summary JSON file to `sessions/<timestamp>-<sha>.json`.
///
/// Call on graceful exit (the `RunEvent::Exit` handler) or from a panic hook.
/// Writes atomically to a temp file then renames to avoid partial JSON on crash.
pub fn write_session_summary(summary: &serde_json::Value) {
    let Some(dir) = log_dir() else { return };
    let ts = chrono::Utc::now().format("%Y%m%dT%H%M%S").to_string();
    let filename = format!("{}-{}.json", ts, GIT_SHA);
    let final_path = dir.join("sessions").join(&filename);
    let tmp_path = dir.join("sessions").join(format!("{}.tmp", filename));
    if let Ok(s) = serde_json::to_string_pretty(summary) {
        // Write to .tmp then rename — avoids partial file if process dies mid-write.
        if std::fs::write(&tmp_path, &s).is_ok() {
            let _ = std::fs::rename(&tmp_path, &final_path);
        } else {
            // Rename failed / no tmp support — write direct.
            let _ = std::fs::write(&final_path, s);
        }
    }
}

/// The 12-character git SHA baked into this binary at compile time.
pub fn git_sha() -> &'static str {
    GIT_SHA
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn git_sha_is_nonempty() {
        assert!(!git_sha().is_empty(), "APEX_GIT_SHA must not be empty");
    }
}
