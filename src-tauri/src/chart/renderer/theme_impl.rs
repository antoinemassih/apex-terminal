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
use crate::ui_kit::widgets::theme::{ComponentTheme, PortableTheme, active_theme_idx, get_ambient_theme};
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
    // Per-style: editorial styles (Mariner/Alto/Relay) render section headers
    // monospace; others proportional. Read from the active StyleSystem token.
    fn section_header_mono(&self) -> bool {
        crate::chart_renderer::ui::style::current().section_header_mono
    }
    // Cards float (rounded + shadow) only on tiled styles (region_gap > 0).
    fn cards_float(&self) -> bool {
        crate::chart_renderer::ui::style::current().region_gap > 0.0
    }
    // Per-style row treatment — read the live StyleSystem `wl_row_*` tokens so
    // the generic PanelListRow matches the RowShell-based WatchlistRow on every
    // style (pill-inset capsules on Aperture/Glass, ledger hairlines on Alto/
    // Mariner). Single source of truth: the same tokens WatchlistRow reads.
    fn row_side_margin(&self) -> f32 {
        crate::chart_renderer::ui::style::current().wl_row_side_margin
    }
    fn row_corner_radius(&self) -> u8 {
        crate::chart_renderer::ui::style::current().wl_row_corner_radius
    }
    fn row_divider_alpha(&self) -> u8 {
        crate::chart_renderer::ui::style::current().wl_row_divider_alpha
    }
    fn row_height(&self) -> f32 {
        crate::chart_renderer::ui::style::style_row_height()
    }
    // ── Derived overlays (Zed-style) ────────────────────────────────────
    // Source: gpu.rs Theme struct doc; previously stored as 10 redundant
    // fields on Theme + 15 initializer copies. Now derived from text/accent
    // at the trait boundary so palettes only need to declare the base 6
    // colors. Identical resolved values vs. the prior table.
    fn element_hover(&self)    -> Color32 { color_alpha(self.text,   12) }
    fn element_active(&self)   -> Color32 { color_alpha(self.text,   24) }
    fn element_selected(&self) -> Color32 { color_alpha(self.accent, 24) }
    fn element_disabled(&self) -> Color32 { color_alpha(self.dim,    80) }
    fn ghost_hover(&self)      -> Color32 { color_alpha(self.text,    6) }
    fn ghost_active(&self)     -> Color32 { color_alpha(self.text,   12) }
    fn icon(&self)             -> Color32 { self.text }
    fn icon_muted(&self)       -> Color32 { color_alpha(self.text,  178) }
    fn icon_disabled(&self)    -> Color32 { color_alpha(self.text,  102) }
    fn icon_accent(&self)      -> Color32 { self.accent }
    fn shadow_color(&self)     -> Color32 { self.shadow_color }
}

#[inline]
fn color_alpha(c: Color32, a: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a)
}

/// Returns an owned `Theme` for the current frame. Resolution order:
///  1. The ambient theme stashed by `set_ambient_theme(ctx, theme)`
///     — the chart-app path used by chart-renderer code that needs the
///     full Theme (with bull/bear and all 30+ fields).
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

/// Bridge: copy a chart `Theme` into a portable `PortableTheme` so it can
/// be ambient-stashed for ui_kit widgets that now read PortableTheme via
/// the portable `ui_kit::widgets::theme::active_theme()` accessor.
///
/// Defined here (not in `ui_kit`) because of Rust's orphan rules: we can't
/// `impl From<&Theme> for PortableTheme` from chart_renderer (both types
/// are foreign relative to `From`). A free function is the simplest fix.
///
/// Called once per frame from `gpu::setup_theme` (P5b Step 3 wire-up).
pub fn theme_to_portable(t: &Theme) -> PortableTheme {
    PortableTheme {
        accent:           t.accent,
        bull:             t.bull,
        bear:             t.bear,
        text:             t.text,
        dim:              t.dim,
        border:           t.toolbar_border,
        border_variant:   t.border_variant,
        warn:             t.warn,
        bg:               t.bg,
        surface:          t.toolbar_bg,
        element_hover:    color_alpha(t.text,   12),
        element_active:   color_alpha(t.text,   24),
        element_selected: color_alpha(t.accent, 24),
        element_disabled: color_alpha(t.dim,    80),
        ghost_hover:      color_alpha(t.text,    6),
        ghost_active:     color_alpha(t.text,   12),
        icon:             t.text,
        icon_muted:       color_alpha(t.text,  178),
        icon_disabled:    color_alpha(t.text,  102),
        icon_accent:      t.accent,
        shadow_color:     t.shadow_color,
        // M0.4: snapshot the per-style flags at conversion time so the
        // ambient PortableTheme answers these the same way the full Theme
        // does (Theme reads `current()` live; the stash is refreshed every
        // frame in setup_theme, so this stays in sync).
        section_header_mono: crate::chart_renderer::ui::style::current().section_header_mono,
        cards_float:         crate::chart_renderer::ui::style::current().region_gap > 0.0,
    }
}
