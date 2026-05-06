//! Local file Save / Open for charts. Uses XOL as the on-disk format —
//! deliberately NOT a separate native binary format. See the discussion in
//! `CHART_STORAGE_ARCHITECTURE.md` Phase 2 — `.apxchart` is deferred until
//! it's actually load-bearing.
//!
//! Two layers:
//!   - `save_xol_to_path` / `load_xol_from_path` — pure I/O helpers, used by
//!     both the native binary and the Tauri commands.
//!   - `save_chart_dialog` / `open_chart_dialog` — blocking system file
//!     pickers (rfd). Suitable for native-binary callers; the Tauri webview
//!     side uses the commands in `commands.rs` and lets the JS run the
//!     picker.

use std::path::{Path, PathBuf};

use super::codec::xol::{self, ImportWarnings, XolError};
use super::ChartState;

#[derive(Debug, thiserror::Error)]
pub enum FileIoError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("xol: {0}")]
    Xol(#[from] XolError),
}

/// Write a `ChartState` to a `.xol` file on disk.
pub fn save_xol_to_path(state: &ChartState, path: &Path) -> Result<(), FileIoError> {
    let bytes = xol::write(state)?;
    std::fs::write(path, bytes)?;
    Ok(())
}

/// Read a `.xol` file from disk into a `ChartState` plus any non-blocking
/// warnings (missing indicators, unknown drawing kinds, etc.).
pub fn load_xol_from_path(path: &Path) -> Result<(ChartState, ImportWarnings), FileIoError> {
    let bytes = std::fs::read(path)?;
    Ok(xol::read(&bytes)?)
}

/// Show a Save dialog and write the chart there. Returns the chosen path,
/// or `None` if the user cancelled.
///
/// **Blocking** — call from a non-render thread. Suitable for the native
/// binary's UI handlers that already run on the worker side of egui's loop.
pub fn save_chart_dialog(
    state: &ChartState,
    suggested_name: &str,
) -> Result<Option<PathBuf>, FileIoError> {
    let suggested = if suggested_name.ends_with(".xol") {
        suggested_name.to_string()
    } else {
        format!("{suggested_name}.xol")
    };
    let path = rfd::FileDialog::new()
        .set_title("Export chart as .xol")
        .add_filter("Apex Chart (.xol)", &["xol"])
        .set_file_name(&suggested)
        .save_file();
    let Some(path) = path else { return Ok(None); };
    save_xol_to_path(state, &path)?;
    Ok(Some(path))
}

/// Show an Open dialog and load the selected `.xol` file. Returns `None` if
/// the user cancelled, or `Some((state, warnings))` on success.
pub fn open_chart_dialog() -> Result<Option<(ChartState, ImportWarnings)>, FileIoError> {
    let path = rfd::FileDialog::new()
        .set_title("Open chart")
        .add_filter("Apex Chart (.xol)", &["xol"])
        .pick_file();
    let Some(path) = path else { return Ok(None); };
    Ok(Some(load_xol_from_path(&path)?))
}

#[cfg(test)]
mod tests {
    use super::super::*;
    use super::*;

    #[test]
    fn round_trip_through_tempfile() {
        let mut state = ChartState::new(
            0,
            Symbol {
                canonical: "SPX".into(),
                asset_class: AssetClass::Index,
                provider_hints: ProviderHints::default(),
            },
            Timeframe::M5,
        );
        state.title = Some("file io smoke".into());

        let dir = std::env::temp_dir();
        let path = dir.join("apex-chart-file-io-test.xol");

        save_xol_to_path(&state, &path).unwrap();
        let (loaded, warnings) = load_xol_from_path(&path).unwrap();

        assert!(warnings.is_empty());
        assert_eq!(loaded.symbol.canonical.as_str(), "SPX");
        assert_eq!(loaded.title.as_ref().map(|s| s.as_str()), Some("file io smoke"));

        let _ = std::fs::remove_file(&path);
    }
}
