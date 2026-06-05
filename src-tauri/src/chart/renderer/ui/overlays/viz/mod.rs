//! **Viz** — a stylable, theme-aware data-visualization library for the chart
//! overlays (and the dashboard). Painter-based primitives composed by widget
//! bodies; every primitive re-tints with the active `ColorScheme` (Sx ramps) and
//! re-shapes with the active `StyleSystem` (via [`style::ChartStyle`]).
//!
//! Layout:
//!   - [`style`]  — `ChartStyle` (the stylable dimension spec), `LinePattern`,
//!                  `FillMode`, colour sequencing, pattern-aware line stroking.
//!   - [`charts`] — the primitives (stat, bars, area/line, histogram, donut, pie,
//!                  radar, heatmap, multi-ring). More waves to come (sunburst,
//!                  treemap, violin, box-bar, …).

pub mod style;
pub mod charts;
