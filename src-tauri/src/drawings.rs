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
        }
    }
}

pub struct DbPool(pub PgPool);

#[tauri::command]
pub async fn drawings_load_all(pool: State<'_, DbPool>) -> Result<Vec<Drawing>, String> {
    let rows = sqlx::query_as::<_, DrawingRow>(
        "SELECT id, symbol, timeframe, type as drawing_type, points, color, opacity, line_style, thickness FROM drawings ORDER BY created_at"
    )
    .fetch_all(&pool.0)
    .await
    .map_err(|e| e.to_string())?;

    Ok(rows.into_iter().map(Drawing::from).collect())
}

#[tauri::command]
pub async fn drawings_load_symbol(pool: State<'_, DbPool>, symbol: String) -> Result<Vec<Drawing>, String> {
    let rows = sqlx::query_as::<_, DrawingRow>(
        "SELECT id, symbol, timeframe, type as drawing_type, points, color, opacity, line_style, thickness FROM drawings WHERE symbol = $1 ORDER BY created_at"
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
        "INSERT INTO drawings (id, symbol, timeframe, type, points, color, opacity, line_style, thickness, updated_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, NOW())
         ON CONFLICT (id) DO UPDATE SET
           points = EXCLUDED.points,
           color = EXCLUDED.color,
           opacity = EXCLUDED.opacity,
           line_style = EXCLUDED.line_style,
           thickness = EXCLUDED.thickness,
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
    if color.is_some() { param_idx += 1; query += &format!(", color = ${param_idx}"); }
    if opacity.is_some() { param_idx += 1; query += &format!(", opacity = ${param_idx}"); }
    if line_style.is_some() { param_idx += 1; query += &format!(", line_style = ${param_idx}"); }
    if thickness.is_some() { param_idx += 1; query += &format!(", thickness = ${param_idx}"); }
    query += " WHERE id = $1";

    let mut q = sqlx::query(&query).bind(uuid);
    if let Some(ref c) = color { q = q.bind(c); }
    if let Some(o) = opacity { q = q.bind(o as f32); }
    if let Some(ref ls) = line_style { q = q.bind(ls); }
    if let Some(t) = thickness { q = q.bind(t as f32); }

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
