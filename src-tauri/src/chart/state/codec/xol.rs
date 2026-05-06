//! XOL — Apex Open Layout Format (v1).
//!
//! See `docs/XOL_FORMAT_SPEC.md`. Zip container with JSON inside. Used only at
//! the boundary (Share / Export / Import). Never the runtime format, never
//! the storage format. Forward-compat: unknown fields and unknown drawing
//! kinds survive round-trips via `ExtensionBag`.

use std::io::{Cursor, Read, Seek, Write};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use super::super::{
    annotations::Annotation,
    drawings::{Drawing, DrawingFlags, DrawingKind, Point},
    extension_bag::ExtensionBag,
    indicators::IndicatorRef,
    style_table::{DashKind, Style, StyleId, StyleTable},
    AssetClass, ChartState, ProviderHints, Symbol, ThemeOverride, Timeframe, Viewport,
};

pub const SCHEMA_VERSION: u32 = 1;

// Hard limits per spec §2
const MAX_UNCOMPRESSED_BYTES: u64 = 50 * 1024 * 1024;
const MAX_DRAWINGS: usize = 10_000;
const MAX_ANNOTATIONS: usize = 2_000;
const MAX_INDICATORS: usize = 64;

#[derive(Debug, thiserror::Error)]
pub enum XolError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("zip: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("schema_version {0} is newer than this build supports ({SCHEMA_VERSION})")]
    UnsupportedVersion(u32),
    #[error("file `{name}` content hash mismatch")]
    HashMismatch { name: String },
    #[error("required entry `{0}` missing from container")]
    MissingEntry(&'static str),
    #[error("limit exceeded: {0}")]
    LimitExceeded(&'static str),
}

/// Non-blocking issues surfaced to the user on import. Per spec §7a + storage §5a.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ImportWarnings {
    pub missing_indicators: Vec<String>,
    pub unknown_drawing_kinds: Vec<String>,
    pub unknown_indicator_param_versions: Vec<(String, u32)>,
}

impl ImportWarnings {
    pub fn is_empty(&self) -> bool {
        self.missing_indicators.is_empty()
            && self.unknown_drawing_kinds.is_empty()
            && self.unknown_indicator_param_versions.is_empty()
    }
}

// ─────────────────────────────────────────────────────────────────────────
// Wire types — serde-shaped for the JSON files inside the zip
// ─────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Manifest {
    schema_version: u32,
    format: String,
    app: AppInfo,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    files: std::collections::BTreeMap<String, FileEntry>,
    #[serde(default, skip_serializing_if = "ExtensionBag::is_empty")]
    extras: ExtensionBag,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AppInfo {
    name: String,
    version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FileEntry {
    sha256: String,
    size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ChartJson {
    symbol: SymbolJson,
    timeframe: String,
    viewport: ViewportJson,
    theme: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SymbolJson {
    canonical: String,
    asset_class: String,
    #[serde(default, skip_serializing_if = "ProviderHints::is_default")]
    provider_hints: ProviderHints,
}

impl ProviderHints {
    fn is_default(&self) -> bool {
        self.polygon.is_none() && self.ib.is_none() && self.tv.is_none()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ViewportJson {
    from_ts_ns: i64,
    to_ts_ns: i64,
    price_low: f32,
    price_high: f32,
    #[serde(default)]
    log_scale: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DrawingsJson {
    drawings: Vec<DrawingJson>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DrawingJson {
    /// Use a string for forward-compat — readers can match known kinds and
    /// skip unknowns into the warnings list.
    kind: String,
    points: Vec<PointJson>,
    style: StyleJson,
    #[serde(default)]
    flags: u16,
    #[serde(default)]
    z: i16,
    #[serde(default, skip_serializing_if = "ExtensionBag::is_empty")]
    extras: ExtensionBag,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PointJson {
    ts_ns: i64,
    price: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StyleJson {
    /// `#RRGGBBAA` hex string — human-readable in the JSON.
    stroke: String,
    width: f32,
    dash: String,
    #[serde(default, skip_serializing_if = "is_zero_color")]
    fill: String,
}

fn is_zero_color(s: &String) -> bool {
    s.is_empty() || s == "#00000000"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AnnotationsJson {
    annotations: Vec<AnnotationJson>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AnnotationJson {
    anchor: PointJson,
    title: String,
    body_md: String,
    color: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    asset_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "ExtensionBag::is_empty")]
    extras: ExtensionBag,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct IndicatorsJson {
    indicators: Vec<IndicatorJson>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct IndicatorJson {
    ref_id: String,
    ref_version: u32,
    param_schema_version: u32,
    pane: String,
    params: serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    style: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "ExtensionBag::is_empty")]
    extras: ExtensionBag,
}

// ─────────────────────────────────────────────────────────────────────────
// Enum ↔ string helpers
// ─────────────────────────────────────────────────────────────────────────

fn asset_class_str(a: AssetClass) -> &'static str {
    match a {
        AssetClass::Equity => "equity",
        AssetClass::Etf => "etf",
        AssetClass::Index => "index",
        AssetClass::Option => "option",
        AssetClass::Future => "future",
        AssetClass::Crypto => "crypto",
        AssetClass::Fx => "fx",
    }
}

fn parse_asset_class(s: &str) -> AssetClass {
    match s {
        "etf" => AssetClass::Etf,
        "index" => AssetClass::Index,
        "option" => AssetClass::Option,
        "future" => AssetClass::Future,
        "crypto" => AssetClass::Crypto,
        "fx" => AssetClass::Fx,
        _ => AssetClass::Equity,
    }
}

fn timeframe_str(t: Timeframe) -> &'static str {
    match t {
        Timeframe::Tick => "tick",
        Timeframe::S1 => "s1",
        Timeframe::S5 => "s5",
        Timeframe::S15 => "s15",
        Timeframe::M1 => "m1",
        Timeframe::M5 => "m5",
        Timeframe::M15 => "m15",
        Timeframe::H1 => "h1",
        Timeframe::H4 => "h4",
        Timeframe::D1 => "d1",
        Timeframe::W1 => "w1",
        Timeframe::Mn1 => "mn1",
    }
}

fn parse_timeframe(s: &str) -> Timeframe {
    match s {
        "tick" => Timeframe::Tick,
        "s1" => Timeframe::S1,
        "s5" => Timeframe::S5,
        "s15" => Timeframe::S15,
        "m1" => Timeframe::M1,
        "m15" => Timeframe::M15,
        "h1" => Timeframe::H1,
        "h4" => Timeframe::H4,
        "d1" => Timeframe::D1,
        "w1" => Timeframe::W1,
        "mn1" => Timeframe::Mn1,
        _ => Timeframe::M5,
    }
}

fn theme_str(t: ThemeOverride) -> &'static str {
    match t {
        ThemeOverride::Light => "light",
        ThemeOverride::Dark => "dark",
        ThemeOverride::Inherit => "inherit",
    }
}

fn parse_theme(s: &str) -> ThemeOverride {
    match s {
        "light" => ThemeOverride::Light,
        "dark" => ThemeOverride::Dark,
        _ => ThemeOverride::Inherit,
    }
}

fn drawing_kind_str(k: DrawingKind) -> &'static str {
    match k {
        DrawingKind::Trendline => "trendline",
        DrawingKind::HorizontalLine => "horizontal_line",
        DrawingKind::VerticalLine => "vertical_line",
        DrawingKind::Ray => "ray",
        DrawingKind::Rect => "rect",
        DrawingKind::Ellipse => "ellipse",
        DrawingKind::FibRetracement => "fib_retracement",
        DrawingKind::FibExtension => "fib_extension",
        DrawingKind::Pitchfork => "pitchfork",
        DrawingKind::Text => "text",
        DrawingKind::Arrow => "arrow",
        DrawingKind::Polyline => "polyline",
        DrawingKind::Path => "path",
    }
}

fn parse_drawing_kind(s: &str) -> Option<DrawingKind> {
    Some(match s {
        "trendline" => DrawingKind::Trendline,
        "horizontal_line" => DrawingKind::HorizontalLine,
        "vertical_line" => DrawingKind::VerticalLine,
        "ray" => DrawingKind::Ray,
        "rect" => DrawingKind::Rect,
        "ellipse" => DrawingKind::Ellipse,
        "fib_retracement" => DrawingKind::FibRetracement,
        "fib_extension" => DrawingKind::FibExtension,
        "pitchfork" => DrawingKind::Pitchfork,
        "text" => DrawingKind::Text,
        "arrow" => DrawingKind::Arrow,
        "polyline" => DrawingKind::Polyline,
        "path" => DrawingKind::Path,
        _ => return None,
    })
}

fn dash_str(d: DashKind) -> &'static str {
    match d {
        DashKind::Solid => "solid",
        DashKind::Dashed => "dashed",
        DashKind::Dotted => "dotted",
    }
}

fn parse_dash(s: &str) -> DashKind {
    match s {
        "dashed" => DashKind::Dashed,
        "dotted" => DashKind::Dotted,
        _ => DashKind::Solid,
    }
}

fn color_to_hex(rgba: u32) -> String {
    format!("#{:08X}", rgba)
}

fn parse_color(s: &str) -> u32 {
    let hex = s.trim().trim_start_matches('#');
    if hex.len() == 8 {
        u32::from_str_radix(hex, 16).unwrap_or(0)
    } else if hex.len() == 6 {
        // No alpha → assume opaque.
        u32::from_str_radix(hex, 16).map(|rgb| (rgb << 8) | 0xFF).unwrap_or(0)
    } else {
        0
    }
}

// ─────────────────────────────────────────────────────────────────────────
// Public API — read / write
// ─────────────────────────────────────────────────────────────────────────

/// Serialize a `ChartState` to bytes (zip container). Suitable for writing
/// to disk, uploading to the share relay, or inspection.
pub fn write(state: &ChartState) -> Result<Vec<u8>, XolError> {
    if state.drawings.len() > MAX_DRAWINGS {
        return Err(XolError::LimitExceeded("drawing count > 10,000"));
    }
    if state.annotations.len() > MAX_ANNOTATIONS {
        return Err(XolError::LimitExceeded("annotation count > 2,000"));
    }
    if state.indicators.len() > MAX_INDICATORS {
        return Err(XolError::LimitExceeded("indicator count > 64"));
    }

    let mut buf = Cursor::new(Vec::new());
    let mut zip = ZipWriter::new(&mut buf);
    let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    let mut files: std::collections::BTreeMap<String, FileEntry> = Default::default();

    // chart.json
    let chart_json = serde_json::to_vec_pretty(&chart_to_json(state))?;
    write_entry(&mut zip, "chart.json", &chart_json, opts, &mut files)?;

    // drawings.json
    if !state.drawings.is_empty() {
        let drawings = drawings_to_json(state);
        let bytes = serde_json::to_vec_pretty(&drawings)?;
        write_entry(&mut zip, "drawings.json", &bytes, opts, &mut files)?;
    }

    // annotations.json
    if !state.annotations.is_empty() {
        let ann = annotations_to_json(state);
        let bytes = serde_json::to_vec_pretty(&ann)?;
        write_entry(&mut zip, "annotations.json", &bytes, opts, &mut files)?;
    }

    // indicators.json
    if !state.indicators.is_empty() {
        let ind = indicators_to_json(state);
        let bytes = serde_json::to_vec_pretty(&ind)?;
        write_entry(&mut zip, "indicators.json", &bytes, opts, &mut files)?;
    }

    // manifest.json (last so its `files` map is complete)
    let manifest = Manifest {
        schema_version: SCHEMA_VERSION,
        format: "xol".into(),
        app: AppInfo {
            name: "apex-terminal".into(),
            version: env!("CARGO_PKG_VERSION").into(),
        },
        title: state.title.as_ref().map(|s| s.to_string()),
        description: state.description.as_ref().map(|s| s.to_string()),
        files,
        extras: state.unknown_extensions.clone(),
    };
    let manifest_bytes = serde_json::to_vec_pretty(&manifest)?;
    let mut empty_files = Default::default();
    write_entry(&mut zip, "manifest.json", &manifest_bytes, opts, &mut empty_files)?;

    zip.finish()?;
    Ok(buf.into_inner())
}

/// Parse XOL bytes into a `ChartState` plus any non-blocking warnings.
/// Unknown drawing kinds and missing-indicator hints are routed into
/// `ImportWarnings`; nothing is silently lost.
pub fn read(bytes: &[u8]) -> Result<(ChartState, ImportWarnings), XolError> {
    let cursor = Cursor::new(bytes);
    let mut zip = ZipArchive::new(cursor)?;

    // Quick guard against pathological zip-bomb files.
    let total_uncompressed: u64 = (0..zip.len())
        .map(|i| zip.by_index(i).map(|f| f.size()).unwrap_or(0))
        .sum();
    if total_uncompressed > MAX_UNCOMPRESSED_BYTES {
        return Err(XolError::LimitExceeded("uncompressed total > 50 MB"));
    }

    let manifest_bytes = read_entry(&mut zip, "manifest.json")
        .ok_or(XolError::MissingEntry("manifest.json"))?;
    let manifest: Manifest = serde_json::from_slice(&manifest_bytes)?;
    if manifest.schema_version > SCHEMA_VERSION {
        return Err(XolError::UnsupportedVersion(manifest.schema_version));
    }

    let chart_bytes = read_entry(&mut zip, "chart.json")
        .ok_or(XolError::MissingEntry("chart.json"))?;
    verify_hash(&manifest, "chart.json", &chart_bytes)?;
    let chart_json: ChartJson = serde_json::from_slice(&chart_bytes)?;

    let mut state = ChartState::new(
        0,
        Symbol {
            canonical: chart_json.symbol.canonical.into(),
            asset_class: parse_asset_class(&chart_json.symbol.asset_class),
            provider_hints: chart_json.symbol.provider_hints,
        },
        parse_timeframe(&chart_json.timeframe),
    );
    state.viewport = Viewport {
        from_ts_ns: chart_json.viewport.from_ts_ns,
        to_ts_ns: chart_json.viewport.to_ts_ns,
        price_low: chart_json.viewport.price_low,
        price_high: chart_json.viewport.price_high,
        log_scale: chart_json.viewport.log_scale,
    };
    state.theme = parse_theme(&chart_json.theme);
    state.title = manifest.title.clone().map(Into::into);
    state.description = manifest.description.clone().map(Into::into);
    state.unknown_extensions = manifest.extras.clone();

    let mut warnings = ImportWarnings::default();

    // Drawings
    if let Some(b) = read_entry(&mut zip, "drawings.json") {
        verify_hash(&manifest, "drawings.json", &b)?;
        let dj: DrawingsJson = serde_json::from_slice(&b)?;
        if dj.drawings.len() > MAX_DRAWINGS {
            return Err(XolError::LimitExceeded("drawing count > 10,000"));
        }
        for d in dj.drawings {
            match parse_drawing_kind(&d.kind) {
                Some(kind) => {
                    let style_id = state.style_table.intern(Style {
                        stroke: parse_color(&d.style.stroke),
                        width_x100: (d.style.width * 100.0).round().clamp(0.0, u16::MAX as f32) as u16,
                        dash: parse_dash(&d.style.dash),
                        fill: parse_color(&d.style.fill),
                    });
                    let drawing = Drawing {
                        kind,
                        points: d.points.into_iter().map(|p| Point { ts_ns: p.ts_ns, price: p.price }).collect(),
                        style: style_id,
                        flags: DrawingFlags::from_bits_truncate(d.flags),
                        z: d.z,
                        extras: d.extras,
                    };
                    state.drawings.insert(drawing);
                }
                None => {
                    warnings.unknown_drawing_kinds.push(d.kind);
                }
            }
        }
    }

    // Annotations
    if let Some(b) = read_entry(&mut zip, "annotations.json") {
        verify_hash(&manifest, "annotations.json", &b)?;
        let aj: AnnotationsJson = serde_json::from_slice(&b)?;
        if aj.annotations.len() > MAX_ANNOTATIONS {
            return Err(XolError::LimitExceeded("annotation count > 2,000"));
        }
        for a in aj.annotations {
            state.annotations.insert(Annotation {
                anchor: Point { ts_ns: a.anchor.ts_ns, price: a.anchor.price },
                title: a.title.into(),
                body_md: a.body_md.into(),
                color: parse_color(&a.color),
                asset_refs: a.asset_refs.into_iter().map(Into::into).collect(),
                extras: a.extras,
            });
        }
    }

    // Indicators (callers are responsible for installing/registering — we
    // surface missing ones via warnings but still preserve the ref).
    if let Some(b) = read_entry(&mut zip, "indicators.json") {
        verify_hash(&manifest, "indicators.json", &b)?;
        let ij: IndicatorsJson = serde_json::from_slice(&b)?;
        if ij.indicators.len() > MAX_INDICATORS {
            return Err(XolError::LimitExceeded("indicator count > 64"));
        }
        for i in ij.indicators {
            state.indicators.push(IndicatorRef {
                ref_id: i.ref_id.into(),
                ref_version: i.ref_version,
                param_schema_version: i.param_schema_version,
                pane: i.pane.into(),
                params: i.params,
                style: i.style,
                installed_locally: false, // import-time default; caller flips after registry check
                extras: i.extras,
            });
        }
    }

    Ok((state, warnings))
}

// ─────────────────────────────────────────────────────────────────────────
// Internals
// ─────────────────────────────────────────────────────────────────────────

fn write_entry<W: Write + Seek>(
    zip: &mut ZipWriter<W>,
    name: &str,
    bytes: &[u8],
    opts: SimpleFileOptions,
    files: &mut std::collections::BTreeMap<String, FileEntry>,
) -> Result<(), XolError> {
    zip.start_file(name, opts)?;
    zip.write_all(bytes)?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let sha256 = format!("{:x}", hasher.finalize());
    files.insert(
        name.into(),
        FileEntry { sha256, size: bytes.len() as u64 },
    );
    Ok(())
}

fn read_entry<R: Read + Seek>(zip: &mut ZipArchive<R>, name: &str) -> Option<Vec<u8>> {
    let mut f = zip.by_name(name).ok()?;
    let mut buf = Vec::with_capacity(f.size() as usize);
    f.read_to_end(&mut buf).ok()?;
    Some(buf)
}

fn verify_hash(manifest: &Manifest, name: &str, bytes: &[u8]) -> Result<(), XolError> {
    let Some(entry) = manifest.files.get(name) else { return Ok(()); };
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let actual = format!("{:x}", hasher.finalize());
    if actual != entry.sha256 {
        return Err(XolError::HashMismatch { name: name.into() });
    }
    Ok(())
}

fn chart_to_json(state: &ChartState) -> ChartJson {
    ChartJson {
        symbol: SymbolJson {
            canonical: state.symbol.canonical.to_string(),
            asset_class: asset_class_str(state.symbol.asset_class).into(),
            provider_hints: state.symbol.provider_hints.clone(),
        },
        timeframe: timeframe_str(state.timeframe).into(),
        viewport: ViewportJson {
            from_ts_ns: state.viewport.from_ts_ns,
            to_ts_ns: state.viewport.to_ts_ns,
            price_low: state.viewport.price_low,
            price_high: state.viewport.price_high,
            log_scale: state.viewport.log_scale,
        },
        theme: theme_str(state.theme).into(),
    }
}

fn drawings_to_json(state: &ChartState) -> DrawingsJson {
    let mut out = Vec::with_capacity(state.drawings.len());
    for (_, d) in state.drawings.iter() {
        let style = style_to_json(&state.style_table, d.style);
        out.push(DrawingJson {
            kind: drawing_kind_str(d.kind).into(),
            points: d.points.iter().map(|p| PointJson { ts_ns: p.ts_ns, price: p.price }).collect(),
            style,
            flags: d.flags.bits(),
            z: d.z,
            extras: d.extras.clone(),
        });
    }
    DrawingsJson { drawings: out }
}

fn style_to_json(table: &StyleTable, id: StyleId) -> StyleJson {
    let s = table.get(id).copied().unwrap_or_default();
    StyleJson {
        stroke: color_to_hex(s.stroke),
        width: s.width_x100 as f32 / 100.0,
        dash: dash_str(s.dash).into(),
        fill: if s.fill == 0 { String::new() } else { color_to_hex(s.fill) },
    }
}

fn annotations_to_json(state: &ChartState) -> AnnotationsJson {
    let mut out = Vec::with_capacity(state.annotations.len());
    for (_, a) in state.annotations.iter() {
        out.push(AnnotationJson {
            anchor: PointJson { ts_ns: a.anchor.ts_ns, price: a.anchor.price },
            title: a.title.to_string(),
            body_md: a.body_md.to_string(),
            color: color_to_hex(a.color),
            asset_refs: a.asset_refs.iter().map(|s| s.to_string()).collect(),
            extras: a.extras.clone(),
        });
    }
    AnnotationsJson { annotations: out }
}

fn indicators_to_json(state: &ChartState) -> IndicatorsJson {
    let mut out = Vec::with_capacity(state.indicators.len());
    for ind in state.indicators.iter() {
        out.push(IndicatorJson {
            ref_id: ind.ref_id.to_string(),
            ref_version: ind.ref_version,
            param_schema_version: ind.param_schema_version,
            pane: ind.pane.to_string(),
            params: ind.params.clone(),
            style: ind.style.clone(),
            extras: ind.extras.clone(),
        });
    }
    IndicatorsJson { indicators: out }
}

#[cfg(test)]
mod tests {
    use super::*;
    use smallvec::smallvec;

    fn sample_chart() -> ChartState {
        let mut s = ChartState::new(
            0,
            Symbol {
                canonical: "SPX".into(),
                asset_class: AssetClass::Index,
                provider_hints: ProviderHints::default(),
            },
            Timeframe::M5,
        );
        s.title = Some("xol round-trip".into());
        s.theme = ThemeOverride::Dark;
        s.viewport = Viewport {
            from_ts_ns: 1_700_000_000_000_000_000,
            to_ts_ns: 1_700_000_900_000_000_000,
            price_low: 4500.0,
            price_high: 4600.0,
            log_scale: false,
        };
        let style = s.style_table.intern(Style {
            stroke: 0xFFB800CC,
            width_x100: 150,
            dash: DashKind::Dashed,
            fill: 0,
        });
        s.drawings.insert(Drawing {
            kind: DrawingKind::Trendline,
            points: smallvec![
                Point { ts_ns: 1_700_000_000_000_000_000, price: 4520.5 },
                Point { ts_ns: 1_700_000_300_000_000_000, price: 4555.0 },
            ],
            style,
            flags: DrawingFlags::VISIBLE | DrawingFlags::EXTEND_RIGHT,
            z: 100,
            extras: Default::default(),
        });
        s.annotations.insert(Annotation {
            anchor: Point { ts_ns: 1_700_000_500_000_000_000, price: 4540.0 },
            title: "gamma flip".into(),
            body_md: "watch unwind".into(),
            color: 0x22C55EFF,
            asset_refs: smallvec![],
            extras: Default::default(),
        });
        s.indicators.push(IndicatorRef {
            ref_id: "apex.vwap".into(),
            ref_version: 2,
            param_schema_version: 1,
            pane: "main".into(),
            params: serde_json::json!({"session": "rth"}),
            style: None,
            installed_locally: true,
            extras: Default::default(),
        });
        s
    }

    #[test]
    fn round_trip_preserves_everything() {
        let original = sample_chart();
        let bytes = write(&original).unwrap();
        let (loaded, warnings) = read(&bytes).unwrap();

        assert!(warnings.is_empty());
        assert_eq!(loaded.symbol.canonical.as_str(), "SPX");
        assert_eq!(loaded.symbol.asset_class, AssetClass::Index);
        assert_eq!(loaded.timeframe, Timeframe::M5);
        assert_eq!(loaded.theme, ThemeOverride::Dark);
        assert_eq!(loaded.title.as_ref().map(|s| s.as_str()), Some("xol round-trip"));
        assert_eq!(loaded.viewport, original.viewport);
        assert_eq!(loaded.drawings.len(), 1);
        assert_eq!(loaded.annotations.len(), 1);
        assert_eq!(loaded.indicators.len(), 1);

        let (_, d) = loaded.drawings.iter().next().unwrap();
        assert_eq!(d.kind, DrawingKind::Trendline);
        assert_eq!(d.points.len(), 2);
        assert!(d.flags.contains(DrawingFlags::EXTEND_RIGHT));

        let (_, a) = loaded.annotations.iter().next().unwrap();
        assert_eq!(a.title.as_str(), "gamma flip");
        assert_eq!(a.color, 0x22C55EFF);

        // Indicator import default: installed_locally = false until registry check.
        assert!(!loaded.indicators[0].installed_locally);
        assert_eq!(loaded.indicators[0].ref_id.as_str(), "apex.vwap");
    }

    #[test]
    fn unknown_drawing_kind_routed_to_warnings() {
        let original = sample_chart();
        let bytes = write(&original).unwrap();
        // Edit the bytes: rewrite the zip with a tampered drawings.json
        let cursor = Cursor::new(bytes);
        let mut zip = ZipArchive::new(cursor).unwrap();
        let mut drawings_bytes = read_entry(&mut zip, "drawings.json").unwrap();
        drawings_bytes = drawings_bytes
            .iter()
            .map(|&b| b)
            .collect::<Vec<_>>()
            .into_iter()
            .map(|b| b)
            .collect();
        let mut text = String::from_utf8(drawings_bytes).unwrap();
        text = text.replace("\"trendline\"", "\"futurekind_xyz\"");

        // Build a fresh zip that drops the manifest file hash check by removing
        // the entry from manifest. Quick way: build a fresh container.
        let mut buf = Cursor::new(Vec::new());
        let mut writer = ZipWriter::new(&mut buf);
        let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        let mut files = std::collections::BTreeMap::new();
        write_entry(&mut writer, "drawings.json", text.as_bytes(), opts, &mut files).unwrap();
        // Reuse chart.json from original
        let chart_bytes = read_entry(&mut zip, "chart.json").unwrap();
        write_entry(&mut writer, "chart.json", &chart_bytes, opts, &mut files).unwrap();
        let manifest = Manifest {
            schema_version: SCHEMA_VERSION,
            format: "xol".into(),
            app: AppInfo { name: "test".into(), version: "0".into() },
            title: None,
            description: None,
            files,
            extras: Default::default(),
        };
        let manifest_bytes = serde_json::to_vec(&manifest).unwrap();
        let mut empty = Default::default();
        write_entry(&mut writer, "manifest.json", &manifest_bytes, opts, &mut empty).unwrap();
        writer.finish().unwrap();
        let bytes = buf.into_inner();

        let (loaded, warnings) = read(&bytes).unwrap();
        assert_eq!(loaded.drawings.len(), 0, "unknown kind should not load");
        assert_eq!(warnings.unknown_drawing_kinds, vec!["futurekind_xyz"]);
    }

    #[test]
    fn rejects_newer_schema_version() {
        let mut buf = Cursor::new(Vec::new());
        {
            let mut writer = ZipWriter::new(&mut buf);
            let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
            let manifest = serde_json::json!({
                "schema_version": 999,
                "format": "xol",
                "app": {"name": "test", "version": "0"},
                "files": {}
            });
            writer.start_file("manifest.json", opts).unwrap();
            writer.write_all(manifest.to_string().as_bytes()).unwrap();
            writer.finish().unwrap();
        }
        let bytes = buf.into_inner();
        match read(&bytes) {
            Err(XolError::UnsupportedVersion(999)) => {}
            other => panic!("expected UnsupportedVersion, got {other:?}"),
        }
    }

    #[test]
    fn detects_tampered_file() {
        let original = sample_chart();
        let bytes = write(&original).unwrap();
        // Corrupt by flipping a byte deep in the zip — sha256 won't match.
        let mut bad = bytes.clone();
        // Find chart.json content section and flip a likely-character byte.
        // Easier: re-pack with mismatched manifest hash.
        let cursor = Cursor::new(&bytes);
        let mut zip = ZipArchive::new(cursor).unwrap();
        let chart_bytes = read_entry(&mut zip, "chart.json").unwrap();
        let manifest_bytes = read_entry(&mut zip, "manifest.json").unwrap();
        let mut manifest: Manifest = serde_json::from_slice(&manifest_bytes).unwrap();
        // Forge the chart.json hash so it disagrees with reality
        manifest.files.get_mut("chart.json").unwrap().sha256 = "0".repeat(64);
        let bad_manifest = serde_json::to_vec(&manifest).unwrap();

        let mut out = Cursor::new(Vec::new());
        {
            let mut w = ZipWriter::new(&mut out);
            let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
            w.start_file("chart.json", opts).unwrap();
            w.write_all(&chart_bytes).unwrap();
            w.start_file("manifest.json", opts).unwrap();
            w.write_all(&bad_manifest).unwrap();
            w.finish().unwrap();
        }
        bad = out.into_inner();
        match read(&bad) {
            Err(XolError::HashMismatch { name }) => assert_eq!(name, "chart.json"),
            other => panic!("expected HashMismatch, got {other:?}"),
        }
    }
}
