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
    TrendLine { price0: f32, bar0: f32, price1: f32, bar1: f32 },
    HZone { price0: f32, price1: f32 },
    BarMarker { bar: f32, price: f32, up: bool },
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
}

