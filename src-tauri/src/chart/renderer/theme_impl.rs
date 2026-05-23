//! `impl ComponentTheme for Theme` — the chart-app's bridge to the
//! `ui_kit::widgets::theme::ComponentTheme` contract.
//!
//! Lives here (not in `ui_kit`) so the dependency direction is correct:
//! `chart_renderer` depends on `ui_kit`, not the other way round. When
//! `ui_kit` extracts to a separate crate (see `docs/UI_EXTRACTION.md`),
//! this file is exactly where the bridge stays — the trading Theme keeps
//! satisfying the ui_kit contract from inside the trading-app crate.
//!
//! Also hosts `active_theme()`, the chart-app accessor that reads the
//! active index from egui memory and returns the live Theme. The portable
//! `active_theme_idx()` accessor stays in `ui_kit::widgets::theme`.

use egui::Color32;
use crate::ui_kit::widgets::theme::{ComponentTheme, active_theme_idx, get_ambient_theme};
use super::gpu::{Theme, live_theme_count, get_theme};

impl ComponentTheme for Theme {
    fn accent(&self) -> Color32 { self.accent }
    fn bull(&self) -> Color32 { self.bull }
    fn bear(&self) -> Color32 { self.bear }
    fn text(&self) -> Color32 { self.text }
    fn dim(&self) -> Color32 { self.dim }
    fn border(&self) -> Color32 { self.toolbar_border }
    fn border_variant(&self) -> Color32 { self.border_variant }
    fn warn(&self) -> Color32 { self.warn }
    fn bg(&self) -> Color32 { self.bg }
    fn surface(&self) -> Color32 { self.toolbar_bg }
    fn element_hover(&self) -> Color32 { self.element_hover }
    fn element_active(&self) -> Color32 { self.element_active }
    fn element_selected(&self) -> Color32 { self.element_selected }
    fn element_disabled(&self) -> Color32 { self.element_disabled }
    fn ghost_hover(&self) -> Color32 { self.ghost_hover }
    fn ghost_active(&self) -> Color32 { self.ghost_active }
    fn icon(&self) -> Color32 { self.icon }
    fn icon_muted(&self) -> Color32 { self.icon_muted }
    fn icon_disabled(&self) -> Color32 { self.icon_disabled }
    fn icon_accent(&self) -> Color32 { self.icon_accent }
    fn shadow_color(&self) -> Color32 { self.shadow_color }
}

/// Returns an owned `Theme` for the current frame. Resolution order:
///  1. The ambient theme stashed by `set_ambient_theme(ctx, theme)`
///     — the portable path used by ui_kit widgets.
///  2. Fallback: read the active idx from egui memory and pull from the
///     chart-app's live theme registry. This is the legacy path for
///     callers that don't go through `set_ambient_theme`.
pub fn active_theme(ctx: &egui::Context) -> Theme {
    if let Some(t) = get_ambient_theme(ctx) {
        return t;
    }
    let n = live_theme_count();
    let idx = active_theme_idx(ctx).min(n.saturating_sub(1));
    get_theme(idx)
}
