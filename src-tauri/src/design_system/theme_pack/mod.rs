//! `ThemePack` — the portable unit of theme distribution for apex-terminal.
//!
//! A `ThemePack` bundles all the ingredients a theme author needs into one
//! transportable `.apextheme` archive:
//!
//! | Section           | Type                      | Purpose                              |
//! |-------------------|---------------------------|--------------------------------------|
//! | `manifest`        | [`ThemeManifest`]         | Identity, schema version, metadata   |
//! | `color_scheme`    | [`ColorScheme`]           | Palette axis (DTCG JSON in archive)  |
//! | `style_system`    | [`StyleSystem`]           | Dimension axis (DTCG JSON in archive)|
//! | `recipes`         | [`RecipeSet`]             | Per-component style overrides        |
//! | `shell_profile`   | `Option<serde_json::Value>` | Future shell layout (stub)         |
//! | `assets`          | [`AssetRegistry`]         | Font/icon/texture byte blobs         |
//!
//! ## Sub-modules
//!
//! - [`manifest`] — `ThemeManifest`, schema version constant, capabilities.
//! - [`bundle`] — `ThemePack::write_bundle` / `ThemePack::read_bundle`.
//! - [`migrate`] — schema version migration (v1 is a no-op stub).
//! - [`pack_registry`] — on-disk `PackRegistry` (install/uninstall/list/active).
//!
//! ## Shell profile stub
//!
//! `shell_profile` is typed as `Option<serde_json::Value>` to serve as a
//! forward-compatible placeholder for the `ShellProfile` type designed in
//! `docs/migration/shell-profile.md`.  No structural `ShellProfile` is
//! implemented here — that is Stream S6 Wave-3 work.  The stub allows
//! `.apextheme` authors to embed a raw JSON blob that a future reader will
//! parse into a typed `ShellProfile`.
//!
//! ## Checksum stub
//!
//! `ThemeManifest::checksum` is stored but not verified.  The hash is written
//! as an informational field; verification belongs to Stream S9 (sandboxing
//! and validation).

pub mod bundle;
pub mod manifest;
pub mod migrate;
pub mod pack_registry;

use crate::design_system::{
    color_scheme::ColorScheme,
    recipes::RecipeSet,
    style_system::StyleSystem,
};
use crate::ui_kit::assets::AssetRegistry;

pub use manifest::{
    AssetInventoryEntry, PackCapabilities, ThemeManifest, CURRENT_SCHEMA_VERSION,
};
pub use pack_registry::{PackRegistry, RegistryError};

// ── BundleError ───────────────────────────────────────────────────────────────

/// Errors returned by `ThemePack::write_bundle` and `ThemePack::read_bundle`.
#[derive(Debug)]
pub enum BundleError {
    /// A filesystem I/O error.
    Io(std::io::Error),
    /// A zip archive error.
    Zip(zip::result::ZipError),
    /// A JSON serialization/deserialization error.
    Json(serde_json::Error),
    /// A required zip entry (e.g. `manifest.json`) is missing.
    MissingSection(String),
    /// The DTCG loader returned an error for a color/style section.
    DtcgLoad(crate::design_system::loader::LoadError),
    /// The pack's `app_schema_version` exceeds the reader's current version.
    SchemaTooNew {
        /// The version declared in the pack manifest.
        pack_version: u32,
        /// The highest version this reader understands.
        reader_version: u32,
    },
    /// A zip entry contained non-UTF-8 bytes where text was expected.
    InvalidUtf8(String),
}

impl std::fmt::Display for BundleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BundleError::Io(e) => write!(f, "I/O error: {e}"),
            BundleError::Zip(e) => write!(f, "zip error: {e}"),
            BundleError::Json(e) => write!(f, "JSON error: {e}"),
            BundleError::MissingSection(s) => write!(f, "missing required section: {s}"),
            BundleError::DtcgLoad(e) => write!(f, "DTCG load error: {e}"),
            BundleError::SchemaTooNew { pack_version, reader_version } => write!(
                f,
                "pack schema version {pack_version} exceeds reader version {reader_version}"
            ),
            BundleError::InvalidUtf8(msg) => write!(f, "invalid UTF-8: {msg}"),
        }
    }
}

// ── ThemePack ─────────────────────────────────────────────────────────────────

/// The in-memory representation of a theme pack.
///
/// Constructed from a `.apextheme` bundle via [`ThemePack::read_bundle`], or
/// built programmatically for authoring / built-in themes.
///
/// Write back to disk with [`ThemePack::write_bundle`].
pub struct ThemePack {
    /// Identity and metadata.
    pub manifest: ThemeManifest,

    /// Palette axis — colors, tints, semantic roles.
    pub color_scheme: ColorScheme,

    /// Dimension axis — typography, spacing, radii, treatments.
    pub style_system: StyleSystem,

    /// Per-component style overrides (namespaced recipe keys).
    pub recipes: RecipeSet,

    /// Future shell layout profile.  `None` = use app default.
    ///
    /// Typed as `serde_json::Value` because `ShellProfile` is DESIGN-ONLY in
    /// `docs/migration/shell-profile.md` — no production Rust type yet.  The
    /// bundle writer stores this as `shellprofile.json`; the reader recovers it
    /// verbatim.  A future stream will add a typed `ShellProfile` here and
    /// implement a proper `from_value` / `to_value` bridge.
    pub shell_profile: Option<serde_json::Value>,

    /// Font, icon, and texture assets bundled with this pack.
    pub assets: AssetRegistry,
}

impl std::fmt::Debug for ThemePack {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // `AssetRegistry` does not implement `Debug` (its `AssetSource::Static`
        // variant holds raw bytes that are not useful to print).  We print the
        // asset count and capability summary instead.
        f.debug_struct("ThemePack")
            .field("manifest",       &self.manifest)
            .field("color_scheme_id", &self.color_scheme.meta.id)
            .field("style_system_id", &self.style_system.meta.id)
            .field("recipe_count",   &self.recipes.len())
            .field("shell_profile",  &self.shell_profile)
            .field("asset_count",    &self.assets.len())
            .finish()
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::design_system::{
        builtin::{builtin_color_schemes, builtin_style_systems},
        color_scheme::builtin_dark,
        theme_pack::{
            manifest::ThemeManifest,
        },
    };
    use crate::ui_kit::assets::{AssetHandle, AssetKind, AssetRegistry};

    /// Integration test: build a ThemePack from builtins + a custom recipe,
    /// write_bundle → read_bundle, assert round-trip equality on key sections.
    #[test]
    fn integration_round_trip_with_recipe() {
        use crate::ui_kit::sx::recipe_spec::{
            ColorSpec, PadTier, RadiusTier, RecipeDelta, RecipeSpec, ShadeRef, ToneRef,
        };

        // Build the pack.
        let cs = builtin_color_schemes()
            .into_iter()
            .next()
            .unwrap_or_else(builtin_dark);
        let ss = builtin_style_systems()
            .into_iter()
            .next()
            .unwrap_or_else(StyleSystem::builtin_default);

        let mut recipes = RecipeSet::new();
        recipes.insert(
            "button.primary",
            RecipeSpec {
                base: RecipeDelta {
                    radius: Some(RadiusTier::Pill),
                    fill: Some(ColorSpec::Tone {
                        tone: ToneRef::Accent,
                        shade: Some(ShadeRef::S500),
                    }),
                    px: Some(PadTier::Lg),
                    ..Default::default()
                },
                ..Default::default()
            },
        );

        let mut assets = AssetRegistry::new();
        assets.register(
            AssetHandle::new_dynamic("font/test", AssetKind::FontFace, b"FONT_DATA".to_vec())
                .with_mime("font/ttf"),
        );

        let original = ThemePack {
            manifest: {
                let mut m = ThemeManifest::new("integration-test", "Integration Test");
                m.author  = "ci".to_string();
                m.version = "1.0.0".to_string();
                m.is_dark = cs.meta.is_dark;
                m
            },
            color_scheme: cs,
            style_system: ss,
            recipes,
            shell_profile: None,
            assets,
        };

        // Write and read back.
        let tmp = tempfile::NamedTempFile::new().expect("tempfile");
        original.write_bundle(tmp.path()).expect("write_bundle");
        let loaded = ThemePack::read_bundle(tmp.path()).expect("read_bundle");

        // Manifest.
        assert_eq!(loaded.manifest.id,      original.manifest.id);
        assert_eq!(loaded.manifest.author,  original.manifest.author);
        assert_eq!(loaded.manifest.version, original.manifest.version);
        assert_eq!(loaded.manifest.is_dark, original.manifest.is_dark);

        // ColorScheme key fields.
        assert_eq!(loaded.color_scheme.meta.id, original.color_scheme.meta.id);
        assert_eq!(loaded.color_scheme.bg,      original.color_scheme.bg);
        assert_eq!(loaded.color_scheme.accent,  original.color_scheme.accent);
        assert_eq!(loaded.color_scheme.bull,    original.color_scheme.bull);
        assert_eq!(loaded.color_scheme.bear,    original.color_scheme.bear);
        assert_eq!(loaded.color_scheme.warn,    original.color_scheme.warn);

        // StyleSystem key fields.
        assert_eq!(loaded.style_system.meta.id,            original.style_system.meta.id);
        assert_eq!(loaded.style_system.typography.size_sm, original.style_system.typography.size_sm);
        assert_eq!(loaded.style_system.radii.sm,           original.style_system.radii.sm);
        assert_eq!(
            loaded.style_system.treatments.solid_active_fills,
            original.style_system.treatments.solid_active_fills,
        );

        // Recipes.
        assert_eq!(loaded.recipes.len(), 1);
        assert!(loaded.recipes.get("button.primary").is_some());

        // Assets.
        assert_eq!(loaded.assets.len(), 1);
        assert!(loaded.assets.capabilities().custom_fonts);
        let font = loaded.assets.get("font/test").expect("font asset");
        assert_eq!(font.bytes().unwrap(), b"FONT_DATA");

        // Capabilities in manifest.
        assert!(loaded.manifest.capabilities.uses_fonts);
    }
}
