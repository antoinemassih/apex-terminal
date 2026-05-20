//! DTCG token JSON loader — parse W3C Design Token Community Group format.
//!
//! Two file *kinds* (per spec §6):
//!
//! ```json
//! // colorscheme.dracula.json
//! { "meta": { "id": "dracula", "name": "Dracula", "is_dark": true },
//!   "palette": {
//!     "bg":     { "$type": "color", "$value": "#282a36" },
//!     "accent": { "$type": "color", "$value": "#bd93f9" } } }
//!
//! // style.meridien.json
//! { "meta": { "id": "meridien", "name": "Meridien", "is_dark": true },
//!   "typography": { "size_sm": { "$type": "dimension", "$value": 11 } },
//!   "treatments": { "solid_active_fills": { "$type": "boolean", "$value": true } } }
//! ```
//!
//! Each leaf value is wrapped as `{ "$type": "...", "$value": ... }`.  Missing
//! fields fall back to the matching `Default` implementation so sparse files
//! (e.g. partial overrides) work without specifying every token.
//!
//! ## Entry points
//! - [`StyleSystem::from_dtcg`] — parse a dimension-axis DTCG file.
//! - [`ColorScheme::from_dtcg`] — parse a palette-axis DTCG file.

use serde_json::Value;
use std::fmt;

use super::{
    color_scheme::{rgba, ColorScheme, Meta, Rgba},
    style_system::{
        Alphas, Density, Elevation, FocusRingStyle, Radii, Shadows, ShadowSpec, Spacing, Strokes,
        StyleSystem, Treatments, Typography,
    },
};

// ── Error ─────────────────────────────────────────────────────────────────────

/// Error returned by the DTCG loader.
#[derive(Debug)]
pub enum LoadError {
    /// `serde_json` parse failure.
    Json(serde_json::Error),
    /// A required top-level key is missing.
    MissingKey(String),
    /// A token value has an unexpected type or shape.
    InvalidToken { path: String, reason: String },
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoadError::Json(e) => write!(f, "JSON parse error: {e}"),
            LoadError::MissingKey(k) => write!(f, "missing required key: {k}"),
            LoadError::InvalidToken { path, reason } => {
                write!(f, "invalid token at {path}: {reason}")
            }
        }
    }
}

impl From<serde_json::Error> for LoadError {
    fn from(e: serde_json::Error) -> Self {
        LoadError::Json(e)
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Extract `$value` from a DTCG token object `{ "$type": "...", "$value": ... }`.
fn dtcg_value<'v>(node: &'v Value, path: &str) -> Result<&'v Value, LoadError> {
    node.get("$value").ok_or_else(|| LoadError::InvalidToken {
        path: path.to_string(),
        reason: "missing $value".to_string(),
    })
}

/// Read an `f32` from a DTCG dimension/number token.
fn read_f32(node: &Value, path: &str) -> Result<f32, LoadError> {
    let v = dtcg_value(node, path)?;
    v.as_f64()
        .map(|n| n as f32)
        .ok_or_else(|| LoadError::InvalidToken {
            path: path.to_string(),
            reason: format!("expected number, got {v}"),
        })
}

/// Read an `f32` from a DTCG node, falling back to `default` if the key is absent.
fn read_f32_or(obj: &Value, key: &str, ctx: &str, default: f32) -> f32 {
    let path = format!("{ctx}.{key}");
    obj.get(key)
        .and_then(|n| read_f32(n, &path).ok())
        .unwrap_or(default)
}

/// Read a `bool` from a DTCG boolean token.
fn read_bool(node: &Value, path: &str) -> Result<bool, LoadError> {
    let v = dtcg_value(node, path)?;
    v.as_bool()
        .ok_or_else(|| LoadError::InvalidToken {
            path: path.to_string(),
            reason: format!("expected boolean, got {v}"),
        })
}

fn read_bool_or(obj: &Value, key: &str, ctx: &str, default: bool) -> bool {
    let path = format!("{ctx}.{key}");
    obj.get(key)
        .and_then(|n| read_bool(n, &path).ok())
        .unwrap_or(default)
}

/// Read an `Rgba` from a DTCG color token (`$value` is `"#rrggbb"` or `"#rrggbbaa"`).
fn read_color(node: &Value, path: &str) -> Result<Rgba, LoadError> {
    let v = dtcg_value(node, path)?;
    let hex = v.as_str().ok_or_else(|| LoadError::InvalidToken {
        path: path.to_string(),
        reason: format!("expected hex color string, got {v}"),
    })?;
    rgba::from_hex(hex).ok_or_else(|| LoadError::InvalidToken {
        path: path.to_string(),
        reason: format!("cannot parse hex color: {hex}"),
    })
}

fn read_color_or(obj: &Value, key: &str, ctx: &str, default: Rgba) -> Rgba {
    let path = format!("{ctx}.{key}");
    obj.get(key)
        .and_then(|n| read_color(n, &path).ok())
        .unwrap_or(default)
}

/// Parse a `Meta` object from the root of a DTCG file.
fn parse_meta(root: &Value) -> Result<Meta, LoadError> {
    let m = root
        .get("meta")
        .ok_or_else(|| LoadError::MissingKey("meta".to_string()))?;
    let id = m
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| LoadError::MissingKey("meta.id".to_string()))?
        .to_string();
    let name = m
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or(&id)
        .to_string();
    let is_dark = m.get("is_dark").and_then(|v| v.as_bool()).unwrap_or(true);
    Ok(Meta { id, name, is_dark })
}

// ── DTCG → ColorScheme ────────────────────────────────────────────────────────

impl ColorScheme {
    /// Parse a W3C DTCG palette-axis JSON string into a `ColorScheme`.
    ///
    /// Missing palette entries fall back to `builtin_dark()` equivalents so
    /// partial / override files work without specifying every colour.
    pub fn from_dtcg(json: &str) -> Result<Self, LoadError> {
        let root: Value = serde_json::from_str(json)?;
        let meta = parse_meta(&root)?;

        let fallback = super::color_scheme::builtin_dark();
        let pal = root.get("palette").cloned().unwrap_or(Value::Object(Default::default()));

        let bg      = read_color_or(&pal, "bg",      "palette", fallback.bg);
        let surface = read_color_or(&pal, "surface",  "palette", fallback.surface);
        let paper   = read_color_or(&pal, "paper",    "palette", fallback.paper);
        let text    = read_color_or(&pal, "text",     "palette", fallback.text);
        let dim     = read_color_or(&pal, "dim",      "palette", fallback.dim);
        let border  = read_color_or(&pal, "border",   "palette", fallback.border);
        let accent  = read_color_or(&pal, "accent",   "palette", fallback.accent);
        let bull    = read_color_or(&pal, "bull",     "palette", fallback.bull);
        let bear    = read_color_or(&pal, "bear",     "palette", fallback.bear);
        let warn    = read_color_or(&pal, "warn",     "palette", fallback.warn);
        let shadow  = read_color_or(&pal, "shadow",   "palette", fallback.shadow);

        // accent_alts: optional array of hex strings (plain or DTCG-wrapped)
        let accent_alts = pal
            .get("accent_alts")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|item| {
                        // Support both plain `"#hex"` and DTCG `{ "$type": "color", "$value": "#hex" }`
                        if let Some(s) = item.as_str() {
                            rgba::from_hex(s)
                        } else {
                            read_color(item, "palette.accent_alts[]").ok()
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok(ColorScheme { meta, bg, surface, paper, text, dim, border, accent, bull, bear, warn, shadow, accent_alts })
    }

    /// Serialize this `ColorScheme` into a W3C DTCG JSON string.
    ///
    /// Produces the `colorscheme.*.json` shape described in spec §6.
    pub fn to_dtcg(&self) -> String {
        let mut pal = serde_json::Map::new();

        macro_rules! color_token {
            ($field:ident) => {{
                let hex = rgba::to_hex(self.$field);
                serde_json::json!({ "$type": "color", "$value": hex })
            }};
        }

        pal.insert("bg".into(),      color_token!(bg));
        pal.insert("surface".into(), color_token!(surface));
        pal.insert("paper".into(),   color_token!(paper));
        pal.insert("text".into(),    color_token!(text));
        pal.insert("dim".into(),     color_token!(dim));
        pal.insert("border".into(),  color_token!(border));
        pal.insert("accent".into(),  color_token!(accent));
        pal.insert("bull".into(),    color_token!(bull));
        pal.insert("bear".into(),    color_token!(bear));
        pal.insert("warn".into(),    color_token!(warn));
        pal.insert("shadow".into(),  color_token!(shadow));

        if !self.accent_alts.is_empty() {
            let alts: Vec<Value> = self
                .accent_alts
                .iter()
                .map(|c| {
                    let hex = rgba::to_hex(*c);
                    serde_json::json!({ "$type": "color", "$value": hex })
                })
                .collect();
            pal.insert("accent_alts".into(), Value::Array(alts));
        }

        let root = serde_json::json!({
            "meta": {
                "id":      self.meta.id,
                "name":    self.meta.name,
                "is_dark": self.meta.is_dark,
            },
            "palette": Value::Object(pal),
        });

        serde_json::to_string_pretty(&root).unwrap_or_default()
    }
}

// ── DTCG → StyleSystem ────────────────────────────────────────────────────────

impl StyleSystem {
    /// Parse a W3C DTCG dimension-axis JSON string into a `StyleSystem`.
    ///
    /// Missing entries fall back to the `Default` implementation of each
    /// sub-struct, so sparse override files work.
    pub fn from_dtcg(json: &str) -> Result<Self, LoadError> {
        let root: Value = serde_json::from_str(json)?;
        let meta = parse_meta(&root)?;

        // Helper: get section or empty object
        let section = |key: &str| -> Value {
            root.get(key).cloned().unwrap_or(Value::Object(Default::default()))
        };

        let d_typ = Typography::default();
        let typ_sec = section("typography");
        let typography = Typography {
            size_xs: read_f32_or(&typ_sec, "size_xs", "typography", d_typ.size_xs),
            size_sm: read_f32_or(&typ_sec, "size_sm", "typography", d_typ.size_sm),
            size_md: read_f32_or(&typ_sec, "size_md", "typography", d_typ.size_md),
            size_lg: read_f32_or(&typ_sec, "size_lg", "typography", d_typ.size_lg),
            size_xl: read_f32_or(&typ_sec, "size_xl", "typography", d_typ.size_xl),
            mono_sm: read_f32_or(&typ_sec, "mono_sm", "typography", d_typ.mono_sm),
            mono_md: read_f32_or(&typ_sec, "mono_md", "typography", d_typ.mono_md),
            mono_lg: read_f32_or(&typ_sec, "mono_lg", "typography", d_typ.mono_lg),
        };

        let d_sp = Spacing::default();
        let sp_sec = section("spacing");
        let spacing = Spacing {
            xs:         read_f32_or(&sp_sec, "xs",         "spacing", d_sp.xs),
            sm:         read_f32_or(&sp_sec, "sm",         "spacing", d_sp.sm),
            md:         read_f32_or(&sp_sec, "md",         "spacing", d_sp.md),
            lg:         read_f32_or(&sp_sec, "lg",         "spacing", d_sp.lg),
            xl:         read_f32_or(&sp_sec, "xl",         "spacing", d_sp.xl),
            xxl:        read_f32_or(&sp_sec, "xxl",        "spacing", d_sp.xxl),
            gmd:        read_f32_or(&sp_sec, "gmd",        "spacing", d_sp.gmd),
            cta_height: read_f32_or(&sp_sec, "cta_height", "spacing", d_sp.cta_height),
        };

        let d_r = Radii::default();
        let r_sec = section("radii");
        let radii = Radii {
            none: read_f32_or(&r_sec, "none", "radii", d_r.none),
            xs:   read_f32_or(&r_sec, "xs",   "radii", d_r.xs),
            sm:   read_f32_or(&r_sec, "sm",   "radii", d_r.sm),
            md:   read_f32_or(&r_sec, "md",   "radii", d_r.md),
            lg:   read_f32_or(&r_sec, "lg",   "radii", d_r.lg),
            full: read_f32_or(&r_sec, "full", "radii", d_r.full),
        };

        let d_st = Strokes::default();
        let st_sec = section("strokes");
        let strokes = Strokes {
            thin:  read_f32_or(&st_sec, "thin",  "strokes", d_st.thin),
            std:   read_f32_or(&st_sec, "std",   "strokes", d_st.std),
            md:    read_f32_or(&st_sec, "md",    "strokes", d_st.md),
            heavy: read_f32_or(&st_sec, "heavy", "strokes", d_st.heavy),
        };

        let d_al = Alphas::default();
        let al_sec = section("alphas");
        let alphas = Alphas {
            subtle:        read_f32_or(&al_sec, "subtle",        "alphas", d_al.subtle),
            soft:          read_f32_or(&al_sec, "soft",          "alphas", d_al.soft),
            muted:         read_f32_or(&al_sec, "muted",         "alphas", d_al.muted),
            mid:           read_f32_or(&al_sec, "mid",           "alphas", d_al.mid),
            strong:        read_f32_or(&al_sec, "strong",        "alphas", d_al.strong),
            opaque:        read_f32_or(&al_sec, "opaque",        "alphas", d_al.opaque),
            header_border: read_f32_or(&al_sec, "header_border", "alphas", d_al.header_border),
        };

        let d_el = Elevation::default();
        let el_sec = section("elevation");
        let elevation = Elevation {
            l1: read_f32_or(&el_sec, "l1", "elevation", d_el.l1),
            l2: read_f32_or(&el_sec, "l2", "elevation", d_el.l2),
            l3: read_f32_or(&el_sec, "l3", "elevation", d_el.l3),
        };

        let d_den = Density::default();
        let den_sec = section("density");
        let density = Density {
            factor:                read_f32_or(&den_sec, "factor",                "density", d_den.factor),
            row_height_dense:      read_f32_or(&den_sec, "row_height_dense",      "density", d_den.row_height_dense),
            row_height_comfortable: read_f32_or(&den_sec, "row_height_comfortable", "density", d_den.row_height_comfortable),
        };

        let d_sh = Shadows::default();
        let sh_sec = section("shadows");
        let shadows = Shadows {
            card:     parse_shadow_spec(sh_sec.get("card"),     "shadows.card",     &d_sh.card),
            modal:    parse_shadow_spec(sh_sec.get("modal"),    "shadows.modal",    &d_sh.modal),
            tooltip:  parse_shadow_spec(sh_sec.get("tooltip"),  "shadows.tooltip",  &d_sh.tooltip),
            dropdown: parse_shadow_spec(sh_sec.get("dropdown"), "shadows.dropdown", &d_sh.dropdown),
        };

        let d_tr = Treatments::default();
        let tr_sec = section("treatments");
        let treatments = Treatments {
            solid_active_fills:       read_bool_or(&tr_sec, "solid_active_fills",       "treatments", d_tr.solid_active_fills),
            hairline_borders:         read_bool_or(&tr_sec, "hairline_borders",         "treatments", d_tr.hairline_borders),
            uppercase_section_labels: read_bool_or(&tr_sec, "uppercase_section_labels", "treatments", d_tr.uppercase_section_labels),
            segmented_filled_idle:    read_bool_or(&tr_sec, "segmented_filled_idle",    "treatments", d_tr.segmented_filled_idle),
            focus_ring: tr_sec
                .get("focus_ring")
                .and_then(|n| dtcg_value(n, "treatments.focus_ring").ok())
                .and_then(|v| v.as_str())
                .map(|s| match s {
                    "none"    => FocusRingStyle::None,
                    "glow"    => FocusRingStyle::Glow,
                    _         => FocusRingStyle::Outline,
                })
                .unwrap_or(d_tr.focus_ring),
        };

        Ok(StyleSystem { meta, typography, spacing, radii, strokes, alphas, elevation, density, shadows, treatments })
    }
}

fn parse_shadow_spec(node: Option<&Value>, ctx: &str, default: &ShadowSpec) -> ShadowSpec {
    match node {
        None => default.clone(),
        Some(n) => ShadowSpec {
            blur:     read_f32_or(n, "blur",     ctx, default.blur),
            spread:   read_f32_or(n, "spread",   ctx, default.spread),
            offset_x: read_f32_or(n, "offset_x", ctx, default.offset_x),
            offset_y: read_f32_or(n, "offset_y", ctx, default.offset_y),
            alpha:    read_f32_or(n, "alpha",    ctx, default.alpha),
        },
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::design_system::color_scheme::builtin_dark;

    // ── ColorScheme round-trip ───────────────────────────────────────────────

    #[test]
    fn color_scheme_dtcg_round_trip() {
        let original = builtin_dark();
        let dtcg_json = original.to_dtcg();

        let parsed = ColorScheme::from_dtcg(&dtcg_json).expect("parse failed");

        assert_eq!(parsed.meta.id,   original.meta.id);
        assert_eq!(parsed.meta.name, original.meta.name);
        assert_eq!(parsed.meta.is_dark, original.meta.is_dark);
        assert_eq!(parsed.bg,     original.bg);
        assert_eq!(parsed.accent, original.accent);
        assert_eq!(parsed.bull,   original.bull);
        assert_eq!(parsed.bear,   original.bear);
    }

    #[test]
    fn color_scheme_parses_minimal_dtcg() {
        let json = r##"
        {
            "meta": { "id": "dracula", "name": "Dracula", "is_dark": true },
            "palette": {
                "bg":     { "$type": "color", "$value": "#282a36" },
                "accent": { "$type": "color", "$value": "#bd93f9" },
                "bull":   { "$type": "color", "$value": "#50fa7b" },
                "bear":   { "$type": "color", "$value": "#ff5555" }
            }
        }"##;
        let cs = ColorScheme::from_dtcg(json).expect("parse failed");
        assert_eq!(cs.meta.id, "dracula");
        assert_eq!(cs.bg,     [0x28, 0x2a, 0x36, 0xff]);
        assert_eq!(cs.accent, [0xbd, 0x93, 0xf9, 0xff]);
        assert_eq!(cs.bull,   [0x50, 0xfa, 0x7b, 0xff]);
        assert_eq!(cs.bear,   [0xff, 0x55, 0x55, 0xff]);
    }

    #[test]
    fn color_scheme_rejects_bad_hex() {
        let json = r##"
        {
            "meta": { "id": "bad", "name": "Bad", "is_dark": true },
            "palette": {
                "bg": { "$type": "color", "$value": "#xyz" }
            }
        }"##;
        // bg is bad → falls back to builtin_dark().bg, not an error (graceful degradation)
        let cs = ColorScheme::from_dtcg(json).expect("should succeed with fallback");
        let fallback = builtin_dark();
        assert_eq!(cs.bg, fallback.bg);
    }

    // ── StyleSystem DTCG parse ───────────────────────────────────────────────

    #[test]
    fn style_system_parses_minimal_dtcg() {
        let json = r#"
        {
            "meta": { "id": "meridien", "name": "Meridien", "is_dark": true },
            "typography": { "size_sm": { "$type": "dimension", "$value": 11 } },
            "spacing":    { "gmd":     { "$type": "dimension", "$value": 12 } },
            "radii":      { "sm":      { "$type": "dimension", "$value": 0 } },
            "treatments": { "solid_active_fills": { "$type": "boolean", "$value": true } }
        }"#;
        let ss = StyleSystem::from_dtcg(json).expect("parse failed");
        assert_eq!(ss.meta.id, "meridien");
        assert_eq!(ss.typography.size_sm, 11.0);
        assert_eq!(ss.spacing.gmd, 12.0);
        assert_eq!(ss.radii.sm, 0.0);
        assert!(ss.treatments.solid_active_fills);
    }

    #[test]
    fn style_system_missing_meta_returns_error() {
        let json = r#"{ "typography": {} }"#;
        let result = StyleSystem::from_dtcg(json);
        assert!(result.is_err(), "expected error for missing meta");
    }
}
