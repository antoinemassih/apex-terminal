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
}
