//! Line-oriented debug log for the ApexData path.
//!
//! Writes to `%TEMP%/apex-terminal-apexdata.log` (or `/tmp/` on Unix) with
//! millisecond timestamps. Auto-truncates at 10 MB on startup. The file stays
//! closed between writes — we open/append/close each line so nothing is
//! buffered and you can `tail -f` it live.
//!
//! Use `log!("short label", "formatted detail {}", arg)` anywhere in the
//! ApexData / option-chart code path.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::SystemTime;

fn log_path() -> &'static PathBuf {
    static P: OnceLock<PathBuf> = OnceLock::new();
    P.get_or_init(|| {
        let dir = std::env::temp_dir();
        let p = dir.join("apex-terminal-apexdata.log");
        // Truncate if > 10 MB to keep it usable for `tail`.
        if let Ok(md) = std::fs::metadata(&p) {
            if md.len() > 10 * 1024 * 1024 {
                let _ = std::fs::remove_file(&p);
            }
        }
        p
    })
}

pub fn write(tag: &str, msg: &str) {
    let ts = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    // Also echo to stderr so `cargo run` users see it live.
    eprintln!("[apex_data.debug][{tag}] {msg}");
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(log_path()) {
        let _ = writeln!(f, "{ts} [{tag}] {msg}");
    }
}

/// Return the absolute log path for display in the UI.
pub fn path_string() -> String {
    log_path().display().to_string()
}

#[macro_export]
macro_rules! apex_log {
    ($tag:expr, $($arg:tt)*) => {
        $crate::apex_data::debug_log::write($tag, &format!($($arg)*))
    };
}
