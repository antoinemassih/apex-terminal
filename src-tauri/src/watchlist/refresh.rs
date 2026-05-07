//! Refreshes symbol_universes from Polygon (via ApexData).
//!
//! Called once at app startup. Skips tickers whose universe row was fetched
//! in the last 7 days (constituents change quarterly, so weekly is plenty).
//!
//! All work happens on a background thread — never blocks startup or the
//! render thread. Failures are logged and skipped; the cache simply stays
//! at whatever the previous successful refresh produced.

use crate::data::apex_data::rest as apex_rest;
use crate::persistence::watchlist_db;

/// (polygon_ticker, universe_name, display_name, kind)
const UNIVERSES: &[(&str, &str, &str, &str)] = &[
    ("SPY",  "sp500",      "S&P 500",             "index_constituents"),
    ("DIA",  "dow30",      "Dow 30",              "index_constituents"),
    ("QQQ",  "qqq100",     "Nasdaq 100",          "index_constituents"),
    ("XLK",  "sp500_xlk",  "XLK Technology",      "etf_holdings"),
    ("XLF",  "sp500_xlf",  "XLF Financials",      "etf_holdings"),
    ("XLV",  "sp500_xlv",  "XLV Healthcare",      "etf_holdings"),
    ("XLY",  "sp500_xly",  "XLY Consumer Disc.",  "etf_holdings"),
    ("XLC",  "sp500_xlc",  "XLC Communication",   "etf_holdings"),
    ("XLI",  "sp500_xli",  "XLI Industrials",     "etf_holdings"),
    ("XLE",  "sp500_xle",  "XLE Energy",          "etf_holdings"),
    ("XLP",  "sp500_xlp",  "XLP Consumer Staples","etf_holdings"),
    ("XLU",  "sp500_xlu",  "XLU Utilities",       "etf_holdings"),
    ("XLRE", "sp500_xlre", "XLRE Real Estate",    "etf_holdings"),
    ("XLB",  "sp500_xlb",  "XLB Materials",       "etf_holdings"),
];

const STALE_AFTER_DAYS: i64 = 7;

/// Spawn a thread that walks `UNIVERSES`, fetches holdings via ApexData,
/// saves them to Postgres, and primes the in-memory cache.
pub fn refresh_universes_in_background() {
    std::thread::spawn(|| {
        eprintln!("[universe-refresh] starting (universes: {})", UNIVERSES.len());

        // First, prime the in-memory cache from whatever's already in the DB.
        // The user opens the app → heat panel can render before any Polygon
        // call lands. If the DB is empty too, the panel falls back to its
        // "Loading universe data…" placeholder.
        for (_, name, _, _) in UNIVERSES {
            let symbols = watchlist_db::load_universe(name);
            if !symbols.is_empty() {
                watchlist_db::set_cached_universe(name, symbols);
            }
        }

        // Now do the freshness pass. Skip universes refreshed within the
        // last STALE_AFTER_DAYS — Polygon constituent data only changes
        // quarterly, no need to re-fetch on every app launch.
        let cutoff = chrono::Utc::now() - chrono::Duration::days(STALE_AFTER_DAYS);
        for (poly_ticker, name, display, kind) in UNIVERSES {
            let last = watchlist_db::universe_fetched_at(name);
            if let Some(at) = last {
                if at >= cutoff {
                    eprintln!("[universe-refresh] {name}: fresh (fetched {at}) — skip");
                    continue;
                }
            }

            match apex_rest::fetch_holdings(poly_ticker) {
                Some(rows) if !rows.is_empty() => {
                    eprintln!("[universe-refresh] {name} ← {poly_ticker}: {} holdings", rows.len());
                    watchlist_db::save_universe(name, display, kind, "polygon", &rows);
                    let symbols: Vec<String> = rows.into_iter().map(|(s, _)| s).collect();
                    watchlist_db::set_cached_universe(name, symbols);
                }
                Some(_) => {
                    eprintln!("[universe-refresh] {name} ← {poly_ticker}: empty response, skip");
                }
                None => {
                    eprintln!("[universe-refresh] {name} ← {poly_ticker}: fetch failed, skip");
                }
            }
        }

        eprintln!("[universe-refresh] done");
    });
}
