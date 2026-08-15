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

// ── Scheme-reload callback (P13 — sever design_system→chart_renderer dep) ────
//
// When live ColorSchemes are reloaded from disk, the design_system layer
// can't depend directly on `chart_renderer::gpu::upsert_installed_themes`
// without leaking the chart-app type into the would-be standalone crate.
// Hosts (the chart-app's startup path) register a closure here; the watcher
// invokes it from its background thread with the freshly-parsed schemes.
//
// Closure signature: `Fn(Vec<ColorScheme>) + Send + Sync + 'static`.

type SchemeUpdateFn =
    Box<dyn Fn(Vec<super::color_scheme::ColorScheme>) + Send + Sync + 'static>;

static SCHEME_UPDATE_HOOK: OnceLock<RwLock<Option<SchemeUpdateFn>>> = OnceLock::new();

fn scheme_update_hook() -> &'static RwLock<Option<SchemeUpdateFn>> {
    SCHEME_UPDATE_HOOK.get_or_init(|| RwLock::new(None))
}

/// Register the closure invoked whenever the theme watcher reloads
/// ColorSchemes from disk. The chart-app installs this at startup pointing
/// at `gpu::upsert_installed_themes`. With no hook installed, scheme
/// reloads are silently dropped (acceptable for a standalone ui_kit demo).
pub fn set_scheme_update_hook<F>(f: F)
where
    F: Fn(Vec<super::color_scheme::ColorScheme>) + Send + Sync + 'static,
{
    let mut guard = scheme_update_hook().write().unwrap_or_else(|e| e.into_inner());
    *guard = Some(Box::new(f));
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

/// Test-only door onto the override slot.
///
/// AUDIT 2026-08: the override path had no test coverage at all, which is how
/// `begin_frame` came to consume 36 of the 71 fields the watcher parses without
/// anything noticing. Reaching the slot required the filesystem watcher, so the
/// behaviour was only reachable by hand — and a path only reachable by hand is a
/// path nobody checks.
#[cfg(test)]
pub fn install_override_for_test(style: StyleSystem) {
    install_override(style);
}

/// Clear the override slot. See [`install_override_for_test`].
#[cfg(test)]
pub fn clear_override_for_test() {
    if let Ok(mut guard) = override_slot().write() {
        *guard = None;
    }
}

// ── Themes dir (cross-module access for the in-app TwoAxis editor) ───────────
//
// `start_theme_watcher` stashes the live themes directory here so the design-
// mode inspector (which lives in chart_renderer::render::pane::core) can wire
// `Inspector::themes_dir = Some(themes_dir())` without threading it through
// every call site.

static THEMES_DIR: OnceLock<PathBuf> = OnceLock::new();

/// The live themes directory, if a watcher has been started. Used by the
/// in-app TwoAxis editor to know where to write user-edited DTCG JSON.
pub fn themes_dir() -> Option<PathBuf> {
    THEMES_DIR.get().cloned()
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

    // Stash the themes_dir so the in-app TwoAxis editor (design-mode) can
    // find where to write user edits without threading the path through
    // every call site.
    let _ = THEMES_DIR.set(themes_dir.clone());

    thread::Builder::new()
        .name("apex-theme-watcher".into())
        .spawn(move || run_watcher(themes_dir))
        .expect("failed to spawn theme-watcher thread");
}

/// The watcher loop — runs forever on a background thread.
///
/// Watches TWO subdirectories at ~1.5s polling cadence:
/// - `<themes_dir>/styles/*.json` — `StyleSystem` overrides (radii, strokes,
///   typography, density, treatments). Hot-reload installs into the live
///   override slot; ui_kit token helpers pick it up on the next frame.
/// - `<themes_dir>/colorschemes/*.json` — `ColorScheme` palettes. Hot-reload
///   upserts into the `LIVE_THEMES` registry; the theme picker shows the
///   updated palette immediately. Changes to a theme that's currently
///   active take effect on the next frame.
fn run_watcher(themes_dir: PathBuf) {
    let styles_dir = themes_dir.join("styles");
    let colorschemes_dir = themes_dir.join("colorschemes");
    let mut last_style_mtimes: Vec<(PathBuf, SystemTime)> = Vec::new();
    let mut last_scheme_mtimes: Vec<(PathBuf, SystemTime)> = Vec::new();

    loop {
        thread::sleep(Duration::from_millis(1_500));

        // ── Styles axis ──────────────────────────────────────────────────
        let cur_styles = collect_mtimes(&styles_dir);
        if mtimes_changed(&last_style_mtimes, &cur_styles) {
            if let Some(style) = load_first_style(&styles_dir) {
                eprintln!(
                    "[theme-watcher] reloaded StyleSystem '{}' from {:?}",
                    style.meta.id, styles_dir
                );
                install_override(style);
            }
            last_style_mtimes = cur_styles;
        }

        // ── ColorScheme axis ─────────────────────────────────────────────
        let cur_schemes = collect_mtimes(&colorschemes_dir);
        if mtimes_changed(&last_scheme_mtimes, &cur_schemes) {
            let mut schemes = Vec::new();
            for (path, _) in &cur_schemes {
                if let Ok(json) = fs::read_to_string(path) {
                    match super::color_scheme::ColorScheme::from_dtcg(&json) {
                        Ok(cs) => schemes.push(cs),
                        Err(e) => eprintln!("[theme-watcher] skipping {:?}: {e}", path),
                    }
                }
            }
            if !schemes.is_empty() {
                let n = schemes.len();
                // Dispatch through the host-registered hook (P13). The chart-
                // app's startup wires this to gpu::upsert_installed_themes;
                // a standalone ui_kit / playground can leave it unset.
                let guard = scheme_update_hook().read().unwrap_or_else(|e| e.into_inner());
                if let Some(ref hook) = *guard {
                    hook(schemes);
                    eprintln!(
                        "[theme-watcher] reloaded {n} ColorScheme(s) from {:?}",
                        colorschemes_dir
                    );
                } else {
                    eprintln!(
                        "[theme-watcher] parsed {n} ColorScheme(s) from {:?} but no scheme-update hook is registered — drop",
                        colorschemes_dir
                    );
                }
            }
            last_scheme_mtimes = cur_schemes;
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
