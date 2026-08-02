//! Top-nav toolbar rendering — extracted from `gpu.rs`.
//!
//! This module owns `render(...)`, the function that draws the
//! `egui::TopBottomPanel::top("tb")` toolbar, all its dropdowns, the account
//! strip, the layout-dropdown popup, all sub-panels (command palette, settings,
//! order toasts, side panels, alert checker, deferred watchlist tooltip, etc.).
//!
//! It was previously a private free function `render_toolbar(...)` in `gpu.rs`.
//! The move is structural-only — every closure, mutation and click handler is
//! unchanged.  See commit message for the line count moved.

#![allow(unused_imports, unused_variables, clippy::too_many_arguments)]

/// Builder-style entry point for the top-nav toolbar.
///
/// Usage:
/// ```ignore
/// TopNav::new()
///     .panes(panes)
///     .active_pane(active_pane)
///     .layout(layout)
///     .watchlist(watchlist)
///     .theme(t, theme_idx)
///     .account(account_data_cached.as_ref())
///     .window(win_ref)
///     .conn_panel_open(conn_panel_open)
///     .toasts(toasts)
///     .show(ctx);
/// ```
pub struct TopNav<'a> {
    panes: Option<&'a mut Vec<Chart>>,
    active_pane: Option<&'a mut usize>,
    layout: Option<&'a mut Layout>,
    watchlist: Option<&'a mut Watchlist>,
    theme: Option<&'a Theme>,
    theme_idx: usize,
    account_data_cached: Option<&'a Option<(AccountSummary, Vec<Position>, Vec<IbOrder>)>>,
    window: Option<Arc<Window>>,
    conn_panel_open: Option<&'a mut bool>,
    toasts: &'a [crate::chart_renderer::ui::tools::notification::Notification],
}

impl<'a> TopNav<'a> {
    pub fn new() -> Self {
        Self {
            panes: None,
            active_pane: None,
            layout: None,
            watchlist: None,
            theme: None,
            theme_idx: 0,
            account_data_cached: None,
            window: None,
            conn_panel_open: None,
            toasts: &[],
        }
    }

    pub fn panes(mut self, p: &'a mut Vec<Chart>) -> Self { self.panes = Some(p); self }
    pub fn active_pane(mut self, p: &'a mut usize) -> Self { self.active_pane = Some(p); self }
    pub fn layout(mut self, l: &'a mut Layout) -> Self { self.layout = Some(l); self }
    pub fn watchlist(mut self, w: &'a mut Watchlist) -> Self { self.watchlist = Some(w); self }
    pub fn theme(mut self, t: &'a Theme, idx: usize) -> Self { self.theme = Some(t); self.theme_idx = idx; self }
    pub fn account(mut self, a: Option<&'a Option<(AccountSummary, Vec<Position>, Vec<IbOrder>)>>) -> Self { self.account_data_cached = a; self }
    pub fn window(mut self, w: Option<Arc<Window>>) -> Self { self.window = w; self }
    pub fn conn_panel_open(mut self, b: &'a mut bool) -> Self { self.conn_panel_open = Some(b); self }
    pub fn toasts(mut self, t: &'a [crate::chart_renderer::ui::tools::notification::Notification]) -> Self { self.toasts = t; self }

    pub fn show(self, ctx: &egui::Context) {
        render(
            ctx,
            self.panes.expect("TopNav requires .panes(...)"),
            self.active_pane.expect("TopNav requires .active_pane(...)"),
            self.layout.expect("TopNav requires .layout(...)"),
            self.watchlist.expect("TopNav requires .watchlist(...)"),
            self.theme.expect("TopNav requires .theme(...)"),
            self.theme_idx,
            self.account_data_cached.unwrap_or(&None),
            self.window,
            self.conn_panel_open.expect("TopNav requires .conn_panel_open(...)"),
            self.toasts,
        );
    }
}

use std::sync::Arc;
use crate::chart_renderer::ui::style::tint;
use crate::ui_kit::sx::Tone;
use winit::window::Window;

use crate::ui_kit::icons::Icon;
use crate::ui_kit::widgets::{Button as KitButton, MenuItem, NumberStepper, SelectableRow, Tooltip, tokens::{Variant as KitVariant, Size as KitSize}};
use crate::ui_kit::widgets::icon_placement::IconPlacement;
use crate::chart_renderer::gpu::{
    Chart, Layout, Watchlist, Theme,
    CURRENT_WINDOW, CLOSE_REQUESTED, TB_BTN_CLICKED, PENDING_WL_TOOLTIP,
    WlTooltipData,
    ALL_LAYOUTS,
    APEXIB_URL,
    CandleMode, VolumeProfileMode,
    IndicatorType, IndicatorCategory, Indicator, INDICATOR_COLORS,
    EventMarker, DarkPoolPrint,
    get_theme, indicator_default_color,
    rgb,
    save_workspace, list_workspaces, save_state, save_templates,
    widget_description, paint_widget_preview,
    new_uuid,
};
use crate::chart_renderer::ui::style::{
    color_alpha, color_subtle, color_muted, color_half, color_dim, color_very_dim, hex_to_color, segmented_control,
    contrast_fg,
    alpha_faint, alpha_ghost, alpha_soft, alpha_muted, alpha_dim, alpha_strong, alpha_heavy,
    BTN_ICON_SM, BTN_ICON_LG,
    icon_sm,
    set_toolbar_rect, tb_group_break, current as style_current, paint_bevel,
    paint_gradient_highlight, current_style_bevel_hi,
    font_4xs, font_xs, font_2xs, font_sm, font_md, font_lg, font_xl,
    mono_xs, mono_sm, mono_md, mono_lg,
    gap_2xs, gap_xs, gap_sm, gap_md, gap_lg, gap_xl,
    row_height_default,
    stroke_std, stroke_thin, r_md_cr,
    elevation_3, shadow_card_themed,
};
use crate::chart_renderer::ui::foundation::text_style::TextStyle;
use crate::chart_renderer::trading::{AccountSummary, Position, IbOrder, OrderStatus};
use crate::chart_renderer::{ChartCommand, ChartWidgetKind, ChartWidget, DrawingGroup};
use crate::state::{BROADCAST_GROUP, PaneEvent, PaneToggle};
use crate::chart_renderer::commands::{self, AppCommand, ChartFlag};
use super::toolbar_btn;

/// Wave 13a helper: publish a `PaneEvent::ToggleChanged` for the
/// fan-out flow used by top_nav menu toggles. Caller has already
/// flipped the originator's field (`panes[ap].<field> = nv`); this
/// just broadcasts the new value to sibling panes when the user held
/// Shift or broadcast mode is on. The originator is skipped by
/// `apply_pane_events` via the `Some(ap)` origin tag, so the caller
/// never double-writes.
pub(crate) fn publish_toggle(
    watchlist: &mut Watchlist,
    fan_out: bool,
    kind: PaneToggle,
    value: bool,
    ap: usize,
) {
    if !fan_out { return; }
    watchlist.subscriptions.publish_from(
        PaneEvent::ToggleChanged { group: BROADCAST_GROUP, kind, value },
        ap,
    );
}

/// Wave 13a helper: sibling of `publish_toggle` for the tri-state
/// `swing_leg_mode` toggle (u8 cycling 0→1→2→0).
pub(crate) fn publish_swing_leg_mode(
    watchlist: &mut Watchlist,
    fan_out: bool,
    value: u8,
    ap: usize,
) {
    if !fan_out { return; }
    watchlist.subscriptions.publish_from(
        PaneEvent::SwingLegModeChanged { group: BROADCAST_GROUP, value },
        ap,
    );
}

// REMOVED (2026-07-31): `paint_nav_col_tint` — a bespoke full-toolbar-height
// column tint that was painted *behind* nav menu buttons on hover/active. It
// stacked on top of the unified `Button`'s own Ghost hover/active fill, so
// every nav item rendered two mismatched highlights for the same state. It was
// neutered by an `if true { return; }` guard on 2026-07-30; the function and
// all of its call sites are now deleted. The unified Button's Ghost
// hover/active fill is the single nav highlight.

/// All supported timeframes — `(label, seconds_per_bar, group)`. Group is for
/// the dropdown's section headers ("Seconds", "Minutes", "Hours", "Days+").
/// Order here is the display order in the dropdown AND the canonical sort
/// order for the favorites segmented control.
pub(crate) const ALL_TIMEFRAMES: &[(&str, u32, &str)] = &[
    ("1s",   1,       "Seconds"),
    ("5s",   5,       "Seconds"),
    ("10s",  10,      "Seconds"),
    ("15s",  15,      "Seconds"),
    ("30s",  30,      "Seconds"),
    ("1m",   60,      "Minutes"),
    ("2m",   120,     "Minutes"),
    ("3m",   180,     "Minutes"),
    ("5m",   300,     "Minutes"),
    ("10m",  600,     "Minutes"),
    ("15m",  900,     "Minutes"),
    ("30m",  1800,    "Minutes"),
    ("45m",  2700,    "Minutes"),
    ("1h",   3600,    "Hours"),
    ("2h",   7200,    "Hours"),
    ("3h",   10800,   "Hours"),
    ("4h",   14400,   "Hours"),
    ("6h",   21600,   "Hours"),
    ("8h",   28800,   "Hours"),
    ("12h",  43200,   "Hours"),
    ("1d",   86400,   "Days+"),
    ("2d",   172800,  "Days+"),
    ("3d",   259200,  "Days+"),
    ("1wk",  604800,  "Days+"),
    ("2wk",  1209600, "Days+"),
    ("1mo",  2592000, "Days+"),
    ("3mo",  7776000, "Days+"),
    ("1y",   31536000,"Days+"),
];

pub(crate) fn tf_to_secs(tf: &str) -> u32 {
    ALL_TIMEFRAMES.iter().find(|t| t.0 == tf).map(|t| t.1).unwrap_or(0)
}

/// Apply the standard menu/dropdown background style so all pop-up menus
/// share the same `toolbar_bg` fill instead of the default egui window fill.
/// Call once at the top of every `.show_menu()` / `.show()` closure.
#[inline]
pub(crate) fn apply_menu_style(ui: &mut egui::Ui, t: &Theme) {
    ui.style_mut().visuals.widgets.inactive.bg_fill = t.toolbar_bg;
    ui.style_mut().visuals.window_fill             = t.toolbar_bg;
}

/// Standard tri-state text colour for dropdown / list rows.
/// - Current/selected → accent
/// - Hovered          → primary text
/// - Default          → dim
#[inline]
pub(crate) fn row_text_color(is_current: bool, hovered: bool, t: &Theme) -> egui::Color32 {
    if is_current { t.accent } else if hovered { t.text } else { t.dim }
}

/// Chart controls cluster — interval / drawing tools / object-tree / indicators
/// / widgets (+ alt-bar settings). Lives in the toolbar (toolnav) when it is
/// visible, else falls back inline in the top-nav row so it stays reachable
/// when the toolbar is toggled off. `tb_rect` is the host row's rect, used to
/// bound the button-group enclosure boxes.
/// Delegates to [`super::chart_controls::render`] — the body was extracted
/// to keep `top_nav.rs` focused on the toolbar chrome and dropdowns.
pub(crate) fn render_chart_controls(
    ui: &mut egui::Ui,
    watchlist: &mut Watchlist,
    panes: &mut [Chart],
    ap: usize,
    t: &Theme,
    tb_rect: egui::Rect,
) {
    super::chart_controls::render(ui, watchlist, panes, ap, t, tb_rect);
}


pub(crate) fn render(
    ctx: &egui::Context,
    panes: &mut Vec<Chart>,
    active_pane: &mut usize,
    layout: &mut Layout,
    watchlist: &mut Watchlist,
    t: &Theme,
    theme_idx: usize,
    account_data_cached: &Option<(AccountSummary, Vec<Position>, Vec<IbOrder>)>,
    win_ref: Option<Arc<Window>>,
    conn_panel_open: &mut bool,
    toasts: &[crate::chart_renderer::ui::tools::notification::Notification],
) {
    // Sync notification routing prefs (toolbar vs toasts, per-category) from the
    // persisted user settings each frame so pushes route correctly.
    crate::chart_renderer::ui::tools::notification::set_routing(
        crate::chart_renderer::ui::tools::notification::routing_from_ctx(ctx),
    );
    {
        use std::sync::Once;
        static SHORTCUTS_REGISTERED: Once = Once::new();
        SHORTCUTS_REGISTERED.call_once(|| {
            use crate::foundation::shortcuts::{register, shortcut_cmd, ShortcutEntry, Shortcut};
            register(ShortcutEntry {
                shortcut: shortcut_cmd(egui::Key::L),
                action: "panel.order_ledger_toggle",
                description: "Toggle order ledger panel",
                category: "Panels",
            });
            // B3 — MSG panel shortcuts (Ctrl+Shift+R/I/T)
            let cmd_shift = egui::Modifiers { command: true, shift: true, ..egui::Modifiers::NONE };
            for (key, action, desc) in [
                (egui::Key::R, "panel.msg_rrg_toggle",      "Toggle MSG Weighted RRG panel"),
                (egui::Key::I, "panel.msg_influence_toggle","Toggle MSG Influence heatmap panel"),
                (egui::Key::T, "panel.msg_tension_toggle",  "Toggle MSG Scenario ladder panel"),
            ] {
                let _ = crate::foundation::shortcuts::registry()
                    .write().unwrap()
                    .register(ShortcutEntry { shortcut: Shortcut { modifiers: cmd_shift, key }, action, description: desc, category: "Panels" });
            }
            // UX-1 Fix 1: Alt+S focuses the symbol input in the top toolbar.
            if let Err(e) = crate::foundation::shortcuts::registry().write().unwrap().register(ShortcutEntry {
                shortcut: Shortcut {
                    modifiers: egui::Modifiers { alt: true, ..egui::Modifiers::NONE },
                    key: egui::Key::S,
                },
                action: "nav.focus_symbol_input",
                description: "Focus symbol input in toolbar",
                category: "Navigation",
            }) {
                eprintln!("[shortcuts] {}", e);
            }
            // Wave S2 — Ctrl+Shift+S opens/closes the screener panel.
            let _ = crate::foundation::shortcuts::registry().write().unwrap().register(ShortcutEntry {
                shortcut: Shortcut { modifiers: cmd_shift, key: egui::Key::S },
                action: "panel.screener_toggle",
                description: "Toggle Screener panel (symbol scanner + builder + results)",
                category: "Panels",
            });
        });
    }
    use crate::monitoring::{span_begin, span_end};
    let ap = *active_pane;
    span_begin("top_panel");

    // ── Trading-block banner (kill / halt / auto-halt) ──────────────────────
    // Renders BEFORE the toolbar so it always sits at the very top of the
    // window, regardless of the toolbar auto-hide state. Operator must see
    // this immediately when trading is gated.
    {
        use crate::chart_renderer::trading::order_manager;
        if order_manager::is_trading_blocked() {
            let (auto, kill, halted) = order_manager::trading_block_reason();
            let (msg, fill) = if kill {
                (
                    "\u{1F6D1} KILL SWITCH ENGAGED \u{2014} new orders blocked",
                    t.bear,
                )
            } else if halted && auto {
                (
                    "\u{26A0} BROKER DISCONNECTED \u{2014} auto-halted, will resume on reconnect",
                    t.bear,
                )
            } else {
                // user-engaged halt
                (
                    "\u{23F8} TRADING HALTED \u{2014} press Ctrl+Shift+R to resume",
                    t.warn,
                )
            };
            egui::TopBottomPanel::top("trading_block_banner")
                .exact_height(28.0)
                .frame(egui::Frame::NONE
                    .fill(fill)
                    .inner_margin(egui::Margin { left: gap_md() as i8, right: gap_md() as i8, top: 4, bottom: 4 }))
                .show(ctx, |ui| {
                    let fg = crate::chart_renderer::ui::style::contrast_fg(fill);
                    ui.horizontal_centered(|ui| {
                        ui.label(TextStyle::Body.as_rich_cascading(msg, fg).strong());
                    });
                });
        }
    }

    // Auto-hide toolbar logic
    let toolbar_visible = if watchlist.toolbar_auto_hide {
        let mouse_y = ctx.input(|i| i.pointer.hover_pos().map(|p| p.y));
        let tb_h = if watchlist.compact_mode { 28.0 } else { 36.0 };
        let in_trigger_zone = mouse_y.map_or(false, |y| y < 8.0);
        let in_toolbar = mouse_y.map_or(false, |y| y < tb_h);
        if in_trigger_zone || in_toolbar {
            watchlist.toolbar_hover_time = Some(std::time::Instant::now());
            true
        } else if let Some(t_hover) = watchlist.toolbar_hover_time {
            if t_hover.elapsed().as_millis() < 500 { true }
            else { watchlist.toolbar_hover_time = None; false }
        } else {
            false
        }
    } else {
        true
    };

    if !toolbar_visible {
        // Show thin accent hint line at the very top
        egui::TopBottomPanel::top("tb_hint")
            .exact_height(2.0)
            .frame(egui::Frame::NONE.fill(t.accent))
            .show(ctx, |_ui| {});
    }

    let mut order_clicked = false;
    if toolbar_visible {
    // Toolbar height scaled per active style (1.40× for Meridien Bloomberg-style tall bar) (#4).
    let tb_scale = style_current().toolbar_height_scale;
    // Shell region: floats as a rounded card with `region_gap` margin when the
    // active style is tiled (Aperture/Glass); flush flat fill otherwise.
    let rgap = crate::chart_renderer::ui::style::region_gap();
    let tb_frame = crate::chart_renderer::ui::style::region_frame(t, t.toolbar_bg)
        .inner_margin(egui::Margin { left: (gap_xs() + rgap) as i8, right: rgap as i8, top: 0, bottom: 0 });
    // The dropdown menu buttons (workspace/indicators/widgets) are ~36.6px tall and
    // do NOT scale with tb_scale, so any toolbar shorter than ~38px clips them at the
    // bottom (compact mode used 30px; tb_scale<1 also shrank it below the buttons).
    // Floor the effective height at 38px — the known clip-free value.
    let base_h = (38.0 * tb_scale).max(38.0);
    egui::TopBottomPanel::top("tb")
        .frame(tb_frame)
        // Add 2×gap to the reserved height so the card itself keeps `base_h`
        // after the outer margin insets it top + bottom.
        .exact_height(base_h + 2.0 * rgap)
        .show(ctx, |ui| {
        let tb_rect = ui.max_rect();
        // Publish toolbar rect so tb_btn can read it for full-height hover/active column overlays.
        set_toolbar_rect(tb_rect);
        // Gradient + bevel for Alto/Mariner/Cadence (Raised bevel):
        // 1. Vertical white-highlight gradient (top→transparent) — matches React's
        //    linear-gradient(bg-elevated, bg-panel) without hardcoding palette colors.
        // 2. Crisp 1px bevel lines on top/bottom edges.
        // Both are no-ops when the active style has no bevel (None).
        paint_gradient_highlight(ui.painter(), tb_rect, current_style_bevel_hi());
        paint_bevel(ui.painter(), tb_rect, egui::CornerRadius::ZERO);
        crate::design_tokens::register_hit(
            [tb_rect.min.x, tb_rect.min.y, tb_rect.width(), tb_rect.height()],
            "TOOLBAR", "Toolbar");

        // Window drag handle — spans the full toolbar. Uses Sense::drag only,
        // so later-drawn buttons (which sense click) get priority for clicks.
        // Double-click toggles maximize.
        let drag_resp = ui.interact(tb_rect, egui::Id::new("tb_window_drag"), egui::Sense::click_and_drag());
        if drag_resp.drag_started() {
            let win_ref: Option<Arc<Window>> = CURRENT_WINDOW.with(|w| w.borrow().clone());
            if let Some(w) = &win_ref {
                // A *borderless* window (we draw our own chrome) that is currently
                // maximized cannot be dragged across monitors via the OS move-loop:
                // Windows can't auto-restore it without a real caption, so it jumps
                // around, resizes, and the drag aborts. Fix: restore it ourselves and
                // place it under the cursor first, *then* begin the native drag — same
                // behaviour as a normal title bar.
                if w.is_maximized() {
                    let scale = w.scale_factor();
                    // Pointer position (logical, window-relative) → physical fraction
                    // across the current (maximized) width so the cursor stays over
                    // the same spot of the title bar after we shrink the window.
                    let ptr = ui.ctx().pointer_interact_pos();
                    let max_w = w.inner_size().width.max(1) as f64;
                    let frac_x = ptr
                        .map(|p| ((p.x as f64 * scale) / max_w).clamp(0.05, 0.95))
                        .unwrap_or(0.5);
                    // Global cursor position (physical) = window origin + local pointer.
                    let cursor_phys = w.outer_position().ok().zip(ptr).map(|(o, p)| {
                        (o.x as f64 + p.x as f64 * scale, o.y as f64 + p.y as f64 * scale)
                    });
                    // winit's set_maximized → SW_RESTORE is synchronous on Windows, so
                    // inner_size() below reflects the restored size.
                    w.set_maximized(false);
                    if let Some((cx, cy)) = cursor_phys {
                        let restored_w = w.inner_size().width.max(1) as f64;
                        let new_x = cx - frac_x * restored_w;
                        let new_y = cy - 12.0 * scale; // keep cursor ~12px into the bar
                        let _ = w.set_outer_position(
                            winit::dpi::PhysicalPosition::new(new_x, new_y),
                        );
                    }
                }
                let _ = w.drag_window();
            }
        }
        if drag_resp.double_clicked() {
            let win_ref: Option<Arc<Window>> = CURRENT_WINDOW.with(|w| w.borrow().clone());
            if let Some(w) = &win_ref { let m = w.is_maximized(); w.set_maximized(!m); }
        }
        // Bottom border line — ONLY for flush styles. When `region_gap > 0` the
        // toolbar is a floating card and `style::region_frame` already strokes
        // the whole card, bottom edge included; painting this line too put two
        // strokes on the same pixel row. `region_frame` does NOT stroke when
        // `region_gap <= 0` (it degrades to `Frame::NONE.fill(..)`), so flush
        // styles still need this manual line to separate nav from the chart.
        if rgap <= 0.0 {
            ui.painter().line_segment(
                [egui::pos2(tb_rect.left(), tb_rect.bottom()), egui::pos2(tb_rect.right(), tb_rect.bottom())],
                egui::Stroke::new(stroke_std(), t.toolbar_border),
            );
        }

        // Paper-mode bottom line removed — the $ badge in the toolbar (below)
        // is now the canonical "live vs paper" affordance.

        ui.horizontal_centered(|ui| {
            ui.spacing_mut().item_spacing.x = gap_xs();

            // ── Logo (with left edge margin so the glyph doesn't kiss the
            //         window border) ──
            ui.add_space(gap_sm());
            let (logo_rect, _) = ui.allocate_exact_size(egui::vec2(18.0, 18.0), egui::Sense::hover());
            draw_xolio_logo(ui.painter_at(logo_rect), logo_rect, t.accent);

            ui.add_space(gap_sm());
            ui.spacing_mut().item_spacing.x = gap_xs();

            // ── Account button (broker + connection state) ──
            // #7: When vertical_group_dividers active (Meridien), paint a full-column
            //     hover fill spanning the entire toolbar height before the button widget.
            {
                let connected = account_data_cached.as_ref().map_or(false, |(s,_,_)| s.connected);
                // PERF: hoist the connected/disconnected variants to &'static strs so
                // we skip a per-frame `format!` heap allocation in the account button.
                let acct_label_owned: &'static str = if connected {
                    concat!("IBKR ", "\u{F1A5}")  // CIRCLE_FILL
                } else {
                    concat!("IBKR ", "\u{F198}")  // CIRCLE
                };
                let acct_active = watchlist.account_strip_open;
                let acct_resp = toolbar_btn(ui, &acct_label_owned, acct_active, t);
                Tooltip::new("Account Summary").show(ui, &acct_resp, t);
                if style_current().vertical_group_dividers && acct_resp.hovered() {
                    let col = tint(t, Tone::Border, 80);
                    let btn_rect = acct_resp.rect;
                    let col_rect = egui::Rect::from_min_max(
                        egui::pos2(btn_rect.left() - 2.0, tb_rect.top()),
                        egui::pos2(btn_rect.right() + 2.0, tb_rect.bottom()),
                    );
                    ui.painter().rect_filled(col_rect, egui::CornerRadius::ZERO, col);
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                if acct_resp.clicked() {
                    watchlist.update_sidebar_state(|s| s.account_strip_open = !s.account_strip_open);
                }
            }

            // ── Workspace rail expand/collapse toggle (left of the $ toggle) ──
            {
                let ws_expanded = watchlist.workspace.nav_expanded;
                let resp = KitButton::icon(Icon::SIDEBAR)
                    .variant(KitVariant::Ghost).size(KitSize::Sm)
                    .active(ws_expanded)
                    .fg(if ws_expanded { t.accent } else { t.text })
                    .glyph_size(font_lg())
                    .min_size(egui::vec2(28.0, row_height_default()))
                    .show(ui, t);
                Tooltip::new(if ws_expanded { "Collapse workspaces" } else { "Expand workspaces" })
                    .show(ui, &resp, t);
                if resp.clicked() {
                    watchlist.workspace.nav_expanded = !watchlist.workspace.nav_expanded;
                }
            }

            // ── Paper / Live — bigger badge $-button. ──
            // LIVE  → solid green fill + dark `$` glyph (very visible "this is real money")
            // PAPER → transparent fill + warn-colored `$` glyph (visible but neutral)
            {
                let paper = crate::chart_renderer::trading::order_manager::is_paper_mode();
                let live = !paper;
                let (fill, fg) = if live {
                    (t.bull, crate::chart_renderer::ui::style::contrast_fg(t.bull))
                } else {
                    (egui::Color32::TRANSPARENT, t.warn)
                };
                let tip = if live {
                    "LIVE — real-money trading active. Click to switch to Paper."
                } else {
                    "PAPER — practice mode (no real orders). Click to switch to Live."
                };
                let resp = KitButton::new("$").variant(KitVariant::Ghost).size(KitSize::Sm)
                    .fg(fg).fill(fill).min_size(egui::vec2(28.0, row_height_default()))
                    .show(ui, t);
                Tooltip::new(tip).show(ui, &resp, t);
                if resp.clicked() {
                    if let Err(reason) = crate::chart_renderer::trading::order_manager::set_paper_mode(!paper) {
                        crate::data::connectivity::errors_sink::report(
                            crate::data::connectivity::errors_sink::ErrorLevel::Warn,
                            "trading", "paper_mode_toggle_blocked", reason,
                        );
                    }
                }
            }


            // ── Market session chip (authoritative, from ApexData) ──────────
            // OPEN = green, pre/after/extended = amber, CLOSED = dim. Read-only.
            // Gates nothing by itself — the order ticket reads the same source.
            if let Some(ms) = crate::apex_data::live_state::market_status() {
                let label = ms.label();
                let fg = if ms.is_rth() {
                    t.bull
                } else if ms.is_tradeable() {
                    t.warn
                } else {
                    tint(t, Tone::Text, 130)
                };
                let resp = KitButton::new(label).variant(KitVariant::Ghost).size(KitSize::Sm)
                    .fg(fg).min_size(egui::vec2(58.0, row_height_default()))
                    .show(ui, t);
                let tip = if ms.server_time.is_empty() {
                    format!("Market: {}", ms.market)
                } else {
                    format!("Market: {}\nServer time: {}", ms.market, ms.server_time)
                };
                Tooltip::new(tip).show(ui, &resp, t);
            }

            crate::ui_kit::widgets::Separator::vertical().spacing(4.0).show(ui, t);

            // ── TPS Reports boss-key button (~70px) ────────────────────────
            // Replaces the ticker/symbol display. Clicking masks the entire app
            // with a fake Excel TPS-report spreadsheet (Cmd+Shift+H to dismiss).
            {
                let active = watchlist.boss_key_active;
                let resp = KitButton::new("TPS")
                    .variant(KitVariant::Ghost)
                    .size(KitSize::Sm)
                    .show(ui, t);
                Tooltip::new(
                    if active { "Show trading view (⌘⇧H)" } else { "Hide screen — TPS Reports (⌘⇧H)" }
                ).show(ui, &resp, t);
                if resp.clicked() {
                    watchlist.boss_key_active = !watchlist.boss_key_active;
                }
            }

            crate::ui_kit::widgets::Separator::vertical().spacing(4.0).show(ui, t);

            // ── Scrollable middle section ──
            // Calculate available width: total - logo(25) - symbol(~70) - right section(~350)
            let right_width = 130.0; // window controls + Opt button
            let middle_width = (ui.available_width() - right_width).max(60.0);
            egui::ScrollArea::horizontal().max_width(middle_width).show(ui, |ui| {
            // Density-first defaults for the entire scrollable middle nav:
            //   item_spacing.x = 0       → no auto gap between buttons; cluster
            //                              breaks come from explicit `add_space()`.
            //   button_padding = (12, 8) → gap_md horizontal, gap_sm vertical.
            //                              Comfortable click targets without
            //                              bloating the toolbar height.
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.spacing_mut().button_padding = egui::vec2(gap_md(), gap_sm());

            // ── Strip the egui-default button bg / border so menu_button and
            //    plain Button widgets paint transparently. The visible
            //    hover/active fill comes from the unified `ui_kit` Button's
            //    Ghost treatment. Without this, the dropdowns paint *both* the
            //    egui-default chrome and the Ghost fill.
            {
                let v = &mut ui.style_mut().visuals.widgets;
                v.inactive.bg_fill        = egui::Color32::TRANSPARENT;
                v.inactive.weak_bg_fill   = egui::Color32::TRANSPARENT;
                v.inactive.bg_stroke      = egui::Stroke::NONE;
                v.hovered.bg_fill         = egui::Color32::TRANSPARENT;
                v.hovered.weak_bg_fill    = egui::Color32::TRANSPARENT;
                v.hovered.bg_stroke       = egui::Stroke::NONE;
                v.active.bg_fill          = egui::Color32::TRANSPARENT;
                v.active.weak_bg_fill     = egui::Color32::TRANSPARENT;
                v.active.bg_stroke        = egui::Stroke::NONE;
                v.open.bg_fill            = egui::Color32::TRANSPARENT;
                v.open.weak_bg_fill       = egui::Color32::TRANSPARENT;
                v.open.bg_stroke          = egui::Stroke::NONE;
            }

            // (Chart controls live in the toolnav row — no fallback when off.)

            // (Hit-highlight toggle moved to the per-pane top-left strip.)

            crate::ui_kit::widgets::Separator::vertical().spacing(4.0).show(ui, t);

            // ── Workspace — icon-only dropdown (active workspace shown inside the menu) ──
            {
                let ws_names = list_workspaces();
                let ws_menu = KitButton::menu(Icon::BROWSERS)
                    .glyph_size(font_lg())
                    .show_menu(ui, t, |ui| {
                    apply_menu_style(ui, t);
                    ui.set_min_width(200.0);

                    ui.label(egui::RichText::new("WORKSPACES").monospace().size(font_xs()).color(color_half(t.dim)));
                    ui.add_space(gap_sm());

                    // Workspace list
                    for name in &ws_names {
                        let is_active = *name == watchlist.workspace.active;
                        ui.horizontal(|ui| {
                            // Active dot
                            if is_active {
                                ui.label(TextStyle::Caption.as_rich_cascading(Icon::CIRCLE_FILL, t.accent));
                            } else {
                                ui.label(egui::RichText::new("  ").size(font_xs()));
                            }
                            if ui.add(SelectableRow::new(name, is_active)).clicked() && !is_active {
                                watchlist.workspace.active = name.clone();
                                watchlist.workspace.pending_load = Some(name.clone());
                                ui.close_menu();
                            }
                        });
                    }

                    ui.add_space(gap_sm());
                    ui.separator();
                    ui.add_space(gap_sm());

                    // Save current
                    if !watchlist.workspace.active.is_empty() {
                        let save_resp = ui.button(egui::RichText::new(format!("{} Save \"{}\"", Icon::CHECK, watchlist.workspace.active))
                            .monospace().size(font_sm()).color(t.accent));
                        #[cfg(debug_assertions)]
                        crate::dev_inspector::record(
                            crate::dev_inspector::WidgetRecord::from_response(
                                "toolbar.save_workspace", "button", "Save workspace", &save_resp, ui,
                            )
                        );
                        if save_resp.clicked() {
                            let ws_name = watchlist.workspace.active.clone();
                            save_workspace(&ws_name, panes, *layout, watchlist);
                            ui.close_menu();
                        }
                    }

                    // Save as new
                    ui.add_space(gap_sm());
                    ui.horizontal(|ui| {
                        crate::ui_kit::widgets::Input::new(&mut watchlist.workspace.save_name)
                            .placeholder("New workspace…")
                            .min_width(130.0)
                            .size(crate::ui_kit::widgets::Size::Sm)
                            .show(ui, t);
                        let can_save = !watchlist.workspace.save_name.trim().is_empty();
                        if can_save {
                            if KitButton::new("Save As").variant(KitVariant::Primary).size(KitSize::Sm)
                                .tint(t.accent).show(ui, t).clicked() {
                                let name = watchlist.workspace.save_name.trim().to_string();
                                save_workspace(&name, panes, *layout, watchlist);
                                watchlist.workspace.active = name;
                                watchlist.workspace.save_name.clear();
                                ui.close_menu();
                            }
                        }
                    });

                    // Auto-save info
                    ui.add_space(gap_sm());
                    ui.label(egui::RichText::new("Auto-saves every 30s").monospace().size(font_xs()).color(color_very_dim(t.dim)));
                });
                {
                    use crate::ui_kit::widgets::Tooltip;
                    let active_ws = watchlist.workspace.active.clone();
                    Tooltip::rich(move |ui, theme| {
                        ui.label(TextStyle::BodySm.as_rich_cascading("Workspaces", theme.text()).strong());
                        ui.label(TextStyle::Caption.as_rich_cascading(&format!("Active: {}", active_ws), theme.dim()));
                    }).show(ui, &ws_menu.response, t);
                }
                #[cfg(debug_assertions)]
                {
                    let ws_name = watchlist.workspace.active.clone();
                    crate::dev_inspector::record(
                        crate::dev_inspector::WidgetRecord::from_response(
                            "toolbar.workspace_btn", "button", &ws_name, &ws_menu.response, ui,
                        ).with_style("toolbar"),
                    );
                }
            }

            crate::ui_kit::widgets::Separator::vertical().spacing(4.0).show(ui, t);

            // ── Layouts — favorites bar + dropdown ──
            // Helper: switch to a layout, creating panes as needed
            let switch_layout = |ly: Layout, panes: &mut Vec<Chart>, layout: &mut Layout, active_pane: &mut usize, watchlist: &mut Watchlist| {
                if *layout == ly { return; }
                let max = ly.max_panes();
                while panes.len() < max {
                    let syms = ["SPY","AAPL","MSFT","NVDA","TSLA","AMZN","META","GOOG","AMD"];
                    let sym = syms.get(panes.len()).unwrap_or(&"SPY");
                    let mut p = Chart::new_with(sym, &panes[0].timeframe);
                    p.theme_idx = panes[0].theme_idx;
                    p.recent_symbols = panes[0].recent_symbols.clone();
                    p.pending_symbol_change = Some(sym.to_string());
                    panes.push(p);
                }
                *layout = ly;
                if *active_pane >= max { *active_pane = 0; }
                // Phase 1: regenerate PaneLayout to match the freshly-picked
                // template. The 19 named templates are now "preset trees" —
                // selecting one resets the tree; subsequent splits mutate
                // the tree from that preset baseline.
                // P16 fix #3 — queue orphan panes (indices >= template_max)
                // for removal via the existing PENDING_PANE_CLOSE drain at
                // end-of-frame. Doing the panes.truncate() inline would
                // invalidate the `ap = *active_pane` snapshot taken at
                // top_nav fn entry (used later in this same frame for
                // panes[ap] lookups). Tree is regenerated NOW so the next
                // frame's renders use the new template.
                // P17 #3 — snapshot before template change (destructive).
                crate::chart_renderer::gpu::pane_layout_record_undo(watchlist);
                let template_max = ly.max_panes();
                if panes.len() > template_max {
                    crate::chart_renderer::gpu::PENDING_PANE_CLOSE.with(|q| {
                        let mut closes = q.borrow_mut();
                        for idx in template_max..panes.len() {
                            closes.push(idx);
                        }
                    });
                    watchlist.maximized_pane = watchlist.maximized_pane.filter(|&m| m < template_max);
                }
                // P17 #1 — sync ids then use them as PaneSlot values so the
                // tree carries stable IDs instead of fragile indices.
                crate::chart_renderer::gpu::ensure_pane_ids_synced(watchlist, panes.len());
                let mut slot_ids: Vec<u64> = watchlist.pane_ids.iter().copied()
                    .take(template_max.max(1)).collect();
                while slot_ids.len() < template_max.max(1) { slot_ids.push(0); }
                watchlist.pane_layout = Some(
                    crate::chart_renderer::pane_layout::PaneLayout::from_template(ly, &slot_ids)
                );
            };
            // Show favorited layouts as segmented control + dropdown caret
            {
                let mut fav_layouts: Vec<&Layout> = ALL_LAYOUTS.iter()
                    .filter(|&&ly| watchlist.layout_favorites.iter().any(|f| f == ly.label()))
                    .collect();
                // If the active layout is not a favorite, temporarily insert it so it shows as selected
                let current_is_fav = fav_layouts.iter().any(|&&ly| ly == *layout);
                let transient_layout_ref: Option<&Layout> = if !current_is_fav {
                    ALL_LAYOUTS.iter().find(|&&ly| ly == *layout)
                } else { None };
                if let Some(transient) = transient_layout_ref {
                    fav_layouts.push(transient);
                }
                if !fav_layouts.is_empty() {
                    ui.add_space(gap_xs());
                    let labels: Vec<&str> = fav_layouts.iter().map(|&&ly| ly.label()).collect();
                    let active_idx = fav_layouts.iter().position(|&&ly| *layout == ly).unwrap_or(0);
                    if let Some(i) = segmented_control(ui, active_idx, &labels, t.toolbar_bg, t.toolbar_border, t.accent, t.dim) {
                        switch_layout(*fav_layouts[i], panes, layout, active_pane, watchlist);
                    }
                    ui.add_space(gap_xs());
                }
                // P17 #4 — sync indicator. When PaneLayout diverges from the
                // selected template (user split/closed since picking it), show
                // "Custom" so the dropdown label stops lying about reality.
                // Comparison is structural: we build a reference tree using the
                // SAME ids the live tree currently has (otherwise PartialEq
                // would always fail due to mismatched stable IDs).
                let pane_layout_matches_template = match &watchlist.pane_layout {
                    None => true, // legacy path always matches
                    Some(pl) => {
                        let template_max = layout.max_panes();
                        let mut slot_ids: Vec<u64> = watchlist.pane_ids.iter().copied()
                            .take(template_max.max(1)).collect();
                        while slot_ids.len() < template_max.max(1) { slot_ids.push(0); }
                        let reference = crate::chart_renderer::pane_layout::PaneLayout::from_template(*layout, &slot_ids);
                        *pl == reference
                    }
                };
                if !pane_layout_matches_template {
                    ui.add_space(gap_xs());
                    ui.label(egui::RichText::new("Custom")
                        .monospace().size(font_xs()).color(tint(t, Tone::Accent, alpha_strong())));
                }
                // Dropdown caret for the full layout picker
                let dd_btn = toolbar_btn(ui, Icon::CARET_DOWN, watchlist.layout_dropdown_open, t);
                #[cfg(debug_assertions)]
                crate::dev_inspector::record(
                    crate::dev_inspector::WidgetRecord::from_response("toolbar.layout_picker", "button", "Layout picker", &dd_btn, ui)
                        .with_style("toolbar")
                );
                Tooltip::new(if pane_layout_matches_template { "Layout picker" } else { "Layout picker (custom — pick to reset)" })
                    .show(ui, &dd_btn, t);
                if dd_btn.clicked() {
                    watchlist.layout_dropdown_open = !watchlist.layout_dropdown_open;
                    watchlist.layout_dropdown_pos = egui::pos2(dd_btn.rect.left(), dd_btn.rect.bottom() + 2.0);
                }
            }
            // (Layout dropdown rendered after toolbar — see below)

            // Theme + Style picker moved to Settings → Appearance.

            }); // end scrollable middle

            // ── Fixed right: panels + window controls ──
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 0.0;

                // Window controls (Close / Maximize / Minimize) — delegated.
                super::window_controls::render_window_controls(ui, panes, *layout, watchlist, &win_ref, t);

                // Separator between window controls and panel toggles
                crate::ui_kit::widgets::Separator::vertical().spacing(4.0).show(ui, t);

                // Panel toggle buttons (right-to-left, so ordered right→left)
                ui.spacing_mut().item_spacing.x = gap_sm();

                // Button-group enclosure — shared by the actions + sidebar-toggle groups.
                use crate::chart_renderer::ui::style::{button_group_enclosed, ButtonGroupBox};
                let bg_enclosed = button_group_enclosed();

                // Connection status — apex_data feed dot mapped from ConnectionState.
                // Green = Subscribed, Amber = Connecting/Authenticated, Red = Backoff/Failed/Idle.
                {
                    use crate::chart_renderer::ui::panels::connection_state_snapshot;
                    use crate::data::connectivity::ConnectionState;
                    let apex_state = connection_state_snapshot::get("apex_data");
                    let (dot_color, tip_label, tip_detail) = match &apex_state {
                        ConnectionState::Subscribed { count } => (
                            t.bull,
                            "apex-data: connected",
                            format!("apex-data: connected ({count} subscriptions)"),
                        ),
                        ConnectionState::Authenticated => (
                            t.warn,
                            "apex-data: authenticated",
                            "apex-data: authenticated (awaiting subscriptions)".to_string(),
                        ),
                        ConnectionState::Connecting { attempt } => (
                            t.warn,
                            "apex-data: connecting",
                            format!("apex-data: connecting (attempt {attempt})"),
                        ),
                        ConnectionState::Backoff { attempt, reason, .. } => (
                            t.bear,
                            "apex-data: reconnecting",
                            format!("apex-data: backoff before attempt {attempt} — {reason}"),
                        ),
                        ConnectionState::Failed { reason } => (
                            t.bear,
                            "apex-data: failed",
                            format!("apex-data: failed — {reason}"),
                        ),
                        ConnectionState::ShuttingDown => (
                            t.bear,
                            "apex-data: shutting down",
                            "apex-data: shutting down".to_string(),
                        ),
                        ConnectionState::Idle => (
                            t.bear,
                            "apex-data: idle",
                            "apex-data: idle (not started)".to_string(),
                        ),
                    };
                    let _ = tip_label; // used in tooltip below
                    let (dot_rect, resp) = ui.allocate_exact_size(egui::vec2(20.0, 20.0), egui::Sense::click());
                    ui.painter().circle_filled(dot_rect.center(), 3.0, dot_color);
                    crate::chart_renderer::ui::style::cursor::clickable(ui, &resp);
                    crate::chart_renderer::ui::style::cursor::focus_ring(ui, &resp, t.accent);
                    let tip_detail_clone = tip_detail.clone();
                    Tooltip::rich(move |ui, theme| {
                        ui.label(TextStyle::Caption.as_rich_cascading(&tip_detail_clone, theme.text()));
                        ui.label(TextStyle::Caption.as_rich_cascading("Click to open Connection panel", theme.dim()));
                    }).show(ui, &resp, t);
                    if resp.clicked() { *conn_panel_open = !*conn_panel_open; }
                    #[cfg(debug_assertions)]
                    crate::dev_inspector::record(
                        crate::dev_inspector::WidgetRecord::from_response(
                            "toolbar.connection_btn", "button", "", &resp, ui,
                        ).with_style("toolbar"),
                    );
                }

                // W1-02/W1-02b (audit): "drawings not saving" chip.
                //
                // Postgres going down used to be a bare eprintln — the trader
                // kept drawing, everything looked normal, and the work was gone
                // at restart. Drawings are now buffered and replayed on
                // reconnect, but the trader still has to KNOW the chart isn't
                // durable yet, so this chip is visible for the whole outage and
                // clears itself the moment the pool attaches.
                //
                // Shown only on the failure path: zero cost and zero clutter in
                // the normal case, which is what keeps it credible when it does
                // appear.
                if !crate::drawing_db::is_persisting() {
                    let chip = ui.label(
                        TextStyle::Caption.as_rich_cascading(&format!("{} drawings not saving", Icon::WARNING), t.warn),
                    );
                    Tooltip::rich(move |ui, theme| {
                        ui.label(TextStyle::Caption.as_rich_cascading("Postgres is unavailable", theme.text()));
                        ui.label(TextStyle::Caption.as_rich_cascading(
                            "Drawings are being buffered to disk and will be saved \
                             automatically when the database returns.", theme.dim()));
                    }).show(ui, &chip, t);
                    #[cfg(debug_assertions)]
                    crate::dev_inspector::record(
                        crate::dev_inspector::WidgetRecord::from_response(
                            "toolbar.drawings_not_saving", "label", "⚠ drawings not saving", &chip, ui,
                        ).with_style("toolbar"),
                    );
                }

                // Style-aware label helper for nav buttons that have a text label.
                // Meridien hides icons and uppercases labels; other styles keep "{ICON} Label".
                // Icon-only buttons (Settings, etc.) are NOT affected — they keep their icon
                // glyph under all styles.
                let st = style_current();
                let nav_label = |icon: &str, label: &str| -> String {
                    let mut txt = if st.nav_buttons_uppercase_labels { label.to_uppercase() } else { label.to_string() };
                    // Apply nav letter-spacing via Unicode thin-spaces (same egui
                    // approximation as `style_label_case`): no CSS letter-spacing
                    // in egui, so we insert U+2009 between glyphs. >1.5px = double.
                    let sp = st.nav_letter_spacing_px;
                    if sp >= 0.5 {
                        let sep = if sp > 1.5 { "\u{2009}\u{2009}" } else { "\u{2009}" };
                        txt = txt.chars().map(|c| c.to_string()).collect::<Vec<_>>().join(sep);
                    }
                    if st.nav_buttons_label_only { txt } else { format!("{} {}", icon, txt) }
                };

                // ── Actions group box (Settings / Search / ORDER / Toolbar toggle) ──
                let actions_box = ButtonGroupBox::begin(ui);
                let mut actions_rect: Option<egui::Rect> = None;

                // Settings — always icon-only. UX-1 Fix 2: updated hover text
                // with Cmd+, shortcut hint; Cmd+, also toggles settings.
                {
                    // Cmd+, opens settings (register once).
                    {
                        use std::sync::Once;
                        static SETTINGS_SC: Once = Once::new();
                        SETTINGS_SC.call_once(|| {
                            use crate::foundation::shortcuts::{ShortcutEntry, Shortcut};
                            if let Err(e) = crate::foundation::shortcuts::registry()
                                .write()
                                .unwrap()
                                .register(ShortcutEntry {
                                    shortcut: Shortcut {
                                        modifiers: egui::Modifiers::COMMAND,
                                        key: egui::Key::Comma,
                                    },
                                    action: "panel.settings_toggle",
                                    description: "Open settings",
                                    category: "Panels",
                                })
                            {
                                eprintln!("[shortcuts] {}", e);
                            }
                        });
                    }
                    let cmd_comma = ctx.input(|i| {
                        i.key_pressed(egui::Key::Comma) && i.modifiers.command
                    });
                    if cmd_comma { watchlist.update_sidebar_state(|s| s.settings_open = !s.settings_open); }

                    let settings_resp = toolbar_btn(ui, Icon::GEAR, watchlist.settings_open, t);
                    #[cfg(debug_assertions)]
                    crate::dev_inspector::record(
                        crate::dev_inspector::WidgetRecord::from_response("toolbar.settings_btn", "button", "Settings", &settings_resp, ui)
                            .with_style("toolbar")
                    );
                    Tooltip::new("Settings (Cmd+,)").show(ui, &settings_resp, t);
                    actions_rect = Some(settings_resp.rect);
                    if settings_resp.clicked() { watchlist.update_sidebar_state(|s| s.settings_open = !s.settings_open); }
                }

                // Search / command palette — icon-only ToolbarBtn.
                {
                    use crate::ui_kit::widgets::{Tooltip, Kbd};
                    let search_resp = toolbar_btn(ui, Icon::MAGNIFYING_GLASS, watchlist.cmd_palette.open, t);
                    #[cfg(debug_assertions)]
                    crate::dev_inspector::record(
                        crate::dev_inspector::WidgetRecord::from_response("toolbar.search_btn", "button", "Search", &search_resp, ui)
                            .with_style("toolbar")
                    );
                    Tooltip::rich(|ui, theme| {
                        ui.label(TextStyle::BodySm.as_rich_cascading("Search", theme.text()).strong());
                        ui.label(TextStyle::Caption.as_rich_cascading("Search & command palette", theme.dim()));
                        ui.add(Kbd::new("Cmd+K"));
                    }).show(ui, &search_resp, t);
                    if search_resp.clicked() {
                        watchlist.cmd_palette.open = !watchlist.cmd_palette.open;
                    }
                }

                // ORDER — prominent CTA; spawns a floating ticket for the
                // active pane. Sits left of Search in the right cluster.
                {
                    let order = crate::ui_kit::widgets::Button::new("ORDER")
                        .fill(t.accent)
                        .show(ui, t);
                    crate::ui_kit::widgets::Tooltip::new("New order ticket (active chart)")
                        .show(ui, &order, t);
                    if order.clicked() { order_clicked = true; }
                }

                // Toolbar (toolnav row) on/off — icon-only toggle, immediately
                // left of Search. Default ON for every style; this overrides it.
                {
                    let tn_on = crate::chart_renderer::ui::style::toolnav_visible();
                    let resp = toolbar_btn(ui, Icon::SLIDERS, tn_on, t); // toolbar-row toggle (was BROWSERS, collided with Workspace menu)
                    #[cfg(debug_assertions)]
                    crate::dev_inspector::record(
                        crate::dev_inspector::WidgetRecord::from_response("toolbar.toolnav_toggle", "button", "Toolbar toggle", &resp, ui)
                            .with_style("toolbar")
                    );
                    crate::ui_kit::widgets::Tooltip::new("Toolbar").show(ui, &resp, t);
                    if resp.clicked() {
                        crate::chart_renderer::ui::style::set_toolnav_override(Some(!tn_on));
                    }
                    actions_rect = Some(actions_rect.map_or(resp.rect, |r: egui::Rect| r.union(resp.rect)));
                }

                // Close the actions group box.
                if let Some(rect) = actions_rect {
                    actions_box.end(ui, t, rect, tb_rect);
                }

                crate::ui_kit::widgets::Separator::vertical().spacing(4.0).show(ui, t);

                // ── Right nav panel toggles — zero item spacing, second-smallest
                //    inner button padding, hairline dividers between each. ──
                let prev_spacing = ui.spacing().item_spacing.x;
                let prev_panel_pad = ui.spacing().button_padding;
                // 8px gap between buttons (4px on each side of the divider).
                ui.spacing_mut().item_spacing.x = gap_sm();
                // gap_lg horizontal padding so labels breathe and the hover
                // column has visible margin on either side of the text.
                ui.spacing_mut().button_padding = egui::vec2(gap_lg(), gap_sm());

                // Sidebar-toggle group box (Feed → Signals). `sidebar_box` reserves
                // the background slot; `sidebar_rect` accumulates the button bounds.
                let sidebar_box = ButtonGroupBox::begin(ui);
                let mut sidebar_rect: Option<egui::Rect> = None;

                // Divider drawn at the left edge of the just-drawn button's rect (RTL layout).
                //
                // Paints on the Foreground layer (NOT `$ui.painter()`) because with
                // `item_spacing.x = 0` the neighbouring button's rect ends at this
                // same x coordinate — its bg / column tint would otherwise overdraw
                // a sub-pixel hairline. Foreground layer + stroke_std + pixel-snapped
                // coordinate guarantee it actually reads.
                // Center the divider in the 8px gap between buttons (btn.left - 4)
                // and span the full toolbar height (top→bottom). Uses `t.dim`
                // so it actually reads — `t.toolbar_border` is intentionally
                // hairline-faint per the theme palette.
                macro_rules! nav_divider {
                    ($ui:expr, $resp:expr) => {{
                        // Accumulate the group's bounding rect for the enclosure box.
                        sidebar_rect = Some(sidebar_rect.map_or($resp.rect, |r: egui::Rect| r.union($resp.rect)));
                        // Enclosed styles draw the box instead of per-button dividers.
                        if !bg_enclosed {
                            let x = ($resp.rect.left() - 4.0).round() + 0.5;
                            let col = tint(t, Tone::Dim, alpha_dim());
                            let painter = $ui.ctx().layer_painter(egui::LayerId::new(
                                egui::Order::Foreground,
                                egui::Id::new(("nav_divider", x.to_bits())),
                            ));
                            painter.line_segment(
                                [egui::pos2(x, tb_rect.top()), egui::pos2(x, tb_rect.bottom())],
                                egui::Stroke::new(stroke_std(), col),
                            );
                        }
                    }};
                }

                // ── Panel toggle helper macro ─────────────────────────────────
                // Renders a toolbar button + tooltip + click handler + divider for
                // panels that follow the standard sidebar-toggle pattern. The
                // button's own Ghost hover/active fill is the only highlight (the
                // old bespoke column tint is gone — see the note at the top of
                // this file).
                macro_rules! panel_toggle {
                    ($icon:expr, $lbl:expr, $field:ident, $tip:expr) => {{
                        let resp = toolbar_btn(ui, &nav_label($icon, $lbl), watchlist.$field, t);
                        Tooltip::new($tip).show(ui, &resp, t);
                        if resp.clicked() { watchlist.update_sidebar_state(|s| s.$field = !s.$field); }
                        nav_divider!(ui, resp);
                    }};
                    // WS-E E3: variant for fields moved into a Watchlist sub-struct — the
                    // display value ($wl expr, e.g. `watchlist.analysis.open`) differs from
                    // the still-flat SidebarState field ($sfield ident, e.g. `analysis_open`).
                    ($icon:expr, $lbl:expr, $wl:expr, $sfield:ident, $tip:expr) => {{
                        let resp = toolbar_btn(ui, &nav_label($icon, $lbl), $wl, t);
                        Tooltip::new($tip).show(ui, &resp, t);
                        if resp.clicked() { watchlist.update_sidebar_state(|s| s.$sfield = !s.$sfield); }
                        nav_divider!(ui, resp);
                    }};
                }

                // ── Screener toggle (Wave S2) — Ctrl+Shift+S ─────────────────
                {
                    let screener_open = watchlist.sidebar_state_snapshot().screener_panel_open;
                    let resp = toolbar_btn(ui, &nav_label(Icon::FUNNEL, "Screener"), screener_open, t); // FUNNEL = filter/screen (was MAGNIFYING_GLASS, collided with Search)
                    crate::ui_kit::widgets::Tooltip::new("Screener (Ctrl+Shift+S)").show(ui, &resp, t);
                    if resp.clicked() {
                        crate::chart_renderer::commands::push(
                            crate::chart_renderer::commands::AppCommand::OpenScreenerPanel { open: !screener_open }
                        );
                    }
                    nav_divider!(ui, resp);
                }

                panel_toggle!(Icon::NEWSPAPER,      "Feed",       watchlist.feed_panel.open, feed_panel_open, "Feed (News, Discord, Screenshots)");
                panel_toggle!(Icon::STAR,            "Playbook",   playbook_panel_open,   "Playbook (Trade Ideas)");

                // Chart Library uses direct assignment (not update_sidebar_state)
                {
                    let resp = toolbar_btn(ui, &nav_label(Icon::FOLDER, "Charts"), watchlist.charts_library_open, t);
                    Tooltip::new("Chart Library (saved layouts)").show(ui, &resp, t);
                    if resp.clicked() { watchlist.charts_library_open = !watchlist.charts_library_open; }
                    nav_divider!(ui, resp);
                }

                // (Toolbar toggle moved next to Search — see below.)

                // Watchlist: display field (`open`) differs from state field (`watchlist_open`)
                {
                    let resp = toolbar_btn(ui, &nav_label(Icon::LIST, "Watchlist"), watchlist.open, t);
                    #[cfg(debug_assertions)]
                    crate::dev_inspector::record(
                        crate::dev_inspector::WidgetRecord::from_response("toolbar.watchlist_toggle", "button", "Watchlist", &resp, ui)
                            .with_style("toolbar")
                    );
                    Tooltip::new("Watchlist").show(ui, &resp, t);
                    if resp.clicked() { watchlist.update_sidebar_state(|s| s.watchlist_open = !s.watchlist_open); }
                    nav_divider!(ui, resp);
                }
                panel_toggle!(Icon::CURRENCY_DOLLAR, "Orders",     orders_panel_open,     "Orders Panel");
                panel_toggle!(Icon::CHART_LINE,      "Analysis",   watchlist.analysis.open, analysis_open, "Analysis Sidebar");
                panel_toggle!(Icon::SPARKLE,         "Auto-Chart", auto_chart_open,       "Auto-Charting (lines, levels, patterns, tuning)"); // SPARKLE = auto/AI (was CHART_LINE, collided with Analysis)
                panel_toggle!(Icon::PULSE,           "Indicators", watchlist.indicators.panel_open, indicators_panel_open, "Indicators (Active + Library + Tools)");

                // AUDIT 2026-08-02 (AT-088 / AT-091): four rail panels were
                // registered in `right_rail::PANELS` and fully implemented, but
                // their `is_open` predicates had NO writer that could ever
                // return true. Each flag's only non-test writer set it to
                // `false` (the panel's own close-X), and each defaults to
                // false — so the panels were unreachable for the whole life of
                // the app. The state plumbing was already complete: every flag
                // exists in SidebarState and round-trips through
                // push_to_sidebar_store / sync_from_sidebar_store. Only the
                // toolbar button was never added.
                //
                // These go through `update_sidebar_state` like every sibling —
                // a direct `watchlist.tape.open = true` would be stomped by the
                // next store sync (see the warning at commands.rs:1232).
                panel_toggle!(Icon::CLOCK,         "T&S",     watchlist.tape.open,          tape_open,          "Time & Sales tape");
                panel_toggle!(Icon::NOTEBOOK,      "Journal", watchlist.journal_panel.open, journal_panel_open, "Trade Journal");
                panel_toggle!(Icon::CIRCLES_FOUR,  "RRG",     watchlist.rrg.open,           rrg_open,           "Relative Rotation Graph");
                panel_toggle!(Icon::CODE,          "Script",  watchlist.script.open,        script_open,        "Script editor");

                // Signals panel (Alerts + Signals) — no divider after, it's the last in the group
                {
                    let active_count = watchlist.alerts.iter().filter(|a| !a.triggered).count()
                        + panes.iter().flat_map(|p| p.price_alerts.iter()).filter(|a| !a.triggered && !a.draft).count();
                    let signals_resp = toolbar_btn(ui, &nav_label(Icon::LIGHTNING, "Signals"), watchlist.signals_panel.open, t);
                    Tooltip::new("Signals (Alerts + Signals)").show(ui, &signals_resp, t);
                    sidebar_rect = Some(sidebar_rect.map_or(signals_resp.rect, |r: egui::Rect| r.union(signals_resp.rect)));
                    if active_count > 0 {
                        // Overlay a Badge at the top-right corner of the Signals button.
                        // Painter-mode positioning: anchor the badge so its center sits at
                        // (rect.right() - 3, rect.top() + 5), matching the previous manual
                        // circle/text overlay. Use `new_child(max_rect=...)` to host the
                        // flow-layout Badge widget at that absolute position.
                        use crate::ui_kit::widgets::{Badge, TagTone};
                        let anchor = egui::pos2(
                            signals_resp.rect.right() - 3.0,
                            signals_resp.rect.top() + 5.0,
                        );
                        // Min badge size is 14x14; for 2+ digit counts it grows wider.
                        // Center the badge on `anchor` by giving the child UI a max_rect
                        // that starts at anchor - half_size. We approximate width by
                        // digit count (each digit ≈ 6px @ 10px monospace + 10px padding).
                        let digits = active_count.to_string().len() as f32;
                        let badge_w = (digits * 6.0 + 10.0).max(14.0);
                        let badge_h = 14.0_f32;
                        let badge_rect = egui::Rect::from_center_size(
                            anchor,
                            egui::vec2(badge_w, badge_h),
                        );
                        let mut child = ui.new_child(
                            egui::UiBuilder::new()
                                .max_rect(egui::Rect::from_min_size(
                                    badge_rect.left_top(),
                                    egui::vec2(badge_w + 4.0, badge_h + 4.0),
                                ))
                                .layout(egui::Layout::left_to_right(egui::Align::Min))
                        );
                        Badge::count(active_count as u32).max(99).tone(TagTone::Accent).show(&mut child, t);
                    }
                    if signals_resp.clicked() { watchlist.update_sidebar_state(|s| s.signals_panel_open = !s.signals_panel_open); }
                }

                // Close the sidebar-toggle group box (Feed → Signals). No-op for flat styles.
                if let Some(rect) = sidebar_rect {
                    sidebar_box.end(ui, t, rect, tb_rect);
                }

                ui.spacing_mut().item_spacing.x = prev_spacing;
                ui.spacing_mut().button_padding = prev_panel_pad;

                // New window — single icon button.
                let r = toolbar_btn(ui, Icon::CIRCLES_THREE_PLUS, false, t);
                Tooltip::new("New chart window").show(ui, &r, t);
                if r.clicked() {
                    let (tx, rx) = std::sync::mpsc::channel();
                    let sym = panes[ap].symbol.clone();
                    let tf = panes[ap].timeframe.clone();
                    let initial = ChartCommand::LoadBars {
                        symbol: sym.clone(), timeframe: tf.clone(), bars: vec![], timestamps: vec![], gen: 0,
                    };
                    {
                        let global = crate::NATIVE_CHART_TXS.get_or_init(|| std::sync::Mutex::new(Vec::new()));
                        global.lock().unwrap().push(tx);
                    }
                    crate::chart_renderer::gpu::open_window(rx, initial);
                    crate::chart_renderer::gpu::fetch_bars_background(
                        panes[ap].symbol.clone(), panes[ap].timeframe.clone(), 0);
                }

                crate::ui_kit::widgets::Separator::vertical().spacing(4.0).show(ui, t);
            });

            // (Opt button is in scroll area, near account strip toggle)
        });
    });
    } // end if toolbar_visible

    // Spawn order ticket if the ORDER button in the top-nav right cluster was clicked.
    if order_clicked {
        if let Some(chart) = panes.get_mut(ap) {
            let fid = chart.floating_order_panes.iter().map(|p| p.id).max().unwrap_or(0) + 1;
            let sym = chart.symbol.clone();
            let sr  = ctx.screen_rect();
            chart.floating_order_panes.push(crate::chart_renderer::trading::FloatingOrderPane {
                id: fid, title: sym.clone(), symbol: sym,
                strike: 0.0, is_call: false, qty: 1,
                pos: egui::pos2(sr.center().x - 105.0, sr.center().y - 100.0),
                collapsed: false,
            });
        }
    }

    // ── Toolnav: second chrome row (tools + alert feed). Renders only when the
    // active style sets Chrome.toolnav_height > 0 (Aperture/Glass). Stacks
    // directly below the main toolbar, above the workspace.
    if toolbar_visible {
        super::toolnav::render_toolnav(ctx, watchlist, panes, ap, t);
    }

    if watchlist.account_strip_open {
        let mut do_cancel_all = false;
        let mut do_flatten    = false;
        egui::TopBottomPanel::top("account_strip")
            .exact_height(style_current().account_strip_height)
            .frame(egui::Frame::NONE.fill(t.toolbar_bg)
                .inner_margin(egui::Margin { left: 0, right: 0, top: 2, bottom: 2 })
                .stroke(egui::Stroke::new(stroke_thin(), tint(t, Tone::Border, alpha_dim()))))
            .show(ctx, |ui| {
                crate::chart::renderer::ui::chrome::pane::AccountStrip::new()
                    .account_data(account_data_cached.as_ref().map(|(a, _, _)| a))
                    .broker_url(APEXIB_URL)
                    .theme(&t)
                    .show(ui,
                        || { do_cancel_all = true; },
                        || { do_flatten    = true; });
            });
        if do_cancel_all {
            // WS-H #41a: guarded cancel-all via OrderManager (was cancel_all("")
            // + a raw broker DELETE that bypassed the paper guard/journal).
            crate::chart_renderer::trading::order_manager::cancel_all_working();
            for chart in panes.iter_mut() { chart.orders.clear(); }
        }
        if do_flatten {
            // WS-H #41a: guarded account-wide flatten via OrderManager (cancels
            // working orders + market-closes each position through submit()).
            crate::chart_renderer::trading::order_manager::flatten_all();
            for chart in panes.iter_mut() { chart.orders.retain(|o| o.status == OrderStatus::Executed); }
        }
    }

    // NOTE: TB_BTN_CLICKED is cleared at the END of draw_chart, AFTER the
    // window drag handler reads it. Do NOT clear it here — it was causing
    // the flag to always be false when the drag handler checked it, making
    // every toolbar click trigger drag_window() and un-maximizing the window.

    // ── Timeframe dropdown popup — delegated to dropdowns.rs ──
    super::dropdowns::render_timeframe_dropdown(ctx, watchlist, panes, ap, t);
    // ── Layout dropdown popup — delegated to dropdowns.rs ──
    super::dropdowns::render_layout_dropdown(ctx, watchlist, panes, layout, active_pane, t);


    // ── command_palette
    crate::chart_renderer::ui::command_palette::draw(ctx, watchlist, panes, layout, active_pane, t);

    // ── hotkey_editor
    crate::chart_renderer::ui::tools::hotkey_editor::draw(ctx, watchlist, panes, ap, t);

    // ── Settings panel
    crate::chart_renderer::ui::panels::settings_panel::draw(ctx, watchlist, &mut panes[ap], t, ap);
    // ── Theme Studio overlay (full-screen; drawn after settings so it renders on top)
    crate::chart_renderer::ui::theme_studio::draw_theme_studio(ctx, t);
    crate::chart_renderer::ui::panels::apex_diagnostics::draw(ctx, watchlist, t);
    crate::chart_renderer::ui::panels::replay_pane::draw(ctx, watchlist, t);

    // ── trendline_filter
    crate::chart_renderer::ui::tools::trendline_filter::draw(ctx, watchlist, panes, ap, t);
    crate::chart_renderer::ui::tools::option_quick_picker::draw(ctx, watchlist, panes, ap, t);
    crate::chart_renderer::ui::tools::template_popup::draw(ctx, watchlist, panes, ap, t);

    // ── indicator_editor
    crate::chart_renderer::ui::tools::indicator_editor::draw(ctx, watchlist, panes, ap, t);

    // ── overlay_manager
    crate::chart_renderer::ui::tools::overlay_manager::draw(ctx, watchlist, panes, ap, t);

    // ── Keyboard-shortcut cheatsheet (F1) — WS-G G4. Reads the central
    // shortcuts registry; renders via ui_kit::Modal. Cheap no-op when closed.
    crate::chart_renderer::ui::tools::shortcuts_help::draw(ctx, t);

    // ── Group manager popup ────────────────────────────────────────────────────
    // Migrated from the raw `dialog_window_themed` factory to the canonical Modal
    // shell so it gets the same scrim + entry/exit animation + draggable header +
    // drop shadow as every other dialog. Modal renders the title + close X and
    // defers `closed` until its exit anim finishes, so the `if open` gate plays
    // the fade-out correctly.
    if panes[ap].group_manager_open {
        let mut close_gm = false;
        let gm_resp = crate::ui_kit::widgets::modal::Modal::new("NEW GROUP")
            .id("group_manager")
            .ctx(ctx)
            .theme(t)
            .size(egui::vec2(250.0, 0.0))
            .anchor(crate::ui_kit::widgets::modal::Anchor::Window { pos: None })
            .frame_kind(crate::ui_kit::widgets::modal::FrameKind::DialogWindow)
            .header_style(crate::ui_kit::widgets::modal::HeaderStyle::Dialog)
            .separator(true)
            .show(|ui| {
                ui.add_space(gap_md());
                let m = 10.0;
                ui.horizontal(|ui| {
                    ui.add_space(m);
                    let resp = crate::ui_kit::widgets::Input::new(&mut panes[ap].new_group_name)
                        .placeholder("Group name...")
                        .min_width(230.0 - m * 2.0)
                        .size(crate::ui_kit::widgets::Size::Sm)
                        .show(ui, t);
                    ui.memory_mut(|m| m.request_focus(resp.editor_id));
                });
                ui.add_space(gap_lg());
                ui.horizontal(|ui| {
                    ui.add_space(m);
                    let can_create = !panes[ap].new_group_name.trim().is_empty();
                    let create_label = format!("{} Create", Icon::PLUS);
                    if crate::ui_kit::widgets::Button::action(create_label.as_str()).tint(t.accent).enabled(can_create).show(ui, t).clicked() {
                        let name = panes[ap].new_group_name.trim().to_string();
                        let id = new_uuid();
                        crate::drawing_db::save_group(&id, &name, None);
                        panes[ap].groups.push(DrawingGroup { id, name, color: None });
                        panes[ap].new_group_name.clear();
                        close_gm = true;
                    }
                });
                ui.add_space(gap_lg());
            });
        if gm_resp.closed { close_gm = true; }
        if close_gm { panes[ap].group_manager_open = false; }
    }

    // ── connection_panel
    crate::chart_renderer::ui::panels::connection_panel::draw(ctx, watchlist, panes, ap, t, conn_panel_open);

    // ── Welcome wizard (P2) ─────────────────────────────────────────────────
    // Runs every frame while the wizard is active. When `.show()` returns
    // false the wizard is finished: flip the flag so the next periodic
    // `save_state` call (which calls `push_to_ui_settings` + persists
    // the UiSettings aggregate) will write `has_seen_welcome = true`.
    //
    // The resume step is mirrored each frame so a force-quit mid-wizard
    // picks up where the user left off on the next launch.
    if watchlist.welcome_wizard.is_some() {
        let step_now = watchlist.welcome_wizard.as_ref().map(|w| w.step).unwrap_or(0);
        // PERF: only mirror when the step actually changed (was: blind write every frame).
        if watchlist.ui_settings.welcome_step_resume != step_now {
            watchlist.update_ui_settings(|s| s.welcome_step_resume = step_now);
        }

        let still_open = {
            let wiz = watchlist.welcome_wizard.as_mut().unwrap();
            wiz.show(ctx, t, conn_panel_open)
        };
        if !still_open {
            // W0-09 (audit): the wizard captured a daily-loss cap and then
            // DISCARDED it — never reaching the enforced risk gate. Apply it now,
            // but only on Finish (not Skip). daily_loss_cap → RiskLimits.
            // max_daily_loss, which the W0-05/06 daily-loss breaker enforces.
            // (max_position_pct has no account-size-aware home yet — disclosed in
            // the wizard's risk step, configured in Settings once connected.)
            if let Some(wiz) = watchlist.welcome_wizard.as_ref() {
                if wiz.finished {
                    let cap = wiz.daily_loss_cap as f64;
                    let mut limits = crate::chart_renderer::trading::order_manager::get_risk_limits();
                    limits.max_daily_loss = cap.max(0.0);
                    crate::chart_renderer::trading::order_manager::update_risk_limits(limits);
                }
            }
            watchlist.update_ui_settings(|s| {
                s.has_seen_welcome = true;
                s.welcome_step_resume = 0;
            });
            watchlist.welcome_wizard = None;
        }
    }

    // ── Toast 2.0 — bottom-left anchor, fixed 360px, severity-coded ─────────
    //
    // Severity → visuals, taken from `Notification.severity`
    // (`NotificationSeverity`, resolved in `severity_visuals` below):
    //   Info    → accent tint, Icon::INFO
    //   Success → bull tint,   Icon::CHECK_CIRCLE
    //   Warning → warn tint,   Icon::WARNING
    //   Error   → bear tint + critical pulse, Icon::SHIELD_WARNING_FILL
    //
    // Layout rules:
    //   - Anchored bottom-left: 16px from left edge, 16px from bottom.
    //   - Fixed width: 360px. Long messages wrap inside.
    //   - Newest toast at the bottom (closest to anchor), oldest floats up.
    //   - Max 4 visible; beyond that a "+N more" pill appears above the stack.
    //   - Pinned toasts (click-to-pin) float to the TOP of the stack.
    //   - Stagger: 80ms per slot (preserved from previous impl).
    //   - Slide-up: each toast slides in 8px from below + fades in over 120ms.
    //
    // Pin state is stored in egui temp memory keyed on toast index + creation
    // time hash. Pinning is session-only (egui memory is not persisted).
    //
    // "Expand" state (N more chip click) is stored in egui temp memory.
    if crate::chart_renderer::ui::tools::notification::routing().toasts_enabled && !toasts.is_empty() {
        use egui::{Id, pos2, vec2, Rect, CornerRadius, Stroke};
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let screen = ctx.screen_rect();
        const TOAST_W: f32 = 360.0;
        const TOAST_MARGIN: f32 = 16.0;
        const LEFT: f32 = TOAST_MARGIN;
        const MAX_VISIBLE: usize = 4;
        const MAX_PINNED: usize = 5;
        const LEFT_BAR_W: f32 = 3.0;
        const DISMISS_SECS: f32 = 5.0;

        // ── Severity → icon + is_critical ──────────────────────────────────
        fn severity_visuals(sev: crate::chart_renderer::ui::tools::notification::NotificationSeverity, t: &crate::chart_renderer::gpu::Theme)
            -> (egui::Color32, &'static str, bool)
        {
            use crate::chart_renderer::ui::tools::notification::NotificationSeverity;
            match sev {
                NotificationSeverity::Info    => (t.accent, Icon::INFO,                false),
                NotificationSeverity::Success => (t.bull,   Icon::CHECK_CIRCLE,        false),
                NotificationSeverity::Warning => (t.warn,   Icon::WARNING,             false),
                NotificationSeverity::Error   => (t.bear,   Icon::SHIELD_WARNING_FILL, true),
            }
        }

        // ── Toast id — stable identifier from notification id ───────────────
        fn toast_id(n: &crate::chart_renderer::ui::tools::notification::Notification) -> u64 {
            n.id
        }

        // ── Expand state (show all beyond MAX_VISIBLE) ──────────────────────
        let expand_id = Id::new("toast_v2_expand");
        let expand_until: f64 = ctx.data(|d| d.get_temp(expand_id).unwrap_or(0.0f64));
        let expanded = ctx.input(|i| i.time) < expand_until;

        // ── Separate pinned vs normal toasts ────────────────────────────────
        let mut pinned_indices: Vec<usize> = Vec::new();
        let mut normal_indices: Vec<usize> = Vec::new();
        for (i, n) in toasts.iter().enumerate() {
            let tid = toast_id(n);
            let pin_id = Id::new(("toast_pin", tid));
            let is_pinned: bool = ctx.data(|d| d.get_temp(pin_id).unwrap_or(false));
            if is_pinned { pinned_indices.push(i); } else { normal_indices.push(i); }
        }

        // Build render order: pinned first (float to top = rendered highest),
        // then normal newest-at-bottom. "Top" means highest Y index = smallest
        // Y coordinate in bottom-left layout.
        // We render bottom-up, so normal order first (oldest top), then pinned.
        // Actual render order for Y calculation: [pinned..., normal...]
        // where index 0 is the topmost (highest above anchor), last is closest to anchor.
        let render_order: Vec<usize> = {
            let mut v = pinned_indices.clone();
            v.extend(normal_indices.iter().cloned());
            v
        };

        let total = render_order.len();
        let visible_limit = if expanded { total } else { MAX_VISIBLE };
        let hidden_count = total.saturating_sub(visible_limit);
        let visible_indices: Vec<usize> = render_order.iter().cloned().take(visible_limit).collect();

        // ── Chip height (if needed) ─────────────────────────────────────────
        let chip_h: f32 = if hidden_count > 0 { 24.0 } else { 0.0 };
        let chip_gap: f32 = if hidden_count > 0 { 4.0 } else { 0.0 };

        // ── Compute Y positions bottom-up ───────────────────────────────────
        // We need approximate heights. We can estimate based on message
        // wrapping, but for simplicity we use egui's auto-sizing windows
        // and stack them with a fixed pitch. Messages that wrap will push the
        // next toast up — use a min pitch of 68px (icon+2 lines comfortable).
        // The actual positions are calculated after we know the count.
        let toast_pitch: f32 = 72.0; // estimated height per toast + gap
        // Sit 8px above the footer (bottom dock) so toasts never cover its
        // header; this rides up automatically as the footer expands.
        let footer_h = crate::chart_renderer::ui::panels::bottom_dock::current_height(watchlist);
        let base_y = screen.bottom() - footer_h - 8.0 - chip_h - chip_gap;

        // ── Render visible toasts ───────────────────────────────────────────
        let now_time = ctx.input(|i| i.time);
        let mut close_toast_idx: Option<usize> = None;
        let mut pin_toggle_idx: Option<(usize, bool)> = None; // (toast_idx, new_pin_state)

        for (slot, &toast_i) in visible_indices.iter().enumerate() {
            let n = &toasts[toast_i];
            let age = n.created.elapsed().as_secs_f32();
            // Stagger: toast at slot only starts appearing after slot*80ms (preserve existing stagger).
            let stagger_delay = toast_i as f32 * 0.08;
            let visible_age = (age - stagger_delay).max(0.0);
            if visible_age <= 0.0 { continue; }

            let tid = toast_id(n);
            let pin_id = Id::new(("toast_pin", tid));
            let is_pinned: bool = ctx.data(|d| d.get_temp(pin_id).unwrap_or(false));

            // Alpha: fade in over 120ms, start fading out 1s before expiry.
            // Pinned toasts don't fade out.
            let fade_in  = (visible_age / 0.12).min(1.0);
            let fade_out = if is_pinned { 1.0f32 } else { ((DISMISS_SECS - age) / 1.0).min(1.0).max(0.0) };
            let alpha = (fade_in * fade_out).min(1.0).max(0.0);
            if alpha <= 0.0 { continue; }

            let (sev_color, icon, is_critical) = severity_visuals(n.severity, t);
            let display_msg = n.message.as_str();

            // Slide-up: offset 8px at start, resolves to 0 over 120ms.
            let slide_offset = 8.0 * (1.0 - fade_in);

            // Y position: bottom-up from base_y, slot 0 is topmost visible.
            // Slot `visible_indices.len()-1` is bottommost (closest to anchor).
            let slot_from_bottom = (visible_indices.len() - 1 - slot) as f32;
            let y_top = base_y - (slot_from_bottom + 1.0) * toast_pitch + slide_offset;

            // ── Critical pulse on left bar ──────────────────────────────────
            // Alpha oscillates 0.8→1.0 on a 0.6s cycle. Stops when pinned.
            let bar_alpha_f = if is_critical && !is_pinned {
                let t_cycle = (now_time % 0.6) as f32 / 0.6;
                0.8 + 0.2 * (t_cycle * std::f32::consts::TAU).sin().abs()
            } else {
                1.0
            };

            // ── Colors ──────────────────────────────────────────────────────
            let base_bg    = elevation_3(t);
            let tint_alpha = if is_critical { alpha_muted() } else { alpha_soft() };
            // Tint is painted additively over the base using alpha blend.
            // We approximate by lerping toward the severity color.
            let tint_col   = color_alpha(sev_color, (tint_alpha as f32 * alpha) as u8);
            let body_bg    = egui::Color32::from_rgba_unmultiplied(
                base_bg.r(), base_bg.g(), base_bg.b(), (230.0 * alpha) as u8);
            let bar_col    = color_alpha(sev_color, (255.0 * bar_alpha_f * alpha) as u8);
            let text_col   = tint(t, Tone::Text, (230.0 * alpha) as u8);
            let icon_col   = color_alpha(sev_color, (200.0 * alpha) as u8);
            let dim_col    = tint(t, Tone::Dim, (160.0 * alpha) as u8);
            let border_col = if is_pinned {
                color_alpha(sev_color, (alpha_muted() as f32 * alpha) as u8)
            } else {
                egui::Color32::TRANSPARENT
            };
            let border_w   = if is_pinned { 1.5 } else { 0.0 };

            let corner = r_md_cr();
            let win_id = format!("toast_v2_{}", toast_i);

            // Use egui::Window for auto-sizing (wraps long text).
            // Shadow via Frame.
            let shadow = shadow_card_themed(t);
            egui::Window::new(&win_id)
                .id(Id::new(win_id.as_str()))
                .fixed_pos(pos2(screen.left() + LEFT, y_top))
                .fixed_size(vec2(TOAST_W, 0.0)) // height is auto
                .max_width(TOAST_W)
                .min_width(TOAST_W)
                .title_bar(false)
                .resizable(false)
                .frame(
                    egui::Frame::NONE
                        .fill(body_bg)
                        .shadow(shadow)
                        .corner_radius(corner)
                        .stroke(Stroke::new(border_w, border_col))
                        .inner_margin(egui::Margin::same(0i8))
                )
                .show(ctx, |ui| {
                    // Paint severity tint OVER the base bg.
                    let r = ui.max_rect();
                    ui.painter().rect_filled(r, corner, tint_col);

                    // Paint left accent bar.
                    let bar_rect = Rect::from_min_size(r.min, egui::vec2(LEFT_BAR_W, r.height()));
                    ui.painter().rect_filled(bar_rect, CornerRadius {
                        nw: corner.nw, sw: corner.sw, ne: 0, se: 0,
                    }, bar_col);

                    ui.set_width(TOAST_W);
                    let inner = ui.available_rect_before_wrap();
                    // Main content row inside left margin for the bar + padding.
                    let content_x = LEFT_BAR_W + gap_sm();
                    ui.add_space(gap_xs());
                    egui::Frame::NONE
                        .inner_margin(egui::Margin {
                            left: content_x as i8, right: gap_sm() as i8, top: 0i8, bottom: 0i8,
                        })
                        .show(ui, |ui| {
                            // ── Top row: icon + message + close button ──────
                            ui.horizontal_wrapped(|ui| {
                                ui.label(TextStyle::Body.as_rich_cascading(icon, icon_col));
                                ui.add_space(gap_xs());
                                // Message — wraps at TOAST_W boundary.
                                ui.label(
                                    TextStyle::BodySm.as_rich_cascading(display_msg, text_col)
                                );
                            });

                            // ── Bottom row: age label + pin hint + close ────
                            ui.horizontal(|ui| {
                                let age_str = if age < 60.0 {
                                    format!("{}s ago", age as u32)
                                } else {
                                    "1m+ ago".to_string()
                                };
                                ui.label(TextStyle::Caption.as_rich_cascading(&age_str, dim_col));

                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    // Close button — always dismisses (even if pinned).
                                    let close_resp = KitButton::new(Icon::X).variant(KitVariant::Ghost)
                                        .size(KitSize::Xs).fg(dim_col).frameless(true).show(ui, t);
                                    if close_resp.clicked() {
                                        close_toast_idx = Some(toast_i);
                                    }

                                    // Pin button (visible on hover + always if pinned).
                                    // We approximate hover by checking if any widget in the
                                    // window rect is hovered. Since we can't get the window
                                    // rect here easily, we show pin always for pinned toasts
                                    // and on hover for unpinned (egui will repaint on hover).
                                    // U1-6: pinned = filled pin, unpinned = outline (both branches
                                    // were identical, so the glyph never signaled state — only color did).
                                    let pin_icon = if is_pinned { Icon::PUSH_PIN_FILL } else { Icon::PUSH_PIN };
                                    let pin_col = if is_pinned {
                                        color_alpha(sev_color, (180.0 * alpha) as u8)
                                    } else {
                                        dim_col
                                    };
                                    let pin_resp = KitButton::new(pin_icon).variant(KitVariant::Ghost)
                                        .size(KitSize::Xs).fg(pin_col).frameless(true).show(ui, t);
                                    Tooltip::new(if is_pinned { "Unpin (right-click)" } else { "Pin toast" }).show(ui, &pin_resp, t);
                                    if pin_resp.clicked() && !is_pinned {
                                        pin_toggle_idx = Some((toast_i, true));
                                    }
                                    if pin_resp.secondary_clicked() && is_pinned {
                                        pin_toggle_idx = Some((toast_i, false));
                                    }
                                });
                            });
                        });
                    ui.add_space(gap_xs());
                });
        }

        // ── "+N more" chip ──────────────────────────────────────────────────
        if hidden_count > 0 {
            let chip_y = base_y - visible_indices.len() as f32 * toast_pitch - chip_h - 2.0;
            let chip_col = tint(t, Tone::Dim, alpha_muted());
            let chip_bg  = tint(t, Tone::Surface, alpha_muted());
            egui::Window::new("toast_v2_chip")
                .id(Id::new("toast_v2_chip"))
                .fixed_pos(pos2(screen.left() + LEFT, chip_y))
                .auto_sized()
                .title_bar(false)
                .resizable(false)
                .frame(egui::Frame::NONE.fill(chip_bg).corner_radius(CornerRadius::same(12u8)))
                .show(ctx, |ui| {
                    let lbl = format!("+{hidden_count} more");
                    let resp = KitButton::new(lbl.as_str()).variant(KitVariant::Ghost).size(KitSize::Xs)
                        .fg(chip_col).frameless(true).show(ui, t);
                    if resp.clicked() {
                        let expand_until_new: f64 = ctx.input(|i| i.time) + 10.0;
                        ctx.data_mut(|d| d.insert_temp(expand_id, expand_until_new));
                    }
                });
        }

        // ── Apply pin toggles ───────────────────────────────────────────────
        if let Some((toast_i, new_pin)) = pin_toggle_idx {
            let tid = toast_id(&toasts[toast_i]);
            let pin_id = Id::new(("toast_pin", tid));
            if new_pin {
                // Enforce max 5 pinned: unpin oldest if at cap.
                if pinned_indices.len() >= MAX_PINNED {
                    if let Some(&oldest_i) = pinned_indices.first() {
                        let old_tid = toast_id(&toasts[oldest_i]);
                        let old_pin_id = Id::new(("toast_pin", old_tid));
                        ctx.data_mut(|d| d.insert_temp::<bool>(old_pin_id, false));
                    }
                }
            }
            ctx.data_mut(|d| d.insert_temp(pin_id, new_pin));
        }

        // Note: close_toast_idx is informational — the actual expiry is handled
        // by gpu.rs (retain toasts where age < 5s). For pinned closes, we unpin immediately.
        if let Some(toast_i) = close_toast_idx {
            let tid = toast_id(&toasts[toast_i]);
            let pin_id = Id::new(("toast_pin", tid));
            ctx.data_mut(|d| d.insert_temp::<bool>(pin_id, false));
        }
    }

    // RegimeTape moved off the top dock — it now lives as a tab inside the
    // Signals sidebar (signals_panel.rs → SignalsTab::Regime). Top dock space
    // was too valuable to reserve for a 40-48px always-visible cell strip.

    // ── Bottom dock (footer): Orders / Positions / Account / Notifications.
    // Created before the rails so it spans the full width beneath them and the
    // workspace + rails shrink above it. Toggle: Ctrl+` .
    if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::Backtick)) {
        let now = crate::chart_renderer::ui::style::footer_visible();
        crate::chart_renderer::ui::style::set_footer_override(Some(!now));
    }
    span_begin("sidebar.bottom_dock");
    crate::chart_renderer::ui::panels::bottom_dock::draw(ctx, watchlist, account_data_cached, t);

    // ── Right rail: the SINGLE auto-arranged container for ALL dockable
    // right-side panels — watchlist, orders, indicators, playbook, scanner,
    // tape, journal, rrg, script, order ledger, provenance. Each renders
    // through `SidePanelShell` rail-mode, dispatched by the `right_rail::PANELS`
    // registry. There are deliberately NO per-panel draw calls for them here.
    span_begin("sidebar.right_rail");
    crate::chart_renderer::ui::panels::right_rail::render(ctx, watchlist, panes, ap, account_data_cached, *layout, t);

    // Provenance auto-opens from external lineage requests (SOTA UX §4.1), so
    // its per-frame pump must run even while the panel is closed; the rail
    // renders the panel itself once open.
    crate::chart_renderer::ui::panels::provenance_pane::pump(watchlist);

    // Order Ledger toggle (Ctrl+L) — the panel is rail-hosted.
    if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::L)) {
        watchlist.update_sidebar_state(|s| s.order_ledger_open = !s.order_ledger_open);
    }

    // ── MSG panel hotkeys (B3) ────────────────────────────────────────────────
    // Three fully-implemented MSG panels were permanently unreachable (their
    // toggle() had zero callers anywhere). Wired here per 09-trader-ux §BLOCKER-1.
    //
    //  Ctrl+Shift+R  — MSG Weighted RRG  (basket_rrg: sector rotation quadrants)
    //  Ctrl+Shift+I  — MSG Influence     (basket_influence: flow-share heat)
    //  Ctrl+Shift+T  — MSG Tension/Scenario ladder (basket_tension payoff ladder)
    //
    // Chosen keys: not used by any existing hotkey block. R=Rotation (RRG),
    // I=Influence, T=Tension/ladder. Shift qualifier avoids Ctrl+R (refresh in
    // some egui builds) and Ctrl+T (tab-related OS shortcuts).
    if ctx.input(|i| i.modifiers.command && i.modifiers.shift && i.key_pressed(egui::Key::R)) {
        crate::chart_renderer::ui::panels::msg_rrg_panel::toggle();
    }
    if ctx.input(|i| i.modifiers.command && i.modifiers.shift && i.key_pressed(egui::Key::I)) {
        crate::chart_renderer::ui::panels::msg_influence_panel::toggle();
    }
    if ctx.input(|i| i.modifiers.command && i.modifiers.shift && i.key_pressed(egui::Key::T)) {
        crate::chart_renderer::ui::panels::msg_tension_panel::toggle();
    }

    // ── Screener panel hotkey — Ctrl+Shift+S (Wave S2) ────────────────────────
    if ctx.input(|i| i.modifiers.command && i.modifiers.shift && i.key_pressed(egui::Key::S)) {
        let currently_open = watchlist.sidebar_state_snapshot().screener_panel_open;
        crate::chart_renderer::commands::push(
            crate::chart_renderer::commands::AppCommand::OpenScreenerPanel { open: !currently_open }
        );
    }

    // ── Object Tree side panel (not rail-hosted)
    span_begin("sidebar.object_tree");
    crate::chart_renderer::ui::panels::object_tree::draw(ctx, watchlist, panes, ap, t);

    // ── Order System Health (floating Modal, not rail-hosted) ────────────────
    // Hotkey: Ctrl+Shift+O toggles a small dashboard with submit-latency,
    // reject-rate, top reject reasons, active-state counts and broker contact age.
    if ctx.input(|i| i.modifiers.command && i.modifiers.shift && i.key_pressed(egui::Key::O)) {
        watchlist.update_sidebar_state(|s| s.order_health_open = !s.order_health_open);
    }
    span_begin("sidebar.order_health");
    crate::chart_renderer::ui::panels::order_health_panel::draw(ctx, watchlist, t);

    // ── Screener panel (Wave S2) — Ctrl+Shift+S ───────────────────────────────
    span_begin("sidebar.screener");
    crate::chart_renderer::ui::panels::screener_panel::draw(ctx, watchlist, panes, ap, t);

    // Analysis + Signals are now standard SidePanelShell::tabs panels hosted by
    // the right rail (see right_rail::PANELS) — no direct draw here.

    // ── Trade plan v2 sidebar (SOTA §4.4)
    span_begin("sidebar.trade_plan_v2");
    crate::chart_renderer::ui::panels::trade_plan_panel::draw(ctx, watchlist, panes, ap, t);

    // ── Spike-explanation popup overlay (SOTA §4.5).
    // Floats over the panel grid in the chart window's top-right corner.
    // Polls live_state.recent_spike_explanations every frame and renders new
    // ids as transient toasts (30s auto-dismiss).
    span_begin("overlay.spike_popup");
    let screen_rect = ctx.screen_rect();
    crate::chart_renderer::ui::panels::spike_popup::draw(ctx, screen_rect);

    // Feed is now a standard SidePanelShell::tabs panel hosted by the right rail.

    // Chart Library ("Charts") is now a standard SidePanelShell::tabs panel
    // hosted by the right rail (see right_rail::PANELS) — no direct draw here.

    // ── Spread Builder panel (floating Modal, not rail-hosted)
    span_begin("sidebar.spread");
    crate::chart_renderer::ui::panels::spread_panel::draw(ctx, watchlist, &panes[ap].symbol, t);
    span_begin("top_panel.tail");

    // ── Alert checking — run every frame, check if any alert prices were crossed ──
    {
        let active_prices: Vec<(String, f32)> = panes.iter()
            .filter_map(|p| p.bars.last().map(|b| (p.symbol.clone(), b.close)))
            .collect();
        // Collect ids + toast info for alerts that need to fire; then batch-update.
        let to_trigger: Vec<(u32, String, f32, bool)> = watchlist.alerts.iter()
            .filter(|a| !a.triggered)
            .filter_map(|alert| {
                active_prices.iter().find(|(s, _)| *s == alert.symbol).and_then(|(_, price)| {
                    if (alert.above && *price >= alert.price) || (!alert.above && *price <= alert.price) {
                        Some((alert.id, alert.symbol.clone(), alert.price, alert.above))
                    } else {
                        None
                    }
                })
            })
            .collect();
        if !to_trigger.is_empty() {
            let ids: Vec<u32> = to_trigger.iter().map(|(id, ..)| *id).collect();
            watchlist.update_alerts_state(|s| {
                for a in s.alerts.iter_mut() {
                    if ids.contains(&a.id) { a.triggered = true; }
                }
            });
            for (_, symbol, price, above) in to_trigger {
                let dir = if above { "above" } else { "below" };
                let msg = format!("ALERT: {} {} {:.2}", symbol, dir, price);
                eprintln!("[ALERT TRIGGERED] {} -- sound notification placeholder", msg);
                crate::chart_renderer::ui::tools::notification::push_pending(
                    crate::chart_renderer::ui::tools::notification::Notification::new(msg, crate::chart_renderer::ui::tools::notification::NotificationSeverity::Warning).with_value(price).with_source("alerts")
                );
            }
        }
    }

    // ── Deferred watchlist tooltip (rendered OUTSIDE the panel) ──
    if let Some(tip) = PENDING_WL_TOOLTIP.with(|t| t.borrow_mut().take()) {
        let tip_w = 220.0;
        let tip_x = (tip.sidebar_left - tip_w - 8.0).max(4.0);
        let tip_y = tip.anchor_y - 60.0;
        let change_pct = if tip.prev_close > 0.0 { (tip.price / tip.prev_close - 1.0) * 100.0 } else { 0.0 };
        let chg_col = if change_pct >= 0.0 { t.bull } else { t.bear };
        let dim = t.dim;
        let st = style_current();
        let wl_tip_cr = st.r_md as f32;
        let wl_tip_stroke_w = if st.hairline_borders { st.stroke_std } else { crate::chart_renderer::ui::style::stroke_thin() };
        let wl_tip_border = if st.hairline_borders { t.toolbar_border } else { tint(t, Tone::Border, crate::chart_renderer::ui::style::alpha_strong()) };
        egui::Area::new(egui::Id::new("wl_tooltip_deferred"))
            .fixed_pos(egui::pos2(tip_x, tip_y))
            .order(egui::Order::Tooltip)
            .show(ctx, |ui| {
                egui::Frame::popup(&ctx.style()).fill(t.toolbar_bg)
                    .stroke(egui::Stroke::new(wl_tip_stroke_w, wl_tip_border))
                    .inner_margin(crate::chart_renderer::ui::style::gap_lg()).corner_radius(wl_tip_cr).show(ui, |ui| {
                    ui.set_max_width(tip_w);
                    ui.label(TextStyle::NumericLg.as_rich_cascading(&tip.sym, t.text));
                    ui.horizontal(|ui| {
                        ui.label(TextStyle::Numeric.as_rich_cascading(&format!("${:.2}", tip.price), tint(t, Tone::Text, 220)));
                        ui.label(TextStyle::Numeric.as_rich_cascading(&format!("{:+.2}%", change_pct), chg_col));
                    });
                    ui.add_space(gap_sm()); ui.separator(); ui.add_space(gap_sm());
                    if tip.day_high > tip.day_low {
                        ui.horizontal(|ui| {
                            ui.label(TextStyle::Caption.as_rich_cascading("Day", dim));
                            ui.label(TextStyle::MonoSm.as_rich_cascading(&format!("{:.2}", tip.day_low), dim));
                            let bar_w = 60.0;
                            let (bar_rect, _) = ui.allocate_exact_size(egui::vec2(bar_w, 8.0), egui::Sense::hover());
                            ui.painter().rect_filled(bar_rect, 2.0, tint(t, Tone::Text, 15));
                            let range = tip.day_high - tip.day_low;
                            if range > 0.0 {
                                let pos = ((tip.price - tip.day_low) / range).clamp(0.0, 1.0);
                                ui.painter().circle_filled(egui::pos2(bar_rect.left() + pos * bar_w, bar_rect.center().y), 3.0, chg_col);
                            }
                            ui.label(TextStyle::MonoSm.as_rich_cascading(&format!("{:.2}", tip.day_high), dim));
                        });
                    }
                    if tip.high_52wk > tip.low_52wk {
                        ui.horizontal(|ui| {
                            ui.label(TextStyle::Caption.as_rich_cascading("52w", dim));
                            ui.label(TextStyle::MonoSm.as_rich_cascading(&format!("{:.0}", tip.low_52wk), dim));
                            let bar_w = 60.0;
                            let (bar_rect, _) = ui.allocate_exact_size(egui::vec2(bar_w, 8.0), egui::Sense::hover());
                            ui.painter().rect_filled(bar_rect, 2.0, tint(t, Tone::Text, 15));
                            let range = tip.high_52wk - tip.low_52wk;
                            if range > 0.0 {
                                let pos = ((tip.price - tip.low_52wk) / range).clamp(0.0, 1.0);
                                ui.painter().circle_filled(egui::pos2(bar_rect.left() + pos * bar_w, bar_rect.center().y), 3.0, t.accent);
                            }
                            ui.label(TextStyle::MonoSm.as_rich_cascading(&format!("{:.0}", tip.high_52wk), dim));
                        });
                    }
                    ui.add_space(gap_xs());
                    ui.horizontal(|ui| {
                        ui.label(TextStyle::MonoSm.as_rich_cascading(&format!("ATR {:.2}", tip.atr), dim));
                        ui.label(TextStyle::MonoSm.as_rich_cascading(&format!("RVOL {:.1}x", tip.rvol),
                            if tip.rvol > 2.0 { t.warn } else { dim }));
                    });
                    if change_pct.abs() > tip.avg_range * 1.5 {
                        ui.label(TextStyle::Caption.as_rich_cascading("EXTREME MOVE", chg_col));
                    }
                    if tip.earnings_days >= 0 && tip.earnings_days <= 14 {
                        ui.add_space(gap_xs());
                        ui.label(TextStyle::MonoSm.as_rich_cascading(&format!("{} Earnings in {} days", Icon::LIGHTNING, tip.earnings_days), tint(t, Tone::Accent, alpha_heavy())));
                    }
                    if !tip.tags.is_empty() {
                        ui.add_space(gap_xs());
                        ui.horizontal_wrapped(|ui| { for tag in &tip.tags { ui.label(TextStyle::Caption.as_rich_cascading(tag, t.accent)); } });
                    }
                    if tip.alert_triggered {
                        ui.label(TextStyle::MonoSm.as_rich_cascading(&format!("{} Alert triggered", Icon::LIGHTNING), t.bear));
                    }
                });
            });
    }

    span_end(); // top_panel
}

/// Draw the three-element Xolio logo mark (dot + boomerang arc + ring) into `rect`.
fn draw_xolio_logo(p: egui::Painter, rect: egui::Rect, color: egui::Color32) {
    let sc = rect.width() / 24.0;
    let sw = (rect.width() * 0.065).max(1.0);
    let to_s = |x: f32, y: f32| egui::pos2(rect.min.x + x * sc, rect.min.y + y * sc);

    // Dot — small circle, top-left
    let dot_c = to_s(6.15852, 6.28967);
    let dot_r = (3.5638 * sc - sw * 0.5).max(0.8);
    p.circle_stroke(dot_c, dot_r, egui::Stroke::new(sw, color));

    // Ring — large circle, bottom-right
    let ring_c = to_s(15.577, 15.7082);
    let ring_r = (5.85481 * sc - sw * 0.5).max(0.8);
    p.circle_stroke(ring_c, ring_r, egui::Stroke::new(sw, color));

    // Boomerang — closed bezier path sampled as polyline
    let n = 6usize;
    let cb = |p0: [f32; 2], p1: [f32; 2], p2: [f32; 2], p3: [f32; 2]| -> Vec<egui::Pos2> {
        (0..n).map(|i| {
            let t = i as f32 / n as f32;
            let u = 1.0 - t;
            to_s(
                u*u*u*p0[0]+3.0*u*u*t*p1[0]+3.0*u*t*t*p2[0]+t*t*t*p3[0],
                u*u*u*p0[1]+3.0*u*u*t*p1[1]+3.0*u*t*t*p2[1]+t*t*t*p3[1],
            )
        }).collect()
    };
    let mut bpts: Vec<egui::Pos2> = Vec::with_capacity(50);
    bpts.extend(cb([13.6456,3.38161],[15.5131,1.51417],[18.5651,1.53812],[20.4625,3.43547]));
    bpts.extend(cb([20.4625,3.43547],[22.3595,5.33285],[22.3837,8.385],[20.5163,10.2525]));
    bpts.push(to_s(20.4293,10.5439)); bpts.push(to_s(10.4567,20.5166));
    bpts.extend(cb([10.4353,20.538],[8.52338,22.45],[5.42336,22.4505],[3.51134,20.5387]));
    bpts.extend(cb([3.51134,20.5387],[1.59935,18.6267],[1.59935,15.526],[3.51134,13.614]));
    bpts.push(to_s(13.1263,3.99895));
    bpts.extend(cb([13.1263,3.99895],[13.2793,3.78238],[13.4519,3.57531],[13.6456,3.38161]));
    bpts.push(bpts[0]); // close path
    p.add(egui::Shape::line(bpts, egui::Stroke::new(sw, color)));
}
