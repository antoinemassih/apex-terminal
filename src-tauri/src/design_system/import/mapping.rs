//! Editable Figma-name → engine-key overrides.
//!
//! Because `apex.tokens.json` names every token to match the engine DTCG field
//! names, the mapping is **identity** for a faithfully-built Figma file and the
//! transformer needs no per-field exceptions. When a designer renames a variable
//! (or a third-party plugin mangles a name), add an entry to `figma-mapping.json`
//! rather than hardcoding the exception in Rust.
//!
//! `figma-mapping.json` shape:
//! ```json
//! {
//!   "ColorScheme": { "brand": "accent" },
//!   "StyleSystem": { "type/body": "typography/size_md" }
//! }
//! ```
//! Keys are Figma variable names; values are the engine path (`/`- or `.`-joined).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Per-collection Figma-name → engine-path overrides.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct FigmaMapping {
    /// collection name → (figma variable name → engine path).
    by_collection: BTreeMap<String, BTreeMap<String, String>>,
}

impl FigmaMapping {
    /// An identity mapping (no overrides) — correct for a faithfully-built file.
    pub fn identity() -> Self {
        FigmaMapping::default()
    }

    /// Parse a `figma-mapping.json` document. An empty / `{}` document is identity.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        if json.trim().is_empty() {
            return Ok(Self::identity());
        }
        serde_json::from_str(json)
    }

    /// Resolve the engine path for a Figma variable, or return the name unchanged
    /// when no override exists.
    pub fn engine_path<'a>(&'a self, collection: &str, figma_name: &'a str) -> String {
        self.by_collection
            .get(collection)
            .and_then(|m| m.get(figma_name))
            .cloned()
            .unwrap_or_else(|| figma_name.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_passes_names_through() {
        let m = FigmaMapping::identity();
        assert_eq!(m.engine_path("ColorScheme", "bg"), "bg");
        assert_eq!(m.engine_path("StyleSystem", "typography/size_xs"), "typography/size_xs");
    }

    #[test]
    fn override_applies_per_collection() {
        let m = FigmaMapping::from_json(r#"{ "ColorScheme": { "brand": "accent" } }"#).unwrap();
        assert_eq!(m.engine_path("ColorScheme", "brand"), "accent");
        // Unlisted names and other collections pass through unchanged.
        assert_eq!(m.engine_path("ColorScheme", "bg"), "bg");
        assert_eq!(m.engine_path("StyleSystem", "brand"), "brand");
    }
}
