//! Native GPU chart renderer — wgpu + winit, zero browser overhead.
//!
//! Architecture:
//! - Spawns a native OS window (winit) on a dedicated thread
//! - Renders candlesticks, volume, indicators via wgpu (same WGSL shaders as WebGPU frontend)
//! - Receives bar data + viewport commands from Tauri WebView via channels
//! - Runs its own render loop at monitor vsync — no rAF, no compositor, no DOM
//!
//! Communication with WebView:
//! - ChartCommand enum sent via crossbeam channel from Tauri commands
//! - Render thread processes commands between frames (non-blocking)

pub mod gpu;
mod types;
pub mod ui;
pub mod compute;
pub mod trading;

pub use types::*;

/// Tab selector for the unified Analysis sidebar.
#[derive(Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) enum AnalysisTab { Rrg, TimeSales, Scanner, Scripts }

/// Line style
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LineStyle { Solid, Dashed, Dotted }

/// Drawing on the chart
/// Significance data for a drawing (populated by backend/ApexSignals)
#[derive(Debug, Clone)]
pub struct DrawingSignificance {
    pub score: f32,            // 0.0-10.0 overall score
    pub touches: u32,          // number of price touches
    pub timeframe: String,     // source timeframe e.g. "1D", "4H"
    pub age_days: u32,         // how old the drawing is
    pub volume_index: f32,     // avg volume at touches relative to normal (1.0 = average)
    pub last_tested_bars: u32, // how many bars ago price last touched it
    pub strength: String,      // "WEAK", "MODERATE", "STRONG", "CRITICAL"
}

impl DrawingSignificance {
    /// Generate placeholder significance from basic bar analysis
    pub fn estimate(kind: &DrawingKind, timestamps: &[i64], bars: &[Bar]) -> Option<Self> {
        if bars.is_empty() || timestamps.is_empty() { return None; }
        let n = bars.len();
        let threshold_pct = 0.003; // 0.3% proximity = touch

        // Get the price function for this drawing at each bar
        let price_at_bar = |i: usize| -> Option<f32> {
            match kind {
                DrawingKind::HLine { price } => Some(*price),
                DrawingKind::TrendLine { price0, time0, price1, time1 } => {
                    let t0 = *time0 as f64; let t1 = *time1 as f64;
                    if (t1 - t0).abs() < 1.0 { return None; }
                    let tc = timestamps.get(i).copied()? as f64;
                    let frac = (tc - t0) / (t1 - t0);
                    Some(*price0 + (*price1 - *price0) * frac as f32)
                }
                _ => None,
            }
        };

        let mut touches = 0u32;
        let mut last_touch_bar = 0usize;
        let mut vol_sum = 0.0f32;
        let mut vol_count = 0u32;
        let avg_vol: f32 = bars.iter().map(|b| b.volume).sum::<f32>() / n as f32;

        for i in 0..n {
            if let Some(level_price) = price_at_bar(i) {
                if level_price <= 0.0 { continue; }
                let threshold = level_price * threshold_pct;
                let bar = &bars[i];
                // Touch = bar high/low within threshold of the level
                if (bar.high - level_price).abs() < threshold || (bar.low - level_price).abs() < threshold {
                    touches += 1;
                    last_touch_bar = i;
                    vol_sum += bar.volume;
                    vol_count += 1;
                }
            }
        }

        if touches == 0 { return None; }

        let volume_index = if vol_count > 0 && avg_vol > 0.0 { (vol_sum / vol_count as f32) / avg_vol } else { 1.0 };
        let last_tested_bars = (n - 1).saturating_sub(last_touch_bar) as u32;

        // Age in days (approximate from timestamps)
        let age_days = if timestamps.len() >= 2 {
            let first = timestamps[0]; let last = *timestamps.last().unwrap();
            ((last - first) as f64 / 86400.0).ceil() as u32
        } else { 0 };

        // Score: weighted combination
        let touch_score = (touches as f32 * 1.5).min(5.0);
        let vol_score = (volume_index * 1.5).min(2.5);
        let recency_score = if last_tested_bars < 5 { 2.0 } else if last_tested_bars < 20 { 1.0 } else { 0.5 };
        let score = (touch_score + vol_score + recency_score).min(10.0);

        let strength = if score >= 7.0 { "CRITICAL" } else if score >= 5.0 { "STRONG" } else if score >= 3.0 { "MODERATE" } else { "WEAK" };

        Some(Self {
            score,
            touches,
            timeframe: String::new(), // filled by backend
            age_days,
            volume_index,
            last_tested_bars,
            strength: strength.to_string(),
        })
    }
}

#[derive(Debug, Clone)]
pub struct Drawing {
    pub id: String,
    pub kind: DrawingKind,
    pub color: String,      // hex color like "#4a9eff"
    pub opacity: f32,       // 0.0-1.0
    pub line_style: LineStyle,
    pub thickness: f32,     // pixels
    pub group_id: String,   // "default" or group UUID
    pub extend_left: bool,  // extend trendline/ray to left chart edge
    pub extend_right: bool, // extend trendline/ray to right chart edge
    pub locked: bool,       // prevent accidental moves
    pub alert_enabled: bool, // show alert bell indicator
    pub significance: Option<DrawingSignificance>, // backend-populated or estimated
}

impl Drawing {
    pub fn new(id: String, kind: DrawingKind) -> Self {
        Self { id, kind, color: "#4a9eff".into(), opacity: 1.0, line_style: LineStyle::Solid, thickness: 1.5, group_id: "default".into(), extend_left: false, extend_right: false, locked: false, alert_enabled: false, significance: None }
    }
}

/// Drawing group
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct DrawingGroup {
    pub id: String,
    pub name: String,
    pub color: Option<String>,
}

#[derive(Debug, Clone)]
pub enum DrawingKind {
    HLine { price: f32 },
    /// Trendline — timestamps (i64) resolve to fractional bar positions on any timeframe.
    TrendLine { price0: f32, time0: i64, price1: f32, time1: i64 },
    HZone { price0: f32, price1: f32 },
    BarMarker { time: i64, price: f32, up: bool },
    /// Fibonacci retracement — two anchor points define the range.
    /// Levels drawn: 0%, 23.6%, 38.2%, 50%, 61.8%, 78.6%, 100%
    Fibonacci { price0: f32, time0: i64, price1: f32, time1: i64 },
    /// Parallel channel — base trendline (p0→p1) + price offset for the parallel line.
    Channel { price0: f32, time0: i64, price1: f32, time1: i64, offset: f32 },
    /// Fibonacci channel — same anchors as channel, internal lines at fib ratios.
    FibChannel { price0: f32, time0: i64, price1: f32, time1: i64, offset: f32 },
    /// Pitchfork — pivot + two reaction points. variant: 0=Standard, 1=Schiff, 2=Modified Schiff
    Pitchfork { price0: f32, time0: i64, price1: f32, time1: i64, price2: f32, time2: i64 },
    /// Gann Fan — origin + scale point defines 1x1 angle, radiating lines
    GannFan { price0: f32, time0: i64, price1: f32, time1: i64 },
    /// Regression Channel — time range, regression + σ bands computed from bars
    RegressionChannel { time0: i64, time1: i64 },
    /// XABCD Harmonic pattern — 5 points stored as (time, price) pairs
    XABCD { points: Vec<(i64, f32)> },
    /// Elliott Wave — labeled wave points, wave_type: 0=impulse(5pt), 1=corrective(3pt)
    ElliottWave { points: Vec<(i64, f32)>, wave_type: u8 },
    /// Anchored VWAP — single anchor timestamp, line computed from bars
    AnchoredVWAP { time: i64 },
    /// Price Range — persistent measurement rectangle
    PriceRange { price0: f32, time0: i64, price1: f32, time1: i64 },
    /// Risk/Reward — entry + stop + target
    RiskReward { entry_price: f32, entry_time: i64, stop_price: f32, target_price: f32 },
    /// Vertical line at a specific timestamp
    VerticalLine { time: i64 },
    /// Ray — trendline that extends forward from second point to chart edge
    Ray { price0: f32, time0: i64, price1: f32, time1: i64 },
    /// Fib Extension (3-point) — A→B move projected from C
    FibExtension { price0: f32, time0: i64, price1: f32, time1: i64, price2: f32, time2: i64 },
    /// Fib Time Zones — vertical lines at fibonacci bar intervals from anchor
    FibTimeZone { time: i64 },
    /// Fib Arcs — semicircular arcs at fib ratios between two points
    FibArc { price0: f32, time0: i64, price1: f32, time1: i64 },
    /// Gann Box — price/time grid with diagonal angles
    GannBox { price0: f32, time0: i64, price1: f32, time1: i64 },
    /// Text annotation — placed at a price/time coordinate
    TextNote { price: f32, time: i64, text: String, font_size: f32 },
}

/// Candlestick pattern label from ApexSignals
#[derive(Debug, Clone)]
pub struct PatternLabel {
    pub time: i64,
    pub label: String,
    pub bullish: bool,
    pub confidence: f32, // 0.0–1.0
}

/// Commands sent from Tauri/WebView to the native chart renderer
#[derive(Debug, Clone)]
pub enum ChartCommand {
    /// Load OHLCV bar data for a symbol
    LoadBars {
        symbol: String,
        timeframe: String,
        bars: Vec<Bar>,
        timestamps: Vec<i64>,
    },
    /// Prepend historical bars (pagination — older data loaded on scroll-left)
    PrependBars {
        symbol: String,
        timeframe: String,
        bars: Vec<Bar>,
        timestamps: Vec<i64>,
    },
    /// Append a single new bar + timestamp
    AppendBar {
        symbol: String,
        timeframe: String,
        bar: Bar,
        timestamp: i64,
    },
    /// Update the last bar (tick)
    UpdateLastBar {
        symbol: String,
        timeframe: String,
        bar: Bar,
    },
    /// Set viewport (from pan/zoom in WebView)
    SetViewport {
        view_start: u32,
        view_count: u32,
        width: u32,
        height: u32,
    },
    /// Set theme colors
    SetTheme {
        background: [f32; 4],
        bull_color: [f32; 4],
        bear_color: [f32; 4],
    },
    /// Add/update a drawing
    SetDrawing(Drawing),
    /// Remove a drawing
    RemoveDrawing { id: String },
    /// Clear all drawings
    ClearDrawings,
    /// Bulk-load drawings from DB (async delivery)
    LoadDrawings {
        symbol: String,
        drawings: Vec<Drawing>,
        groups: Vec<DrawingGroup>,
    },
    /// Resize the window
    Resize { width: u32, height: u32 },
    /// Close the renderer
    Shutdown,
    /// Show/reactivate the window (sent when GPU button clicked again)
    Show,
    /// Watchlist price update
    WatchlistPrice {
        symbol: String,
        price: f32,
        prev_close: f32,
    },
    /// Time & Sales tape entry
    TapeEntry {
        symbol: String,
        price: f32,
        qty: f32,
        time: i64,
        is_buy: bool,
    },
    /// Scanner bulk price update (symbol, price, prev_close, volume)
    ScannerPrice {
        symbol: String,
        price: f32,
        prev_close: f32,
        volume: u64,
    },
    /// Signal drawings from analysis server
    SignalDrawings {
        symbol: String,
        drawings_json: String, // raw JSON — parsed in gpu.rs
    },
    /// Deliver source bars for a cross-timeframe indicator
    IndicatorSourceBars {
        indicator_id: u32,
        timeframe: String,
        bars: Vec<Bar>,
        timestamps: Vec<i64>,
    },
    /// Options chain data fetched from ApexIB
    ChainData {
        symbol: String,
        dte: i32,
        underlying_price: f32, // real-time price from IB
        calls: Vec<(f32, f32, f32, f32, i32, i32, f32, bool, String)>, // strike, last, bid, ask, vol, oi, iv, itm, contract
        puts: Vec<(f32, f32, f32, f32, i32, i32, f32, bool, String)>,
    },
    /// Event markers (earnings, dividends, splits, economic) for overlay display
    EventData {
        symbol: String,
        events: Vec<(i64, String, String, String, i8)>, // (timestamp, event_type, label, details, impact)
    },
    /// Options chain data for the chart overlay (independent of sidebar chain tab)
    OverlayChainData {
        symbol: String,
        calls: Vec<(f32, f32, f32, f32, i32, i32, f32, bool, String)>,
        puts: Vec<(f32, f32, f32, f32, i32, i32, f32, bool, String)>,
    },
    /// Symbol search results from ApexIB
    SearchResults {
        query: String,
        results: Vec<(String, String)>, // (symbol, name)
        source: String, // "watchlist" or "chain"
    },
    /// Overlay bars for a secondary symbol overlay
    OverlayBars {
        symbol: String,
        bars: Vec<Bar>,
        timestamps: Vec<i64>,
    },
    /// Dark pool / off-exchange print data
    DarkPoolData {
        symbol: String,
        prints: Vec<(f32, u64, i64, i8)>, // (price, size, timestamp, side: 1=buy/-1=sell/0=unknown)
    },
    /// Candlestick pattern labels from ApexSignals (via signals feed)
    PatternLabels {
        symbol: String,
        labels: Vec<PatternLabel>,
    },
    /// Alert triggered notification from ApexSignals
    AlertTriggered {
        symbol: String,
        alert_id: String,
        price: f32,
        message: String,
    },
    /// Auto trendlines pushed from ApexSignals (replaces signal_drawings for this symbol)
    AutoTrendlines {
        symbol: String,
        drawings_json: String, // same JSON format as SignalDrawings
    },
    /// Significance score update for a drawing from ApexSignals
    SignificanceUpdate {
        symbol: String,
        drawing_id: String,
        score: f32,
        touches: u32,
        strength: String, // "WEAK", "MODERATE", "STRONG", "CRITICAL"
    },
    /// Trend health score from ApexSignals (0-100 composite)
    TrendHealthUpdate {
        symbol: String,
        timeframe: String,
        score: f32,            // 0-100
        direction: i8,         // 1=bullish, -1=bearish, 0=neutral
        exhaustion_count: u8,  // number of active exhaustion signals
        regime: String,        // "strong_trend", "weakening", "exhausted", "reversal"
    },
    /// Exit gauge score from ApexSignals (0-100 master exit signal)
    ExitGaugeUpdate {
        symbol: String,
        score: f32,            // 0-100
        urgency: String,       // "hold", "tighten", "partial", "close", "exit_now"
        components: Vec<(String, f32)>, // (engine_name, contribution)
    },
    /// Supply/demand zones from ApexSignals
    SupplyDemandZones {
        symbol: String,
        timeframe: String,
        zones: Vec<SignalZone>,
    },
    /// Precursor alert from ApexSignals (smart money front-running detected)
    PrecursorAlert {
        symbol: String,
        score: f32,            // 0-100
        direction: i8,         // 1=bullish, -1=bearish
        surge_ratio: f32,      // volume / baseline
        lead_minutes: f32,     // estimated time to move
        description: String,
    },
    /// Change-point detection — exact moment of regime shift
    ChangePointMarker {
        symbol: String,
        time: i64,             // bar timestamp of change
        change_type: String,   // "volume", "directional", "volatility", "institutional"
        confidence: f32,       // 0-1
    },
    /// Trade plan suggestion from ApexSignals
    TradePlanUpdate {
        symbol: String,
        direction: i8,         // 1=long, -1=short
        entry_price: f32,
        target_price: f32,
        stop_price: f32,
        contract_name: String, // e.g. "AAPL 195C 5DTE"
        contract_entry: f32,
        contract_target: f32,
        risk_reward: f32,
        conviction: f32,       // 0-100
        summary: String,
    },
    /// Divergence visual overlay from ApexSignals
    DivergenceOverlay {
        symbol: String,
        timeframe: String,
        divergences: Vec<DivergenceMarker>,
    },
}

/// A supply/demand zone from the signal engine.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SignalZone {
    pub zone_type: String,     // "supply", "demand", "order_block", "fvg", "breaker"
    pub price_high: f32,
    pub price_low: f32,
    pub start_time: i64,
    pub strength: f32,         // 0-10
    pub touches: u32,
    pub fresh: bool,           // untested zone
}

/// A divergence marker for chart overlay.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DivergenceMarker {
    pub indicator: String,     // "RSI", "MACD", etc.
    pub div_type: String,      // "regular_bullish", "hidden_bearish", etc.
    pub start_bar: u32,        // bar index of first point
    pub end_bar: u32,          // bar index of second point
    pub start_price: f32,
    pub end_price: f32,
    pub confidence: f32,       // 0-1
}

