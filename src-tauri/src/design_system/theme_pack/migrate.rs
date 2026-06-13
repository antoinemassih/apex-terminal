//! Schema version migration for `.apextheme` bundles.
//!
//! ## Extension point
//!
//! `ThemePack::migrate(version)` is called by the bundle reader after
//! deserializing all sections.  For version 1 it is a no-op; future schema
//! bumps add arms to the `match`.
//!
//! ## Versioning policy
//!
//! - `CURRENT_SCHEMA_VERSION` (in `manifest`) is the authoritative constant.
//! - Each integer step in `app_schema_version` corresponds to one or more
//!   breaking changes in the bundle layout or required sections.
//! - Additive changes (new optional JSON fields) do NOT require a version bump —
//!   `#[serde(default)]` handles those.
//! - When a bump IS needed, add a `v{N}_to_v{N+1}` function here and call it
//!   from the `migrate` match arm.

use super::ThemePack;
use crate::design_system::theme_pack::BundleError;

impl ThemePack {
    /// Run any in-place schema migrations needed to bring a pack loaded from
    /// `from_version` up to [`CURRENT_SCHEMA_VERSION`].
    ///
    /// Returns `Err(BundleError::SchemaTooNew)` when `from_version` exceeds
    /// the current version (the reader does not know how to handle it).
    ///
    /// # Extension point
    ///
    /// Add a new `match` arm per schema bump:
    ///
    /// ```ignore
    /// 1 => v1_to_v2(self),   // migrate from v1 → v2
    /// 2 => {                 // migrate from v2 → v3
    ///     v1_to_v2(self);    // first bring v1→v2 …
    ///     v2_to_v3(self);    // … then v2→v3
    /// }
    /// ```
    pub(super) fn migrate(&mut self, from_version: u32) -> Result<(), BundleError> {
        use crate::design_system::theme_pack::manifest::CURRENT_SCHEMA_VERSION;

        if from_version > CURRENT_SCHEMA_VERSION {
            return Err(BundleError::SchemaTooNew {
                pack_version:    from_version,
                reader_version:  CURRENT_SCHEMA_VERSION,
            });
        }

        // Dispatch per version.  The pattern "run all migrations from
        // from_version up to current" is achieved by falling through or
        // calling each step in order.
        match from_version {
            // v1 is the baseline — no structural changes needed.
            1 => { /* no-op for v1 */ }

            // Future: add arms here.
            // 1 => { v1_to_v2(self); }

            // version 0 should never appear (schema starts at 1), but
            // handle it gracefully by doing nothing extra.
            0 => { /* treat as v1 baseline */ }

            // Already validated above that from_version ≤ CURRENT, so this
            // arm is unreachable in practice; kept for exhaustiveness.
            _ => {}
        }

        Ok(())
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use crate::design_system::{
        color_scheme::builtin_dark,
        style_system::StyleSystem,
        recipes::RecipeSet,
        theme_pack::{ThemePack, BundleError},
    };
    use crate::ui_kit::assets::AssetRegistry;

    fn minimal_pack(schema: u32) -> ThemePack {
        let mut tp = ThemePack {
            manifest: {
                let mut m = crate::design_system::theme_pack::manifest::ThemeManifest::new(
                    "test", "Test",
                );
                m.app_schema_version = schema;
                m
            },
            color_scheme:  builtin_dark(),
            style_system:  StyleSystem::builtin_default(),
            recipes:       RecipeSet::new(),
            shell_profile: None,
            assets:        AssetRegistry::new(),
        };
        tp
    }

    #[test]
    fn v1_migration_is_noop() {
        let mut pack = minimal_pack(1);
        let result = pack.migrate(1);
        assert!(result.is_ok());
    }

    #[test]
    fn v0_migration_is_noop() {
        let mut pack = minimal_pack(0);
        let result = pack.migrate(0);
        assert!(result.is_ok());
    }

    #[test]
    fn future_version_returns_error() {
        let mut pack = minimal_pack(99);
        let result = pack.migrate(99);
        assert!(matches!(result, Err(BundleError::SchemaTooNew { .. })));
    }
}
