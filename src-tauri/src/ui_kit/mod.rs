//! UI Kit — Design system for the native GPU chart.
//! Provides consistent theming, icons, and reusable widgets.
//! Some items are reserved for future use.

#[allow(dead_code)]
pub mod theme;
#[allow(dead_code)]
pub mod icons;
#[allow(dead_code)]
pub mod widgets;
pub mod symbols;
pub mod tokens;
pub mod style;
pub mod cursor;

/// Line-style enum — used by drawing widgets and renderers. Portable
/// primitive (no theme/state coupling); lives here so `ui_kit` doesn't have
/// to import it back from `chart_renderer`. The chart-app re-exports this
/// at `crate::chart_renderer::LineStyle` for back-compat.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LineStyle { Solid, Dashed, Dotted }
