//! Morning-brief toast + 3:45 trade-idea enrichment cache.
//!
//! ## Morning-brief toast
//!
//! At startup (and on each session-day change), fetches `GET /admin/digest`
//! from ApexSignals (`APEX_SIGNALS_HTTP`, default `http://localhost:8100`).
//! When the `brief` field is present and non-empty, pushes it to
//! `apex_data::live_state::push_toast` so the existing toast overlay in
//! `top_nav.rs` surfaces it. The toast degrades silently when ApexSignals is
//! unreachable or the brief is absent.
//!
//! ## Trade-idea enrichment cache
//!
//! The same `GET /admin/digest` response carries a `trade_ideas` array (one
//! entry per parent, written by `TradeIdeaEngine` to `signals:trade_idea:{SYMBOL}`).
//! The cache is refreshed every 5 minutes so that the 3:45 badge in
//! `signals_v2_feed.rs` can look up structure/strike/expiry/breakeven without
//! making a network call on the hot badge path.
//!
//! ## Env vars
//!
//! | Variable | Default | Purpose |
//! |---|---|---|
//! | `APEX_SIGNALS_HTTP` | `http://localhost:8100` | ApexSignals REST base URL |
//! | `BRIEF_FEED_ENABLED` | `1` (on) | Set to `0` to disable entirely |
//!
//! ## Degrade behaviour
//!
//! - Network failure → silent (no toast, no error surface; polled again next cycle)
//! - Empty/null brief → silent (no toast)
//! - Empty trade_ideas → cache stays empty; badge enrichment falls back to plain msg

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

// ── Shared HTTP client for ApexSignals REST calls ──────────────────────────

fn apex_signals_http() -> String {
    std::env::var("APEX_SIGNALS_HTTP")
        .unwrap_or_else(|_| "http://localhost:8100".to_string())
}

fn http_client() -> &'static reqwest::blocking::Client {
    static CLIENT: OnceLock<reqwest::blocking::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(5))
            .user_agent("apex-terminal/brief-feed")
            .build()
            .unwrap_or_else(|_| reqwest::blocking::Client::new())
    })
}

// ── Trade-idea summary (fields the badge needs) ─────────────────────────────

/// Minimal trade-idea fields needed for badge enrichment.
/// Mirrors the relevant subset of `TradeIdeaSignal` from ApexSignals.
#[derive(Clone, Debug)]
pub struct TradeIdeaSummary {
    /// e.g. "long_put", "put_spread"
    pub structure: String,
    /// Long leg strike price ($)
    pub long_strike: f64,
    /// Approximate days to expiry
    pub dte: f64,
    /// Expiry as YYYY-MM-DD string (best-effort from epoch; empty if absent)
    pub expiry_label: String,
    /// Break-even price at expiry ($)
    pub breakeven: f64,
    /// Risk/reward estimate
    pub rr_estimate: f64,
    /// When this entry was last refreshed (for staleness check)
    pub fetched_at: Instant,
}

// ── Trade-idea cache ─────────────────────────────────────────────────────────

/// Global trade-idea cache: keyed by upper-case parent symbol (e.g. "QQQ").
/// Updated by the background poller; read by `signals_v2_feed` on the badge path.
static TRADE_IDEA_CACHE: OnceLock<Mutex<HashMap<String, TradeIdeaSummary>>> = OnceLock::new();

fn trade_idea_cache() -> &'static Mutex<HashMap<String, TradeIdeaSummary>> {
    TRADE_IDEA_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Look up a fresh trade idea for `symbol` (upper-case). Returns `None` if
/// absent or older than `stale_secs` seconds. Called by `signals_v2_feed`.
pub fn get_trade_idea(symbol: &str, stale_secs: u64) -> Option<TradeIdeaSummary> {
    trade_idea_cache().lock().ok().and_then(|g| {
        g.get(&symbol.to_uppercase()).and_then(|ti| {
            if ti.fetched_at.elapsed().as_secs() < stale_secs {
                Some(ti.clone())
            } else {
                None
            }
        })
    })
}

// ── Session-day tracking ─────────────────────────────────────────────────────

/// Returns the current local date as "YYYY-MM-DD".
fn today_label() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    // Simple epoch → date (UTC; good enough for session-day change detection)
    let days = now / 86400;
    let y = 1970u64;
    // Fast-path: use chrono if available, else use a simple approximation.
    // We only need it to detect "has the day changed?" — exact date formatting
    // is nice but not critical; the label is only used for dedup.
    let _ = y; // suppress unused warning when chrono path is used
    format!("{}", days) // day count since epoch — changes at UTC midnight
}

// ── Digest fetch ─────────────────────────────────────────────────────────────

/// Fetch `GET /admin/digest` and return the raw JSON value. Returns `None` on
/// any error (network, HTTP, parse). The caller interprets the fields it needs.
fn fetch_digest() -> Option<serde_json::Value> {
    let url = format!("{}/admin/digest", apex_signals_http());
    let resp = http_client().get(&url).send().ok()?;
    if !resp.status().is_success() {
        return None;
    }
    resp.json::<serde_json::Value>().ok()
}

/// Extract the brief text from a digest JSON response.
/// Returns `None` when absent or blank.
fn extract_brief(digest: &serde_json::Value) -> Option<String> {
    // `brief` may be a plain string, a JSON-encoded string, or a JSON object
    // with a `text` field. We try the common shapes in order.
    let brief = digest.get("brief")?;
    if let Some(s) = brief.as_str() {
        let s = s.trim().to_string();
        if !s.is_empty() { return Some(s); }
    }
    // Object with a `text` key (e.g. { "text": "...", "score": ... })
    if let Some(obj) = brief.as_object() {
        if let Some(serde_json::Value::String(s)) = obj.get("text") {
            let s = s.trim().to_string();
            if !s.is_empty() { return Some(s); }
        }
        // Fallback: join values that look like strings
        let parts: Vec<&str> = obj.values()
            .filter_map(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .collect();
        if !parts.is_empty() { return Some(parts.join(" | ")); }
    }
    None
}

/// Parse a Unix epoch-seconds f64 into a "YYYY-MM-DD" label (UTC, approximate).
fn epoch_to_date(epoch_secs: f64) -> String {
    if epoch_secs <= 0.0 { return String::new(); }
    let secs = epoch_secs as u64;
    // Days since 1970-01-01
    let days = secs / 86400;
    // Rough Gregorian: good enough for a label
    let mut y = 1970u64;
    let mut remaining = days;
    loop {
        let leap = (y % 4 == 0 && (y % 100 != 0 || y % 400 == 0)) as u64;
        let dy = 365 + leap;
        if remaining < dy { break; }
        remaining -= dy;
        y += 1;
    }
    let leap = (y % 4 == 0 && (y % 100 != 0 || y % 400 == 0)) as u64;
    let month_days = [31u64, 28 + leap, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut m = 1u64;
    for &md in &month_days {
        if remaining < md { break; }
        remaining -= md;
        m += 1;
    }
    format!("{y:04}-{m:02}-{:02}", remaining + 1)
}

/// Update the trade-idea cache from a digest JSON value.
fn update_trade_idea_cache(digest: &serde_json::Value) {
    let now = Instant::now();
    let arr = match digest.get("trade_ideas").and_then(|v| v.as_array()) {
        Some(a) => a,
        None => return,
    };
    let Ok(mut cache) = trade_idea_cache().lock() else { return };
    for item in arr {
        let symbol = item.get("symbol")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_uppercase();
        if symbol.is_empty() { continue; }

        let structure = item.get("structure").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let long_strike = item.get("long_strike").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let dte = item.get("dte").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let breakeven = item.get("breakeven").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let rr_estimate = item.get("rr_estimate").and_then(|v| v.as_f64()).unwrap_or(0.0);
        // expiry: prefer expiry_epoch (secs), fall back to expiry_label string
        let expiry_label = if let Some(ep) = item.get("expiry_epoch").and_then(|v| v.as_f64()) {
            epoch_to_date(ep)
        } else {
            item.get("expiry").and_then(|v| v.as_str()).unwrap_or("").to_string()
        };

        cache.insert(symbol, TradeIdeaSummary {
            structure,
            long_strike,
            dte,
            expiry_label,
            breakeven,
            rr_estimate,
            fetched_at: now,
        });
    }
}

// ── Background poller ─────────────────────────────────────────────────────────

static STARTED: OnceLock<()> = OnceLock::new();

/// Spawn the brief-feed background poller. Idempotent — safe to call once at
/// startup. Opt-out via `BRIEF_FEED_ENABLED=0`.
pub fn start() {
    if std::env::var("BRIEF_FEED_ENABLED").map(|v| v == "0").unwrap_or(false) {
        return;
    }
    if STARTED.set(()).is_err() {
        return;
    }

    crate::foundation::guard::spawn_guarded("brief_feed", || {
        // Track which session-day we last pushed a toast for (dedup: push once
        // per day, not once per restart).
        let mut last_brief_day = String::new();
        // Refresh trade-idea cache every 5 minutes; brief toast once per day.
        let cache_interval = Duration::from_secs(5 * 60);
        let mut last_cache_refresh = Instant::now() - cache_interval; // force immediate first fetch

        loop {
            let now_day = today_label();
            let need_cache = last_cache_refresh.elapsed() >= cache_interval;

            if need_cache || now_day != last_brief_day {
                if let Some(digest) = fetch_digest() {
                    // Update trade-idea cache every cycle
                    update_trade_idea_cache(&digest);
                    last_cache_refresh = Instant::now();

                    // Push brief toast once per session day
                    if now_day != last_brief_day {
                        if let Some(brief) = extract_brief(&digest) {
                            // Prepend a marker so the user knows this is the morning brief.
                            // Truncate to avoid a multi-paragraph toast swallowing the screen.
                            const MAX_BRIEF: usize = 400;
                            let text = if brief.chars().count() > MAX_BRIEF {
                                let mut s: String = brief.chars().take(MAX_BRIEF).collect();
                                s.push_str("…");
                                s
                            } else {
                                brief
                            };
                            let toast_msg = format!("MORNING BRIEF: {text}");
                            crate::data::apex_data::live_state::push_toast(toast_msg);
                            crate::wake_native_ui();
                        }
                        // Mark day as seen even if brief was absent (avoids hammering
                        // the endpoint every minute on days with no brief).
                        last_brief_day = now_day;
                    }
                }
            }

            std::thread::sleep(Duration::from_secs(60));
        }
    });
}
