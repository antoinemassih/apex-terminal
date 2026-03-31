//! Direct drawing persistence for the native GPU chart.
//! Bypasses Tauri command layer — calls PostgreSQL directly via a global pool.

use sqlx::postgres::PgPool;
use std::sync::OnceLock;

static DB_POOL: OnceLock<PgPool> = OnceLock::new();

/// Initialize the global pool (call once from lib.rs setup, after pool is created).
pub fn init(pool: PgPool) {
    let _ = DB_POOL.set(pool);
    eprintln!("[drawing-db] Global pool initialized");
}

/// Drawing as stored in PostgreSQL.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DbDrawing {
    pub id: String,
    pub symbol: String,
    pub timeframe: String,
    pub drawing_type: String,
    pub points: Vec<(f64, f64)>, // (bar_index_or_time, price)
    pub color: String,
    pub opacity: f32,
    pub line_style: String,
    pub thickness: f32,
    pub group_id: String,
}

/// Load all drawings for a symbol (blocking, for use from native chart thread).
pub fn load_symbol(symbol: &str) -> Vec<DbDrawing> {
    let Some(pool) = DB_POOL.get() else { return vec![]; };
    let sym = symbol.to_string();
    let _rt = match tokio::runtime::Handle::try_current() {
        Ok(h) => h,
        Err(_) => return vec![], // no async runtime available
    };

    // Use spawn_blocking to avoid blocking the async runtime
    let result = std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        rt.block_on(async {
            let rows = sqlx::query_as::<_, DrawingRow>(
                "SELECT id, symbol, timeframe, type as drawing_type, points, color, opacity, line_style, thickness, group_id FROM drawings WHERE symbol = $1 ORDER BY created_at"
            )
            .bind(&sym)
            .fetch_all(pool)
            .await;

            match rows {
                Ok(rows) => rows.into_iter().filter_map(|r| row_to_drawing(r)).collect(),
                Err(e) => { eprintln!("[drawing-db] load error: {e}"); vec![] }
            }
        })
    }).join().unwrap_or_default();

    result
}

/// Save a drawing (fire-and-forget from native chart thread).
pub fn save(drawing: &DbDrawing) {
    let Some(pool) = DB_POOL.get() else { return; };
    let d = drawing.clone();
    let pool = pool.clone();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        rt.block_on(async {
            let id = match uuid::Uuid::parse_str(&d.id) {
                Ok(u) => u,
                Err(_) => { eprintln!("[drawing-db] invalid UUID: {}", d.id); return; }
            };
            let points = serde_json::json!(d.points.iter().map(|(t, p)| {
                serde_json::json!({"time": t, "price": p})
            }).collect::<Vec<_>>());

            let _ = sqlx::query(
                "INSERT INTO drawings (id, symbol, timeframe, type, points, color, opacity, line_style, thickness, group_id, updated_at)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, NOW())
                 ON CONFLICT (id) DO UPDATE SET
                   points = EXCLUDED.points, color = EXCLUDED.color, opacity = EXCLUDED.opacity,
                   line_style = EXCLUDED.line_style, thickness = EXCLUDED.thickness,
                   group_id = EXCLUDED.group_id, updated_at = NOW()"
            )
            .bind(id).bind(&d.symbol).bind(&d.timeframe).bind(&d.drawing_type)
            .bind(&points).bind(&d.color).bind(d.opacity).bind(&d.line_style)
            .bind(d.thickness).bind(&d.group_id)
            .execute(&pool).await;
        });
    });
}

/// Remove a drawing by ID (fire-and-forget).
pub fn remove(id: &str) {
    let Some(pool) = DB_POOL.get() else { return; };
    let id = id.to_string();
    let pool = pool.clone();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        rt.block_on(async {
            if let Ok(uuid) = uuid::Uuid::parse_str(&id) {
                let _ = sqlx::query("DELETE FROM drawings WHERE id = $1").bind(uuid).execute(&pool).await;
            }
        });
    });
}

/// Load all drawing groups.
pub fn load_groups() -> Vec<(String, String, Option<String>)> {
    let Some(pool) = DB_POOL.get() else { return vec![]; };
    let result = std::thread::spawn({
        let pool = pool.clone();
        move || {
            let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
            rt.block_on(async {
                let rows = sqlx::query_as::<_, (String, String, Option<String>)>(
                    "SELECT id, name, color FROM drawing_groups ORDER BY name"
                ).fetch_all(&pool).await;
                rows.unwrap_or_default()
            })
        }
    }).join().unwrap_or_default();
    result
}

/// Save a drawing group.
pub fn save_group(id: &str, name: &str, color: Option<&str>) {
    let Some(pool) = DB_POOL.get() else { return; };
    let id = id.to_string();
    let name = name.to_string();
    let color = color.map(|s| s.to_string());
    let pool = pool.clone();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        rt.block_on(async {
            let _ = sqlx::query(
                "INSERT INTO drawing_groups (id, name, color, updated_at) VALUES ($1, $2, $3, NOW())
                 ON CONFLICT (id) DO UPDATE SET name = EXCLUDED.name, color = EXCLUDED.color, updated_at = NOW()"
            ).bind(&id).bind(&name).bind(&color).execute(&pool).await;
        });
    });
}

/// Remove a drawing group (moves orphaned drawings to default).
pub fn remove_group(id: &str) {
    let Some(pool) = DB_POOL.get() else { return; };
    if id == "default" { return; }
    let id = id.to_string();
    let pool = pool.clone();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        rt.block_on(async {
            let _ = sqlx::query("UPDATE drawings SET group_id = 'default' WHERE group_id = $1").bind(&id).execute(&pool).await;
            let _ = sqlx::query("DELETE FROM drawing_groups WHERE id = $1").bind(&id).execute(&pool).await;
        });
    });
}

// Internal row type
#[derive(sqlx::FromRow)]
struct DrawingRow {
    id: uuid::Uuid,
    symbol: String,
    timeframe: String,
    #[sqlx(rename = "type")]
    drawing_type: String,
    points: serde_json::Value,
    color: String,
    opacity: f32,
    line_style: String,
    thickness: f32,
    group_id: String,
}

fn row_to_drawing(r: DrawingRow) -> Option<DbDrawing> {
    let points: Vec<(f64, f64)> = if let Some(arr) = r.points.as_array() {
        arr.iter().filter_map(|p| {
            let time = p.get("time")?.as_f64()?;
            let price = p.get("price")?.as_f64()?;
            Some((time, price))
        }).collect()
    } else { vec![] };

    Some(DbDrawing {
        id: r.id.to_string(),
        symbol: r.symbol,
        timeframe: r.timeframe,
        drawing_type: r.drawing_type,
        points,
        color: r.color,
        opacity: r.opacity,
        line_style: r.line_style,
        thickness: r.thickness,
        group_id: r.group_id,
    })
}
