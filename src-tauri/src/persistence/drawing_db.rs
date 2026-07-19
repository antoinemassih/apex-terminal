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
use tracing::{debug, warn, error};

use crate::chart::state::codec::db::points_packing;
use crate::chart::state::drawings::{DrawingFlags, DrawingKind, Point};

static DB_POOL: OnceLock<PgPool> = OnceLock::new();

/// W1-02b: whether a live pool is currently attached. This — not the existence
/// of the worker channel — is what "drawings are being saved" means, because
/// the worker now runs even while Postgres is down (so saves can be buffered
/// instead of dropped on the floor at the call site).
static PG_CONNECTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// How many times a failed save is re-queued before it is moved to the
/// bounded dead-letter list.
const SAVE_MAX_ATTEMPTS: u32 = 3;

/// W1-02b: how many un-persisted saves are held in memory while PG is down.
/// Every buffered save is ALSO spilled to JSONL, so this cap bounds memory
/// without losing data — the spill is the durable copy.
const PENDING_CAP: usize = 512;

/// Messages for the DB worker thread.
enum DbOp {
    /// `attempts` starts at 1; the worker increments it on each retry.
    Save { drawing: DbDrawing, attempts: u32 },
    Remove(String),
    LoadSymbol { symbol: String, reply: std::sync::mpsc::Sender<Vec<DbDrawing>> },
    LoadGroups { reply: std::sync::mpsc::Sender<Vec<(String, String, Option<String>)>> },
    SaveGroup { id: String, name: String, color: Option<String> },
    RemoveGroup(String),
    /// W1-02b: Postgres became available — attach it and drain what we buffered.
    AttachPool(PgPool),
}

static DB_TX: OnceLock<std::sync::mpsc::Sender<DbOp>> = OnceLock::new();

/// W1-02b: push a save into the bounded pending buffer, dropping the OLDEST on
/// overflow. Pure so the cap behaviour is unit-testable without a worker or a
/// database. Returns the id evicted, if any — the caller reports it, because a
/// silent eviction is the failure mode this whole item exists to kill.
fn push_pending(
    pending: &mut std::collections::VecDeque<DbDrawing>,
    d: DbDrawing,
    cap: usize,
) -> Option<String> {
    let evicted = if pending.len() >= cap {
        pending.pop_front().map(|e| e.id)
    } else {
        None
    };
    pending.push_back(d);
    evicted
}

/// W1-02b: start the worker WITHOUT a pool. Idempotent — safe to call before we
/// know whether Postgres is reachable.
///
/// This is the structural fix. Previously the channel was created only inside
/// `init(pool)`, so a PG-down startup left `DB_TX` unset and every call site did
/// `let Some(tx) = DB_TX.get() else { return; }` — i.e. drawings were discarded
/// at the source, with nothing to replay when PG came back. The worker now
/// always exists; only the *pool* is optional.
pub fn start_worker() {
    if DB_TX.get().is_some() {
        return;
    }
    let (tx, rx) = std::sync::mpsc::channel::<DbOp>();
    if DB_TX.set(tx).is_err() {
        return; // raced with another caller; theirs won
    }
    spawn_worker(rx);
}

/// Initialize with an already-connected pool and start the worker.
/// Preserved for callers that connect eagerly at startup.
pub fn init(pool: PgPool) {
    start_worker();
    attach_pool(pool);
}

/// W1-02b: hand a freshly connected pool to the running worker. Triggers a
/// drain of both the in-memory pending buffer and the on-disk JSONL spill.
pub fn attach_pool(pool: PgPool) {
    let _ = DB_POOL.set(pool.clone());
    start_worker();
    if let Some(tx) = DB_TX.get() {
        let _ = tx.send(DbOp::AttachPool(pool));
    }
}

fn spawn_worker(rx: std::sync::mpsc::Receiver<DbOp>) {
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        rt.block_on(async {
            // Bounded dead-letter list: drawings that exhausted all retries.
            // Kept in memory so the current session can observe them.
            let mut dead_letters: std::collections::VecDeque<DbDrawing> =
                std::collections::VecDeque::new();
            const DEAD_LETTER_CAP: usize = 64;

            // W1-02b: the pool is optional and may arrive mid-session.
            let mut pool_opt: Option<PgPool> = None;
            // Saves accepted while PG was down, awaiting a pool.
            let mut pending: std::collections::VecDeque<DbDrawing> =
                std::collections::VecDeque::new();

            while let Ok(op) = rx.recv() {
                // W1-02b: with no pool, buffer writes (durably, via the spill)
                // and answer reads as empty rather than dropping everything.
                let Some(pool) = pool_opt.clone() else {
                    match op {
                        DbOp::AttachPool(p) => {
                            PG_CONNECTED.store(true, std::sync::atomic::Ordering::Relaxed);
                            crate::data::connectivity::errors_sink::report(
                                crate::data::connectivity::errors_sink::ErrorLevel::Info,
                                "drawing_db", "pg_connected",
                                "Postgres connected — replaying drawings saved while it was down".to_string());
                            drain_pending(&p, &mut pending).await;
                            drain_spill(&p).await;
                            pool_opt = Some(p);
                        }
                        DbOp::Save { drawing, .. } => {
                            // Durable first: the spill is what survives a crash.
                            spill_dead_letter(&drawing);
                            if let Some(evicted) = push_pending(&mut pending, drawing, PENDING_CAP) {
                                crate::data::connectivity::errors_sink::report(
                                    crate::data::connectivity::errors_sink::ErrorLevel::Warn,
                                    "drawing_db", "pending_overflow",
                                    format!("in-memory pending-save buffer full — drawing {evicted} is on disk only until Postgres returns"));
                            }
                        }
                        DbOp::LoadSymbol { reply, .. } => { let _ = reply.send(vec![]); }
                        DbOp::LoadGroups { reply } => { let _ = reply.send(vec![]); }
                        // Removes/group-writes while down are not replayed: a
                        // remove of a row that was never written is a no-op, and
                        // replaying one against a row the user later re-created
                        // would destroy data. Dropping is the safe direction.
                        DbOp::Remove(_) | DbOp::SaveGroup { .. } | DbOp::RemoveGroup(_) => {}
                    }
                    continue;
                };

                match op {
                    DbOp::AttachPool(p) => {
                        // Reconnect with a fresh pool (e.g. after PG bounced).
                        PG_CONNECTED.store(true, std::sync::atomic::Ordering::Relaxed);
                        drain_pending(&p, &mut pending).await;
                        drain_spill(&p).await;
                        pool_opt = Some(p);
                    }
                    DbOp::Save { drawing, attempts } => {
                        if do_save(&pool, drawing.clone()).await {
                            // success — nothing more to do
                        } else if attempts < SAVE_MAX_ATTEMPTS {
                            // Transient failure — re-queue after a brief backoff
                            // proportional to the attempt number.
                            let backoff =
                                std::time::Duration::from_millis(200 * attempts as u64);
                            tokio::time::sleep(backoff).await;
                            if let Some(t) = DB_TX.get() {
                                let _ = t.send(DbOp::Save {
                                    drawing,
                                    attempts: attempts + 1,
                                });
                            }
                        } else {
                            // Exhausted retries — park in dead-letter list.
                            error!(
                                drawing_id = %drawing.id,
                                attempts,
                                "[drawing-db] drawing dropped after max retry attempts"
                            );
                            // W1-02 (audit): the dead-letter queue was write-only
                            // (never surfaced, never drained). (1) Report it so the
                            // trader SEES a drawing failed to save instead of it
                            // vanishing silently; (2) spill to a local JSONL file
                            // so nothing is lost past the in-memory cap of 64.
                            crate::data::connectivity::errors_sink::report(
                                crate::data::connectivity::errors_sink::ErrorLevel::Warn,
                                "drawing_db", "save_dead_lettered",
                                format!("drawing {} failed to persist after {} attempts — spilled to disk", drawing.id, attempts));
                            spill_dead_letter(&drawing);
                            if dead_letters.len() >= DEAD_LETTER_CAP {
                                dead_letters.pop_front();
                            }
                            dead_letters.push_back(drawing);
                        }
                    }
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

    debug!("[drawing-db] Worker started against new chart_state schema");
}

/// Get a reference to the pool (for direct queries from background threads).
pub fn get_pool() -> Option<&'static PgPool> {
    DB_POOL.get()
}

/// W1-02 (audit): true when drawings are actually reaching Postgres.
///
/// W1-02b correction: this used to be `DB_TX.get().is_some()`, which was right
/// only while the worker existed *only* when PG connected. The worker now runs
/// unconditionally so saves can be buffered during an outage, so channel
/// existence no longer implies persistence — it must track the pool. Reporting
/// "saving" while rows sit in a buffer would be precisely the kind of
/// comfortable lie this audit item exists to remove.
///
/// When false, drawings are buffered (in memory + JSONL spill) and replayed on
/// reconnect; the UI should still surface a "drawings not saving" indicator.
pub fn is_persisting() -> bool {
    PG_CONNECTED.load(std::sync::atomic::Ordering::Relaxed)
}

/// W1-02: path of the local dead-letter spill file — drawings that exhausted
/// retries, appended as JSONL so nothing is lost past the in-memory cap and a
/// future reconnect (W1-02b) can replay them. Next to the orders state dir.
fn dead_letter_path() -> std::path::PathBuf {
    let dir = std::env::current_exe().ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    let state = dir.join("state");
    let _ = std::fs::create_dir_all(&state);
    state.join("drawings_dead_letter.jsonl")
}

/// W1-02: append a dead-lettered drawing to the JSONL spill (best-effort — a
/// spill failure must never crash the worker).
fn spill_dead_letter(d: &DbDrawing) {
    use std::io::Write;
    if let Ok(line) = serde_json::to_string(d) {
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(dead_letter_path()) {
            let _ = writeln!(f, "{line}");
        }
    }
}

/// W1-02b: parse a JSONL spill into drawings, skipping blank and corrupt lines.
///
/// Pure → unit-testable without a database. A truncated last line (power loss
/// mid-append) must NOT discard the whole file: every parseable drawing is
/// recovered and the bad line is skipped, because the alternative is losing a
/// trader's chart work over one partial write.
fn parse_spill_lines(contents: &str) -> Vec<DbDrawing> {
    contents
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .filter_map(|l| serde_json::from_str::<DbDrawing>(l).ok())
        .collect()
}

/// W1-02b: replay saves buffered in memory while PG was down.
async fn drain_pending(pool: &PgPool, pending: &mut std::collections::VecDeque<DbDrawing>) {
    if pending.is_empty() {
        return;
    }
    let total = pending.len();
    let mut ok = 0usize;
    while let Some(d) = pending.pop_front() {
        if do_save(pool, d).await {
            ok += 1;
        }
    }
    crate::data::connectivity::errors_sink::report(
        if ok == total {
            crate::data::connectivity::errors_sink::ErrorLevel::Info
        } else {
            crate::data::connectivity::errors_sink::ErrorLevel::Warn
        },
        "drawing_db", "pending_drained",
        format!("replayed {ok}/{total} drawing(s) buffered while Postgres was down"),
    );
}

/// W1-02b: replay the on-disk JSONL spill, then clear it.
///
/// The file is only removed once every row it holds has been accepted by
/// Postgres. A partial drain leaves it intact so the next reconnect retries —
/// deleting on partial success would silently lose exactly the drawings that
/// were hardest to save. Saves are upserts keyed on id (`ON CONFLICT (id) DO
/// UPDATE`), so replaying a row that also came through the in-memory buffer is
/// harmless.
async fn drain_spill(pool: &PgPool) {
    let path = dead_letter_path();
    let Ok(contents) = std::fs::read_to_string(&path) else { return };
    let drawings = parse_spill_lines(&contents);
    if drawings.is_empty() {
        let _ = std::fs::remove_file(&path);
        return;
    }
    let total = drawings.len();
    let mut ok = 0usize;
    for d in drawings {
        if do_save(pool, d).await {
            ok += 1;
        }
    }
    if ok == total {
        let _ = std::fs::remove_file(&path);
    }
    crate::data::connectivity::errors_sink::report(
        if ok == total {
            crate::data::connectivity::errors_sink::ErrorLevel::Info
        } else {
            crate::data::connectivity::errors_sink::ErrorLevel::Warn
        },
        "drawing_db", "spill_drained",
        format!("recovered {ok}/{total} drawing(s) from the on-disk spill{}",
                if ok == total { "" } else { " — file kept for the next retry" }),
    );
}

/// W1-02b: backoff schedule for the reconnect loop, in seconds. Pure →
/// testable. Ramps to a 60s ceiling so a long outage costs a trickle of
/// connection attempts rather than a tight retry storm against a downed DB.
fn reconnect_backoff_secs(attempt: u32) -> u64 {
    match attempt {
        0..=2 => 2,
        3..=5 => 5,
        6..=10 => 15,
        _ => 60,
    }
}

/// W1-02b: retry Postgres in the background until it comes up, then attach the
/// pool (which drains the buffer and the spill) and run `on_connect` for any
/// extra wiring the caller owns (pool registration, watchlist worker, …).
///
/// Runs on its own thread: a downed database must never block startup or the
/// render loop. Safe to call when PG was merely slow to boot — the first
/// successful connect ends the loop.
pub fn spawn_reconnect<F>(pg_url: String, on_connect: F)
where
    F: FnOnce(PgPool) + Send + 'static,
{
    start_worker();
    crate::foundation::guard::spawn_guarded("drawing_db_reconnect", move || {
        let rt = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
            Ok(rt) => rt,
            Err(_) => return,
        };
        rt.block_on(async {
            let mut attempt: u32 = 0;
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(
                    reconnect_backoff_secs(attempt),
                )).await;
                attempt = attempt.saturating_add(1);
                match sqlx::postgres::PgPoolOptions::new()
                    .max_connections(3)
                    .acquire_timeout(std::time::Duration::from_secs(5))
                    .connect(&pg_url)
                    .await
                {
                    Ok(pool) => {
                        attach_pool(pool.clone());
                        on_connect(pool);
                        return;
                    }
                    Err(e) => {
                        debug!("[drawing-db] reconnect attempt {attempt} failed: {e}");
                    }
                }
            }
        });
    });
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

/// Save a drawing. Enqueues to the worker thread; failed saves are retried
/// up to `SAVE_MAX_ATTEMPTS` times before landing on the bounded dead-letter
/// list.
pub fn save(drawing: &DbDrawing) {
    if let Some(tx) = DB_TX.get() {
        let _ = tx.send(DbOp::Save { drawing: drawing.clone(), attempts: 1 });
    }
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

/// W1-01: sentinel i16 for a kind the crude DB-layer enum can't represent. The
/// real kind lives in `extras.drawing_type`; this is just the legacy index
/// column, and `kind_from_i16` returns None for it (load prefers the string).
const DRAWING_KIND_OTHER: i16 = 100;

/// W1-01: the i16 to store for a drawing_type string. Best-effort via the crude
/// enum; an unrepresentable (rich) kind gets the OTHER sentinel instead of being
/// dropped. Pure → unit-testable.
fn save_kind_i16(drawing_type: &str) -> i16 {
    parse_kind(drawing_type).map(kind_to_i16).unwrap_or(DRAWING_KIND_OTHER)
}

/// W1-01: resolve the drawing_type string on load. Prefers the preserved real
/// string in `extras`; falls back to the legacy i16→string mapping for old rows;
/// None only when neither is available. Pure → unit-testable.
fn resolve_drawing_type(extras_str: Option<&str>, kind_i16: i16) -> Option<String> {
    match extras_str {
        Some(s) => Some(s.to_string()),
        None => kind_from_i16(kind_i16).map(|k| kind_to_str(k).to_string()),
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
/// Returns the chart's UUID.
///
/// Uses INSERT … ON CONFLICT DO NOTHING to eliminate the TOCTOU race that
/// existed in the old SELECT-then-INSERT pattern.  The UNIQUE constraint on
/// (user_id, symbol_canonical) is added by migration 003 (see
/// `migrations/003_charts_unique_user_symbol.sql`).
async fn find_or_create_chart(pool: &PgPool, symbol: &str) -> Result<Uuid, sqlx::Error> {
    // Default viewport bytes (25 zeros) — replaced on first real save through
    // the canonical path.  Zero bytes decode to an all-zero Viewport.
    let viewport_bytes: Vec<u8> = vec![0u8; 25];

    // Attempt an idempotent insert.  ON CONFLICT DO NOTHING means if another
    // writer (or a prior attempt) already inserted this row we simply get no
    // rows back from RETURNING — handled by the follow-up SELECT below.
    let inserted: Option<Uuid> = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO charts \
             (user_id, symbol_canonical, asset_class, timeframe, theme, viewport, schema_version) \
         VALUES (0, $1, 0, 0, 0, $2, 1) \
         ON CONFLICT (user_id, symbol_canonical) DO NOTHING \
         RETURNING id",
    )
    .bind(symbol)
    .bind(&viewport_bytes)
    .fetch_optional(pool)
    .await?;

    if let Some(id) = inserted {
        return Ok(id);
    }

    // Row already existed — fetch it.
    sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM charts WHERE user_id = 0 AND symbol_canonical = $1 LIMIT 1",
    )
    .bind(symbol)
    .fetch_one(pool)
    .await
}

/// Find or create a style row matching the given fields for this chart.
/// Returns the per-chart `style_id` (i32).
///
/// The SELECT → MAX(style_id)+1 → INSERT sequence is wrapped in a transaction
/// so that a retry after a transient PG failure cannot compute the same
/// `next_id` and hit a PRIMARY KEY violation that previously caused `do_save`
/// to return early and silently drop the drawing.
async fn intern_style(
    pool: &PgPool,
    chart_id: Uuid,
    stroke_rgba: i32,
    width_x100: i16,
    dash: i16,
    fill: i32,
) -> Result<i32, sqlx::Error> {
    // Fast path: style already exists — no transaction needed.
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

    // Slow path: style is new.  Wrap the MAX→INSERT in a transaction so that
    // a concurrent/retried call cannot allocate the same style_id and collide
    // on the PRIMARY KEY (chart_id, style_id).
    let mut tx = pool.begin().await?;

    // Re-check inside the transaction in case another writer just inserted.
    if let Some(row) = sqlx::query(
        "SELECT style_id FROM chart_styles \
         WHERE chart_id = $1 AND stroke = $2 AND width_x100 = $3 AND dash = $4 AND fill = $5 LIMIT 1",
    )
    .bind(chart_id)
    .bind(stroke_rgba)
    .bind(width_x100)
    .bind(dash)
    .bind(fill)
    .fetch_optional(&mut *tx)
    .await?
    {
        tx.rollback().await?;
        return row.try_get::<i32, _>("style_id");
    }

    let next_id: i32 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(style_id) + 1, 0) FROM chart_styles WHERE chart_id = $1",
    )
    .bind(chart_id)
    .fetch_one(&mut *tx)
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
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
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
        Err(e) => { warn!("[drawing-db] load error: {e}"); return vec![]; }
    };

    let mut drawings = Vec::with_capacity(rows.len());
    for r in rows {
        let id: Uuid = match r.try_get("id") { Ok(v) => v, Err(_) => continue };
        let kind_i: i16 = r.try_get("kind").unwrap_or(0);
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

        // W1-01: prefer the preserved real drawing_type string; fall back to the
        // legacy i16→string mapping for rows saved before the extras field
        // existed. Only skip the row if we have neither (truly unknown).
        let drawing_type = match resolve_drawing_type(
            extras.get("drawing_type").and_then(|v| v.as_str()), kind_i)
        {
            Some(dt) => dt,
            None => continue,
        };

        drawings.push(DbDrawing {
            id: id.to_string(),
            symbol: symbol.to_string(),
            timeframe,
            drawing_type,
            points: decode_points(&points_bytes),
            color: rgb_to_hex(rgb),
            opacity,
            line_style: dash_i16_to_str(dash_i).to_string(),
            thickness: width_x100 as f32 / 100.0,
            group_id,
        });
    }
    debug!("[drawing-db] loaded {} drawings for {}", drawings.len(), symbol);
    drawings
}

/// Returns `true` on success, `false` on any transient DB error (so the
/// caller can retry).  Non-retryable failures (bad UUID, unknown kind) also
/// return `false` but log a different message so they are distinguishable.
async fn do_save(pool: &PgPool, d: DbDrawing) -> bool {
    let id = match Uuid::parse_str(&d.id) {
        Ok(u) => u,
        Err(_) => { warn!("[drawing-db] invalid UUID (not retrying): {}", d.id); return false; }
    };
    // W1-01 (audit): do NOT drop an unknown kind. The DB-layer DrawingKind is a
    // crude 13-variant enum whose parse_kind recognizes only 6 of the 22 strings
    // that gpu.rs::drawing_to_db actually emits ("hzone", "channel", "gannfan",
    // "elliott", "fibext", "avwap", ...), so 16 of 22 drawing kinds were silently
    // dead-lettered here forever. The drawing_type STRING is the lossless
    // identity — it is preserved in `extras` below and read back on load, so the
    // i16 `kind` column is now just a best-effort legacy index. Never reject.
    let kind_i16 = save_kind_i16(&d.drawing_type);

    let chart_id = match find_or_create_chart(pool, &d.symbol).await {
        Ok(c) => c,
        Err(e) => { warn!("[drawing-db] chart upsert (will retry): {e}"); return false; }
    };

    let rgb = parse_rgb(&d.color);
    let alpha = (d.opacity.clamp(0.0, 1.0) * 255.0).round() as u32 & 0xFF;
    let stroke = ((rgb << 8) | alpha) as i32;
    let width_x100 = (d.thickness * 100.0).round().clamp(0.0, i16::MAX as f32) as i16;
    let dash = dash_str_to_i16(&d.line_style);

    let style_id = match intern_style(pool, chart_id, stroke, width_x100, dash, 0).await {
        Ok(s) => s,
        Err(e) => { warn!("[drawing-db] style intern (will retry): {e}"); return false; }
    };

    let mut extras = serde_json::Map::new();
    if !d.timeframe.is_empty() {
        extras.insert("timeframe".into(), serde_json::Value::String(d.timeframe.clone()));
    }
    if !d.group_id.is_empty() && d.group_id != "default" {
        extras.insert("group_id".into(), serde_json::Value::String(d.group_id.clone()));
    }
    // W1-01: preserve the real drawing_type string (the lossless kind identity)
    // so all 22 kinds survive the round-trip regardless of the crude i16 enum.
    extras.insert("drawing_type".into(), serde_json::Value::String(d.drawing_type.clone()));
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
    .bind(kind_i16)
    .bind(flags)
    .bind(style_id)
    .bind(&points_bytes)
    .bind(&extras_json)
    .execute(pool)
    .await;

    match result {
        Ok(_) => { debug!("[drawing-db] saved {} {} {}", d.drawing_type, d.symbol, d.id); true }
        Err(e) => { warn!("[drawing-db] save error (will retry): {e}"); false }
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

// ─────────────────────────────────────────────────────────────────────────
// Failure-injection tests for the PostgreSQL persistence layer.
//
// These tests do NOT require a live Postgres instance — they use clearly-
// bogus URLs / closed ports to provoke connection failures deterministically.
// Each test enforces an upper time bound via `tokio::time::timeout` so
// the CI job cannot hang on a blocked pool.
// ─────────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use std::time::Duration;
    use super::{save_kind_i16, resolve_drawing_type, DRAWING_KIND_OTHER};
    use super::{parse_spill_lines, push_pending, reconnect_backoff_secs, DbDrawing, PENDING_CAP};

    fn drawing(id: &str) -> DbDrawing {
        DbDrawing {
            id: id.into(),
            symbol: "SPY".into(),
            timeframe: "5m".into(),
            drawing_type: "trendline".into(),
            points: vec![(1.0, 400.0), (2.0, 410.0)],
            color: "#FF0000".into(),
            opacity: 1.0,
            line_style: "solid".into(),
            thickness: 1.5,
            group_id: String::new(),
        }
    }

    // ─── W1-02b: reconnect / drain ───────────────────────────────────────────

    #[test]
    fn spill_round_trips_through_jsonl() {
        let a = drawing("a");
        let b = drawing("b");
        let jsonl = format!(
            "{}\n{}\n",
            serde_json::to_string(&a).unwrap(),
            serde_json::to_string(&b).unwrap()
        );
        let got = parse_spill_lines(&jsonl);
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].id, "a");
        assert_eq!(got[1].points, b.points, "geometry must survive the spill");
    }

    #[test]
    fn a_truncated_last_line_does_not_discard_the_whole_spill() {
        // Power loss mid-append leaves a partial JSON object. Losing every
        // recovered drawing over one bad line is not an acceptable trade.
        let good = serde_json::to_string(&drawing("good")).unwrap();
        let jsonl = format!("{good}\n{{\"id\":\"trunca");
        let got = parse_spill_lines(&jsonl);
        assert_eq!(got.len(), 1, "the intact drawing is still recovered");
        assert_eq!(got[0].id, "good");
    }

    #[test]
    fn blank_lines_are_ignored() {
        let good = serde_json::to_string(&drawing("x")).unwrap();
        assert_eq!(parse_spill_lines(&format!("\n\n{good}\n\n  \n")).len(), 1);
        assert!(parse_spill_lines("").is_empty());
    }

    #[test]
    fn pending_buffer_evicts_oldest_and_reports_it() {
        let mut q = std::collections::VecDeque::new();
        assert!(push_pending(&mut q, drawing("1"), 2).is_none());
        assert!(push_pending(&mut q, drawing("2"), 2).is_none());
        // Third push at cap 2 evicts the oldest, and says which one — a silent
        // eviction is the exact failure mode W1-02 exists to remove.
        assert_eq!(push_pending(&mut q, drawing("3"), 2).as_deref(), Some("1"));
        assert_eq!(q.len(), 2);
        assert_eq!(q.front().unwrap().id, "2");
        assert_eq!(q.back().unwrap().id, "3");
    }

    #[test]
    fn pending_cap_is_generous_enough_for_a_real_outage() {
        // A trader marking up charts through a PG restart should not hit this.
        assert!(PENDING_CAP >= 256, "cap {PENDING_CAP} is too small to ride out an outage");
    }

    #[test]
    fn reconnect_backoff_ramps_then_ceilings() {
        assert_eq!(reconnect_backoff_secs(0), 2, "retry quickly at first");
        assert_eq!(reconnect_backoff_secs(4), 5);
        assert_eq!(reconnect_backoff_secs(8), 15);
        // A long outage must not become a retry storm against a downed DB.
        assert_eq!(reconnect_backoff_secs(50), 60);
        assert_eq!(reconnect_backoff_secs(u32::MAX), 60, "no overflow at the ceiling");
    }

    // W1-01 (audit): every drawing_type string gpu.rs::drawing_to_db can emit.
    // 16 of these 22 were silently dead-lettered by do_save's parse_kind reject
    // (it knows only 6). The fix preserves the string in extras + never rejects.
    const ALL_DRAWING_TYPES: &[&str] = &[
        "hline", "trendline", "hzone", "barmarker", "fibonacci", "channel",
        "fibchannel", "pitchfork", "gannfan", "regression", "xabcd", "elliott",
        "avwap", "pricerange", "riskreward", "vline", "ray", "fibext",
        "fibtimezone", "fibarc", "gannbox", "textnote",
    ];

    #[test]
    fn w1_01_no_drawing_type_is_dropped_on_save() {
        // The save path must never reject a kind: save_kind_i16 returns a value
        // for EVERY emitted string (the crude enum's 6 map to their code, the
        // other 16 get the OTHER sentinel) — none are dead-lettered.
        for dt in ALL_DRAWING_TYPES {
            let i16v = save_kind_i16(dt);
            // sanity: it produced *some* code (no panic / no reject path).
            let _ = i16v;
        }
        // The exotic kinds specifically get OTHER (the crude enum can't name
        // them) — proving they'd previously have been dropped.
        assert_eq!(save_kind_i16("gannfan"), DRAWING_KIND_OTHER);
        assert_eq!(save_kind_i16("elliott"), DRAWING_KIND_OTHER);
        assert_eq!(save_kind_i16("hzone"), DRAWING_KIND_OTHER);
        // A crude-enum kind still maps to its real code, not OTHER.
        assert_ne!(save_kind_i16("trendline"), DRAWING_KIND_OTHER);
    }

    #[test]
    fn w1_01_load_round_trips_every_kind_via_preserved_string() {
        // On load, the preserved extras string is authoritative — so every one
        // of the 22 kinds round-trips its exact identity, even the ones the i16
        // column stores as OTHER.
        for dt in ALL_DRAWING_TYPES {
            let i16v = save_kind_i16(dt);
            let resolved = resolve_drawing_type(Some(dt), i16v);
            assert_eq!(resolved.as_deref(), Some(*dt),
                "kind {dt} must round-trip its exact string via extras");
        }
    }

    #[test]
    fn w1_01_load_falls_back_to_legacy_i16_when_no_extras() {
        // Legacy rows (saved before the extras field) have no string — fall back
        // to the i16 mapping; a valid crude code resolves, OTHER/unknown → None.
        assert_eq!(resolve_drawing_type(None, 0).as_deref(), Some("trendline"));
        assert_eq!(resolve_drawing_type(None, DRAWING_KIND_OTHER), None);
    }

    const BOGUS_PG_URL: &str = "postgres://apex:apex@127.0.0.1:1/apex_test";
    const STARTUP_DEADLINE: Duration = Duration::from_secs(5);
    const DRAIN_DEADLINE: Duration = Duration::from_secs(3);

    /// Attempting to connect to an unreachable Postgres URL must complete
    /// within `STARTUP_DEADLINE` and must not block indefinitely.
    ///
    /// The init path in `lib.rs` already guards against blocking startup
    /// (pool acquire errors are reported as warnings, not panics). This test
    /// exercises the `PgPool::connect_lazy` / timeout semantics directly so
    /// we have explicit coverage before hitting prod.
    ///
    /// Note: `drawing_db::init` itself is NOT called here because the module
    /// uses a `OnceLock` and is exercised in the integration init path.
    /// Instead we verify that constructing a pool against an unreachable URL
    /// completes within the deadline and does not panic.
    #[tokio::test]
    async fn pg_pool_unreachable_does_not_block_startup() {
        let result = tokio::time::timeout(STARTUP_DEADLINE, async {
            // `PgPoolOptions::connect_lazy` returns immediately (no network
            // call at build time). The failure surfaces only on the first
            // query — which is the contract the app relies on.
            sqlx::postgres::PgPoolOptions::new()
                .max_connections(1)
                .acquire_timeout(Duration::from_secs(2))
                .connect_lazy(BOGUS_PG_URL)
        })
        .await;

        assert!(
            result.is_ok(),
            "Pool construction timed out — should return immediately for lazy connect"
        );

        // If lazy connect is unavailable, try a non-blocking connect attempt
        // that is guaranteed to fail fast (connection refused on port 1).
        // This verifies the pool is None-equivalent for callers checking `get_pool()`.
        if let Ok(pool_result) = result {
            assert!(
                pool_result.is_ok(),
                "connect_lazy should succeed immediately (no network I/O at build time)"
            );
            // The pool was built but `drawing_db::init` was intentionally NOT
            // called with this broken pool — confirming `get_pool()` stays None.
            assert!(
                super::get_pool().is_none(),
                "get_pool() should be None when drawing_db::init was never called"
            );
        }
    }

    /// `PgPoolShutdown::drain()` must complete within `DRAIN_DEADLINE` even
    /// when the pool was built against a reachable URL that has since become
    /// unreachable. We test the happy path: close() on a lazy-built pool
    /// (no connections ever opened) should return immediately.
    #[tokio::test]
    async fn pg_pool_shutdown_drains_cleanly() {
        use crate::data::connectivity::shutdown::{PgPoolShutdown, Shutdown};

        // Build a lazy pool — no actual connections are held.
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(2)
            .acquire_timeout(Duration::from_secs(1))
            .connect_lazy(BOGUS_PG_URL)
            .expect("connect_lazy must succeed without a network call");

        let shutdown = PgPoolShutdown { name: "test_pool", pool };

        let result = tokio::time::timeout(
            DRAIN_DEADLINE,
            shutdown.drain(DRAIN_DEADLINE),
        )
        .await;

        assert!(
            result.is_ok(),
            "drain() exceeded the {DRAIN_DEADLINE:?} deadline"
        );
        assert!(
            result.unwrap().is_ok(),
            "drain() returned an error on a pool with no live connections"
        );
    }

    /// Pool exhaustion test: acquiring a connection from a 1-connection pool
    /// against a bogus URL should time out and return an error — not panic or
    /// block indefinitely.
    ///
    /// This validates that the `acquire_timeout` setting is respected, which
    /// is the production guard against "query piles up forever" when PG is
    /// temporarily unavailable.
    ///
    /// Marked `#[ignore]` because this test has a 2-second mandatory wait
    /// (the acquire timeout) and would slow down every CI run — run it
    /// manually with `cargo test -- --ignored persistence::drawing_db::tests::pg_pool_exhausted_save_returns_error`.
    #[tokio::test]
    #[ignore = "requires 2-second acquire_timeout to elapse; run manually with --ignored"]
    async fn pg_pool_exhausted_save_returns_error() {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .acquire_timeout(Duration::from_secs(2))
            .connect_lazy(BOGUS_PG_URL)
            .expect("connect_lazy must succeed");

        // Attempting to acquire from a pool that can never connect should
        // time out and return an error (not panic).
        let result = pool.acquire().await;
        assert!(
            result.is_err(),
            "Expected pool.acquire() to fail against unreachable URL, got Ok"
        );
    }
}
