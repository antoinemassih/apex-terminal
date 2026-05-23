//! Design tokens — re-exported from BOTH `ui_kit::style` (the canonical
//! portable home) AND `chart_renderer::ui::style` (for the chart-app
//! helpers ui_kit widgets still call: `mono_sm` / `contrast_fg` /
//! `cursor` / etc).
//!
//! When `ui_kit` extracts to a workspace crate, this file becomes
//! `pub use crate::style::*` (drop the chart_renderer glob); ui_kit
//! widgets that call chart-app helpers (mono_*, contrast_fg, cursor)
//! either get equivalents in ui_kit::style or rewire to import from
//! chart_renderer directly.

#[allow(unused_imports)]
pub use crate::ui_kit::style::*;
// Bridge to chart-app style helpers that are genuinely state-bearing
// (`current()` reads StyleSettings) or chart-app-specific (Theme-taking
// `panel_surface(t)` etc., button treatment enums). These prevent the
// literal workspace crate move; each one needs to either migrate into
// ui_kit (via a trait method or owned definition) or have its ui_kit
// callers rewired. Tracked in docs/UI_EXTRACTION.md.
#[allow(unused_imports)]
pub use crate::chart_renderer::ui::style::*;
