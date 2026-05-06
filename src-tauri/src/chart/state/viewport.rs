//! Visible window over the chart.

use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Viewport {
    pub from_ts_ns: i64,
    pub to_ts_ns: i64,
    pub price_low: f32,
    pub price_high: f32,
    #[serde(default)]
    pub log_scale: bool,
}
