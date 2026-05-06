//! Drawing persistence for the native GPU chart.
//!
//! Same public API as before (`init`, `load_symbol`, `save`, `remove`,
//! `load_groups`, `save_group`, `remove_group`, `get_pool`) — but backed by
//! the new normalized `chart_state` schema. The renderer's hot path is
//! unchanged: callers still receive `Vec<DbDrawing>` and still pay zero DB
//! cost on the render thread.
//!
//! Architecture, unchanged from before:
//!   - One persistent worker thread owns the tokio runtime and PgPool.
//!   - All DB ops are mpsc messages to that thread.
//!   - Saves/removes are fire-and-forget. Loads are called only from
//!     background threads, never the render thread.

use sqlx::postgres::PgPool;
use sqlx::Row;
use std::sync::OnceLock;
use uuid::Uuid;

use crate::chart::state::codec::db::points_packing;
use crate::chart::state::drawings::{DrawingFlags, DrawingKind, Point};

static DB_POOL: OnceLock<PgPool> = OnceLock::new();

/// Messages for the DB worker thread.
enum DbOp {
    Save(DbDrawing),
    Remove(String),
    LoadSymbol { symbol: String, reply: std::sync::mpsc::Sender<Vec<DbDrawing>> },
    LoadGroups { reply: std::sync::mpsc::Sender<Vec<(String, String, Option<String>)>> },
    SaveGroup { id: String, name: String, color: Option<String> },
    RemoveGroup(String),
}

static DB_TX: OnceLock<std::sync::mpsc::Sender<DbOp>> = OnceLock::new();

/// Initialize the global pool and start the DB worker thread.
pub fn init(pool: PgPool) {
    let _ = DB_POOL.set(pool.clone());

    let (tx, rx) = std::sync::mpsc::channel::<DbOp>();
    let _ = DB_TX.set(tx);

    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        rt.block_on(async {
            while let Ok(op) = rx.recv() {
                match op {
                    DbOp::Save(d) => { do_save(&pool, d).await; }
                    DbOp::Remove(id) => { do_remove(&pool, &id).await; }
                    DbOp::LoadSymbol { symbol, reply } => {
                        let result = do_load_symbol(&pool, &symbol).await;
                        let _ = reply.send(result);
                    }
                    DbOp::LoadGroups { reply } => {
                        let result = do_load_groups(&pool).await;
                        let _ = reply.send(result);
                    }
                    DbOp::SaveGroup { id, name, color } => {
                        do_save_group(&pool, &id, &name, color.as_deref()).await;
                    }
                    DbOp::RemoveGroup(id) => { do_remove_group(&pool, &id).await; }
                }
            }
        });
    });

    eprintln!("[drawing-db] Worker started against new chart_state schema");
}

/// Get a reference to the pool (for direct queries from background threads).
pub fn get_pool() -> Option<&'static PgPool> {
    DB_POOL.get()
}

/// Drawing as the caller (renderer) sees it. Wire-compatible with the prior
/// version of this module — same field names and types.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DbDrawing {
    pub id: String,
    pub symbol: String,
    pub timeframe: String,
    pub drawing_type: String,
    pub points: Vec<(f64, f64)>, // (time_seconds, price)
    pub color: String,           // "#RRGGBB"
    pub opacity: f32,
    pub line_style: String,
    pub thickness: f32,
    pub group_id: String,
}

/// Load all drawings for a symbol (blocking — sends to worker, waits for reply).
pub fn load_symbol(symbol: &str) -> Vec<DbDrawing> {
    let Some(tx) = DB_TX.get() else { return vec![]; };
    let (reply_tx, reply_rx) = std::sync::mpsc::channel();
    let _ = tx.send(DbOp::LoadSymbol { symbol: symbol.into(), reply: reply_tx });
    reply_rx.recv_timeout(std::time::Duration::from_secs(5)).unwrap_or_default()
}

/// Save a drawing (fire-and-forget).
pub fn save(drawing: &DbDrawing) {
    if let Some(tx) = DB_TX.get() { let _ = tx.send(DbOp::Save(drawing.clone())); }
}

/// Remove a drawing by ID (fire-and-forget).
pub fn remove(id: &str) {
    if let Some(tx) = DB_TX.get() { let _ = tx.send(DbOp::Remove(id.into())); }
}

/// Load all drawing groups (blocking).
pub fn load_groups() -> Vec<(String, String, Option<String>)> {
    let Some(tx) = DB_TX.get() else { return vec![]; };
    let (reply_tx, reply_rx) = std::sync::mpsc::channel();
    let _ = tx.send(DbOp::LoadGroups { reply: reply_tx });
    reply_rx.recv_timeout(std::time::Duration::from_secs(5)).unwrap_or_default()
}

/// Save a drawing group (fire-and-forget).
pub fn save_group(id: &str, name: &str, color: Option<&str>) {
    if let Some(tx) = DB_TX.get() {
        let _ = tx.send(DbOp::SaveGroup {
            id: id.into(), name: name.into(), color: color.map(|s| s.into()),
        });
    }
}

/// Remove a drawing group (fire-and-forget).
pub fn remove_group(id: &str) {
    if id == "default" { return; }
    if let Some(tx) = DB_TX.get() { let _ = tx.send(DbOp::RemoveGroup(id.into())); }
}

// ─────────────────────────────────────────────────────────────────────────
// Translation helpers
// ─────────────────────────────────────────────────────────────────────────

fn parse_kind(s: &str) -> Option<DrawingKind> {
    let n = s.to_ascii_lowercase().replace('-', "_");
    Some(match n.as_str() {
        "trendline" | "trend_line" | "line" => DrawingKind::Trendline,
        "horizontal" | "horizontal_line" | "hline" => DrawingKind::HorizontalLine,
        "vertical" | "vertical_line" | "vline" => DrawingKind::VerticalLine,
        "ray" => DrawingKind::Ray,
        "rect" | "rectangle" | "box" => DrawingKind::Rect,
        "ellipse" | "circle" => DrawingKind::Ellipse,
        "fib_retracement" | "fib" | "fibonacci" => DrawingKind::FibRetracement,
        "fib_extension" => DrawingKind::FibExtension,
        "pitchfork" => DrawingKind::Pitchfork,
        "text" | "label" | "note" => DrawingKind::Text,
        "arrow" => DrawingKind::Arrow,
        "polyline" => DrawingKind::Polyline,
        "path" | "freehand" | "brush" => DrawingKind::Path,
        _ => return None,
    })
}

fn kind_to_str(k: DrawingKind) -> &'static str {
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

fn kind_to_i16(k: DrawingKind) -> i16 {
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

fn kind_from_i16(v: i16) -> Option<DrawingKind> {
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

fn dash_str_to_i16(s: &str) -> i16 {
    match s {
        "dashed" | "dash" => 1,
        "dotted" | "dot" => 2,
        _ => 0,
    }
}

fn dash_i16_to_str(v: i16) -> &'static str {
    match v {
        1 => "dashed",
        2 => "dotted",
        _ => "solid",
    }
}

/// Parse `#RRGGBB` (or `#RRGGBBAA`) into a 24-bit RGB; alpha stripped.
fn parse_rgb(s: &str) -> u32 {
    let hex = s.trim().trim_start_matches('#');
    if hex.len() != 6 && hex.len() != 8 { return 0xCCCCCC; }
    u32::from_str_radix(&hex[..6], 16).unwrap_or(0xCCCCCC)
}

fn rgb_to_hex(rgb: u32) -> String {
    format!("#{:06X}", rgb & 0xFFFFFF)
}

/// Pack DbDrawing's `(f64 seconds, f64 price)` points into the canonical
/// `(i64 ns, f32 price)` packed format used by the new schema.
fn encode_points(pts: &[(f64, f64)]) -> Vec<u8> {
    let canonical: Vec<Point> = pts
        .iter()
        .map(|(t, p)| Point {
            ts_ns: (*t * 1_000_000_000.0) as i64,
            price: *p as f32,
        })
        .collect();
    points_packing::encode(&canonical)
}

fn decode_points(buf: &[u8]) -> Vec<(f64, f64)> {
    points_packing::decode(buf)
        .unwrap_or_default()
        .into_iter()
        .map(|p| (p.ts_ns as f64 / 1_000_000_000.0, p.price as f64))
        .collect()
}

/// Find or create a chart row for (user_id=0, symbol_canonical=`symbol`).
/// Returns the chart's UUID. Caches via the worker's serial execution; safe
/// without a transaction because only this thread writes.
async fn find_or_create_chart(pool: &PgPool, symbol: &str) -> Result<Uuid, sqlx::Error> {
    if let Some(row) = sqlx::query("SELECT id FROM charts WHERE user_id = 0 AND symbol_canonical = $1 LIMIT 1")
        .bind(symbol)
        .fetch_optional(pool)
        .await?
    {
        return row.try_get::<Uuid, _>("id");
    }

    // Default viewport bytes (25 zeros) — replaced on first real save through
    // the canonical path. Zero bytes decode to an all-zero Viewport.
    let viewport_bytes: Vec<u8> = vec![0u8; 25];

    sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO charts (user_id, symbol_canonical, asset_class, timeframe, theme, viewport, schema_version) \
         VALUES (0, $1, 0, 0, 0, $2, 1) RETURNING id",
    )
    .bind(symbol)
    .bind(viewport_bytes)
    .fetch_one(pool)
    .await
}

/// Find or create a style row matching the given fields for this chart.
/// Returns the per-chart `style_id` (i32).
async fn intern_style(
    pool: &PgPool,
    chart_id: Uuid,
    stroke_rgba: i32,
    width_x100: i16,
    dash: i16,
    fill: i32,
) -> Result<i32, sqlx::Error> {
    if let Some(row) = sqlx::query(
        "SELECT style_id FROM chart_styles \
         WHERE chart_id = $1 AND stroke = $2 AND width_x100 = $3 AND dash = $4 AND fill = $5 LIMIT 1",
    )
    .bind(chart_id)
    .bind(stroke_rgba)
    .bind(width_x100)
    .bind(dash)
    .bind(fill)
    .fetch_optional(pool)
    .await?
    {
        return row.try_get::<i32, _>("style_id");
    }

    let next_id: i32 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(style_id) + 1, 0) FROM chart_styles WHERE chart_id = $1",
    )
    .bind(chart_id)
    .fetch_one(pool)
    .await?;

    sqlx::query(
        "INSERT INTO chart_styles (chart_id, style_id, stroke, width_x100, dash, fill) \
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(chart_id)
    .bind(next_id)
    .bind(stroke_rgba)
    .bind(width_x100)
    .bind(dash)
    .bind(fill)
    .execute(pool)
    .await?;

    Ok(next_id)
}

// ─────────────────────────────────────────────────────────────────────────
// Worker implementations
// ─────────────────────────────────────────────────────────────────────────

async fn do_load_symbol(pool: &PgPool, symbol: &str) -> Vec<DbDrawing> {
    let result = sqlx::query(
        "SELECT d.id, d.kind, d.flags, d.points, d.extras, \
                cs.stroke, cs.width_x100, cs.dash \
         FROM drawings d \
         JOIN charts c ON d.chart_id = c.id \
         JOIN chart_styles cs ON cs.chart_id = c.id AND cs.style_id = d.style_id \
         WHERE c.user_id = 0 AND c.symbol_canonical = $1",
    )
    .bind(symbol)
    .fetch_all(pool)
    .await;

    let rows = match result {
        Ok(r) => r,
        Err(e) => { eprintln!("[drawing-db] load error: {e}"); return vec![]; }
    };

    let mut drawings = Vec::with_capacity(rows.len());
    for r in rows {
        let id: Uuid = match r.try_get("id") { Ok(v) => v, Err(_) => continue };
        let kind_i: i16 = r.try_get("kind").unwrap_or(0);
        let kind = match kind_from_i16(kind_i) { Some(k) => k, None => continue };
        let flags_i: i16 = r.try_get("flags").unwrap_or(0);
        let _flags = DrawingFlags::from_bits_truncate(flags_i as u16);
        let points_bytes: Vec<u8> = r.try_get("points").unwrap_or_default();
        let extras: serde_json::Value = r.try_get("extras").unwrap_or(serde_json::json!({}));
        let stroke_rgba: i32 = r.try_get("stroke").unwrap_or(0xCCCCCCFFu32 as i32);
        let width_x100: i16 = r.try_get("width_x100").unwrap_or(100);
        let dash_i: i16 = r.try_get("dash").unwrap_or(0);

        let stroke_u = stroke_rgba as u32;
        let rgb = stroke_u >> 8;
        let alpha = stroke_u & 0xFF;
        let opacity = alpha as f32 / 255.0;

        let timeframe = extras.get("timeframe").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let group_id = extras.get("group_id").and_then(|v| v.as_str()).unwrap_or("default").to_string();

        drawings.push(DbDrawing {
            id: id.to_string(),
            symbol: symbol.to_string(),
            timeframe,
            drawing_type: kind_to_str(kind).to_string(),
            points: decode_points(&points_bytes),
            color: rgb_to_hex(rgb),
            opacity,
            line_style: dash_i16_to_str(dash_i).to_string(),
            thickness: width_x100 as f32 / 100.0,
            group_id,
        });
    }
    eprintln!("[drawing-db] loaded {} drawings for {}", drawings.len(), symbol);
    drawings
}

async fn do_save(pool: &PgPool, d: DbDrawing) {
    let id = match Uuid::parse_str(&d.id) {
        Ok(u) => u,
        Err(_) => { eprintln!("[drawing-db] invalid UUID: {}", d.id); return; }
    };
    let kind = match parse_kind(&d.drawing_type) {
        Some(k) => k,
        None => { eprintln!("[drawing-db] unknown kind: {}", d.drawing_type); return; }
    };

    let chart_id = match find_or_create_chart(pool, &d.symbol).await {
        Ok(c) => c,
        Err(e) => { eprintln!("[drawing-db] chart upsert: {e}"); return; }
    };

    let rgb = parse_rgb(&d.color);
    let alpha = (d.opacity.clamp(0.0, 1.0) * 255.0).round() as u32 & 0xFF;
    let stroke = ((rgb << 8) | alpha) as i32;
    let width_x100 = (d.thickness * 100.0).round().clamp(0.0, i16::MAX as f32) as i16;
    let dash = dash_str_to_i16(&d.line_style);

    let style_id = match intern_style(pool, chart_id, stroke, width_x100, dash, 0).await {
        Ok(s) => s,
        Err(e) => { eprintln!("[drawing-db] style intern: {e}"); return; }
    };

    let mut extras = serde_json::Map::new();
    if !d.timeframe.is_empty() {
        extras.insert("timeframe".into(), serde_json::Value::String(d.timeframe.clone()));
    }
    if !d.group_id.is_empty() && d.group_id != "default" {
        extras.insert("group_id".into(), serde_json::Value::String(d.group_id.clone()));
    }
    let extras_json = serde_json::Value::Object(extras);

    let flags = DrawingFlags::VISIBLE.bits() as i16;
    let points_bytes = encode_points(&d.points);

    let result = sqlx::query(
        "INSERT INTO drawings (id, chart_id, kind, z, flags, style_id, points, extras) \
         VALUES ($1, $2, $3, 0, $4, $5, $6, $7) \
         ON CONFLICT (id) DO UPDATE SET \
           chart_id = EXCLUDED.chart_id, \
           kind     = EXCLUDED.kind, \
           flags    = EXCLUDED.flags, \
           style_id = EXCLUDED.style_id, \
           points   = EXCLUDED.points, \
           extras   = EXCLUDED.extras",
    )
    .bind(id)
    .bind(chart_id)
    .bind(kind_to_i16(kind))
    .bind(flags)
    .bind(style_id)
    .bind(&points_bytes)
    .bind(&extras_json)
    .execute(pool)
    .await;

    match result {
        Ok(_) => eprintln!("[drawing-db] saved {} {} {}", d.drawing_type, d.symbol, d.id),
        Err(e) => eprintln!("[drawing-db] save error: {e}"),
    }
}

async fn do_remove(pool: &PgPool, id: &str) {
    let Ok(uuid) = Uuid::parse_str(id) else { return };
    let _ = sqlx::query("DELETE FROM drawings WHERE id = $1")
        .bind(uuid)
        .execute(pool)
        .await;
}

async fn do_load_groups(pool: &PgPool) -> Vec<(String, String, Option<String>)> {
    sqlx::query_as::<_, (String, String, Option<String>)>(
        "SELECT id, name, color FROM chart_drawing_groups ORDER BY name",
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default()
}

async fn do_save_group(pool: &PgPool, id: &str, name: &str, color: Option<&str>) {
    let _ = sqlx::query(
        "INSERT INTO chart_drawing_groups (id, name, color, updated_at) \
         VALUES ($1, $2, $3, NOW()) \
         ON CONFLICT (id) DO UPDATE SET name = EXCLUDED.name, color = EXCLUDED.color, updated_at = NOW()",
    )
    .bind(id)
    .bind(name)
    .bind(color)
    .execute(pool)
    .await;
}

async fn do_remove_group(pool: &PgPool, id: &str) {
    // Best-effort: rewrite drawings.extras to drop group_id pointing at this group.
    let _ = sqlx::query(
        "UPDATE drawings SET extras = extras - 'group_id' \
         WHERE extras ->> 'group_id' = $1",
    )
    .bind(id)
    .execute(pool)
    .await;
    let _ = sqlx::query("DELETE FROM chart_drawing_groups WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await;
}
