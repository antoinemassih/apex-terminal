//! On-chart **overlay widgets** — the floating gauges / HUD cards painted onto
//! the chart canvas (distinct from panel-side *info cards* like Positions /
//! Playbook).
//!
//! System layers (built incrementally):
//!   1. [`kit`]  — shared Sx-styled painter primitives the widget bodies compose
//!                 from, instead of each hand-painting. (this step)
//!   2. trait    — `OverlayWidget` { meta, mini, body } per kind (next).
//!   3. card     — one `OverlayCard` chrome (shell, modes, docking). (next)
//!   4. data     — per-kind data, replacing the 78-field `WidgetData`. (next)
//!
//! Legacy rendering still lives in `super::chart_widgets`; widgets migrate onto
//! the kit one at a time.

pub mod kit;
pub mod indicators;
