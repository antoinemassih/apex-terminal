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

unsafe impl bytemuck::Pod for Bar {}
unsafe impl bytemuck::Zeroable for Bar {}

/// Candle uniform — 80 bytes matching candles_gpu.wgsl
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct CandleUniforms {
    pub view_start: u32,
    pub view_count: u32,
    pub _pad0: u32,
    pub _pad1: u32,
    pub step_px: f32,
    pub half_step_px: f32,
    pub price_a: f32,
    pub price_b: f32,
    pub offset_px: f32,
    pub _pad2: f32,
    pub canvas_width: f32,
    pub canvas_height: f32,
    pub up_color: [f32; 4],
    pub down_color: [f32; 4],
}

unsafe impl bytemuck::Pod for CandleUniforms {}
unsafe impl bytemuck::Zeroable for CandleUniforms {}

/// Volume uniform — 80 bytes matching volume_gpu.wgsl
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct VolumeUniforms {
    pub view_start: u32,
    pub view_count: u32,
    pub _pad0: u32,
    pub _pad1: u32,
    pub bar_step_clip: f32,
    pub pixel_offset_frac: f32,
    pub body_width_clip: f32,
    pub max_volume: f32,
    pub vol_bottom_clip: f32,
    pub vol_height_clip: f32,
    pub _pad2: f32,
    pub _pad3: f32,
    pub up_color: [f32; 4],
    pub down_color: [f32; 4],
}

unsafe impl bytemuck::Pod for VolumeUniforms {}
unsafe impl bytemuck::Zeroable for VolumeUniforms {}

/// Line indicator uniform — 64 bytes matching line_gpu.wgsl
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct LineUniforms {
    pub view_start: u32,
    pub seg_count: u32,
    pub _pad0: u32,
    pub _pad1: u32,
    pub bar_step_clip: f32,
    pub pixel_offset_frac: f32,
    pub price_a: f32,
    pub price_b: f32,
    pub line_width_clip: f32,
    pub _pad2: f32,
    pub _pad3: f32,
    pub _pad4: f32,
    pub color: [f32; 4],
}

unsafe impl bytemuck::Pod for LineUniforms {}
unsafe impl bytemuck::Zeroable for LineUniforms {}

/// Grid vertex — position + color, 24 bytes
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct GridVertex {
    pub pos: [f32; 2],
    pub color: [f32; 4],
}

unsafe impl bytemuck::Pod for GridVertex {}
unsafe impl bytemuck::Zeroable for GridVertex {}
