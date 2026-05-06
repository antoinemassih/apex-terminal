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
    get_theme,
    rgb,
    save_workspace, list_workspaces, save_state,
    widget_description, paint_widget_preview,
    new_uuid,
};
use crate::chart_renderer::ui::style::{
    color_alpha, hex_to_color, segmented_control,
    dialog_window_themed, dialog_header, action_btn,
    FONT_MD, FONT_SM, STROKE_STD, STROKE_THIN,
    ALPHA_FAINT, ALPHA_GHOST, ALPHA_DIM,
    BTN_ICON_SM, BTN_ICON_LG,
    set_toolbar_rect, tb_group_break, current as style_current,
    font_xs, font_sm, font_md,
    gap_xs, gap_sm, gap_md, gap_lg, gap_xl,
    stroke_std, stroke_thin, r_md_cr,
};
use crate::chart_renderer::ui::widgets::foundation::text_style::TextStyle;
use crate::chart_renderer::trading::{AccountSummary, Position, IbOrder, OrderStatus};
use crate::chart_renderer::{ChartCommand, ChartWidgetKind, ChartWidget, DrawingGroup};
use super::ToolbarBtn;

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
    use crate::monitoring::{span_begin, span_end};
    let ap = *active_pane;
    span_begin("top_panel");

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
        .frame(egui::Frame::NONE.fill(t.toolbar_bg).inner_margin(egui::Margin { left: gap_lg() as i8, right: 0, top: 0, bottom: 0 }))
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

        // Paper mode indicator — green line below toolbar
        if crate::chart_renderer::trading::order_manager::is_paper_mode() {
            let paper_line_y = tb_rect.bottom();
            ui.painter().line_segment(
                [egui::pos2(tb_rect.left(), paper_line_y),
                 egui::pos2(tb_rect.right(), paper_line_y)],
                egui::Stroke::new(4.0, t.bull));
        }

        ui.horizontal_centered(|ui| {
            ui.spacing_mut().item_spacing.x = gap_sm();

            // ── Logo ──
            let (logo_rect, _) = ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
            let lp = ui.painter_at(logo_rect);
            let lc = logo_rect.center();
            lp.add(egui::Shape::line(vec![
                egui::pos2(lc.x, lc.y - 6.0), egui::pos2(lc.x + 6.0, lc.y + 5.0),
                egui::pos2(lc.x - 6.0, lc.y + 5.0), egui::pos2(lc.x, lc.y - 6.0),
            ], egui::Stroke::new(STROKE_STD, t.accent)));
            lp.line_segment([egui::pos2(lc.x - 3.5, lc.y + 1.0), egui::pos2(lc.x + 3.5, lc.y + 1.0)], egui::Stroke::new(STROKE_STD, t.accent));

            ui.add_space(gap_sm());
            ui.spacing_mut().item_spacing.x = 3.0;

            // ── Account button (broker + connection state) ──
            // #7: When vertical_group_dividers active (Meridien), paint a full-column
            //     hover fill spanning the entire toolbar height before the button widget.
            {
                let connected = account_data_cached.as_ref().map_or(false, |(s,_,_)| s.connected);
                let acct_label = if connected { "IBKR ●" } else { "IBKR ○" };
                let acct_active = watchlist.account_strip_open;
                let acct_resp = ui.add(ToolbarBtn::new(acct_label).active(acct_active).theme(t))
                    .on_hover_text("Account Summary");
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
                    watchlist.account_strip_open = !watchlist.account_strip_open;
                }
            }

            // ── Paper / Live — colored text, no background fill ──
            {
                let paper = crate::chart_renderer::trading::order_manager::is_paper_mode();
                const PAPER_ORANGE: egui::Color32 = rgb(255, 165, 0);
                let (label, color) = if paper {
                    ("PAPER", PAPER_ORANGE)
                } else {
                    ("LIVE", t.dim)
                };
                let tip = if paper { "Switch to Live" } else { "Switch to Paper" };
                let resp = ui.add(crate::chart::renderer::ui::inputs::buttons::ChromeBtn::new(
                    egui::RichText::new(label).monospace().size(FONT_MD).strong().color(color))
                    .frameless(true));
                // pointing-hand cursor already set by ChromeBtn on hover
                if resp.on_hover_text(tip).clicked() {
                    crate::chart_renderer::trading::order_manager::set_paper_mode(!paper);
                }
            }

            // ── Orders book ──
            if ui.add(ToolbarBtn::new("ORDERS").active(watchlist.orders_panel_open).theme(t)).on_hover_text("Orders").clicked() {
                watchlist.orders_panel_open = !watchlist.orders_panel_open;
            }

            // ── DOM sidebar ──
            if ui.add(ToolbarBtn::new("DOM").active(panes[ap].dom_sidebar_open).theme(t)).on_hover_text("DOM Sidebar").clicked() {
                panes[ap].dom_sidebar_open = !panes[ap].dom_sidebar_open;
            }
            // ── Order Entry ──
            if ui.add(ToolbarBtn::new("ORDER").active(watchlist.order_entry_open).theme(t)).on_hover_text("Order Entry").clicked() {
                watchlist.order_entry_open = !watchlist.order_entry_open;
            }

            ui.add(egui::Separator::default().spacing(4.0));

            // ── Scrollable middle section ──
            // Calculate available width: total - logo(25) - symbol(~70) - right section(~350)
            let right_width = 130.0; // window controls + Opt button
            let middle_width = (ui.available_width() - right_width).max(60.0);
            egui::ScrollArea::horizontal().max_width(middle_width).show(ui, |ui| {
            ui.spacing_mut().item_spacing.x = gap_xs();

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
                let tf_dd_btn = ui.add(ToolbarBtn::new(Icon::CARET_DOWN).active(watchlist.timeframe_dropdown_open).theme(t));
                if tf_dd_btn.clicked() {
                    watchlist.timeframe_dropdown_open = !watchlist.timeframe_dropdown_open;
                    watchlist.timeframe_dropdown_pos = egui::pos2(tf_dd_btn.rect.left(), tf_dd_btn.rect.bottom() + 2.0);
                }
            }
            ui.add_space(gap_sm());
            // ── Range dropdown (sets interval + visible bars) ──
            {
                let range_resp = ui.menu_button(egui::RichText::new("Range").monospace().size(FONT_MD).strong().color(t.dim), |ui| {
                    ui.style_mut().visuals.widgets.inactive.bg_fill = t.toolbar_bg;
                    ui.style_mut().visuals.window_fill = t.toolbar_bg;
                    ui.label(egui::RichText::new("RANGE").monospace().size(FONT_SM).color(t.dim.gamma_multiply(0.4)));
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
                        if ui.selectable_label(false, egui::RichText::new(label).monospace().size(FONT_SM)).clicked() {
                            panes[ap].pending_timeframe_change = Some(tf.to_string());
                            panes[ap].vc = preset_vc;
                            panes[ap].vc_target = preset_vc;
                            ui.close_menu();
                        }
                    }
                });
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
                ui.menu_button(egui::RichText::new(draw_label).monospace().size(FONT_MD).color(if has_tool { t.accent } else { t.dim }), |ui| {
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
                        ui.label(egui::RichText::new(*section).monospace().size(FONT_SM).color(t.dim));
                        for (tool, label) in *tools {
                            let shortcut = tool_shortcut(tool);
                            let resp = ui.horizontal(|ui| {
                                let r = ui.selectable_label(cur == *tool, egui::RichText::new(*label).monospace().size(FONT_SM));
                                if let Some(ref key) = shortcut {
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        ui.label(egui::RichText::new(key).monospace().size(FONT_SM).color(t.dim.gamma_multiply(0.7)));
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
                        if ui.selectable_label(false, egui::RichText::new("Cancel Tool").monospace().size(FONT_SM).color(t.bear)).clicked() {
                            new_tool = Some(String::new());
                        }
                    }
                });
                if let Some(tool) = new_tool {
                    panes[ap].draw_tool = tool;
                    panes[ap].pending_pt = None; panes[ap].pending_pt2 = None; panes[ap].pending_pts.clear();
                }
                TB_BTN_CLICKED.with(|f| f.set(true));
            }
            // Magnet snap
            if ui.add(ToolbarBtn::new(Icon::MAGNET).active(panes[ap].magnet).theme(t)).on_hover_text("Magnet Snap").clicked() { panes[ap].magnet = !panes[ap].magnet; }
            // Object tree toggle (consolidated drawings/indicators/overlays panel)
            let draw_count = panes[ap].drawings.len();
            let list_label = if draw_count > 0 {
                format!("{} {}", Icon::TREE_STRUCTURE, draw_count)
            } else {
                Icon::TREE_STRUCTURE.to_string()
            };
            if ui.add(ToolbarBtn::new(&list_label).active(watchlist.object_tree_open).theme(t)).on_hover_text("Object Tree").clicked() {
                watchlist.object_tree_open = !watchlist.object_tree_open;
            }
            // ── Broadcast — drawing section (applies to all panes) ──
            {
                let bc = watchlist.broadcast_mode;
                if ui.add(ToolbarBtn::new(Icon::BROADCAST).active(bc).theme(t)).on_hover_text("Broadcast — changes apply to all panes").clicked() {
                    watchlist.broadcast_mode = !watchlist.broadcast_mode;
                    TB_BTN_CLICKED.with(|f| f.set(true));
                }
            }
            // ── Trendline filter — drawing section ──
            if ui.add(ToolbarBtn::new(Icon::FUNNEL).active(watchlist.trendline_filter_open).theme(t)).on_hover_text("Trendline Filter").clicked() {
                watchlist.trendline_filter_open = !watchlist.trendline_filter_open;
            }

            ui.add(egui::Separator::default().spacing(4.0));

            // ── Organized dropdown menus ──
            let _menu_font = egui::FontId::monospace(10.0);
            let check = |active: bool| if active { Icon::CHECK } else { "  " };

            // Chart Type dropdown (single-select)
            let cm_label = match panes[ap].candle_mode {
                CandleMode::Standard => "STD", CandleMode::Violin => "VLN",
                CandleMode::Gradient => "GRD", CandleMode::ViolinGradient => "V+G",
                CandleMode::HeikinAshi => "HA", CandleMode::Line => "LN", CandleMode::Area => "AR",
                CandleMode::Renko => "RNK", CandleMode::RangeBar => "RNG", CandleMode::TickBar => "TCK",
            };
            let prev_candle_mode = panes[ap].candle_mode;
            ui.menu_button(egui::RichText::new(cm_label).monospace().size(FONT_MD).strong().color(t.dim), |ui| {
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
                    if ui.selectable_label(active, egui::RichText::new(format!("{} {}", check(active), label)).monospace().size(FONT_SM)).clicked() {
                        panes[ap].candle_mode = mode;
                    }
                }
                ui.separator();
                let log = panes[ap].log_scale;
                if ui.selectable_label(log, egui::RichText::new(format!("{} Log Scale", check(log))).monospace().size(FONT_SM)).clicked() {
                    let shift = ui.input(|i| i.modifiers.shift); let nv = !log;
                    if shift || watchlist.broadcast_mode { for p in panes.iter_mut() { p.log_scale = nv; } } else { panes[ap].log_scale = nv; }
                }
            });
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
                    if ui.add(egui::Button::new(egui::RichText::new(auto_label).monospace().size(FONT_SM).color(if is_auto { t.accent } else { t.dim }))
                        .frame(false).min_size(egui::vec2(32.0, 16.0))).clicked() {
                        if is_auto {
                            panes[ap].renko_brick_size = Chart::auto_brick_size(&panes[ap].bars, 0.5);
                        } else {
                            panes[ap].renko_brick_size = 0.0;
                        }
                        panes[ap].alt_bars_dirty = true;
                    }
                    if !is_auto {
                        let mut val = panes[ap].renko_brick_size;
                        let resp = ui.add(egui::DragValue::new(&mut val).speed(0.01).range(0.01..=10000.0)
                            .custom_formatter(|v, _| format!("{:.2}", v))
                            .prefix("Brick: "));
                        if resp.changed() {
                            panes[ap].renko_brick_size = val;
                            panes[ap].alt_bars_dirty = true;
                        }
                    }
                }
                CandleMode::RangeBar => {
                    let is_auto = panes[ap].range_bar_size == 0.0;
                    let auto_label = if is_auto { "Auto" } else { "Manual" };
                    if ui.add(egui::Button::new(egui::RichText::new(auto_label).monospace().size(FONT_SM).color(if is_auto { t.accent } else { t.dim }))
                        .frame(false).min_size(egui::vec2(32.0, 16.0))).clicked() {
                        if is_auto {
                            panes[ap].range_bar_size = Chart::auto_brick_size(&panes[ap].bars, 1.0);
                        } else {
                            panes[ap].range_bar_size = 0.0;
                        }
                        panes[ap].alt_bars_dirty = true;
                    }
                    if !is_auto {
                        let mut val = panes[ap].range_bar_size;
                        let resp = ui.add(egui::DragValue::new(&mut val).speed(0.01).range(0.01..=10000.0)
                            .custom_formatter(|v, _| format!("{:.2}", v))
                            .prefix("Range: "));
                        if resp.changed() {
                            panes[ap].range_bar_size = val;
                            panes[ap].alt_bars_dirty = true;
                        }
                    }
                }
                CandleMode::TickBar => {
                    let mut val = panes[ap].tick_bar_count as i32;
                    let resp = ui.add(egui::DragValue::new(&mut val).speed(10).range(1..=100000)
                        .prefix("Ticks: "));
                    if resp.changed() {
                        panes[ap].tick_bar_count = val.max(1) as u32;
                        panes[ap].alt_bars_dirty = true;
                    }
                }
                _ => {}
            }

            // Moving Averages dropdown (always creates new instance — supports multiple)
            ui.menu_button(egui::RichText::new("MAs").monospace().size(FONT_MD).strong().color(t.dim), |ui| {
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
                        let label_text = format!("{} {} {}", if *evis { Icon::CHECK } else { "" }, ekind.label(), eperiod);
                        let c = hex_to_color(ecolor, 1.0);
                        ui.horizontal(|ui| {
                            ui.painter().circle_filled(egui::pos2(ui.cursor().min.x + 5.0, ui.cursor().min.y + 9.0), 3.0, c);
                            ui.add_space(gap_xl());
                            if ui.selectable_label(*evis, egui::RichText::new(label_text.trim()).monospace().size(FONT_SM)).clicked() {
                                let shift = ui.input(|i| i.modifiers.shift);
                                let nv = !*evis;
                                if shift || watchlist.broadcast_mode {
                                    for p in panes.iter_mut() {
                                        if let Some(ind) = p.indicators.iter_mut().find(|i| i.kind == *ekind && i.period == *eperiod) { ind.visible = nv; }
                                    }
                                } else {
                                    if let Some(ind) = panes[ap].indicators.iter_mut().find(|i| i.id == *eid) { ind.visible = nv; }
                                }
                            }
                            if ui.add(egui::Button::new(egui::RichText::new(Icon::PENCIL_LINE).size(FONT_SM).color(t.dim.gamma_multiply(0.5)))
                                .frame(false).min_size(BTN_ICON_SM)).clicked() {
                                panes[ap].editing_indicator = Some(*eid);
                            }
                            if ui.add(egui::Button::new(egui::RichText::new(Icon::X).size(FONT_SM).color(t.bear.gamma_multiply(0.5)))
                                .frame(false).min_size(BTN_ICON_SM)).clicked() {
                                let shift = ui.input(|i| i.modifiers.shift);
                                if shift || watchlist.broadcast_mode {
                                    for p in panes.iter_mut() {
                                        p.indicators.retain(|i| !(i.kind == *ekind && i.period == *eperiod));
                                        p.indicator_bar_count = 0;
                                    }
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
                    if ui.selectable_label(false, egui::RichText::new(format!("{} + {}", Icon::PLUS, label)).monospace().size(FONT_SM)).clicked() {
                        let shift = ui.input(|i| i.modifiers.shift);
                        if shift || watchlist.broadcast_mode {
                            for p in panes.iter_mut() {
                                let id = p.next_indicator_id; p.next_indicator_id += 1;
                                let color = INDICATOR_COLORS[p.indicators.len() % INDICATOR_COLORS.len()];
                                p.indicators.push(Indicator::new(id, itype, itype.default_period(), color));
                                p.indicator_bar_count = 0;
                            }
                            panes[ap].editing_indicator = Some(panes[ap].indicators.last().map_or(0, |i| i.id));
                        } else {
                            let id = panes[ap].next_indicator_id; panes[ap].next_indicator_id += 1;
                            let color = INDICATOR_COLORS[panes[ap].indicators.len() % INDICATOR_COLORS.len()];
                            panes[ap].indicators.push(Indicator::new(id, itype, itype.default_period(), color));
                            panes[ap].indicator_bar_count = 0;
                            panes[ap].editing_indicator = Some(id);
                        }
                    }
                }
                ui.separator();
                let ribbon_active = panes[ap].show_ma_ribbon;
                if ui.selectable_label(ribbon_active, egui::RichText::new(format!("{} MA Ribbon (8-89)", check(ribbon_active))).monospace().size(FONT_SM)).clicked() {
                    let shift = ui.input(|i| i.modifiers.shift);
                    let nv = !ribbon_active;
                    if shift || watchlist.broadcast_mode { for p in panes.iter_mut() { p.show_ma_ribbon = nv; } } else { panes[ap].show_ma_ribbon = nv; }
                }
            });

            // Oscillators dropdown (multi-select)
            ui.menu_button(egui::RichText::new("Osc").monospace().size(FONT_MD).strong().color(t.dim), |ui| {
                ui.style_mut().visuals.widgets.inactive.bg_fill = t.toolbar_bg;
                ui.style_mut().visuals.window_fill = t.toolbar_bg;
                let osc_types = [(IndicatorType::RSI, "RSI"), (IndicatorType::MACD, "MACD"),
                    (IndicatorType::Stochastic, "Stochastic"), (IndicatorType::CCI, "CCI"),
                    (IndicatorType::WilliamsR, "Williams %R"), (IndicatorType::ADX, "ADX"), (IndicatorType::ATR, "ATR")];
                for (itype, label) in osc_types {
                    let has = panes[ap].indicators.iter().any(|i| i.kind == itype && i.visible);
                    if ui.selectable_label(has, egui::RichText::new(format!("{} {}", check(has), label)).monospace().size(FONT_SM)).clicked() {
                        let shift = ui.input(|i| i.modifiers.shift);
                        if shift || watchlist.broadcast_mode {
                            for p in panes.iter_mut() {
                                let p_has = p.indicators.iter().any(|i| i.kind == itype && i.visible);
                                if has {
                                    if let Some(ind) = p.indicators.iter_mut().find(|i| i.kind == itype) { ind.visible = false; }
                                } else {
                                    if let Some(ind) = p.indicators.iter_mut().find(|i| i.kind == itype) { ind.visible = true; }
                                    else if !p_has {
                                        let id = p.next_indicator_id; p.next_indicator_id += 1;
                                        let color = INDICATOR_COLORS[p.indicators.len() % INDICATOR_COLORS.len()];
                                        p.indicators.push(Indicator::new(id, itype, itype.default_period(), color));
                                        p.indicator_bar_count = 0;
                                    }
                                }
                            }
                        } else {
                            if has {
                                if let Some(ind) = panes[ap].indicators.iter_mut().find(|i| i.kind == itype) { ind.visible = false; }
                            } else {
                                if let Some(ind) = panes[ap].indicators.iter_mut().find(|i| i.kind == itype) { ind.visible = true; }
                                else {
                                    let id = panes[ap].next_indicator_id; panes[ap].next_indicator_id += 1;
                                    let color = INDICATOR_COLORS[panes[ap].indicators.len() % INDICATOR_COLORS.len()];
                                    panes[ap].indicators.push(Indicator::new(id, itype, itype.default_period(), color));
                                    panes[ap].indicator_bar_count = 0;
                                }
                            }
                        }
                    }
                }
                ui.separator();
                let cvd = panes[ap].show_cvd;
                if ui.selectable_label(cvd, egui::RichText::new(format!("{} CVD", check(cvd))).monospace().size(FONT_SM)).clicked() {
                    let shift = ui.input(|i| i.modifiers.shift);
                    let nv = !cvd;
                    if shift || watchlist.broadcast_mode { for p in panes.iter_mut() { p.show_cvd = nv; } } else { panes[ap].show_cvd = nv; }
                }
            });

            // Volume dropdown
            ui.menu_button(egui::RichText::new("Vol").monospace().size(font_md()).strong().color(t.dim), |ui| {
                ui.style_mut().visuals.widgets.inactive.bg_fill = t.toolbar_bg;
                ui.style_mut().visuals.window_fill = t.toolbar_bg;
                let vol = panes[ap].show_volume;
                if ui.selectable_label(vol, egui::RichText::new(format!("{} Volume Bars", check(vol))).monospace().size(FONT_SM)).clicked() {
                    let shift = ui.input(|i| i.modifiers.shift); let nv = !vol;
                    if shift || watchlist.broadcast_mode { for p in panes.iter_mut() { p.show_volume = nv; } } else { panes[ap].show_volume = nv; }
                }
                let dvol = panes[ap].show_delta_volume;
                if ui.selectable_label(dvol, egui::RichText::new(format!("{} Delta Volume", check(dvol))).monospace().size(FONT_SM)).clicked() {
                    let shift = ui.input(|i| i.modifiers.shift); let nv = !dvol;
                    if shift || watchlist.broadcast_mode { for p in panes.iter_mut() { p.show_delta_volume = nv; } } else { panes[ap].show_delta_volume = nv; }
                }
                let rvol = panes[ap].show_rvol;
                if ui.selectable_label(rvol, egui::RichText::new(format!("{} Relative Volume", check(rvol))).monospace().size(FONT_SM)).clicked() {
                    let shift = ui.input(|i| i.modifiers.shift); let nv = !rvol;
                    if shift || watchlist.broadcast_mode { for p in panes.iter_mut() { p.show_rvol = nv; } } else { panes[ap].show_rvol = nv; }
                }
                ui.separator();
                ui.label(egui::RichText::new("Volume Profile").monospace().size(FONT_SM).color(t.dim));
                for (mode, label) in [
                    (VolumeProfileMode::Off, "Off"), (VolumeProfileMode::Classic, "Classic"),
                    (VolumeProfileMode::Heatmap, "Heatmap"), (VolumeProfileMode::Strip, "Strip"),
                    (VolumeProfileMode::Clean, "Clean (POC/VA)"),
                ] {
                    let active = panes[ap].vp_mode == mode;
                    if ui.selectable_label(active, egui::RichText::new(format!("{} {}", check(active), label)).monospace().size(FONT_SM)).clicked() {
                        panes[ap].vp_mode = mode; panes[ap].vp_data = None;
                    }
                }
            });

            // Overlays dropdown — two-layer with categories
            ui.menu_button(egui::RichText::new("Overlay").monospace().size(FONT_MD).strong().color(t.dim), |ui| {
                ui.style_mut().visuals.widgets.inactive.bg_fill = t.toolbar_bg;
                ui.style_mut().visuals.window_fill = t.toolbar_bg;
                ui.set_min_width(150.0);

                // ── Technical Overlays (indicator-based)
                ui.menu_button(egui::RichText::new("\u{2248} Technical").monospace().size(FONT_SM).color(t.dim), |ui| {
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
                        if ui.selectable_label(has, egui::RichText::new(format!("{} {}", check(has), label)).monospace().size(FONT_SM)).clicked() {
                            if has {
                                if let Some(ind) = panes[ap].indicators.iter_mut().find(|i| i.kind == itype) { ind.visible = false; }
                            } else {
                                if let Some(ind) = panes[ap].indicators.iter_mut().find(|i| i.kind == itype) { ind.visible = true; }
                                else {
                                    let id = panes[ap].next_indicator_id; panes[ap].next_indicator_id += 1;
                                    let color = INDICATOR_COLORS[panes[ap].indicators.len() % INDICATOR_COLORS.len()];
                                    panes[ap].indicators.push(Indicator::new(id, itype, itype.default_period(), color));
                                    panes[ap].indicator_bar_count = 0;
                                }
                            }
                        }
                    }
                    ui.separator();
                    let vwap = panes[ap].show_vwap_bands;
                    if ui.selectable_label(vwap, egui::RichText::new(format!("{} VWAP + Bands", check(vwap))).monospace().size(FONT_SM)).clicked() {
                        panes[ap].show_vwap_bands = !panes[ap].show_vwap_bands;
                    }
                    let sr = panes[ap].show_auto_sr;
                    if ui.selectable_label(sr, egui::RichText::new(format!("{} Auto S/R Levels", check(sr))).monospace().size(FONT_SM)).clicked() {
                        panes[ap].show_auto_sr = !panes[ap].show_auto_sr;
                    }
                });

                // ── Structure (S/R, volume, price levels)
                ui.menu_button(egui::RichText::new("\u{2261} Structure").monospace().size(FONT_SM).color(t.dim), |ui| {
                    ui.set_min_width(220.0);
                    macro_rules! overlay_toggle {
                        ($field:ident, $label:expr) => {
                            let v = panes[ap].$field;
                            if ui.selectable_label(v, egui::RichText::new(format!("{} {}", check(v), $label)).monospace().size(FONT_SM)).clicked() {
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
                    if ui.selectable_label(gamma, egui::RichText::new(format!("{} Gamma Levels (GEX)", check(gamma))).monospace().size(FONT_SM)).clicked() {
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
                ui.menu_button(egui::RichText::new("\u{224B} Regime").monospace().size(FONT_SM).color(t.dim), |ui| {
                    ui.set_min_width(220.0);
                    macro_rules! overlay_toggle {
                        ($field:ident, $label:expr) => {
                            let v = panes[ap].$field;
                            if ui.selectable_label(v, egui::RichText::new(format!("{} {}", check(v), $label)).monospace().size(FONT_SM)).clicked() {
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
                ui.menu_button(egui::RichText::new("\u{1F4CA} Data").monospace().size(FONT_SM).color(t.dim), |ui| {
                    ui.set_min_width(200.0);
                    let events = panes[ap].show_events;
                    if ui.selectable_label(events, egui::RichText::new(format!("{} Event Markers", check(events))).monospace().size(FONT_SM)).clicked() {
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
                    if ui.selectable_label(dp, egui::RichText::new(format!("{} Dark Pool Prints", check(dp))).monospace().size(FONT_SM)).clicked() {
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
                ui.label(egui::RichText::new("SYMBOL OVERLAY").monospace().size(FONT_SM).color(t.dim.gamma_multiply(0.5)));
                let mut remove_idx: Option<usize> = None;
                let mut edit_idx: Option<usize> = None;
                for (oi, ov) in panes[ap].symbol_overlays.iter().enumerate() {
                    ui.horizontal(|ui| {
                        let oc = hex_to_color(&ov.color, 1.0);
                        ui.painter().circle_filled(egui::pos2(ui.cursor().min.x + 5.0, ui.cursor().min.y + 9.0), 3.0, oc);
                        ui.add_space(gap_xl());
                        let label_resp = ui.label(egui::RichText::new(&ov.symbol).monospace().size(FONT_SM).color(oc));
                        if label_resp.double_clicked() { edit_idx = Some(oi); }
                        if ui.add(egui::Button::new(egui::RichText::new(Icon::X).size(FONT_SM).color(t.bear.gamma_multiply(0.5)))
                            .frame(false).min_size(BTN_ICON_SM)).clicked() {
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
                if ui.selectable_label(false, egui::RichText::new(format!("{} Add Symbol Overlay", Icon::PLUS)).monospace().size(FONT_SM)).clicked() {
                    watchlist.pending_overlay_add = true;
                }
            });

            // Tools dropdown — display tools and cursor enhancements
            ui.menu_button(egui::RichText::new("Tools").monospace().size(FONT_MD).strong().color(t.dim), |ui| {
                ui.style_mut().visuals.widgets.inactive.bg_fill = t.toolbar_bg;
                ui.style_mut().visuals.window_fill = t.toolbar_bg;

                ui.label(egui::RichText::new("DISPLAY").monospace().size(FONT_SM).color(t.dim.gamma_multiply(0.5)));
                let ohlc = panes[ap].ohlc_tooltip;
                if ui.selectable_label(ohlc, egui::RichText::new(format!("{} OHLC Tooltip", check(ohlc))).monospace().size(FONT_SM)).clicked() {
                    let shift = ui.input(|i| i.modifiers.shift); let nv = !ohlc;
                    if shift || watchlist.broadcast_mode { for p in panes.iter_mut() { p.ohlc_tooltip = nv; } } else { panes[ap].ohlc_tooltip = nv; }
                }
                let mt = panes[ap].measure_tooltip;
                if ui.selectable_label(mt, egui::RichText::new(format!("{} Measure Tooltip", check(mt))).monospace().size(FONT_SM)).clicked() {
                    let shift = ui.input(|i| i.modifiers.shift); let nv = !mt;
                    if shift || watchlist.broadcast_mode { for p in panes.iter_mut() { p.measure_tooltip = nv; } } else { panes[ap].measure_tooltip = nv; }
                }
                let pc = panes[ap].show_prev_close;
                if ui.selectable_label(pc, egui::RichText::new(format!("{} Prev Close / Open", check(pc))).monospace().size(FONT_SM)).clicked() {
                    let shift = ui.input(|i| i.modifiers.shift); let nv = !pc;
                    if shift || watchlist.broadcast_mode { for p in panes.iter_mut() { p.show_prev_close = nv; } } else { panes[ap].show_prev_close = nv; }
                }
                let pl = panes[ap].show_pattern_labels;
                if ui.selectable_label(pl, egui::RichText::new(format!("{} Pattern Labels", check(pl))).monospace().size(FONT_SM)).clicked() {
                    let shift = ui.input(|i| i.modifiers.shift); let nv = !pl;
                    if shift || watchlist.broadcast_mode { for p in panes.iter_mut() { p.show_pattern_labels = nv; } } else { panes[ap].show_pattern_labels = nv; }
                }
                let pnl = panes[ap].show_pnl_curve;
                if ui.selectable_label(pnl, egui::RichText::new(format!("{} P&L Curve", check(pnl))).monospace().size(FONT_SM)).clicked() { panes[ap].show_pnl_curve = !panes[ap].show_pnl_curve; }

                ui.separator();
                ui.label(egui::RichText::new("CURSOR").monospace().size(FONT_SM).color(t.dim.gamma_multiply(0.5)));
                let fp = panes[ap].show_footprint;
                if ui.selectable_label(fp, egui::RichText::new(format!("{} Footprint (hover)", check(fp))).monospace().size(FONT_SM)).clicked() {
                    let shift = ui.input(|i| i.modifiers.shift); let nv = !fp;
                    if shift || watchlist.broadcast_mode { for p in panes.iter_mut() { p.show_footprint = nv; } } else { panes[ap].show_footprint = nv; }
                }

                ui.separator();
                ui.label(egui::RichText::new("REPLAY").monospace().size(FONT_SM).color(t.dim.gamma_multiply(0.5)));
                let rpl = panes[ap].replay_mode;
                if ui.selectable_label(rpl, egui::RichText::new(format!("{} Bar Replay", check(rpl))).monospace().size(FONT_SM)).clicked() {
                    panes[ap].replay_mode = !panes[ap].replay_mode;
                    if panes[ap].replay_mode {
                        panes[ap].replay_bar_count = panes[ap].bars.len().min(50);
                        panes[ap].replay_playing = false;
                        panes[ap].indicator_bar_count = 0;
                    }
                }
            });
            // Deferred: open overlay editor after menu closes
            if watchlist.pending_overlay_add {
                watchlist.pending_overlay_add = false;
                panes[ap].overlay_editing = true;
                panes[ap].overlay_editing_idx = None;
            }

            // ── Suites dropdown (advanced analysis tools) ──
            ui.menu_button(egui::RichText::new("Suites").monospace().size(FONT_MD).strong().color(t.dim), |ui| {
                ui.style_mut().visuals.widgets.inactive.bg_fill = t.toolbar_bg;
                ui.style_mut().visuals.window_fill = t.toolbar_bg;
                let sl_mode = panes[ap].swing_leg_mode;
                let sl_active = sl_mode > 0;
                let sl_suffix = match sl_mode { 1 => " (Vertical)", 2 => " (Diagonal)", _ => "" };
                if ui.selectable_label(sl_active, egui::RichText::new(format!("{} SwingRange{}", check(sl_active), sl_suffix)).monospace().size(FONT_SM)).clicked() {
                    let shift = ui.input(|i| i.modifiers.shift); let nv = (sl_mode + 1) % 3;
                    if shift || watchlist.broadcast_mode { for p in panes.iter_mut() { p.swing_leg_mode = nv; } } else { panes[ap].swing_leg_mode = nv; }
                }
                let afib = panes[ap].show_auto_fib;
                if ui.selectable_label(afib, egui::RichText::new(format!("{} Auto Fibonacci", check(afib))).monospace().size(FONT_SM)).clicked() {
                    let shift = ui.input(|i| i.modifiers.shift); let nv = !afib;
                    if shift || watchlist.broadcast_mode { for p in panes.iter_mut() { p.show_auto_fib = nv; } } else { panes[ap].show_auto_fib = nv; }
                }
                ui.separator();
                ui.selectable_label(false, egui::RichText::new("  Triangulator").monospace().size(FONT_SM).color(t.dim.gamma_multiply(0.4)));
                ui.selectable_label(false, egui::RichText::new("  Auto Target").monospace().size(FONT_SM).color(t.dim.gamma_multiply(0.4)));
            });

            // ⚡ Hit Highlight icon toggle
            {
                let hh = panes[ap].hit_highlight;
                let hh_resp = ui.add(ToolbarBtn::new("SIGNALS").active(hh).theme(t)).on_hover_text("Hit Highlight");
                if hh_resp.clicked() { panes[ap].hit_highlight = !hh; }
            }

            // ── Widgets dropdown — two-layer categorized picker with mini previews ──
            ui.menu_button(egui::RichText::new("Widgets").monospace().size(FONT_MD).strong().color(t.dim), |ui| {
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

                    ui.menu_button(egui::RichText::new(&cat_label).monospace().size(FONT_SM)
                        .color(if active_in_cat > 0 { t.accent } else { t.dim }), |ui| {
                        ui.set_min_width(280.0);
                        ui.label(egui::RichText::new(*cat_name).monospace().size(font_xs()).strong().color(t.accent));
                        ui.add_space(gap_xs());

                        for &kind in *kinds {
                            let is_active = active_kinds.contains(&kind);
                            let item_h = 36.0;
                            let (_, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), item_h), egui::Sense::click());
                            let r = resp.rect;
                            let p = ui.painter();

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
                                kind.label(), egui::FontId::monospace(FONT_SM),
                                if is_active { t.text } else { t.dim });

                            // Description
                            let desc = widget_description(kind);
                            p.text(egui::pos2(name_x, r.top() + 23.0), egui::Align2::LEFT_CENTER,
                                desc, egui::FontId::monospace(7.0), t.dim.gamma_multiply(0.35));

                            // Active checkmark
                            if is_active {
                                p.text(egui::pos2(r.right() - 12.0, r.center().y),
                                    egui::Align2::CENTER_CENTER, "\u{2713}",
                                    egui::FontId::proportional(FONT_SM), t.accent);
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
                    if ui.selectable_label(false, egui::RichText::new("\u{1F5D1} Remove All Widgets")
                        .monospace().size(FONT_SM).color(t.bear)).clicked() {
                        panes[ap].chart_widgets.clear();
                        ui.close_menu();
                    }
                }
            });

            ui.add(egui::Separator::default().spacing(4.0));

            // ── Workspace — clean dropdown (templates moved to pane header ★ button) ──
            {
                let ws_names = list_workspaces();
                let ws_label = format!("{} {}", Icon::BROWSERS, &watchlist.active_workspace);
                ui.menu_button(egui::RichText::new(&ws_label).monospace().size(font_md()).strong().color(t.dim), |ui| {
                    ui.style_mut().visuals.widgets.inactive.bg_fill = t.toolbar_bg;
                    ui.style_mut().visuals.window_fill = t.toolbar_bg;
                    ui.set_min_width(200.0);

                    ui.label(egui::RichText::new("WORKSPACES").monospace().size(font_xs()).color(t.dim.gamma_multiply(0.5)));
                    ui.add_space(gap_sm());

                    // Workspace list
                    for name in &ws_names {
                        let is_active = *name == watchlist.active_workspace;
                        ui.horizontal(|ui| {
                            // Active dot
                            if is_active {
                                ui.label(egui::RichText::new("●").size(font_xs()).color(t.accent));
                            } else {
                                ui.label(egui::RichText::new("  ").size(font_xs()));
                            }
                            let label_col = if is_active { t.accent } else { t.dim };
                            if ui.selectable_label(is_active,
                                egui::RichText::new(name).monospace().size(FONT_SM).color(label_col)).clicked() && !is_active {
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
                            .monospace().size(FONT_SM).color(t.accent)).clicked() {
                            save_workspace(&watchlist.active_workspace, panes, *layout);
                            ui.close_menu();
                        }
                    }

                    // Save as new
                    ui.add_space(gap_sm());
                    ui.horizontal(|ui| {
                        ui.add(egui::TextEdit::singleline(&mut watchlist.workspace_save_name)
                            .hint_text("New workspace…")
                            .desired_width(130.0)
                            .font(egui::FontId::monospace(FONT_SM)));
                        let can_save = !watchlist.workspace_save_name.trim().is_empty();
                        if can_save {
                            if ui.add(egui::Button::new(egui::RichText::new("Save As").monospace().size(FONT_SM).color(t.accent)))
                                .clicked() {
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
                    ui.label(egui::RichText::new("Auto-saves every 30s").monospace().size(font_xs()).color(t.dim.gamma_multiply(0.3)));
                });
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
                let fav_layouts: Vec<&Layout> = ALL_LAYOUTS.iter()
                    .filter(|&&ly| watchlist.layout_favorites.iter().any(|f| f == ly.label()))
                    .collect();
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
                let dd_btn = ui.add(ToolbarBtn::new(Icon::CARET_DOWN).active(watchlist.layout_dropdown_open).theme(t));
                if dd_btn.clicked() {
                    watchlist.layout_dropdown_open = !watchlist.layout_dropdown_open;
                    watchlist.layout_dropdown_pos = egui::pos2(dd_btn.rect.left(), dd_btn.rect.bottom() + 2.0);
                }
            }
            // (Layout dropdown rendered after toolbar — see below)

            ui.add(egui::Separator::default().spacing(4.0));

            // ── Theme + Style dropdown — two columns separated by a divider ──
            {
                let mut ti = panes[ap].theme_idx;
                let style_presets = crate::chart_renderer::ui::style::list_style_presets();
                let safe_si = watchlist.style_idx.min(style_presets.len().saturating_sub(1));
                let style_name_cur = style_presets.get(safe_si).map(|(_, n)| n.as_str()).unwrap_or("Meridien");
                let mut si = safe_si;
                let combined = format!("{}/{}", get_theme(ti).name, style_name_cur);
                let current_label = egui::RichText::new(combined).monospace().size(FONT_MD).strong().color(t.dim);
                ui.menu_button(current_label, |ui| {
                    ui.style_mut().visuals.widgets.inactive.bg_fill = t.toolbar_bg;
                    ui.style_mut().visuals.window_fill = t.toolbar_bg;
                    ui.horizontal_top(|ui| {
                        // ── THEME column ──
                        ui.vertical(|ui| {
                            ui.set_min_width(160.0);
                            ui.label(egui::RichText::new("THEME").monospace().size(FONT_SM).color(t.dim.gamma_multiply(0.5)));
                            for (i, th) in THEMES.iter().enumerate() {
                                let sel = i == ti;
                                let row = ui.horizontal(|ui| {
                                    let (sr, _) = ui.allocate_exact_size(egui::vec2(16.0, 14.0), egui::Sense::hover());
                                    ui.painter().rect_filled(sr, 2.0, th.bg);
                                    ui.painter().circle_filled(egui::pos2(sr.left() + 4.0, sr.center().y), 2.5, th.bull);
                                    ui.painter().circle_filled(egui::pos2(sr.left() + 11.0, sr.center().y), 2.5, th.bear);
                                    let text_col = if sel { th.accent } else { t.dim };
                                    let check = if sel { "\u{2713} " } else { "  " };
                                    ui.selectable_label(sel, egui::RichText::new(format!("{}{}", check, th.name))
                                        .monospace().size(FONT_MD).color(text_col))
                                });
                                if row.inner.clicked() { ti = i; }
                            }
                        });
                        // Vertical separator
                        ui.add(egui::Separator::default().vertical().spacing(8.0));
                        // ── STYLE column — populated from live preset list ──
                        ui.vertical(|ui| {
                            ui.set_min_width(120.0);
                            ui.label(egui::RichText::new("STYLE").monospace().size(FONT_SM).color(t.dim.gamma_multiply(0.5)));
                            for (id, name) in &style_presets {
                                let sel = *id as usize == si;
                                let text_col = if sel { t.accent } else { t.dim };
                                let check = if sel { "\u{2713} " } else { "  " };
                                let r = ui.selectable_label(sel,
                                    egui::RichText::new(format!("{}{}", check, name))
                                        .monospace().size(FONT_MD).color(text_col));
                                if r.clicked() { si = *id as usize; }
                            }
                        });
                    });
                });
                if ti != panes[ap].theme_idx { for p in panes.iter_mut() { p.theme_idx = ti; } }
                if si != watchlist.style_idx { watchlist.style_idx = si; }
            }

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
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    if resp.clicked() { TB_BTN_CLICKED.with(|f| f.set(true)); }
                    (resp, r)
                };

                // Close — draw X with lines
                {
                    let (resp, r) = win_btn(ui, true);
                    let c = r.center();
                    let s = 4.5;
                    let col = if resp.hovered() { egui::Color32::WHITE } else { t.dim.gamma_multiply(0.7) };
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
                    let col = if resp.hovered() { t.dim } else { t.dim.gamma_multiply(0.7) };
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
                    let col = if resp.hovered() { t.dim } else { t.dim.gamma_multiply(0.7) };
                    ui.painter().line_segment([egui::pos2(c.x - s, c.y), egui::pos2(c.x + s, c.y)], egui::Stroke::new(STROKE_STD, col));
                    if resp.clicked() {
                        if let Some(w) = &win_ref { w.set_minimized(true); }
                    }
                }

                // Separator between window controls and panel toggles
                ui.add(egui::Separator::default().spacing(4.0));

                // Panel toggle buttons (right-to-left, so ordered right→left)
                ui.spacing_mut().item_spacing.x = gap_sm();


                // Connection status — small painted dot, no button frame
                {
                    let connected = account_data_cached.as_ref().map_or(false, |(s, _, _)| s.connected);
                    let (rect, resp) = ui.allocate_exact_size(egui::vec2(20.0, 20.0), egui::Sense::click());
                    let dot_color = if connected {
                        t.bull
                    } else {
                        rgb(230, 160, 40)
                    };
                    ui.painter().circle_filled(rect.center(), 3.0, dot_color);
                    if resp.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    let tip = if connected { "Connection: OK" } else { "Connection: Issue" };
                    let resp = resp.on_hover_text(tip);
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

                // Settings — always icon-only.
                if ui.add(ToolbarBtn::new(Icon::GEAR).active(watchlist.settings_open).theme(t)).on_hover_text("Settings").clicked() {
                    watchlist.settings_open = !watchlist.settings_open;
                }

                // SearchPill — command palette trigger (#6)
                {
                    let tb_h = tb_rect.height();
                    if super::super::status::SearchPill::new()
                        .height(tb_h - 14.0)
                        .theme(t)
                        .show(ui)
                        .clicked()
                    {
                        watchlist.cmd_palette_open = !watchlist.cmd_palette_open;
                    }
                }

                ui.add(egui::Separator::default().spacing(4.0));

                // Feed pane (News + Discord + Screenshots)
                if ui.add(ToolbarBtn::new(&nav_label(Icon::NEWSPAPER, "Feed")).active(watchlist.feed_panel_open).theme(t)).on_hover_text("Feed (News, Discord, Screenshots)").clicked() {
                    watchlist.feed_panel_open = !watchlist.feed_panel_open;
                }
                tb_group_break(ui, tb_rect, t.toolbar_border);

                // Playbook
                if ui.add(ToolbarBtn::new(&nav_label(Icon::STAR, "Playbook")).active(watchlist.playbook_panel_open).theme(t)).on_hover_text("Playbook (Trade Ideas)").clicked() {
                    watchlist.playbook_panel_open = !watchlist.playbook_panel_open;
                }
                tb_group_break(ui, tb_rect, t.toolbar_border);

                // Watchlist toggle
                if ui.add(ToolbarBtn::new(&nav_label(Icon::LIST, "Watchlist")).active(watchlist.open).theme(t)).on_hover_text("Watchlist").clicked() {
                    watchlist.open = !watchlist.open;
                }
                tb_group_break(ui, tb_rect, t.toolbar_border);

                // Analysis sidebar toggle (unified RRG / T&S / Scanner / Scripts)
                if ui.add(ToolbarBtn::new(&nav_label(Icon::CHART_LINE, "Analysis")).active(watchlist.analysis_open).theme(t)).on_hover_text("Analysis Sidebar").clicked() {
                    watchlist.analysis_open = !watchlist.analysis_open;
                }
                tb_group_break(ui, tb_rect, t.toolbar_border);

                // Signals pane (Alerts + Signals)
                {
                    let active_count = watchlist.alerts.iter().filter(|a| !a.triggered).count()
                        + panes.iter().flat_map(|p| p.price_alerts.iter()).filter(|a| !a.triggered && !a.draft).count();
                    let signals_resp = ui.add(ToolbarBtn::new(&nav_label(Icon::LIGHTNING, "Signals")).active(watchlist.signals_panel_open).theme(t)).on_hover_text("Signals (Alerts + Signals)");
                    if active_count > 0 {
                        let badge_x = signals_resp.rect.right() - 3.0;
                        let badge_y = signals_resp.rect.top() + 5.0;
                        ui.painter().circle_filled(egui::pos2(badge_x, badge_y), 5.0, t.accent);
                        ui.painter().text(egui::pos2(badge_x, badge_y), egui::Align2::CENTER_CENTER,
                            &format!("{}", active_count), egui::FontId::monospace(7.0), t.bg);
                    }
                    if signals_resp.clicked() { watchlist.signals_panel_open = !watchlist.signals_panel_open; }
                }
                tb_group_break(ui, tb_rect, t.toolbar_border);

                // New window — single icon button.
                if ui.add(ToolbarBtn::new(Icon::CIRCLES_THREE_PLUS).active(false).theme(t)).on_hover_text("New chart window").clicked() {
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
                            ui.label(egui::RichText::new(section).monospace().size(font_xs()).strong().color(t.dim.gamma_multiply(0.5)));
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
                        egui::FontId::monospace(11.0), lc,
                    );

                    // Star — toggles favorite without closing the dropdown
                    let sr = egui::Rect::from_min_size(egui::pos2(row_rect.right() - 22.0, row_rect.center().y - 8.0), egui::vec2(16.0, 16.0));
                    let sh = hover_pos.map_or(false, |p| sr.contains(p));
                    let sc = if is_fav { t.gold } else if sh { t.dim.gamma_multiply(0.5) } else if hovered { t.dim.gamma_multiply(0.2) } else { t.dim.gamma_multiply(0.08) };
                    ui.painter().text(sr.center(), egui::Align2::CENTER_CENTER, Icon::STAR_FILL, egui::FontId::proportional(11.0), sc);
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
                            ui.label(egui::RichText::new(sec).monospace().size(font_xs()).strong().color(t.dim.gamma_multiply(0.5)));
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
                    let gc = if is_cur { t.accent } else if hovered { t.dim } else { t.dim.gamma_multiply(0.5) };
                    let mini = ly.pane_rects(gr, ly.max_panes(), 0.5, 0.5, 0.5, 0.5);
                    for mr in &mini {
                        let s = egui::Rect::from_min_max(egui::pos2(mr.left() + 0.5, mr.top() + 0.5), egui::pos2(mr.right() - 0.5, mr.bottom() - 0.5));
                        ui.painter().rect_filled(s, 1.0, color_alpha(gc, 80));
                        ui.painter().rect_stroke(s, 1.0, egui::Stroke::new(stroke_thin(), color_alpha(gc, 150)), egui::StrokeKind::Outside);
                    }

                    // Label + description
                    let lc = if is_cur { t.accent } else if hovered { t.text } else { t.dim };
                    ui.painter().text(egui::pos2(row_rect.left() + 42.0, row_rect.center().y), egui::Align2::LEFT_CENTER, ly.label(), egui::FontId::monospace(9.0), lc);
                    let dc = if hovered { t.text_muted } else { t.dim.gamma_multiply(0.55) };
                    ui.painter().text(egui::pos2(row_rect.left() + 74.0, row_rect.center().y), egui::Align2::LEFT_CENTER, ly.description(), egui::FontId::monospace(10.0), dc);

                    // Star — filled, raw pointer click
                    let sr = egui::Rect::from_min_size(egui::pos2(row_rect.right() - 22.0, row_rect.center().y - 8.0), egui::vec2(16.0, 16.0));
                    let sh = hover_pos.map_or(false, |p| sr.contains(p));
                    let sc = if is_fav { t.gold } else if sh { t.dim.gamma_multiply(0.5) } else if hovered { t.dim.gamma_multiply(0.2) } else { t.dim.gamma_multiply(0.08) };
                    ui.painter().text(sr.center(), egui::Align2::CENTER_CENTER, Icon::STAR_FILL, egui::FontId::proportional(11.0), sc);
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
                if dialog_header(ui, "NEW GROUP", t.dim) { close_gm = true; }
                ui.add_space(gap_xl());
                let m = 10.0;
                ui.horizontal(|ui| {
                    ui.add_space(m);
                    let resp = ui.add(egui::TextEdit::singleline(&mut panes[ap].new_group_name)
                        .hint_text("Group name...").desired_width(230.0 - m * 2.0).font(egui::FontId::monospace(10.0)));
                    resp.request_focus();
                });
                ui.add_space(gap_lg());
                ui.horizontal(|ui| {
                    ui.add_space(m);
                    let can_create = !panes[ap].new_group_name.trim().is_empty();
                    if action_btn(ui, &format!("{} Create", Icon::PLUS), t.accent, can_create) {
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

    // ── Order execution toasts ───────────────────────────────────────────────
    if !toasts.is_empty() {
        let screen = ctx.screen_rect();
        for (i, (msg, _price, created, is_buy)) in toasts.iter().enumerate() {
            let age = created.elapsed().as_secs_f32();
            let alpha = ((5.0 - age) / 1.0).min(1.0).max(0.0); // fade out in last second
            if alpha <= 0.0 { continue; }
            let color = if *is_buy { t.bull } else { t.bear };
            let y_offset = screen.top() + 44.0 + i as f32 * 28.0;

            egui::Window::new(format!("toast_{}", i))
                .fixed_pos(egui::pos2(screen.center().x - 100.0, y_offset))
                .fixed_size(egui::vec2(200.0, 20.0))
                .title_bar(false)
                .frame(egui::Frame::popup(&ctx.style())
                    .fill(egui::Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), (40.0 * alpha) as u8))
                    .inner_margin(gap_sm()))
                .show(ctx, |ui| {
                    ui.label(egui::RichText::new(format!("{} {}", Icon::CHECK, msg)).monospace().size(font_sm())
                        .color(egui::Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), (255.0 * alpha) as u8)));
                });
        }
    }

    // ── Watchlist side panel
    crate::chart_renderer::ui::panels::watchlist_panel::draw(ctx, watchlist, panes, ap, t);

    // ── Object Tree side panel
    crate::chart_renderer::ui::panels::object_tree::draw(ctx, watchlist, panes, ap, t);

    // ── Book pane (Positions/Orders + Journal tabs) ─────────────────────────
    crate::chart_renderer::ui::panels::orders_panel::draw(ctx, watchlist, panes, ap, t, account_data_cached);

    // ── Scanner side panel
    crate::chart_renderer::ui::panels::scanner_panel::draw(ctx, watchlist, panes, ap, t);

    // ── Time & Sales side panel
    crate::chart_renderer::ui::panels::tape_panel::draw(ctx, watchlist, &panes[ap].symbol, t);

    // ── RRG (Relative Rotation Graph) side panel
    crate::chart_renderer::ui::panels::rrg_panel::draw(ctx, watchlist, t);

    // ── Analysis sidebar (unified RRG / T&S / Scanner / Scripts)
    crate::chart_renderer::ui::panels::analysis_panel::draw(ctx, watchlist, panes, *active_pane, t);

    // ── Signals sidebar (unified Alerts + Signals)
    crate::chart_renderer::ui::panels::signals_panel::draw(ctx, watchlist, panes, ap, t);

    // ── Feed sidebar (unified News + Discord + Screenshots)
    crate::chart_renderer::ui::panels::feed_panel::draw(ctx, watchlist, panes, ap, t);

    // ── Playbook sidebar
    crate::chart_renderer::ui::panels::playbook_panel::draw(ctx, watchlist, panes, ap, t);

    // ── Journal sidebar
    crate::chart_renderer::ui::panels::journal_panel::draw(ctx, watchlist, t);

    // ── Script / Backtesting panel
    crate::chart_renderer::ui::panels::script_panel::draw(ctx, watchlist, t);

    // ── Spread Builder panel
    crate::chart_renderer::ui::panels::spread_panel::draw(ctx, watchlist, &panes[ap].symbol, t);

    // ── Alert checking — run every frame, check if any alert prices were crossed ──
    {
        let active_prices: Vec<(String, f32)> = panes.iter()
            .filter_map(|p| p.bars.last().map(|b| (p.symbol.clone(), b.close)))
            .collect();
        for alert in &mut watchlist.alerts {
            if alert.triggered { continue; }
            if let Some((_, price)) = active_prices.iter().find(|(s, _)| *s == alert.symbol) {
                if (alert.above && *price >= alert.price) || (!alert.above && *price <= alert.price) {
                    alert.triggered = true;
                    let dir = if alert.above { "above" } else { "below" };
                    let msg = format!("ALERT: {} {} {:.2}", alert.symbol, dir, alert.price);
                    eprintln!("[ALERT TRIGGERED] {} -- sound notification placeholder", msg);
                    PENDING_TOASTS.with(|ts| ts.borrow_mut().push((msg, alert.price, alert.above)));
                }
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
                    ui.label(TextStyle::NumericLg.as_rich(&tip.sym, egui::Color32::WHITE));
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
                        ui.label(TextStyle::MonoSm.as_rich(&format!("{} Earnings in {} days", Icon::LIGHTNING, tip.earnings_days), t.gold));
                    }
                    if !tip.tags.is_empty() {
                        ui.add_space(gap_xs());
                        ui.horizontal_wrapped(|ui| { for tag in &tip.tags { ui.label(TextStyle::Caption.as_rich(tag, t.accent)); } });
                    }
                    if tip.alert_triggered {
                        ui.label(TextStyle::MonoSm.as_rich(&format!("{} Alert triggered", Icon::LIGHTNING), t.notification_red));
                    }
                });
            });
    }

    span_end(); // top_panel
}
