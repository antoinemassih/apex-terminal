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

pub use types::*;

use std::sync::mpsc;
use std::thread;

/// Drawing on the chart
#[derive(Debug, Clone)]
pub struct Drawing {
    pub id: String,
    pub kind: DrawingKind,
    pub color: [f32; 4],
    pub width: f32,
    pub dashed: bool,
}

#[derive(Debug, Clone)]
pub enum DrawingKind {
    HLine { price: f32 },
    TrendLine { price0: f32, bar0: f32, price1: f32, bar1: f32 },
    HZone { price0: f32, price1: f32 },
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
    /// Resize the window
    Resize { width: u32, height: u32 },
    /// Close the renderer
    Shutdown,
}

/// Handle to the native chart renderer thread
pub struct ChartRendererHandle {
    tx: mpsc::Sender<ChartCommand>,
    thread: Option<thread::JoinHandle<()>>,
}

impl ChartRendererHandle {
    /// Send a command to the renderer (non-blocking)
    pub fn send(&self, cmd: ChartCommand) {
        let _ = self.tx.send(cmd);
    }

    /// Shut down the renderer and wait for the thread to exit
    pub fn shutdown(mut self) {
        let _ = self.tx.send(ChartCommand::Shutdown);
        if let Some(handle) = self.thread.take() {
            let _ = handle.join();
        }
    }
}

/// Spawn the native chart renderer on a dedicated thread.
/// Returns a handle for sending commands from the Tauri main thread.
pub fn spawn(title: &str, width: u32, height: u32) -> ChartRendererHandle {
    let (tx, rx) = mpsc::channel::<ChartCommand>();
    let title = title.to_string();

    let thread = thread::spawn(move || {
        gpu::run_render_loop(&title, width, height, rx);
    });

    ChartRendererHandle {
        tx,
        thread: Some(thread),
    }
}
