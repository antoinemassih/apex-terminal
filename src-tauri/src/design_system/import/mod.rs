//! # Figma → apex-terminal theme import (the PULL half of the sync tool)
//!
//! Reads a [`FigmaThemeExport`] (a Figma-variables export — see [`model`]) and
//! emits a validated `.apextheme` [`ThemePack`]. The companion PUSH half is the
//! TypeScript plugin under `figma-plugin/`, which produces the same envelope
//! from inside Figma (the only place Figma variables/components can be written).
//!
//! ## Pipeline
//!
//! ```text
//! FigmaThemeExport ──figma_export_to_pack──▶ ThemePack
//!     (per chosen mode: Figma vars → DTCG → ColorScheme/StyleSystem::from_dtcg,
//!      components → RecipeSet)
//! ThemePack ──theme_pack::validate──▶ ValidationReport   (refuse on errors)
//! ThemePack ──write_bundle──▶ <name>.apextheme            (installable via PackRegistry)
//! ```
//!
//! The emitter [`theme_to_figma_export`] is the exact inverse and backs both the
//! round-trip acceptance test and Theme Studio's *Export to Figma* payload.
//!
//! ## Acceptance gate
//!
//! The `tests` below take the built-in **Aperture** theme, push it to the Figma
//! envelope, pull it back, and assert the recovered `ColorScheme` + `StyleSystem`
//! match the builtin field-by-field (via `PartialEq`) and that the pack passes
//! `validate`. This is fully automated; the live-Figma plugin run is documented
//! in `figma-plugin/README.md` and is the only manual step.

pub mod convert;
pub mod mapping;
pub mod model;

pub use convert::{figma_export_to_pack, theme_to_figma_export, ImportError, ImportOptions};
pub use mapping::FigmaMapping;
pub use model::FigmaThemeExport;

use std::path::Path;

use crate::design_system::theme_pack::validate::{validate, AccessibilityMode, ValidationReport};
use crate::design_system::{builtin_color_schemes, builtin_style_systems, ColorScheme, StyleSystem};

/// The built-in **Aperture** `ColorScheme` (the round-trip ground truth).
pub fn aperture_color_scheme() -> ColorScheme {
    builtin_color_schemes()
        .into_iter()
        .find(|s| s.meta.id == "aperture")
        .expect("Aperture color scheme is a built-in")
}

/// The built-in **Aperture** `StyleSystem`.
pub fn aperture_style_system() -> StyleSystem {
    builtin_style_systems()
        .into_iter()
        .find(|s| s.meta.id == "aperture")
        .expect("Aperture style system is a built-in")
}

/// Outcome of [`import_to_bundle`].
#[derive(Debug)]
pub struct ImportOutcome {
    /// The validation report. Inspect [`ValidationReport::is_installable`].
    pub report: ValidationReport,
    /// `true` if the bundle was written (only when the report is installable).
    pub written: bool,
}

/// Transform a Figma export into a `.apextheme` bundle, gated by `validate`.
///
/// Builds the pack, validates it in [`AccessibilityMode::Standard`], and writes
/// `out_path` **only when there are no errors** (warnings do not block). The
/// returned [`ImportOutcome`] always carries the full report so the caller can
/// surface it to the user.
pub fn import_to_bundle(
    export: &FigmaThemeExport,
    out_path: &Path,
    opts: &ImportOptions,
    mapping: &FigmaMapping,
) -> Result<ImportOutcome, ImportError> {
    let pack = figma_export_to_pack(export, opts, mapping)?;
    let report = validate(&pack, AccessibilityMode::Standard);
    let written = if report.is_installable() {
        pack.write_bundle(out_path).map_err(|e| ImportError::Bundle(format!("{e:?}")))?;
        true
    } else {
        false
    };
    Ok(ImportOutcome { report, written })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::design_system::color_scheme::builtin_dark;
    use crate::design_system::theme_pack::ThemePack;
    use crate::ui_kit::sx::recipe_spec::{AlphaTier, 
        ColorSpec, PadTier, RadiusTier, RecipeDelta, RecipeSpec, ShadeRef, ToneRef,
    };
    use std::collections::BTreeMap;

    /// A small set of adopted recipe keys, exercised by the round-trip.
    fn sample_components() -> BTreeMap<String, RecipeSpec> {
        let mut m = BTreeMap::new();
        m.insert(
            "tag".to_string(),
            RecipeSpec {
                base: RecipeDelta {
                    radius: Some(RadiusTier::Pill),
                    fill: Some(ColorSpec::Alpha { tone: ToneRef::Accent, alpha: AlphaTier::Raw(32) }),
                    text: None,
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        m.insert(
            "row.list".to_string(),
            RecipeSpec {
                base: RecipeDelta { radius: Some(RadiusTier::None), py: Some(PadTier::Xs), ..Default::default() },
                selected: Some(RecipeDelta {
                    fill: Some(ColorSpec::Alpha { tone: ToneRef::Accent, alpha: AlphaTier::Raw(24) }),
                    ..Default::default()
                }),
                ..Default::default()
            },
        );
        m.insert(
            "tab.line.active".to_string(),
            RecipeSpec {
                base: RecipeDelta {
                    fill: Some(ColorSpec::Tone { tone: ToneRef::Accent, shade: Some(ShadeRef::S500) }),
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        m
    }

    /// Build a two-mode (Aperture/Meridien) Figma export from the builtins.
    fn aperture_meridien_export(components: &BTreeMap<String, RecipeSpec>) -> FigmaThemeExport {
        let aperture_cs = aperture_color_scheme();
        let aperture_ss = aperture_style_system();

        // Meridien color = the generic dark palette (per figma/README.md), tagged
        // as the Meridien mode; Meridien style = the builtin Meridien preset.
        let mut meridien_cs = builtin_dark();
        meridien_cs.meta.id = "meridien".into();
        meridien_cs.meta.name = "Meridien".into();
        let meridien_ss = StyleSystem::meridien();

        theme_to_figma_export(
            &[("Aperture".into(), aperture_cs), ("Meridien".into(), meridien_cs)],
            &[("Aperture".into(), aperture_ss), ("Meridien".into(), meridien_ss)],
            components,
        )
    }

    /// THE ACCEPTANCE GATE — fully automated, no live Figma needed.
    ///
    /// Push Aperture into the Figma envelope, pull it back, and assert exact
    /// `ColorScheme` + `StyleSystem` equality plus a passing `validate`.
    #[test]
    fn aperture_round_trip_in_memory() {
        let components = sample_components();
        let export = aperture_meridien_export(&components);

        let pack = figma_export_to_pack(&export, &ImportOptions::default(), &FigmaMapping::identity())
            .expect("import should succeed");

        // Field-by-field equality against the builtin (PartialEq on the whole struct).
        assert_eq!(pack.color_scheme, aperture_color_scheme(), "ColorScheme must match Aperture builtin");
        assert_eq!(pack.style_system, aperture_style_system(), "StyleSystem must match Aperture builtin");

        // Recipes survived the JSON round-trip.
        assert_eq!(pack.recipes.len(), components.len(), "recipe count preserved");
        for (key, spec) in &components {
            let got = pack.recipes.get(key).expect("recipe key present");
            assert_eq!(
                serde_json::to_value(got).unwrap(),
                serde_json::to_value(spec).unwrap(),
                "recipe {key} must round-trip"
            );
        }

        // Validation gate.
        let report = validate(&pack, AccessibilityMode::Standard);
        assert!(report.is_installable(), "Aperture pack must be installable; report: {report}, errors: {:?}", report.errors);
    }

    /// The full pack write→read bundle round-trip preserves both axes.
    #[test]
    fn aperture_bundle_write_read_round_trip() {
        let components = sample_components();
        let export = aperture_meridien_export(&components);
        let pack = figma_export_to_pack(&export, &ImportOptions::default(), &FigmaMapping::identity())
            .expect("import should succeed");

        let tmp = std::env::temp_dir().join("apex_figma_import_roundtrip.apextheme");
        pack.write_bundle(&tmp).expect("write bundle");
        let back = ThemePack::read_bundle(&tmp).expect("read bundle");
        let _ = std::fs::remove_file(&tmp);

        assert_eq!(back.color_scheme, aperture_color_scheme());
        assert_eq!(back.style_system, aperture_style_system());
        assert_eq!(back.recipes.len(), components.len());
    }

    /// Importing the *Meridien* mode yields the Meridien style preset — proving
    /// the two-axis mode switch crosses the bridge intact.
    #[test]
    fn meridien_mode_imports_meridien_style() {
        let components = sample_components();
        let export = aperture_meridien_export(&components);
        let opts = ImportOptions { color_mode: Some("Meridien".into()), style_mode: Some("Meridien".into()) };
        let pack = figma_export_to_pack(&export, &opts, &FigmaMapping::identity())
            .expect("import should succeed");
        assert_eq!(pack.style_system, StyleSystem::meridien(), "Meridien mode must recover the Meridien preset");
    }

    /// Generates the committed fixture `fixtures/aperture_figma_export.json`.
    /// Ignored in normal runs; regenerate with:
    /// `cargo test -p <crate> design_system::import::tests::write_aperture_fixture -- --ignored`
    #[test]
    #[ignore]
    fn write_aperture_fixture() {
        let components = sample_components();
        let export = aperture_meridien_export(&components);
        let json = serde_json::to_string_pretty(&export).expect("serialise export");
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/design_system/import/fixtures/aperture_figma_export.json");
        std::fs::create_dir_all(path.parent().unwrap()).expect("mkdir fixtures");
        std::fs::write(&path, json).expect("write fixture");
        eprintln!("wrote fixture to {}", path.display());
    }

    /// Guards the TS-plugin ↔ Rust contract: a hand-authored JSON in the exact
    /// shape the plugin's `buildFigmaThemeExportJson` emits (single mode,
    /// synthesized meta, enum as FLOAT integer, `cmd_palette/<slot>`) must parse
    /// and import. If a serde field name drifts (e.g. `valuesByMode`,
    /// `colorCollection`, `type`), this fails.
    #[test]
    fn plugin_shaped_export_imports() {
        use crate::design_system::style_system::FocusRingStyle;
        let json = r#"{
          "schema": 1,
          "generator": "apex-figma-sync plugin export",
          "colorCollection": {
            "name": "ColorScheme",
            "defaultMode": "Aperture",
            "modes": ["Aperture"],
            "meta": { "Aperture": { "id": "aperture", "name": "Aperture", "is_dark": true } }
          },
          "styleCollection": {
            "name": "StyleSystem",
            "defaultMode": "Aperture",
            "modes": ["Aperture"],
            "meta": { "Aperture": { "id": "aperture", "name": "Aperture", "is_dark": true } }
          },
          "variables": [
            { "collection": "ColorScheme", "name": "bg",      "type": "COLOR", "valuesByMode": { "Aperture": { "r": 0.0, "g": 0.0, "b": 0.0, "a": 1.0 } } },
            { "collection": "ColorScheme", "name": "accent",  "type": "COLOR", "valuesByMode": { "Aperture": { "r": 0.937, "g": 0.357, "b": 0.231, "a": 1.0 } } },
            { "collection": "ColorScheme", "name": "cmd_palette/symbol", "type": "COLOR", "valuesByMode": { "Aperture": { "r": 0.47, "g": 0.70, "b": 1.0, "a": 1.0 } } },
            { "collection": "StyleSystem", "name": "radii/sm",  "type": "FLOAT",   "valuesByMode": { "Aperture": 10 } },
            { "collection": "StyleSystem", "name": "treatments/focus_ring", "type": "FLOAT", "description": "enum focus_ring: 0=none 1=outline 2=glow", "valuesByMode": { "Aperture": 2 } },
            { "collection": "StyleSystem", "name": "treatments/shadows_enabled", "type": "BOOLEAN", "valuesByMode": { "Aperture": true } },
            { "collection": "StyleSystem", "name": "typography/family_ui", "type": "STRING", "valuesByMode": { "Aperture": "Inter" } }
          ],
          "components": { "tag": { "radius": "pill", "fill": { "kind": "tone", "tone": "accent" } } }
        }"#;

        let export: FigmaThemeExport = serde_json::from_str(json).expect("plugin JSON parses into FigmaThemeExport");
        let pack = figma_export_to_pack(&export, &ImportOptions::default(), &FigmaMapping::identity())
            .expect("plugin-shaped export imports");

        // Colour read correctly (0.937*255≈239 = #ef accent red).
        assert_eq!(pack.color_scheme.accent, [239, 91, 59, 255]);
        assert_eq!(pack.color_scheme.meta.id, "aperture");
        // Enum FLOAT 2 → "glow".
        assert_eq!(pack.style_system.treatments.focus_ring, FocusRingStyle::Glow);
        assert_eq!(pack.style_system.radii.sm, 10.0);
        // Component recipe mapped.
        assert!(pack.recipes.get("tag").is_some(), "tag recipe imported");

        let report = validate(&pack, AccessibilityMode::Standard);
        assert!(report.is_installable(), "plugin-shaped pack installable; errors: {:?}", report.errors);
    }

    /// Loads the committed Figma-export fixture and round-trips it. This proves
    /// the importer works on a *saved artifact* (what a live `get_figma_data`
    /// pull resembles), independent of the in-memory emitter.
    #[test]
    fn committed_fixture_round_trips() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/design_system/import/fixtures/aperture_figma_export.json");
        let json = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read fixture {} (regenerate with the ignored write_aperture_fixture test): {e}", path.display()));
        let export: FigmaThemeExport = serde_json::from_str(&json).expect("parse fixture");

        let pack = figma_export_to_pack(&export, &ImportOptions::default(), &FigmaMapping::identity())
            .expect("import fixture");
        assert_eq!(pack.color_scheme, aperture_color_scheme(), "fixture ColorScheme must match Aperture");
        assert_eq!(pack.style_system, aperture_style_system(), "fixture StyleSystem must match Aperture");
        let report = validate(&pack, AccessibilityMode::Standard);
        assert!(report.is_installable(), "fixture pack installable; errors: {:?}", report.errors);
    }
}
