//! `SidePanel` trait — the unified contract every side panel implements.
//!
//! Each panel owns its own state struct (or borrows shared state from the app)
//! and renders into the egui context via `draw(ctx, &mut PanelContext)`. The
//! context bundles theme, panes, watchlist, and any other shared globals
//! the panel needs — so call sites pass ONE thing instead of N args.
//!
//! # Migration status
//!
//! Panels wrapped: watchlist_panel, discord_panel, script_panel, spread_panel,
//! screenshot_panel, journal_panel, settings_panel, indicator_editor,
//! hotkey_editor, option_quick_picker, apex_diagnostics.
//!
//! Panels skipped (bespoke args — see TODO comments in each file):
//! - dom_panel: takes a raw `ui: &mut Ui` + many bespoke state refs (price
//!   ladder, drag state, order refs). Not a top-level `ctx` panel.
//! - plays_panel: `draw_content` only — renders inside an existing `Ui`, not
//!   a standalone `ctx` panel.
//! - portfolio_pane: `render` takes extra pane geometry args (`pane_rects`,
//!   `visible_count`, `account_data`). Pane-type, not a side panel.
//! - spreadsheet_pane: `render` is a pane-type renderer, not a side panel.

use egui;
use crate::chart_renderer::gpu::{Chart, Theme, Watchlist};

/// Shared context passed to every panel's `draw` call.
///
/// Panels that don't need a particular field can simply ignore it.
pub struct PanelContext<'a> {
    pub theme: &'a Theme,
    pub panes: &'a mut Vec<Chart>,
    pub active_pane: &'a mut usize,
    pub watchlist: &'a mut Watchlist,
}

/// Unified contract for every side panel.
pub trait SidePanel {
    /// Returns whether this panel is currently visible.
    ///
    /// Implementations that manage their own visibility internally
    /// (e.g., guard with `if !watchlist.foo_open { return; }` inside `draw`)
    /// may return `true` unconditionally and let `draw` short-circuit itself.
    fn is_open(&self) -> bool;

    /// Renders the panel into `ctx` if visible.
    fn draw(&mut self, ctx: &egui::Context, cx: &mut PanelContext<'_>);
}

// ── Concrete wrappers ────────────────────────────────────────────────────────
// Each wrapper is a zero-size unit struct that delegates to the existing
// free function in its module. No behaviour change; purely additive.

/// Wrapper for [`watchlist_panel`].
pub struct WatchlistPanel;
impl SidePanel for WatchlistPanel {
    fn is_open(&self) -> bool { true /* watchlist_panel::draw guards internally */ }
    fn draw(&mut self, ctx: &egui::Context, cx: &mut PanelContext<'_>) {
        super::watchlist_panel::draw(ctx, cx.watchlist, cx.panes, *cx.active_pane, cx.theme);
    }
}

/// Wrapper for [`discord_panel`].
pub struct DiscordPanel;
impl SidePanel for DiscordPanel {
    fn is_open(&self) -> bool { true /* discord_panel::draw guards internally */ }
    fn draw(&mut self, ctx: &egui::Context, cx: &mut PanelContext<'_>) {
        super::discord_panel::draw(ctx, cx.watchlist, cx.theme);
    }
}

/// Wrapper for [`script_panel`].
pub struct ScriptPanel;
impl SidePanel for ScriptPanel {
    fn is_open(&self) -> bool { true /* script_panel::draw guards internally */ }
    fn draw(&mut self, ctx: &egui::Context, cx: &mut PanelContext<'_>) {
        super::script_panel::draw(ctx, cx.watchlist, cx.theme);
    }
}

/// Wrapper for [`spread_panel`].
///
/// `spread_panel::draw` takes an `active_symbol: &str` which we derive from
/// the active pane.  If there are no panes the panel simply will not render.
pub struct SpreadPanel;
impl SidePanel for SpreadPanel {
    fn is_open(&self) -> bool { true /* spread_panel::draw guards internally */ }
    fn draw(&mut self, ctx: &egui::Context, cx: &mut PanelContext<'_>) {
        let sym = cx.panes.get(*cx.active_pane)
            .map(|p| p.symbol.clone())
            .unwrap_or_default();
        super::spread_panel::draw(ctx, cx.watchlist, &sym, cx.theme);
    }
}

/// Wrapper for [`screenshot_panel`].
pub struct ScreenshotPanel;
impl SidePanel for ScreenshotPanel {
    fn is_open(&self) -> bool { true /* screenshot_panel::draw guards internally */ }
    fn draw(&mut self, ctx: &egui::Context, cx: &mut PanelContext<'_>) {
        super::screenshot_panel::draw(ctx, cx.watchlist, cx.theme, cx.panes, *cx.active_pane);
    }
}

/// Wrapper for [`journal_panel`].
pub struct JournalPanel;
impl SidePanel for JournalPanel {
    fn is_open(&self) -> bool { true /* journal_panel::draw guards internally */ }
    fn draw(&mut self, ctx: &egui::Context, cx: &mut PanelContext<'_>) {
        super::journal_panel::draw(ctx, cx.watchlist, cx.theme);
    }
}

/// Wrapper for [`settings_panel`].
///
/// `settings_panel::draw` takes `chart: &mut Chart` (the active pane) and
/// `ap: usize`. Derived from `PanelContext`.
pub struct SettingsPanel;
impl SidePanel for SettingsPanel {
    fn is_open(&self) -> bool { true /* settings_panel::draw guards internally */ }
    fn draw(&mut self, ctx: &egui::Context, cx: &mut PanelContext<'_>) {
        let ap = *cx.active_pane;
        if let Some(chart) = cx.panes.get_mut(ap) {
            super::settings_panel::draw(ctx, cx.watchlist, chart, cx.theme, ap);
        }
    }
}

/// Wrapper for [`indicator_editor`].
pub struct IndicatorEditor;
impl SidePanel for IndicatorEditor {
    fn is_open(&self) -> bool { true /* indicator_editor::draw guards internally */ }
    fn draw(&mut self, ctx: &egui::Context, cx: &mut PanelContext<'_>) {
        super::super::tools::indicator_editor::draw(ctx, cx.watchlist, cx.panes, *cx.active_pane, cx.theme);
    }
}

/// Wrapper for [`hotkey_editor`].
pub struct HotkeyEditor;
impl SidePanel for HotkeyEditor {
    fn is_open(&self) -> bool { true /* hotkey_editor::draw guards internally */ }
    fn draw(&mut self, ctx: &egui::Context, cx: &mut PanelContext<'_>) {
        super::super::tools::hotkey_editor::draw(ctx, cx.watchlist, cx.panes, *cx.active_pane, cx.theme);
    }
}

/// Wrapper for [`option_quick_picker`].
pub struct OptionQuickPicker;
impl SidePanel for OptionQuickPicker {
    fn is_open(&self) -> bool { true /* option_quick_picker::draw guards per-pane */ }
    fn draw(&mut self, ctx: &egui::Context, cx: &mut PanelContext<'_>) {
        super::super::tools::option_quick_picker::draw(ctx, cx.watchlist, cx.panes, *cx.active_pane, cx.theme);
    }
}

/// Wrapper for [`apex_diagnostics`].
pub struct ApexDiagnostics;
impl SidePanel for ApexDiagnostics {
    fn is_open(&self) -> bool { true /* apex_diagnostics::draw guards internally */ }
    fn draw(&mut self, ctx: &egui::Context, cx: &mut PanelContext<'_>) {
        super::apex_diagnostics::draw(ctx, cx.watchlist, cx.theme);
    }
}

// ── Skipped panels ───────────────────────────────────────────────────────────
// dom_panel       — TODO(SidePanel-trait): bespoke args (ui, price ladder,
//                   drag state, order refs). Renders into an existing Ui,
//                   not a top-level Context panel.
// plays_panel     — TODO(SidePanel-trait): bespoke args (draw_content only,
//                   renders inside an existing Ui tab).
// portfolio_pane  — TODO(SidePanel-trait): bespoke args (pane_rects,
//                   visible_count, account_data). Pane-type renderer.
// spreadsheet_pane — TODO(SidePanel-trait): bespoke args (pane_idx, rect).
//                   Pane-type renderer, not a side panel.
