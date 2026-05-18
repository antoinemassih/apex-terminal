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

// ── §5.6 Stocks: bulk snapshots / movers / grouped daily ──────────────────

/// Polygon-shape minute aggregate inside a `StockSnapshot`.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct MinAgg {
    #[serde(default, alias = "av")] pub accumulated_volume: f64,
    #[serde(default, alias = "o")]  pub open: f64,
    #[serde(default, alias = "h")]  pub high: f64,
    #[serde(default, alias = "l")]  pub low: f64,
    #[serde(default, alias = "c")]  pub close: f64,
    #[serde(default, alias = "v")]  pub volume: f64,
    #[serde(default, alias = "vw")] pub vwap: f64,
    /// Epoch millis end of the minute window.
    #[serde(default, alias = "t")]  pub timestamp_ms: i64,
    #[serde(default, alias = "n")]  pub trades: u64,
}

/// Polygon-shape day aggregate inside a `StockSnapshot`.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct DayAgg {
    #[serde(default, alias = "o")]  pub open: f64,
    #[serde(default, alias = "h")]  pub high: f64,
    #[serde(default, alias = "l")]  pub low: f64,
    #[serde(default, alias = "c")]  pub close: f64,
    #[serde(default, alias = "v")]  pub volume: f64,
    #[serde(default, alias = "vw")] pub vwap: f64,
}

/// Previous-day aggregate inside a `StockSnapshot`.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct PrevDayAgg {
    #[serde(default, alias = "o")]  pub open: f64,
    #[serde(default, alias = "h")]  pub high: f64,
    #[serde(default, alias = "l")]  pub low: f64,
    #[serde(default, alias = "c")]  pub close: f64,
    #[serde(default, alias = "v")]  pub volume: f64,
    #[serde(default, alias = "vw")] pub vwap: f64,
}

/// Last trade nested inside a `StockSnapshot`.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct LastTrade {
    #[serde(default, alias = "p")]  pub price: f64,
    #[serde(default, alias = "s")]  pub size: f64,
    #[serde(default, alias = "t")]  pub timestamp_ns: i64,
    #[serde(default, alias = "x")]  pub exchange: i32,
}

/// Last NBBO quote nested inside a `StockSnapshot`.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct LastQuote {
    #[serde(default, alias = "P")]  pub ask_price: f64,
    #[serde(default, alias = "S")]  pub ask_size: f64,
    #[serde(default, alias = "p")]  pub bid_price: f64,
    #[serde(default, alias = "s")]  pub bid_size: f64,
    #[serde(default, alias = "t")]  pub timestamp_ns: i64,
}

/// §5.6.a — `GET /api/stocks/snap/bulk?tickers=...` and
/// `GET /api/stocks/movers?direction=gainers|losers`. Mirrors the Polygon v2
/// snapshot envelope so it can be reused for both endpoints.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct StockSnapshot {
    pub ticker: String,
    #[serde(default)] pub day: DayAgg,
    #[serde(default)] pub min: Option<MinAgg>,
    #[serde(default, alias = "prevDay")] pub prev_day: PrevDayAgg,
    #[serde(default, alias = "lastTrade")] pub last_trade: LastTrade,
    #[serde(default, alias = "lastQuote")] pub last_quote: LastQuote,
    #[serde(default, alias = "todaysChange")] pub todays_change: f64,
    #[serde(default, alias = "todaysChangePerc")] pub todays_change_perc: f64,
    /// Epoch nanos — `updated` per Polygon spec.
    #[serde(default)] pub updated: i64,
}

impl StockSnapshot {
    /// Best-effort "current price": last trade → minute close → day close → prev close.
    pub fn current_price(&self) -> f64 {
        if self.last_trade.price > 0.0 { return self.last_trade.price; }
        if let Some(m) = &self.min { if m.close > 0.0 { return m.close; } }
        if self.day.close > 0.0 { return self.day.close; }
        self.prev_day.close
    }

    /// Best-effort total day volume: prefer the running day agg, fall back to
    /// the minute window's accumulated_volume (covers pre-open / early session).
    pub fn day_volume(&self) -> u64 {
        if self.day.volume > 0.0 { return self.day.volume as u64; }
        if let Some(m) = &self.min { return m.accumulated_volume as u64; }
        0
    }
}

// ── Wave 10 projector outputs — sector rotation / breadth / movers / halts ─

/// One sector ETF's rotation reading. The 11 SPDR sector ETFs (XLK / XLF / XLE /
/// XLV / XLI / XLP / XLY / XLU / XLB / XLRE / XLC) are scored on two axes
/// (relative strength vs SPY, and rate-of-change of that RS). Quadrant per RRG:
/// `Leading`, `Weakening`, `Lagging`, `Improving`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RotationQuadrant { Leading, Weakening, Lagging, Improving, Unknown }

impl Default for RotationQuadrant {
    fn default() -> Self { Self::Unknown }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct SectorRotationRow {
    pub symbol: String,                       // "XLK", "XLF", …
    #[serde(default)] pub name: String,       // "Technology"
    #[serde(default)] pub rs_ratio: f32,      // relative strength vs benchmark
    #[serde(default)] pub rs_momentum: f32,   // rate of change of rs_ratio
    #[serde(default)] pub change_pct: f32,    // intraday %
    #[serde(default)] pub quadrant: RotationQuadrant,
}

/// `GET /api/stocks/sector_rotation` — Wave 10 projector output.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct SectorRotationReading {
    #[serde(default)] pub benchmark: String,         // "SPY"
    #[serde(default)] pub session_date: String,      // "YYYY-MM-DD"
    #[serde(default)] pub rows: Vec<SectorRotationRow>,
    #[serde(default)] pub updated_at_ms: i64,
}

/// `GET /api/stocks/breadth/:index` — Wave 10 projector output.
/// `index` is one of `spx`, `ndx`, `compq`, `rut` or `us` (whole market).
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct BreadthReading {
    #[serde(default)] pub index: String,
    #[serde(default)] pub advancers: u32,
    #[serde(default)] pub decliners: u32,
    #[serde(default)] pub unchanged: u32,
    #[serde(default)] pub new_highs: u32,        // 52-week
    #[serde(default)] pub new_lows: u32,         // 52-week
    #[serde(default)] pub pct_above_sma50: f32,
    #[serde(default)] pub pct_above_sma200: f32,
    #[serde(default)] pub up_volume: f64,
    #[serde(default)] pub down_volume: f64,
    #[serde(default)] pub session_date: String,
    #[serde(default)] pub updated_at_ms: i64,
}

/// Movers bucket kind — drives `GET /api/stocks/movers/:kind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoverKind { Gainers, Losers, Active, RvolLeaders, Gappers }

impl MoverKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Gainers => "gainers",
            Self::Losers  => "losers",
            Self::Active  => "active",
            Self::RvolLeaders => "rvol_leaders",
            Self::Gappers => "gappers",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Gainers => "Gainers",
            Self::Losers  => "Losers",
            Self::Active  => "Active",
            Self::RvolLeaders => "RVOL Leaders",
            Self::Gappers => "Gappers",
        }
    }
    pub fn all() -> [MoverKind; 5] {
        [Self::Gainers, Self::Losers, Self::Active, Self::RvolLeaders, Self::Gappers]
    }
}

/// One row inside a `MoversReading`. Fields are projector-shaped, not Polygon —
/// the projector enriches with RVOL + market-cap when available.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct MoverRow {
    pub symbol: String,
    #[serde(default)] pub last: f64,
    #[serde(default)] pub change_pct: f32,
    #[serde(default)] pub volume: u64,
    #[serde(default)] pub rvol: Option<f32>,
    #[serde(default)] pub market_cap: Option<f64>,
    #[serde(default)] pub gap_pct: Option<f32>,
}

/// `GET /api/stocks/movers/:kind` — Wave 10 projector output.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct MoversReading {
    #[serde(default)] pub kind: String,
    #[serde(default)] pub rows: Vec<MoverRow>,
    #[serde(default)] pub updated_at_ms: i64,
}

/// Halt kind for `Frame::Halt`. Matches the projector's classification:
/// `HaltActive` (LULD trading-paused), `HaltCleared` (resumed),
/// `NearLuldUp` / `NearLuldDown` (within X% of LULD band — early warning).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HaltKind { HaltActive, HaltCleared, NearLuldUp, NearLuldDown, Unknown }

impl Default for HaltKind {
    fn default() -> Self { Self::Unknown }
}

/// WS `Frame::Halt` payload — also returned by any REST snapshot endpoint
/// for recent halts (not wired yet — pure WS for now).
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct HaltReading {
    pub symbol: String,
    #[serde(default)] pub kind: HaltKind,
    #[serde(default)] pub reason: String,        // e.g. "LULD", "T1", "M"
    #[serde(default)] pub price: f64,
    #[serde(default)] pub time_ms: i64,
    /// Resumes-at timestamp for `HaltActive`; 0 when unknown.
    #[serde(default)] pub resumes_at_ms: i64,
}

/// §5.6.c — `GET /api/stocks/grouped/:date` row (Polygon "grouped daily bars").
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct GroupedDailyBar {
    #[serde(alias = "T")] pub ticker: String,
    #[serde(default, alias = "o")]  pub o: f64,
    #[serde(default, alias = "h")]  pub h: f64,
    #[serde(default, alias = "l")]  pub l: f64,
    #[serde(default, alias = "c")]  pub c: f64,
    #[serde(default, alias = "v")]  pub v: f64,
    #[serde(default, alias = "vw")] pub vw: f64,
    #[serde(default, alias = "n")]  pub n: u64,
    /// Epoch millis at session end.
    #[serde(default, alias = "t")]  pub t: i64,
}

#[cfg(test)]
mod wave10_parse_tests {
    use super::*;

    #[test]
    fn sector_rotation_parses_with_quadrants() {
        let body = r#"{
          "benchmark":"SPY",
          "session_date":"2026-05-17",
          "rows":[
            {"symbol":"XLK","name":"Technology","rs_ratio":101.2,"rs_momentum":100.8,"change_pct":1.4,"quadrant":"leading"},
            {"symbol":"XLF","name":"Financials","rs_ratio":99.8,"rs_momentum":100.2,"change_pct":0.3,"quadrant":"improving"},
            {"symbol":"XLE","name":"Energy","rs_ratio":98.5,"rs_momentum":99.4,"change_pct":-0.7,"quadrant":"lagging"}
          ],
          "updated_at_ms":1700000000000
        }"#;
        let r: SectorRotationReading = serde_json::from_str(body).expect("parse");
        assert_eq!(r.rows.len(), 3);
        assert_eq!(r.rows[0].quadrant, RotationQuadrant::Leading);
        assert_eq!(r.rows[1].quadrant, RotationQuadrant::Improving);
        assert_eq!(r.rows[2].quadrant, RotationQuadrant::Lagging);
    }

    #[test]
    fn breadth_parses() {
        let body = r#"{
          "index":"spx","advancers":1240,"decliners":950,"unchanged":20,
          "new_highs":87,"new_lows":12,
          "pct_above_sma50":65.4,"pct_above_sma200":58.1,
          "up_volume":1.2e10,"down_volume":8.0e9
        }"#;
        let b: BreadthReading = serde_json::from_str(body).expect("parse");
        assert_eq!(b.advancers, 1240);
        assert_eq!(b.new_highs, 87);
        assert!((b.pct_above_sma50 - 65.4).abs() < 0.01);
    }

    #[test]
    fn movers_parses_all_kinds() {
        for kind in MoverKind::all() {
            let body = format!(r#"{{
              "kind":"{}",
              "rows":[
                {{"symbol":"AAPL","last":195.2,"change_pct":2.1,"volume":50000000,"rvol":1.8,"market_cap":3.0e12}}
              ]
            }}"#, kind.as_str());
            let m: MoversReading = serde_json::from_str(&body).expect("parse");
            assert_eq!(m.kind, kind.as_str());
            assert_eq!(m.rows.len(), 1);
            assert_eq!(m.rows[0].symbol, "AAPL");
            assert!(m.rows[0].rvol.unwrap_or(0.0) > 1.0);
        }
    }

    #[test]
    fn mover_kind_str_matches_rest_path_segment() {
        // The scanner panel tab selector flips `scanner_mover_tab`, which we use
        // to index `MoverKind::all()` and feed `get_movers(kind)`. This test
        // pins the URL-path segment per kind so a typo there is caught early.
        assert_eq!(MoverKind::Gainers.as_str(),     "gainers");
        assert_eq!(MoverKind::Losers.as_str(),      "losers");
        assert_eq!(MoverKind::Active.as_str(),      "active");
        assert_eq!(MoverKind::RvolLeaders.as_str(), "rvol_leaders");
        assert_eq!(MoverKind::Gappers.as_str(),     "gappers");
        // All 5 kinds participate in the tab selector.
        assert_eq!(MoverKind::all().len(), 5);
    }

    #[test]
    fn halt_frame_parses_with_kind_variants() {
        for (s, want) in [
            ("halt_active", HaltKind::HaltActive),
            ("halt_cleared", HaltKind::HaltCleared),
            ("near_luld_up", HaltKind::NearLuldUp),
            ("near_luld_down", HaltKind::NearLuldDown),
        ] {
            let body = format!(r#"{{"symbol":"GME","kind":"{s}","reason":"LULD","price":42.5,"time_ms":1700000000000}}"#);
            let h: HaltReading = serde_json::from_str(&body).expect("parse");
            assert_eq!(h.kind, want);
            assert_eq!(h.symbol, "GME");
        }
    }
}
