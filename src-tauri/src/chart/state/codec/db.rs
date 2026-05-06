//! Postgres codec — see `docs/CHART_STORAGE_ARCHITECTURE.md` §4.
//!
//! The schema is defined in `migrations/001_chart_state.sql`. Run that once
//! against the database before using these functions.
//!
//! This module provides:
//!   - `points_packing` — pure-Rust packed binary encoding for `BYTEA points`
//!     and `BYTEA viewport` columns. Tested without a DB.
//!   - `load_chart` / `save_chart` — async sqlx round-trips. (Wired in once
//!     the renderer reads from `ChartState`.)

use sqlx::{postgres::PgPool, Row};
use uuid::Uuid;

use super::super::{
    annotations::Annotation,
    drawings::{Drawing, DrawingFlags, DrawingKind, Point},
    indicators::IndicatorRef,
    style_table::{DashKind, Style, StyleTable},
    AssetClass, ChartState, ProviderHints, Symbol, ThemeOverride, Timeframe, Viewport,
};

// ─────────────────────────────────────────────────────────────────────────
// Discriminant mappings (enum ↔ SMALLINT)
// ─────────────────────────────────────────────────────────────────────────

fn asset_class_to_i16(a: AssetClass) -> i16 {
    match a {
        AssetClass::Equity => 0,
        AssetClass::Etf => 1,
        AssetClass::Index => 2,
        AssetClass::Option => 3,
        AssetClass::Future => 4,
        AssetClass::Crypto => 5,
        AssetClass::Fx => 6,
    }
}

fn asset_class_from_i16(v: i16) -> AssetClass {
    match v {
        0 => AssetClass::Equity,
        1 => AssetClass::Etf,
        2 => AssetClass::Index,
        3 => AssetClass::Option,
        4 => AssetClass::Future,
        5 => AssetClass::Crypto,
        _ => AssetClass::Fx,
    }
}

fn timeframe_to_i16(t: Timeframe) -> i16 {
    match t {
        Timeframe::Tick => 0,
        Timeframe::S1 => 1,
        Timeframe::S5 => 2,
        Timeframe::S15 => 3,
        Timeframe::M1 => 4,
        Timeframe::M5 => 5,
        Timeframe::M15 => 6,
        Timeframe::H1 => 7,
        Timeframe::H4 => 8,
        Timeframe::D1 => 9,
        Timeframe::W1 => 10,
        Timeframe::Mn1 => 11,
    }
}

fn timeframe_from_i16(v: i16) -> Timeframe {
    match v {
        0 => Timeframe::Tick,
        1 => Timeframe::S1,
        2 => Timeframe::S5,
        3 => Timeframe::S15,
        4 => Timeframe::M1,
        5 => Timeframe::M5,
        6 => Timeframe::M15,
        7 => Timeframe::H1,
        8 => Timeframe::H4,
        9 => Timeframe::D1,
        10 => Timeframe::W1,
        _ => Timeframe::Mn1,
    }
}

fn theme_to_i16(t: ThemeOverride) -> i16 {
    match t {
        ThemeOverride::Inherit => 0,
        ThemeOverride::Light => 1,
        ThemeOverride::Dark => 2,
    }
}

fn theme_from_i16(v: i16) -> ThemeOverride {
    match v {
        1 => ThemeOverride::Light,
        2 => ThemeOverride::Dark,
        _ => ThemeOverride::Inherit,
    }
}

fn drawing_kind_to_i16(k: DrawingKind) -> i16 {
    match k {
        DrawingKind::Trendline => 0,
        DrawingKind::HorizontalLine => 1,
        DrawingKind::VerticalLine => 2,
        DrawingKind::Ray => 3,
        DrawingKind::Rect => 4,
        DrawingKind::Ellipse => 5,
        DrawingKind::FibRetracement => 6,
        DrawingKind::FibExtension => 7,
        DrawingKind::Pitchfork => 8,
        DrawingKind::Text => 9,
        DrawingKind::Arrow => 10,
        DrawingKind::Polyline => 11,
        DrawingKind::Path => 12,
    }
}

fn drawing_kind_from_i16(v: i16) -> Option<DrawingKind> {
    Some(match v {
        0 => DrawingKind::Trendline,
        1 => DrawingKind::HorizontalLine,
        2 => DrawingKind::VerticalLine,
        3 => DrawingKind::Ray,
        4 => DrawingKind::Rect,
        5 => DrawingKind::Ellipse,
        6 => DrawingKind::FibRetracement,
        7 => DrawingKind::FibExtension,
        8 => DrawingKind::Pitchfork,
        9 => DrawingKind::Text,
        10 => DrawingKind::Arrow,
        11 => DrawingKind::Polyline,
        12 => DrawingKind::Path,
        _ => return None,
    })
}

fn dash_to_i16(d: DashKind) -> i16 {
    match d {
        DashKind::Solid => 0,
        DashKind::Dashed => 1,
        DashKind::Dotted => 2,
    }
}

fn dash_from_i16(v: i16) -> DashKind {
    match v {
        1 => DashKind::Dashed,
        2 => DashKind::Dotted,
        _ => DashKind::Solid,
    }
}

// ─────────────────────────────────────────────────────────────────────────
// Packed binary helpers — tested without a DB
// ─────────────────────────────────────────────────────────────────────────

pub mod points_packing {
    //! Compact `BYTEA points` encoding.
    //!
    //! - **v1**: small lists (≤4 points). Layout: `[tag=1][n: u8][i64 ts × n][f32 price × n]`.
    //!   Costs 12 bytes per point + 2 byte header. A 2-point trendline = 26 bytes.
    //! - **v2**: paths/polylines. Delta-varint timestamps + raw f32 prices.
    //!   Layout: `[tag=2][n: varint][base_ts: i64][delta_varint × (n-1)][f32 price × n]`.
    //!   Long polyline of 100 points typically lands ~600 bytes vs ~3 KB JSON.

    use super::Point;

    pub fn encode(points: &[Point]) -> Vec<u8> {
        if points.len() <= 4 {
            encode_v1(points)
        } else {
            encode_v2(points)
        }
    }

    pub fn decode(buf: &[u8]) -> Option<Vec<Point>> {
        match *buf.first()? {
            1 => decode_v1(&buf[1..]),
            2 => decode_v2(&buf[1..]),
            _ => None,
        }
    }

    fn encode_v1(points: &[Point]) -> Vec<u8> {
        let mut out = Vec::with_capacity(2 + points.len() * 12);
        out.push(1);
        out.push(points.len() as u8);
        for p in points {
            out.extend_from_slice(&p.ts_ns.to_le_bytes());
        }
        for p in points {
            out.extend_from_slice(&p.price.to_le_bytes());
        }
        out
    }

    fn decode_v1(buf: &[u8]) -> Option<Vec<Point>> {
        let n = *buf.first()? as usize;
        let ts_end = 1 + n * 8;
        let pr_end = ts_end + n * 4;
        if buf.len() < pr_end {
            return None;
        }
        let mut out = Vec::with_capacity(n);
        for i in 0..n {
            let ts_ns = i64::from_le_bytes(buf[1 + i * 8..1 + (i + 1) * 8].try_into().ok()?);
            let price = f32::from_le_bytes(
                buf[ts_end + i * 4..ts_end + (i + 1) * 4].try_into().ok()?,
            );
            out.push(Point { ts_ns, price });
        }
        Some(out)
    }

    fn encode_v2(points: &[Point]) -> Vec<u8> {
        let mut out = vec![2];
        write_varint(&mut out, points.len() as u64);
        out.extend_from_slice(&points[0].ts_ns.to_le_bytes());
        for w in points.windows(2) {
            let delta = w[1].ts_ns.wrapping_sub(w[0].ts_ns);
            write_signed_varint(&mut out, delta);
        }
        for p in points {
            out.extend_from_slice(&p.price.to_le_bytes());
        }
        out
    }

    fn decode_v2(buf: &[u8]) -> Option<Vec<Point>> {
        let mut idx = 0;
        let n = read_varint(buf, &mut idx)? as usize;
        if n == 0 {
            return Some(Vec::new());
        }
        if buf.len() < idx + 8 {
            return None;
        }
        let mut ts = i64::from_le_bytes(buf[idx..idx + 8].try_into().ok()?);
        idx += 8;
        let mut timestamps = Vec::with_capacity(n);
        timestamps.push(ts);
        for _ in 1..n {
            let delta = read_signed_varint(buf, &mut idx)?;
            ts = ts.wrapping_add(delta);
            timestamps.push(ts);
        }
        if buf.len() < idx + n * 4 {
            return None;
        }
        let mut out = Vec::with_capacity(n);
        for (i, ts_ns) in timestamps.into_iter().enumerate() {
            let price = f32::from_le_bytes(
                buf[idx + i * 4..idx + (i + 1) * 4].try_into().ok()?,
            );
            out.push(Point { ts_ns, price });
        }
        Some(out)
    }

    fn write_varint(out: &mut Vec<u8>, mut v: u64) {
        while v >= 0x80 {
            out.push((v as u8) | 0x80);
            v >>= 7;
        }
        out.push(v as u8);
    }

    fn read_varint(buf: &[u8], idx: &mut usize) -> Option<u64> {
        let mut shift = 0u32;
        let mut result = 0u64;
        loop {
            let byte = *buf.get(*idx)?;
            *idx += 1;
            result |= ((byte & 0x7F) as u64) << shift;
            if byte & 0x80 == 0 {
                return Some(result);
            }
            shift += 7;
            if shift >= 64 {
                return None;
            }
        }
    }

    fn write_signed_varint(out: &mut Vec<u8>, v: i64) {
        // ZigZag — keeps small positive and negative values short.
        let zz = ((v << 1) ^ (v >> 63)) as u64;
        write_varint(out, zz);
    }

    fn read_signed_varint(buf: &[u8], idx: &mut usize) -> Option<i64> {
        let zz = read_varint(buf, idx)?;
        Some(((zz >> 1) as i64) ^ -((zz & 1) as i64))
    }
}

pub mod viewport_packing {
    //! `[i64 from_ts_ns][i64 to_ts_ns][f32 price_low][f32 price_high][u8 flags]`
    //! → 25 bytes total. Bit 0 of flags = log_scale.

    use super::Viewport;

    pub fn encode(v: &Viewport) -> Vec<u8> {
        let mut out = Vec::with_capacity(25);
        out.extend_from_slice(&v.from_ts_ns.to_le_bytes());
        out.extend_from_slice(&v.to_ts_ns.to_le_bytes());
        out.extend_from_slice(&v.price_low.to_le_bytes());
        out.extend_from_slice(&v.price_high.to_le_bytes());
        out.push(if v.log_scale { 1 } else { 0 });
        out
    }

    pub fn decode(buf: &[u8]) -> Option<Viewport> {
        if buf.len() < 25 {
            return None;
        }
        Some(Viewport {
            from_ts_ns: i64::from_le_bytes(buf[0..8].try_into().ok()?),
            to_ts_ns: i64::from_le_bytes(buf[8..16].try_into().ok()?),
            price_low: f32::from_le_bytes(buf[16..20].try_into().ok()?),
            price_high: f32::from_le_bytes(buf[20..24].try_into().ok()?),
            log_scale: buf[24] != 0,
        })
    }
}

// ─────────────────────────────────────────────────────────────────────────
// sqlx round-trips
// ─────────────────────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum DbCodecError {
    #[error("sqlx: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("malformed packed bytes in column `{0}`")]
    MalformedBytes(&'static str),
    #[error("unknown drawing kind discriminant: {0}")]
    UnknownDrawingKind(i16),
    #[error("chart {0} not found")]
    NotFound(Uuid),
}

/// Insert a fresh `ChartState` into the database. Returns the new UUID.
///
/// Caller manages the `ChartState::id` field — we don't write it back; the
/// authoritative DB id is the returned UUID.
pub async fn save_chart(pool: &PgPool, state: &ChartState) -> Result<Uuid, DbCodecError> {
    let mut tx = pool.begin().await?;

    let chart_id: Uuid = sqlx::query_scalar(
        "INSERT INTO charts \
         (title, symbol_canonical, asset_class, timeframe, theme, viewport, schema_version, description, extras) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9) RETURNING id",
    )
    .bind(state.title.as_ref().map(|s| s.as_str()))
    .bind(state.symbol.canonical.as_str())
    .bind(asset_class_to_i16(state.symbol.asset_class))
    .bind(timeframe_to_i16(state.timeframe))
    .bind(theme_to_i16(state.theme))
    .bind(viewport_packing::encode(&state.viewport))
    .bind(1_i32)
    .bind(state.description.as_ref().map(|s| s.as_str()))
    .bind(serde_json::to_value(&state.unknown_extensions).unwrap_or(serde_json::json!({})))
    .fetch_one(&mut *tx)
    .await?;

    // Styles
    for (idx, style) in (0..state.style_table.len())
        .filter_map(|i| state.style_table.get(super::super::style_table::StyleId(i as u32)).map(|s| (i, *s)))
    {
        sqlx::query(
            "INSERT INTO chart_styles (chart_id, style_id, stroke, width_x100, dash, fill) \
             VALUES ($1,$2,$3,$4,$5,$6)",
        )
        .bind(chart_id)
        .bind(idx as i32)
        .bind(style.stroke as i32)
        .bind(style.width_x100 as i16)
        .bind(dash_to_i16(style.dash))
        .bind(style.fill as i32)
        .execute(&mut *tx)
        .await?;
    }

    // Drawings
    for (_, d) in state.drawings.iter() {
        sqlx::query(
            "INSERT INTO drawings (chart_id, kind, z, flags, style_id, points, extras) \
             VALUES ($1,$2,$3,$4,$5,$6,$7)",
        )
        .bind(chart_id)
        .bind(drawing_kind_to_i16(d.kind))
        .bind(d.z)
        .bind(d.flags.bits() as i16)
        .bind(d.style.0 as i32)
        .bind(points_packing::encode(&d.points))
        .bind(serde_json::to_value(&d.extras).unwrap_or(serde_json::json!({})))
        .execute(&mut *tx)
        .await?;
    }

    // Annotations
    for (_, a) in state.annotations.iter() {
        sqlx::query(
            "INSERT INTO chart_annotations (chart_id, anchor_ts_ns, anchor_price, title, body_md, color, asset_refs, extras) \
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8)",
        )
        .bind(chart_id)
        .bind(a.anchor.ts_ns)
        .bind(a.anchor.price)
        .bind(a.title.as_str())
        .bind(a.body_md.as_str())
        .bind(a.color as i32)
        .bind(a.asset_refs.iter().map(|s| s.as_str().to_string()).collect::<Vec<_>>())
        .bind(serde_json::to_value(&a.extras).unwrap_or(serde_json::json!({})))
        .execute(&mut *tx)
        .await?;
    }

    // Indicator refs
    for ind in state.indicators.iter() {
        sqlx::query(
            "INSERT INTO indicator_refs \
             (chart_id, ref_id, ref_version, param_schema_version, pane, params, style, installed_locally, extras) \
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)",
        )
        .bind(chart_id)
        .bind(ind.ref_id.as_str())
        .bind(ind.ref_version as i32)
        .bind(ind.param_schema_version as i32)
        .bind(ind.pane.as_str())
        .bind(&ind.params)
        .bind(ind.style.as_ref())
        .bind(ind.installed_locally)
        .bind(serde_json::to_value(&ind.extras).unwrap_or(serde_json::json!({})))
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(chart_id)
}

/// Load a chart by UUID into a `ChartState`. Returns `NotFound` if missing.
pub async fn load_chart(pool: &PgPool, chart_id: Uuid) -> Result<ChartState, DbCodecError> {
    let row = sqlx::query(
        "SELECT title, symbol_canonical, asset_class, timeframe, theme, viewport, description, extras \
         FROM charts WHERE id = $1",
    )
    .bind(chart_id)
    .fetch_optional(pool)
    .await?
    .ok_or(DbCodecError::NotFound(chart_id))?;

    let title: Option<String> = row.try_get("title")?;
    let symbol_canonical: String = row.try_get("symbol_canonical")?;
    let asset_class: i16 = row.try_get("asset_class")?;
    let timeframe: i16 = row.try_get("timeframe")?;
    let theme: i16 = row.try_get("theme")?;
    let viewport_bytes: Vec<u8> = row.try_get("viewport")?;
    let description: Option<String> = row.try_get("description")?;
    let extras_json: serde_json::Value = row.try_get("extras")?;

    let viewport = viewport_packing::decode(&viewport_bytes)
        .ok_or(DbCodecError::MalformedBytes("viewport"))?;

    let mut state = ChartState::new(
        0, // ChartId in memory; the DB UUID is the truth
        Symbol {
            canonical: symbol_canonical.into(),
            asset_class: asset_class_from_i16(asset_class),
            provider_hints: ProviderHints::default(),
        },
        timeframe_from_i16(timeframe),
    );
    state.viewport = viewport;
    state.theme = theme_from_i16(theme);
    state.title = title.map(Into::into);
    state.description = description.map(Into::into);
    state.unknown_extensions = serde_json::from_value(extras_json).unwrap_or_default();

    // Styles
    let style_rows = sqlx::query(
        "SELECT style_id, stroke, width_x100, dash, fill FROM chart_styles \
         WHERE chart_id = $1 ORDER BY style_id",
    )
    .bind(chart_id)
    .fetch_all(pool)
    .await?;
    for r in style_rows {
        let _id: i32 = r.try_get("style_id")?;
        let style = Style {
            stroke: r.try_get::<i32, _>("stroke")? as u32,
            width_x100: r.try_get::<i16, _>("width_x100")? as u16,
            dash: dash_from_i16(r.try_get("dash")?),
            fill: r.try_get::<i32, _>("fill")? as u32,
        };
        state.style_table.intern(style);
    }

    // Drawings
    let drawing_rows = sqlx::query(
        "SELECT kind, z, flags, style_id, points, extras FROM drawings WHERE chart_id = $1",
    )
    .bind(chart_id)
    .fetch_all(pool)
    .await?;
    for r in drawing_rows {
        let kind_i: i16 = r.try_get("kind")?;
        let kind = drawing_kind_from_i16(kind_i)
            .ok_or(DbCodecError::UnknownDrawingKind(kind_i))?;
        let flags = DrawingFlags::from_bits_truncate(r.try_get::<i16, _>("flags")? as u16);
        let style_id = super::super::style_table::StyleId(r.try_get::<i32, _>("style_id")? as u32);
        let points_bytes: Vec<u8> = r.try_get("points")?;
        let pts = points_packing::decode(&points_bytes)
            .ok_or(DbCodecError::MalformedBytes("drawings.points"))?;
        let extras_json: serde_json::Value = r.try_get("extras")?;
        let drawing = Drawing {
            kind,
            points: pts.into_iter().collect(),
            style: style_id,
            flags,
            z: r.try_get("z")?,
            extras: serde_json::from_value(extras_json).unwrap_or_default(),
        };
        state.drawings.insert(drawing);
    }

    // Annotations
    let annotation_rows = sqlx::query(
        "SELECT anchor_ts_ns, anchor_price, title, body_md, color, asset_refs, extras \
         FROM chart_annotations WHERE chart_id = $1",
    )
    .bind(chart_id)
    .fetch_all(pool)
    .await?;
    for r in annotation_rows {
        let anchor = Point {
            ts_ns: r.try_get("anchor_ts_ns")?,
            price: r.try_get("anchor_price")?,
        };
        let asset_refs_vec: Vec<String> = r.try_get("asset_refs").unwrap_or_default();
        let extras_json: serde_json::Value = r.try_get("extras")?;
        let ann = Annotation {
            anchor,
            title: r.try_get::<String, _>("title")?.into(),
            body_md: r.try_get::<String, _>("body_md")?.into(),
            color: r.try_get::<i32, _>("color")? as u32,
            asset_refs: asset_refs_vec.into_iter().map(Into::into).collect(),
            extras: serde_json::from_value(extras_json).unwrap_or_default(),
        };
        state.annotations.insert(ann);
    }

    // Indicators
    let ind_rows = sqlx::query(
        "SELECT ref_id, ref_version, param_schema_version, pane, params, style, installed_locally, extras \
         FROM indicator_refs WHERE chart_id = $1",
    )
    .bind(chart_id)
    .fetch_all(pool)
    .await?;
    for r in ind_rows {
        let ind = IndicatorRef {
            ref_id: r.try_get::<String, _>("ref_id")?.into(),
            ref_version: r.try_get::<i32, _>("ref_version")? as u32,
            param_schema_version: r.try_get::<i32, _>("param_schema_version")? as u32,
            pane: r.try_get::<String, _>("pane")?.into(),
            params: r.try_get("params")?,
            style: r.try_get("style")?,
            installed_locally: r.try_get("installed_locally")?,
            extras: serde_json::from_value(r.try_get("extras")?).unwrap_or_default(),
        };
        state.indicators.push(ind);
    }

    Ok(state)
}

#[cfg(test)]
mod tests {
    use super::points_packing::{decode, encode};
    use super::viewport_packing;
    use super::{Point, Viewport};

    #[test]
    fn v1_round_trip_preserves_2_point_trendline() {
        let pts = vec![
            Point { ts_ns: 1_700_000_000_000_000_000, price: 4520.5 },
            Point { ts_ns: 1_700_000_300_000_000_000, price: 4555.0 },
        ];
        let bytes = encode(&pts);
        // v1 layout: tag + count + 2×i64 + 2×f32 = 26 bytes
        assert_eq!(bytes.len(), 26);
        assert_eq!(bytes[0], 1, "should pick v1 for ≤4 points");
        let decoded = decode(&bytes).unwrap();
        assert_eq!(decoded, pts);
    }

    #[test]
    fn v2_round_trip_preserves_polyline() {
        let mut pts = Vec::new();
        let base = 1_700_000_000_000_000_000_i64;
        for i in 0..50 {
            pts.push(Point {
                ts_ns: base + (i as i64) * 60_000_000_000,
                price: 4500.0 + (i as f32) * 0.5,
            });
        }
        let bytes = encode(&pts);
        assert_eq!(bytes[0], 2, "should pick v2 for >4 points");
        let decoded = decode(&bytes).unwrap();
        assert_eq!(decoded, pts);
        // delta-varint should crush this hard vs raw 12 bytes/point
        assert!(bytes.len() < pts.len() * 12, "v2 should be smaller than v1 layout would be");
    }

    #[test]
    fn v2_handles_negative_deltas() {
        let pts = vec![
            Point { ts_ns: 1_000_000_000, price: 1.0 },
            Point { ts_ns: 500_000_000,   price: 2.0 }, // backwards
            Point { ts_ns: 1_500_000_000, price: 3.0 },
            Point { ts_ns: 2_000_000_000, price: 4.0 },
            Point { ts_ns: 2_500_000_000, price: 5.0 },
        ];
        let decoded = decode(&encode(&pts)).unwrap();
        assert_eq!(decoded, pts);
    }

    #[test]
    fn malformed_returns_none() {
        assert!(decode(&[]).is_none());
        assert!(decode(&[99]).is_none()); // bad tag
        assert!(decode(&[1, 5, 0, 0, 0, 0, 0, 0, 0]).is_none()); // truncated
    }

    #[test]
    fn viewport_round_trips() {
        let v = Viewport {
            from_ts_ns: 1_700_000_000_000_000_000,
            to_ts_ns: 1_700_000_900_000_000_000,
            price_low: 4500.0,
            price_high: 4600.0,
            log_scale: true,
        };
        let bytes = viewport_packing::encode(&v);
        assert_eq!(bytes.len(), 25);
        let decoded = viewport_packing::decode(&bytes).unwrap();
        assert_eq!(decoded, v);
    }
}
