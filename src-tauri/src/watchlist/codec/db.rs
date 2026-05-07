//! Async sqlx round-trip for watchlists + symbol universes.
//!
//! The save path is a single transaction: delete every watchlist (and via
//! ON DELETE CASCADE, all sections + items) for the given user, then
//! re-insert from the in-memory snapshot. This mirrors `drawing_db`'s
//! delete-and-reinsert strategy — simple, correct, and fast for the small
//! data volumes a watchlist UI produces.

use sqlx::postgres::PgPool;
use sqlx::Row;
use uuid::Uuid;

use crate::chart_renderer::gpu::{SavedWatchlist, WatchlistItem, WatchlistSection};

// ─────────────────────────────────────────────────────────────────────────
// Errors
// ─────────────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum DbCodecError {
    Sqlx(sqlx::Error),
}

impl std::fmt::Display for DbCodecError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DbCodecError::Sqlx(e) => write!(f, "sqlx: {e}"),
        }
    }
}

impl std::error::Error for DbCodecError {}

impl From<sqlx::Error> for DbCodecError {
    fn from(e: sqlx::Error) -> Self {
        DbCodecError::Sqlx(e)
    }
}

// ─────────────────────────────────────────────────────────────────────────
// Watchlist save / load
// ─────────────────────────────────────────────────────────────────────────

/// Replace every watchlist for `user_id` with the contents of `watchlists`.
/// `active_idx` is persisted via the `is_active` boolean column — exactly
/// one watchlist per user is marked active.
pub async fn save_watchlists(
    pool: &PgPool,
    user_id: i64,
    watchlists: &[SavedWatchlist],
    active_idx: usize,
) -> Result<(), DbCodecError> {
    let mut tx = pool.begin().await?;

    // Delete-and-reinsert. ON DELETE CASCADE handles sections + items.
    sqlx::query("DELETE FROM watchlists WHERE user_id = $1")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;

    for (wl_idx, wl) in watchlists.iter().enumerate() {
        let is_active = wl_idx == active_idx;
        let wl_id: Uuid = sqlx::query_scalar(
            "INSERT INTO watchlists (user_id, name, kind, is_active, position) \
             VALUES ($1, $2, 0, $3, $4) RETURNING id",
        )
        .bind(user_id)
        .bind(&wl.name)
        .bind(is_active)
        .bind(wl_idx as i32)
        .fetch_one(&mut *tx)
        .await?;

        for (sec_idx, sec) in wl.sections.iter().enumerate() {
            let sec_id: Uuid = sqlx::query_scalar(
                "INSERT INTO watchlist_sections (watchlist_id, title, color, collapsed, position) \
                 VALUES ($1, $2, $3, $4, $5) RETURNING id",
            )
            .bind(wl_id)
            .bind(&sec.title)
            .bind(sec.color.as_deref())
            .bind(sec.collapsed)
            .bind(sec_idx as i32)
            .fetch_one(&mut *tx)
            .await?;

            for (item_idx, item) in sec.items.iter().enumerate() {
                let asset_class: i16 = if item.is_option { 3 } else { 0 };
                sqlx::query(
                    "INSERT INTO watchlist_items \
                     (section_id, symbol, asset_class, pinned, note, position, \
                      is_option, underlying, option_type, strike, expiry) \
                     VALUES ($1, $2, $3, $4, NULL, $5, $6, $7, $8, $9, $10)",
                )
                .bind(sec_id)
                .bind(&item.symbol)
                .bind(asset_class)
                .bind(item.pinned)
                .bind(item_idx as i32)
                .bind(item.is_option)
                .bind(opt_str(&item.underlying))
                .bind(opt_str(&item.option_type))
                .bind(if item.is_option { Some(item.strike) } else { None })
                .bind(opt_str(&item.expiry))
                .execute(&mut *tx)
                .await?;
            }
        }
    }

    tx.commit().await?;
    Ok(())
}

fn opt_str(s: &str) -> Option<&str> {
    if s.is_empty() { None } else { Some(s) }
}

/// Load all watchlists for `user_id`. Returns `(watchlists, active_idx)`.
/// Active idx is the index of the watchlist with `is_active = TRUE`, or 0.
pub async fn load_watchlists(
    pool: &PgPool,
    user_id: i64,
) -> Result<(Vec<SavedWatchlist>, usize), DbCodecError> {
    let wl_rows = sqlx::query(
        "SELECT id, name, is_active FROM watchlists \
         WHERE user_id = $1 ORDER BY position ASC, created_at ASC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    let mut out: Vec<SavedWatchlist> = Vec::with_capacity(wl_rows.len());
    let mut active_idx: usize = 0;

    for (idx, wl_row) in wl_rows.iter().enumerate() {
        let wl_id: Uuid = wl_row.try_get("id")?;
        let name: String = wl_row.try_get("name")?;
        let is_active: bool = wl_row.try_get("is_active").unwrap_or(false);
        if is_active {
            active_idx = idx;
        }

        let sec_rows = sqlx::query(
            "SELECT id, title, color, collapsed FROM watchlist_sections \
             WHERE watchlist_id = $1 ORDER BY position ASC",
        )
        .bind(wl_id)
        .fetch_all(pool)
        .await?;

        let mut sections: Vec<WatchlistSection> = Vec::with_capacity(sec_rows.len());
        let mut next_section_id: u32 = 1;

        for (sec_idx, sec_row) in sec_rows.iter().enumerate() {
            let sec_id: Uuid = sec_row.try_get("id")?;
            let title: String = sec_row.try_get("title").unwrap_or_default();
            let color: Option<String> = sec_row.try_get("color").ok();
            let collapsed: bool = sec_row.try_get("collapsed").unwrap_or(false);

            let item_rows = sqlx::query(
                "SELECT symbol, pinned, is_option, underlying, option_type, strike, expiry \
                 FROM watchlist_items WHERE section_id = $1 ORDER BY position ASC",
            )
            .bind(sec_id)
            .fetch_all(pool)
            .await?;

            let mut items: Vec<WatchlistItem> = Vec::with_capacity(item_rows.len());
            for ir in item_rows {
                let symbol: String = ir.try_get("symbol").unwrap_or_default();
                if symbol.is_empty() { continue; }
                let pinned: bool = ir.try_get("pinned").unwrap_or(false);
                let is_option: bool = ir.try_get("is_option").unwrap_or(false);
                let underlying: String = ir.try_get("underlying").ok().unwrap_or_default();
                let option_type: String = ir.try_get("option_type").ok().unwrap_or_default();
                let strike: f32 = ir.try_get::<Option<f32>, _>("strike").ok().flatten().unwrap_or(0.0);
                let expiry: String = ir.try_get("expiry").ok().unwrap_or_default();

                items.push(WatchlistItem {
                    symbol,
                    price: 0.0,
                    prev_close: 0.0,
                    loaded: false,
                    is_option,
                    underlying,
                    option_type,
                    strike,
                    expiry,
                    bid: 0.0,
                    ask: 0.0,
                    pinned,
                    tags: vec![],
                    rvol: 1.0,
                    atr: 0.0,
                    high_52wk: 0.0,
                    low_52wk: 0.0,
                    day_high: 0.0,
                    day_low: 0.0,
                    avg_daily_range: 2.0,
                    earnings_days: -1,
                    alert_triggered: false,
                    price_history: vec![],
                });
            }

            let id_u32 = (sec_idx as u32) + 1;
            if id_u32 >= next_section_id { next_section_id = id_u32 + 1; }
            sections.push(WatchlistSection {
                id: id_u32,
                title,
                color,
                collapsed,
                items,
            });
        }

        out.push(SavedWatchlist {
            name,
            sections,
            next_section_id: next_section_id.max(2),
        });
    }

    if !out.is_empty() && active_idx >= out.len() {
        active_idx = 0;
    }
    Ok((out, active_idx))
}

// ─────────────────────────────────────────────────────────────────────────
// Symbol universes
// ─────────────────────────────────────────────────────────────────────────

/// Upsert a universe by `name`, replacing all members.
pub async fn save_universe(
    pool: &PgPool,
    name: &str,
    display_name: &str,
    kind: &str,
    source: &str,
    members: &[(String, Option<f32>)],
) -> Result<(), DbCodecError> {
    let mut tx = pool.begin().await?;

    // Upsert by name
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO symbol_universes (kind, name, display_name, source, fetched_at) \
         VALUES ($1, $2, $3, $4, NOW()) \
         ON CONFLICT (name) DO UPDATE SET \
           kind = EXCLUDED.kind, \
           display_name = EXCLUDED.display_name, \
           source = EXCLUDED.source, \
           fetched_at = NOW() \
         RETURNING id",
    )
    .bind(kind)
    .bind(name)
    .bind(display_name)
    .bind(source)
    .fetch_one(&mut *tx)
    .await?;

    sqlx::query("DELETE FROM symbol_universe_members WHERE universe_id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await?;

    for (idx, (sym, weight)) in members.iter().enumerate() {
        sqlx::query(
            "INSERT INTO symbol_universe_members (universe_id, symbol, weight, position) \
             VALUES ($1, $2, $3, $4)",
        )
        .bind(id)
        .bind(sym)
        .bind(*weight)
        .bind(idx as i32)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(())
}

/// Load just the symbols for a universe, in stored position order.
pub async fn load_universe(pool: &PgPool, name: &str) -> Result<Vec<String>, DbCodecError> {
    let rows = sqlx::query(
        "SELECT m.symbol FROM symbol_universe_members m \
         JOIN symbol_universes u ON u.id = m.universe_id \
         WHERE u.name = $1 ORDER BY m.position ASC",
    )
    .bind(name)
    .fetch_all(pool)
    .await?;

    let mut out = Vec::with_capacity(rows.len());
    for r in rows {
        if let Ok(s) = r.try_get::<String, _>("symbol") {
            out.push(s);
        }
    }
    Ok(out)
}

/// Look up the `fetched_at` timestamp for a universe. Returns `None` if
/// the universe doesn't exist yet (never seeded). Used by the refresh job
/// to skip universes fetched in the last N days.
pub async fn universe_fetched_at(
    pool: &PgPool,
    name: &str,
) -> Result<Option<chrono::DateTime<chrono::Utc>>, DbCodecError> {
    let row: Option<(chrono::DateTime<chrono::Utc>,)> = sqlx::query_as(
        "SELECT fetched_at FROM symbol_universes WHERE name = $1",
    )
    .bind(name)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|(t,)| t))
}

/// List all universes (or just those of a given `kind`). Returns `(name, display_name)`.
pub async fn list_universes(
    pool: &PgPool,
    kind: Option<&str>,
) -> Result<Vec<(String, String)>, DbCodecError> {
    let rows = match kind {
        Some(k) => sqlx::query_as::<_, (String, String)>(
            "SELECT name, display_name FROM symbol_universes \
             WHERE kind = $1 ORDER BY display_name ASC",
        )
        .bind(k)
        .fetch_all(pool)
        .await?,
        None => sqlx::query_as::<_, (String, String)>(
            "SELECT name, display_name FROM symbol_universes ORDER BY display_name ASC",
        )
        .fetch_all(pool)
        .await?,
    };
    Ok(rows)
}
