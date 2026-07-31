//! UI Kit — Design system for the native GPU chart.
//! Provides consistent theming, icons, and reusable widgets.
//! Some items are reserved for future use.

#[allow(dead_code)]
pub mod icons;
#[allow(dead_code)]
pub mod widgets;
pub mod symbols;
pub mod tokens;
pub mod style;
/// Taffy-backed flexbox layout (geometry only; styling stays in the tokens).
pub mod layout;
/// Typed design scales (Space/Radius/Weight/Level) — the constraint layer.
pub mod scale;
pub mod cursor;
#[allow(dead_code)]
pub mod sx;

// S7: Themable assets — fonts, icons, imagery
#[allow(dead_code)]
pub mod fonts;
#[allow(dead_code)]
pub mod assets;

/// Line-style enum — used by drawing widgets and renderers. Portable
/// primitive (no theme/state coupling); lives here so `ui_kit` doesn't have
/// to import it back from `chart_renderer`. The chart-app re-exports this
/// at `crate::chart_renderer::LineStyle` for back-compat.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LineStyle { Solid, Dashed, Dotted }
