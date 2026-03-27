//! Shared types for the chart renderer

/// OHLCV bar — matches the WebGPU storage buffer layout (6 × f32 = 24 bytes)
#[derive(Debug, Clone, Copy, serde::Deserialize)]
#[repr(C)]
pub struct Bar {
    pub open: f32,
    pub high: f32,
    pub low: f32,
    pub close: f32,
    pub volume: f32,
    pub _pad: f32,
}

// Safety: Bar is repr(C) with only f32 fields
unsafe impl bytemuck::Pod for Bar {}
unsafe impl bytemuck::Zeroable for Bar {}

/// Viewport uniform — matches candle shader layout (80 bytes)
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct CandleUniforms {
    // [0..3] u32: viewStart, viewCount, pad, pad
    pub view_start: u32,
    pub view_count: u32,
    pub _pad0: u32,
    pub _pad1: u32,
    // [16..31] f32: stepPx, halfStepPx, priceA, priceB
    pub step_px: f32,
    pub half_step_px: f32,
    pub price_a: f32,
    pub price_b: f32,
    // [32..47] f32: offsetPx, pad, canvasWidth, canvasHeight
    pub offset_px: f32,
    pub _pad2: f32,
    pub canvas_width: f32,
    pub canvas_height: f32,
    // [48..63] f32: upColor rgba
    pub up_color: [f32; 4],
    // [64..79] f32: downColor rgba
    pub down_color: [f32; 4],
}

unsafe impl bytemuck::Pod for CandleUniforms {}
unsafe impl bytemuck::Zeroable for CandleUniforms {}
