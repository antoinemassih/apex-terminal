//! Foundation types — owned data shapes shared across the app.
//!
//! Wave 3 split `data_types.rs` into a directory so typed primitives
//! (`Price`, `Timestamp`, `Symbol`) live alongside the legacy wire types
//! (`Bar`, `OptionContract`, `OptionsChain`) without growing one giant file.
//!
//! Existing re-exports stay flat via `pub use` — downstream code that
//! `use crate::foundation::*`-ed the old module is unaffected.

pub mod price;
pub mod timestamp;
pub mod symbol;
pub mod fundamentals;
pub mod news;
pub mod earnings;
pub mod corporate_actions;

pub use price::Price;
pub use timestamp::{Timestamp, TimeSource};
pub use symbol::{Symbol, SymbolRegistry, Vendor, AssetClass, registry};
pub use fundamentals::Fundamentals;
pub use news::NewsItem;
pub use earnings::{EarningsItem, EarningsWhen};
pub use corporate_actions::{CorporateAction, CorporateActionKind};

// `symbol_or_guess` is defined further down in this module (after `is_crypto`)
// but re-exported here for convenience so callers can `use foundation::types::symbol_or_guess`.

use serde::{Deserialize, Serialize};

/// Detect crypto symbols (Binance pairs)
pub fn is_crypto(symbol: &str) -> bool {
    let s = symbol.to_uppercase();
    s.ends_with("USDT") || s.ends_with("BUSD") || s.ends_with("USDC")
        || s.ends_with("BTC") && s.len() > 3 && s != "GBTC"
}

/// Resolve a canonical ticker to a typed `Symbol`, preferring the global
/// `SymbolRegistry` (populated from index/ETF constituents at startup) and
/// falling back to string-pattern heuristics for tickers the registry has
/// never seen.
///
/// This is the bridge point that lets call sites move off the brittle
/// `is_crypto(&str)` heuristic onto registry-backed truth — once a ticker is
/// registered as `Equity` the heuristic's `"XUSDT" → crypto` misclassification
/// stops mattering.
pub fn symbol_or_guess(canonical: &str) -> Symbol {
    if let Some(s) = registry().get(canonical) { return s; }
    if canonical.starts_with("O:") {
        Symbol::option(canonical)
    } else if is_crypto(canonical) {
        Symbol::crypto(canonical)
    } else {
        Symbol::equity(canonical)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bar {
    pub time: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptionContract {
    pub strike: f64,
    #[serde(rename = "lastPrice")]
    pub last_price: f64,
    pub bid: f64,
    pub ask: f64,
    pub volume: i64,
    #[serde(rename = "openInterest")]
    pub open_interest: i64,
    #[serde(rename = "impliedVolatility")]
    pub implied_volatility: f64,
    #[serde(rename = "inTheMoney")]
    pub in_the_money: bool,
    #[serde(rename = "contractSymbol")]
    pub contract_symbol: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptionsChain {
    pub expirations: Vec<String>,
    pub date: Option<String>,
    pub calls: Vec<OptionContract>,
    pub puts: Vec<OptionContract>,
}

// ── Input validation helpers ─────────────────────────────────────────────────

/// Validate a ticker symbol.
///
/// Accepts: uppercase letters, digits, colon (options `O:...`), dot
/// (share-class suffix like `BRK.B`), and hyphen (warrants etc.).
/// Length: 1–32 chars.
fn validate_symbol(symbol: &str) -> Result<(), crate::error::AppError> {
    if symbol.is_empty() || symbol.len() > 32 {
        return Err(crate::error::AppError::invalid_input(
            "symbol must be 1–32 characters",
        ));
    }
    if !symbol
        .chars()
        .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || matches!(c, ':' | '.' | '-'))
    {
        return Err(crate::error::AppError::invalid_input(
            "symbol contains invalid characters (expected A-Z 0-9 : . -)",
        ));
    }
    Ok(())
}

/// Validate a bar interval against a known-good list.
fn validate_interval(interval: &str) -> Result<(), crate::error::AppError> {
    const VALID: &[&str] = &[
        "1m", "2m", "5m", "15m", "30m", "60m", "1h", "4h",
        "1d", "1wk", "1w", "1mo",
    ];
    if VALID.contains(&interval) {
        Ok(())
    } else {
        Err(crate::error::AppError::invalid_input(
            "interval is not a recognised value",
        ))
    }
}

/// Validate a bar period against a known-good list.
fn validate_period(period: &str) -> Result<(), crate::error::AppError> {
    const VALID: &[&str] = &[
        "1d", "5d", "1mo", "3mo", "6mo",
        "1y", "2y", "5y", "10y", "ytd", "max",
    ];
    if VALID.contains(&period) {
        Ok(())
    } else {
        Err(crate::error::AppError::invalid_input(
            "period is not a recognised value",
        ))
    }
}

/// Validate an optional date string in `YYYY-MM-DD` format.
fn validate_date(date: &str) -> Result<(), crate::error::AppError> {
    let bytes = date.as_bytes();
    let ok = bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes[..4].iter().all(|b| b.is_ascii_digit())
        && bytes[5..7].iter().all(|b| b.is_ascii_digit())
        && bytes[8..].iter().all(|b| b.is_ascii_digit());
    if ok {
        Ok(())
    } else {
        Err(crate::error::AppError::invalid_input(
            "date must be YYYY-MM-DD",
        ))
    }
}

// ── Commands ─────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn get_bars(
    symbol: String,
    interval: String,
    period: String,
) -> Result<Vec<Bar>, crate::error::AppError> {
    use crate::error::AppError;

    // ── Whitelist validation (defense-in-depth, before any URL construction) ──
    validate_symbol(&symbol)?;
    validate_interval(&interval)?;
    validate_period(&period)?;

    // ── Belt-and-suspenders URL encoding ──────────────────────────────────────
    let enc_symbol   = urlencoding::encode(&symbol);
    let enc_interval = urlencoding::encode(&interval);
    let enc_period   = urlencoding::encode(&period);

    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0")
        .build()
        .map_err(|e| AppError::network_error(e))?;

    // 0. Crypto → ApexCrypto directly (manages its own cache + Binance backfill)
    if is_crypto(&symbol) {
        let apex_url = format!("http://192.168.1.56:30840/api/bars/{}/{}", enc_symbol, enc_interval);
        if let Ok(resp) = client.get(&apex_url).timeout(std::time::Duration::from_secs(5)).send().await {
            if let Ok(bars) = resp.json::<Vec<Bar>>().await {
                if !bars.is_empty() {
                    eprintln!("[get_bars] {} bars for {} from ApexCrypto", bars.len(), symbol);
                    return Ok(bars);
                }
            }
        }
        return Ok(vec![]);
    }

    // 1. Redis cache (stocks only)
    if let Some(cached) = crate::bar_cache::get(&symbol, &interval) {
        if !cached.is_empty() {
            eprintln!("[get_bars] Cache hit for {}:{} ({} bars)", symbol, interval, cached.len());
            return Ok(cached);
        }
    }

    // 2. OCOCO
    let ococo_url = format!("http://192.168.1.60:30300/api/bars?symbol={}&interval={}&limit=500", enc_symbol, enc_interval);
    if let Ok(resp) = client.get(&ococo_url).timeout(std::time::Duration::from_secs(2)).send().await {
        if let Ok(bars) = resp.json::<Vec<Bar>>().await {
            if !bars.is_empty() {
                crate::bar_cache::set(&symbol, &interval, &bars);
                return Ok(bars);
            }
        }
    }

    // 3. yfinance sidecar
    let yf_url = format!("http://127.0.0.1:8777/bars?symbol={}&interval={}&period={}", enc_symbol, enc_interval, enc_period);
    if let Ok(resp) = client.get(&yf_url).timeout(std::time::Duration::from_secs(3)).send().await {
        if let Ok(bars) = resp.json::<Vec<Bar>>().await {
            if !bars.is_empty() {
                crate::bar_cache::set(&symbol, &interval, &bars);
                return Ok(bars);
            }
        }
    }

    // 4. Direct Yahoo Finance v8 API
    let (yf_interval, yf_range) = match interval.as_str() {
        "1m" => ("1m","5d"), "2m" => ("2m","5d"), "5m" => ("5m","5d"),
        "15m" => ("15m","60d"), "30m" => ("30m","60d"),
        "1h" | "60m" => ("60m","60d"), "4h" => ("1h","730d"),
        "1d" => ("1d","5y"), "1wk" => ("1wk","10y"),
        _ => (interval.as_str(), &*period),
    };
    // yf_interval/yf_range are our own constants — encode enc_symbol only.
    let yahoo_url = format!(
        "https://query1.finance.yahoo.com/v8/finance/chart/{}?interval={}&range={}",
        enc_symbol, yf_interval, yf_range
    );
    if let Ok(resp) = client.get(&yahoo_url).timeout(std::time::Duration::from_secs(5)).send().await {
        if let Ok(json) = resp.json::<serde_json::Value>().await {
            if let Some(bars) = parse_yahoo_v8(&json) {
                crate::bar_cache::set(&symbol, &interval, &bars);
                return Ok(bars);
            }
        }
    }

    Ok(vec![])
}

/// Parse Yahoo Finance v8 chart JSON response into a Bar vec.
pub fn parse_yahoo_v8(json: &serde_json::Value) -> Option<Vec<Bar>> {
    let result = json.get("chart")?.get("result")?.get(0)?;
    let timestamps = result.get("timestamp")?.as_array()?;
    let quote = result.get("indicators")?.get("quote")?.get(0)?;
    let opens = quote.get("open")?.as_array()?;
    let highs = quote.get("high")?.as_array()?;
    let lows = quote.get("low")?.as_array()?;
    let closes = quote.get("close")?.as_array()?;
    let volumes = quote.get("volume")?.as_array()?;
    let mut bars = Vec::with_capacity(timestamps.len());
    for i in 0..timestamps.len() {
        let o = opens.get(i).and_then(|v| v.as_f64());
        let h = highs.get(i).and_then(|v| v.as_f64());
        let l = lows.get(i).and_then(|v| v.as_f64());
        let c = closes.get(i).and_then(|v| v.as_f64());
        let v = volumes.get(i).and_then(|v| v.as_f64()).unwrap_or(0.0);
        let t = timestamps.get(i).and_then(|v| v.as_i64()).unwrap_or(0);
        if let (Some(o), Some(h), Some(l), Some(c)) = (o, h, l, c) {
            bars.push(Bar { time: t, open: o, high: h, low: l, close: c, volume: v });
        }
    }
    if bars.is_empty() { None } else { Some(bars) }
}

#[tauri::command]
pub async fn get_options_chain(
    symbol: String,
    date: Option<String>,
) -> Result<OptionsChain, crate::error::AppError> {
    use crate::error::AppError;

    // ── Whitelist validation ───────────────────────────────────────────────────
    validate_symbol(&symbol)?;
    if let Some(ref d) = date {
        validate_date(d)?;
    }

    // ── Belt-and-suspenders URL encoding ──────────────────────────────────────
    let enc_symbol = urlencoding::encode(&symbol);
    let mut url = format!("http://127.0.0.1:8777/options?symbol={}", enc_symbol);
    if let Some(ref d) = date {
        url.push_str(&format!("&date={}", urlencoding::encode(d)));
    }

    let resp = reqwest::get(&url)
        .await
        .map_err(|e| AppError::network_error(e))?;
    let chain: OptionsChain = resp
        .json()
        .await
        .map_err(|e| AppError::parse_error(e))?;
    Ok(chain)
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod input_validation_tests {
    use super::{validate_symbol, validate_interval, validate_period, validate_date};

    // ── get_bars: symbol injection ────────────────────────────────────────────

    #[test]
    fn get_bars_rejects_symbol_with_query_inject() {
        // "AAPL&foo=bar" would inject an extra query param without validation.
        let result = validate_symbol("AAPL&foo=bar");
        assert!(
            result.is_err(),
            "expected Err for symbol with query injection"
        );
        assert_eq!(result.unwrap_err().code, "invalid_input");
    }

    #[test]
    fn get_bars_rejects_symbol_with_path_traversal() {
        // "AAPL/../admin" would escape the URL path segment.
        let result = validate_symbol("AAPL/../admin");
        assert!(
            result.is_err(),
            "expected Err for symbol with path traversal"
        );
        assert_eq!(result.unwrap_err().code, "invalid_input");
    }

    // ── get_bars: interval whitelist ──────────────────────────────────────────

    #[test]
    fn get_bars_rejects_unknown_interval() {
        let result = validate_interval("99x");
        assert!(result.is_err(), "expected Err for unrecognised interval");
        assert_eq!(result.unwrap_err().code, "invalid_input");
    }

    // ── get_options_chain: date format ────────────────────────────────────────

    #[test]
    fn get_options_chain_rejects_malformed_date() {
        // SQL/shell injection attempt disguised as a date.
        for bad in &["2025/06/01", "20250601", "2025-6-1", "'; DROP TABLE--"] {
            let result = validate_date(bad);
            assert!(
                result.is_err(),
                "expected Err for malformed date {:?}",
                bad
            );
        }
    }

    // ── Sanity: good inputs must still pass ───────────────────────────────────

    #[test]
    fn get_bars_accepts_aapl_5m_5d() {
        assert!(validate_symbol("AAPL").is_ok());
        assert!(validate_interval("5m").is_ok());
        assert!(validate_period("5d").is_ok());
    }

    #[test]
    fn get_options_chain_accepts_o_prefix_option() {
        // Options symbols use the "O:" prefix and are up to 32 chars.
        assert!(validate_symbol("O:SPY251219C00450000").is_ok());
        // Date in YYYY-MM-DD format should also pass.
        assert!(validate_date("2025-12-19").is_ok());
    }
}
