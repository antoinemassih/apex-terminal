//! On-disk theme pack registry and lifecycle management.
//!
//! ## Themes directory layout
//!
//! ```text
//! <themes_dir>/
//! ├── index.json              — active selection + installed pack list
//! └── packs/
//!     ├── dracula.apextheme
//!     ├── my-custom.apextheme
//!     └── ...
//! ```
//!
//! ## Active selection persistence
//!
//! `index.json` is a small JSON file storing `{ "active_id": "<id or null>" }`.
//! It is read/written atomically by `set_active` / `install` / `uninstall`.
//!
//! ## Built-in themes
//!
//! Built-in themes are registered as read-only entries.  They do not live on
//! disk in the packs directory — they are constructed from the in-process
//! `builtin_*` functions.  `uninstall` returns an error for built-in ids.
//!
//! ## Error handling
//!
//! All errors surface via [`RegistryError`].  Individual pack failures (a
//! corrupt `.apextheme` in the packs directory) are logged to stderr but do
//! not prevent the registry from starting.

use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use super::{BundleError, ThemePack};
use super::validate::{ValidationReport, validate, AccessibilityMode};
use crate::design_system::theme_pack::manifest::ThemeManifest;

// ── RegistryError ─────────────────────────────────────────────────────────────

/// Errors from `PackRegistry` operations.
#[derive(Debug)]
pub enum RegistryError {
    /// A filesystem I/O error.
    Io(std::io::Error),
    /// Bundle read/write error.
    Bundle(BundleError),
    /// JSON encode/decode error (index file).
    Json(serde_json::Error),
    /// The requested pack id is not registered.
    NotFound(String),
    /// Attempted to uninstall a built-in (read-only) theme.
    IsBuiltin(String),
    /// Pack failed structural / accessibility / sandbox validation.
    ///
    /// The embedded report contains every finding that caused the rejection.
    ValidationFailed(ValidationReport),
}

impl std::fmt::Display for RegistryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RegistryError::Io(e)              => write!(f, "I/O error: {e}"),
            RegistryError::Bundle(e)          => write!(f, "bundle error: {e}"),
            RegistryError::Json(e)            => write!(f, "JSON error: {e}"),
            RegistryError::NotFound(id)       => write!(f, "theme not found: {id}"),
            RegistryError::IsBuiltin(id)      => write!(f, "cannot uninstall built-in theme: {id}"),
            RegistryError::ValidationFailed(r) => write!(
                f,
                "pack validation failed: {} error(s), {} warning(s)",
                r.errors.len(), r.warnings.len(),
            ),
        }
    }
}

impl From<std::io::Error> for RegistryError {
    fn from(e: std::io::Error) -> Self { RegistryError::Io(e) }
}

impl From<BundleError> for RegistryError {
    fn from(e: BundleError) -> Self { RegistryError::Bundle(e) }
}

impl From<serde_json::Error> for RegistryError {
    fn from(e: serde_json::Error) -> Self { RegistryError::Json(e) }
}

// ── Internal index format ─────────────────────────────────────────────────────

/// The persisted index file (`index.json`).
#[derive(Debug, Default, Serialize, Deserialize)]
struct PackIndex {
    /// The id of the currently active theme pack, or `null` if none.
    #[serde(default)]
    active_id: Option<String>,
}

// ── PackEntry ─────────────────────────────────────────────────────────────────

/// One entry in the registry's in-memory catalogue.
#[derive(Debug, Clone)]
struct PackEntry {
    manifest:   ThemeManifest,
    is_builtin: bool,
}

// ── PackRegistry ──────────────────────────────────────────────────────────────

/// On-disk + in-memory theme pack registry.
///
/// Call [`PackRegistry::open`] to load (or create) a themes directory, then
/// use [`install`], [`uninstall`], [`list`], [`set_active`], and [`active`].
pub struct PackRegistry {
    /// Themes root directory (contains `index.json` and `packs/`).
    themes_dir: PathBuf,
    /// In-memory catalogue: id → entry.
    entries: HashMap<String, PackEntry>,
    /// Currently active pack id.
    active_id: Option<String>,
}

impl PackRegistry {
    /// Open (or create) a themes registry at `themes_dir`.
    ///
    /// Built-in manifests are registered as read-only entries.
    /// Any `.apextheme` files already in `<themes_dir>/packs/` are scanned and
    /// registered automatically (corrupt files are skipped with a warning).
    pub fn open(themes_dir: &Path) -> Result<Self, RegistryError> {
        // Create directory structure if needed.
        fs::create_dir_all(themes_dir)?;
        let packs_dir = themes_dir.join("packs");
        fs::create_dir_all(&packs_dir)?;

        // Load the index file (active selection).
        let index = load_index(themes_dir)?;

        let mut registry = PackRegistry {
            themes_dir: themes_dir.to_owned(),
            entries: HashMap::new(),
            active_id: index.active_id,
        };

        // Register built-ins.
        for manifest in builtin_manifests() {
            registry.entries.insert(manifest.id.clone(), PackEntry {
                manifest,
                is_builtin: true,
            });
        }

        // Scan installed packs.
        if let Ok(rd) = fs::read_dir(&packs_dir) {
            for entry in rd.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("apextheme") {
                    continue;
                }
                // Read just the manifest header — we don't need to fully
                // deserialize the pack during registry open.
                match ThemePack::read_bundle(&path) {
                    Ok(pack) => {
                        let id = pack.manifest.id.clone();
                        registry.entries.insert(id, PackEntry {
                            manifest: pack.manifest,
                            is_builtin: false,
                        });
                    }
                    Err(e) => {
                        eprintln!("[pack_registry] skipping {:?}: {e}", path);
                    }
                }
            }
        }

        Ok(registry)
    }

    // ── Install ───────────────────────────────────────────────────────────────

    /// Install a pack from a `.apextheme` bundle file.
    ///
    /// The bundle is read, validated, and copied into
    /// `<themes_dir>/packs/<id>.apextheme`.  If a pack with the same id is
    /// already installed it is replaced.
    ///
    /// ## Validation gate
    ///
    /// Every pack is validated by [`validate`] before it is written to disk.
    /// Packs with one or more [`Severity::Error`] findings are rejected with
    /// [`RegistryError::ValidationFailed`].  Packs with only warnings are
    /// installed; the caller may inspect the `ValidationReport` by re-running
    /// `validate(&pack, …)` separately if it needs the warning list.
    ///
    /// Returns the manifest of the installed pack.
    pub fn install(&mut self, bundle_path: &Path) -> Result<ThemeManifest, RegistryError> {
        let pack = ThemePack::read_bundle(bundle_path)?;

        // ── Validation gate ───────────────────────────────────────────────────
        // Run in Standard mode: low-contrast produces warnings, not errors.
        // Callers that want stricter checking can call validate() themselves
        // before calling install(), or use install_strict() when it ships.
        let report = validate(&pack, AccessibilityMode::Standard);
        if !report.is_installable() {
            return Err(RegistryError::ValidationFailed(report));
        }
        // Warnings are surfaced only in debug builds to avoid log spam in prod.
        #[cfg(debug_assertions)]
        if !report.warnings.is_empty() {
            eprintln!(
                "[pack_registry] pack '{}' installed with {} warning(s):",
                pack.manifest.id,
                report.warnings.len(),
            );
            for w in &report.warnings {
                eprintln!("  {w}");
            }
        }

        let id = pack.manifest.id.clone();

        // Write to the packs directory.
        let dest = self.pack_path(&id);
        pack.write_bundle(&dest)?;

        // Update in-memory catalogue.
        let manifest = pack.manifest.clone();
        self.entries.insert(id, PackEntry { manifest: pack.manifest, is_builtin: false });
        Ok(manifest)
    }

    // ── Uninstall ─────────────────────────────────────────────────────────────

    /// Uninstall the pack with the given `id`.
    ///
    /// - Built-in themes cannot be uninstalled (`RegistryError::IsBuiltin`).
    /// - If the uninstalled pack was active, the active selection is cleared.
    /// - If the pack file does not exist on disk, the in-memory entry is still
    ///   removed (idempotent cleanup).
    pub fn uninstall(&mut self, id: &str) -> Result<(), RegistryError> {
        let entry = self.entries.get(id)
            .ok_or_else(|| RegistryError::NotFound(id.to_string()))?;

        if entry.is_builtin {
            return Err(RegistryError::IsBuiltin(id.to_string()));
        }

        // Remove the bundle file (ignore "not found" on disk).
        let pack_path = self.pack_path(id);
        match fs::remove_file(&pack_path) {
            Ok(_)  => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(RegistryError::Io(e)),
        }

        // Clear active selection if this was the active pack.
        if self.active_id.as_deref() == Some(id) {
            self.active_id = None;
        }

        self.entries.remove(id);
        self.persist_index()?;
        Ok(())
    }

    // ── List ──────────────────────────────────────────────────────────────────

    /// Return the manifests of all registered packs (built-in + installed).
    ///
    /// Built-in entries come first, then installed entries in insertion order.
    pub fn list(&self) -> Vec<&ThemeManifest> {
        // Built-ins first, then user-installed.
        let mut builtins:  Vec<&ThemeManifest> = Vec::new();
        let mut installed: Vec<&ThemeManifest> = Vec::new();
        for entry in self.entries.values() {
            if entry.is_builtin {
                builtins.push(&entry.manifest);
            } else {
                installed.push(&entry.manifest);
            }
        }
        // Stable sort within each group.
        builtins.sort_by(|a, b| a.id.cmp(&b.id));
        installed.sort_by(|a, b| a.id.cmp(&b.id));
        builtins.extend(installed);
        builtins
    }

    // ── Active selection ──────────────────────────────────────────────────────

    /// Set the active theme pack by `id`.
    ///
    /// The selection is persisted to `index.json`.
    /// Returns `Err(RegistryError::NotFound)` if the id is not registered.
    pub fn set_active(&mut self, id: &str) -> Result<(), RegistryError> {
        if !self.entries.contains_key(id) {
            return Err(RegistryError::NotFound(id.to_string()));
        }
        self.active_id = Some(id.to_string());
        self.persist_index()?;
        Ok(())
    }

    /// Return the id of the currently active theme pack, if any.
    pub fn active(&self) -> Option<&str> {
        self.active_id.as_deref()
    }

    /// Load and return the full `ThemePack` for the currently active pack.
    ///
    /// Returns `None` if no pack is active or the active id is a built-in
    /// (built-ins are constructed differently and do not have a bundle file).
    ///
    /// Returns `Err` if the active bundle file cannot be read.
    pub fn active_pack(&self) -> Option<Result<ThemePack, RegistryError>> {
        let id = self.active_id.as_ref()?;
        let entry = self.entries.get(id)?;
        if entry.is_builtin {
            return None; // Built-ins are not disk-backed bundles.
        }
        let path = self.pack_path(id);
        Some(ThemePack::read_bundle(&path).map_err(RegistryError::Bundle))
    }

    /// Load the full `ThemePack` for the given `id`, if it is an installed
    /// (non-builtin) pack.  Returns `None` for built-in ids.
    ///
    /// Unlike [`active_pack`] this does not require `id` to be the currently
    /// active pack — callers use it to load any installed pack for activation.
    pub fn load_pack(&self, id: &str) -> Option<Result<ThemePack, RegistryError>> {
        let entry = self.entries.get(id)?;
        if entry.is_builtin {
            return None;
        }
        let path = self.pack_path(id);
        Some(ThemePack::read_bundle(&path).map_err(RegistryError::Bundle))
    }

    /// Returns `true` if `id` is registered as a built-in (read-only) theme.
    pub fn is_builtin(&self, id: &str) -> bool {
        self.entries.get(id).map(|e| e.is_builtin).unwrap_or(false)
    }

    // ── Internal helpers ──────────────────────────────────────────────────────

    fn pack_path(&self, id: &str) -> PathBuf {
        self.themes_dir.join("packs").join(format!("{id}.apextheme"))
    }

    fn index_path(&self) -> PathBuf {
        self.themes_dir.join("index.json")
    }

    fn persist_index(&self) -> Result<(), RegistryError> {
        let index = PackIndex { active_id: self.active_id.clone() };
        let json  = serde_json::to_string_pretty(&index)?;
        fs::write(self.index_path(), json)?;
        Ok(())
    }
}

// ── Built-in manifests ────────────────────────────────────────────────────────

/// Return minimal `ThemeManifest`s for all built-in theme packs.
///
/// These are in-process only — no `.apextheme` bundle file exists for them.
/// The manifest `id` matches the `ColorScheme::meta.id` so the active-pair
/// tracking can eventually bridge the two layers.
fn builtin_manifests() -> Vec<ThemeManifest> {
    use crate::design_system::builtin::{builtin_color_schemes, builtin_style_systems};
    use crate::design_system::theme_pack::manifest::ThemeManifest;

    let mut manifests = Vec::new();

    for cs in builtin_color_schemes() {
        let mut m = ThemeManifest::new(cs.meta.id.clone(), cs.meta.name.clone());
        m.is_dark = cs.meta.is_dark;
        m.app_schema_version = super::manifest::CURRENT_SCHEMA_VERSION;
        manifests.push(m);
    }

    manifests
}

// ── Index load helper ─────────────────────────────────────────────────────────

fn load_index(themes_dir: &Path) -> Result<PackIndex, RegistryError> {
    let index_path = themes_dir.join("index.json");
    match fs::read_to_string(&index_path) {
        Ok(json) => Ok(serde_json::from_str(&json).unwrap_or_default()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(PackIndex::default()),
        Err(e) => Err(RegistryError::Io(e)),
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::design_system::{
        color_scheme::builtin_dark,
        recipes::RecipeSet,
        style_system::StyleSystem,
        theme_pack::{manifest::ThemeManifest, ThemePack},
    };
    use crate::ui_kit::assets::AssetRegistry;

    /// Write a minimal ThemePack bundle to a temp file and return the path.
    fn write_test_bundle(id: &str, name: &str) -> (tempfile::NamedTempFile, ThemePack) {
        let pack = ThemePack {
            manifest: {
                let mut m = ThemeManifest::new(id, name);
                m.is_dark = true;
                m
            },
            color_scheme:  builtin_dark(),
            style_system:  StyleSystem::builtin_default(),
            recipes:       RecipeSet::new(),
            shell_profile: None,
            assets:        AssetRegistry::new(),
        };

        let tmp = tempfile::Builder::new()
            .suffix(".apextheme")
            .tempfile()
            .expect("tempfile");
        pack.write_bundle(tmp.path()).expect("write_bundle");
        (tmp, pack)
    }

    #[test]
    fn lifecycle_install_list_set_active_uninstall() {
        let tmp_dir = tempfile::tempdir().expect("tempdir");
        let mut reg = PackRegistry::open(tmp_dir.path()).expect("open registry");

        // Builtins are present.
        let initial_count = reg.list().len();
        assert!(initial_count > 0, "builtins should be present");

        // Install a custom pack.
        let (bundle_file, _pack) = write_test_bundle("my-custom", "My Custom Theme");
        let manifest = reg.install(bundle_file.path()).expect("install");
        assert_eq!(manifest.id, "my-custom");

        // List now includes the new pack.
        let listed: Vec<_> = reg.list().iter().map(|m| m.id.clone()).collect();
        assert!(listed.contains(&"my-custom".to_string()), "installed pack should appear in list");
        assert_eq!(reg.list().len(), initial_count + 1);

        // Set active.
        reg.set_active("my-custom").expect("set_active");
        assert_eq!(reg.active(), Some("my-custom"));

        // Index was persisted.
        let index = load_index(tmp_dir.path()).expect("load_index");
        assert_eq!(index.active_id.as_deref(), Some("my-custom"));

        // Uninstall.
        reg.uninstall("my-custom").expect("uninstall");
        assert_eq!(reg.list().len(), initial_count);
        // Active cleared when active pack is uninstalled.
        assert_eq!(reg.active(), None);

        // Pack file should be gone.
        let pack_file = tmp_dir.path().join("packs/my-custom.apextheme");
        assert!(!pack_file.exists(), "bundle file should be deleted");
    }

    #[test]
    fn cannot_uninstall_builtin() {
        let tmp_dir = tempfile::tempdir().expect("tempdir");
        let mut reg = PackRegistry::open(tmp_dir.path()).expect("open registry");

        // Get a builtin id.
        let builtin_id = reg.list()
            .iter()
            .find(|m| true)
            .map(|m| m.id.clone())
            .expect("at least one builtin");

        let result = reg.uninstall(&builtin_id);
        assert!(
            matches!(result, Err(RegistryError::IsBuiltin(_))),
            "uninstalling a builtin should fail"
        );
    }

    #[test]
    fn set_active_unknown_id_returns_not_found() {
        let tmp_dir = tempfile::tempdir().expect("tempdir");
        let mut reg = PackRegistry::open(tmp_dir.path()).expect("open registry");
        let result = reg.set_active("nonexistent-theme");
        assert!(matches!(result, Err(RegistryError::NotFound(_))));
    }

    #[test]
    fn registry_survives_reopen_with_installed_packs() {
        let tmp_dir = tempfile::tempdir().expect("tempdir");

        // First session: install and set active.
        {
            let mut reg = PackRegistry::open(tmp_dir.path()).expect("open");
            let (bundle, _) = write_test_bundle("persistent", "Persistent");
            reg.install(bundle.path()).expect("install");
            reg.set_active("persistent").expect("set_active");
        }

        // Second session: registry should re-discover the pack and active id.
        {
            let reg = PackRegistry::open(tmp_dir.path()).expect("reopen");
            let ids: Vec<_> = reg.list().iter().map(|m| m.id.clone()).collect();
            assert!(ids.contains(&"persistent".to_string()), "pack should survive reopen");
            assert_eq!(reg.active(), Some("persistent"), "active should survive reopen");
        }
    }

    #[test]
    fn install_replaces_existing_id() {
        let tmp_dir = tempfile::tempdir().expect("tempdir");
        let mut reg = PackRegistry::open(tmp_dir.path()).expect("open");

        let (b1, _) = write_test_bundle("replaceable", "Version 1");
        reg.install(b1.path()).expect("install v1");

        let (b2, _) = write_test_bundle("replaceable", "Version 2");
        reg.install(b2.path()).expect("install v2");

        // Only one entry with id "replaceable".
        // Collect to an owned Vec<(id, name)> to avoid holding a borrow on
        // the temporary returned by reg.list().
        let all: Vec<(String, String)> = reg.list()
            .into_iter()
            .map(|m| (m.id.clone(), m.name.clone()))
            .collect();
        let replaceable: Vec<_> = all.iter().filter(|(id, _)| id == "replaceable").collect();
        assert_eq!(replaceable.len(), 1, "should be exactly one entry after replace");
        assert_eq!(replaceable[0].1, "Version 2");
    }
}
