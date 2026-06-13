//! Data model for the Figma ⇄ apex-terminal theme bridge.
//!
//! [`FigmaThemeExport`] is the **shared contract** between the two halves of the
//! sync tool:
//!
//! - the **PUSH** Figma plugin (`figma-plugin/`) creates real Figma variables
//!   from this shape (and can serialise the file's variables back into it), and
//! - the **PULL** transformer (this module) reads it and emits a `.apextheme`.
//!
//! The layout deliberately mirrors the Figma REST `GET /v1/files/:key/variables/local`
//! response — a flat list of variables, each carrying its collection, name,
//! resolved type, and per-mode values — so the same [`crate::design_system::import`]
//! code path works on a live `get_figma_data` pull and on a saved fixture alike.
//!
//! ## Why an envelope around DTCG instead of a bespoke parser
//!
//! The engine already round-trips [`ColorScheme`](crate::design_system::ColorScheme)
//! and [`StyleSystem`](crate::design_system::StyleSystem) losslessly through W3C
//! DTCG JSON (`from_dtcg` / `to_dtcg`). The transformer therefore only translates
//! between this Figma-variable envelope and DTCG, then defers to the proven
//! engine parser. The only hand-written conversions are:
//!
//! 1. colour `#rrggbb[aa]` ↔ Figma `{r,g,b,a}` floats (exact for `u8` via round),
//! 2. the four **string-enum** style fields ↔ Figma `FLOAT` integers
//!    (`focus_ring`, `surface_bevel`, `pane_active_indicator`, `button_group`), and
//! 3. the 11-slot `cmd_palette` array ↔ 11 named Figma colour variables.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Collection name for the palette axis (a Figma mode = a `ColorScheme`).
pub const COLOR_COLLECTION: &str = "ColorScheme";
/// Collection name for the dimension axis (a Figma mode = a `StyleSystem`).
pub const STYLE_COLLECTION: &str = "StyleSystem";
/// Default mode name used when an export does not name one explicitly.
pub const DEFAULT_MODE: &str = "Aperture";

/// The 11 `cmd_palette` slots, in the engine's array order. Figma exposes them
/// as named colour variables `cmd_palette/<slot>` because Tokens Studio / Figma
/// variables have no array type.
pub const CMD_PALETTE_SLOTS: [&str; 11] = [
    "symbol", "widget", "overlay", "theme", "timeframe", "layout", "play",
    "alert", "ai", "dynamic", "calc",
];

/// The four style fields the engine serialises as DTCG **strings** but that Figma
/// represents as `FLOAT` integer variables (Tokens Studio has no enum type). The
/// canonical string ↔ integer mapping is taken verbatim from
/// `design_system::export` / `style_system`.
///
/// Path form is the dot-joined DTCG path (`section.field`).
pub fn enum_int_to_str(path: &str, n: i64) -> Option<&'static str> {
    Some(match (path, n) {
        ("treatments.focus_ring", 0) => "none",
        ("treatments.focus_ring", 1) => "outline",
        ("treatments.focus_ring", 2) => "glow",
        ("treatments.surface_bevel", 0) => "none",
        ("treatments.surface_bevel", 1) => "raised",
        ("treatments.surface_bevel", 2) => "inset",
        ("chrome.pane_active_indicator", 0) => "none",
        ("chrome.pane_active_indicator", 1) => "top_stripe",
        ("chrome.pane_active_indicator", 2) => "header_fill",
        ("chrome.pane_active_indicator", 3) => "both",
        ("chrome.button_group", 0) => "none",
        ("chrome.button_group", 1) => "bordered",
        ("chrome.button_group", 2) => "frosted",
        ("chrome.button_group", 3) => "sharp",
        _ => return None,
    })
}

/// Inverse of [`enum_int_to_str`] — canonical string → Figma integer.
pub fn enum_str_to_int(path: &str, s: &str) -> Option<i64> {
    Some(match (path, s) {
        ("treatments.focus_ring", "none") => 0,
        ("treatments.focus_ring", "outline") => 1,
        ("treatments.focus_ring", "glow") => 2,
        ("treatments.surface_bevel", "none") => 0,
        ("treatments.surface_bevel", "raised") => 1,
        ("treatments.surface_bevel", "inset") => 2,
        ("chrome.pane_active_indicator", "none") => 0,
        ("chrome.pane_active_indicator", "top_stripe") => 1,
        ("chrome.pane_active_indicator", "header_fill") => 2,
        ("chrome.pane_active_indicator", "both") => 3,
        ("chrome.button_group", "none") => 0,
        ("chrome.button_group", "bordered") => 1,
        ("chrome.button_group", "frosted") => 2,
        ("chrome.button_group", "sharp") => 3,
        _ => return None,
    })
}

/// True if the dot-joined DTCG path is one of the four string-enum style fields.
pub fn is_enum_path(path: &str) -> bool {
    matches!(
        path,
        "treatments.focus_ring"
            | "treatments.surface_bevel"
            | "chrome.pane_active_indicator"
            | "chrome.button_group"
    )
}

// ── Figma value forms ───────────────────────────────────────────────────────

/// A Figma colour-variable value: linear-unmultiplied RGBA floats in `0.0..=1.0`,
/// exactly as the Figma REST API and Plugin API return them.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct FigmaColor {
    pub r: f64,
    pub g: f64,
    pub b: f64,
    #[serde(default = "one")]
    pub a: f64,
}

fn one() -> f64 {
    1.0
}

impl FigmaColor {
    /// Convert from an `#rrggbb` / `#rrggbbaa` hex string. Unknown formats yield
    /// transparent black (the engine's own `from_dtcg` rejects those upstream).
    pub fn from_hex(hex: &str) -> Self {
        let h = hex.trim_start_matches('#');
        let byte = |s: &str| u8::from_str_radix(s, 16).unwrap_or(0) as f64 / 255.0;
        match h.len() {
            6 => FigmaColor { r: byte(&h[0..2]), g: byte(&h[2..4]), b: byte(&h[4..6]), a: 1.0 },
            8 => FigmaColor {
                r: byte(&h[0..2]),
                g: byte(&h[2..4]),
                b: byte(&h[4..6]),
                a: byte(&h[6..8]),
            },
            _ => FigmaColor { r: 0.0, g: 0.0, b: 0.0, a: 0.0 },
        }
    }

    /// Convert to a `#rrggbbaa` hex string (always 8 digits). `u8` → `f64` → `u8`
    /// via `round` is exact, so this is a lossless round-trip for engine colours.
    pub fn to_hex(&self) -> String {
        let q = |v: f64| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
        format!("#{:02x}{:02x}{:02x}{:02x}", q(self.r), q(self.g), q(self.b), q(self.a))
    }
}

/// The resolved type of a Figma variable. Mirrors `VariableResolvedDataType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum FigmaType {
    Color,
    Float,
    Boolean,
    String,
}

/// A per-mode variable value. Untagged so the JSON is the bare value Figma stores.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FigmaValue {
    Color(FigmaColor),
    Bool(bool),
    Num(f64),
    Str(String),
}

/// One Figma variable: which collection it lives in, its name (`group/leaf`
/// path), resolved type, and its value in each mode.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FigmaVariable {
    pub collection: String,
    /// Figma variable name. Groups are `/`-separated, e.g. `typography/size_xs`,
    /// `shadows/card/blur`, `cmd_palette/symbol`. Palette colours are flat (`bg`).
    pub name: String,
    #[serde(rename = "type")]
    pub ty: FigmaType,
    /// Value per mode name. Color collection modes are `ColorScheme`s; style
    /// collection modes are `StyleSystem`s.
    #[serde(rename = "valuesByMode")]
    pub values_by_mode: BTreeMap<String, FigmaValue>,
    /// Optional human description. For enum FLOATs it carries the canonical
    /// string set (e.g. `"enum focus_ring: 0=none 1=outline 2=glow"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Identity + dark flag for one mode of a collection (a `ColorScheme` /
/// `StyleSystem` `meta`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModeMeta {
    pub id: String,
    pub name: String,
    #[serde(default = "default_true")]
    pub is_dark: bool,
}

fn default_true() -> bool {
    true
}

/// A variable collection (`ColorScheme` or `StyleSystem`) and its modes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CollectionInfo {
    pub name: String,
    #[serde(rename = "defaultMode")]
    pub default_mode: String,
    pub modes: Vec<String>,
    /// `meta` per mode (id/name/is_dark) used to populate the engine `meta`.
    #[serde(default)]
    pub meta: BTreeMap<String, ModeMeta>,
}

/// The top-level Figma export envelope — the shared PUSH/PULL contract.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FigmaThemeExport {
    /// Envelope schema version. Current = 1.
    pub schema: u32,
    /// Free-text provenance string.
    #[serde(default)]
    pub generator: String,
    #[serde(rename = "colorCollection")]
    pub color_collection: CollectionInfo,
    #[serde(rename = "styleCollection")]
    pub style_collection: CollectionInfo,
    /// Flat variable list (Figma REST shape).
    pub variables: Vec<FigmaVariable>,
    /// Component recipes keyed by recipe key (`button.primary`, `row.list`, …).
    /// Each value is a `RecipeSpec` in its canonical JSON form.
    #[serde(default)]
    pub components: BTreeMap<String, serde_json::Value>,
}

impl FigmaThemeExport {
    /// All variables belonging to `collection`.
    pub fn vars_in<'a>(&'a self, collection: &'a str) -> impl Iterator<Item = &'a FigmaVariable> + 'a {
        self.variables.iter().filter(move |v| v.collection == collection)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_color_round_trips_exactly_for_all_u8() {
        // Every u8 channel survives hex → float → hex.
        for &byte in &[0u8, 1, 18, 60, 127, 128, 200, 254, 255] {
            let hex = format!("#{0:02x}{0:02x}{0:02x}{0:02x}", byte);
            let c = FigmaColor::from_hex(&hex);
            assert_eq!(c.to_hex(), hex, "channel {byte} did not round-trip");
        }
    }

    #[test]
    fn enum_tables_are_inverse() {
        for path in [
            "treatments.focus_ring",
            "treatments.surface_bevel",
            "chrome.pane_active_indicator",
            "chrome.button_group",
        ] {
            assert!(is_enum_path(path));
            for n in 0..4 {
                if let Some(s) = enum_int_to_str(path, n) {
                    assert_eq!(enum_str_to_int(path, s), Some(n), "{path} {n}");
                }
            }
        }
    }
}
