//! Direct drawing persistence for the native GPU chart.
//! Bypasses Tauri command layer — calls PostgreSQL directly via a global pool.
//!
//! Architecture: A single persistent background thread owns the tokio runtime
//! and PgPool. All DB operations are sent as messages to this thread.
//! This avoids per-call thread/runtime spawning that exhausts connections.

use sqlx::postgres::PgPool;
use std::sync::OnceLock;

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
                    DbOp::SaveGroup { id, name, color } => { do_save_group(&pool, &id, &name, color.as_deref()).await; }
                    DbOp::RemoveGroup(id) => { do_remove_group(&pool, &id).await; }
                }
            }
        });
    });

    eprintln!("[drawing-db] Global pool initialized");
}

/// Get a reference to the pool (for direct queries from background threads).
pub fn get_pool() -> Option<&'static PgPool> {
    DB_POOL.get()
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
        let _ = tx.send(DbOp::SaveGroup { id: id.into(), name: name.into(), color: color.map(|s| s.into()) });
    }
}

/// Remove a drawing group (fire-and-forget).
pub fn remove_group(id: &str) {
    if id == "default" { return; }
    if let Some(tx) = DB_TX.get() { let _ = tx.send(DbOp::RemoveGroup(id.into())); }
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

// ─── Worker implementations (run on the persistent tokio runtime) ────────────

async fn do_load_symbol(pool: &PgPool, symbol: &str) -> Vec<DbDrawing> {
    match sqlx::query_as::<_, DrawingRow>(
        "SELECT id, symbol, timeframe, type, points, color, opacity, line_style, thickness, group_id FROM drawings WHERE symbol = $1 ORDER BY created_at"
    ).bind(symbol).fetch_all(pool).await {
        Ok(rows) => {
            let drawings: Vec<DbDrawing> = rows.into_iter().filter_map(|r| row_to_drawing(r)).collect();
            eprintln!("[drawing-db] loaded {} drawings for {}", drawings.len(), symbol);
            drawings
        }
        Err(e) => { eprintln!("[drawing-db] load error: {e}"); vec![] }
    }
}

async fn do_save(pool: &PgPool, d: DbDrawing) {
    let id = match uuid::Uuid::parse_str(&d.id) {
        Ok(u) => u,
        Err(_) => { eprintln!("[drawing-db] invalid UUID: {}", d.id); return; }
    };
    let points = serde_json::json!(d.points.iter().map(|(t, p)| {
        serde_json::json!({"time": t, "price": p})
    }).collect::<Vec<_>>());

    match sqlx::query(
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
    .execute(pool).await {
        Ok(_) => eprintln!("[drawing-db] saved {} {} {}", d.drawing_type, d.symbol, d.id),
        Err(e) => eprintln!("[drawing-db] save error: {e}"),
    }
}

async fn do_remove(pool: &PgPool, id: &str) {
    if let Ok(uuid) = uuid::Uuid::parse_str(id) {
        let _ = sqlx::query("DELETE FROM drawings WHERE id = $1").bind(uuid).execute(pool).await;
    }
}

async fn do_load_groups(pool: &PgPool) -> Vec<(String, String, Option<String>)> {
    sqlx::query_as::<_, (String, String, Option<String>)>(
        "SELECT id, name, color FROM drawing_groups ORDER BY name"
    ).fetch_all(pool).await.unwrap_or_default()
}

async fn do_save_group(pool: &PgPool, id: &str, name: &str, color: Option<&str>) {
    let _ = sqlx::query(
        "INSERT INTO drawing_groups (id, name, color, updated_at) VALUES ($1, $2, $3, NOW())
         ON CONFLICT (id) DO UPDATE SET name = EXCLUDED.name, color = EXCLUDED.color, updated_at = NOW()"
    ).bind(id).bind(name).bind(color).execute(pool).await;
}

async fn do_remove_group(pool: &PgPool, id: &str) {
    let _ = sqlx::query("UPDATE drawings SET group_id = 'default' WHERE group_id = $1").bind(id).execute(pool).await;
    let _ = sqlx::query("DELETE FROM drawing_groups WHERE id = $1").bind(id).execute(pool).await;
}
