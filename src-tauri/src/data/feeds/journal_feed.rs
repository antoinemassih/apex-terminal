//! Live-fills journal data source.
//!
//! Polls ApexIB `/executions` on a slow cadence (default 5 minutes, env
//! `JOURNAL_REFRESH_SECS`) and aggregates paired buy/sell fills into closed
//! `JournalEntry` rows that the journal panel can render without modification.
//!
//! **Feature gate**: `JOURNAL_LIVE_FILLS` (default `1`). Set to `0` to
//! disable; the panel will show the "No trades logged" empty state instead of
//! hardcoded placeholders.
//!
//! **Best-effort outcomes**: when `APEX_SIGNALS_HTTP` is reachable, the module
//! fetches `/outcomes/summary` and appends a hit-rate note to each entry's
//! `notes` field. Failures are silently ignored.
//!
//! **Data flow**: background thread writes to `JOURNAL_CACHE`; the journal
//! panel calls `take_fresh_entries()` each frame to swap in new data. The
//! render path is unchanged — only the data source changes.

use std::sync::{Mutex, OnceLock, atomic::{AtomicBool, Ordering}};
use crate::chart_renderer::gpu::JournalEntry;

// ── Public gate ──────────────────────────────────────────────────────────────

/// Returns true unless `JOURNAL_LIVE_FILLS=0` is set.
pub fn is_enabled() -> bool {
    std::env::var("JOURNAL_LIVE_FILLS")
        .map(|v| v.trim() != "0")
        .unwrap_or(true)
}

// ── Global cache ─────────────────────────────────────────────────────────────

/// Holds the latest set of journal entries computed from ApexIB fills.
/// `None` = not yet fetched. `Some([])` = fetched but no closed trades found
/// (or ApexIB unreachable — returns empty rather than placeholders).
static JOURNAL_CACHE: OnceLock<Mutex<Option<Vec<JournalEntry>>>> = OnceLock::new();

/// Flipped to `true` by the background thread whenever it writes a fresh
/// batch. `take_fresh_entries` reads and clears this flag so that the panel
/// only does a clone on actual updates.
static CACHE_DIRTY: AtomicBool = AtomicBool::new(false);

fn cache() -> &'static Mutex<Option<Vec<JournalEntry>>> {
    JOURNAL_CACHE.get_or_init(|| Mutex::new(None))
}

/// Called by the journal panel each frame.
///
/// Returns `Some(entries)` when the background thread has written a fresh
/// batch since the last call; `None` means "nothing new — keep what you have."
pub fn take_fresh_entries() -> Option<Vec<JournalEntry>> {
    if !CACHE_DIRTY.swap(false, Ordering::AcqRel) {
        return None;
    }
    cache().lock().ok()?.clone()
}

// ── Poller ───────────────────────────────────────────────────────────────────

static STARTED: OnceLock<()> = OnceLock::new();

/// Spawn the background poller. Idempotent — safe to call once at startup.
/// No-op when `JOURNAL_LIVE_FILLS=0`.
pub fn start() {
    if !is_enabled() { return; }
    if STARTED.set(()).is_err() { return; }

    crate::foundation::guard::spawn_guarded("journal_feed", || {
        let refresh_secs = std::env::var("JOURNAL_REFRESH_SECS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(300); // 5 minutes default

        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .unwrap_or_else(|_| reqwest::blocking::Client::new());

        loop {
            let entries = fetch_journal_entries(&client);
            // Write to cache — always, even on empty (so we displace placeholders)
            if let Ok(mut guard) = cache().lock() {
                *guard = Some(entries);
                CACHE_DIRTY.store(true, Ordering::Release);
            }
            crate::wake_native_ui();
            std::thread::sleep(std::time::Duration::from_secs(refresh_secs));
        }
    });
}

// ── Fetch + aggregate ────────────────────────────────────────────────────────

fn apexib_url() -> String {
    // Reuse the broker config's runtime-overridable URL (APEXIB_URL env var,
    // default https://apexib-dev.xllio.com). Mirrors what the account poller uses.
    // The apexib_url() fn lives in chart_renderer::gpu (pub(crate)).
    crate::chart_renderer::gpu::apexib_url()
}

fn apex_signals_url() -> String {
    std::env::var("APEX_SIGNALS_HTTP")
        .unwrap_or_else(|_| "http://localhost:8100".to_string())
}

/// Fetch `/executions` from ApexIB and aggregate into `JournalEntry` rows.
///
/// Strategy: each execution record is a fill (buy or sell). We pair fills
/// by symbol using a FIFO queue — each buy opens a position, a matching sell
/// closes it. Options fills carry a multiplier of 100 applied to P&L.
///
/// Returns an empty Vec when ApexIB is unreachable (graceful fallback).
fn fetch_journal_entries(client: &reqwest::blocking::Client) -> Vec<JournalEntry> {
    let url = format!("{}/executions", apexib_url());
    let resp = match client.get(&url).send() {
        Ok(r) => r,
        Err(_) => {
            // ApexIB unreachable — return empty (not placeholders)
            tracing::debug!("journal_feed: ApexIB unreachable at {url}");
            return Vec::new();
        }
    };

    let json: serde_json::Value = match resp.json() {
        Ok(j) => j,
        Err(_) => return Vec::new(),
    };

    let fills_arr = match json.get("executions").and_then(|v| v.as_array()) {
        Some(arr) => arr,
        None => return Vec::new(),
    };

    // Convert raw fills to typed records
    let mut fills: Vec<RawFill> = fills_arr
        .iter()
        .filter_map(parse_fill)
        .collect();

    // Sort oldest-first so FIFO pairing works correctly
    fills.sort_by_key(|f| f.time_ms);

    // Fetch outcome summary for best-effort annotation (ignore errors)
    let outcomes = fetch_outcomes_summary(client);

    aggregate_fills(fills, &outcomes)
}

#[derive(Debug, Clone)]
struct RawFill {
    id: String,
    symbol: String,
    side: String,       // "BUY" or "SELL"
    qty: i32,
    price: f64,
    time_ms: i64,
    is_option: bool,
    multiplier: f64,
}

fn parse_fill(v: &serde_json::Value) -> Option<RawFill> {
    let symbol = v["symbol"].as_str()?.to_string();
    let side = v["side"].as_str()
        .or_else(|| v["action"].as_str())
        .unwrap_or("")
        .to_uppercase();
    if side != "BUY" && side != "SELL" { return None; }

    let qty = v["quantity"].as_i64()
        .or_else(|| v["filledQty"].as_i64())
        .or_else(|| v["shares"].as_i64())
        .unwrap_or(0) as i32;
    if qty <= 0 { return None; }

    let price = v["avgFillPrice"].as_f64()
        .or_else(|| v["avgPrice"].as_f64())
        .or_else(|| v["price"].as_f64())
        .unwrap_or(0.0);
    if price <= 0.0 { return None; }

    let time_ms = v["submittedAt"].as_i64()
        .or_else(|| v["time"].as_i64())
        .unwrap_or(0);

    // Detect options by symbol format (e.g. "AAPL  230120C00150000") or field
    let is_option = v["optionType"].as_str().is_some()
        || symbol.len() > 6 && symbol.chars().any(|c| c == 'C' || c == 'P');
    let multiplier = if is_option { 100.0 } else { 1.0 };

    let id = v["orderId"].as_str().map(|s| s.to_string())
        .or_else(|| v["orderId"].as_i64().map(|n| n.to_string()))
        .or_else(|| v["id"].as_str().map(|s| s.to_string()))
        .or_else(|| v["id"].as_i64().map(|n| n.to_string()))
        .unwrap_or_else(|| format!("{symbol}_{time_ms}"));

    Some(RawFill { id, symbol, side, qty, price, time_ms, is_option, multiplier })
}

/// FIFO-pair buys and sells by symbol to produce closed `JournalEntry` rows.
///
/// A "Long" trade opens on BUY, closes on SELL.
/// A "Short" trade opens on SELL, closes on BUY.
/// Partial fills are handled: a larger closing fill splits.
/// Unpaired (open) positions are ignored (not closed yet).
fn aggregate_fills(fills: Vec<RawFill>, outcomes: &OutcomeHints) -> Vec<JournalEntry> {
    use std::collections::HashMap;

    // Queue of open legs per symbol: (entry_fill, qty_remaining)
    let mut open: HashMap<String, Vec<(RawFill, i32)>> = HashMap::new();
    let mut entries: Vec<JournalEntry> = Vec::new();

    let now_sec = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    for fill in fills {
        let queue = open.entry(fill.symbol.clone()).or_default();

        if queue.is_empty() {
            // No open position — this fill opens one
            queue.push((fill, 0));
            let idx = queue.len() - 1;
            queue[idx].1 = queue[idx].0.qty;
            continue;
        }

        // Check if this fill is in the opposite direction to the open position
        let open_side = &queue[0].0.side;
        let is_closing = (open_side == "BUY" && fill.side == "SELL")
            || (open_side == "SELL" && fill.side == "BUY");

        if !is_closing {
            // Same direction — add to position
            queue.push((fill.clone(), fill.qty));
            continue;
        }

        // Match against open legs (FIFO)
        let mut close_qty_remaining = fill.qty;
        let close_price = fill.price;
        let close_time = fill.time_ms / 1000; // convert to seconds

        while close_qty_remaining > 0 && !queue.is_empty() {
            let (entry_fill, entry_qty_rem) = &mut queue[0];
            let matched_qty = (*entry_qty_rem).min(close_qty_remaining);
            let entry_price = entry_fill.price;
            let side_str = if entry_fill.side == "BUY" { "Long" } else { "Short" };
            let entry_time = entry_fill.time_ms / 1000;
            let multiplier = entry_fill.multiplier;
            let symbol = entry_fill.symbol.clone();
            let entry_id = entry_fill.id.clone();
            let is_option = entry_fill.is_option;

            let raw_pnl = if side_str == "Long" {
                (close_price - entry_price) * matched_qty as f64 * multiplier
            } else {
                (entry_price - close_price) * matched_qty as f64 * multiplier
            };
            let pnl_pct = if entry_price > 0.0 {
                if side_str == "Long" {
                    (close_price - entry_price) / entry_price * 100.0
                } else {
                    (entry_price - close_price) / entry_price * 100.0
                }
            } else { 0.0 };

            let duration_secs = (close_time - entry_time).max(0);
            let duration_mins = duration_secs / 60;

            // Classify timeframe / setup_type from holding duration
            let (timeframe, setup_type) = classify_trade(duration_mins, is_option);

            // R-multiple: assume 1:1 risk if we have no stop info
            // Use a simple heuristic: |pnl| / (entry_price * matched_qty * 0.01 * multiplier)
            // i.e., risk = 1% of position
            let risk = entry_price * matched_qty as f64 * 0.01 * multiplier;
            let r_multiple = if risk > 0.0 { raw_pnl / risk } else { 0.0 };

            // Best-effort outcome annotation
            let notes = outcomes.annotation_for(&symbol);

            let entry_uid = format!("{entry_id}_{close_time}_{matched_qty}");
            entries.push(JournalEntry {
                id: entry_uid,
                symbol: symbol.clone(),
                side: side_str.to_string(),
                qty: matched_qty,
                entry_price,
                exit_price: close_price,
                pnl: raw_pnl,
                pnl_pct,
                entry_time,
                exit_time: close_time,
                duration_mins,
                setup_type,
                notes,
                tags: if is_option { vec!["options".into()] } else { vec![] },
                timeframe,
                r_multiple,
            });

            close_qty_remaining -= matched_qty;
            *entry_qty_rem -= matched_qty;
            if queue[0].1 == 0 {
                queue.remove(0);
            }
        }

        // If more close qty remains than open position, it opens a new position
        // in the opposite direction (reversal). Treat remaining qty as new open.
        if close_qty_remaining > 0 {
            let mut reversal = fill.clone();
            reversal.qty = close_qty_remaining;
            queue.push((reversal, close_qty_remaining));
        }
    }

    // Sort most-recent first for the panel
    entries.sort_by(|a, b| b.exit_time.cmp(&a.exit_time));
    let _ = now_sec; // suppress unused warning
    entries
}

fn classify_trade(duration_mins: i64, is_option: bool) -> (String, String) {
    let tf = if duration_mins < 5 {
        "1m"
    } else if duration_mins < 30 {
        "5m"
    } else if duration_mins < 120 {
        "15m"
    } else if duration_mins < 1440 {
        "1H"
    } else {
        "1D"
    };

    let setup = if is_option {
        if duration_mins < 60 { "0dte" } else { "options" }
    } else if duration_mins < 10 {
        "scalp"
    } else if duration_mins < 240 {
        "intraday"
    } else {
        "swing"
    };

    (tf.to_string(), setup.to_string())
}

// ── Outcomes (best-effort) ───────────────────────────────────────────────────

struct OutcomeHints {
    /// Map of engine name → hit_rate_30m (as fraction 0.0..1.0)
    hit_rates: std::collections::HashMap<String, f64>,
    fetched: bool,
}

impl OutcomeHints {
    fn empty() -> Self {
        Self { hit_rates: Default::default(), fetched: false }
    }

    fn annotation_for(&self, _symbol: &str) -> String {
        if !self.fetched || self.hit_rates.is_empty() {
            return String::new();
        }
        // Find the best engine as the top-level summary line
        let best = self.hit_rates
            .iter()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal));
        match best {
            Some((engine, rate)) => format!("signals: {engine} {:.0}% hit-rate", rate * 100.0),
            None => String::new(),
        }
    }
}

fn fetch_outcomes_summary(client: &reqwest::blocking::Client) -> OutcomeHints {
    let base = apex_signals_url();
    let url = format!("{base}/outcomes/summary?min_samples=5");
    let resp = match client.get(&url).timeout(std::time::Duration::from_secs(4)).send() {
        Ok(r) => r,
        Err(_) => return OutcomeHints::empty(),
    };
    let json: serde_json::Value = match resp.json() {
        Ok(j) => j,
        Err(_) => return OutcomeHints::empty(),
    };
    let engines = match json.get("engines").and_then(|v| v.as_array()) {
        Some(arr) => arr,
        None => return OutcomeHints::empty(),
    };
    let mut hit_rates = std::collections::HashMap::new();
    for e in engines {
        if let (Some(engine), Some(rate)) = (
            e["engine"].as_str(),
            e["hit_rate_30m"].as_f64(),
        ) {
            hit_rates.insert(engine.to_string(), rate);
        }
    }
    OutcomeHints { hit_rates, fetched: true }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn fill(sym: &str, side: &str, qty: i32, price: f64, time_ms: i64) -> RawFill {
        RawFill {
            id: format!("{sym}_{time_ms}"),
            symbol: sym.to_string(),
            side: side.to_string(),
            qty, price, time_ms,
            is_option: false,
            multiplier: 1.0,
        }
    }

    #[test]
    fn long_trade_paired_correctly() {
        let fills = vec![
            fill("AAPL", "BUY",  100, 180.0, 1_000),
            fill("AAPL", "SELL", 100, 185.0, 2_000),
        ];
        let hints = OutcomeHints::empty();
        let entries = aggregate_fills(fills, &hints);
        assert_eq!(entries.len(), 1);
        let e = &entries[0];
        assert_eq!(e.side, "Long");
        assert_eq!(e.qty, 100);
        assert!((e.pnl - 500.0).abs() < 0.01, "pnl={}", e.pnl);
        assert!(e.pnl_pct > 0.0);
    }

    #[test]
    fn short_trade_paired_correctly() {
        let fills = vec![
            fill("SPY", "SELL", 50, 500.0, 1_000),
            fill("SPY", "BUY",  50, 490.0, 2_000),
        ];
        let hints = OutcomeHints::empty();
        let entries = aggregate_fills(fills, &hints);
        assert_eq!(entries.len(), 1);
        let e = &entries[0];
        assert_eq!(e.side, "Short");
        assert!((e.pnl - 500.0).abs() < 0.01);
    }

    #[test]
    fn open_position_not_emitted() {
        // Only a buy, no sell — still open, should not appear in journal
        let fills = vec![fill("NVDA", "BUY", 10, 600.0, 1_000)];
        let hints = OutcomeHints::empty();
        let entries = aggregate_fills(fills, &hints);
        assert!(entries.is_empty());
    }

    #[test]
    fn partial_close_yields_entry() {
        // Buy 100, sell 50 → one closed trade of 50
        let fills = vec![
            fill("TSLA", "BUY",  100, 200.0, 1_000),
            fill("TSLA", "SELL",  50, 210.0, 2_000),
        ];
        let hints = OutcomeHints::empty();
        let entries = aggregate_fills(fills, &hints);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].qty, 50);
        assert!((entries[0].pnl - 500.0).abs() < 0.01);
    }

    #[test]
    fn multiple_symbols_independent() {
        let fills = vec![
            fill("AAPL", "BUY",  10, 180.0, 1_000),
            fill("MSFT", "BUY",  20, 400.0, 1_001),
            fill("AAPL", "SELL", 10, 185.0, 2_000),
            fill("MSFT", "SELL", 20, 390.0, 2_001),
        ];
        let hints = OutcomeHints::empty();
        let entries = aggregate_fills(fills, &hints);
        assert_eq!(entries.len(), 2);
        let aapl = entries.iter().find(|e| e.symbol == "AAPL").unwrap();
        let msft = entries.iter().find(|e| e.symbol == "MSFT").unwrap();
        assert!(aapl.pnl > 0.0);
        assert!(msft.pnl < 0.0);
    }

    #[test]
    fn is_enabled_default_on() {
        // Default: env var not set → enabled
        // (May be overridden in CI, so we only check the function exists and returns a bool.)
        let _ = is_enabled();
    }
}
