//! Seed (or refresh) the `symbol_universes` table from Polygon via ApexData.
//!
//! Phase (d): this used to write hardcoded constituent arrays. It now drives
//! the *same* code path as the in-app refresh job
//! (`watchlist::refresh::refresh_universes_in_background`) — fetches each
//! ETF/index ticker through ApexData's `/api/holdings/:ticker`, writes the
//! result via the watchlist_db codec.
//!
//! Run with:
//!   cargo run --example seed_watchlist_universes
//!
//! Requires:
//!   * ApexData reachable at the URL configured in apex_data::config
//!     (defaults from .env / `set_apex_url`)
//!   * Polygon plan that exposes `v3/reference/tickers/{ticker}/holdings`
//!   * Postgres at the connection string below

use _scaffold_lib::data::apex_data::rest as apex_rest;
use _scaffold_lib::watchlist::codec::db::save_universe;
use sqlx::postgres::PgPoolOptions;

/// Same list as `watchlist::refresh::UNIVERSES` — kept locally so this
/// binary stays standalone (it can be run before the app library wires
/// the refresh thread up). If you need to change tickers, update both.
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect("postgresql://postgres:monkeyxx@192.168.1.143:5432/ococo")
        .await?;

    let mut ok = 0usize;
    let mut failed: Vec<&str> = Vec::new();
    for (poly_ticker, name, display, kind) in UNIVERSES {
        // Blocking REST call — fine here since main() is the only thread.
        match apex_rest::fetch_holdings(poly_ticker) {
            Some(rows) if !rows.is_empty() => {
                save_universe(&pool, name, display, kind, "polygon", &rows).await?;
                println!("✓ {name} ← {poly_ticker} ({} holdings)", rows.len());
                ok += 1;
            }
            Some(_) => {
                eprintln!("✗ {name} ← {poly_ticker}: empty response");
                failed.push(name);
            }
            None => {
                eprintln!("✗ {name} ← {poly_ticker}: fetch failed (ApexData unreachable, breaker open, or Polygon plan lacks holdings endpoint)");
                failed.push(name);
            }
        }
    }

    println!("\n{} succeeded, {} failed", ok, failed.len());
    if !failed.is_empty() {
        println!("Failed: {failed:?}");
    }
    println!("\nVerify with:");
    println!("  psql … -c 'SELECT name, display_name, fetched_at FROM symbol_universes ORDER BY name;'");
    Ok(())
}
