//! Bidirectional translation between the [`FigmaThemeExport`] envelope and the
//! engine's DTCG JSON, plus the public emitter and importer.
//!
//! - [`theme_to_figma_export`] (engine → Figma) reshapes `ColorScheme::to_dtcg`
//!   / `StyleSystem::to_dtcg` output into Figma variables. It backs both the
//!   round-trip test fixture and Theme Studio's *Export to Figma* payload.
//! - [`figma_export_to_pack`] (Figma → engine) reassembles DTCG from the
//!   variables of a chosen mode and defers to `from_dtcg`, then builds a
//!   [`ThemePack`].

use std::collections::BTreeMap;

use serde_json::{json, Map, Value};

use crate::design_system::{ColorScheme, StyleSystem};
use crate::design_system::theme_pack::{manifest::ThemeManifest, ThemePack};
use crate::design_system::recipes::RecipeSet;
use crate::ui_kit::assets::AssetRegistry;
use crate::ui_kit::sx::recipe_spec::RecipeSpec;

use super::mapping::FigmaMapping;
use super::model::*;

// ── Error ─────────────────────────────────────────────────────────────────────

/// Failure modes of the Figma → engine transform.
#[derive(Debug)]
pub enum ImportError {
    /// A `from_dtcg` parse failed (malformed colour/value).
    Parse(String),
    /// A component recipe value was not a valid `RecipeSpec`.
    Recipe(String),
    /// The requested mode was not present in the export.
    MissingMode(String),
    /// Writing/reading the `.apextheme` bundle failed.
    Bundle(String),
}

impl std::fmt::Display for ImportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ImportError::Parse(m) => write!(f, "DTCG parse error: {m}"),
            ImportError::Recipe(m) => write!(f, "invalid component recipe: {m}"),
            ImportError::MissingMode(m) => write!(f, "mode not found in export: {m}"),
            ImportError::Bundle(m) => write!(f, "bundle error: {m}"),
        }
    }
}

impl std::error::Error for ImportError {}

/// Which Figma modes to import, and the pack identity to stamp.
#[derive(Clone, Debug)]
pub struct ImportOptions {
    /// Colour mode to read (default [`DEFAULT_MODE`], else the collection default).
    pub color_mode: Option<String>,
    /// Style mode to read (default [`DEFAULT_MODE`], else the collection default).
    pub style_mode: Option<String>,
}

impl Default for ImportOptions {
    fn default() -> Self {
        ImportOptions { color_mode: None, style_mode: None }
    }
}

// ── Emitter: engine → Figma export ─────────────────────────────────────────────

/// Build a [`FigmaThemeExport`] from one or more `ColorScheme` / `StyleSystem`
/// modes plus component recipes. The first entry in each list is the default mode.
///
/// `components` maps recipe key → `RecipeSpec`; it is serialised verbatim.
pub fn theme_to_figma_export(
    color_modes: &[(String, ColorScheme)],
    style_modes: &[(String, StyleSystem)],
    components: &BTreeMap<String, RecipeSpec>,
) -> FigmaThemeExport {
    let mut variables: Vec<FigmaVariable> = Vec::new();

    // Colour collection.
    let mut color_meta = BTreeMap::new();
    for (mode, cs) in color_modes {
        color_meta.insert(
            mode.clone(),
            ModeMeta { id: cs.meta.id.clone(), name: cs.meta.name.clone(), is_dark: cs.meta.is_dark },
        );
        let dtcg: Value = serde_json::from_str(&cs.to_dtcg()).expect("to_dtcg is valid JSON");
        flatten_palette(&dtcg, mode, &mut variables);
    }

    // Style collection.
    let mut style_meta = BTreeMap::new();
    for (mode, ss) in style_modes {
        style_meta.insert(
            mode.clone(),
            ModeMeta { id: ss.meta.id.clone(), name: ss.meta.name.clone(), is_dark: ss.meta.is_dark },
        );
        let dtcg: Value = serde_json::from_str(&ss.to_dtcg()).expect("to_dtcg is valid JSON");
        flatten_style(&dtcg, mode, &mut variables);
    }

    // Merge: one FigmaVariable per (collection, name), values keyed by mode.
    let merged = merge_modes(variables);

    let color_default = color_modes.first().map(|(m, _)| m.clone()).unwrap_or_else(|| DEFAULT_MODE.into());
    let style_default = style_modes.first().map(|(m, _)| m.clone()).unwrap_or_else(|| DEFAULT_MODE.into());

    let components_json = components
        .iter()
        .map(|(k, spec)| (k.clone(), serde_json::to_value(spec).expect("RecipeSpec serialises")))
        .collect();

    FigmaThemeExport {
        schema: 1,
        generator: "apex-figma-sync v1 (design_system::import)".into(),
        color_collection: CollectionInfo {
            name: COLOR_COLLECTION.into(),
            default_mode: color_default,
            modes: color_modes.iter().map(|(m, _)| m.clone()).collect(),
            meta: color_meta,
        },
        style_collection: CollectionInfo {
            name: STYLE_COLLECTION.into(),
            default_mode: style_default,
            modes: style_modes.iter().map(|(m, _)| m.clone()).collect(),
            meta: style_meta,
        },
        variables: merged,
        components: components_json,
    }
}

/// Flatten a colour DTCG document's `palette` into Figma `ColorScheme` variables
/// for one mode. `cmd_palette` becomes 11 named `cmd_palette/<slot>` colours.
fn flatten_palette(dtcg: &Value, mode: &str, out: &mut Vec<FigmaVariable>) {
    let Some(palette) = dtcg.get("palette").and_then(Value::as_object) else { return };
    for (key, tok) in palette {
        if key == "cmd_palette" {
            if let Some(arr) = tok.as_array() {
                for (i, slot) in CMD_PALETTE_SLOTS.iter().enumerate() {
                    if let Some(hex) = arr.get(i).and_then(|t| t.get("$value")).and_then(Value::as_str) {
                        push_color(out, COLOR_COLLECTION, &format!("cmd_palette/{slot}"), mode, hex);
                    }
                }
            }
            continue;
        }
        if let Some(hex) = tok.get("$value").and_then(Value::as_str) {
            push_color(out, COLOR_COLLECTION, key, mode, hex);
        }
    }
}

/// Flatten a style DTCG document's sections into Figma `StyleSystem` variables
/// for one mode. Nested groups (e.g. `shadows.card.blur`) become `/`-joined names.
fn flatten_style(dtcg: &Value, mode: &str, out: &mut Vec<FigmaVariable>) {
    let Some(root) = dtcg.as_object() else { return };
    for (section, body) in root {
        if section == "meta" {
            continue;
        }
        let Some(body) = body.as_object() else { continue };
        for (key, val) in body {
            walk_style_leaf(&[section.as_str(), key.as_str()], val, mode, out);
        }
    }
}

/// Recurse into a style value: either a DTCG leaf token (`{$type,$value}`) or a
/// nested group object. `path` is the dotted DTCG path segments so far.
fn walk_style_leaf(path: &[&str], val: &Value, mode: &str, out: &mut Vec<FigmaVariable>) {
    let dotted = path.join(".");
    let name = path.join("/");
    if let Some(obj) = val.as_object() {
        if let (Some(ty), Some(value)) = (obj.get("$type").and_then(Value::as_str), obj.get("$value")) {
            push_style_leaf(out, &dotted, &name, mode, ty, value);
            return;
        }
        // Nested group (e.g. a shadow spec): recurse one level deeper.
        for (k, v) in obj {
            let mut next: Vec<&str> = path.to_vec();
            next.push(k.as_str());
            walk_style_leaf(&next, v, mode, out);
        }
    }
}

fn push_style_leaf(
    out: &mut Vec<FigmaVariable>,
    dotted: &str,
    name: &str,
    mode: &str,
    ty: &str,
    value: &Value,
) {
    match ty {
        "boolean" => {
            push_value(out, STYLE_COLLECTION, name, FigmaType::Boolean, mode,
                FigmaValue::Bool(value.as_bool().unwrap_or(false)), None);
        }
        "string" => {
            if is_enum_path(dotted) {
                // Engine emits the canonical string; Figma stores the integer.
                let s = value.as_str().unwrap_or("");
                let n = enum_str_to_int(dotted, s).unwrap_or(0);
                let desc = enum_description(dotted);
                push_value(out, STYLE_COLLECTION, name, FigmaType::Float, mode,
                    FigmaValue::Num(n as f64), Some(desc));
            } else {
                push_value(out, STYLE_COLLECTION, name, FigmaType::String, mode,
                    FigmaValue::Str(value.as_str().unwrap_or("").to_string()), None);
            }
        }
        // dimension / number / integer all map to a Figma FLOAT.
        _ => {
            push_value(out, STYLE_COLLECTION, name, FigmaType::Float, mode,
                FigmaValue::Num(value.as_f64().unwrap_or(0.0)), None);
        }
    }
}

fn enum_description(dotted: &str) -> String {
    match dotted {
        "treatments.focus_ring" => "enum focus_ring: 0=none 1=outline 2=glow".into(),
        "treatments.surface_bevel" => "enum surface_bevel: 0=none 1=raised 2=inset".into(),
        "chrome.pane_active_indicator" => {
            "enum pane_active_indicator: 0=none 1=top_stripe 2=header_fill 3=both".into()
        }
        "chrome.button_group" => "enum button_group: 0=none 1=bordered 2=frosted 3=sharp".into(),
        _ => String::new(),
    }
}

fn push_color(out: &mut Vec<FigmaVariable>, collection: &str, name: &str, mode: &str, hex: &str) {
    push_value(out, collection, name, FigmaType::Color, mode,
        FigmaValue::Color(FigmaColor::from_hex(hex)), None);
}

fn push_value(
    out: &mut Vec<FigmaVariable>,
    collection: &str,
    name: &str,
    ty: FigmaType,
    mode: &str,
    value: FigmaValue,
    description: Option<String>,
) {
    let mut values_by_mode = BTreeMap::new();
    values_by_mode.insert(mode.to_string(), value);
    out.push(FigmaVariable {
        collection: collection.to_string(),
        name: name.to_string(),
        ty,
        values_by_mode,
        description,
    });
}

/// Collapse per-mode `FigmaVariable`s (one per mode) into one variable per
/// `(collection, name)` with a merged `valuesByMode` map.
fn merge_modes(vars: Vec<FigmaVariable>) -> Vec<FigmaVariable> {
    // Preserve first-seen ordering for stable, diff-friendly output.
    let mut order: Vec<(String, String)> = Vec::new();
    let mut by_key: BTreeMap<(String, String), FigmaVariable> = BTreeMap::new();
    for v in vars {
        let key = (v.collection.clone(), v.name.clone());
        match by_key.get_mut(&key) {
            Some(existing) => {
                for (m, val) in v.values_by_mode {
                    existing.values_by_mode.insert(m, val);
                }
                if existing.description.is_none() {
                    existing.description = v.description;
                }
            }
            None => {
                order.push(key.clone());
                by_key.insert(key, v);
            }
        }
    }
    order.into_iter().filter_map(|k| by_key.remove(&k)).collect()
}

// ── Importer: Figma export → ThemePack ─────────────────────────────────────────

/// Transform a [`FigmaThemeExport`] into a [`ThemePack`] for the chosen modes.
///
/// Reuses the engine's `from_dtcg` for the heavy lifting: this function only
/// reassembles DTCG JSON from the variables, applying the name-override
/// [`FigmaMapping`] and the enum/colour conversions.
pub fn figma_export_to_pack(
    export: &FigmaThemeExport,
    opts: &ImportOptions,
    mapping: &FigmaMapping,
) -> Result<ThemePack, ImportError> {
    let color_mode = resolve_mode(&export.color_collection, opts.color_mode.as_deref())?;
    let style_mode = resolve_mode(&export.style_collection, opts.style_mode.as_deref())?;

    // ── ColorScheme ─────────────────────────────────────────────────────────
    let color_meta = export.color_collection.meta.get(&color_mode);
    let mut palette = Map::new();
    let mut cmd_slots: BTreeMap<usize, String> = BTreeMap::new();
    for var in export.vars_in(COLOR_COLLECTION) {
        let Some(FigmaValue::Color(c)) = var.values_by_mode.get(&color_mode) else { continue };
        let engine = mapping.engine_path(COLOR_COLLECTION, &var.name);
        if let Some(slot) = engine.strip_prefix("cmd_palette/").or_else(|| engine.strip_prefix("cmd_palette.")) {
            if let Some(i) = CMD_PALETTE_SLOTS.iter().position(|s| *s == slot) {
                cmd_slots.insert(i, c.to_hex());
            }
            continue;
        }
        palette.insert(engine.replace('/', "."), color_token(&c.to_hex()));
    }
    if !cmd_slots.is_empty() {
        // Reassemble the 11-slot array in canonical order; only fully-specified
        // palettes are emitted (the loader fills gaps from CMD_PALETTE_DEFAULT).
        let arr: Vec<Value> = (0..CMD_PALETTE_SLOTS.len())
            .filter_map(|i| cmd_slots.get(&i).map(|hex| color_token(hex)))
            .collect();
        if arr.len() == CMD_PALETTE_SLOTS.len() {
            palette.insert("cmd_palette".into(), Value::Array(arr));
        }
    }
    let color_root = json!({ "meta": meta_value(color_meta, &color_mode), "palette": Value::Object(palette) });
    let color_scheme = ColorScheme::from_dtcg(&color_root.to_string())
        .map_err(|e| ImportError::Parse(format!("color scheme: {e:?}")))?;

    // ── StyleSystem ───────────────────────────────────────────────────────────
    let style_meta = export.style_collection.meta.get(&style_mode);
    let mut style_root = Map::new();
    style_root.insert("meta".into(), meta_value(style_meta, &style_mode));
    for var in export.vars_in(STYLE_COLLECTION) {
        let Some(value) = var.values_by_mode.get(&style_mode) else { continue };
        let engine = mapping.engine_path(STYLE_COLLECTION, &var.name);
        let dotted = engine.replace('/', ".");
        let token = style_token(&dotted, var.ty, value);
        insert_path(&mut style_root, &dotted, token);
    }
    let style_system = StyleSystem::from_dtcg(&Value::Object(style_root).to_string())
        .map_err(|e| ImportError::Parse(format!("style system: {e:?}")))?;

    // ── Recipes ─────────────────────────────────────────────────────────────
    let mut recipes = RecipeSet::new();
    for (key, spec_json) in &export.components {
        let spec: RecipeSpec = serde_json::from_value(spec_json.clone())
            .map_err(|e| ImportError::Recipe(format!("{key}: {e}")))?;
        recipes.insert(key.clone(), spec);
    }

    // ── Manifest + pack ───────────────────────────────────────────────────────
    let id = color_meta.map(|m| m.id.clone()).unwrap_or_else(|| color_scheme.meta.id.clone());
    let name = color_meta.map(|m| m.name.clone()).unwrap_or_else(|| color_scheme.meta.name.clone());
    let mut manifest = ThemeManifest::new(id, name);
    manifest.is_dark = color_scheme.meta.is_dark;

    Ok(ThemePack {
        manifest,
        color_scheme,
        style_system,
        recipes,
        shell_profile: None,
        assets: AssetRegistry::new(),
    })
}

/// Pick the mode to import: explicit request → collection default → first listed.
fn resolve_mode(c: &CollectionInfo, requested: Option<&str>) -> Result<String, ImportError> {
    if let Some(r) = requested {
        if c.modes.iter().any(|m| m == r) {
            return Ok(r.to_string());
        }
        return Err(ImportError::MissingMode(r.to_string()));
    }
    if c.modes.iter().any(|m| m == DEFAULT_MODE) {
        return Ok(DEFAULT_MODE.to_string());
    }
    if c.modes.iter().any(|m| m == &c.default_mode) {
        return Ok(c.default_mode.clone());
    }
    c.modes.first().cloned().ok_or_else(|| ImportError::MissingMode("<none>".into()))
}

fn color_token(hex: &str) -> Value {
    json!({ "$type": "color", "$value": hex })
}

fn meta_value(meta: Option<&ModeMeta>, mode: &str) -> Value {
    match meta {
        Some(m) => json!({ "id": m.id, "name": m.name, "is_dark": m.is_dark }),
        // Fall back to the mode name as a sane identity.
        None => json!({ "id": mode.to_lowercase(), "name": mode, "is_dark": true }),
    }
}

/// Build the DTCG leaf token for a style variable, inverting the enum/number forms.
fn style_token(dotted: &str, ty: FigmaType, value: &FigmaValue) -> Value {
    match (ty, value) {
        (FigmaType::Boolean, FigmaValue::Bool(b)) => json!({ "$type": "boolean", "$value": b }),
        (FigmaType::String, FigmaValue::Str(s)) => json!({ "$type": "string", "$value": s }),
        (FigmaType::Color, FigmaValue::Color(c)) => color_token(&c.to_hex()),
        (FigmaType::Float, FigmaValue::Num(n)) => {
            if is_enum_path(dotted) {
                // Figma stores an integer; the engine wants the canonical string.
                let s = enum_int_to_str(dotted, *n as i64).unwrap_or("none");
                json!({ "$type": "string", "$value": s })
            } else {
                json!({ "$type": "number", "$value": n })
            }
        }
        // Defensive: a mode value whose JSON form disagrees with the declared
        // type still produces a best-effort token rather than failing the import.
        (_, FigmaValue::Num(n)) => json!({ "$type": "number", "$value": n }),
        (_, FigmaValue::Bool(b)) => json!({ "$type": "boolean", "$value": b }),
        (_, FigmaValue::Str(s)) => json!({ "$type": "string", "$value": s }),
        (_, FigmaValue::Color(c)) => color_token(&c.to_hex()),
    }
}

/// Insert `token` at the dotted `path` into `root`, creating intermediate objects.
/// e.g. `shadows.card.blur` → `root["shadows"]["card"]["blur"] = token`.
fn insert_path(root: &mut Map<String, Value>, path: &str, token: Value) {
    let segs: Vec<&str> = path.split('.').collect();
    if segs.is_empty() {
        return;
    }
    let mut cur = root;
    for seg in &segs[..segs.len() - 1] {
        let entry = cur
            .entry(seg.to_string())
            .or_insert_with(|| Value::Object(Map::new()));
        if !entry.is_object() {
            *entry = Value::Object(Map::new());
        }
        cur = entry.as_object_mut().expect("just ensured object");
    }
    cur.insert(segs[segs.len() - 1].to_string(), token);
}
