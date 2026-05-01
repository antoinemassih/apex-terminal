//! ApexData data models — mirrors §4 of `FRONTEND_INTEGRATION.md`.
//!
//! All time fields are annotated with the unit (seconds vs ms) per the spec.

use serde::{Deserialize, Deserializer, Serialize};

/// Accept `null` or a missing field as `0.0`. Backend returns `null` for quote fields
/// (last/bid/ask/mid/…) when no quote is available; keeping the struct field as plain
/// `f64` lets every existing call site stay unchanged.
fn de_f64_or_zero<'de, D: Deserializer<'de>>(d: D) -> Result<f64, D::Error> {
    Ok(Option::<f64>::deserialize(d)?.unwrap_or(0.0))
}

/// Same trick for `i64` — backend sends `null` for unavailable counters
/// (`day_volume`, `oi_change`, `open_interest`, …).
fn de_i64_or_zero<'de, D: Deserializer<'de>>(d: D) -> Result<i64, D::Error> {
    Ok(Option::<i64>::deserialize(d)?.unwrap_or(0))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AssetClass { Stock, Option }

impl AssetClass {
    pub fn path(self) -> &'static str { match self { Self::Stock => "stocks", Self::Option => "options" } }
    pub fn from_symbol(sym: &str) -> Self {
        if sym.starts_with("O:") { Self::Option } else { Self::Stock }
    }
}

/// §4.1 — chart-facing bar. `time` in **epoch seconds**.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChartBar {
    pub time: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
}

/// §4.2 — wire-form bar (WS + `/api/replay`). `time` in **epoch milliseconds**.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BarWire {
    pub symbol: String,
    pub asset_class: AssetClass,
    pub timeframe: String,
    pub time: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
    #[serde(default)]
    pub vwap: f64,
    #[serde(default)]
    pub trades: u64,
    #[serde(default)]
    pub closed: bool,
}

/// §4.3 — BarUpdate (WS `bar` frame payload).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BarUpdate {
    pub bar: BarWire,
    #[serde(default)]
    pub is_closed: bool,
    /// MARK_BARS_PROTOCOL §"Bar frame — extended": "last" | "mark".
    /// Default to `"last"` for back-compat with servers/messages that omit it.
    #[serde(default = "default_source")]
    pub source: String,
}

fn default_source() -> String { "last".to_string() }

/// MARK_BARS_PROTOCOL: which series a bar/sub belongs to.
/// `Last` = trade-print bars (default). `Mark` = NBBO-mid bars.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BarSource { Last, Mark }

impl BarSource {
    pub fn as_str(self) -> &'static str {
        match self { BarSource::Last => "last", BarSource::Mark => "mark" }
    }
    pub fn from_bool_mark(mark: bool) -> Self {
        if mark { BarSource::Mark } else { BarSource::Last }
    }
}

/// §4.4 — L1 snapshot for watchlist / order ticket.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Snapshot {
    pub symbol: String,
    pub asset_class: AssetClass,
    #[serde(default)] pub last: f64,
    #[serde(default)] pub bid: f64,
    #[serde(default)] pub ask: f64,
    #[serde(default)] pub bid_size: f64,
    #[serde(default)] pub ask_size: f64,
    #[serde(default)] pub spread: f64,
    #[serde(default)] pub day_open: f64,
    #[serde(default)] pub day_high: f64,
    #[serde(default)] pub day_low: f64,
    #[serde(default)] pub day_volume: f64,
    #[serde(default)] pub trades: u64,
    #[serde(default)] pub updated_at_ms: i64,
    #[serde(default)] pub session_date: String,
}

/// §4.5 — NBBO quote.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Quote {
    pub symbol: String,
    pub asset_class: AssetClass,
    #[serde(default)] pub bid: f64,
    #[serde(default)] pub ask: f64,
    #[serde(default)] pub bid_size: f64,
    #[serde(default)] pub ask_size: f64,
    #[serde(default)] pub spread: f64,
    #[serde(default)] pub time: i64,
}

/// §4.6 — Tape print.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Trade {
    pub symbol: String,
    pub asset_class: AssetClass,
    pub price: f64,
    pub qty: f64,
    #[serde(default)] pub time: i64,
}

/// §4.7 + §5.4.d — ChainRow. Greeks nullable; OI/volume included post-v1.
///
/// Serde aliases let us accept both the REST shape (`theta_per_day`) and the
/// chain_delta shape (`theta`) — same field, different names by context.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChainRow {
    pub ticker: String,
    pub underlying: String,
    pub expiry: String,
    pub side: String, // "C" | "P"
    pub strike: f64,

    #[serde(default, deserialize_with = "de_f64_or_zero")] pub last: f64,
    #[serde(default, deserialize_with = "de_f64_or_zero")] pub bid: f64,
    #[serde(default, deserialize_with = "de_f64_or_zero")] pub ask: f64,
    #[serde(default, deserialize_with = "de_f64_or_zero")] pub bid_size: f64,
    #[serde(default, deserialize_with = "de_f64_or_zero")] pub ask_size: f64,
    #[serde(default, deserialize_with = "de_f64_or_zero")] pub mid: f64,

    #[serde(default)] pub iv:    Option<f64>,
    #[serde(default)] pub delta: Option<f64>,
    #[serde(default)] pub gamma: Option<f64>,
    #[serde(default, alias = "theta")] pub theta_per_day: Option<f64>,
    #[serde(default, alias = "vega")]  pub vega_per_pct:  Option<f64>,

    #[serde(default, deserialize_with = "de_i64_or_zero")] pub open_interest: i64,
    #[serde(default, deserialize_with = "de_i64_or_zero")] pub oi_change:     i64,
    #[serde(default, deserialize_with = "de_i64_or_zero")] pub day_volume:    i64,

    #[serde(default, deserialize_with = "de_i64_or_zero")] pub updated_at_ms: i64,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct ChainFilters {
    #[serde(default)] pub expiry: Option<String>,
    #[serde(default)] pub dte_max: Option<i32>,
    #[serde(default)] pub strike_window_pct: Option<f64>,
    #[serde(default)] pub side: Option<String>,
    #[serde(default)] pub spot: Option<f64>,
    #[serde(default)] pub all: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChainResponse {
    /// Optional — newer backend drops this from the top-level envelope since
    /// every row already carries `underlying`. Derived from `rows[0]` when absent.
    #[serde(default)] pub underlying: String,
    pub contracts: u32,
    #[serde(default)] pub total_in_cache: u32,
    #[serde(default)] pub filters: ChainFilters,
    pub rows: Vec<ChainRow>,
}

/// §5.4.d — chain_delta frame payload
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChainDelta {
    pub underlying: String,
    pub rows: Vec<ChainRow>,
}

/// Query params for `/api/chain/:ul` (§5.4.c).
#[derive(Debug, Clone, Default)]
pub struct ChainQuery {
    pub expiry: Option<String>,
    pub dte: Option<i32>,
    pub dte_max: Option<i32>,
    pub strike_window_pct: Option<f64>,
    pub side: Option<char>,
    pub all: bool,
}

impl ChainQuery {
    pub fn to_query_string(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        if let Some(e) = &self.expiry { parts.push(format!("expiry={e}")); }
        if let Some(d) = self.dte     { parts.push(format!("dte={d}")); }
        if let Some(d) = self.dte_max { parts.push(format!("dte_max={d}")); }
        if let Some(p) = self.strike_window_pct { parts.push(format!("strike_window_pct={p}")); }
        if let Some(s) = self.side    { parts.push(format!("side={s}")); }
        if self.all                   { parts.push("all=true".into()); }
        if parts.is_empty() { String::new() } else { format!("?{}", parts.join("&")) }
    }
}

/// §4.8 — GreeksRow.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GreeksRow {
    pub contract: String,
    pub underlying: String,
    pub side: String,
    pub strike: f64,
    pub expiry: String,
    #[serde(default)] pub spot: f64,
    #[serde(default)] pub mid: f64,
    pub iv:            Option<f64>,
    pub delta:         Option<f64>,
    pub gamma:         Option<f64>,
    pub theta_per_day: Option<f64>,
    pub vega_per_pct:  Option<f64>,
    #[serde(default)] pub t_years: f64,
    #[serde(default)] pub rate: f64,
    #[serde(default)] pub updated_at_ms: i64,
}

/// §4.9 — feed status registry entry.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FeedStatus {
    pub id: String,
    #[serde(default)] pub url: String,
    #[serde(default)] pub subscriptions: u32,
    #[serde(default)] pub connected: bool,
    #[serde(default)] pub connected_at_ms: i64,
    #[serde(default)] pub last_msg_at_ms: i64,
    #[serde(default)] pub reconnects: u32,
    #[serde(default)] pub messages: u64,
    #[serde(default)] pub last_error: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CircuitSnapshot {
    pub name: String,
    pub state: String, // "closed" | "open" | "half_open"
    #[serde(default)] pub failures: u32,
    #[serde(default)] pub opens_total: u64,
    #[serde(default)] pub rejections_total: u64,
    #[serde(default)] pub successes_total: u64,
    #[serde(default)] pub failures_total: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct FeedsResponse {
    #[serde(default)] pub feeds: std::collections::HashMap<String, FeedStatus>,
    #[serde(default)] pub circuits: std::collections::HashMap<String, CircuitSnapshot>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct HealthReady {
    #[serde(default)] pub ready: bool,
    #[serde(default)] pub tick_age_ms: i64,
    #[serde(default)] pub tick_fresh: bool,
    #[serde(default)] pub redis: bool,
    #[serde(default)] pub questdb: bool,
    #[serde(default)] pub feeds_connected: u32,
    #[serde(default)] pub feeds_total: u32,
}

/// Indicators preset response — §5.4.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct IndicatorValues {
    #[serde(default)] pub sma20: Option<f64>,
    #[serde(default)] pub sma50: Option<f64>,
    #[serde(default)] pub sma200: Option<f64>,
    #[serde(default)] pub ema9: Option<f64>,
    #[serde(default)] pub ema21: Option<f64>,
    #[serde(default)] pub ema50: Option<f64>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct IndicatorsResponse {
    pub symbol: String,
    pub asset_class: AssetClass,
    pub timeframe: String,
    pub indicators: IndicatorValues,
}

/// §5.3 — cursor-paginated replay response.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ReplayResponse {
    pub bars: Vec<BarWire>,
    pub from: i64,
    pub to: i64,
    pub next_cursor: Option<i64>,
    pub count: u32,
}

/// §5.2 — `/api/symbols`
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct SymbolsResponse {
    #[serde(default)] pub stocks: Vec<String>,
    #[serde(default)] pub option_underlyings: Vec<String>,
}

impl AssetClass {
    pub fn as_default_stock() -> Self { Self::Stock }
}
