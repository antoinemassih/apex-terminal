//! Asset-handle contract for theme-supplied bytes.
//!
//! ## Purpose
//!
//! Provides a small typed handle + registry so Stream S8 (packaging/install)
//! and Stream S10 (authoring/preview) can reference the **same bytes** for the
//! same conceptual asset, regardless of whether those bytes came from:
//!   - a compiled-in static slice (built-in themes),
//!   - a runtime `Vec<u8>` loaded from a `.apextheme` bundle, or
//!   - a file path on disk (authoring tool preview).
//!
//! ## What this module provides
//!
//! - [`AssetKind`] — typed tag for the role of an asset (font face, icon font, logo, …).
//! - [`AssetSource`] — the actual bytes (static / dynamic / path).
//! - [`AssetHandle`] — a stable `String` key + kind + source + metadata.
//! - [`AssetRegistry`] — the per-theme-pack inventory: register, look up, iterate.
//!
//! ## What this module does NOT provide
//!
//! - Rendering / egui wiring — that belongs to the font registry
//!   (`ui_kit::fonts`) or future image/texture handling.
//! - Validation — that belongs to Stream S9.
//! - Serialization of the full pack — that belongs to Stream S8.
//!
//! `AssetHandle` is designed to be serialization-friendly: the key is a plain
//! string and `AssetKind` / `AssetCapabilities` derive `serde::{Serialize, Deserialize}`.
//! The byte blobs themselves are transient runtime data — `AssetSource` does
//! not derive `Serialize` (the pack bundle writer in S8 handles byte I/O).
//!
//! ## Imagery / background textures (capability stub)
//!
//! `AssetKind::Texture` reserves the slot for background textures and logo
//! assets. It is **not wired** at this stage — the pack reader, renderer, and
//! validation gate for textures belong to later streams (S8 bundling, S9
//! sandboxing, S10 authoring preview). The variant exists so `AssetRegistry`
//! can track the inventory and S8 can include texture files in the bundle
//! without schema changes. See `AssetCapabilities::textures`.

use std::sync::Arc;

// ── AssetKind ─────────────────────────────────────────────────────────────────

/// The role / kind of a theme asset.
///
/// Derives `serde::{Serialize, Deserialize}` so S8 can write the manifest's
/// asset inventory as structured JSON/TOML.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum AssetKind {
    /// A proportional or display font face (.ttf / .otf bytes).
    FontFace,
    /// A monospace font face (.ttf / .otf bytes).
    FontFaceMono,
    /// An icon-glyph font (.ttf containing icon codepoints).
    IconFont,
    /// A logo or wordmark image (PNG/SVG bytes).
    /// **Not wired** — capability stub; see module doc.
    #[serde(rename = "texture")]
    Texture,
    /// An application-specific asset kind; the `String` is a domain tag.
    /// Allows S8/S10 to extend without modifying this enum.
    Custom(String),
}

impl AssetKind {
    /// Returns `true` if this asset kind is currently wired for rendering.
    /// `Texture` returns `false` — it is stubbed behind a capability flag.
    pub fn is_active(&self) -> bool {
        matches!(self, AssetKind::FontFace | AssetKind::FontFaceMono | AssetKind::IconFont)
    }
}

// ── AssetSource ───────────────────────────────────────────────────────────────

/// The byte source for an asset.
///
/// `Clone` is cheap for `Static` and `Dynamic` (the latter is `Arc`-shared).
/// Intentionally does NOT derive `Serialize` — byte blobs are transient
/// runtime data; the pack bundle writer (S8) handles byte I/O separately.
#[derive(Clone)]
pub enum AssetSource {
    /// Compiled-in bytes — zero runtime allocation.
    Static(&'static [u8]),
    /// Runtime bytes from a loaded bundle (Arc-shared to avoid copies).
    Dynamic(Arc<Vec<u8>>),
    /// A filesystem path (authoring tool only — not valid in installed packs).
    Path(std::path::PathBuf),
}

impl AssetSource {
    /// Borrow the asset bytes if they are in memory.  Returns `None` for
    /// `Path` sources (the caller must read the file).
    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            AssetSource::Static(b)  => Some(b),
            AssetSource::Dynamic(b) => Some(b.as_slice()),
            AssetSource::Path(_)    => None,
        }
    }

    /// Returns `true` if the bytes are already in memory.
    pub fn is_in_memory(&self) -> bool {
        !matches!(self, AssetSource::Path(_))
    }

    /// Returns the path if this is a `Path` source.
    pub fn path(&self) -> Option<&std::path::Path> {
        if let AssetSource::Path(p) = self { Some(p) } else { None }
    }

    /// Returns the size of the asset in bytes, if known.
    pub fn byte_len(&self) -> Option<usize> {
        match self {
            AssetSource::Static(b)  => Some(b.len()),
            AssetSource::Dynamic(b) => Some(b.len()),
            AssetSource::Path(_)    => None,
        }
    }
}

// ── AssetHandle ───────────────────────────────────────────────────────────────

/// A stable reference to one theme asset.
///
/// The `key` is the stable identifier used in pack manifests, font registry
/// lookups, and icon set wiring. Suggestion: use a slug like
/// `"font/my-font-regular"` or `"icon/phosphor-bold"`.
///
/// The struct is intentionally lightweight — it carries the source by `Arc`
/// when dynamic, so cloning handles is cheap.
#[derive(Clone)]
pub struct AssetHandle {
    /// Stable string key — unique within an `AssetRegistry`.
    pub key:    String,
    /// Role of the asset.
    pub kind:   AssetKind,
    /// Byte source.
    pub source: AssetSource,
    /// Optional MIME type / format hint (e.g. `"font/ttf"`, `"image/png"`).
    /// Used by S9 for validation and by S8 for the manifest.
    pub mime:   Option<String>,
}

impl AssetHandle {
    /// Create a new handle with a static byte source.
    pub fn new_static(key: impl Into<String>, kind: AssetKind, bytes: &'static [u8]) -> Self {
        Self { key: key.into(), kind, source: AssetSource::Static(bytes), mime: None }
    }

    /// Create a new handle with a dynamic (heap) byte source.
    pub fn new_dynamic(key: impl Into<String>, kind: AssetKind, bytes: Vec<u8>) -> Self {
        Self { key: key.into(), kind, source: AssetSource::Dynamic(Arc::new(bytes)), mime: None }
    }

    /// Create a new handle pointing at a filesystem path (authoring use only).
    pub fn new_path(key: impl Into<String>, kind: AssetKind, path: impl Into<std::path::PathBuf>) -> Self {
        Self { key: key.into(), kind, source: AssetSource::Path(path.into()), mime: None }
    }

    /// Attach a MIME/format hint (builder style).
    pub fn with_mime(mut self, mime: impl Into<String>) -> Self {
        self.mime = Some(mime.into());
        self
    }

    /// Returns the asset bytes if available in memory.
    pub fn bytes(&self) -> Option<&[u8]> {
        self.source.as_bytes()
    }

    /// Returns the asset byte count if known.
    pub fn byte_len(&self) -> Option<usize> {
        self.source.byte_len()
    }
}

// ── AssetCapabilities ─────────────────────────────────────────────────────────

/// Which optional asset classes a theme pack claims to use.
///
/// S8 writes this to the pack manifest; S9 uses it as the validation gate
/// before activating class-specific features. Each flag is individually
/// opt-in so a simple color+scale pack incurs zero asset-handling overhead.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AssetCapabilities {
    /// Pack supplies one or more custom font families.
    pub custom_fonts: bool,
    /// Pack supplies a custom icon glyph font.
    pub custom_icons: bool,
    /// Pack supplies background textures or logo images.
    /// **Not wired** — capability stub; S8/S9/S10 activate this later.
    pub textures: bool,
}

impl AssetCapabilities {
    /// A capability set with no optional features — safe default.
    pub fn none() -> Self {
        Self::default()
    }

    /// Returns `true` if any optional feature is claimed.
    pub fn any(&self) -> bool {
        self.custom_fonts || self.custom_icons || self.textures
    }
}

// ── AssetRegistry ─────────────────────────────────────────────────────────────

/// An inventory of the assets belonging to one theme pack.
///
/// - Keys must be unique within a registry.
/// - The registry is `Send + Sync` so it can be shared across threads in the
///   hot-reload path (wrap in an `Arc<RwLock<AssetRegistry>>`).
/// - Capability flags are tracked automatically on insert.
pub struct AssetRegistry {
    handles:      Vec<AssetHandle>,
    capabilities: AssetCapabilities,
}

impl AssetRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self { handles: Vec::new(), capabilities: AssetCapabilities::none() }
    }

    /// Register an asset.  If a handle with the same key already exists it is
    /// replaced. The capability flags are updated automatically.
    pub fn register(&mut self, handle: AssetHandle) {
        // Update capability flags.
        match &handle.kind {
            AssetKind::FontFace | AssetKind::FontFaceMono => {
                self.capabilities.custom_fonts = true;
            }
            AssetKind::IconFont => {
                self.capabilities.custom_icons = true;
            }
            AssetKind::Texture => {
                self.capabilities.textures = true;
            }
            AssetKind::Custom(_) => {}
        }
        // Replace existing or append.
        if let Some(pos) = self.handles.iter().position(|h| h.key == handle.key) {
            self.handles[pos] = handle;
        } else {
            self.handles.push(handle);
        }
    }

    /// Look up an asset by key.
    pub fn get(&self, key: &str) -> Option<&AssetHandle> {
        self.handles.iter().find(|h| h.key == key)
    }

    /// Iterate over all registered assets.
    pub fn iter(&self) -> impl Iterator<Item = &AssetHandle> {
        self.handles.iter()
    }

    /// Iterate over assets of a specific kind.
    ///
    /// The explicit lifetime annotation is required because the closure
    /// captures `kind` which lives for `'kind`, while `self` is `'reg`.
    pub fn iter_kind<'reg, 'kind>(
        &'reg self,
        kind: &'kind AssetKind,
    ) -> impl Iterator<Item = &'reg AssetHandle> + 'kind
    where
        'reg: 'kind,
    {
        self.handles.iter().filter(move |h| &h.kind == kind)
    }

    /// Returns the capability flags derived from the registered assets.
    pub fn capabilities(&self) -> &AssetCapabilities {
        &self.capabilities
    }

    /// Returns the total number of registered assets.
    pub fn len(&self) -> usize {
        self.handles.len()
    }

    /// Returns `true` if the registry has no assets.
    pub fn is_empty(&self) -> bool {
        self.handles.is_empty()
    }

    /// Remove all assets and reset capability flags.
    pub fn clear(&mut self) {
        self.handles.clear();
        self.capabilities = AssetCapabilities::none();
    }
}

impl Default for AssetRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_register_and_lookup() {
        let mut reg = AssetRegistry::new();
        let h = AssetHandle::new_static("font/inter-medium", AssetKind::FontFace, b"fake_bytes");
        reg.register(h);
        assert!(reg.get("font/inter-medium").is_some());
        assert_eq!(reg.len(), 1);
        assert!(reg.capabilities().custom_fonts);
        assert!(!reg.capabilities().custom_icons);
    }

    #[test]
    fn registry_replace_existing_key() {
        let mut reg = AssetRegistry::new();
        reg.register(AssetHandle::new_dynamic(
            "font/x", AssetKind::FontFace, b"v1".to_vec(),
        ));
        reg.register(AssetHandle::new_dynamic(
            "font/x", AssetKind::FontFace, b"v2".to_vec(),
        ));
        assert_eq!(reg.len(), 1, "replacing existing key must not grow the registry");
        assert_eq!(reg.get("font/x").unwrap().bytes().unwrap(), b"v2");
    }

    #[test]
    fn asset_kind_active_flag() {
        assert!(AssetKind::FontFace.is_active());
        assert!(AssetKind::FontFaceMono.is_active());
        assert!(AssetKind::IconFont.is_active());
        // Texture is stubbed — not active yet.
        assert!(!AssetKind::Texture.is_active());
    }

    #[test]
    fn empty_registry_has_no_capabilities() {
        let reg = AssetRegistry::new();
        assert!(!reg.capabilities().any());
    }

    #[test]
    fn iter_kind_filters_correctly() {
        let mut reg = AssetRegistry::new();
        reg.register(AssetHandle::new_dynamic("f1", AssetKind::FontFace,     vec![]));
        reg.register(AssetHandle::new_dynamic("i1", AssetKind::IconFont,     vec![]));
        reg.register(AssetHandle::new_dynamic("f2", AssetKind::FontFaceMono, vec![]));
        let fonts: Vec<_> = reg.iter_kind(&AssetKind::FontFace).collect();
        assert_eq!(fonts.len(), 1);
        assert_eq!(fonts[0].key, "f1");
    }
}
