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
    color_scheme::{rgba, ColorScheme, Meta, Rgba, CMD_PALETTE_DEFAULT},
    style_system::{
        Alphas, BevelStyle, Chrome, Density, Elevation, FocusRingStyle, GroupEnclosure,
        PaneActiveIndicator, Radii, Shadows, ShadowSpec, Spacing, Strokes, StyleSystem, Treatments,
        Typography,
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

/// Read a `u8` from a DTCG integer/number token.
fn read_u8(node: &Value, path: &str) -> Result<u8, LoadError> {
    let v = dtcg_value(node, path)?;
    v.as_u64()
        .map(|n| n.min(255) as u8)
        .or_else(|| v.as_f64().map(|n| n.clamp(0.0, 255.0) as u8))
        .ok_or_else(|| LoadError::InvalidToken {
            path: path.to_string(),
            reason: format!("expected integer 0-255, got {v}"),
        })
}

fn read_u8_or(obj: &Value, key: &str, ctx: &str, default: u8) -> u8 {
    let path = format!("{ctx}.{key}");
    obj.get(key)
        .and_then(|n| read_u8(n, &path).ok())
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

/// Read a `String` from a DTCG string token (`$value` is a JSON string).
/// Falls back to `default` if the key is absent or the value is not a string.
fn read_string_or(obj: &Value, key: &str, _ctx: &str, default: &str) -> String {
    obj.get(key)
        .and_then(|n| n.get("$value"))
        .and_then(|v| v.as_str())
        .map(str::to_owned)
        .unwrap_or_else(|| default.to_owned())
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

/// Read an optional `Rgba` from a DTCG color token — returns `None` if the
/// key is absent or malformed (not an error; these fields are truly optional).
fn read_color_opt(obj: &Value, key: &str, ctx: &str) -> Option<Rgba> {
    let path = format!("{ctx}.{key}");
    obj.get(key).and_then(|n| read_color(n, &path).ok())
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
        let text    = read_color_or(&pal, "text",     "palette", fallback.text);
        let dim     = read_color_or(&pal, "dim",      "palette", fallback.dim);
        let border  = read_color_or(&pal, "border",   "palette", fallback.border);
        let accent  = read_color_or(&pal, "accent",   "palette", fallback.accent);
        let bull    = read_color_or(&pal, "bull",     "palette", fallback.bull);
        let bear    = read_color_or(&pal, "bear",     "palette", fallback.bear);
        let warn    = read_color_or(&pal, "warn",     "palette", fallback.warn);
        let shadow  = read_color_or(&pal, "shadow",   "palette", fallback.shadow);

        // ── Hand-authored extras — fall back to builtin_dark() defaults ───────
        // Sparse / partial DTCG files (e.g. design-tool exports) may omit these
        // fields; the fallback keeps the struct valid even for minimal overrides.
        let notification_red = read_color_or(&pal, "notification_red", "palette", fallback.notification_red);
        let gold             = read_color_or(&pal, "gold",             "palette", fallback.gold);
        let overlay_text     = read_color_or(&pal, "overlay_text",     "palette", fallback.overlay_text);
        let rrg_leading      = read_color_or(&pal, "rrg_leading",      "palette", fallback.rrg_leading);
        let rrg_improving    = read_color_or(&pal, "rrg_improving",    "palette", fallback.rrg_improving);
        let rrg_weakening    = read_color_or(&pal, "rrg_weakening",    "palette", fallback.rrg_weakening);
        let rrg_lagging      = read_color_or(&pal, "rrg_lagging",      "palette", fallback.rrg_lagging);
        let pinned_row_tint  = read_color_or(&pal, "pinned_row_tint",  "palette", fallback.pinned_row_tint);
        let text_muted       = read_color_or(&pal, "text_muted",       "palette", fallback.text_muted);
        let hud_bg           = read_color_or(&pal, "hud_bg",           "palette", fallback.hud_bg);
        let hud_border       = read_color_or(&pal, "hud_border",       "palette", fallback.hud_border);

        // ── Extended semantic palette (PALETTE-DEPTH) — all optional ─────────
        // None means "fall back to bull/bear/warn at render time" (zero visual
        // change for themes that don't set them explicitly).
        let success      = read_color_opt(&pal, "success",       "palette");
        let danger       = read_color_opt(&pal, "danger",        "palette");
        let warning      = read_color_opt(&pal, "warning",       "palette");
        let info         = read_color_opt(&pal, "info",          "palette");
        let pane_gap_color = read_color_opt(&pal, "pane_gap_color", "palette");

        // M1 Change A/C: authored ramp + bevel tints — all optional.
        let bg_panel        = read_color_opt(&pal, "bg_panel",        "palette");
        let bg_elevated     = read_color_opt(&pal, "bg_elevated",     "palette");
        let bg_hover        = read_color_opt(&pal, "bg_hover",        "palette");
        let fg_xmuted       = read_color_opt(&pal, "fg_xmuted",       "palette");
        let accent_sub      = read_color_opt(&pal, "accent_sub",      "palette");
        let bull_alpha      = read_color_opt(&pal, "bull_alpha",      "palette");
        let bear_alpha      = read_color_opt(&pal, "bear_alpha",      "palette");
        let border_dim      = read_color_opt(&pal, "border_dim",      "palette");
        let bevel_highlight = read_color_opt(&pal, "bevel_highlight", "palette");
        let bevel_shadow    = read_color_opt(&pal, "bevel_shadow",    "palette");

        // cmd_palette — optional 11-element array of color tokens.
        // Missing or malformed entries in the array fall back to CMD_PALETTE_DEFAULT slots.
        let cmd_palette = if let Some(arr) = pal.get("cmd_palette").and_then(|v| v.as_array()) {
            let mut out = CMD_PALETTE_DEFAULT;
            for (i, tok) in arr.iter().enumerate().take(11) {
                if let Ok(c) = read_color(tok, &format!("palette.cmd_palette[{i}]")) {
                    out[i] = c;
                }
            }
            out
        } else {
            CMD_PALETTE_DEFAULT
        };

        Ok(ColorScheme {
            meta, bg, surface, text, dim, border, accent, bull, bear, warn, shadow,
            notification_red, gold, overlay_text,
            rrg_leading, rrg_improving, rrg_weakening, rrg_lagging,
            pinned_row_tint, text_muted, hud_bg, hud_border,
            // Extended semantic palette — None if absent in DTCG file.
            success, danger, warning, info, pane_gap_color,
            bg_panel, bg_elevated, bg_hover, fg_xmuted, accent_sub,
            bull_alpha, bear_alpha, border_dim, bevel_highlight, bevel_shadow,
            cmd_palette,
        })
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
        pal.insert("text".into(),    color_token!(text));
        pal.insert("dim".into(),     color_token!(dim));
        pal.insert("border".into(),  color_token!(border));
        pal.insert("accent".into(),  color_token!(accent));
        pal.insert("bull".into(),    color_token!(bull));
        pal.insert("bear".into(),    color_token!(bear));
        pal.insert("warn".into(),    color_token!(warn));
        pal.insert("shadow".into(),  color_token!(shadow));
        // Hand-authored extras
        pal.insert("notification_red".into(), color_token!(notification_red));
        pal.insert("gold".into(),             color_token!(gold));
        pal.insert("overlay_text".into(),     color_token!(overlay_text));
        pal.insert("rrg_leading".into(),      color_token!(rrg_leading));
        pal.insert("rrg_improving".into(),    color_token!(rrg_improving));
        pal.insert("rrg_weakening".into(),    color_token!(rrg_weakening));
        pal.insert("rrg_lagging".into(),      color_token!(rrg_lagging));
        pal.insert("pinned_row_tint".into(),  color_token!(pinned_row_tint));
        pal.insert("text_muted".into(),       color_token!(text_muted));
        pal.insert("hud_bg".into(),           color_token!(hud_bg));
        pal.insert("hud_border".into(),       color_token!(hud_border));
        // Extended semantic palette — only emit when set (None = inherit from bull/bear/warn).
        macro_rules! opt_color_token {
            ($field:ident, $key:expr) => {
                if let Some(c) = self.$field {
                    let hex = rgba::to_hex(c);
                    pal.insert($key.into(), serde_json::json!({ "$type": "color", "$value": hex }));
                }
            };
        }
        opt_color_token!(success,       "success");
        opt_color_token!(danger,        "danger");
        opt_color_token!(warning,       "warning");
        opt_color_token!(info,          "info");
        opt_color_token!(pane_gap_color, "pane_gap_color");
        opt_color_token!(bg_panel,        "bg_panel");
        opt_color_token!(bg_elevated,     "bg_elevated");
        opt_color_token!(bg_hover,        "bg_hover");
        opt_color_token!(fg_xmuted,       "fg_xmuted");
        opt_color_token!(accent_sub,      "accent_sub");
        opt_color_token!(bull_alpha,      "bull_alpha");
        opt_color_token!(bear_alpha,      "bear_alpha");
        opt_color_token!(border_dim,      "border_dim");
        opt_color_token!(bevel_highlight, "bevel_highlight");
        opt_color_token!(bevel_shadow,    "bevel_shadow");

        // cmd_palette — emit all 11 slots as an array of color tokens.
        let cmd_arr: serde_json::Value = self.cmd_palette
            .iter()
            .map(|&c| serde_json::json!({ "$type": "color", "$value": rgba::to_hex(c) }))
            .collect::<Vec<_>>()
            .into();
        pal.insert("cmd_palette".into(), cmd_arr);

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
            size_section_label: read_f32_or(&typ_sec, "size_section_label", "typography", d_typ.size_section_label),
            label_tracking:   read_f32_or(&typ_sec, "label_tracking",   "typography", d_typ.label_tracking),
            nav_tracking:     read_f32_or(&typ_sec, "nav_tracking",     "typography", d_typ.nav_tracking),
            section_tracking: read_f32_or(&typ_sec, "section_tracking", "typography", d_typ.section_tracking),
            // Font family identifiers (S7 blocker) — gracefully absent in legacy JSON.
            family_ui:      read_string_or(&typ_sec, "family_ui",      "typography", &d_typ.family_ui),
            family_mono:    read_string_or(&typ_sec, "family_mono",    "typography", &d_typ.family_mono),
            family_display: read_string_or(&typ_sec, "family_display", "typography", &d_typ.family_display),
            ..Typography::default()
        };

        let d_sp = Spacing::default();
        let sp_sec = section("spacing");
        let spacing = Spacing {
            xs:         read_f32_or(&sp_sec, "xs",         "spacing", d_sp.xs),
            sm:         read_f32_or(&sp_sec, "sm",         "spacing", d_sp.sm),
            xs_mid:     read_f32_or(&sp_sec, "xs_mid",     "spacing", d_sp.xs_mid),
            md:         read_f32_or(&sp_sec, "md",         "spacing", d_sp.md),
            lg:         read_f32_or(&sp_sec, "lg",         "spacing", d_sp.lg),
            xl:         read_f32_or(&sp_sec, "xl",         "spacing", d_sp.xl),
            xxl:        read_f32_or(&sp_sec, "xxl",        "spacing", d_sp.xxl),
            gmd:        read_f32_or(&sp_sec, "gmd",        "spacing", d_sp.gmd),
            cta_height: read_f32_or(&sp_sec, "cta_height", "spacing", d_sp.cta_height),
            cta_padding_x:    read_f32_or(&sp_sec, "cta_padding_x",    "spacing", d_sp.cta_padding_x),
            button_height:    read_f32_or(&sp_sec, "button_height",    "spacing", d_sp.button_height),
            button_padding_x: read_f32_or(&sp_sec, "button_padding_x", "spacing", d_sp.button_padding_x),
            tab_height:       read_f32_or(&sp_sec, "tab_height",       "spacing", d_sp.tab_height),
            ..Spacing::default()
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
            pill: read_f32_or(&r_sec, "pill", "radii", d_r.pill),
            chip: read_f32_or(&r_sec, "chip", "radii", d_r.chip),
        };

        let d_st = Strokes::default();
        let st_sec = section("strokes");
        let strokes = Strokes {
            hair:   read_f32_or(&st_sec, "hair",   "strokes", d_st.hair),
            thin:   read_f32_or(&st_sec, "thin",   "strokes", d_st.thin),
            medium: read_f32_or(&st_sec, "medium", "strokes", d_st.medium),
            std:    read_f32_or(&st_sec, "std",    "strokes", d_st.std),
            bold:   read_f32_or(&st_sec, "bold",   "strokes", d_st.bold),
            thick:  read_f32_or(&st_sec, "thick",  "strokes", d_st.thick),
            md:     read_f32_or(&st_sec, "md",     "strokes", d_st.md),
            heavy:  read_f32_or(&st_sec, "heavy",  "strokes", d_st.heavy),
        };

        let d_al = Alphas::default();
        let al_sec = section("alphas");
        let alphas = Alphas {
            // u8 tiers
            faint:     read_u8_or(&al_sec, "faint",     "alphas", d_al.faint),
            ghost:     read_u8_or(&al_sec, "ghost",     "alphas", d_al.ghost),
            soft_u8:   read_u8_or(&al_sec, "soft_u8",   "alphas", d_al.soft_u8),
            subtle_u8: read_u8_or(&al_sec, "subtle_u8", "alphas", d_al.subtle_u8),
            tint:      read_u8_or(&al_sec, "tint",      "alphas", d_al.tint),
            muted_u8:  read_u8_or(&al_sec, "muted_u8",  "alphas", d_al.muted_u8),
            dim:       read_u8_or(&al_sec, "dim",       "alphas", d_al.dim),
            line:      read_u8_or(&al_sec, "line",      "alphas", d_al.line),
            strong_u8: read_u8_or(&al_sec, "strong_u8", "alphas", d_al.strong_u8),
            active:    read_u8_or(&al_sec, "active",    "alphas", d_al.active),
            heavy_u8:  read_u8_or(&al_sec, "heavy_u8",  "alphas", d_al.heavy_u8),
            scrim:     read_u8_or(&al_sec, "scrim",     "alphas", d_al.scrim),
            solid:     read_u8_or(&al_sec, "solid",     "alphas", d_al.solid),
            // f32 multipliers
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
                    "none" => FocusRingStyle::None,
                    "glow" => FocusRingStyle::Glow,
                    _      => FocusRingStyle::Outline,
                })
                .unwrap_or(d_tr.focus_ring),
            surface_bevel: tr_sec
                .get("surface_bevel")
                .and_then(|n| dtcg_value(n, "treatments.surface_bevel").ok())
                .and_then(|v| v.as_str())
                .map(|s| match s {
                    "raised" => BevelStyle::Raised,
                    "inset"  => BevelStyle::Inset,
                    _        => BevelStyle::None,
                })
                .unwrap_or(d_tr.surface_bevel),
            bevel_highlight_alpha: read_u8_or(&tr_sec, "bevel_highlight_alpha", "treatments", d_tr.bevel_highlight_alpha),
            bevel_shadow_alpha:    read_u8_or(&tr_sec, "bevel_shadow_alpha",    "treatments", d_tr.bevel_shadow_alpha),
            wl_row_side_margin:    read_f32_or(&tr_sec, "wl_row_side_margin",   "treatments", d_tr.wl_row_side_margin),
            wl_row_corner_radius:  read_u8_or(&tr_sec, "wl_row_corner_radius",  "treatments", d_tr.wl_row_corner_radius),
            wl_row_divider_alpha:  read_u8_or(&tr_sec, "wl_row_divider_alpha",  "treatments", d_tr.wl_row_divider_alpha),
            section_header_mono:   read_bool_or(&tr_sec, "section_header_mono", "treatments", d_tr.section_header_mono),
            wl_symbol_mono:        read_bool_or(&tr_sec, "wl_symbol_mono",      "treatments", d_tr.wl_symbol_mono),
            panel_tab_treatment:   read_u8_or(&tr_sec, "panel_tab_treatment",   "treatments", d_tr.panel_tab_treatment),
            pane_active_fill_accent: read_bool_or(&tr_sec, "pane_active_fill_accent", "treatments", d_tr.pane_active_fill_accent),
            serif_headlines:       read_bool_or(&tr_sec, "serif_headlines",       "treatments", d_tr.serif_headlines),
            button_treatment:      read_u8_or(&tr_sec, "button_treatment",        "treatments", d_tr.button_treatment),
            invert_active_fill:    read_bool_or(&tr_sec, "invert_active_fill",    "treatments", d_tr.invert_active_fill),
            vertical_group_dividers: read_bool_or(&tr_sec, "vertical_group_dividers", "treatments", d_tr.vertical_group_dividers),
            show_active_tab_underline: read_bool_or(&tr_sec, "show_active_tab_underline", "treatments", d_tr.show_active_tab_underline),
            inactive_header_fill:  read_bool_or(&tr_sec, "inactive_header_fill",  "treatments", d_tr.inactive_header_fill),
            nav_buttons_label_only: read_bool_or(&tr_sec, "nav_buttons_label_only", "treatments", d_tr.nav_buttons_label_only),
            nav_buttons_uppercase_labels: read_bool_or(&tr_sec, "nav_buttons_uppercase_labels", "treatments", d_tr.nav_buttons_uppercase_labels),
            tab_underline_under_text: read_bool_or(&tr_sec, "tab_underline_under_text", "treatments", d_tr.tab_underline_under_text),
            card_floating_shadow:  read_bool_or(&tr_sec, "card_floating_shadow",  "treatments", d_tr.card_floating_shadow),
            shadows_enabled:       read_bool_or(&tr_sec, "shadows_enabled",       "treatments", d_tr.shadows_enabled),
            animations_enabled:    read_bool_or(&tr_sec, "animations_enabled",    "treatments", d_tr.animations_enabled),
        };

        // Chrome — fully round-tripped now.
        let d_ch = Chrome::default();
        let ch_sec = section("chrome");
        let chrome = Chrome {
            toolbar_height_scale:          read_f32_or(&ch_sec, "toolbar_height_scale",          "chrome", d_ch.toolbar_height_scale),
            header_height_scale:           read_f32_or(&ch_sec, "header_height_scale",           "chrome", d_ch.header_height_scale),
            account_strip_height:          read_f32_or(&ch_sec, "account_strip_height",          "chrome", d_ch.account_strip_height),
            pane_border_width:             read_f32_or(&ch_sec, "pane_border_width",             "chrome", d_ch.pane_border_width),
            pane_gap:                      read_f32_or(&ch_sec, "pane_gap",                      "chrome", d_ch.pane_gap),
            pane_gap_alpha:                read_u8_or(&ch_sec,  "pane_gap_alpha",                "chrome", d_ch.pane_gap_alpha),
            pane_active_indicator: {
                let indicator = ch_sec
                    .get("pane_active_indicator")
                    .and_then(|n| dtcg_value(n, "chrome.pane_active_indicator").ok())
                    .and_then(|v| v.as_str())
                    .map(|s| match s {
                        "none"        => PaneActiveIndicator::None,
                        "top_stripe"  => PaneActiveIndicator::TopStripe,
                        "both"        => PaneActiveIndicator::Both,
                        _             => PaneActiveIndicator::HeaderFill,
                    })
                    .unwrap_or(PaneActiveIndicator::from_u8(d_ch.pane_active_indicator));
                indicator.as_u8()
            },
            active_header_fill_multiply:   read_f32_or(&ch_sec, "active_header_fill_multiply",   "chrome", d_ch.active_header_fill_multiply),
            inactive_header_fill_multiply: read_f32_or(&ch_sec, "inactive_header_fill_multiply", "chrome", d_ch.inactive_header_fill_multiply),
            header_outer_border_alpha:     read_u8_or(&ch_sec,  "header_outer_border_alpha",     "chrome", d_ch.header_outer_border_alpha),
            header_outer_border_width:     read_f32_or(&ch_sec, "header_outer_border_width",     "chrome", d_ch.header_outer_border_width),
            header_divider_alpha:          read_u8_or(&ch_sec,  "header_divider_alpha",          "chrome", d_ch.header_divider_alpha),
            nav_active_col_alpha:          read_u8_or(&ch_sec,  "nav_active_col_alpha",          "chrome", d_ch.nav_active_col_alpha),
            dialog_backdrop_alpha:         read_u8_or(&ch_sec,  "dialog_backdrop_alpha",         "chrome", d_ch.dialog_backdrop_alpha),
            tab_inactive_alpha:            read_f32_or(&ch_sec, "tab_inactive_alpha",            "chrome", d_ch.tab_inactive_alpha),
            tab_hover_bg_alpha:            read_u8_or(&ch_sec,  "tab_hover_bg_alpha",            "chrome", d_ch.tab_hover_bg_alpha),
            tab_underline_thickness:       read_f32_or(&ch_sec, "tab_underline_thickness",       "chrome", d_ch.tab_underline_thickness),
            section_label_padding_top:     read_f32_or(&ch_sec, "section_label_padding_top",     "chrome", d_ch.section_label_padding_top),
            section_label_padding_bottom:  read_f32_or(&ch_sec, "section_label_padding_bottom",  "chrome", d_ch.section_label_padding_bottom),
            drag_handle_alpha:             read_f32_or(&ch_sec, "drag_handle_alpha",             "chrome", d_ch.drag_handle_alpha),
            drag_handle_dot_scale:         read_f32_or(&ch_sec, "drag_handle_dot_scale",         "chrome", d_ch.drag_handle_dot_scale),
            toast_bg_alpha:                read_u8_or(&ch_sec,  "toast_bg_alpha",                "chrome", d_ch.toast_bg_alpha),
            card_stripe_alpha:             read_u8_or(&ch_sec,  "card_stripe_alpha",             "chrome", d_ch.card_stripe_alpha),
            card_floating_shadow_alpha:    read_u8_or(&ch_sec,  "card_floating_shadow_alpha",    "chrome", d_ch.card_floating_shadow_alpha),
            accent_emphasis:               read_f32_or(&ch_sec, "accent_emphasis",               "chrome", d_ch.accent_emphasis),
            disabled_opacity:              read_f32_or(&ch_sec, "disabled_opacity",              "chrome", d_ch.disabled_opacity),
            focus_ring_width:              read_f32_or(&ch_sec, "focus_ring_width",              "chrome", d_ch.focus_ring_width),
            focus_ring_alpha:              read_u8_or(&ch_sec,  "focus_ring_alpha",              "chrome", d_ch.focus_ring_alpha),
            hover_bg_alpha:                read_u8_or(&ch_sec,  "hover_bg_alpha",                "chrome", d_ch.hover_bg_alpha),
            active_bg_alpha:               read_u8_or(&ch_sec,  "active_bg_alpha",               "chrome", d_ch.active_bg_alpha),
            region_gap:                    read_f32_or(&ch_sec, "region_gap",                    "chrome", d_ch.region_gap),
            region_radius:                 read_f32_or(&ch_sec, "region_radius",                 "chrome", d_ch.region_radius),
            region_border_alpha:           read_u8_or(&ch_sec,  "region_border_alpha",           "chrome", d_ch.region_border_alpha),
            nav_cluster_radius:            read_f32_or(&ch_sec, "nav_cluster_radius",            "chrome", d_ch.nav_cluster_radius),
            nav_cluster_fill_alpha:        read_u8_or(&ch_sec,  "nav_cluster_fill_alpha",        "chrome", d_ch.nav_cluster_fill_alpha),
            nav_cluster_padding:           read_f32_or(&ch_sec, "nav_cluster_padding",           "chrome", d_ch.nav_cluster_padding),
            button_group: ch_sec
                .get("button_group")
                .and_then(|n| dtcg_value(n, "chrome.button_group").ok())
                .and_then(|v| v.as_str())
                .map(|s| match s {
                    "bordered" => GroupEnclosure::Bordered,
                    "frosted"  => GroupEnclosure::Frosted,
                    "sharp"    => GroupEnclosure::Sharp,
                    _          => GroupEnclosure::None,
                })
                .unwrap_or(d_ch.button_group),
            toolnav_height:                read_f32_or(&ch_sec, "toolnav_height",                "chrome", d_ch.toolnav_height),
            footer_default_open:           read_bool_or(&ch_sec, "footer_default_open",          "chrome", d_ch.footer_default_open),
            panel_header_treatment:        read_u8_or(&ch_sec,  "panel_header_treatment",        "chrome", d_ch.panel_header_treatment),
            panel_section_fill_alpha:      read_u8_or(&ch_sec,  "panel_section_fill_alpha",      "chrome", d_ch.panel_section_fill_alpha),
            panel_footer_card:             read_bool_or(&ch_sec, "panel_footer_card",            "chrome", d_ch.panel_footer_card),
            panel_footer_radius:           read_f32_or(&ch_sec, "panel_footer_radius",           "chrome", d_ch.panel_footer_radius),
        };

        Ok(StyleSystem { meta, typography, spacing, radii, strokes, alphas, elevation, density, shadows, treatments, chrome })
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

    // ── Full round-trip: StyleSystem with distinctive values in every section ──

    #[test]
    fn style_system_full_round_trip_with_custom_chrome_and_treatments() {
        use crate::design_system::{
            color_scheme::Meta,
            style_system::{
                Alphas, BevelStyle, Chrome, Density, Elevation, FocusRingStyle, GroupEnclosure,
                PaneActiveIndicator, Radii, Shadows, ShadowSpec, Spacing, Strokes, StyleSystem,
                Treatments, Typography,
            },
        };

        let original = StyleSystem {
            meta: Meta::new("test-full", "Test Full", false),
            typography: Typography {
                size_xs: 7.5, size_sm: 10.0, size_md: 12.5, size_lg: 14.0, size_xl: 20.0,
                mono_sm: 9.5, mono_md: 11.5, mono_lg: 14.5,
                size_section_label: 8.0,
                label_tracking: 0.5, nav_tracking: 1.0, section_tracking: 1.5,
                family_ui: "Roboto".into(),
                family_mono: "Fira Code".into(),
                family_display: "Playfair Display".into(),
                ..Typography::default()
            },
            spacing: Spacing {
                xs: 3.0, sm: 6.0, xs_mid: 5.0, md: 10.0, lg: 14.0, xl: 18.0, xxl: 22.0,
                gmd: 7.0, cta_height: 30.0, cta_padding_x: 14.0,
                button_height: 26.0, button_padding_x: 11.0, tab_height: 30.0,
                ..Spacing::default()
            },
            radii: Radii { none: 0.0, xs: 3.0, sm: 5.0, md: 8.0, lg: 14.0, full: 9999.0, pill: 50.0, chip: 3.0 },
            strokes: Strokes { hair: 0.25, thin: 0.4, medium: 0.7, std: 0.9, bold: 1.3, thick: 1.8, md: 1.3, heavy: 1.8 },
            alphas: Alphas {
                faint: 8, ghost: 12, soft_u8: 18, subtle_u8: 35, tint: 45, muted_u8: 55,
                dim: 55, line: 75, strong_u8: 75, active: 95, heavy_u8: 115, scrim: 130, solid: 190,
                subtle: 0.03, soft: 0.10, muted: 0.22, mid: 0.45, strong: 0.70, opaque: 0.99, header_border: 0.15,
            },
            elevation: Elevation { l1: 1.08, l2: 0.92, l3: 0.85 },
            density: Density { factor: 0.9, row_height_dense: 20.0, row_height_comfortable: 30.0 },
            shadows: Shadows {
                card:     ShadowSpec { blur: 6.0,  spread: 1.0, offset_x: 1.0, offset_y: 3.0, alpha: 0.25 },
                modal:    ShadowSpec { blur: 20.0, spread: 2.0, offset_x: 0.0, offset_y: 6.0, alpha: 0.45 },
                tooltip:  ShadowSpec { blur: 5.0,  spread: 0.0, offset_x: 0.0, offset_y: 1.5, alpha: 0.35 },
                dropdown: ShadowSpec { blur: 10.0, spread: 0.0, offset_x: 0.0, offset_y: 3.0, alpha: 0.38 },
            },
            treatments: Treatments {
                solid_active_fills: true,
                hairline_borders: true,
                uppercase_section_labels: true,
                segmented_filled_idle: true,
                focus_ring: FocusRingStyle::Glow,
                surface_bevel: BevelStyle::Raised,
                bevel_highlight_alpha: 42,
                bevel_shadow_alpha: 38,
                wl_row_side_margin: 6.0,
                wl_row_corner_radius: 8,
                wl_row_divider_alpha: 25,
                section_header_mono: true,
                wl_symbol_mono: true,
                panel_tab_treatment: 2,
                pane_active_fill_accent: true,
                serif_headlines: true,
                button_treatment: 3,
                invert_active_fill: true,
                vertical_group_dividers: true,
                show_active_tab_underline: false,
                inactive_header_fill: false,
                nav_buttons_label_only: true,
                nav_buttons_uppercase_labels: true,
                tab_underline_under_text: true,
                card_floating_shadow: true,
                shadows_enabled: false,
                animations_enabled: false,
            },
            chrome: Chrome {
                toolbar_height_scale: 1.3,
                header_height_scale: 1.2,
                account_strip_height: 30.0,
                pane_border_width: 2.0,
                pane_gap: 8.0,
                pane_gap_alpha: 80,
                pane_active_indicator: PaneActiveIndicator::Both.as_u8(),
                active_header_fill_multiply: 0.6,
                inactive_header_fill_multiply: 1.12,
                header_outer_border_alpha: 50,
                header_outer_border_width: 1.0,
                header_divider_alpha: 60,
                nav_active_col_alpha: 30,
                dialog_backdrop_alpha: 120,
                tab_inactive_alpha: 0.45,
                tab_hover_bg_alpha: 25,
                tab_underline_thickness: 3.0,
                section_label_padding_top: 6.0,
                section_label_padding_bottom: 3.0,
                drag_handle_alpha: 0.5,
                drag_handle_dot_scale: 1.2,
                toast_bg_alpha: 200,
                card_stripe_alpha: 200,
                card_floating_shadow_alpha: 40,
                accent_emphasis: 1.2,
                disabled_opacity: 0.4,
                focus_ring_width: 2.0,
                focus_ring_alpha: 140,
                hover_bg_alpha: 22,
                active_bg_alpha: 38,
                region_gap: 8.0,
                region_radius: 10.0,
                region_border_alpha: 55,
                nav_cluster_radius: 6.0,
                nav_cluster_fill_alpha: 30,
                nav_cluster_padding: 8.0,
                button_group: GroupEnclosure::Bordered,
                toolnav_height: 30.0,
                footer_default_open: true,
                panel_header_treatment: 2,
                panel_section_fill_alpha: 18,
                panel_footer_card: true,
                panel_footer_radius: 8.0,
            },
        };

        let json = original.to_dtcg();
        assert!(!json.is_empty(), "to_dtcg produced empty string");

        let parsed = StyleSystem::from_dtcg(&json)
            .expect("full round-trip parse failed");

        // Meta
        assert_eq!(parsed.meta.id, original.meta.id);
        assert_eq!(parsed.meta.is_dark, original.meta.is_dark);

        // Typography
        assert_eq!(parsed.typography.size_sm, original.typography.size_sm);
        assert_eq!(parsed.typography.family_ui, original.typography.family_ui);
        assert_eq!(parsed.typography.family_mono, original.typography.family_mono);
        assert_eq!(parsed.typography.family_display, original.typography.family_display);
        assert_eq!(parsed.typography.label_tracking, original.typography.label_tracking);

        // Spacing (including new fields)
        assert_eq!(parsed.spacing.cta_padding_x, original.spacing.cta_padding_x);
        assert_eq!(parsed.spacing.button_height, original.spacing.button_height);
        assert_eq!(parsed.spacing.button_padding_x, original.spacing.button_padding_x);
        assert_eq!(parsed.spacing.tab_height, original.spacing.tab_height);

        // Radii (including new fields)
        assert_eq!(parsed.radii.pill, original.radii.pill);
        assert_eq!(parsed.radii.chip, original.radii.chip);
        assert_eq!(parsed.radii.sm, original.radii.sm);

        // Alphas (including scrim)
        assert_eq!(parsed.alphas.scrim, original.alphas.scrim);
        assert_eq!(parsed.alphas.faint, original.alphas.faint);
        assert_eq!(parsed.alphas.header_border, original.alphas.header_border);

        // Treatments — previously defaulted fields
        assert_eq!(parsed.treatments.focus_ring, FocusRingStyle::Glow);
        assert_eq!(parsed.treatments.surface_bevel, BevelStyle::Raised);
        assert_eq!(parsed.treatments.bevel_highlight_alpha, 42);
        assert_eq!(parsed.treatments.bevel_shadow_alpha, 38);
        assert_eq!(parsed.treatments.wl_row_side_margin, 6.0);
        assert_eq!(parsed.treatments.wl_row_corner_radius, 8);
        assert_eq!(parsed.treatments.wl_row_divider_alpha, 25);
        assert_eq!(parsed.treatments.section_header_mono, true);
        assert_eq!(parsed.treatments.wl_symbol_mono, true);
        assert_eq!(parsed.treatments.panel_tab_treatment, 2);
        assert_eq!(parsed.treatments.pane_active_fill_accent, true);
        assert_eq!(parsed.treatments.serif_headlines, true);
        assert_eq!(parsed.treatments.button_treatment, 3);
        assert_eq!(parsed.treatments.invert_active_fill, true);
        assert_eq!(parsed.treatments.shadows_enabled, false);
        assert_eq!(parsed.treatments.animations_enabled, false);
        assert_eq!(parsed.treatments.show_active_tab_underline, false);
        assert_eq!(parsed.treatments.inactive_header_fill, false);

        // Chrome — complete
        assert_eq!(parsed.chrome.toolbar_height_scale, original.chrome.toolbar_height_scale);
        assert_eq!(parsed.chrome.pane_gap, original.chrome.pane_gap);
        assert_eq!(parsed.chrome.pane_gap_alpha, original.chrome.pane_gap_alpha);
        assert_eq!(parsed.chrome.pane_active_indicator, PaneActiveIndicator::Both.as_u8(),
            "pane_active_indicator round-trip failed (expected Both=3)");
        assert_eq!(parsed.chrome.active_header_fill_multiply, original.chrome.active_header_fill_multiply);
        assert_eq!(parsed.chrome.header_outer_border_alpha, original.chrome.header_outer_border_alpha);
        assert_eq!(parsed.chrome.dialog_backdrop_alpha, original.chrome.dialog_backdrop_alpha);
        assert_eq!(parsed.chrome.tab_inactive_alpha, original.chrome.tab_inactive_alpha);
        assert_eq!(parsed.chrome.drag_handle_alpha, original.chrome.drag_handle_alpha);
        assert_eq!(parsed.chrome.toast_bg_alpha, original.chrome.toast_bg_alpha);
        assert_eq!(parsed.chrome.accent_emphasis, original.chrome.accent_emphasis);
        assert_eq!(parsed.chrome.disabled_opacity, original.chrome.disabled_opacity);
        assert_eq!(parsed.chrome.focus_ring_width, original.chrome.focus_ring_width);
        assert_eq!(parsed.chrome.focus_ring_alpha, original.chrome.focus_ring_alpha);
        assert_eq!(parsed.chrome.hover_bg_alpha, original.chrome.hover_bg_alpha);
        assert_eq!(parsed.chrome.active_bg_alpha, original.chrome.active_bg_alpha);
        assert_eq!(parsed.chrome.region_gap, original.chrome.region_gap);
        assert_eq!(parsed.chrome.region_radius, original.chrome.region_radius);
        assert_eq!(parsed.chrome.region_border_alpha, original.chrome.region_border_alpha);
        assert_eq!(parsed.chrome.nav_cluster_radius, original.chrome.nav_cluster_radius);
        assert_eq!(parsed.chrome.nav_cluster_fill_alpha, original.chrome.nav_cluster_fill_alpha);
        assert_eq!(parsed.chrome.nav_cluster_padding, original.chrome.nav_cluster_padding);
        assert_eq!(parsed.chrome.button_group, GroupEnclosure::Bordered,
            "button_group round-trip failed (expected Bordered)");
        assert_eq!(parsed.chrome.toolnav_height, original.chrome.toolnav_height);
        assert_eq!(parsed.chrome.footer_default_open, original.chrome.footer_default_open);
        assert_eq!(parsed.chrome.panel_header_treatment, original.chrome.panel_header_treatment);
        assert_eq!(parsed.chrome.panel_section_fill_alpha, original.chrome.panel_section_fill_alpha);
        assert_eq!(parsed.chrome.panel_footer_card, original.chrome.panel_footer_card);
        assert_eq!(parsed.chrome.panel_footer_radius, original.chrome.panel_footer_radius);

        // Full structural equality
        assert_eq!(parsed, original, "full StyleSystem round-trip equality failed");
    }

    // ── ColorScheme cmd_palette round-trip ────────────────────────────────────

    #[test]
    fn color_scheme_cmd_palette_round_trip() {
        use crate::design_system::color_scheme::{rgba, builtin_dark, CMD_PALETTE_DEFAULT};

        // Custom palette — differs from CMD_PALETTE_DEFAULT in every slot.
        let custom_palette: [_; 11] = [
            rgba::rgb(255,   0,   0),
            rgba::rgb(  0, 255,   0),
            rgba::rgb(  0,   0, 255),
            rgba::rgb(255, 255,   0),
            rgba::rgb(  0, 255, 255),
            rgba::rgb(255,   0, 255),
            rgba::rgb(128, 128,   0),
            rgba::rgb(  0, 128, 128),
            rgba::rgb(128,   0, 128),
            rgba::rgb(200, 100,  50),
            rgba::rgb( 50, 100, 200),
        ];
        // Sanity: custom differs from default.
        assert_ne!(custom_palette, CMD_PALETTE_DEFAULT);

        let mut scheme = builtin_dark();
        scheme.cmd_palette = custom_palette;

        let json = scheme.to_dtcg();
        let parsed = ColorScheme::from_dtcg(&json).expect("cmd_palette round-trip parse failed");

        assert_eq!(parsed.cmd_palette, custom_palette,
            "cmd_palette was not preserved through DTCG round-trip");

        // Also verify the default is preserved when cmd_palette is absent from JSON.
        let json_no_palette = r##"{
            "meta": { "id": "test-no-cp", "name": "Test", "is_dark": true },
            "palette": {
                "bg":     { "$type": "color", "$value": "#121212ff" },
                "accent": { "$type": "color", "$value": "#6366f1ff" }
            }
        }"##;
        let parsed_no_cp = ColorScheme::from_dtcg(json_no_palette)
            .expect("parse without cmd_palette failed");
        assert_eq!(parsed_no_cp.cmd_palette, CMD_PALETTE_DEFAULT,
            "missing cmd_palette should fall back to CMD_PALETTE_DEFAULT");
    }
}
