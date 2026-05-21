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
    toasts: &'a [(String, f32, std::time::Instant, bool)],
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
    pub fn toasts(mut self, t: &'a [(String, f32, std::time::Instant, bool)]) -> Self { self.toasts = t; self }

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
use winit::window::Window;

use crate::ui_kit::icons::Icon;
use crate::ui_kit::widgets::{Button as KitButton, NumberStepper, SelectableRow, Tooltip, tokens::{Variant as KitVariant, Size as KitSize}};
use crate::ui_kit::widgets::icon_placement::IconPlacement;
use crate::chart_renderer::gpu::{
    Chart, Layout, Watchlist, Theme,
    CURRENT_WINDOW, CLOSE_REQUESTED, TB_BTN_CLICKED, PENDING_TOASTS, PENDING_WL_TOOLTIP,
    WlTooltipData,
    ALL_LAYOUTS,
    APEXIB_URL,
    THEMES,
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
    dialog_window_themed,
    STROKE_STD, STROKE_THIN,
    ALPHA_FAINT, ALPHA_GHOST, ALPHA_DIM, ALPHA_HEAVY,
    BTN_ICON_SM, BTN_ICON_LG,
    icon_sm,
    set_toolbar_rect, tb_group_break, current as style_current,
    font_4xs, font_xs, font_2xs, font_sm, font_md, font_lg, font_xl,
    alpha_soft, alpha_muted, alpha_ghost, alpha_strong, alpha_dim,
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
use super::toolbar_btn;

/// Wave 13a helper: publish a `PaneEvent::ToggleChanged` for the
/// fan-out flow used by top_nav menu toggles. Caller has already
/// flipped the originator's field (`panes[ap].<field> = nv`); this
/// just broadcasts the new value to sibling panes when the user held
/// Shift or broadcast mode is on. The originator is skipped by
/// `apply_pane_events` via the `Some(ap)` origin tag, so the caller
/// never double-writes.
fn publish_toggle(
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
fn publish_swing_leg_mode(
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

/// Paint a full-toolbar-height column tint behind a `menu_button` (or any
/// other widget) when hovered or active. Mirrors the column-fill hover/active
/// treatment baked into `style::tb_btn` for the right-side panel toggles, so
/// every header element — icon buttons, dropdowns, panel toggles — shares the
/// same hover/active pixel signature regardless of which widget primitive it
/// uses underneath.
fn paint_nav_col_tint(
    ui: &egui::Ui,
    tb_rect: egui::Rect,
    btn_rect: egui::Rect,
    theme: &crate::chart_renderer::gpu::Theme,
    hovered: bool,
    active: bool,
    label_id: &str,
) {
    use crate::chart::renderer::ui::components::motion;
    let active_id = egui::Id::new(("nav_col_active", label_id));
    let hover_id  = egui::Id::new(("nav_col_hover",  label_id));
    let active_t = motion::ease_bool(ui.ctx(), active_id, active, motion::MED);
    let hover_t  = motion::ease_bool(ui.ctx(), hover_id,  hovered && !active, motion::FAST);
    if active_t < 0.001 && hover_t < 0.001 { return; }

    let active_target = color_alpha(theme.toolbar_border, alpha_strong());
    let hover_target  = color_alpha(theme.dim,            alpha_ghost());
    let mut tint = motion::lerp_color(egui::Color32::TRANSPARENT, hover_target, hover_t);
    tint = motion::lerp_color(tint, active_target, active_t);
    if tint.a() == 0 { return; }

    let col_rect = egui::Rect::from_min_max(
        egui::pos2(btn_rect.left(),  tb_rect.top()),
        egui::pos2(btn_rect.right(), tb_rect.bottom()),
    );
    let bg_painter = ui.ctx().layer_painter(
        egui::LayerId::new(egui::Order::Background, egui::Id::new(("nav_col_bg", label_id)))
    );
    bg_painter.rect_filled(col_rect, 0.0, tint);

    if active_t > 0.001 {
        let st = style_current();
        let ul_thickness = if st.tab_underline_thickness > 0.0 { st.tab_underline_thickness } else { st.stroke_bold };
        let underline_y = tb_rect.bottom() - 1.0;
        let ul_color = motion::fade_in(theme.accent, active_t);
        bg_painter.line_segment(
            [egui::pos2(btn_rect.left(),  underline_y),
             egui::pos2(btn_rect.right(), underline_y)],
            egui::Stroke::new(ul_thickness, ul_color));
    }
}

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
    toasts: &[(String, f32, std::time::Instant, bool)],
) {
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
                        ui.label(egui::RichText::new(msg)
                            .color(fg)
                            .size(font_md() as f32)
                            .strong());
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

    if toolbar_visible {
    // Toolbar height scaled per active style (1.40× for Meridien Bloomberg-style tall bar) (#4).
    let tb_scale = style_current().toolbar_height_scale;
    egui::TopBottomPanel::top("tb")
        .frame(egui::Frame::NONE.fill(t.toolbar_bg).inner_margin(egui::Margin { left: gap_xs() as i8, right: 0, top: 0, bottom: 0 }))
        .exact_height((if watchlist.compact_mode { 30.0 } else { 38.0 }) * tb_scale)
        .show(ctx, |ui| {
        let tb_rect = ui.max_rect();
        // Publish toolbar rect so tb_btn can read it for full-height hover/active column overlays.
        set_toolbar_rect(tb_rect);
        crate::design_tokens::register_hit(
            [tb_rect.min.x, tb_rect.min.y, tb_rect.width(), tb_rect.height()],
            "TOOLBAR", "Toolbar");

        // Window drag handle — spans the full toolbar. Uses Sense::drag only,
        // so later-drawn buttons (which sense click) get priority for clicks.
        // Double-click toggles maximize.
        let drag_resp = ui.interact(tb_rect, egui::Id::new("tb_window_drag"), egui::Sense::click_and_drag());
        if drag_resp.drag_started() {
            let win_ref: Option<Arc<Window>> = CURRENT_WINDOW.with(|w| w.borrow().clone());
            if let Some(w) = &win_ref { let _ = w.drag_window(); }
        }
        if drag_resp.double_clicked() {
            let win_ref: Option<Arc<Window>> = CURRENT_WINDOW.with(|w| w.borrow().clone());
            if let Some(w) = &win_ref { let m = w.is_maximized(); w.set_maximized(!m); }
        }
        // Bottom border line
        ui.painter().line_segment(
            [egui::pos2(tb_rect.left(), tb_rect.bottom()), egui::pos2(tb_rect.right(), tb_rect.bottom())],
            egui::Stroke::new(STROKE_STD, t.toolbar_border),
        );

        // Paper-mode bottom line removed — the $ badge in the toolbar (below)
        // is now the canonical "live vs paper" affordance.

        ui.horizontal_centered(|ui| {
            ui.spacing_mut().item_spacing.x = gap_xs();

            // ── Logo (with left edge margin so the glyph doesn't kiss the
            //         window border) ──
            ui.add_space(gap_sm());
            let (logo_rect, _) = ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
            let lp = ui.painter_at(logo_rect);
            let lc = logo_rect.center();
            lp.add(egui::Shape::line(vec![
                egui::pos2(lc.x, lc.y - 6.0), egui::pos2(lc.x + 6.0, lc.y + 5.0),
                egui::pos2(lc.x - 6.0, lc.y + 5.0), egui::pos2(lc.x, lc.y - 6.0),
            ], egui::Stroke::new(STROKE_STD, t.accent)));
            lp.line_segment([egui::pos2(lc.x - 3.5, lc.y + 1.0), egui::pos2(lc.x + 3.5, lc.y + 1.0)], egui::Stroke::new(STROKE_STD, t.accent));

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
                    let col = color_alpha(t.toolbar_border, 80);
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
                    crate::chart_renderer::trading::order_manager::set_paper_mode(!paper);
                }
            }


            ui.add(egui::Separator::default().spacing(4.0));

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

            ui.add(egui::Separator::default().spacing(4.0));

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
            //    hover/active fill comes from `paint_nav_col_tint` (full
            //    toolbar-height column treatment, matching the right-side panel
            //    toggles). Without this, the dropdowns paint *both* treatments.
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

            // ── Interval buttons — favorites segmented control + dropdown caret ──
            // Favorites appear "outside" as quick-access buttons (mirrors layouts).
            // Full timeframe list lives in the dropdown; star toggles favoriting.
            ui.add_space(gap_xs());
            {
                let cur_secs = tf_to_secs(&panes[ap].timeframe);
                // Build favorites list in canonical order from ALL_TIMEFRAMES so
                // the segmented control orders consistently regardless of how
                // the user added them.
                let fav_tfs: Vec<&'static str> = ALL_TIMEFRAMES.iter()
                    .map(|t| t.0)
                    .filter(|tf| watchlist.timeframe_favorites.iter().any(|f| f == tf))
                    .collect();
                if !fav_tfs.is_empty() {
                    let active_idx = fav_tfs.iter().position(|&tf| tf == panes[ap].timeframe).unwrap_or(0);
                    if let Some(i) = segmented_control(ui, active_idx, &fav_tfs, t.toolbar_bg, t.toolbar_border, t.accent, t.dim) {
                        let new_tf = fav_tfs[i];
                        if new_tf != panes[ap].timeframe {
                            let new_secs = tf_to_secs(new_tf);
                            if cur_secs > 0 && new_secs > 0 {
                                let new_vc = ((panes[ap].vc as u64 * cur_secs as u64) / new_secs as u64).max(20).min(2000) as u32;
                                panes[ap].vc = new_vc;
                                panes[ap].vc_target = new_vc;
                            }
                            panes[ap].pending_timeframe_change = Some(new_tf.to_string());
                        }
                    }
                    ui.add_space(gap_xs());
                }
                // Dropdown caret — opens the full timeframe picker with star-favorite toggles.
                let tf_dd_btn = toolbar_btn(ui, Icon::CARET_DOWN, watchlist.timeframe_dropdown_open, t);
                Tooltip::new("Timeframe picker").show(ui, &tf_dd_btn, t);
                if tf_dd_btn.clicked() {
                    watchlist.timeframe_dropdown_open = !watchlist.timeframe_dropdown_open;
                    watchlist.timeframe_dropdown_pos = egui::pos2(tf_dd_btn.rect.left(), tf_dd_btn.rect.bottom() + 2.0);
                }
            }
            ui.add_space(gap_xs());
            // ── Range dropdown (sets interval + visible bars) ──
            {
                let range_label: String = {
                    let presets: &[(&str, &str, u32)] = &[
                        ("1D", "5m", 78), ("2D", "5m", 156), ("3D", "5m", 234),
                        ("5D", "15m", 130), ("2W", "30m", 130), ("1M", "1h", 130),
                        ("3M", "1d", 63), ("1Y", "1d", 252),
                    ];
                    presets.iter()
                        .find(|&&(_, tf, vc)| tf == panes[ap].timeframe && vc == panes[ap].vc)
                        .map(|&(label, _, _)| label.to_string())
                        .unwrap_or_else(|| panes[ap].timeframe.clone())
                };
                let range_resp = KitButton::menu(range_label.as_str()).show_menu(ui, t, |ui| {
                    ui.style_mut().visuals.widgets.inactive.bg_fill = t.toolbar_bg;
                    ui.style_mut().visuals.window_fill = t.toolbar_bg;
                    ui.label(egui::RichText::new("RANGE").monospace().size(font_sm()).color(color_dim(t.dim)));
                    let presets: &[(&str, &str, u32)] = &[
                        ("1 Day",    "5m",  78),
                        ("2 Days",   "5m",  156),
                        ("3 Days",   "5m",  234),
                        ("5 Days",   "15m", 130),
                        ("2 Weeks",  "30m", 130),
                        ("1 Month",  "1h",  130),
                        ("3 Months", "1d",  63),
                        ("1 Year",   "1d",  252),
                    ];
                    for &(label, tf, preset_vc) in presets {
                        if ui.add(SelectableRow::new(label, false)).clicked() {
                            panes[ap].pending_timeframe_change = Some(tf.to_string());
                            panes[ap].vc = preset_vc;
                            panes[ap].vc_target = preset_vc;
                            ui.close_menu();
                        }
                    }
                });
                paint_nav_col_tint(ui, tb_rect, range_resp.response.rect, t,
                    range_resp.response.hovered(), false, "range");
                {
                    use crate::ui_kit::widgets::Tooltip;
                    Tooltip::rich(|ui, theme| {
                        ui.label(egui::RichText::new("Range").size(font_sm()).strong().color(theme.text()));
                        ui.label(egui::RichText::new("Quick presets (1D, 2D, 1M, …)").size(font_xs()).color(theme.dim()));
                    }).show(ui, &range_resp.response, t);
                }
                if range_resp.response.clicked() { TB_BTN_CLICKED.with(|f| f.set(true)); }
            }

            ui.add(egui::Separator::default().spacing(4.0));

            // ── Draw dropdown + magnet + count ──
            {
                let draw_label = match panes[ap].draw_tool.as_str() {
                    "trendline" => "Trend", "hline" => "HLine", "hzone" => "Zone",
                    "fibonacci" => "Fib", "channel" => "Chan", "ray" => "Ray",
                    "vline" => "VLine", "pitchfork" => "Fork", "fibext" => "FibX",
                    "fibchannel" => "FibCh", "gannfan" => "Gann", "gannbox" => "GBox",
                    "textnote" => "Text", "pricerange" => "Range", "riskreward" => "R/R",
                    "fibtimezone" => "FibT", "fibarc" => "FibA", "regression" => "Reg",
                    "xabcd" => "XABCD", "barmarker" => "Mark",
                    s if s.starts_with("elliott") => "Wave",
                    _ => Icon::PENCIL_LINE,
                };
                let has_tool = !panes[ap].draw_tool.is_empty();
                let cur_tool = panes[ap].draw_tool.clone();
                let mut new_tool: Option<String> = None;
                let drawing_menu = KitButton::menu(draw_label)
                    .glyph_size(font_lg())
                    .fg(if has_tool { t.accent } else { t.dim })
                    .show_menu(ui, t, |ui| {
                    ui.style_mut().visuals.widgets.inactive.bg_fill = t.toolbar_bg;
                    ui.style_mut().visuals.window_fill = t.toolbar_bg;
                    let cur = cur_tool.as_str();
                    let sections: &[(&str, &[(&str, &str)])] = &[
                        ("LINES", &[("trendline", "Trendline"), ("hline", "Horizontal Line"), ("vline", "Vertical Line"), ("ray", "Ray")]),
                        ("CHANNELS", &[("channel", "Parallel Channel"), ("fibchannel", "Fib Channel"), ("pitchfork", "Pitchfork")]),
                        ("FIBONACCI", &[("fibonacci", "Fib Retracement"), ("fibext", "Fib Extension"), ("fibtimezone", "Fib Time Zones"), ("fibarc", "Fib Arcs")]),
                        ("GANN", &[("gannfan", "Gann Fan"), ("gannbox", "Gann Box")]),
                        ("RANGES", &[("hzone", "Zone / Rectangle"), ("pricerange", "Price Range"), ("riskreward", "Risk / Reward")]),
                        ("COMPUTED", &[("regression", "Regression Channel"), ("avwap", "Anchored VWAP")]),
                        ("PATTERNS", &[("xabcd", "XABCD Harmonic"), ("elliott_impulse", "Elliott Impulse"), ("elliott_corrective", "Elliott ABC"),
                            ("elliott_wxy", "Elliott WXY"), ("elliott_sub_impulse", "Sub Impulse"), ("elliott_sub_corrective", "Sub Corrective")]),
                        ("OTHER", &[("barmarker", "Bar Marker"), ("textnote", "Text Note")]),
                    ];
                    // Build tool→shortcut lookup from hotkeys
                    let tool_shortcut = |tool_name: &str| -> Option<String> {
                        let action = format!("tool_{}", tool_name);
                        watchlist.hotkeys.iter().find(|hk| hk.action == action).map(|hk| hk.key_name.clone())
                    };
                    for (si, (section, tools)) in sections.iter().enumerate() {
                        if si > 0 { ui.separator(); }
                        ui.label(egui::RichText::new(*section).monospace().size(font_sm()).color(t.dim));
                        for (tool, label) in *tools {
                            let shortcut = tool_shortcut(tool);
                            let resp = ui.horizontal(|ui| {
                                let r = ui.add(SelectableRow::new(label, cur == *tool));
                                if let Some(ref key) = shortcut {
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        crate::ui_kit::widgets::Kbd::new(key.clone()).show(ui, t);
                                    });
                                }
                                r
                            });
                            if resp.inner.clicked() {
                                new_tool = Some(tool.to_string());
                            }
                        }
                    }
                    if !cur.is_empty() {
                        ui.separator();
                        if ui.add(SelectableRow::new("Cancel Tool", false)).clicked() {
                            new_tool = Some(String::new());
                        }
                    }
                });
                paint_nav_col_tint(ui, tb_rect, drawing_menu.response.rect, t,
                    drawing_menu.response.hovered(), has_tool, "drawing");
                {
                    use crate::ui_kit::widgets::Tooltip;
                    Tooltip::rich(|ui, theme| {
                        ui.label(egui::RichText::new("Drawing Tools").size(font_sm()).strong().color(theme.text()));
                        ui.label(egui::RichText::new("Lines, channels, fibs, patterns").size(font_xs()).color(theme.dim()));
                    }).show(ui, &drawing_menu.response, t);
                }
                if let Some(tool) = new_tool {
                    panes[ap].draw_tool = tool;
                    panes[ap].pending_pt = None; panes[ap].pending_pt2 = None; panes[ap].pending_pts.clear();
                }
                TB_BTN_CLICKED.with(|f| f.set(true));
            }
            // ── Drawing-section toggles — same padding/spacing as the rest of the toolbar ──
            {
                let prev_sp = ui.spacing().item_spacing.x;
                let prev_pad = ui.spacing().button_padding;
                ui.spacing_mut().item_spacing.x = gap_xs();
                ui.spacing_mut().button_padding = egui::vec2(gap_sm(), gap_sm());

                // Magnet snap
                let r = toolbar_btn(ui, Icon::MAGNET, panes[ap].magnet, t);
                Tooltip::new("Magnet Snap").show(ui, &r, t);
                if r.clicked() {
                    panes[ap].magnet = !panes[ap].magnet;
                }

                // Object tree — icon button with a count badge painted in the top-right corner
                {
                    let draw_count = panes[ap].drawings.len();
                    let tree_resp = toolbar_btn(ui, Icon::TREE_STRUCTURE, watchlist.object_tree_open, t);
                    Tooltip::new("Object Tree").show(ui, &tree_resp, t);
                    if draw_count > 0 {
                        let painter = ui.painter();
                        let r = tree_resp.rect;
                        let badge_center = egui::pos2(r.right() - 2.0, r.top() + 3.0);
                        let badge_r = 5.0_f32;
                        painter.circle_filled(badge_center, badge_r, t.accent);
                        painter.text(
                            badge_center,
                            egui::Align2::CENTER_CENTER,
                            draw_count.to_string(),
                            egui::FontId::proportional(font_4xs()),
                            contrast_fg(t.accent),
                        );
                    }
                    if tree_resp.clicked() {
                        watchlist.update_sidebar_state(|s| s.object_tree_open = !s.object_tree_open);
                    }
                }

                // Broadcast
                {
                    let bc = watchlist.broadcast_mode;
                    let r = toolbar_btn(ui, Icon::BROADCAST, bc, t);
                    Tooltip::new("Broadcast — changes apply to all panes").show(ui, &r, t);
                    if r.clicked() {
                        watchlist.broadcast_mode = !watchlist.broadcast_mode;
                        TB_BTN_CLICKED.with(|f| f.set(true));
                    }
                }

                // Trendline filter
                let r = toolbar_btn(ui, Icon::FUNNEL, watchlist.trendline_filter_open, t);
                Tooltip::new("Trendline Filter").show(ui, &r, t);
                if r.clicked() {
                    watchlist.update_sidebar_state(|s| s.trendline_filter_open = !s.trendline_filter_open);
                }

                ui.spacing_mut().item_spacing.x = prev_sp;
                ui.spacing_mut().button_padding = prev_pad;
            }

            ui.add(egui::Separator::default().spacing(4.0));

            // ── Organized dropdown menus ──
            let _menu_font = mono_sm();

            // Chart Type dropdown (single-select)
            let cm_label = match panes[ap].candle_mode {
                CandleMode::Standard => "STD", CandleMode::Violin => "VLN",
                CandleMode::Gradient => "GRD", CandleMode::ViolinGradient => "V+G",
                CandleMode::HeikinAshi => "HA", CandleMode::Line => "LN", CandleMode::Area => "AR",
                CandleMode::Renko => "RNK", CandleMode::RangeBar => "RNG", CandleMode::TickBar => "TCK",
            };
            let prev_candle_mode = panes[ap].candle_mode;
            let mode_menu = KitButton::menu(cm_label).show_menu(ui, t, |ui| {
                ui.style_mut().visuals.widgets.inactive.bg_fill = t.toolbar_bg;
                ui.style_mut().visuals.window_fill = t.toolbar_bg;
                for (mode, label) in [
                    (CandleMode::Standard, "Candlestick"), (CandleMode::HeikinAshi, "Heikin Ashi"),
                    (CandleMode::Line, "Line"), (CandleMode::Area, "Area"),
                    (CandleMode::Violin, "Violin"), (CandleMode::Gradient, "Gradient"),
                    (CandleMode::ViolinGradient, "Violin + Gradient"),
                    (CandleMode::Renko, "Renko"), (CandleMode::RangeBar, "Range Bars"),
                    (CandleMode::TickBar, "Tick Bars"),
                ] {
                    let active = panes[ap].candle_mode == mode;
                    if ui.add(SelectableRow::new(label, active)).clicked() {
                        panes[ap].candle_mode = mode;
                    }
                }
                ui.separator();
                let log = panes[ap].log_scale;
                if ui.add(SelectableRow::new("Log Scale", log)).clicked() {
                    let shift = ui.input(|i| i.modifiers.shift); let nv = !log;
                    let fan = shift || watchlist.broadcast_mode;
                    panes[ap].log_scale = nv;
                    publish_toggle(watchlist, fan, PaneToggle::LogScale, nv, ap);
                }
            });
            paint_nav_col_tint(ui, tb_rect, mode_menu.response.rect, t,
                mode_menu.response.hovered(), false, "candle_mode");
            {
                use crate::ui_kit::widgets::Tooltip;
                Tooltip::rich(|ui, theme| {
                    ui.label(egui::RichText::new("Candle Mode").size(font_sm()).strong().color(theme.text()));
                    ui.label(egui::RichText::new("Standard, Heikin Ashi, Renko, … + log scale").size(font_xs()).color(theme.dim()));
                }).show(ui, &mode_menu.response, t);
            }
            // Mark alt bars dirty when candle mode changes
            if panes[ap].candle_mode != prev_candle_mode {
                panes[ap].alt_bars_dirty = true;
                panes[ap].indicator_bar_count = 0; // force indicator recompute
            }
            // Alt chart type settings row
            match panes[ap].candle_mode {
                CandleMode::Renko => {
                    let is_auto = panes[ap].renko_brick_size == 0.0;
                    let auto_label = if is_auto { "Auto" } else { "Manual" };
                    if KitButton::new(auto_label).variant(KitVariant::Ghost).size(KitSize::Sm)
                        .fg(if is_auto { t.accent } else { t.dim }).frameless(true)
                        .min_size(egui::vec2(32.0, 16.0)).show(ui, t).clicked() {
                        if is_auto {
                            panes[ap].renko_brick_size = Chart::auto_brick_size(&panes[ap].bars, 0.5);
                        } else {
                            panes[ap].renko_brick_size = 0.0;
                        }
                        panes[ap].alt_bars_dirty = true;
                    }
                    if !is_auto {
                        let mut val = panes[ap].renko_brick_size;
                        let resp = NumberStepper::new(&mut val).step(0.01).range(0.01..=10000.0).decimals(2).prefix("Brick: ").show(ui, t);
                        if resp.changed() {
                            panes[ap].renko_brick_size = val;
                            panes[ap].alt_bars_dirty = true;
                        }
                    }
                }
                CandleMode::RangeBar => {
                    let is_auto = panes[ap].range_bar_size == 0.0;
                    let auto_label = if is_auto { "Auto" } else { "Manual" };
                    if KitButton::new(auto_label).variant(KitVariant::Ghost).size(KitSize::Sm)
                        .fg(if is_auto { t.accent } else { t.dim }).frameless(true)
                        .min_size(egui::vec2(32.0, 16.0)).show(ui, t).clicked() {
                        if is_auto {
                            panes[ap].range_bar_size = Chart::auto_brick_size(&panes[ap].bars, 1.0);
                        } else {
                            panes[ap].range_bar_size = 0.0;
                        }
                        panes[ap].alt_bars_dirty = true;
                    }
                    if !is_auto {
                        let mut val = panes[ap].range_bar_size;
                        let resp = NumberStepper::new(&mut val).step(0.01).range(0.01..=10000.0).decimals(2).prefix("Range: ").show(ui, t);
                        if resp.changed() {
                            panes[ap].range_bar_size = val;
                            panes[ap].alt_bars_dirty = true;
                        }
                    }
                }
                CandleMode::TickBar => {
                    let mut val = panes[ap].tick_bar_count as i32;
                    let resp = NumberStepper::new(&mut val).step(10.0).range(1..=100000).prefix("Ticks: ").integer().show(ui, t);
                    if resp.changed() {
                        panes[ap].tick_bar_count = val.max(1) as u32;
                        panes[ap].alt_bars_dirty = true;
                    }
                }
                _ => {}
            }

            // ── Indicators dropdown — single chart-icon entry point with nested
            //    submenus for MAs / Oscillators / Volume / Overlays / Tools / Suites.
            let indicators_menu = KitButton::menu(Icon::CHART_LINE)
                .glyph_size(font_lg())
                .show_menu(ui, t, |ui| {
                ui.style_mut().visuals.widgets.inactive.bg_fill = t.toolbar_bg;
                ui.style_mut().visuals.window_fill = t.toolbar_bg;

            // Moving Averages dropdown (always creates new instance — supports multiple)
            KitButton::menu("MAs").show_menu(ui, t, |ui| {
                ui.style_mut().visuals.widgets.inactive.bg_fill = t.toolbar_bg;
                ui.style_mut().visuals.window_fill = t.toolbar_bg;
                let ma_types = [(IndicatorType::SMA, "SMA"), (IndicatorType::EMA, "EMA"), (IndicatorType::WMA, "WMA"),
                    (IndicatorType::DEMA, "DEMA"), (IndicatorType::TEMA, "TEMA"), (IndicatorType::VWAP, "VWAP")];
                // Show existing MA instances with edit/remove
                let existing: Vec<(u32, IndicatorType, usize, String, bool)> = panes[ap].indicators.iter()
                    .filter(|i| i.kind.category() == IndicatorCategory::Overlay && ma_types.iter().any(|(t,_)| *t == i.kind))
                    .map(|i| (i.id, i.kind, i.period, i.color.clone(), i.visible))
                    .collect();
                if !existing.is_empty() {
                    for (eid, ekind, eperiod, ecolor, evis) in &existing {
                        let label_text = format!("{} {}", ekind.label(), eperiod);
                        let c = hex_to_color(ecolor, 1.0);
                        ui.horizontal(|ui| {
                            ui.painter().circle_filled(egui::pos2(ui.cursor().min.x + 5.0, ui.cursor().min.y + 9.0), 3.0, c);
                            ui.add_space(gap_xl());
                            if ui.add(SelectableRow::new(&label_text, *evis)).clicked() {
                                let shift = ui.input(|i| i.modifiers.shift);
                                let nv = !*evis;
                                let fan = shift || watchlist.broadcast_mode;
                                if fan {
                                    // Originator: flip every matching (kind, period) instance.
                                    if let Some(ind) = panes[ap].indicators.iter_mut().find(|i| i.kind == *ekind && i.period == *eperiod) { ind.visible = nv; }
                                    watchlist.subscriptions.publish_from(
                                        PaneEvent::IndicatorVisibilityChanged { group: BROADCAST_GROUP, kind: *ekind, visible: nv },
                                        ap,
                                    );
                                } else {
                                    if let Some(ind) = panes[ap].indicators.iter_mut().find(|i| i.id == *eid) { ind.visible = nv; }
                                }
                            }
                            let r = KitButton::icon(Icon::PENCIL_LINE).variant(KitVariant::MutedIcon).placement(IconPlacement::Toolbar).show(ui, t);
                            Tooltip::new("Edit indicator").show(ui, &r, t);
                            if r.clicked() {
                                panes[ap].editing_indicator = Some(*eid);
                            }
                            let r = KitButton::icon(Icon::X).variant(KitVariant::MutedIcon).placement(IconPlacement::Toolbar).tone_destructive().show(ui, t);
                            Tooltip::new("Remove indicator").show(ui, &r, t);
                            if r.clicked() {
                                let shift = ui.input(|i| i.modifiers.shift);
                                let fan = shift || watchlist.broadcast_mode;
                                if fan {
                                    // Originator: apply the same (kind, period) predicate the dispatcher uses on siblings.
                                    panes[ap].indicators.retain(|i| !(i.kind == *ekind && i.period == *eperiod));
                                    panes[ap].indicator_bar_count = 0;
                                    watchlist.subscriptions.publish_from(
                                        PaneEvent::IndicatorsRemoved { group: BROADCAST_GROUP, kind: *ekind, period: Some(*eperiod) },
                                        ap,
                                    );
                                } else {
                                    panes[ap].indicators.retain(|i| i.id != *eid);
                                    panes[ap].indicator_bar_count = 0;
                                }
                            }
                        });
                    }
                    ui.separator();
                }
                // Add new MA instance
                for (itype, label) in ma_types {
                    if ui.add(SelectableRow::new(label, false).leading_icon(Icon::PLUS)).clicked() {
                        let shift = ui.input(|i| i.modifiers.shift);
                        let fan = shift || watchlist.broadcast_mode;
                        // Originator: allocate id from its own counter, push, reset bar count.
                        let id = panes[ap].next_indicator_id; panes[ap].next_indicator_id += 1;
                        let color_owned = indicator_default_color(panes[ap].indicators.len(), t);
                        let new_ind = Indicator::new(id, itype, itype.default_period(), &color_owned);
                        panes[ap].indicators.push(new_ind.clone());
                        panes[ap].indicator_bar_count = 0;
                        panes[ap].editing_indicator = Some(id);
                        if fan {
                            // Sibling panes get a clone with fresh per-pane id allocated
                            // by the dispatcher from each sibling's `next_indicator_id`.
                            watchlist.subscriptions.publish_from(
                                PaneEvent::IndicatorAdded { group: BROADCAST_GROUP, indicator: new_ind },
                                ap,
                            );
                        }
                    }
                }
                ui.separator();
                let ribbon_active = panes[ap].show_ma_ribbon;
                if ui.add(SelectableRow::new("MA Ribbon (8-89)", ribbon_active)).clicked() {
                    let shift = ui.input(|i| i.modifiers.shift);
                    let nv = !ribbon_active;
                    let fan = shift || watchlist.broadcast_mode;
                    panes[ap].show_ma_ribbon = nv;
                    publish_toggle(watchlist, fan, PaneToggle::ShowMaRibbon, nv, ap);
                }
            });

            // Oscillators dropdown (multi-select)
            KitButton::menu("Osc").show_menu(ui, t, |ui| {
                ui.style_mut().visuals.widgets.inactive.bg_fill = t.toolbar_bg;
                ui.style_mut().visuals.window_fill = t.toolbar_bg;
                let osc_types = [(IndicatorType::RSI, "RSI"), (IndicatorType::MACD, "MACD"),
                    (IndicatorType::Stochastic, "Stochastic"), (IndicatorType::CCI, "CCI"),
                    (IndicatorType::WilliamsR, "Williams %R"), (IndicatorType::ADX, "ADX"), (IndicatorType::ATR, "ATR")];
                for (itype, label) in osc_types {
                    let has = panes[ap].indicators.iter().any(|i| i.kind == itype && i.visible);
                    if ui.add(SelectableRow::new(label, has)).clicked() {
                        let shift = ui.input(|i| i.modifiers.shift);
                        let fan = shift || watchlist.broadcast_mode;
                        // Resolve the originator's mutation first. Three sub-cases:
                        //   (a) `has`: flip visible→false on the first matching instance.
                        //   (b) `!has` + instance exists: flip visible→true.
                        //   (c) `!has` + no instance: push a brand-new one.
                        // Compute which sub-case to publish for sibling fan-out.
                        enum Sub { Vis(bool), Add(Indicator) }
                        let sub = if has {
                            if let Some(ind) = panes[ap].indicators.iter_mut().find(|i| i.kind == itype) { ind.visible = false; }
                            Sub::Vis(false)
                        } else if panes[ap].indicators.iter().any(|i| i.kind == itype) {
                            if let Some(ind) = panes[ap].indicators.iter_mut().find(|i| i.kind == itype) { ind.visible = true; }
                            Sub::Vis(true)
                        } else {
                            let id = panes[ap].next_indicator_id; panes[ap].next_indicator_id += 1;
                            let color_owned = indicator_default_color(panes[ap].indicators.len(), t);
                            let new_ind = Indicator::new(id, itype, itype.default_period(), &color_owned);
                            panes[ap].indicators.push(new_ind.clone());
                            panes[ap].indicator_bar_count = 0;
                            Sub::Add(new_ind)
                        };
                        if fan {
                            match sub {
                                Sub::Vis(v) => {
                                    watchlist.subscriptions.publish_from(
                                        PaneEvent::IndicatorVisibilityChanged { group: BROADCAST_GROUP, kind: itype, visible: v },
                                        ap,
                                    );
                                }
                                Sub::Add(ind) => {
                                    // Siblings without an instance of this kind get a clone;
                                    // siblings that already have one keep their existing
                                    // (potentially configured) instance — the original loop
                                    // only push'd when `!p_has`. To reproduce, flip visible
                                    // on those that have it, then ask the dispatcher to add
                                    // for the rest. The dispatcher unconditionally adds, so
                                    // we publish IndicatorAdded; siblings that already had
                                    // an instance will end up with two — matching the
                                    // legacy guard `else if !p_has` is not reproducible with
                                    // a single event variant. Publishing both keeps the
                                    // most common case (sibling matches originator state)
                                    // correct: when siblings track in lock-step, they
                                    // never had an instance either.
                                    watchlist.subscriptions.publish_from(
                                        PaneEvent::IndicatorVisibilityChanged { group: BROADCAST_GROUP, kind: itype, visible: true },
                                        ap,
                                    );
                                    watchlist.subscriptions.publish_from(
                                        PaneEvent::IndicatorAdded { group: BROADCAST_GROUP, indicator: ind },
                                        ap,
                                    );
                                }
                            }
                        }
                    }
                }
                ui.separator();
                let cvd = panes[ap].show_cvd;
                if ui.add(SelectableRow::new("CVD", cvd)).clicked() {
                    let shift = ui.input(|i| i.modifiers.shift);
                    let nv = !cvd;
                    let fan = shift || watchlist.broadcast_mode;
                    panes[ap].show_cvd = nv;
                    publish_toggle(watchlist, fan, PaneToggle::ShowCvd, nv, ap);
                }
            });

            // Volume dropdown
            KitButton::menu("Vol").show_menu(ui, t, |ui| {
                ui.style_mut().visuals.widgets.inactive.bg_fill = t.toolbar_bg;
                ui.style_mut().visuals.window_fill = t.toolbar_bg;
                let vol = panes[ap].show_volume;
                if ui.add(SelectableRow::new("Volume Bars", vol)).clicked() {
                    let shift = ui.input(|i| i.modifiers.shift); let nv = !vol;
                    let fan = shift || watchlist.broadcast_mode;
                    panes[ap].show_volume = nv;
                    publish_toggle(watchlist, fan, PaneToggle::ShowVolume, nv, ap);
                }
                let dvol = panes[ap].show_delta_volume;
                if ui.add(SelectableRow::new("Delta Volume", dvol)).clicked() {
                    let shift = ui.input(|i| i.modifiers.shift); let nv = !dvol;
                    let fan = shift || watchlist.broadcast_mode;
                    panes[ap].show_delta_volume = nv;
                    publish_toggle(watchlist, fan, PaneToggle::ShowDeltaVolume, nv, ap);
                }
                let rvol = panes[ap].show_rvol;
                if ui.add(SelectableRow::new("Relative Volume", rvol)).clicked() {
                    let shift = ui.input(|i| i.modifiers.shift); let nv = !rvol;
                    let fan = shift || watchlist.broadcast_mode;
                    panes[ap].show_rvol = nv;
                    publish_toggle(watchlist, fan, PaneToggle::ShowRvol, nv, ap);
                }
                ui.separator();
                ui.label(egui::RichText::new("Volume Profile").monospace().size(font_sm()).color(t.dim));
                for (mode, label) in [
                    (VolumeProfileMode::Off, "Off"), (VolumeProfileMode::Classic, "Classic"),
                    (VolumeProfileMode::Heatmap, "Heatmap"), (VolumeProfileMode::Strip, "Strip"),
                    (VolumeProfileMode::Clean, "Clean (POC/VA)"),
                ] {
                    let active = panes[ap].vp_mode == mode;
                    if ui.add(SelectableRow::new(label, active)).clicked() {
                        panes[ap].vp_mode = mode; panes[ap].vp_data = None;
                    }
                }
            });

            // Overlays dropdown — two-layer with categories
            KitButton::menu("Overlay").show_menu(ui, t, |ui| {
                ui.style_mut().visuals.widgets.inactive.bg_fill = t.toolbar_bg;
                ui.style_mut().visuals.window_fill = t.toolbar_bg;
                ui.set_min_width(150.0);

                // ── Technical Overlays (indicator-based)
                KitButton::menu("Technical").leading_icon(Icon::PULSE).trailing_icon(Icon::CARET_RIGHT).show_menu(ui, t, |ui| {
                    ui.set_min_width(200.0);
                    let overlay_types = [
                        (IndicatorType::BollingerBands, "Bollinger Bands"),
                        (IndicatorType::KeltnerChannels, "Keltner Channels"),
                        (IndicatorType::Ichimoku, "Ichimoku Cloud"),
                        (IndicatorType::Supertrend, "Supertrend"),
                        (IndicatorType::ParabolicSAR, "Parabolic SAR"),
                    ];
                    for (itype, label) in overlay_types {
                        let has = panes[ap].indicators.iter().any(|i| i.kind == itype && i.visible);
                        if ui.add(SelectableRow::new(label, has)).clicked() {
                            if has {
                                if let Some(ind) = panes[ap].indicators.iter_mut().find(|i| i.kind == itype) { ind.visible = false; }
                            } else {
                                if let Some(ind) = panes[ap].indicators.iter_mut().find(|i| i.kind == itype) { ind.visible = true; }
                                else {
                                    let id = panes[ap].next_indicator_id; panes[ap].next_indicator_id += 1;
                                    let color_owned = indicator_default_color(panes[ap].indicators.len(), t);
                                    panes[ap].indicators.push(Indicator::new(id, itype, itype.default_period(), &color_owned));
                                    panes[ap].indicator_bar_count = 0;
                                }
                            }
                        }
                    }
                    ui.separator();
                    let vwap = panes[ap].show_vwap_bands;
                    if ui.add(SelectableRow::new("VWAP + Bands", vwap)).clicked() {
                        panes[ap].show_vwap_bands = !panes[ap].show_vwap_bands;
                    }
                    let sr = panes[ap].show_auto_sr;
                    if ui.add(SelectableRow::new("Auto S/R Levels", sr)).clicked() {
                        panes[ap].show_auto_sr = !panes[ap].show_auto_sr;
                    }
                });

                // ── Structure (S/R, volume, price levels)
                KitButton::menu("Structure").leading_icon(Icon::TREE_STRUCTURE_FILL).trailing_icon(Icon::CARET_RIGHT).show_menu(ui, t, |ui| {
                    ui.set_min_width(220.0);
                    macro_rules! overlay_toggle {
                        ($field:ident, $label:expr) => {
                            let v = panes[ap].$field;
                            if ui.add(SelectableRow::new($label, v)).clicked() {
                                panes[ap].$field = !v;
                            }
                        }
                    }
                    overlay_toggle!(show_vol_shelves, "Volume Shelves");
                    overlay_toggle!(show_confluence, "S/R Confluence");
                    overlay_toggle!(show_price_memory, "Price Memory");
                    overlay_toggle!(show_liquidity_voids, "Liquidity Voids");
                    ui.separator();
                    overlay_toggle!(show_analyst_targets, "Analyst Targets");
                    overlay_toggle!(show_pe_band, "PE Valuation Band");
                    overlay_toggle!(show_insider_trades, "Insider Trades");
                    ui.separator();
                    let gamma = panes[ap].show_gamma;
                    if ui.add(SelectableRow::new("Gamma Levels (GEX)", gamma)).clicked() {
                        panes[ap].show_gamma = !panes[ap].show_gamma;
                        if panes[ap].show_gamma && panes[ap].gamma_levels.is_empty() {
                            if let Some(last_bar) = panes[ap].bars.last() {
                                let price = last_bar.close;
                                let step = if price > 200.0 { 5.0 } else if price > 50.0 { 2.5 } else { 1.0 };
                                let mut levels = vec![];
                                for i in -15..=15_i32 {
                                    let level_price = (price / step).round() * step + i as f32 * step;
                                    let dist = i.abs() as f32;
                                    let gex = if dist < 5.0 { (500.0 - dist * 80.0) * (1.0 + 0.3 * (level_price * 7.3).sin()) }
                                    else { (-100.0 - (dist - 5.0) * 50.0) * (1.0 + 0.2 * (level_price * 3.1).sin()) };
                                    levels.push((level_price, gex));
                                }
                                let max_pos = levels.iter().filter(|(_, g)| *g > 0.0).max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
                                let max_neg = levels.iter().filter(|(_, g)| *g < 0.0).min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
                                panes[ap].gamma_call_wall = max_pos.map_or(price + 10.0 * step, |l| l.0);
                                panes[ap].gamma_put_wall = max_neg.map_or(price - 10.0 * step, |l| l.0);
                                let mut zero = price;
                                for w in levels.windows(2) { if w[0].1 >= 0.0 && w[1].1 < 0.0 { zero = (w[0].0 + w[1].0) / 2.0; break; } }
                                panes[ap].gamma_zero = zero;
                                panes[ap].gamma_hvl = max_pos.map_or(price, |l| l.0);
                                panes[ap].gamma_levels = levels;
                            }
                        }
                    }
                });

                // ── Regime (momentum, volatility, correlation)
                KitButton::menu("Regime").leading_icon(Icon::BROADCAST_FILL).trailing_icon(Icon::CARET_RIGHT).show_menu(ui, t, |ui| {
                    ui.set_min_width(220.0);
                    macro_rules! overlay_toggle {
                        ($field:ident, $label:expr) => {
                            let v = panes[ap].$field;
                            if ui.add(SelectableRow::new($label, v)).clicked() {
                                panes[ap].$field = !v;
                            }
                        }
                    }
                    overlay_toggle!(show_momentum_heat, "Momentum Heatmap");
                    overlay_toggle!(show_trend_strip, "Trend Alignment Strip");
                    overlay_toggle!(show_breadth_tint, "Breadth Tint");
                    overlay_toggle!(show_vol_cone, "Volatility Cone");
                    overlay_toggle!(show_corr_ribbon, "Correlation Ribbon");
                });

                // ── Data (events, dark pool, etc.)
                KitButton::menu("Data").leading_icon(Icon::CHART_LINE_UP_FILL).trailing_icon(Icon::CARET_RIGHT).show_menu(ui, t, |ui| {
                    ui.set_min_width(200.0);
                    let events = panes[ap].show_events;
                    if ui.add(SelectableRow::new("Event Markers", events)).clicked() {
                        panes[ap].show_events = !panes[ap].show_events;
                        if panes[ap].show_events && panes[ap].event_markers.is_empty() && !panes[ap].timestamps.is_empty() {
                            let ts = &panes[ap].timestamps;
                            let n = ts.len();
                            let mut markers = vec![];
                            let mut i = 30;
                            while i < n { markers.push(EventMarker { time: ts[i], event_type: 0, label: format!("Q{} Earnings", (i/60)%4+1), details: String::new(), impact: if i%3==0{1}else if i%3==1{-1}else{0} }); i += 60; }
                            i = 45; let mut ei = 0;
                            let econ = ["FOMC","CPI","NFP","PPI"];
                            while i < n { markers.push(EventMarker { time: ts[i], event_type: 3, label: econ[ei%4].into(), details: String::new(), impact: 0 }); i += 90; ei += 1; }
                            markers.sort_by_key(|m| m.time);
                            panes[ap].event_markers = markers;
                        }
                    }
                    let dp = panes[ap].show_darkpool;
                    if ui.add(SelectableRow::new("Dark Pool Prints", dp)).clicked() {
                        panes[ap].show_darkpool = !panes[ap].show_darkpool;
                        if panes[ap].show_darkpool && panes[ap].darkpool_prints.is_empty() {
                            if let Some(last_bar) = panes[ap].bars.last() {
                                let price = last_bar.close; let bar_count = panes[ap].bars.len(); let ts_len = panes[ap].timestamps.len();
                                let mut prints = vec![]; let sizes: [u64;6] = [50_000,100_000,150_000,200_000,250_000,500_000];
                                for k in 0..18_u32 {
                                    let seed = (price * 1000.0) as u32 ^ (k * 7919);
                                    let bar_idx = if bar_count > 20 { bar_count - 1 - ((seed as usize) % bar_count.min(60)) } else { (seed as usize) % bar_count.max(1) };
                                    let bar = &panes[ap].bars[bar_idx.min(bar_count-1)];
                                    let offset = (((seed>>4)%100) as f32/100.0-0.5) * (bar.high-bar.low).max(0.01) * 3.0;
                                    let ts = if bar_idx < ts_len { panes[ap].timestamps[bar_idx] } else { 0 };
                                    prints.push(DarkPoolPrint { price: bar.close+offset, size: sizes[(seed as usize)%6], time: ts, side: match seed%3{0=>1_i8,1=>-1,_=>0} });
                                }
                                panes[ap].darkpool_prints = prints;
                            }
                        }
                    }
                });

                ui.separator();
                // Symbol overlays
                ui.label(egui::RichText::new("SYMBOL OVERLAY").monospace().size(font_sm()).color(color_half(t.dim)));
                let mut remove_idx: Option<usize> = None;
                let mut edit_idx: Option<usize> = None;
                for (oi, ov) in panes[ap].symbol_overlays.iter().enumerate() {
                    ui.horizontal(|ui| {
                        let oc = hex_to_color(&ov.color, 1.0);
                        ui.painter().circle_filled(egui::pos2(ui.cursor().min.x + 5.0, ui.cursor().min.y + 9.0), 3.0, oc);
                        ui.add_space(gap_xl());
                        let label_resp = ui.label(egui::RichText::new(&ov.symbol).monospace().size(font_sm()).color(oc));
                        if label_resp.double_clicked() { edit_idx = Some(oi); }
                        let r = KitButton::icon(Icon::X).variant(KitVariant::Ghost).placement(IconPlacement::Toolbar).tone_destructive().show(ui, t);
                        Tooltip::new("Remove overlay").show(ui, &r, t);
                        if r.clicked() {
                            remove_idx = Some(oi);
                        }
                    });
                }
                if let Some(ri) = remove_idx { panes[ap].symbol_overlays.remove(ri); }
                if let Some(ei) = edit_idx {
                    panes[ap].overlay_editing = true;
                    panes[ap].overlay_editing_idx = Some(ei);
                    panes[ap].overlay_input = panes[ap].symbol_overlays[ei].symbol.clone();
                }
                if ui.add(SelectableRow::new("Add Symbol Overlay", false).leading_icon(Icon::PLUS)).clicked() {
                    watchlist.pending_overlay_add = true;
                }
            });

            // Tools dropdown — display tools and cursor enhancements (now nested under Indicators)
            KitButton::menu("Tools").show_menu(ui, t, |ui| {
                ui.style_mut().visuals.widgets.inactive.bg_fill = t.toolbar_bg;
                ui.style_mut().visuals.window_fill = t.toolbar_bg;

                ui.label(egui::RichText::new("DISPLAY").monospace().size(font_sm()).color(color_half(t.dim)));
                let ohlc = panes[ap].ohlc_tooltip;
                if ui.add(SelectableRow::new("OHLC Tooltip", ohlc)).clicked() {
                    let shift = ui.input(|i| i.modifiers.shift); let nv = !ohlc;
                    let fan = shift || watchlist.broadcast_mode;
                    panes[ap].ohlc_tooltip = nv;
                    publish_toggle(watchlist, fan, PaneToggle::OhlcTooltip, nv, ap);
                }
                let mt = panes[ap].measure_tooltip;
                if ui.add(SelectableRow::new("Measure Tooltip", mt)).clicked() {
                    let shift = ui.input(|i| i.modifiers.shift); let nv = !mt;
                    let fan = shift || watchlist.broadcast_mode;
                    panes[ap].measure_tooltip = nv;
                    publish_toggle(watchlist, fan, PaneToggle::MeasureTooltip, nv, ap);
                }
                let pc = panes[ap].show_prev_close;
                if ui.add(SelectableRow::new("Prev Close / Open", pc)).clicked() {
                    let shift = ui.input(|i| i.modifiers.shift); let nv = !pc;
                    let fan = shift || watchlist.broadcast_mode;
                    panes[ap].show_prev_close = nv;
                    publish_toggle(watchlist, fan, PaneToggle::ShowPrevClose, nv, ap);
                }
                let pl = panes[ap].show_pattern_labels;
                if ui.add(SelectableRow::new("Pattern Labels", pl)).clicked() {
                    let shift = ui.input(|i| i.modifiers.shift); let nv = !pl;
                    let fan = shift || watchlist.broadcast_mode;
                    panes[ap].show_pattern_labels = nv;
                    publish_toggle(watchlist, fan, PaneToggle::ShowPatternLabels, nv, ap);
                }
                let pnl = panes[ap].show_pnl_curve;
                if ui.add(SelectableRow::new("P&L Curve", pnl)).clicked() { panes[ap].show_pnl_curve = !panes[ap].show_pnl_curve; }

                ui.separator();
                ui.label(egui::RichText::new("CURSOR").monospace().size(font_sm()).color(color_half(t.dim)));
                let fp = panes[ap].show_footprint;
                if ui.add(SelectableRow::new("Footprint (hover)", fp)).clicked() {
                    let shift = ui.input(|i| i.modifiers.shift); let nv = !fp;
                    let fan = shift || watchlist.broadcast_mode;
                    panes[ap].show_footprint = nv;
                    publish_toggle(watchlist, fan, PaneToggle::ShowFootprint, nv, ap);
                }

                ui.separator();
                ui.label(egui::RichText::new("REPLAY").monospace().size(font_sm()).color(color_half(t.dim)));
                let rpl = panes[ap].replay_mode;
                if ui.add(SelectableRow::new("Bar Replay", rpl)).clicked() {
                    panes[ap].replay_mode = !panes[ap].replay_mode;
                    if panes[ap].replay_mode {
                        panes[ap].replay_bar_count = panes[ap].bars.len().min(50);
                        panes[ap].replay_playing = false;
                        panes[ap].indicator_bar_count = 0;
                    }
                }
            });

            // ── Suites dropdown (advanced analysis tools — also nested under Indicators) ──
            KitButton::menu("Suites").show_menu(ui, t, |ui| {
                ui.style_mut().visuals.widgets.inactive.bg_fill = t.toolbar_bg;
                ui.style_mut().visuals.window_fill = t.toolbar_bg;
                let sl_mode = panes[ap].swing_leg_mode;
                let sl_active = sl_mode > 0;
                let sl_suffix = match sl_mode { 1 => " (Vertical)", 2 => " (Diagonal)", _ => "" };
                if ui.add(SelectableRow::new(&format!("SwingRange{}", sl_suffix), sl_active)).clicked() {
                    let shift = ui.input(|i| i.modifiers.shift); let nv = (sl_mode + 1) % 3;
                    let fan = shift || watchlist.broadcast_mode;
                    panes[ap].swing_leg_mode = nv;
                    publish_swing_leg_mode(watchlist, fan, nv, ap);
                }
                let afib = panes[ap].show_auto_fib;
                if ui.add(SelectableRow::new("Auto Fibonacci", afib)).clicked() {
                    let shift = ui.input(|i| i.modifiers.shift); let nv = !afib;
                    let fan = shift || watchlist.broadcast_mode;
                    panes[ap].show_auto_fib = nv;
                    publish_toggle(watchlist, fan, PaneToggle::ShowAutoFib, nv, ap);
                }
                ui.separator();
                ui.add(SelectableRow::new("Triangulator", false).disabled(true));
                ui.add(SelectableRow::new("Auto Target", false).disabled(true));
            });

            }); // ── end Indicators outer dropdown (wraps MAs/Osc/Vol/Overlay/Tools/Suites) ──
            paint_nav_col_tint(ui, tb_rect, indicators_menu.response.rect, t,
                indicators_menu.response.hovered(), false, "indicators");
            {
                use crate::ui_kit::widgets::Tooltip;
                Tooltip::rich(|ui, theme| {
                    ui.label(egui::RichText::new("Indicators").size(font_sm()).strong().color(theme.text()));
                    ui.label(egui::RichText::new("MAs, Oscillators, Volume, Overlays, Tools, Suites").size(font_xs()).color(theme.dim()));
                }).show(ui, &indicators_menu.response, t);
            }

            // Deferred: open overlay editor after menu closes
            if watchlist.pending_overlay_add {
                watchlist.pending_overlay_add = false;
                panes[ap].overlay_editing = true;
                panes[ap].overlay_editing_idx = None;
            }

            // ── Widgets dropdown — two-layer categorized picker with mini previews ──
            let widgets_menu = KitButton::menu(Icon::CIRCLES_FOUR)
                .glyph_size(font_lg())
                .show_menu(ui, t, |ui| {
                ui.style_mut().visuals.widgets.inactive.bg_fill = t.toolbar_bg;
                ui.style_mut().visuals.window_fill = t.toolbar_bg;
                ui.set_min_width(160.0);
                let active_kinds: Vec<ChartWidgetKind> = panes[ap].chart_widgets.iter()
                    .filter(|w| w.visible).map(|w| w.kind).collect();

                use ChartWidgetKind as W;
                let categories: &[(&str, &str, &[W])] = &[
                    ("Gauges", "\u{25CE}", &[W::TrendStrength, W::Momentum, W::Volatility,
                        W::RsiMulti, W::ConvictionMeter, W::LiquidityScore]),
                    ("Analytics", "\u{2593}", &[W::TrendAlign, W::VolumeShelf, W::Confluence,
                        W::MomentumHeat, W::VolRegime, W::BreadthThermo, W::RelStrength]),
                    ("Market", "\u{2194}", &[W::Correlation, W::DarkPool, W::FlowCompass,
                        W::SectorRotation, W::OptionsSentiment, W::SignalRadar, W::CrossAssetPulse, W::TapeSpeed]),
                    ("Position", "\u{0024}", &[W::PositionPnl, W::PositionsPanel, W::DailyPnl,
                        W::RiskDash, W::RiskReward]),
                    ("Info", "\u{1F4F0}", &[W::VolumeProfile, W::SessionTimer, W::KeyLevels,
                        W::OptionGreeks, W::MarketBreadth, W::EarningsBadge, W::EarningsMom,
                        W::Fundamentals, W::EconCalendar, W::Latency,
                        W::PayoffChart, W::OptionsFlow, W::NewsTicker]),
                    ("Signals", "\u{26A1}", &[W::ExitGauge, W::PrecursorAlert, W::TradePlan,
                        W::ChangePoints, W::ZoneStrength, W::PatternScanner, W::VixMonitor,
                        W::SignalDashboard, W::DivergenceMonitor]),
                ];

                for (cat_name, cat_icon, kinds) in categories {
                    // Category as a submenu — opens a flyout with widget items
                    let active_in_cat = kinds.iter().filter(|k| active_kinds.contains(k)).count();
                    let cat_label = if active_in_cat > 0 {
                        format!("{} {} ({})", cat_icon, cat_name, active_in_cat)
                    } else {
                        format!("{} {}", cat_icon, cat_name)
                    };

                    KitButton::menu(cat_label.as_str())
                        .fg(if active_in_cat > 0 { t.accent } else { t.dim })
                        .show_menu(ui, t, |ui| {
                        ui.set_min_width(280.0);
                        ui.label(egui::RichText::new(*cat_name).monospace().size(font_xs()).strong().color(t.accent));
                        ui.add_space(gap_xs());

                        for &kind in *kinds {
                            let is_active = active_kinds.contains(&kind);
                            let item_h = 36.0;
                            let (_, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), item_h), egui::Sense::click());
                            let r = resp.rect;
                            let p = ui.painter();

                            crate::chart_renderer::ui::style::cursor::clickable(ui, &resp);
                            crate::chart_renderer::ui::style::cursor::focus_ring(ui, &resp, t.accent);
                            if resp.hovered() {
                                p.rect_filled(r, 4.0, color_alpha(t.accent, ALPHA_GHOST));
                            }

                            // Mini preview thumbnail (28x28 painted icon)
                            let preview_rect = egui::Rect::from_min_size(
                                egui::pos2(r.left() + 4.0, r.top() + 4.0), egui::vec2(28.0, 28.0));
                            let preview_bg = color_alpha(t.toolbar_border, ALPHA_FAINT);
                            p.rect_filled(preview_rect, 4.0, preview_bg);
                            paint_widget_preview(p, preview_rect, kind, t, is_active);

                            // Name
                            let name_x = r.left() + 38.0;
                            p.text(egui::pos2(name_x, r.top() + 10.0), egui::Align2::LEFT_CENTER,
                                kind.label(), egui::FontId::monospace(font_sm()),
                                if is_active { t.text } else { t.dim });

                            // Description
                            let desc = widget_description(kind);
                            p.text(egui::pos2(name_x, r.top() + 23.0), egui::Align2::LEFT_CENTER,
                                desc, mono_sm(), color_dim(t.dim));

                            // Active checkmark
                            if is_active {
                                p.text(egui::pos2(r.right() - 12.0, r.center().y),
                                    egui::Align2::CENTER_CENTER, "\u{2713}",
                                    egui::FontId::proportional(font_sm()), t.accent);
                            }

                            if resp.clicked() {
                                if is_active {
                                    panes[ap].chart_widgets.retain(|w| w.kind != kind);
                                } else {
                                    let n = panes[ap].chart_widgets.len();
                                    let x = 0.02 + (n as f32 * 0.05).min(0.5);
                                    let y = 0.05 + (n as f32 * 0.08).min(0.6);
                                    panes[ap].chart_widgets.push(ChartWidget::new(kind, x, y));
                                }
                                ui.close_menu();
                            }
                        }
                    });
                }

                ui.add_space(gap_sm());
                ui.separator();
                if !panes[ap].chart_widgets.is_empty() {
                    if ui.add(SelectableRow::new("Remove All Widgets", false).leading_icon(Icon::TRASH)).clicked() {
                        panes[ap].chart_widgets.clear();
                        ui.close_menu();
                    }
                }
            });
            paint_nav_col_tint(ui, tb_rect, widgets_menu.response.rect, t,
                widgets_menu.response.hovered(), false, "widgets");
            {
                use crate::ui_kit::widgets::Tooltip;
                Tooltip::rich(|ui, theme| {
                    ui.label(egui::RichText::new("Widgets").size(font_sm()).strong().color(theme.text()));
                    ui.label(egui::RichText::new("Add live data tiles to the chart").size(font_xs()).color(theme.dim()));
                }).show(ui, &widgets_menu.response, t);
            }

            // Hit-highlight toggle — trendline/swing hit detection flash
            {
                let hh_resp = toolbar_btn(ui, Icon::LINE_SEGMENT, panes[ap].hit_highlight, t);
                Tooltip::new("Trendline Hit Detection").show(ui, &hh_resp, t);
                if hh_resp.clicked() {
                    let shift = ui.input(|i| i.modifiers.shift);
                    let nv = !panes[ap].hit_highlight;
                    let fan = shift || watchlist.broadcast_mode;
                    panes[ap].hit_highlight = nv;
                    publish_toggle(watchlist, fan, PaneToggle::HitHighlight, nv, ap);
                }
            }

            ui.add(egui::Separator::default().spacing(4.0));

            // ── Workspace — icon-only dropdown (active workspace shown inside the menu) ──
            {
                let ws_names = list_workspaces();
                let ws_menu = KitButton::menu(Icon::BROWSERS)
                    .glyph_size(font_lg())
                    .show_menu(ui, t, |ui| {
                    ui.style_mut().visuals.widgets.inactive.bg_fill = t.toolbar_bg;
                    ui.style_mut().visuals.window_fill = t.toolbar_bg;
                    ui.set_min_width(200.0);

                    ui.label(egui::RichText::new("WORKSPACES").monospace().size(font_xs()).color(color_half(t.dim)));
                    ui.add_space(gap_sm());

                    // Workspace list
                    for name in &ws_names {
                        let is_active = *name == watchlist.active_workspace;
                        ui.horizontal(|ui| {
                            // Active dot
                            if is_active {
                                ui.label(egui::RichText::new(Icon::CIRCLE_FILL).size(font_xs()).color(t.accent));
                            } else {
                                ui.label(egui::RichText::new("  ").size(font_xs()));
                            }
                            if ui.add(SelectableRow::new(name, is_active)).clicked() && !is_active {
                                watchlist.active_workspace = name.clone();
                                watchlist.pending_workspace_load = Some(name.clone());
                                ui.close_menu();
                            }
                        });
                    }

                    ui.add_space(gap_sm());
                    ui.separator();
                    ui.add_space(gap_sm());

                    // Save current
                    if !watchlist.active_workspace.is_empty() {
                        if ui.button(egui::RichText::new(format!("{} Save \"{}\"", Icon::CHECK, watchlist.active_workspace))
                            .monospace().size(font_sm()).color(t.accent)).clicked() {
                            save_workspace(&watchlist.active_workspace, panes, *layout);
                            ui.close_menu();
                        }
                    }

                    // Save as new
                    ui.add_space(gap_sm());
                    ui.horizontal(|ui| {
                        crate::ui_kit::widgets::Input::new(&mut watchlist.workspace_save_name)
                            .placeholder("New workspace…")
                            .min_width(130.0)
                            .size(crate::ui_kit::widgets::Size::Sm)
                            .show(ui, t);
                        let can_save = !watchlist.workspace_save_name.trim().is_empty();
                        if can_save {
                            if KitButton::new("Save As").variant(KitVariant::Primary).size(KitSize::Sm)
                                .tint(t.accent).show(ui, t).clicked() {
                                let name = watchlist.workspace_save_name.trim().to_string();
                                save_workspace(&name, panes, *layout);
                                watchlist.active_workspace = name;
                                watchlist.workspace_save_name.clear();
                                ui.close_menu();
                            }
                        }
                    });

                    // Auto-save info
                    ui.add_space(gap_sm());
                    ui.label(egui::RichText::new("Auto-saves every 30s").monospace().size(font_xs()).color(color_very_dim(t.dim)));
                });
                paint_nav_col_tint(ui, tb_rect, ws_menu.response.rect, t,
                    ws_menu.response.hovered(), false, "workspace");
                {
                    use crate::ui_kit::widgets::Tooltip;
                    let active_ws = watchlist.active_workspace.clone();
                    Tooltip::rich(move |ui, theme| {
                        ui.label(egui::RichText::new("Workspaces").size(font_sm()).strong().color(theme.text()));
                        ui.label(egui::RichText::new(format!("Active: {}", active_ws)).size(font_xs()).color(theme.dim()));
                    }).show(ui, &ws_menu.response, t);
                }
            }

            ui.add(egui::Separator::default().spacing(4.0));

            // ── Layouts — favorites bar + dropdown ──
            // Helper: switch to a layout, creating panes as needed
            let mut switch_layout = |ly: Layout, panes: &mut Vec<Chart>, layout: &mut Layout, active_pane: &mut usize| {
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
                        switch_layout(*fav_layouts[i], panes, layout, active_pane);
                    }
                    ui.add_space(gap_xs());
                }
                // Dropdown caret for the full layout picker
                let dd_btn = toolbar_btn(ui, Icon::CARET_DOWN, watchlist.layout_dropdown_open, t);
                Tooltip::new("Layout picker").show(ui, &dd_btn, t);
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

                // Window control buttons — custom drawn for clean look
                let win_btn = |ui: &mut egui::Ui, danger: bool| -> (egui::Response, egui::Rect) {
                    let (r, resp) = ui.allocate_exact_size(BTN_ICON_LG, egui::Sense::click());
                    if resp.hovered() {
                        let bg = if danger { t.bear } else { color_alpha(t.toolbar_border, 80) };
                        ui.painter().rect_filled(r, 0.0, bg);
                    }
                    crate::chart_renderer::ui::style::cursor::clickable(ui, &resp);
                    if resp.clicked() { TB_BTN_CLICKED.with(|f| f.set(true)); }
                    (resp, r)
                };

                // Close — draw X with lines
                {
                    let (resp, r) = win_btn(ui, true);
                    let c = r.center();
                    let s = 4.5;
                    let col = if resp.hovered() { contrast_fg(t.bear) } else { color_subtle(t.dim) };
                    ui.painter().line_segment([egui::pos2(c.x - s, c.y - s), egui::pos2(c.x + s, c.y + s)], egui::Stroke::new(STROKE_STD, col));
                    ui.painter().line_segment([egui::pos2(c.x + s, c.y - s), egui::pos2(c.x - s, c.y + s)], egui::Stroke::new(STROKE_STD, col));
                    if resp.clicked() {
                        save_state(panes, *layout, watchlist);
                        watchlist.persist();
                        CLOSE_REQUESTED.with(|f| f.set(true));
                    }
                }
                // Maximize — draw square outline (or overlapping squares when maximized)
                {
                    let (resp, r) = win_btn(ui, false);
                    let c = r.center();
                    let s = 4.5;
                    let col = if resp.hovered() { t.dim } else { color_subtle(t.dim) };
                    let is_max = win_ref.as_ref().map_or(false, |w| w.is_maximized());
                    if is_max {
                        // Restore icon: two overlapping squares
                        let o = 1.5;
                        ui.painter().rect_stroke(egui::Rect::from_min_size(egui::pos2(c.x - s + o, c.y - s), egui::vec2(s * 2.0 - o, s * 2.0 - o)), 0.5, egui::Stroke::new(STROKE_STD, col), egui::StrokeKind::Outside);
                        ui.painter().rect_stroke(egui::Rect::from_min_size(egui::pos2(c.x - s, c.y - s + o), egui::vec2(s * 2.0 - o, s * 2.0 - o)), 0.5, egui::Stroke::new(STROKE_STD, col), egui::StrokeKind::Outside);
                    } else {
                        ui.painter().rect_stroke(egui::Rect::from_center_size(c, egui::vec2(s * 2.0, s * 2.0)), 0.5, egui::Stroke::new(STROKE_STD, col), egui::StrokeKind::Outside);
                    }
                    if resp.clicked() {
                        if let Some(w) = &win_ref { let m = w.is_maximized(); w.set_maximized(!m); }
                    }
                }
                // Minimize — draw horizontal line
                {
                    let (resp, r) = win_btn(ui, false);
                    let c = r.center();
                    let s = 5.0;
                    let col = if resp.hovered() { t.dim } else { color_subtle(t.dim) };
                    ui.painter().line_segment([egui::pos2(c.x - s, c.y), egui::pos2(c.x + s, c.y)], egui::Stroke::new(STROKE_STD, col));
                    if resp.clicked() {
                        if let Some(w) = &win_ref { w.set_minimized(true); }
                    }
                }

                // Separator between window controls and panel toggles
                ui.add(egui::Separator::default().spacing(4.0));

                // Panel toggle buttons (right-to-left, so ordered right→left)
                ui.spacing_mut().item_spacing.x = gap_sm();


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
                        ui.label(egui::RichText::new(&tip_detail_clone).size(font_xs()).color(theme.text()));
                        ui.label(egui::RichText::new("Click to open Connection panel").size(font_xs()).color(theme.dim()));
                    }).show(ui, &resp, t);
                    if resp.clicked() { *conn_panel_open = !*conn_panel_open; }
                }

                // Style-aware label helper for nav buttons that have a text label.
                // Meridien hides icons and uppercases labels; other styles keep "{ICON} Label".
                // Icon-only buttons (Settings, etc.) are NOT affected — they keep their icon
                // glyph under all styles.
                let st = style_current();
                let nav_label = |icon: &str, label: &str| -> String {
                    let txt = if st.nav_buttons_uppercase_labels { label.to_uppercase() } else { label.to_string() };
                    if st.nav_buttons_label_only { txt } else { format!("{} {}", icon, txt) }
                };

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
                    Tooltip::new("Settings (Cmd+,)").show(ui, &settings_resp, t);
                    paint_nav_col_tint(ui, tb_rect, settings_resp.rect, t, settings_resp.hovered(), watchlist.settings_open, "right_settings");
                    if settings_resp.clicked() { watchlist.update_sidebar_state(|s| s.settings_open = !s.settings_open); }
                }

                // Search / command palette — icon-only ToolbarBtn.
                {
                    use crate::ui_kit::widgets::{Tooltip, Kbd};
                    let search_resp = toolbar_btn(ui, Icon::MAGNIFYING_GLASS, watchlist.cmd_palette_open, t);
                    paint_nav_col_tint(ui, tb_rect, search_resp.rect, t, search_resp.hovered(), watchlist.cmd_palette_open, "right_search");
                    Tooltip::rich(|ui, theme| {
                        ui.label(egui::RichText::new("Search").size(font_sm()).strong().color(theme.text()));
                        ui.label(egui::RichText::new("Search & command palette").size(font_xs()).color(theme.dim()));
                        ui.add(Kbd::new("Cmd+K"));
                    }).show(ui, &search_resp, t);
                    if search_resp.clicked() {
                        watchlist.cmd_palette_open = !watchlist.cmd_palette_open;
                    }
                }

                ui.add(egui::Separator::default().spacing(4.0));

                // ── Right nav panel toggles — zero item spacing, second-smallest
                //    inner button padding, hairline dividers between each. ──
                let prev_spacing = ui.spacing().item_spacing.x;
                let prev_panel_pad = ui.spacing().button_padding;
                // 8px gap between buttons (4px on each side of the divider).
                ui.spacing_mut().item_spacing.x = gap_sm();
                // gap_lg horizontal padding so labels breathe and the hover
                // column has visible margin on either side of the text.
                ui.spacing_mut().button_padding = egui::vec2(gap_lg(), gap_sm());

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
                        let x = ($resp.rect.left() - 4.0).round() + 0.5;
                        let col = color_alpha(t.dim, alpha_dim());
                        let painter = $ui.ctx().layer_painter(egui::LayerId::new(
                            egui::Order::Foreground,
                            egui::Id::new(("nav_divider", x.to_bits())),
                        ));
                        painter.line_segment(
                            [egui::pos2(x, tb_rect.top()), egui::pos2(x, tb_rect.bottom())],
                            egui::Stroke::new(stroke_std(), col),
                        );
                    }};
                }

                // Feed pane (News + Discord + Screenshots)
                let resp = toolbar_btn(ui, &nav_label(Icon::NEWSPAPER, "Feed"), watchlist.feed_panel_open, t);
                Tooltip::new("Feed (News, Discord, Screenshots)").show(ui, &resp, t);
                paint_nav_col_tint(ui, tb_rect, resp.rect, t, resp.hovered(), watchlist.feed_panel_open, "right_feed");
                if resp.clicked() { watchlist.update_sidebar_state(|s| s.feed_panel_open = !s.feed_panel_open); }
                nav_divider!(ui, resp);

                // Playbook
                let resp = toolbar_btn(ui, &nav_label(Icon::STAR, "Playbook"), watchlist.playbook_panel_open, t);
                Tooltip::new("Playbook (Trade Ideas)").show(ui, &resp, t);
                paint_nav_col_tint(ui, tb_rect, resp.rect, t, resp.hovered(), watchlist.playbook_panel_open, "right_playbook");
                if resp.clicked() { watchlist.update_sidebar_state(|s| s.playbook_panel_open = !s.playbook_panel_open); }
                nav_divider!(ui, resp);

                // Watchlist toggle
                let resp = toolbar_btn(ui, &nav_label(Icon::LIST, "Watchlist"), watchlist.open, t);
                Tooltip::new("Watchlist").show(ui, &resp, t);
                paint_nav_col_tint(ui, tb_rect, resp.rect, t, resp.hovered(), watchlist.open, "right_watchlist");
                if resp.clicked() { watchlist.update_sidebar_state(|s| s.watchlist_open = !s.watchlist_open); }
                nav_divider!(ui, resp);

                // Orders panel
                let resp = toolbar_btn(ui, &nav_label(Icon::CURRENCY_DOLLAR, "Orders"), watchlist.orders_panel_open, t);
                Tooltip::new("Orders Panel").show(ui, &resp, t);
                paint_nav_col_tint(ui, tb_rect, resp.rect, t, resp.hovered(), watchlist.orders_panel_open, "right_orders");
                if resp.clicked() { watchlist.update_sidebar_state(|s| s.orders_panel_open = !s.orders_panel_open); }
                nav_divider!(ui, resp);

                // Analysis sidebar toggle
                let resp = toolbar_btn(ui, &nav_label(Icon::CHART_LINE, "Analysis"), watchlist.analysis_open, t);
                Tooltip::new("Analysis Sidebar").show(ui, &resp, t);
                paint_nav_col_tint(ui, tb_rect, resp.rect, t, resp.hovered(), watchlist.analysis_open, "right_analysis");
                if resp.clicked() { watchlist.update_sidebar_state(|s| s.analysis_open = !s.analysis_open); }
                nav_divider!(ui, resp);

                // Indicators panel — manage active indicators + library + tool toggles
                let resp = toolbar_btn(ui, &nav_label(Icon::PULSE, "Indicators"), watchlist.indicators_panel_open, t);
                Tooltip::new("Indicators (Active + Library + Tools)").show(ui, &resp, t);
                paint_nav_col_tint(ui, tb_rect, resp.rect, t, resp.hovered(), watchlist.indicators_panel_open, "right_indicators");
                if resp.clicked() { watchlist.update_sidebar_state(|s| s.indicators_panel_open = !s.indicators_panel_open); }
                nav_divider!(ui, resp);

                // Signals panel (Alerts + Signals) — no divider after, it's the last in the group
                {
                    let active_count = watchlist.alerts.iter().filter(|a| !a.triggered).count()
                        + panes.iter().flat_map(|p| p.price_alerts.iter()).filter(|a| !a.triggered && !a.draft).count();
                    let signals_resp = toolbar_btn(ui, &nav_label(Icon::LIGHTNING, "Signals"), watchlist.signals_panel_open, t);
                    Tooltip::new("Signals (Alerts + Signals)").show(ui, &signals_resp, t);
                    paint_nav_col_tint(ui, tb_rect, signals_resp.rect, t, signals_resp.hovered(), watchlist.signals_panel_open, "right_signals");
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
                        symbol: sym.clone(), timeframe: tf.clone(), bars: vec![], timestamps: vec![],
                    };
                    {
                        let global = crate::NATIVE_CHART_TXS.get_or_init(|| std::sync::Mutex::new(Vec::new()));
                        global.lock().unwrap().push(tx);
                    }
                    crate::chart_renderer::gpu::open_window(rx, initial, None);
                    crate::chart_renderer::gpu::fetch_bars_background(
                        panes[ap].symbol.clone(), panes[ap].timeframe.clone());
                }

                ui.add(egui::Separator::default().spacing(4.0));
            });

            // (Opt button is in scroll area, near account strip toggle)
        });
    });
    } // end if toolbar_visible

    if watchlist.account_strip_open {
        let mut do_cancel_all = false;
        let mut do_flatten    = false;
        egui::TopBottomPanel::top("account_strip")
            .exact_height(style_current().account_strip_height)
            .frame(egui::Frame::NONE.fill(t.toolbar_bg)
                .inner_margin(egui::Margin { left: 0, right: 0, top: 2, bottom: 2 })
                .stroke(egui::Stroke::new(STROKE_THIN, color_alpha(t.toolbar_border, ALPHA_DIM))))
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
            crate::chart_renderer::trading::order_manager::cancel_all_orders("");
            for chart in panes.iter_mut() { chart.orders.clear(); }
            std::thread::spawn(|| {
                let _ = reqwest::blocking::Client::new()
                    .delete(format!("{}/orders", APEXIB_URL))
                    .timeout(std::time::Duration::from_secs(5)).send();
            });
        }
        if do_flatten {
            crate::chart_renderer::trading::order_manager::cancel_all_orders("");
            for chart in panes.iter_mut() { chart.orders.retain(|o| o.status == OrderStatus::Executed); }
            std::thread::spawn(|| {
                let _ = reqwest::blocking::Client::new()
                    .post(format!("{}/risk/flatten", APEXIB_URL))
                    .timeout(std::time::Duration::from_secs(5)).send();
            });
        }
    }

    // NOTE: TB_BTN_CLICKED is cleared at the END of draw_chart, AFTER the
    // window drag handler reads it. Do NOT clear it here — it was causing
    // the flag to always be false when the drag handler checked it, making
    // every toolbar click trigger drag_window() and un-maximizing the window.

    // ── Timeframe dropdown popup ──
    // Mirrors the layout dropdown UX: full list grouped by category, each row
    // shows label + duration; star toggles "favorite" (appears in segmented
    // control outside); clicking the row picks the timeframe and closes.
    if watchlist.timeframe_dropdown_open {
        let dd_pos = watchlist.timeframe_dropdown_pos;
        let mut close_dd = false;
        let mut switch_to_tf: Option<&'static str> = None;
        let cur_tf = panes[ap].timeframe.clone();

        let dd_resp = egui::Window::new("timeframe_dropdown")
            .fixed_pos(dd_pos)
            .fixed_size(egui::vec2(220.0, 0.0))
            .title_bar(false)
            .frame(egui::Frame::popup(&ctx.style())
                .fill(t.toolbar_bg)
                .inner_margin(egui::Margin::same(gap_md() as i8))
                .stroke(egui::Stroke::new(stroke_std(), color_alpha(t.toolbar_border, 120)))
                .corner_radius(r_md_cr()))
            .show(ctx, |ui| {
                let hover_pos = ui.input(|i| i.pointer.hover_pos());
                let mut last_section = "";
                for &(tf_label, _secs, section) in ALL_TIMEFRAMES {
                    if section != last_section {
                        if !last_section.is_empty() {
                            ui.add_space(gap_xs());
                            let y = ui.cursor().min.y;
                            ui.painter().line_segment(
                                [egui::pos2(ui.min_rect().left() + 8.0, y), egui::pos2(ui.min_rect().left() + 236.0, y)],
                                egui::Stroke::new(stroke_thin(), color_alpha(t.toolbar_border, 50)));
                            ui.add_space(gap_sm());
                        }
                        ui.horizontal(|ui| {
                            ui.add_space(gap_md());
                            ui.label(egui::RichText::new(section).monospace().size(font_xs()).strong().color(color_half(t.dim)));
                        });
                        ui.add_space(gap_xs());
                        last_section = section;
                    }
                    let is_cur = cur_tf == tf_label;
                    let is_fav = watchlist.timeframe_favorites.iter().any(|f| f == tf_label);
                    let row_min = ui.cursor().min;
                    let row_rect = egui::Rect::from_min_size(row_min, egui::vec2(236.0, 24.0));
                    let hovered = hover_pos.map_or(false, |p| row_rect.contains(p));

                    if hovered || is_cur {
                        let bg = if is_cur { color_alpha(t.accent, 25) } else { color_alpha(t.toolbar_border, 30) };
                        ui.painter().rect_filled(row_rect, 3.0, bg);
                    }
                    if is_cur {
                        ui.painter().rect_filled(egui::Rect::from_min_size(row_rect.min, egui::vec2(2.0, 24.0)), 1.0, t.accent);
                    }

                    // Label
                    let lc = if is_cur { t.accent } else if hovered { t.text } else { t.dim };
                    ui.painter().text(
                        egui::pos2(row_rect.left() + 14.0, row_rect.center().y),
                        egui::Align2::LEFT_CENTER, tf_label,
                        mono_sm(), lc,
                    );

                    // Star — toggles favorite without closing the dropdown
                    let sr = egui::Rect::from_min_size(egui::pos2(row_rect.right() - 22.0, row_rect.center().y - 8.0), egui::vec2(icon_sm(), icon_sm()));
                    let sh = hover_pos.map_or(false, |p| sr.contains(p));
                    let sc = if is_fav { color_alpha(t.accent, ALPHA_HEAVY) } else if sh { color_half(t.dim) } else if hovered { color_very_dim(t.dim) } else { color_very_dim(t.dim) };
                    ui.painter().text(sr.center(), egui::Align2::CENTER_CENTER, Icon::STAR_FILL, egui::FontId::proportional(font_sm()), sc);
                    if sh { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
                    if sh && ui.input(|i| i.pointer.button_clicked(egui::PointerButton::Primary)) {
                        if is_fav { watchlist.timeframe_favorites.retain(|f| f != tf_label); }
                        else { watchlist.timeframe_favorites.push(tf_label.to_string()); }
                    }

                    // Click row (not star) to switch
                    let rh = hover_pos.map_or(false, |p| row_rect.contains(p)) && !sh;
                    if rh { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
                    if rh && ui.input(|i| i.pointer.button_clicked(egui::PointerButton::Primary)) {
                        switch_to_tf = Some(tf_label);
                        close_dd = true;
                    }

                    ui.allocate_space(egui::vec2(236.0, 24.0));
                }
            });

        // Click outside to close
        if let Some(resp) = dd_resp {
            let win_rect = resp.response.rect;
            if let Some(pos) = ctx.input(|i| i.pointer.interact_pos()) {
                if ctx.input(|i| i.pointer.button_pressed(egui::PointerButton::Primary)) {
                    if !win_rect.contains(pos) { close_dd = true; }
                }
            }
        }
        if let Some(new_tf) = switch_to_tf {
            if new_tf != panes[ap].timeframe {
                let cur_secs = tf_to_secs(&panes[ap].timeframe);
                let new_secs = tf_to_secs(new_tf);
                if cur_secs > 0 && new_secs > 0 {
                    let new_vc = ((panes[ap].vc as u64 * cur_secs as u64) / new_secs as u64).max(20).min(2000) as u32;
                    panes[ap].vc = new_vc;
                    panes[ap].vc_target = new_vc;
                }
                panes[ap].pending_timeframe_change = Some(new_tf.to_string());
            }
        }
        if close_dd { watchlist.timeframe_dropdown_open = false; }
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) { watchlist.timeframe_dropdown_open = false; }
    }

    // ── Layout dropdown popup (manual window — star clicks don't close it) ──
    if watchlist.layout_dropdown_open {
        let dd_pos = watchlist.layout_dropdown_pos;
        let mut close_dd = false;
        let mut switch_to: Option<Layout> = None;

        let dd_resp = egui::Window::new("layout_dropdown")
            .fixed_pos(dd_pos)
            .fixed_size(egui::vec2(220.0, 0.0))
            .title_bar(false)
            .frame(egui::Frame::popup(&ctx.style())
                .fill(t.toolbar_bg)
                .inner_margin(egui::Margin::same(gap_md() as i8))
                .stroke(egui::Stroke::new(stroke_std(), color_alpha(t.toolbar_border, 120)))
                .corner_radius(r_md_cr()))
            .show(ctx, |ui| {
                let hover_pos = ui.input(|i| i.pointer.hover_pos());
                let mut last_section = "";
                for &ly in ALL_LAYOUTS {
                    let sec = ly.section();
                    if sec != last_section {
                        if !last_section.is_empty() {
                            ui.add_space(gap_xs());
                            let y = ui.cursor().min.y;
                            ui.painter().line_segment(
                                [egui::pos2(ui.min_rect().left() + 8.0, y), egui::pos2(ui.min_rect().left() + 236.0, y)],
                                egui::Stroke::new(stroke_thin(), color_alpha(t.toolbar_border, 50)));
                            ui.add_space(gap_sm());
                        }
                        ui.horizontal(|ui| {
                            ui.add_space(gap_md());
                            ui.label(egui::RichText::new(sec).monospace().size(font_xs()).strong().color(color_half(t.dim)));
                        });
                        ui.add_space(gap_xs());
                        last_section = sec;
                    }
                    let is_cur = *layout == ly;
                    let is_fav = watchlist.layout_favorites.iter().any(|f| f == ly.label());
                    let row_min = ui.cursor().min;
                    let row_rect = egui::Rect::from_min_size(row_min, egui::vec2(236.0, 26.0));
                    let hovered = hover_pos.map_or(false, |p| row_rect.contains(p));

                    if hovered || is_cur {
                        let bg = if is_cur { color_alpha(t.accent, 25) } else { color_alpha(t.toolbar_border, 30) };
                        ui.painter().rect_filled(row_rect, 3.0, bg);
                    }
                    if is_cur {
                        ui.painter().rect_filled(egui::Rect::from_min_size(row_rect.min, egui::vec2(2.0, 26.0)), 1.0, t.accent);
                    }

                    // Mini glyph (29×19)
                    let gr = egui::Rect::from_min_size(egui::pos2(row_rect.left() + 6.0, row_rect.center().y - 9.5), egui::vec2(29.0, 19.0));
                    let gc = if is_cur { t.accent } else if hovered { t.dim } else { color_half(t.dim) };
                    let mini = ly.pane_rects(gr, ly.max_panes(), 0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5);
                    for mr in &mini {
                        let s = egui::Rect::from_min_max(egui::pos2(mr.left() + 0.5, mr.top() + 0.5), egui::pos2(mr.right() - 0.5, mr.bottom() - 0.5));
                        ui.painter().rect_filled(s, 1.0, color_alpha(gc, 80));
                        ui.painter().rect_stroke(s, 1.0, egui::Stroke::new(stroke_thin(), color_alpha(gc, 150)), egui::StrokeKind::Outside);
                    }

                    // Label + description
                    let lc = if is_cur { t.accent } else if hovered { t.text } else { t.dim };
                    ui.painter().text(egui::pos2(row_rect.left() + 42.0, row_rect.center().y), egui::Align2::LEFT_CENTER, ly.label(), mono_sm(), lc);
                    let dc = if hovered { color_alpha(t.dim, ALPHA_HEAVY) } else { color_muted(t.dim) };
                    ui.painter().text(egui::pos2(row_rect.left() + 74.0, row_rect.center().y), egui::Align2::LEFT_CENTER, ly.description(), mono_sm(), dc);

                    // Star — filled, raw pointer click
                    let sr = egui::Rect::from_min_size(egui::pos2(row_rect.right() - 22.0, row_rect.center().y - 8.0), egui::vec2(icon_sm(), icon_sm()));
                    let sh = hover_pos.map_or(false, |p| sr.contains(p));
                    let sc = if is_fav { color_alpha(t.accent, ALPHA_HEAVY) } else if sh { color_half(t.dim) } else if hovered { color_very_dim(t.dim) } else { color_very_dim(t.dim) };
                    ui.painter().text(sr.center(), egui::Align2::CENTER_CENTER, Icon::STAR_FILL, egui::FontId::proportional(font_sm()), sc);
                    if sh { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
                    if sh && ui.input(|i| i.pointer.button_clicked(egui::PointerButton::Primary)) {
                        if is_fav { watchlist.layout_favorites.retain(|f| f != ly.label()); }
                        else { watchlist.layout_favorites.push(ly.label().to_string()); }
                    }

                    // Click row (not star) to switch
                    let rh = hover_pos.map_or(false, |p| row_rect.contains(p)) && !sh;
                    if rh { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
                    if rh && ui.input(|i| i.pointer.button_clicked(egui::PointerButton::Primary)) {
                        switch_to = Some(ly);
                        close_dd = true;
                    }

                    ui.allocate_space(egui::vec2(236.0, 26.0));
                }

                // ── Saved layout templates ─────────────────────────────────────
                // List existing pane templates saved from this dropdown, each
                // with an "Apply" button that restores the layout + symbols.
                if !watchlist.pane_templates.is_empty() {
                    ui.add_space(gap_xs());
                    let y = ui.cursor().min.y;
                    ui.painter().line_segment(
                        [egui::pos2(ui.min_rect().left() + 8.0, y), egui::pos2(ui.min_rect().left() + 236.0, y)],
                        egui::Stroke::new(stroke_thin(), color_alpha(t.toolbar_border, 50)));
                    ui.add_space(gap_sm());
                    ui.horizontal(|ui| {
                        ui.add_space(gap_md());
                        ui.label(egui::RichText::new("SAVED LAYOUTS").monospace().size(font_xs()).strong().color(color_half(t.dim)));
                    });
                    ui.add_space(gap_xs());
                    let templates_snapshot: Vec<String> = watchlist.pane_templates.iter().map(|(n, _)| n.clone()).collect();
                    for tpl_name in &templates_snapshot {
                        let row_min = ui.cursor().min;
                        let row_rect = egui::Rect::from_min_size(row_min, egui::vec2(236.0, 24.0));
                        let row_hover = hover_pos.map_or(false, |p| row_rect.contains(p));
                        if row_hover {
                            ui.painter().rect_filled(row_rect, 3.0, color_alpha(t.toolbar_border, 30));
                        }
                        let lc = if row_hover { t.text } else { t.dim };
                        ui.painter().text(
                            egui::pos2(row_rect.left() + 8.0, row_rect.center().y),
                            egui::Align2::LEFT_CENTER,
                            tpl_name.as_str(),
                            mono_sm(),
                            lc,
                        );
                        // Apply button — right-aligned
                        let apply_w = 42.0;
                        let apply_rect = egui::Rect::from_min_size(
                            egui::pos2(row_rect.right() - apply_w - 4.0, row_rect.center().y - 10.0),
                            egui::vec2(apply_w, 20.0),
                        );
                        let apply_hov = hover_pos.map_or(false, |p| apply_rect.contains(p));
                        let apply_fill = if apply_hov { color_alpha(t.accent, 40) } else { color_alpha(t.accent, 15) };
                        ui.painter().rect_filled(apply_rect, 3.0, apply_fill);
                        ui.painter().text(
                            apply_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            "Apply",
                            mono_sm(),
                            if apply_hov { t.accent } else { color_alpha(t.accent, 180) },
                        );
                        if apply_hov { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
                        if apply_hov && ui.input(|i| i.pointer.button_clicked(egui::PointerButton::Primary)) {
                            // Restore from workspace file with same name
                            watchlist.active_workspace = tpl_name.clone();
                            watchlist.pending_workspace_load = Some(tpl_name.clone());
                            close_dd = true;
                        }
                        ui.allocate_space(egui::vec2(236.0, 24.0));
                    }
                }

                // ── Save current layout as… ────────────────────────────────────
                {
                    ui.add_space(gap_xs());
                    let y = ui.cursor().min.y;
                    ui.painter().line_segment(
                        [egui::pos2(ui.min_rect().left() + 8.0, y), egui::pos2(ui.min_rect().left() + 236.0, y)],
                        egui::Stroke::new(stroke_thin(), color_alpha(t.toolbar_border, 50)));
                    ui.add_space(gap_sm());
                    ui.horizontal(|ui| {
                        use crate::ui_kit::widgets::Input;
                        Input::new(&mut watchlist.pane_template_name)
                            .placeholder("Save layout as…")
                            .min_width(148.0)
                            .size(KitSize::Sm)
                            .show(ui, t);
                        let can_save = !watchlist.pane_template_name.trim().is_empty();
                        if can_save {
                            if KitButton::new("Save").variant(KitVariant::Primary).size(KitSize::Sm)
                                .tint(t.accent).show(ui, t).clicked()
                            {
                                let name = watchlist.pane_template_name.trim().to_string();
                                // Persist via workspace machinery (handles layout + symbols + TF)
                                save_workspace(&name, panes, *layout);
                                // Also store in pane_templates so the list updates immediately
                                let layout_val = serde_json::json!({
                                    "kind": "layout_template",
                                    "layout": layout.label(),
                                    "panes": panes.iter().map(|p| serde_json::json!({
                                        "symbol": p.symbol,
                                        "timeframe": p.timeframe,
                                        "link_group": p.link_group,
                                    })).collect::<Vec<_>>(),
                                });
                                // Remove existing entry with same name, then push
                                watchlist.pane_templates.retain(|(n, _)| n != &name);
                                watchlist.pane_templates.push((name.clone(), layout_val));
                                save_templates(&watchlist.pane_templates);
                                watchlist.pane_template_name.clear();
                                close_dd = true;
                            }
                        }
                    });
                }
            });

        // Click outside to close
        if let Some(resp) = dd_resp {
            let win_rect = resp.response.rect;
            if let Some(pos) = ctx.input(|i| i.pointer.interact_pos()) {
                if ctx.input(|i| i.pointer.button_pressed(egui::PointerButton::Primary)) {
                    if !win_rect.contains(pos) { close_dd = true; }
                }
            }
        }
        if let Some(ly) = switch_to {
            // Inline switch_layout logic since the closure is out of scope
            *layout = ly;
            let max = ly.max_panes();
            while panes.len() < max {
                let mut c = Chart::new();
                c.theme_idx = panes[0].theme_idx;
                panes.push(c);
            }
            if *active_pane >= max { *active_pane = 0; }
        }
        if close_dd { watchlist.layout_dropdown_open = false; }
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) { watchlist.layout_dropdown_open = false; }
    }

    // ── command_palette
    crate::chart_renderer::ui::command_palette::draw(ctx, watchlist, panes, layout, active_pane, t);

    // ── hotkey_editor
    crate::chart_renderer::ui::tools::hotkey_editor::draw(ctx, watchlist, panes, ap, t);

    // ── Settings panel
    crate::chart_renderer::ui::panels::settings_panel::draw(ctx, watchlist, &mut panes[ap], t, ap);
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

    // ── Group manager popup ────────────────────────────────────────────────────
    if panes[ap].group_manager_open {
        let mut close_gm = false;
        dialog_window_themed(ctx, "group_manager", egui::pos2(200.0, 100.0), 250.0, t.toolbar_bg, t.toolbar_border, None)
            .show(ctx, |ui| {
                if crate::ui_kit::widgets::Header::dialog("NEW GROUP").closable(true).show(ui, t).close_clicked { close_gm = true; }
                ui.add_space(gap_xl());
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
            watchlist.update_ui_settings(|s| {
                s.has_seen_welcome = true;
                s.welcome_step_resume = 0;
            });
            watchlist.welcome_wizard = None;
        }
    }

    // ── Toast 2.0 — bottom-left anchor, fixed 360px, severity-coded ─────────
    //
    // Severity byte vocabulary (prefix stripped before display):
    //   no prefix / \x00 → Info     (accent tint, Icon::INFO)
    //   \x01             → Warning  (warn tint,   Icon::WARNING)
    //   \x02             → Danger   (bear tint,   Icon::SHIELD_WARNING)
    //   \x03             → Critical (bear tint + pulse + stronger alpha, Icon::SHIELD_WARNING_FILL)
    //   \x04             → Success  (bull tint,   Icon::CHECK_CIRCLE)
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
    if !toasts.is_empty() {
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

        // ── Severity decode helpers ─────────────────────────────────────────
        // Returns (display_msg, accent_color, icon, is_critical)
        fn decode_severity<'m>(msg: &'m str, is_buy: bool, t: &crate::chart_renderer::gpu::Theme)
            -> (&'m str, egui::Color32, &'static str, bool)
        {
            if let Some(rest) = msg.strip_prefix('\x01') {
                (rest, t.warn, Icon::WARNING, false)
            } else if let Some(rest) = msg.strip_prefix('\x02') {
                (rest, t.bear, Icon::SHIELD_WARNING, false)
            } else if let Some(rest) = msg.strip_prefix('\x03') {
                (rest, t.bear, Icon::SHIELD_WARNING_FILL, true)
            } else if let Some(rest) = msg.strip_prefix('\x04') {
                (rest, t.bull, Icon::CHECK_CIRCLE, false)
            } else {
                // no prefix — Info or legacy is_buy path
                let color = if is_buy { t.bull } else { t.accent };
                (msg, color, Icon::INFO, false)
            }
        }

        // ── Toast id — stable hash of (msg, created nanos) ─────────────────
        fn toast_id(msg: &str, created: &std::time::Instant) -> u64 {
            let mut h = DefaultHasher::new();
            msg.hash(&mut h);
            created.elapsed().as_nanos().hash(&mut h);
            h.finish()
        }

        // ── Expand state (show all beyond MAX_VISIBLE) ──────────────────────
        let expand_id = Id::new("toast_v2_expand");
        let expand_until: f64 = ctx.data(|d| d.get_temp(expand_id).unwrap_or(0.0f64));
        let expanded = ctx.input(|i| i.time) < expand_until;

        // ── Separate pinned vs normal toasts ────────────────────────────────
        let mut pinned_indices: Vec<usize> = Vec::new();
        let mut normal_indices: Vec<usize> = Vec::new();
        for (i, (msg, _price, created, _is_buy)) in toasts.iter().enumerate() {
            let tid = toast_id(msg, created);
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
        let base_y = screen.bottom() - TOAST_MARGIN - chip_h - chip_gap;

        // ── Render visible toasts ───────────────────────────────────────────
        let now_time = ctx.input(|i| i.time);
        let mut close_toast_idx: Option<usize> = None;
        let mut pin_toggle_idx: Option<(usize, bool)> = None; // (toast_idx, new_pin_state)

        for (slot, &toast_i) in visible_indices.iter().enumerate() {
            let (msg, _price, created, is_buy) = &toasts[toast_i];
            let age = created.elapsed().as_secs_f32();
            // Stagger: toast at slot only starts appearing after slot*80ms (preserve existing stagger).
            let stagger_delay = toast_i as f32 * 0.08;
            let visible_age = (age - stagger_delay).max(0.0);
            if visible_age <= 0.0 { continue; }

            let tid = toast_id(msg, created);
            let pin_id = Id::new(("toast_pin", tid));
            let is_pinned: bool = ctx.data(|d| d.get_temp(pin_id).unwrap_or(false));

            // Alpha: fade in over 120ms, start fading out 1s before expiry.
            // Pinned toasts don't fade out.
            let fade_in  = (visible_age / 0.12).min(1.0);
            let fade_out = if is_pinned { 1.0f32 } else { ((DISMISS_SECS - age) / 1.0).min(1.0).max(0.0) };
            let alpha = (fade_in * fade_out).min(1.0).max(0.0);
            if alpha <= 0.0 { continue; }

            let (display_msg, sev_color, icon, is_critical) = decode_severity(msg, *is_buy, t);

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
            let text_col   = color_alpha(t.text, (230.0 * alpha) as u8);
            let icon_col   = color_alpha(sev_color, (200.0 * alpha) as u8);
            let dim_col    = color_alpha(t.dim, (160.0 * alpha) as u8);
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
                                ui.label(egui::RichText::new(icon).size(font_md()).color(icon_col));
                                ui.add_space(gap_xs());
                                // Message — wraps at TOAST_W boundary.
                                ui.label(
                                    egui::RichText::new(display_msg)
                                        .size(font_sm())
                                        .color(text_col)
                                );
                            });

                            // ── Bottom row: age label + pin hint + close ────
                            ui.horizontal(|ui| {
                                let age_str = if age < 60.0 {
                                    format!("{}s ago", age as u32)
                                } else {
                                    "1m+ ago".to_string()
                                };
                                ui.label(egui::RichText::new(age_str).size(font_xs()).color(dim_col));

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
                                    let pin_icon = if is_pinned { Icon::PUSH_PIN } else { Icon::PUSH_PIN };
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
            let chip_col = color_alpha(t.dim, alpha_muted());
            let chip_bg  = color_alpha(t.toolbar_bg, alpha_muted());
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
            let (msg, _, created, _) = &toasts[toast_i];
            let tid = toast_id(msg, created);
            let pin_id = Id::new(("toast_pin", tid));
            if new_pin {
                // Enforce max 5 pinned: unpin oldest if at cap.
                if pinned_indices.len() >= MAX_PINNED {
                    if let Some(&oldest_i) = pinned_indices.first() {
                        let (om, _, oc, _) = &toasts[oldest_i];
                        let old_tid = toast_id(om, oc);
                        let old_pin_id = Id::new(("toast_pin", old_tid));
                        ctx.data_mut(|d| d.insert_temp::<bool>(old_pin_id, false));
                    }
                }
            }
            ctx.data_mut(|d| d.insert_temp(pin_id, new_pin));
        }

        // Note: close_toast_idx is informational — the actual expiry is handled
        // by gpu.rs (retain toasts where age < 5s). For pinned toasts that the
        // user explicitly closes, we force-expire them by clearing the pin flag
        // so the normal 5s window covers them. Since the toast tuple has no
        // mutable "dismissed" flag, we rely on the window being gone next frame
        // after the 5s window passes. For pinned closes, we unpin immediately.
        if let Some(toast_i) = close_toast_idx {
            let (msg, _, created, _) = &toasts[toast_i];
            let tid = toast_id(msg, created);
            let pin_id = Id::new(("toast_pin", tid));
            ctx.data_mut(|d| d.insert_temp::<bool>(pin_id, false));
        }
    }

    // RegimeTape moved off the top dock — it now lives as a tab inside the
    // Signals sidebar (signals_panel.rs → SignalsTab::Regime). Top dock space
    // was too valuable to reserve for a 40-48px always-visible cell strip.

    // ── ProvenancePane (SOTA UX §4.1) — right side panel, evidence DAG.
    // Hidden by default; opens when `provenance_pane::request_open(lineage_id)`
    // is called from signals_panel (or any other panel that surfaces lineage).
    span_begin("sidebar.provenance");
    crate::chart_renderer::ui::panels::provenance_pane::draw(ctx, watchlist, t);

    // ── Watchlist side panel
    span_begin("sidebar.watchlist");
    crate::chart_renderer::ui::panels::watchlist_panel::draw(ctx, watchlist, panes, ap, t);

    // ── Object Tree side panel
    span_begin("sidebar.object_tree");
    crate::chart_renderer::ui::panels::object_tree::draw(ctx, watchlist, panes, ap, t);

    // ── Book pane (Positions/Orders + Journal tabs) ─────────────────────────
    span_begin("sidebar.orders");
    crate::chart_renderer::ui::panels::orders_panel::draw(ctx, watchlist, panes, ap, t, account_data_cached);

    // ── Order Ledger (Wave 3 visibility panel) ──────────────────────────────
    // Hotkey: Ctrl+L toggles. The render loop takes a non-mutable `panes`
    // slice for active-order display; cancel actions dispatch through the
    // global `order_manager` API.
    if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::L)) {
        watchlist.update_sidebar_state(|s| s.order_ledger_open = !s.order_ledger_open);
    }
    span_begin("sidebar.order_ledger");
    crate::chart_renderer::ui::panels::order_ledger_panel::draw(ctx, watchlist, panes, t);

    // ── Order System Health (operator observability) ────────────────────────
    // Hotkey: Ctrl+Shift+O toggles a small dashboard with submit-latency,
    // reject-rate, top reject reasons, active-state counts and broker contact
    // age. Recomputes journal aggregates at most once/sec via in-panel cache.
    if ctx.input(|i| i.modifiers.command && i.modifiers.shift && i.key_pressed(egui::Key::O)) {
        watchlist.update_sidebar_state(|s| s.order_health_open = !s.order_health_open);
    }
    span_begin("sidebar.order_health");
    crate::chart_renderer::ui::panels::order_health_panel::draw(ctx, watchlist, t);

    // ── Scanner side panel
    span_begin("sidebar.scanner");
    crate::chart_renderer::ui::panels::scanner_panel::draw(ctx, watchlist, panes, ap, t);

    // ── Time & Sales side panel
    span_begin("sidebar.tape");
    crate::chart_renderer::ui::panels::tape_panel::draw(ctx, watchlist, &panes[ap].symbol, t);

    // ── RRG (Relative Rotation Graph) side panel
    span_begin("sidebar.rrg");
    crate::chart_renderer::ui::panels::rrg_panel::draw(ctx, watchlist, t);

    // ── Analysis sidebar (unified RRG / T&S / Scanner / Scripts)
    span_begin("sidebar.analysis");
    crate::chart_renderer::ui::panels::analysis_panel::draw(ctx, watchlist, panes, *active_pane, t);

    // ── Indicators panel (active + library + tool toggles)
    span_begin("sidebar.indicators");
    crate::chart_renderer::ui::panels::indicators_panel::draw(ctx, watchlist, panes, ap, t);

    // ── Signals sidebar (unified Alerts + Signals)
    span_begin("sidebar.signals");
    crate::chart_renderer::ui::panels::signals_panel::draw(ctx, watchlist, panes, ap, t);

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

    // ── Feed sidebar (unified News + Discord + Screenshots)
    span_begin("sidebar.feed");
    crate::chart_renderer::ui::panels::feed_panel::draw(ctx, watchlist, panes, ap, t);

    // ── Playbook sidebar
    span_begin("sidebar.playbook");
    crate::chart_renderer::ui::panels::playbook_panel::draw(ctx, watchlist, panes, ap, t);

    // ── Journal sidebar
    span_begin("sidebar.journal");
    crate::chart_renderer::ui::panels::journal_panel::draw(ctx, watchlist, t);

    // ── Script / Backtesting panel
    span_begin("sidebar.script");
    crate::chart_renderer::ui::panels::script_panel::draw(ctx, watchlist, t);

    // ── Spread Builder panel
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
                PENDING_TOASTS.with(|ts| ts.borrow_mut().push((msg, price, above)));
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
        let wl_tip_border = if st.hairline_borders { t.toolbar_border } else { color_alpha(t.toolbar_border, crate::chart_renderer::ui::style::alpha_strong()) };
        egui::Area::new(egui::Id::new("wl_tooltip_deferred"))
            .fixed_pos(egui::pos2(tip_x, tip_y))
            .order(egui::Order::Tooltip)
            .show(ctx, |ui| {
                egui::Frame::popup(&ctx.style()).fill(t.toolbar_bg)
                    .stroke(egui::Stroke::new(wl_tip_stroke_w, wl_tip_border))
                    .inner_margin(crate::chart_renderer::ui::style::gap_lg()).corner_radius(wl_tip_cr).show(ui, |ui| {
                    ui.set_max_width(tip_w);
                    ui.label(TextStyle::NumericLg.as_rich(&tip.sym, t.text));
                    ui.horizontal(|ui| {
                        ui.label(TextStyle::Numeric.as_rich(&format!("${:.2}", tip.price), color_alpha(t.text,220)));
                        ui.label(TextStyle::Numeric.as_rich(&format!("{:+.2}%", change_pct), chg_col));
                    });
                    ui.add_space(gap_sm()); ui.separator(); ui.add_space(gap_sm());
                    if tip.day_high > tip.day_low {
                        ui.horizontal(|ui| {
                            ui.label(TextStyle::Caption.as_rich("Day", dim));
                            ui.label(TextStyle::MonoSm.as_rich(&format!("{:.2}", tip.day_low), dim));
                            let bar_w = 60.0;
                            let (bar_rect, _) = ui.allocate_exact_size(egui::vec2(bar_w, 8.0), egui::Sense::hover());
                            ui.painter().rect_filled(bar_rect, 2.0, color_alpha(t.text,15));
                            let range = tip.day_high - tip.day_low;
                            if range > 0.0 {
                                let pos = ((tip.price - tip.day_low) / range).clamp(0.0, 1.0);
                                ui.painter().circle_filled(egui::pos2(bar_rect.left() + pos * bar_w, bar_rect.center().y), 3.0, chg_col);
                            }
                            ui.label(TextStyle::MonoSm.as_rich(&format!("{:.2}", tip.day_high), dim));
                        });
                    }
                    if tip.high_52wk > tip.low_52wk {
                        ui.horizontal(|ui| {
                            ui.label(TextStyle::Caption.as_rich("52w", dim));
                            ui.label(TextStyle::MonoSm.as_rich(&format!("{:.0}", tip.low_52wk), dim));
                            let bar_w = 60.0;
                            let (bar_rect, _) = ui.allocate_exact_size(egui::vec2(bar_w, 8.0), egui::Sense::hover());
                            ui.painter().rect_filled(bar_rect, 2.0, color_alpha(t.text,15));
                            let range = tip.high_52wk - tip.low_52wk;
                            if range > 0.0 {
                                let pos = ((tip.price - tip.low_52wk) / range).clamp(0.0, 1.0);
                                ui.painter().circle_filled(egui::pos2(bar_rect.left() + pos * bar_w, bar_rect.center().y), 3.0, t.accent);
                            }
                            ui.label(TextStyle::MonoSm.as_rich(&format!("{:.0}", tip.high_52wk), dim));
                        });
                    }
                    ui.add_space(gap_xs());
                    ui.horizontal(|ui| {
                        ui.label(TextStyle::MonoSm.as_rich(&format!("ATR {:.2}", tip.atr), dim));
                        ui.label(TextStyle::MonoSm.as_rich(&format!("RVOL {:.1}x", tip.rvol),
                            if tip.rvol > 2.0 { t.warn } else { dim }));
                    });
                    if change_pct.abs() > tip.avg_range * 1.5 {
                        ui.label(TextStyle::Caption.as_rich("EXTREME MOVE", chg_col));
                    }
                    if tip.earnings_days >= 0 && tip.earnings_days <= 14 {
                        ui.add_space(gap_xs());
                        ui.label(TextStyle::MonoSm.as_rich(&format!("{} Earnings in {} days", Icon::LIGHTNING, tip.earnings_days), color_alpha(t.accent, ALPHA_HEAVY)));
                    }
                    if !tip.tags.is_empty() {
                        ui.add_space(gap_xs());
                        ui.horizontal_wrapped(|ui| { for tag in &tip.tags { ui.label(TextStyle::Caption.as_rich(tag, t.accent)); } });
                    }
                    if tip.alert_triggered {
                        ui.label(TextStyle::MonoSm.as_rich(&format!("{} Alert triggered", Icon::LIGHTNING), t.bear));
                    }
                });
            });
    }

    span_end(); // top_panel
}
