//! Design tokens — re-exported from `ui_kit::style` (portable home) plus
//! the legacy chart_renderer glob for back-compat.
//!
//! ### P5b extraction status (audit's HARD blocker)
//! Every ui_kit widget that REQUIRED chart-app state has been migrated
//! to read from `TokenSnapshot` via `frame_tokens()` (P5b: 5 sites in
//! button/input/toast). ButtonTreatment moved into
//! `ui_kit/widgets/tokens.rs`. `focus_ring_alpha`, `focus_ring_width`,
//! `toast_bg_alpha`, `button_treatment` now live on TokenSnapshot —
//! the chart-app populates them per frame in `begin_frame()`.
//!
//! ### Remaining glob
//! The `pub use chart_renderer::ui::style::*` glob below is RETAINED for
//! chart-app callers that import chart-specific helpers (themed shadows,
//! `current()`, `panel_surface(t)`, brand colors, dialog chrome helpers,
//! etc.) through this bridge module. New ui_kit widget code MUST NOT
//! depend on anything pulled through this glob — use the explicit
//! `ui_kit::style::*` re-exports above or read from `frame_tokens()`.
//!
//! Severing the glob fully requires migrating ~40 chart-app callers to
//! import directly from `chart_renderer::ui::style` instead of through
//! `ui_kit::tokens`. That's a focused per-callsite sweep — tracked as
//! the remaining extraction work in `docs/UI_EXTRACTION.md`.

#[allow(unused_imports)]
pub use crate::ui_kit::style::*;
#[allow(unused_imports)]
pub use crate::chart_renderer::ui::style::*;
