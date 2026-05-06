//! Shared types for the chart renderer

/// OHLCV bar
#[derive(Debug, Clone, Copy, serde::Deserialize)]
pub struct Bar {
    pub open: f32,
    pub high: f32,
    pub low: f32,
    pub close: f32,
    pub volume: f32,
    pub _pad: f32,
}
