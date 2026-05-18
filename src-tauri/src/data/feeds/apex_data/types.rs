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

// ════════════════════════════════════════════════════════════════════════════
// SOTA UX — Agent A: Provenance + Regime + Calibrated
//
// Wire types for SOTA panels (see docs/SOTA_UX_DESIGN.md §3, §4.1, §4.3, §4.6).
// These extend the existing ApexData contract; they don't replace anything.
// ════════════════════════════════════════════════════════════════════════════

/// Lineage identifier — a Crockford-base32-ish string from ApexData's
/// provenance log. Treated as opaque for now; promote to a newtype later
/// if we need parsing.
pub type LineageId = String;

/// `GET /api/provenance/:id?format=tree&depth=N` mode selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProvenanceMode {
    /// Recursive `TreeNode` JSON — children inline.
    Tree,
    /// `{ root, nodes[], edges[] }` flattened for d3-style force-directed view.
    Dag,
}

impl Default for ProvenanceMode {
    fn default() -> Self { ProvenanceMode::Tree }
}

impl ProvenanceMode {
    pub fn as_str(self) -> &'static str {
        match self { Self::Tree => "tree", Self::Dag => "dag" }
    }
}

/// One node in the provenance evidence DAG. Recursive — children are
/// `ProvenanceTreeNode`s themselves, with cycles cut server-side and rendered
/// as `is_cycle = true` leaves.
///
/// Fields use `#[serde(default)]` so the wire shape can evolve (add fields)
/// without breaking older clients.
#[derive(Debug, Clone, Deserialize, Serialize, Default, PartialEq)]
pub struct ProvenanceTreeNode {
    pub lineage_id: LineageId,
    #[serde(default)] pub source_engine: String,
    #[serde(default)] pub symbol: String,
    #[serde(default)] pub t_ms: i64,
    #[serde(default)] pub score: Option<f64>,
    #[serde(default)] pub kind: String, // e.g. "signal", "trade_plan", "combined", "regime"
    /// Free-form payload for the focused-node detail panel (full engine output).
    #[serde(default)] pub payload: Option<serde_json::Value>,
    /// Children (upstream evidence). Empty `Vec` = leaf.
    #[serde(default)] pub children: Vec<ProvenanceTreeNode>,
    /// Server-side cycle marker. If true, `lineage_id` points back at an
    /// ancestor and the renderer should show "↻ Cycle to <prefix>".
    #[serde(default)] pub is_cycle: bool,
    /// Set by the server when the response was truncated at the depth cap.
    /// Render a banner ("Showing first 500 of N nodes") when present.
    #[serde(default)] pub truncated_total: Option<u32>,
}

/// Adaptive learner calibration for a single signal/engine. Empty / `n_samples<30`
/// means "no calibration yet — render '—'".
#[derive(Debug, Clone, Deserialize, Serialize, Default, PartialEq)]
pub struct Calibrated {
    #[serde(default)] pub hit_rate: Option<f64>,        // 0.0..=1.0
    #[serde(default)] pub n_samples: u32,
    #[serde(default)] pub ci_low: Option<f64>,          // 90% CI lower bound
    #[serde(default)] pub ci_high: Option<f64>,         // 90% CI upper bound
    #[serde(default)] pub conformal_coverage: Option<f64>, // for trade plans
}

impl Calibrated {
    /// Trust factor per `adaptive_learner` spec — `min(n_samples, 50)/50`,
    /// clamped to `[0, 1]`. Drives the SignalsPanel `trust` bar.
    pub fn trust_factor(&self) -> f32 {
        (self.n_samples.min(50) as f32) / 50.0
    }

    /// True when we have enough samples to render the hit rate. Threshold
    /// matches the spec — anything below 30 reads as "—" in the UI.
    pub fn is_calibrated(&self) -> bool {
        self.hit_rate.is_some() && self.n_samples >= 30
    }
}

/// CombinedSignal v2 — extends the existing CombinedSignal (which the
/// terminal already reads from `signals:combined:{symbol}`) with:
/// - `calibrated_contributors` — per-engine adaptive-learner calibration
/// - `provenance` — top-level lineage_id so the row's 🔍 button can open
///   ProvenancePane without a side fetch
///
/// Wire-compatible: extra fields are tolerated by serde. Older messages
/// without these fields parse cleanly with the defaults.
#[derive(Debug, Clone, Deserialize, Serialize, Default, PartialEq)]
pub struct CombinedSignalV2 {
    pub symbol: String,
    #[serde(default)] pub t_ms: i64,
    #[serde(default)] pub score: f64,
    #[serde(default)] pub direction: String, // "long" | "short" | "neutral"
    #[serde(default)] pub top_contributors: Vec<ContributorEntry>,
    #[serde(default)] pub calibrated_contributors: std::collections::HashMap<String, Calibrated>,
    #[serde(default)] pub provenance: Option<ProvenanceRef>,
}

/// One contributor entry inside `top_contributors[]`. Engine name + score
/// + lineage so each row can drill into its own evidence subtree.
#[derive(Debug, Clone, Deserialize, Serialize, Default, PartialEq)]
pub struct ContributorEntry {
    pub engine: String,
    #[serde(default)] pub score: f64,
    #[serde(default)] pub lineage_id: Option<LineageId>,
}

/// Minimal provenance reference embedded in a higher-level signal so the
/// UI can open ProvenancePane without an extra REST round-trip.
#[derive(Debug, Clone, Deserialize, Serialize, Default, PartialEq)]
pub struct ProvenanceRef {
    pub lineage_id: LineageId,
    #[serde(default)] pub inputs: Vec<LineageId>,
}

// ── Regime ──────────────────────────────────────────────────────────────────

/// One axis-level regime classification. Strings rather than enums so new
/// values (e.g. "Crush") added server-side don't break old clients.
#[derive(Debug, Clone, Deserialize, Serialize, Default, PartialEq)]
pub struct RegimeAxis {
    /// e.g. "Bull", "Pin", "Trend", "LowContango", "RiskOnGrowth", "PreClassify"
    pub value: String,
    #[serde(default)] pub confidence: Option<f64>,
    /// Upstream signal names that drove this value (for hover tooltips).
    #[serde(default)] pub upstream_tells: Vec<String>,
}

/// Full 4-axis regime classification. Stored on the live_state and rendered
/// in the always-visible top RegimeTape strip.
#[derive(Debug, Clone, Deserialize, Serialize, Default, PartialEq)]
pub struct Regime {
    pub intraday: RegimeAxis,
    pub multiday: RegimeAxis,
    pub vol: RegimeAxis,
    pub sector: RegimeAxis,
}

/// Wire frame from `Signal::Regime` (RegimeRouter publishes every 5min +
/// on axis change).
#[derive(Debug, Clone, Deserialize, Serialize, Default, PartialEq)]
pub struct RegimeFrame {
    pub regime: Regime,
    #[serde(default)] pub t_ms: i64,
    #[serde(default)] pub regime_key: String,
    #[serde(default)] pub session_day: String,
    #[serde(default)] pub provenance_inputs: Vec<LineageId>,
    #[serde(default)] pub provenance: Option<ProvenanceRef>,
}

/// One axis transition for the optional bottom-of-tape strip ("intraday flipped
/// from PreClassify to Bull at 10:15 ET"). Capped at 20 per session.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RegimeTransition {
    pub axis: String,    // "intraday" | "multiday" | "vol" | "sector"
    pub from: String,
    pub to: String,
    pub t_ms: i64,
// ────────────────────────────────────────────────────────────────────────────
// Replay (SOTA §4.2) — historical scrubbing session
// ────────────────────────────────────────────────────────────────────────────

/// Stable identifier for a replay session, returned by `POST /api/replay/start`.
pub type ReplayId = String;

/// What kind of frames the replay session should emit. The MVP backend
/// supports the four `*_bars` modes only; trades/quotes return HTTP 501.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplayMode {
    StockBars,
    OptionBars,
    MarkBarsStocks,
    MarkBarsOptions,
    /// Returns 501 today — UI keeps the option visible with a "(not yet
    /// implemented)" label so the user knows it's a roadmap item.
    Trades,
    /// Returns 501 today — see above.
    Quotes,
}

impl ReplayMode {
    /// Wire string sent to the backend. Matches the snake_case serialization
    /// the server expects in `POST /api/replay/start { mode }`.
    pub fn as_str(self) -> &'static str {
        match self {
            ReplayMode::StockBars       => "stock_bars",
            ReplayMode::OptionBars      => "option_bars",
            ReplayMode::MarkBarsStocks  => "mark_bars_stocks",
            ReplayMode::MarkBarsOptions => "mark_bars_options",
            ReplayMode::Trades          => "trades",
            ReplayMode::Quotes          => "quotes",
        }
    }
    /// Human-friendly label for the radio buttons.
    pub fn label(self) -> &'static str {
        match self {
            ReplayMode::StockBars       => "Stock bars",
            ReplayMode::OptionBars      => "Option bars",
            ReplayMode::MarkBarsStocks  => "Mark bars (stocks)",
            ReplayMode::MarkBarsOptions => "Mark bars (options)",
            ReplayMode::Trades          => "Trades",
            ReplayMode::Quotes          => "Quotes",
        }
    }
    /// MVP backend only supports the bars modes; trades/quotes 501 today.
    pub fn is_implemented(self) -> bool {
        matches!(self, ReplayMode::StockBars | ReplayMode::OptionBars
                     | ReplayMode::MarkBarsStocks | ReplayMode::MarkBarsOptions)
    }
}

/// Lifecycle state of a replay session. Mirrors what the backend's
/// `GET /api/replay/:id/status` returns in its `state` field. Anything we
/// don't recognize is mapped to `Unknown` so the UI never crashes on a new
/// server-side state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReplayState {
    NotStarted,
    Running,
    Paused,
    Completed,
    Stopped,
    Failed,
    Unknown,
}

impl Default for ReplayState {
    fn default() -> Self { ReplayState::NotStarted }
}

impl ReplayState {
    pub fn is_terminal(self) -> bool {
        matches!(self, ReplayState::Completed | ReplayState::Stopped | ReplayState::Failed)
    }
    pub fn is_active(self) -> bool {
        matches!(self, ReplayState::Running | ReplayState::Paused)
    }
}

/// User-side configuration the panel posts to `POST /api/replay/start`.
/// `from_ts` / `to_ts` are epoch milliseconds (matches the rest of the
/// ApexData contract — see §4.2 / `BarUpdate.bar.time`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayConfig {
    pub from_ts: i64,
    pub to_ts: i64,
    pub symbols: Vec<String>,
    pub speed_multiplier: f32,
    pub asset_class: AssetClass,
    /// `"last"` or `"mark"` — matches the bars `source` query param used elsewhere.
    pub source: String,
    pub mode: ReplayMode,
}

/// Response from `POST /api/replay/start`. We tolerate extra fields the
/// server may add later via `#[serde(default)]` on everything except the id.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ReplayStartResponse {
    pub replay_id: ReplayId,
    #[serde(default)] pub state: Option<ReplayState>,
    #[serde(default)] pub estimated_frames: Option<u64>,
    /// Free-form note from the server (e.g. "1 day, 1 symbol → ~21k bars").
    #[serde(default)] pub note: Option<String>,
}

/// Status from `GET /api/replay/:id/status`. Field set follows the spec at
/// SOTA §3.1 — anything not present defaults to a sensible value so the UI
/// can render even if the server omits fields.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ReplayStatus {
    #[serde(default)] pub state: ReplayState,
    #[serde(default)] pub frames_emitted: u64,
    /// 0.0..=1.0 — fraction of the requested time window that has been emitted.
    #[serde(default)] pub progress: f32,
    /// Current playhead, epoch milliseconds. 0 before the first frame.
    #[serde(default)] pub current_ts_ms: i64,
    #[serde(default)] pub speed_multiplier: f32,
    #[serde(default)] pub error: Option<String>,
}

/// Lightweight summary of one streamed frame, retained in the events log.
/// We don't keep the whole bar — the chart pane already owns that. Keeping
/// this tiny lets the 500-cap log stay under ~50 KB even with long symbols.
#[derive(Debug, Clone)]
pub struct ReplayEvent {
    /// `"bar" | "trade" | "quote" | "snapshot" | "error" | "info"` — the
    /// envelope `type` field, copied verbatim. Pre-canned to a `&'static
    /// str` when the type is one of the common ones; otherwise `String`
    /// via `event_type_owned`.
    pub kind: &'static str,
    pub symbol: String,
    pub t_ms: i64,
    /// Convenience for the UI — usually `bar.close` when `kind == "bar"`.
    pub price: Option<f64>,
}

impl ReplayEvent {
    pub fn new_bar(symbol: impl Into<String>, t_ms: i64, close: f64) -> Self {
        ReplayEvent { kind: "bar", symbol: symbol.into(), t_ms, price: Some(close) }
    }
    pub fn new_trade(symbol: impl Into<String>, t_ms: i64, price: f64) -> Self {
        ReplayEvent { kind: "trade", symbol: symbol.into(), t_ms, price: Some(price) }
    }
    pub fn new_info(msg: impl Into<String>, t_ms: i64) -> Self {
        ReplayEvent { kind: "info", symbol: msg.into(), t_ms, price: None }
    }
    pub fn new_error(msg: impl Into<String>) -> Self {
        ReplayEvent { kind: "error", symbol: msg.into(), t_ms: 0, price: None }
// ────────────────────────────────────────────────────────────────────────────
// SOTA §4.4 — TradePlan v2 (Calibrated + Conformal + Provenance)
// ────────────────────────────────────────────────────────────────────────────

/// Provenance metadata attached to model-produced wire artifacts (trade plans,
/// spike explanations, regime calls). The `lineage_id` is the stable id that
/// the provenance pane uses to open the full lineage tree.
///
/// All fields are `#[serde(default)]` so older wire shapes (which omit the
/// block entirely or only carry `lineage_id`) still deserialize cleanly.
#[derive(Debug, Clone, Deserialize, Serialize, Default, PartialEq)]
pub struct ProvenanceMeta {
    #[serde(default)] pub lineage_id: String,
    #[serde(default)] pub model: Option<String>,
    #[serde(default)] pub run_id: Option<String>,
    #[serde(default)] pub source: Option<String>,
}

/// SOTA §4.4 — calibrated trade plan with conformal target/stop ranges.
///
/// Replaces the legacy point-only `(dir, entry, target, stop, contract, rr,
/// conviction)` tuple in `Chart::trade_plan`. The legacy tuple stays in place
/// for the existing chart-widget renderer; the new panel reads `TradePlanV2`
/// out of `live_state.latest_trade_plan` and renders the calibrated form.
///
/// Every field added on top of the legacy schema is `#[serde(default)]` so
/// older backend builds (which don't yet emit conformal ranges or calibration
/// metadata) deserialize without error and just render the point form.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TradePlanV2 {
    pub symbol: String,
    /// "long" | "short" — string-keyed so it round-trips with the Python
    /// emitter without an enum schema dance.
    pub direction: String,
    pub entry_price: f64,
    pub target_price: f64,
    pub stop_price: f64,

    /// Conformal-prediction target band (low, high). None when the calibrator
    /// didn't run for this plan (e.g. cold-start, < 30 samples).
    #[serde(default)]
    pub target_range: Option<(f64, f64)>,
    /// Conformal-prediction stop band (low, high).
    #[serde(default)]
    pub stop_range: Option<(f64, f64)>,

    /// Historical hit-rate (0.0..=1.0) for plans matching this setup. None
    /// when the back-test history has fewer than `min_n` samples.
    #[serde(default)]
    pub historical_hit_rate: Option<f32>,
    /// How many historical samples back the hit-rate. Used to grey out
    /// low-confidence stats in the UI.
    #[serde(default)]
    pub historical_n_samples: u32,
    /// Conformal coverage level (e.g. 0.8 = 80% prediction intervals).
    /// Default 0.0 means "no calibration metadata wired up".
    #[serde(default)]
    pub conformal_coverage: f32,

    /// Plain-text exit rule emitted by the planner ("scale 50% at +1R, runner
    /// to +2R; stop to break-even on first target"). Free-form, panel renders
    /// verbatim.
    #[serde(default)]
    pub exit_rule: Option<String>,

    /// Day-type classifier output: "BULL" | "BEAR" | "PIN" | "MIXED" | "CHOP".
    /// Per the user's MEMORY.md day-type-classifier rule, MIXED/CHOP plans
    /// should be visually suppressed by the renderer.
    #[serde(default)]
    pub day_type: Option<String>,
    /// Day-type confidence (0.0..=1.0).
    #[serde(default)]
    pub day_type_confidence: Option<f32>,

    /// Provenance pointer for the [🔍 prov] button. None when the upstream
    /// planner doesn't emit lineage.
    #[serde(default)]
    pub provenance: Option<ProvenanceMeta>,

    /// Server-side emit timestamp in epoch milliseconds.
    #[serde(default)]
    pub t_ms: i64,
}

impl TradePlanV2 {
    /// Whether the panel should visually suppress this plan per the day-type
    /// rule. MIXED/CHOP days mean "no edge" — show greyed banner instead of a
    /// loud plan.
    pub fn day_type_suppressed(&self) -> bool {
        matches!(self.day_type.as_deref(), Some("MIXED" | "CHOP"))
    }

    /// Calibration tier for color routing (>=0.6 green, 0.5..0.6 yellow,
    /// <0.5 red, None → grey "no calibration"). Falls back to a stable
    /// "Unknown" when `historical_hit_rate` is `None`.
    pub fn calibration_tier(&self) -> CalibrationTier {
        match self.historical_hit_rate {
            None => CalibrationTier::Unknown,
            Some(r) if r >= 0.6 => CalibrationTier::Strong,
            Some(r) if r >= 0.5 => CalibrationTier::Marginal,
            Some(_) => CalibrationTier::Weak,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalibrationTier {
    Strong,    // >= 0.6 — green
    Marginal,  // 0.5..0.6 — yellow
    Weak,      // < 0.5 — red
    Unknown,   // missing hit-rate — grey
}

// ────────────────────────────────────────────────────────────────────────────
// SOTA §4.5 — Spike explanation (transient toast popup)
// ────────────────────────────────────────────────────────────────────────────

/// SOTA §4.5 — spike-explanation toast payload. Pushed by ApexData (which in
/// turn subscribes to the apex-spike-explainer Redis pubsub) as a `spike`
/// frame. `id` is derived client-side from `{symbol}:{t_ms}` so the popup
/// state machine can dedup on it without coordination with the server.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SpikeExplanation {
    pub symbol: String,
    pub t_ms: i64,
    pub sigma: f32,
    pub pct_move: f32,
    pub headline: String,
    pub explanation: String,
    #[serde(default)]
    pub sources: Vec<String>,
    /// Stable dedup key: `format!("{symbol}:{t_ms}")`. Skipped on the wire —
    /// derived in `From<SpikeWire>` so producers don't have to send it.
    #[serde(skip)]
    pub id: String,
}

impl SpikeExplanation {
    /// Derive the dedup key. Called from `from_wire` below; exposed publicly
    /// so the popup component can synthesize ids for test fixtures.
    pub fn derive_id(symbol: &str, t_ms: i64) -> String {
        format!("{symbol}:{t_ms}")
    }

    /// Convenience constructor: build a `SpikeExplanation` with the id
    /// derived from `(symbol, t_ms)`.
    pub fn new(
        symbol: String, t_ms: i64, sigma: f32, pct_move: f32,
        headline: String, explanation: String, sources: Vec<String>,
    ) -> Self {
        let id = Self::derive_id(&symbol, t_ms);
        Self { symbol, t_ms, sigma, pct_move, headline, explanation, sources, id }
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
// ── Wave 10 projector outputs — news / IV rank / ETF IIV / corp actions ──

/// Single news article reading from the `news_sentiment` projector.
/// `published_ms` is epoch milliseconds. `sentiment` is the bucketed label
/// returned by the spike_explainer (bullish/bearish/neutral); `score` is the
/// raw signed sentiment score in `[-1.0, 1.0]`.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct NewsReading {
    #[serde(default)] pub symbol: String,
    #[serde(default)] pub headline: String,
    #[serde(default)] pub source: String,
    #[serde(default)] pub url: String,
    #[serde(default)] pub published_ms: i64,
    /// "bullish" | "bearish" | "neutral" (lowercase). Empty when projector
    /// couldn't classify.
    #[serde(default)] pub sentiment: String,
    /// Signed score in [-1.0, 1.0]; `0.0` when missing.
    #[serde(default)] pub score: f64,
}

impl NewsReading {
    /// `-1 | 0 | 1` for the legacy NewsItem sentiment field.
    pub fn sentiment_tristate(&self) -> i8 {
        match self.sentiment.as_str() {
            "bullish" | "bull" | "positive" => 1,
            "bearish" | "bear" | "negative" => -1,
            _ if self.score > 0.15 => 1,
            _ if self.score < -0.15 => -1,
            _ => 0,
        }
    }
}

/// REST envelope for `/api/stocks/news/:ticker`.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct NewsResponse {
    #[serde(default)] pub ticker: String,
    #[serde(default)] pub articles: Vec<NewsReading>,
    #[serde(default)] pub updated_at_ms: i64,
}

/// `vol_surface` migration — current ATM IV and percentile rank over a
/// configurable lookback (default 252 sessions).
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct IvRankV2 {
    #[serde(default)] pub underlying: String,
    /// Current ATM implied vol (annualized, decimal, e.g. 0.245 = 24.5%).
    #[serde(default)] pub atm_iv: f64,
    /// 0–100 percentile rank within the lookback window.
    #[serde(default)] pub iv_rank: f64,
    /// Number of session samples actually used for the rank.
    #[serde(default)] pub n_samples: u32,
    /// Sessions requested by the caller (echoed by backend).
    #[serde(default)] pub lookback: u32,
    /// Backend sets this true when n_samples < min_samples — UI shows the
    /// "insufficient history" message instead of the bar.
    #[serde(default)] pub insufficient_history: bool,
    #[serde(default)] pub updated_at_ms: i64,
}

/// `etf_iiv` projector reading — ETF mark vs indicative NAV.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct EtfIivReading {
    #[serde(default)] pub etf: String,
    /// Last trade / mark.
    #[serde(default)] pub market_price: f64,
    /// Indicative intraday NAV.
    #[serde(default)] pub iiv: f64,
    /// (market_price - iiv) / iiv expressed in basis points.
    #[serde(default)] pub premium_disc_bps: f64,
    /// Stale fraction of the basket constituents (0.0–1.0).
    #[serde(default)] pub staleness_pct: f64,
    #[serde(default)] pub updated_at_ms: i64,
}

/// Kind of corporate action — used by both the UI badge picker and tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionKind {
    Earnings,
    ExDividend,
    Split,
    Halt,
    Other,
}

impl Default for ActionKind {
    fn default() -> Self { ActionKind::Other }
}

/// A scheduled / recently-fired corporate event for a ticker.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct UpcomingEvent {
    #[serde(default)] pub kind: ActionKind,
    /// Event time in epoch milliseconds (UTC).
    #[serde(default)] pub event_ms: i64,
    /// Free-form details, e.g. "EPS est 1.23" or "0.24/share".
    #[serde(default)] pub detail: String,
}

/// `corporate_actions` projector reading for a single ticker.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct CorporateActionsReading {
    #[serde(default)] pub ticker: String,
    #[serde(default)] pub events: Vec<UpcomingEvent>,
    #[serde(default)] pub updated_at_ms: i64,
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
