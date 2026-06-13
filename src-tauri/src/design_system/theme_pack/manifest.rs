//! `ThemeManifest` — the metadata header of a `.apextheme` bundle.
//!
//! Stored as `manifest.json` at the root of the zip archive.
//!
//! ## Versioning & forward compatibility
//!
//! - `app_schema_version`: the bundle format version this pack was authored
//!   against (a monotonically increasing `u32`).  The current version is
//!   [`CURRENT_SCHEMA_VERSION`].  The reader accepts any pack whose schema
//!   version is ≤ current — unknown *future* versions are rejected.
//! - All fields except `id`, `name`, and `app_schema_version` use
//!   `#[serde(default)]` so older packs (missing newer fields) deserialize
//!   cleanly via `Default`.
//! - New fields must always carry a sensible `Default` impl.
//!
//! ## Checksum
//!
//! `checksum` is an optional SHA-256 hex string over the bundle zip bytes
//! (excluding the manifest entry itself).  The current implementation treats
//! it as informational — the reader stores it but does not verify it.
//! Verification is deferred to Stream S9 (validation / sandboxing).

use serde::{Deserialize, Serialize};

use crate::ui_kit::assets::{AssetCapabilities, AssetKind};

// ── Schema version constant ───────────────────────────────────────────────────

/// The bundle format version understood by this build.
///
/// Increment this when the zip layout or mandatory JSON sections change in
/// a way that older readers cannot safely ignore.  For additive changes
/// (new optional fields) a version bump is NOT required — `serde(default)`
/// handles those transparently.
pub const CURRENT_SCHEMA_VERSION: u32 = 1;

// ── AssetInventoryEntry ───────────────────────────────────────────────────────

/// One entry in the manifest's asset inventory.
///
/// Lists the assets bundled under `assets/<key>` in the zip.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssetInventoryEntry {
    /// The asset key (matches the path `assets/<key>` inside the zip).
    pub key: String,
    /// The kind of asset.
    pub kind: AssetKind,
    /// Optional MIME / format hint (e.g. `"font/ttf"`, `"image/png"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mime: Option<String>,
}

// ── ThemeManifest ─────────────────────────────────────────────────────────────

/// The metadata header for a `.apextheme` bundle.
///
/// Serialized as `manifest.json` inside the zip archive.
///
/// All fields except `id`, `name`, and `app_schema_version` use
/// `#[serde(default)]` to ensure forward compatibility when older packs are
/// loaded by newer readers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ThemeManifest {
    // ── Identity ─────────────────────────────────────────────────────────────

    /// Stable machine-readable identifier (slug, e.g. `"dracula"`).
    /// Used as the primary key for install/uninstall/set-active operations.
    pub id: String,

    /// Human-readable display name (e.g. `"Dracula"`).
    pub name: String,

    /// Theme author name or handle.
    #[serde(default)]
    pub author: String,

    /// Semver version string (e.g. `"1.0.0"`).
    ///
    /// No semver parsing is done at this layer — it is stored and displayed
    /// as-is.  Validation belongs to Stream S9.
    #[serde(default = "default_version")]
    pub version: String,

    // ── Schema compatibility ──────────────────────────────────────────────────

    /// The bundle format version this pack was authored against.
    ///
    /// Readers reject packs whose schema version exceeds
    /// [`CURRENT_SCHEMA_VERSION`].  Packs with older schema versions are
    /// accepted; missing fields fall back to `Default`.
    pub app_schema_version: u32,

    // ── Style metadata ────────────────────────────────────────────────────────

    /// `true` if the pack's color scheme is a dark-mode palette.
    #[serde(default = "default_true")]
    pub is_dark: bool,

    // ── Optional capability flags ─────────────────────────────────────────────

    /// Which optional asset classes this pack uses.
    ///
    /// Derived from the asset inventory during `write_bundle`; stored
    /// explicitly so readers can gate features without unpacking assets.
    #[serde(default)]
    pub capabilities: PackCapabilities,

    // ── Asset inventory ───────────────────────────────────────────────────────

    /// Inventory of all assets bundled under `assets/<key>` in the zip.
    ///
    /// The list is informational at read time — the actual bytes live in the
    /// zip entries.  Missing entries do not cause an error (graceful
    /// degradation).
    #[serde(default)]
    pub asset_inventory: Vec<AssetInventoryEntry>,

    // ── Integrity ─────────────────────────────────────────────────────────────

    /// Optional SHA-256 hex checksum over the non-manifest zip entries.
    ///
    /// Stored as an informational field.  Verification is deferred to
    /// Stream S9 — the current reader does not validate it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checksum: Option<String>,
}

// ── PackCapabilities ──────────────────────────────────────────────────────────

/// High-level capability flags declared in the manifest.
///
/// These mirror `AssetCapabilities` but are expressed in terms of the
/// ThemePack feature set (fonts, icons, imagery, shell profile) so the reader
/// can gate features without inspecting the full asset inventory.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct PackCapabilities {
    /// Pack supplies one or more custom font families.
    #[serde(default)]
    pub uses_fonts: bool,
    /// Pack supplies a custom icon glyph font.
    #[serde(default)]
    pub uses_icons: bool,
    /// Pack supplies background textures or logo images.
    /// **Stub** — not wired at this stage.
    #[serde(default)]
    pub uses_imagery: bool,
    /// Pack contains a `ShellProfile` section.
    /// **Stub** — `shell_profile` is `Option<serde_json::Value>` for now.
    #[serde(default)]
    pub uses_shell_profile: bool,
}

impl PackCapabilities {
    /// Derive capability flags from an `AssetCapabilities` struct (S7 type).
    pub fn from_asset_capabilities(ac: &AssetCapabilities) -> Self {
        Self {
            uses_fonts:    ac.custom_fonts,
            uses_icons:    ac.custom_icons,
            uses_imagery:  ac.textures,
            uses_shell_profile: false, // never auto-detected; set by caller
        }
    }
}

// ── Default helpers ───────────────────────────────────────────────────────────

fn default_version() -> String { "1.0.0".to_string() }
fn default_true()    -> bool   { true }

// ── ThemeManifest impl ────────────────────────────────────────────────────────

impl ThemeManifest {
    /// Create a minimal manifest with the current schema version.
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            author: String::new(),
            version: default_version(),
            app_schema_version: CURRENT_SCHEMA_VERSION,
            is_dark: true,
            capabilities: PackCapabilities::default(),
            asset_inventory: Vec::new(),
            checksum: None,
        }
    }

    /// Returns `true` if this pack's schema version is compatible with the
    /// current reader (i.e. not from a future, unknown format).
    pub fn is_compatible(&self) -> bool {
        self.app_schema_version <= CURRENT_SCHEMA_VERSION
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_round_trip() {
        let mut m = ThemeManifest::new("dracula", "Dracula");
        m.author  = "dracula-theme".to_string();
        m.version = "2.1.0".to_string();
        m.is_dark = true;
        m.capabilities.uses_fonts = true;
        m.asset_inventory.push(AssetInventoryEntry {
            key:  "font/dracula-mono".to_string(),
            kind: AssetKind::FontFaceMono,
            mime: Some("font/ttf".to_string()),
        });

        let json = serde_json::to_string_pretty(&m).expect("serialize");
        let back: ThemeManifest = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(back.id,      m.id);
        assert_eq!(back.name,    m.name);
        assert_eq!(back.author,  m.author);
        assert_eq!(back.version, m.version);
        assert_eq!(back.app_schema_version, CURRENT_SCHEMA_VERSION);
        assert!(back.is_dark);
        assert!(back.capabilities.uses_fonts);
        assert_eq!(back.asset_inventory.len(), 1);
        assert_eq!(back.asset_inventory[0].key, "font/dracula-mono");
    }

    #[test]
    fn older_schema_missing_fields_use_defaults() {
        // Simulate a pack from schema v1 that is missing `checksum`,
        // `capabilities`, `asset_inventory`, and `author`.
        let json = r#"{
            "id": "legacy",
            "name": "Legacy Pack",
            "app_schema_version": 1
        }"#;

        let m: ThemeManifest = serde_json::from_str(json).expect("deserialize");
        assert_eq!(m.id, "legacy");
        assert_eq!(m.author, "");
        assert_eq!(m.version, "1.0.0");
        assert!(m.is_dark, "is_dark should default to true");
        assert!(!m.capabilities.uses_fonts);
        assert!(m.asset_inventory.is_empty());
        assert!(m.checksum.is_none());
    }

    #[test]
    fn is_compatible_rejects_future_schema() {
        let mut m = ThemeManifest::new("future", "Future Pack");
        m.app_schema_version = CURRENT_SCHEMA_VERSION + 1;
        assert!(!m.is_compatible());
    }

    #[test]
    fn is_compatible_accepts_current_and_older() {
        let m = ThemeManifest::new("current", "Current Pack");
        assert!(m.is_compatible());

        // Older schema versions (if any existed) are also accepted.
        // Since CURRENT_SCHEMA_VERSION = 1, there's no version 0; just test = 1.
        let mut old = ThemeManifest::new("old", "Old Pack");
        old.app_schema_version = 1;
        assert!(old.is_compatible());
    }
}
