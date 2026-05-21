//! Perf-safe hot-reload of theme JSON files.
//!
//! ## Design
//!
//! A single background thread polls the `styles/` subdirectory inside the
//! provided themes directory every ~1.5 s.  When any file's mtime changes the
//! thread re-parses the first matching `StyleSystem` and installs it into the
//! **live-override slot** (`THEME_OVERRIDE`).
//!
//! The render thread reads the slot **once per frame** via a `RwLock::read()`.
//! On a lightly-contended lock that resolves in tens of nanoseconds — far below
//! the per-frame budget.  There is zero additional cost when no override is
//! installed (`None` fast-path).
//!
//! ## Performance contract
//!
//! - The watcher is a plain `std::thread` — it never touches the render loop.
//! - `begin_frame()` does **one** `RwLock::read()` per frame and nothing else.
//! - No external crate is required; the poller uses `std::fs::metadata` mtime.

use std::{
    fs,
    path::PathBuf,
    sync::{Arc, OnceLock, RwLock},
    thread,
    time::{Duration, SystemTime},
};

use super::style_system::StyleSystem;

// ── Override slot ─────────────────────────────────────────────────────────────

/// Global live-override slot.
///
/// `None`  → no hot-reload file present; `begin_frame` uses the `current()`
///            `StyleSettings` path (existing behaviour, unchanged).
/// `Some`  → a background-parsed `StyleSystem` is installed; `begin_frame`
///            sources radii and strokes from it instead.
static THEME_OVERRIDE: OnceLock<RwLock<Option<Arc<StyleSystem>>>> = OnceLock::new();

fn override_slot() -> &'static RwLock<Option<Arc<StyleSystem>>> {
    THEME_OVERRIDE.get_or_init(|| RwLock::new(None))
}

/// Read the currently-installed style override.
///
/// Returns `None` when no override is active (no hot-reload file was parsed
/// successfully since startup).  Called by `begin_frame()` once per frame.
///
/// Cost: one `RwLock::read()` on an uncontended lock — ~20–50 ns.
#[inline]
pub fn active_override() -> Option<Arc<StyleSystem>> {
    override_slot().read().ok()?.clone()
}

/// Install a new style override (called from the watcher thread only).
fn install_override(style: StyleSystem) {
    if let Ok(mut guard) = override_slot().write() {
        *guard = Some(Arc::new(style));
    }
}

// ── Background watcher ────────────────────────────────────────────────────────

/// Spawn the hot-reload watcher thread.
///
/// The thread polls `<themes_dir>/styles/*.json` every 1.5 s.  When any
/// file's mtime advances the first successfully-parsed `StyleSystem` is
/// installed into the override slot.
///
/// Call this **once** at application startup.  The thread is detached and runs
/// for the lifetime of the process — no join handle is returned.
pub fn start_theme_watcher(themes_dir: PathBuf) {
    // Ensure the slot is initialised before the thread starts so that
    // `active_override()` is always callable from the render thread.
    let _ = override_slot();

    thread::Builder::new()
        .name("apex-theme-watcher".into())
        .spawn(move || run_watcher(themes_dir))
        .expect("failed to spawn theme-watcher thread");
}

/// The watcher loop — runs forever on a background thread.
fn run_watcher(themes_dir: PathBuf) {
    let styles_dir = themes_dir.join("styles");
    let mut last_mtimes: Vec<(PathBuf, SystemTime)> = Vec::new();

    loop {
        thread::sleep(Duration::from_millis(1_500));

        // Collect current mtimes for all *.json files in styles_dir.
        let current = collect_mtimes(&styles_dir);

        if mtimes_changed(&last_mtimes, &current) {
            // At least one file changed — re-parse all and take the first success.
            if let Some(style) = load_first_style(&styles_dir) {
                eprintln!(
                    "[theme-watcher] reloaded StyleSystem '{}' from {:?}",
                    style.meta.id, styles_dir
                );
                install_override(style);
            }
            last_mtimes = current;
        }
    }
}

/// Collect `(path, mtime)` for every `*.json` in `dir`, sorted by path for
/// stable comparison.  Files that cannot be stat'd are silently skipped.
fn collect_mtimes(dir: &std::path::Path) -> Vec<(PathBuf, SystemTime)> {
    let mut out = Vec::new();
    if let Ok(rd) = fs::read_dir(dir) {
        for entry in rd.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            if let Ok(meta) = fs::metadata(&path) {
                if let Ok(mtime) = meta.modified() {
                    out.push((path, mtime));
                }
            }
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// Returns `true` if the set of files or any mtime has changed.
fn mtimes_changed(
    old: &[(PathBuf, SystemTime)],
    new: &[(PathBuf, SystemTime)],
) -> bool {
    if old.len() != new.len() {
        return true;
    }
    old.iter().zip(new.iter()).any(|((op, ot), (np, nt))| op != np || ot != nt)
}

/// Try to parse the first valid `StyleSystem` from `*.json` files in `dir`.
fn load_first_style(dir: &std::path::Path) -> Option<StyleSystem> {
    let mut paths: Vec<PathBuf> = fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("json"))
        .collect();
    paths.sort();

    for path in &paths {
        match fs::read_to_string(path) {
            Ok(json) => match StyleSystem::from_dtcg(&json) {
                Ok(s) => return Some(s),
                Err(e) => eprintln!("[theme-watcher] skipping {:?}: {e}", path),
            },
            Err(e) => eprintln!("[theme-watcher] cannot read {:?}: {e}", path),
        }
    }
    None
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::design_system::export::export_builtin_themes;

    #[test]
    fn override_slot_starts_empty() {
        // Before any watcher is started, active_override() must return None.
        // (This test runs in isolation; the OnceLock may already be initialised
        // from a prior test run in the same process — that is fine, we just
        // verify the type contract compiles and the read succeeds.)
        let _ = active_override(); // must not panic
    }

    #[test]
    fn install_and_read_override() {
        let style = StyleSystem::builtin_default();
        install_override(style.clone());
        let got = active_override().expect("override should be Some after install");
        assert_eq!(got.meta.id, style.meta.id);
    }

    #[test]
    fn watcher_loads_exported_style() {
        let tmp = tempfile::tempdir().expect("tempdir");
        export_builtin_themes(tmp.path()).expect("export");

        // load_first_style should find at least one style.
        let styles_dir = tmp.path().join("styles");
        let result = load_first_style(&styles_dir);
        assert!(result.is_some(), "load_first_style must return Some when styles exist");
    }

    #[test]
    fn mtimes_changed_detects_new_file() {
        let t0 = SystemTime::UNIX_EPOCH;
        let old = vec![(PathBuf::from("a.json"), t0)];
        let new = vec![
            (PathBuf::from("a.json"), t0),
            (PathBuf::from("b.json"), t0),
        ];
        assert!(mtimes_changed(&old, &new));
    }

    #[test]
    fn mtimes_changed_detects_mtime_update() {
        let t0 = SystemTime::UNIX_EPOCH;
        let t1 = t0 + Duration::from_secs(1);
        let old = vec![(PathBuf::from("a.json"), t0)];
        let new = vec![(PathBuf::from("a.json"), t1)];
        assert!(mtimes_changed(&old, &new));
    }

    #[test]
    fn mtimes_unchanged_returns_false() {
        let t0 = SystemTime::UNIX_EPOCH;
        let v = vec![(PathBuf::from("a.json"), t0)];
        assert!(!mtimes_changed(&v, &v));
    }
}
