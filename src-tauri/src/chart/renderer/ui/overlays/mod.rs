//! On-chart **overlay widgets** — the floating gauges / HUD cards painted onto
//! the chart canvas (distinct from panel-side *info cards* like Positions /
//! Playbook).
//!
//! System layers (built incrementally):
//!   1. [`kit`]      — shared Sx-styled painter primitives the widget bodies
//!                     compose from, plus the card shell (`overlay_card_frame`,
//!                     `overlay_card_header`). ✅
//!   2. [`indicators`] — pure indicator math, extracted out of the UI file. ✅
//!   3. [`registry`] — the `OverlayWidget` contract (`mini` + `body`): one named
//!                     interface and call seam for per-kind dispatch. ✅ (impl on
//!                     `ChartWidgetKind` lives in `super::chart_widgets`)
//!   4. data         — per-kind data, replacing the 78-field `WidgetData`. (next)
//!
//! Per-kind body logic still lives in `super::chart_widgets` behind the
//! `OverlayWidget` seam; it migrates into per-kind code one widget at a time.

pub mod kit;
pub mod indicators;
pub mod registry;
pub mod viz;
