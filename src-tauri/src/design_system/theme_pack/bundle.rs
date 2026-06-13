//! `.apextheme` bundle read/write — zip-backed serialization for `ThemePack`.
//!
//! ## Bundle layout (zip archive)
//!
//! ```text
//! my-theme.apextheme   (zip)
//! ├── manifest.json         — ThemeManifest (JSON)
//! ├── colorscheme.json      — ColorScheme (DTCG JSON via to_dtcg / from_dtcg)
//! ├── stylesystem.json      — StyleSystem (DTCG JSON via to_dtcg / from_dtcg)
//! ├── recipes.json          — RecipeSet (plain serde_json)
//! ├── shellprofile.json     — optional; raw JSON blob (placeholder)
//! └── assets/
//!     └── <key>             — one file per AssetHandle with in-memory source
//! ```
//!
//! ## Streaming / size bounds
//!
//! - Each section is written as a separate zip entry so the archive is
//!   section-addressable.
//! - The reader uses `zip::ZipArchive` which reads the central directory
//!   first — no full decompress needed to enumerate entries.
//! - Individual entries are fully read into `Vec<u8>` before parsing.
//!   For the current scale (theme packs are ≤ a few MB) this is safe;
//!   a streaming JSON parser would be the S9 concern.
//!
//! ## Error handling
//!
//! All errors surface via [`BundleError`] (defined in `super`).

use std::{
    fs::File,
    io::{Read, Write},
    path::Path,
};

use zip::{
    ZipArchive, ZipWriter,
    write::SimpleFileOptions,
};

use super::{BundleError, ThemePack};
use crate::design_system::{
    color_scheme::ColorScheme,
    loader::LoadError,
    recipes::RecipeSet,
    style_system::StyleSystem,
    theme_pack::manifest::{AssetInventoryEntry, PackCapabilities, ThemeManifest},
};
use crate::ui_kit::assets::{AssetHandle, AssetKind, AssetRegistry, AssetSource};

// ── Entry name constants ──────────────────────────────────────────────────────

const MANIFEST_ENTRY:    &str = "manifest.json";
const COLORSCHEME_ENTRY: &str = "colorscheme.json";
const STYLESYSTEM_ENTRY: &str = "stylesystem.json";
const RECIPES_ENTRY:     &str = "recipes.json";
const SHELLPROFILE_ENTRY: &str = "shellprofile.json";
const ASSETS_PREFIX:     &str = "assets/";

// ── Write ─────────────────────────────────────────────────────────────────────

impl ThemePack {
    /// Write this `ThemePack` to a `.apextheme` bundle at `path`.
    ///
    /// The file is created (or truncated) at `path`.  If writing fails partway
    /// through the file may be incomplete — the caller is responsible for
    /// cleanup on error.
    ///
    /// ## Bundle layout
    ///
    /// See module doc for the full zip layout.
    pub fn write_bundle(&self, path: &Path) -> Result<(), BundleError> {
        let file = File::create(path).map_err(BundleError::Io)?;
        let mut zip = ZipWriter::new(file);

        let options = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);

        // ── Build the manifest with a current asset inventory ─────────────────
        // The manifest stored on `self` is used as-is but we update the
        // asset_inventory and capabilities to reflect the actual AssetRegistry.
        let mut manifest = self.manifest.clone();
        manifest.asset_inventory.clear();
        for handle in self.assets.iter() {
            manifest.asset_inventory.push(AssetInventoryEntry {
                key:  handle.key.clone(),
                kind: handle.kind.clone(),
                mime: handle.mime.clone(),
            });
        }
        // Capabilities derived from the live registry.
        manifest.capabilities = PackCapabilities::from_asset_capabilities(
            self.assets.capabilities(),
        );
        if self.shell_profile.is_some() {
            manifest.capabilities.uses_shell_profile = true;
        }

        // ── manifest.json ─────────────────────────────────────────────────────
        let manifest_json = serde_json::to_string_pretty(&manifest)
            .map_err(BundleError::Json)?;
        zip.start_file(MANIFEST_ENTRY, options).map_err(BundleError::Zip)?;
        zip.write_all(manifest_json.as_bytes()).map_err(BundleError::Io)?;

        // ── colorscheme.json (DTCG) ───────────────────────────────────────────
        let cs_json = self.color_scheme.to_dtcg();
        zip.start_file(COLORSCHEME_ENTRY, options).map_err(BundleError::Zip)?;
        zip.write_all(cs_json.as_bytes()).map_err(BundleError::Io)?;

        // ── stylesystem.json (DTCG) ───────────────────────────────────────────
        let ss_json = self.style_system.to_dtcg();
        zip.start_file(STYLESYSTEM_ENTRY, options).map_err(BundleError::Zip)?;
        zip.write_all(ss_json.as_bytes()).map_err(BundleError::Io)?;

        // ── recipes.json ──────────────────────────────────────────────────────
        let recipes_json = serde_json::to_string_pretty(&self.recipes)
            .map_err(BundleError::Json)?;
        zip.start_file(RECIPES_ENTRY, options).map_err(BundleError::Zip)?;
        zip.write_all(recipes_json.as_bytes()).map_err(BundleError::Io)?;

        // ── shellprofile.json (optional) ──────────────────────────────────────
        if let Some(ref sp) = self.shell_profile {
            let sp_json = serde_json::to_string_pretty(sp).map_err(BundleError::Json)?;
            zip.start_file(SHELLPROFILE_ENTRY, options).map_err(BundleError::Zip)?;
            zip.write_all(sp_json.as_bytes()).map_err(BundleError::Io)?;
        }

        // ── assets/<key> ──────────────────────────────────────────────────────
        for handle in self.assets.iter() {
            let bytes = match &handle.source {
                AssetSource::Static(b)  => *b,
                AssetSource::Dynamic(b) => b.as_slice(),
                AssetSource::Path(p) => {
                    // Path-backed assets are not supported in write_bundle
                    // (authoring tool concern). Skip with a diagnostic.
                    eprintln!(
                        "[theme_pack] skipping path-backed asset {:?} — load bytes first",
                        p
                    );
                    continue;
                }
            };
            let entry_name = format!("{ASSETS_PREFIX}{}", handle.key);
            zip.start_file(&entry_name, options).map_err(BundleError::Zip)?;
            zip.write_all(bytes).map_err(BundleError::Io)?;
        }

        zip.finish().map_err(BundleError::Zip)?;
        Ok(())
    }

    /// Read a `.apextheme` bundle from `path` and return a `ThemePack`.
    ///
    /// ## Migration
    ///
    /// After loading, [`ThemePack::migrate`] is called to bring older-schema
    /// packs up to the current format.
    ///
    /// ## Error handling
    ///
    /// Returns `Err` if:
    /// - The file cannot be opened (`BundleError::Io`).
    /// - The zip is malformed (`BundleError::Zip`).
    /// - `manifest.json` is missing or cannot be parsed (`BundleError::MissingSection` / `BundleError::Json`).
    /// - `colorscheme.json` or `stylesystem.json` cannot be parsed (`BundleError::DtcgLoad`).
    /// - The pack schema version exceeds [`CURRENT_SCHEMA_VERSION`] (`BundleError::SchemaTooNew`).
    ///
    /// Missing *optional* sections (`shellprofile.json`, individual assets)
    /// are silently ignored.
    pub fn read_bundle(path: &Path) -> Result<ThemePack, BundleError> {
        let file = File::open(path).map_err(BundleError::Io)?;
        let mut archive = ZipArchive::new(file).map_err(BundleError::Zip)?;

        // ── manifest.json ─────────────────────────────────────────────────────
        let manifest: ThemeManifest = {
            let bytes = read_entry(&mut archive, MANIFEST_ENTRY)?;
            serde_json::from_slice(&bytes).map_err(BundleError::Json)?
        };

        // Reject packs from future schema versions.
        if !manifest.is_compatible() {
            return Err(BundleError::SchemaTooNew {
                pack_version:   manifest.app_schema_version,
                reader_version: super::manifest::CURRENT_SCHEMA_VERSION,
            });
        }
        let schema_version = manifest.app_schema_version;

        // ── colorscheme.json ──────────────────────────────────────────────────
        let color_scheme: ColorScheme = {
            let bytes  = read_entry(&mut archive, COLORSCHEME_ENTRY)?;
            let json   = String::from_utf8(bytes).map_err(|e| BundleError::InvalidUtf8(e.to_string()))?;
            ColorScheme::from_dtcg(&json).map_err(BundleError::DtcgLoad)?
        };

        // ── stylesystem.json ──────────────────────────────────────────────────
        let style_system: StyleSystem = {
            let bytes = read_entry(&mut archive, STYLESYSTEM_ENTRY)?;
            let json  = String::from_utf8(bytes).map_err(|e| BundleError::InvalidUtf8(e.to_string()))?;
            StyleSystem::from_dtcg(&json).map_err(BundleError::DtcgLoad)?
        };

        // ── recipes.json ──────────────────────────────────────────────────────
        let recipes: RecipeSet = {
            match try_read_entry(&mut archive, RECIPES_ENTRY) {
                Some(bytes) => serde_json::from_slice(&bytes).unwrap_or_default(),
                None        => RecipeSet::default(),
            }
        };

        // ── shellprofile.json (optional placeholder) ──────────────────────────
        let shell_profile: Option<serde_json::Value> = try_read_entry(&mut archive, SHELLPROFILE_ENTRY)
            .and_then(|bytes| serde_json::from_slice(&bytes).ok());

        // ── assets/<key> ──────────────────────────────────────────────────────
        // Build the registry from the manifest inventory + zip entries.
        let mut assets = AssetRegistry::new();
        for inv_entry in &manifest.asset_inventory {
            let zip_path = format!("{ASSETS_PREFIX}{}", inv_entry.key);
            if let Some(bytes) = try_read_entry(&mut archive, &zip_path) {
                let mut handle = AssetHandle::new_dynamic(
                    inv_entry.key.clone(),
                    inv_entry.kind.clone(),
                    bytes,
                );
                if let Some(ref mime) = inv_entry.mime {
                    handle = handle.with_mime(mime.clone());
                }
                assets.register(handle);
            }
        }

        // ── Assemble and migrate ───────────────────────────────────────────────
        let mut pack = ThemePack { manifest, color_scheme, style_system, recipes, shell_profile, assets };
        pack.migrate(schema_version)?;
        Ok(pack)
    }
}

// ── Zip entry helpers ─────────────────────────────────────────────────────────

/// Read a named zip entry into `Vec<u8>`, returning `BundleError::MissingSection`
/// when the entry is not present.
fn read_entry<R: Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
    name: &str,
) -> Result<Vec<u8>, BundleError> {
    try_read_entry(archive, name).ok_or_else(|| BundleError::MissingSection(name.to_string()))
}

/// Read a named zip entry into `Vec<u8>`, returning `None` when absent.
fn try_read_entry<R: Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
    name: &str,
) -> Option<Vec<u8>> {
    let mut entry = archive.by_name(name).ok()?;
    let mut buf = Vec::with_capacity(entry.size() as usize);
    entry.read_to_end(&mut buf).ok()?;
    Some(buf)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::design_system::{
        builtin::{builtin_color_schemes, builtin_style_systems},
        color_scheme::builtin_dark,
        recipes::RecipeSet,
        style_system::StyleSystem,
        theme_pack::{manifest::ThemeManifest, ThemePack},
    };
    use crate::ui_kit::assets::{AssetHandle, AssetKind, AssetRegistry};

    fn make_test_pack() -> ThemePack {
        let mut manifest = ThemeManifest::new("test-pack", "Test Pack");
        manifest.author  = "tester".to_string();
        manifest.version = "0.1.0".to_string();
        manifest.is_dark = true;

        // Use the first built-in color scheme and style system for fidelity.
        let cs = builtin_color_schemes().into_iter().next().unwrap_or_else(builtin_dark);
        let ss = builtin_style_systems()
            .into_iter()
            .next()
            .unwrap_or_else(StyleSystem::builtin_default);

        let mut recipes = RecipeSet::new();
        // (empty recipes — sufficient for round-trip test)

        let mut assets = AssetRegistry::new();
        assets.register(
            AssetHandle::new_dynamic("font/test-mono", AssetKind::FontFaceMono, b"FAKE_FONT_BYTES".to_vec())
                .with_mime("font/ttf"),
        );

        ThemePack {
            manifest,
            color_scheme: cs,
            style_system: ss,
            recipes,
            shell_profile: None,
            assets,
        }
    }

    #[test]
    fn round_trip_bundle() {
        let original = make_test_pack();
        let tmp = tempfile::NamedTempFile::new().expect("tempfile");
        let path = tmp.path().to_owned();

        original.write_bundle(&path).expect("write_bundle failed");
        let loaded = ThemePack::read_bundle(&path).expect("read_bundle failed");

        // Manifest identity
        assert_eq!(loaded.manifest.id,      original.manifest.id);
        assert_eq!(loaded.manifest.name,    original.manifest.name);
        assert_eq!(loaded.manifest.author,  original.manifest.author);
        assert_eq!(loaded.manifest.version, original.manifest.version);
        assert_eq!(loaded.manifest.is_dark, original.manifest.is_dark);

        // ColorScheme key fields
        assert_eq!(loaded.color_scheme.meta.id, original.color_scheme.meta.id);
        assert_eq!(loaded.color_scheme.bg,      original.color_scheme.bg);
        assert_eq!(loaded.color_scheme.accent,  original.color_scheme.accent);
        assert_eq!(loaded.color_scheme.bull,    original.color_scheme.bull);
        assert_eq!(loaded.color_scheme.bear,    original.color_scheme.bear);

        // StyleSystem key fields
        assert_eq!(loaded.style_system.meta.id,            original.style_system.meta.id);
        assert_eq!(loaded.style_system.typography.size_sm, original.style_system.typography.size_sm);
        assert_eq!(loaded.style_system.radii.sm,           original.style_system.radii.sm);

        // Asset round-trip
        assert_eq!(loaded.assets.len(), 1);
        let asset = loaded.assets.get("font/test-mono").expect("asset should be present");
        assert_eq!(asset.bytes().unwrap(), b"FAKE_FONT_BYTES");
        assert_eq!(asset.mime.as_deref(), Some("font/ttf"));

        // Capabilities auto-derived
        assert!(loaded.manifest.capabilities.uses_fonts);
        assert!(!loaded.manifest.capabilities.uses_shell_profile);

        // No shell profile
        assert!(loaded.shell_profile.is_none());
    }

    #[test]
    fn round_trip_with_shell_profile_placeholder() {
        let mut pack = make_test_pack();
        pack.shell_profile = Some(serde_json::json!({
            "nav": "TopPills",
            "dock": "BottomBar"
        }));

        let tmp = tempfile::NamedTempFile::new().expect("tempfile");
        let path = tmp.path().to_owned();

        pack.write_bundle(&path).expect("write_bundle");
        let loaded = ThemePack::read_bundle(&path).expect("read_bundle");

        assert!(loaded.shell_profile.is_some());
        let sp = loaded.shell_profile.unwrap();
        assert_eq!(sp["nav"].as_str(), Some("TopPills"));
        assert!(loaded.manifest.capabilities.uses_shell_profile);
    }

    #[test]
    fn future_schema_version_rejected() {
        let mut pack = make_test_pack();
        pack.manifest.app_schema_version = 9999;

        let tmp = tempfile::NamedTempFile::new().expect("tempfile");
        let path = tmp.path().to_owned();

        pack.write_bundle(&path).expect("write_bundle should succeed (no version check on write)");
        let result = ThemePack::read_bundle(&path);

        assert!(
            matches!(result, Err(BundleError::SchemaTooNew { .. })),
            "expected SchemaTooNew, got: {:?}", result
        );
    }
}
