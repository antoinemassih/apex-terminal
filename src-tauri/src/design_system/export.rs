//! DTCG export and directory-scan helpers for the built-in theme catalogue.
//!
//! ## Functions
//!
//! - [`export_builtin_themes`] — write every built-in [`ColorScheme`] and
//!   [`StyleSystem`] to `<dir>/colorschemes/<id>.json` /
//!   `<dir>/styles/<id>.json` using DTCG serialization.
//! - [`scan_theme_dir`] — read back any `colorschemes/*.json` and
//!   `styles/*.json` files in a directory tree, skipping files that fail to
//!   parse (graceful, no panic).
//!
//! Neither function runs at startup.  `export_builtin_themes` is called on
//! demand (e.g. from a menu action or CLI flag); `scan_theme_dir` is the
//! "installed themes" scan path for the registry.

use std::{
    fs,
    io::{self, Write},
    path::Path,
};

use serde_json::{json, Value};

use super::{
    builtin::{builtin_color_schemes, builtin_style_systems},
    color_scheme::ColorScheme,
    loader::LoadError,
    style_system::{FocusRingStyle, StyleSystem},
};

// ── StyleSystem::to_dtcg ─────────────────────────────────────────────────────

impl StyleSystem {
    /// Serialize this `StyleSystem` into a W3C DTCG JSON string.
    ///
    /// Mirrors `ColorScheme::to_dtcg`.  Produces the `style.*.json` shape
    /// described in the loader module comment.
    pub fn to_dtcg(&self) -> String {
        macro_rules! dim {
            ($v:expr) => {
                json!({ "$type": "dimension", "$value": $v })
            };
        }
        macro_rules! bool_tok {
            ($v:expr) => {
                json!({ "$type": "boolean", "$value": $v })
            };
        }

        let typ = &self.typography;
        let sp  = &self.spacing;
        let r   = &self.radii;
        let st  = &self.strokes;
        let al  = &self.alphas;
        let el  = &self.elevation;
        let den = &self.density;
        let sh  = &self.shadows;
        let tr  = &self.treatments;

        let focus_ring_str = match tr.focus_ring {
            FocusRingStyle::None    => "none",
            FocusRingStyle::Outline => "outline",
            FocusRingStyle::Glow    => "glow",
        };

        let shadow_obj = |s: &super::style_system::ShadowSpec| -> Value {
            json!({
                "blur":     { "$type": "dimension", "$value": s.blur },
                "spread":   { "$type": "dimension", "$value": s.spread },
                "offset_x": { "$type": "dimension", "$value": s.offset_x },
                "offset_y": { "$type": "dimension", "$value": s.offset_y },
                "alpha":    { "$type": "number",    "$value": s.alpha },
            })
        };

        let root = json!({
            "meta": {
                "id":      self.meta.id,
                "name":    self.meta.name,
                "is_dark": self.meta.is_dark,
            },
            "typography": {
                "size_xs": dim!(typ.size_xs),
                "size_sm": dim!(typ.size_sm),
                "size_md": dim!(typ.size_md),
                "size_lg": dim!(typ.size_lg),
                "size_xl": dim!(typ.size_xl),
                "mono_sm": dim!(typ.mono_sm),
                "mono_md": dim!(typ.mono_md),
                "mono_lg": dim!(typ.mono_lg),
                "size_section_label": dim!(typ.size_section_label),
                "label_tracking": { "$type": "number", "$value": typ.label_tracking },
                "nav_tracking":   { "$type": "number", "$value": typ.nav_tracking },
                "section_tracking": { "$type": "number", "$value": typ.section_tracking },
                "family_ui":      { "$type": "string", "$value": &typ.family_ui },
                "family_mono":    { "$type": "string", "$value": &typ.family_mono },
                "family_display": { "$type": "string", "$value": &typ.family_display },
            },
            "spacing": {
                "xs":         dim!(sp.xs),
                "sm":         dim!(sp.sm),
                "xs_mid":     dim!(sp.xs_mid),
                "md":         dim!(sp.md),
                "lg":         dim!(sp.lg),
                "xl":         dim!(sp.xl),
                "xxl":        dim!(sp.xxl),
                "gmd":        dim!(sp.gmd),
                "cta_height": dim!(sp.cta_height),
            },
            "radii": {
                "none": dim!(r.none),
                "xs":   dim!(r.xs),
                "sm":   dim!(r.sm),
                "md":   dim!(r.md),
                "lg":   dim!(r.lg),
                "full": dim!(r.full),
            },
            "strokes": {
                "hair":   dim!(st.hair),
                "thin":   dim!(st.thin),
                "medium": dim!(st.medium),
                "std":    dim!(st.std),
                "bold":   dim!(st.bold),
                "thick":  dim!(st.thick),
                "md":     dim!(st.md),
                "heavy":  dim!(st.heavy),
            },
            "alphas": {
                // u8 tiers
                "faint":     { "$type": "integer", "$value": al.faint },
                "ghost":     { "$type": "integer", "$value": al.ghost },
                "soft_u8":   { "$type": "integer", "$value": al.soft_u8 },
                "subtle_u8": { "$type": "integer", "$value": al.subtle_u8 },
                "tint":      { "$type": "integer", "$value": al.tint },
                "muted_u8":  { "$type": "integer", "$value": al.muted_u8 },
                "dim":       { "$type": "integer", "$value": al.dim },
                "line":      { "$type": "integer", "$value": al.line },
                "strong_u8": { "$type": "integer", "$value": al.strong_u8 },
                "active":    { "$type": "integer", "$value": al.active },
                "heavy_u8":  { "$type": "integer", "$value": al.heavy_u8 },
                "solid":     { "$type": "integer", "$value": al.solid },
                // f32 multipliers
                "subtle":        { "$type": "number", "$value": al.subtle },
                "soft":          { "$type": "number", "$value": al.soft },
                "muted":         { "$type": "number", "$value": al.muted },
                "mid":           { "$type": "number", "$value": al.mid },
                "strong":        { "$type": "number", "$value": al.strong },
                "opaque":        { "$type": "number", "$value": al.opaque },
                "header_border": { "$type": "number", "$value": al.header_border },
            },
            "elevation": {
                "l1": { "$type": "number", "$value": el.l1 },
                "l2": { "$type": "number", "$value": el.l2 },
                "l3": { "$type": "number", "$value": el.l3 },
            },
            "density": {
                "factor":                   { "$type": "number", "$value": den.factor },
                "row_height_dense":         dim!(den.row_height_dense),
                "row_height_comfortable":   dim!(den.row_height_comfortable),
            },
            "shadows": {
                "card":     shadow_obj(&sh.card),
                "modal":    shadow_obj(&sh.modal),
                "tooltip":  shadow_obj(&sh.tooltip),
                "dropdown": shadow_obj(&sh.dropdown),
            },
            "treatments": {
                "solid_active_fills":       bool_tok!(tr.solid_active_fills),
                "hairline_borders":         bool_tok!(tr.hairline_borders),
                "uppercase_section_labels": bool_tok!(tr.uppercase_section_labels),
                "segmented_filled_idle":    bool_tok!(tr.segmented_filled_idle),
                "focus_ring": { "$type": "string", "$value": focus_ring_str },
            },
        });

        serde_json::to_string_pretty(&root).unwrap_or_default()
    }
}

// ── export_builtin_themes ────────────────────────────────────────────────────

/// Write every built-in [`ColorScheme`] and [`StyleSystem`] to disk in DTCG
/// JSON format.
///
/// Layout:
/// ```text
/// <dir>/
///   colorschemes/<id>.json   — one file per ColorScheme
///   styles/<id>.json         — one file per StyleSystem
/// ```
///
/// Both subdirectories are created if they do not already exist.
///
/// # Returns
///
/// The total number of files written (colorschemes + styles).
///
/// # Errors
///
/// Returns `Err` if a directory cannot be created or a file cannot be written.
/// Individual serialization failures are silently skipped (the count excludes
/// them), but `to_dtcg()` is infallible in practice.
pub fn export_builtin_themes(dir: &Path) -> io::Result<usize> {
    let cs_dir = dir.join("colorschemes");
    let st_dir = dir.join("styles");

    fs::create_dir_all(&cs_dir)?;
    fs::create_dir_all(&st_dir)?;

    let mut count = 0_usize;

    for scheme in builtin_color_schemes() {
        let path = cs_dir.join(format!("{}.json", scheme.meta.id));
        let json = scheme.to_dtcg();
        if !json.is_empty() {
            let mut f = fs::File::create(&path)?;
            f.write_all(json.as_bytes())?;
            count += 1;
        }
    }

    for style in builtin_style_systems() {
        let path = st_dir.join(format!("{}.json", style.meta.id));
        let json = style.to_dtcg();
        if !json.is_empty() {
            let mut f = fs::File::create(&path)?;
            f.write_all(json.as_bytes())?;
            count += 1;
        }
    }

    Ok(count)
}

// ── scan_theme_dir ───────────────────────────────────────────────────────────

/// Scan `<dir>/colorschemes/*.json` and `<dir>/styles/*.json`, parsing each
/// file via the DTCG loader.  Files that fail to parse are skipped with a
/// diagnostic printed to stderr — the function never panics.
///
/// This is the "installed themes" scan path for [`ThemeRegistry`].
///
/// [`ThemeRegistry`]: super::registry::ThemeRegistry
pub fn scan_theme_dir(dir: &Path) -> (Vec<ColorScheme>, Vec<StyleSystem>) {
    let mut schemes = Vec::new();
    let mut styles  = Vec::new();

    // ── colorschemes ─────────────────────────────────────────────────────────
    let cs_dir = dir.join("colorschemes");
    if let Ok(rd) = fs::read_dir(&cs_dir) {
        for entry in rd.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            match fs::read_to_string(&path) {
                Err(e) => eprintln!("[design_system] cannot read {:?}: {e}", path),
                Ok(json) => match ColorScheme::from_dtcg(&json) {
                    Ok(cs) => schemes.push(cs),
                    Err(e) => eprintln!("[design_system] skipping {:?}: {}", path, format_load_error(&e)),
                },
            }
        }
    }

    // ── styles ───────────────────────────────────────────────────────────────
    let st_dir = dir.join("styles");
    if let Ok(rd) = fs::read_dir(&st_dir) {
        for entry in rd.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            match fs::read_to_string(&path) {
                Err(e) => eprintln!("[design_system] cannot read {:?}: {e}", path),
                Ok(json) => match StyleSystem::from_dtcg(&json) {
                    Ok(ss) => styles.push(ss),
                    Err(e) => eprintln!("[design_system] skipping {:?}: {}", path, format_load_error(&e)),
                },
            }
        }
    }

    (schemes, styles)
}

fn format_load_error(e: &LoadError) -> String {
    format!("{e}")
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn style_system_to_dtcg_round_trip() {
        use crate::design_system::builtin::builtin_style_systems;
        let styles = builtin_style_systems();
        for original in &styles {
            let json = original.to_dtcg();
            assert!(!json.is_empty(), "to_dtcg() must not return empty string for '{}'", original.meta.id);
            let parsed = StyleSystem::from_dtcg(&json)
                .unwrap_or_else(|e| panic!("round-trip failed for '{}': {e}", original.meta.id));
            assert_eq!(parsed.meta.id, original.meta.id);
            assert_eq!(parsed.typography.size_sm, original.typography.size_sm);
            assert_eq!(parsed.radii.sm, original.radii.sm);
            assert_eq!(parsed.treatments.solid_active_fills, original.treatments.solid_active_fills);
            assert_eq!(parsed.treatments.focus_ring, original.treatments.focus_ring);
        }
    }

    #[test]
    fn export_and_scan_round_trip() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dir = tmp.path();

        let written = export_builtin_themes(dir).expect("export failed");
        // 21 colorschemes (16 THEMES + 5 React ports) + 9 styles = 30
        assert_eq!(written, 30, "expected 30 files written (21 + 9)");

        let (schemes, styles) = scan_theme_dir(dir);
        assert_eq!(
            schemes.len(), 21,
            "scan_theme_dir must recover all 21 colorschemes, got {}",
            schemes.len()
        );
        assert_eq!(
            styles.len(), 9,
            "scan_theme_dir must recover all 9 style systems, got {}",
            styles.len()
        );
    }
}
