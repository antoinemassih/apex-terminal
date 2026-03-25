use serde::{Deserialize, Serialize};
use sqlx::postgres::PgPool;
use sqlx::FromRow;
use tauri::State;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Point {
    pub time: f64,
    pub price: f64,
}

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct DrawingRow {
    pub id: Uuid,
    pub symbol: String,
    pub timeframe: String,
    #[sqlx(rename = "type")]
    pub drawing_type: String,
    pub points: serde_json::Value,
    pub color: String,
    pub opacity: f32,
    pub line_style: String,
    pub thickness: f32,
    pub group_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Drawing {
    pub id: String,
    pub symbol: String,
    pub timeframe: String,
    #[serde(rename = "type")]
    pub drawing_type: String,
    pub points: Vec<Point>,
    pub color: String,
    pub opacity: f64,
    #[serde(rename = "lineStyle")]
    pub line_style: String,
    pub thickness: f64,
    #[serde(rename = "groupId", default = "default_group_id")]
    pub group_id: String,
}

fn default_group_id() -> String {
    "default".to_string()
}

impl From<DrawingRow> for Drawing {
    fn from(r: DrawingRow) -> Self {
        let points: Vec<Point> = serde_json::from_value(r.points).unwrap_or_default();
        Drawing {
            id: r.id.to_string(),
            symbol: r.symbol,
            timeframe: r.timeframe,
            drawing_type: r.drawing_type,
            points,
            color: r.color,
            opacity: r.opacity as f64,
            line_style: r.line_style,
            thickness: r.thickness as f64,
            group_id: r.group_id,
        }
    }
}

// --- Drawing Groups ---

#[derive(Debug, Serialize, Deserialize, FromRow, Clone)]
pub struct DrawingGroupRow {
    pub id: String,
    pub name: String,
    pub color: Option<String>,
    pub opacity: Option<f32>,
    pub line_style: Option<String>,
    pub thickness: Option<f32>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DrawingGroup {
    pub id: String,
    pub name: String,
    pub color: Option<String>,
    pub opacity: Option<f64>,
    #[serde(rename = "lineStyle")]
    pub line_style: Option<String>,
    pub thickness: Option<f64>,
}

impl From<DrawingGroupRow> for DrawingGroup {
    fn from(r: DrawingGroupRow) -> Self {
        DrawingGroup {
            id: r.id,
            name: r.name,
            color: r.color,
            opacity: r.opacity.map(|v| v as f64),
            line_style: r.line_style,
            thickness: r.thickness.map(|v| v as f64),
        }
    }
}

pub struct DbPool(pub PgPool);

/// Idempotent schema migration — safe to run on every startup.
pub async fn ensure_schema(pool: &PgPool) -> Result<(), sqlx::Error> {
    // Ensure drawing_groups table exists
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS drawing_groups (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            color TEXT,
            opacity FLOAT4,
            line_style TEXT,
            thickness FLOAT4,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )"
    )
    .execute(pool)
    .await?;

    // Ensure drawings table has group_id column (safe on fresh and existing tables)
    sqlx::query(
        "ALTER TABLE drawings ADD COLUMN IF NOT EXISTS group_id TEXT NOT NULL DEFAULT 'default'"
    )
    .execute(pool)
    .await?;

    // Seed default group
    sqlx::query(
        "INSERT INTO drawing_groups (id, name) VALUES ('default', 'Default') ON CONFLICT (id) DO NOTHING"
    )
    .execute(pool)
    .await?;

    Ok(())
}

// --- Drawing commands ---

#[tauri::command]
pub async fn drawings_load_all(pool: State<'_, DbPool>) -> Result<Vec<Drawing>, String> {
    let rows = sqlx::query_as::<_, DrawingRow>(
        "SELECT id, symbol, timeframe, type as drawing_type, points, color, opacity, line_style, thickness, group_id FROM drawings ORDER BY created_at"
    )
    .fetch_all(&pool.0)
    .await
    .map_err(|e| e.to_string())?;

    Ok(rows.into_iter().map(Drawing::from).collect())
}

#[tauri::command]
pub async fn drawings_load_symbol(pool: State<'_, DbPool>, symbol: String) -> Result<Vec<Drawing>, String> {
    let rows = sqlx::query_as::<_, DrawingRow>(
        "SELECT id, symbol, timeframe, type as drawing_type, points, color, opacity, line_style, thickness, group_id FROM drawings WHERE symbol = $1 ORDER BY created_at"
    )
    .bind(&symbol)
    .fetch_all(&pool.0)
    .await
    .map_err(|e| e.to_string())?;

    Ok(rows.into_iter().map(Drawing::from).collect())
}

#[tauri::command]
pub async fn drawings_save(pool: State<'_, DbPool>, drawing: Drawing) -> Result<(), String> {
    let id = Uuid::parse_str(&drawing.id).map_err(|e| e.to_string())?;
    let points = serde_json::to_value(&drawing.points).map_err(|e| e.to_string())?;

    sqlx::query(
        "INSERT INTO drawings (id, symbol, timeframe, type, points, color, opacity, line_style, thickness, group_id, updated_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, NOW())
         ON CONFLICT (id) DO UPDATE SET
           points = EXCLUDED.points,
           color = EXCLUDED.color,
           opacity = EXCLUDED.opacity,
           line_style = EXCLUDED.line_style,
           thickness = EXCLUDED.thickness,
           group_id = EXCLUDED.group_id,
           updated_at = NOW()"
    )
    .bind(id)
    .bind(&drawing.symbol)
    .bind(&drawing.timeframe)
    .bind(&drawing.drawing_type)
    .bind(&points)
    .bind(&drawing.color)
    .bind(drawing.opacity as f32)
    .bind(&drawing.line_style)
    .bind(drawing.thickness as f32)
    .bind(&drawing.group_id)
    .execute(&pool.0)
    .await
    .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn drawings_update_points(pool: State<'_, DbPool>, id: String, points: Vec<Point>) -> Result<(), String> {
    let uuid = Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    let pts = serde_json::to_value(&points).map_err(|e| e.to_string())?;

    sqlx::query("UPDATE drawings SET points = $1, updated_at = NOW() WHERE id = $2")
        .bind(&pts)
        .bind(uuid)
        .execute(&pool.0)
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn drawings_update_style(
    pool: State<'_, DbPool>,
    id: String,
    color: Option<String>,
    opacity: Option<f64>,
    line_style: Option<String>,
    thickness: Option<f64>,
) -> Result<(), String> {
    let uuid = Uuid::parse_str(&id).map_err(|e| e.to_string())?;

    // Build dynamic update
    let mut query = String::from("UPDATE drawings SET updated_at = NOW()");
    let mut param_idx = 1;
    if color.is_some()      { param_idx += 1; query += &format!(", color = ${param_idx}"); }
    if opacity.is_some()    { param_idx += 1; query += &format!(", opacity = ${param_idx}"); }
    if line_style.is_some() { param_idx += 1; query += &format!(", line_style = ${param_idx}"); }
    if thickness.is_some()  { param_idx += 1; query += &format!(", thickness = ${param_idx}"); }
    query += " WHERE id = $1";

    let mut q = sqlx::query(&query).bind(uuid);
    if let Some(ref c) = color      { q = q.bind(c); }
    if let Some(o) = opacity        { q = q.bind(o as f32); }
    if let Some(ref ls) = line_style { q = q.bind(ls); }
    if let Some(t) = thickness      { q = q.bind(t as f32); }

    q.execute(&pool.0).await.map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn drawings_remove(pool: State<'_, DbPool>, id: String) -> Result<(), String> {
    let uuid = Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM drawings WHERE id = $1")
        .bind(uuid)
        .execute(&pool.0)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn drawings_clear(pool: State<'_, DbPool>) -> Result<(), String> {
    sqlx::query("DELETE FROM drawings")
        .execute(&pool.0)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

// --- Group commands ---

#[tauri::command]
pub async fn groups_load_all(pool: State<'_, DbPool>) -> Result<Vec<DrawingGroup>, String> {
    let rows = sqlx::query_as::<_, DrawingGroupRow>(
        "SELECT id, name, color, opacity, line_style, thickness FROM drawing_groups ORDER BY name"
    )
    .fetch_all(&pool.0)
    .await
    .map_err(|e| e.to_string())?;

    Ok(rows.into_iter().map(DrawingGroup::from).collect())
}

#[tauri::command]
pub async fn groups_save(pool: State<'_, DbPool>, group: DrawingGroup) -> Result<(), String> {
    sqlx::query(
        "INSERT INTO drawing_groups (id, name, color, opacity, line_style, thickness, updated_at)
         VALUES ($1, $2, $3, $4, $5, $6, NOW())
         ON CONFLICT (id) DO UPDATE SET
           name = EXCLUDED.name,
           color = EXCLUDED.color,
           opacity = EXCLUDED.opacity,
           line_style = EXCLUDED.line_style,
           thickness = EXCLUDED.thickness,
           updated_at = NOW()"
    )
    .bind(&group.id)
    .bind(&group.name)
    .bind(&group.color)
    .bind(group.opacity.map(|v| v as f32))
    .bind(&group.line_style)
    .bind(group.thickness.map(|v| v as f32))
    .execute(&pool.0)
    .await
    .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn groups_remove(pool: State<'_, DbPool>, id: String) -> Result<(), String> {
    if id == "default" { return Ok(()) }

    // Move any orphaned drawings back to default
    sqlx::query("UPDATE drawings SET group_id = 'default' WHERE group_id = $1")
        .bind(&id)
        .execute(&pool.0)
        .await
        .map_err(|e| e.to_string())?;

    sqlx::query("DELETE FROM drawing_groups WHERE id = $1 AND id != 'default'")
        .bind(&id)
        .execute(&pool.0)
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn groups_update_style(
    pool: State<'_, DbPool>,
    id: String,
    color: Option<String>,
    opacity: Option<f64>,
    line_style: Option<String>,
    thickness: Option<f64>,
) -> Result<(), String> {
    let mut query = String::from("UPDATE drawing_groups SET updated_at = NOW()");
    let mut param_idx = 1;
    if color.is_some()      { param_idx += 1; query += &format!(", color = ${param_idx}"); }
    if opacity.is_some()    { param_idx += 1; query += &format!(", opacity = ${param_idx}"); }
    if line_style.is_some() { param_idx += 1; query += &format!(", line_style = ${param_idx}"); }
    if thickness.is_some()  { param_idx += 1; query += &format!(", thickness = ${param_idx}"); }
    query += " WHERE id = $1";

    let mut q = sqlx::query(&query).bind(&id);
    if let Some(ref c) = color       { q = q.bind(c); }
    if let Some(o) = opacity         { q = q.bind(o as f32); }
    if let Some(ref ls) = line_style  { q = q.bind(ls); }
    if let Some(t) = thickness       { q = q.bind(t as f32); }

    q.execute(&pool.0).await.map_err(|e| e.to_string())?;
    Ok(())
}

/// Batch apply style to all drawings in a group + update the group record.
/// Single operation — avoids N concurrent IPC calls that cause UI freezes.
#[tauri::command]
pub async fn drawings_apply_group_style(
    pool: State<'_, DbPool>,
    group_id: String,
    color: Option<String>,
    opacity: Option<f64>,
    line_style: Option<String>,
    thickness: Option<f64>,
) -> Result<(), String> {
    // Update all drawings in the group
    let mut dq = String::from("UPDATE drawings SET updated_at = NOW()");
    let mut pi = 1usize;
    if color.is_some()      { pi += 1; dq += &format!(", color = ${pi}"); }
    if opacity.is_some()    { pi += 1; dq += &format!(", opacity = ${pi}"); }
    if line_style.is_some() { pi += 1; dq += &format!(", line_style = ${pi}"); }
    if thickness.is_some()  { pi += 1; dq += &format!(", thickness = ${pi}"); }
    dq += " WHERE group_id = $1";

    let mut dq2 = sqlx::query(&dq).bind(&group_id);
    if let Some(ref c) = color       { dq2 = dq2.bind(c); }
    if let Some(o) = opacity         { dq2 = dq2.bind(o as f32); }
    if let Some(ref ls) = line_style { dq2 = dq2.bind(ls); }
    if let Some(t) = thickness       { dq2 = dq2.bind(t as f32); }
    dq2.execute(&pool.0).await.map_err(|e| e.to_string())?;

    // Update the group style record
    let mut gq_str = String::from("UPDATE drawing_groups SET updated_at = NOW()");
    let mut gpi = 1usize;
    if color.is_some()      { gpi += 1; gq_str += &format!(", color = ${gpi}"); }
    if opacity.is_some()    { gpi += 1; gq_str += &format!(", opacity = ${gpi}"); }
    if line_style.is_some() { gpi += 1; gq_str += &format!(", line_style = ${gpi}"); }
    if thickness.is_some()  { gpi += 1; gq_str += &format!(", thickness = ${gpi}"); }
    gq_str += " WHERE id = $1";

    let mut gq = sqlx::query(&gq_str).bind(&group_id);
    if let Some(ref c) = color       { gq = gq.bind(c); }
    if let Some(o) = opacity         { gq = gq.bind(o as f32); }
    if let Some(ref ls) = line_style { gq = gq.bind(ls); }
    if let Some(t) = thickness       { gq = gq.bind(t as f32); }
    gq.execute(&pool.0).await.map_err(|e| e.to_string())?;

    Ok(())
}
