//! Indicator references. The chart owns indicators independently of any
//! template — a template *applies* a default set, but the chart's own list is
//! what the renderer consumes.

use arcstr::ArcStr;
use serde::{Deserialize, Serialize};

use super::extension_bag::ExtensionBag;

/// Each indicator owns its own param schema version. Independent of the XOL
/// schema_version — they evolve on different cadences.
pub type ParamSchemaVersion = u32;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndicatorRef {
    /// E.g., `apex.vwap`, `thirdparty.acme.gex`.
    pub ref_id: ArcStr,
    pub ref_version: u32,
    pub param_schema_version: ParamSchemaVersion,

    /// Pane key. The template (or the renderer's default layout) maps this to
    /// an actual pane index.
    pub pane: ArcStr,

    /// Indicator-specific params. Stays as a JSON value rather than a typed
    /// struct so each indicator owns its own schema independently.
    pub params: serde_json::Value,

    /// Optional style overrides (line color, band fills, etc.).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<serde_json::Value>,

    /// True when the indicator is installed locally and renderable.
    /// False = placeholder / install prompt. Default true on insert; flipped
    /// by the loader if the registry can't resolve `ref_id`.
    #[serde(default = "default_true")]
    pub installed_locally: bool,

    #[serde(default, skip_serializing_if = "ExtensionBag::is_empty")]
    pub extras: ExtensionBag,
}

fn default_true() -> bool {
    true
}
