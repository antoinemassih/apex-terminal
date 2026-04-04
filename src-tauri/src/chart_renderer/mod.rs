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

pub use types::*;

/// Line style
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LineStyle { Solid, Dashed, Dotted }

/// Drawing on the chart
#[derive(Debug, Clone)]
pub struct Drawing {
    pub id: String,
    pub kind: DrawingKind,
    pub color: String,      // hex color like "#4a9eff"
    pub opacity: f32,       // 0.0-1.0
    pub line_style: LineStyle,
    pub thickness: f32,     // pixels
    pub group_id: String,   // "default" or group UUID
}

impl Drawing {
    pub fn new(id: String, kind: DrawingKind) -> Self {
        Self { id, kind, color: "#4a9eff".into(), opacity: 1.0, line_style: LineStyle::Solid, thickness: 1.5, group_id: "default".into() }
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
    /// Symbol search results from ApexIB
    SearchResults {
        query: String,
        results: Vec<(String, String)>, // (symbol, name)
        source: String, // "watchlist" or "chain"
    },
}

