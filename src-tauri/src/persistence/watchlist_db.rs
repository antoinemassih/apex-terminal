//! Sync persistence wrapper for watchlists + symbol universes.
//!
//! Mirrors `drawing_db`: a dedicated worker thread owns a tokio runtime and
//! services a stream of `DbOp` messages over an mpsc channel. The renderer
//! never blocks on Postgres; saves are fire-and-forget, loads are called
//! only from background threads with a 5s timeout fallback.
//!
//! The Postgres pool is shared with `drawing_db` via `drawing_db::get_pool()`
//! so we don't open a second connection pool to the same database.

use sqlx::postgres::PgPool;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use crate::chart_renderer::gpu::SavedWatchlist;
use crate::watchlist::codec::db as codec;

// ─────────────────────────────────────────────────────────────────────────
// Process-level universe cache.
//
// The render thread reads from this synchronously every frame. The
// background refresh thread (see `crate::watchlist::refresh`) populates it
// from Postgres / Polygon. Cold start = empty map → heat panel renders a
// loading placeholder.
// ─────────────────────────────────────────────────────────────────────────

static UNIVERSE_CACHE: OnceLock<Mutex<HashMap<String, Vec<String>>>> = OnceLock::new();

fn universe_cache() -> &'static Mutex<HashMap<String, Vec<String>>> {
    UNIVERSE_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Synchronous in-RAM lookup. Safe to call from the render thread — never
/// blocks on Postgres or HTTP. Returns empty Vec when the universe hasn't
/// been refreshed yet.
pub fn cached_universe(name: &str) -> Vec<String> {
    universe_cache().lock().ok()
        .and_then(|g| g.get(name).cloned())
        .unwrap_or_default()
}

/// Replace the cached symbol list for `name`. Called by the refresh thread
/// after a successful Polygon fetch + DB save, and by the load-from-DB
/// path on cold start.
pub fn set_cached_universe(name: &str, symbols: Vec<String>) {
    if let Ok(mut g) = universe_cache().lock() {
        g.insert(name.to_string(), symbols);
    }
}

/// Bulk snapshot of the cache (diagnostics).
pub fn list_universe_cache() -> Vec<(String, usize)> {
    universe_cache().lock().ok()
        .map(|g| g.iter().map(|(k, v)| (k.clone(), v.len())).collect())
        .unwrap_or_default()
}

const USER_ID: i64 = 0; // single-tenant for now, matches drawing_db

/// Messages for the watchlist DB worker thread.
enum DbOp {
    SaveAll {
        watchlists: Vec<SavedWatchlist>,
        active_idx: usize,
    },
    LoadAll {
        reply: std::sync::mpsc::Sender<(Vec<SavedWatchlist>, usize)>,
    },
    SaveUniverse {
        name: String,
        display_name: String,
        kind: String,
        source: String,
        members: Vec<(String, Option<f32>)>,
    },
    LoadUniverse {
        name: String,
        reply: std::sync::mpsc::Sender<Vec<String>>,
    },
    ListUniverses {
        kind: Option<String>,
        reply: std::sync::mpsc::Sender<Vec<(String, String)>>,
    },
    UniverseFetchedAt {
        name: String,
        reply: std::sync::mpsc::Sender<Option<chrono::DateTime<chrono::Utc>>>,
    },
}

static DB_TX: OnceLock<std::sync::mpsc::Sender<DbOp>> = OnceLock::new();

/// Start the watchlist DB worker. Call after `drawing_db::init` so we share
/// its pool. Passing the pool explicitly is also supported (it just uses the
/// passed pool directly, ignoring `drawing_db::get_pool`).
pub fn init(pool: PgPool) {
    let (tx, rx) = std::sync::mpsc::channel::<DbOp>();
    if DB_TX.set(tx).is_err() {
        // Already initialized — keep the existing worker.
        return;
    }

    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("[watchlist-db] tokio runtime build");
        rt.block_on(async {
            while let Ok(op) = rx.recv() {
                match op {
                    DbOp::SaveAll { watchlists, active_idx } => {
                        if let Err(e) = codec::save_watchlists(&pool, USER_ID, &watchlists, active_idx).await {
                            eprintln!("[watchlist-db] save_all error: {e}");
                        }
                    }
                    DbOp::LoadAll { reply } => {
                        let result = match codec::load_watchlists(&pool, USER_ID).await {
                            Ok(v) => v,
                            Err(e) => {
                                eprintln!("[watchlist-db] load_all error: {e}");
                                (Vec::new(), 0)
                            }
                        };
                        let _ = reply.send(result);
                    }
                    DbOp::SaveUniverse { name, display_name, kind, source, members } => {
                        if let Err(e) = codec::save_universe(&pool, &name, &display_name, &kind, &source, &members).await {
                            eprintln!("[watchlist-db] save_universe '{name}' error: {e}");
                        }
                    }
                    DbOp::LoadUniverse { name, reply } => {
                        let result = codec::load_universe(&pool, &name).await.unwrap_or_else(|e| {
                            eprintln!("[watchlist-db] load_universe '{name}' error: {e}");
                            Vec::new()
                        });
                        let _ = reply.send(result);
                    }
                    DbOp::UniverseFetchedAt { name, reply } => {
                        let result = codec::universe_fetched_at(&pool, &name).await.unwrap_or_else(|e| {
                            eprintln!("[watchlist-db] universe_fetched_at '{name}' error: {e}");
                            None
                        });
                        let _ = reply.send(result);
                    }
                    DbOp::ListUniverses { kind, reply } => {
                        let result = codec::list_universes(&pool, kind.as_deref()).await.unwrap_or_else(|e| {
                            eprintln!("[watchlist-db] list_universes error: {e}");
                            Vec::new()
                        });
                        let _ = reply.send(result);
                    }
                }
            }
        });
    });

    eprintln!("[watchlist-db] Worker started against watchlists/symbol_universes schema");
}

/// Save the entire watchlist set (fire-and-forget).
pub fn save_all(watchlists: &[SavedWatchlist], active_idx: usize) {
    if let Some(tx) = DB_TX.get() {
        let _ = tx.send(DbOp::SaveAll {
            watchlists: watchlists.to_vec(),
            active_idx,
        });
    }
}

/// Load the entire watchlist set (blocking with 5s timeout, falls back to empty).
pub fn load_all() -> (Vec<SavedWatchlist>, usize) {
    let Some(tx) = DB_TX.get() else { return (Vec::new(), 0); };
    let (reply_tx, reply_rx) = std::sync::mpsc::channel();
    if tx.send(DbOp::LoadAll { reply: reply_tx }).is_err() {
        return (Vec::new(), 0);
    }
    reply_rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap_or_else(|_| (Vec::new(), 0))
}

/// Save (upsert) a symbol universe (fire-and-forget).
pub fn save_universe(
    name: &str,
    display_name: &str,
    kind: &str,
    source: &str,
    members: &[(String, Option<f32>)],
) {
    if let Some(tx) = DB_TX.get() {
        let _ = tx.send(DbOp::SaveUniverse {
            name: name.to_string(),
            display_name: display_name.to_string(),
            kind: kind.to_string(),
            source: source.to_string(),
            members: members.to_vec(),
        });
    }
}

/// Load a universe's symbol list (blocking with 5s timeout, falls back to empty).
pub fn load_universe(name: &str) -> Vec<String> {
    let Some(tx) = DB_TX.get() else { return Vec::new(); };
    let (reply_tx, reply_rx) = std::sync::mpsc::channel();
    if tx.send(DbOp::LoadUniverse { name: name.to_string(), reply: reply_tx }).is_err() {
        return Vec::new();
    }
    reply_rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap_or_default()
}

/// Get the `fetched_at` timestamp for a universe. `None` = never seeded
/// or DB unavailable. Blocking with 5s timeout — only call from background
/// threads (refresh job).
pub fn universe_fetched_at(name: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    let tx = DB_TX.get()?;
    let (reply_tx, reply_rx) = std::sync::mpsc::channel();
    if tx.send(DbOp::UniverseFetchedAt { name: name.to_string(), reply: reply_tx }).is_err() {
        return None;
    }
    reply_rx.recv_timeout(std::time::Duration::from_secs(5)).ok().flatten()
}

/// List universes, optionally filtered by `kind` (blocking with 5s timeout).
pub fn list_universes(kind: Option<&str>) -> Vec<(String, String)> {
    let Some(tx) = DB_TX.get() else { return Vec::new(); };
    let (reply_tx, reply_rx) = std::sync::mpsc::channel();
    if tx.send(DbOp::ListUniverses {
        kind: kind.map(|s| s.to_string()),
        reply: reply_tx,
    }).is_err() {
        return Vec::new();
    }
    reply_rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap_or_default()
}
