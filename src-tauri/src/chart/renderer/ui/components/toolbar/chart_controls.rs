//! Chart-controls cluster rendering — extracted from `top_nav.rs`.
//!
//! Owns the 7 controls that live in the toolnav (toolbar row 2):
//!   1. Interval buttons (favorites segmented-control + dropdown caret)
//!   2. Drawing dropdown (tools menu + broadcast/object-tree toggles)
//!   3. Alt-bar settings row (Renko / RangeBar / TickBar steppers)
//!   4. Indicators dropdown (MAs / Osc / Vol / Overlays / Tools / Suites)
//!   5. Widgets dropdown (categorised picker with mini previews)
//!   6. Magnet snap toggle
//!   7. Hit-alert toggle
//!
//! Call via the re-exported `render_chart_controls` shim in `top_nav.rs`.
//! All helpers shared with `top_nav.rs` are referenced via `super::top_nav::`.

#![allow(unused_imports, unused_variables)]

use crate::ui_kit::icons::Icon;
use crate::chart_renderer::ui::style::tint;
use crate::ui_kit::sx::Tone;
use crate::ui_kit::widgets::{
    Button as KitButton, MenuItem, NumberStepper, SelectableRow, Tooltip,
    tokens::{Variant as KitVariant, Size as KitSize},
};
use crate::ui_kit::widgets::icon_placement::IconPlacement;
use crate::chart_renderer::ui::foundation::text_style::TextStyle;
use crate::chart_renderer::gpu::{
    Chart, Watchlist, Theme,
    TB_BTN_CLICKED,
    CandleMode, VolumeProfileMode,
    IndicatorType, IndicatorCategory, Indicator,
    EventMarker, DarkPoolPrint,
    indicator_default_color,
    widget_description, paint_widget_preview,
};
use crate::chart_renderer::ui::style::{
    color_alpha, color_subtle, color_half, color_dim, hex_to_color, segmented_control,
    contrast_fg,
    alpha_faint, alpha_ghost,
    font_4xs, font_xs, font_sm, font_lg,
    mono_sm,
    gap_xs, gap_sm, gap_md, gap_lg, gap_xl,
    radius_sm,
};
use crate::chart_renderer::{ChartWidget, ChartWidgetKind, DrawingGroup};
use crate::state::{BROADCAST_GROUP, PaneEvent, PaneToggle};
use crate::chart_renderer::commands::{self as commands, AppCommand, ChartFlag};

/// Render the full chart-controls cluster into `ui`.
///
/// This is the extracted body of the old `render_chart_controls` in `top_nav.rs`.
/// Called from the `render_chart_controls` shim that remains in `top_nav.rs` for
/// backwards compatibility.
pub(crate) fn render(
    ui: &mut egui::Ui,
    watchlist: &mut Watchlist,
    panes: &mut [Chart],
    ap: usize,
    t: &Theme,
    tb_rect: egui::Rect,
) {
    use super::top_nav::{
        apply_menu_style, publish_swing_leg_mode,
        publish_toggle, ALL_TIMEFRAMES, tf_to_secs,
    };
    use super::toolbar_btn;

    if panes.is_empty() { return; }
    let ap = ap.min(panes.len() - 1);
    ui.spacing_mut().item_spacing.x = 0.0;
    ui.spacing_mut().button_padding = egui::vec2(gap_md(), gap_sm());

    // ── Toolbar runs ONE TIER BELOW body (subtree cascade override) ─────────
    //
    // The chart toolbar is dense chrome: a single fixed-width row that has to
    // fit ~10 labelled menus. When the app-wide type scale was lifted
    // (font_sm 11->12, font_md 13->14) this row overflowed and egui clipped the
    // last two buttons — caught by the corpus as
    // `clipped: toolbar.indicators_btn, toolbar.widgets_btn` in both the design
    // and UX audit baselines. Unit tests and screenshots both missed it.
    //
    // The fix is the cascade doing the job it exists for: re-point the tiers
    // for THIS SUBTREE only. Every widget below inherits it with no call site
    // changed, and the global scale (which made the rest of the app readable)
    // is untouched. This is precisely what hand-passed FontIds could never do.
    {
        use crate::chart_renderer::ui::foundation::text_style::TextStyle as Tier;
        let s = ui.style_mut();
        for (tier, size) in [
            (Tier::Body,    crate::ui_kit::style::font_sm()),
            (Tier::BodySm,  crate::ui_kit::style::font_xs()),
            (Tier::Label,   crate::ui_kit::style::font_xs()),
            (Tier::Caption, crate::ui_kit::style::font_xs()),
        ] {
            // `font_id_at` keeps the tier's own family — the call site never
            // builds a FontId and so cannot flip mono/proportional by accident.
            s.text_styles.insert(tier.egui(), tier.font_id_at(size));
        }
    }
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
    // Button-group enclosure: when the active style draws group boxes (Aperture)
    // we wrap each section in a rounded rect and drop the internal separators.
    use crate::chart_renderer::ui::style::{button_group_enclosed, ButtonGroupBox};
    let bg_enclosed = button_group_enclosed();
    let group_box = |ui: &mut egui::Ui, x0: f32, x1: f32, b: ButtonGroupBox| {
        b.end(ui, t, egui::Rect::from_min_max(
            egui::pos2(x0, tb_rect.top()), egui::pos2(x1, tb_rect.bottom())), tb_rect);
    };

            // ── Interval group (own button-section box) ──
            let interval_box = ButtonGroupBox::begin(ui);
            let interval_x0 = ui.cursor().left();
            // ── Interval buttons — favorites segmented control + dropdown caret ──
            ui.add_space(gap_xs());
            {
                let cur_secs = tf_to_secs(&panes[ap].timeframe);
                let fav_tfs: Vec<&'static str> = ALL_TIMEFRAMES.iter()
                    .map(|t| t.0)
                    .filter(|tf| watchlist.timeframe.favorites.iter().any(|f| f == tf))
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
                let tf_dd_btn = toolbar_btn(ui, Icon::CARET_DOWN, watchlist.timeframe.dropdown_open, t);
                Tooltip::new("Timeframe picker").show(ui, &tf_dd_btn, t);
                if tf_dd_btn.clicked() {
                    watchlist.timeframe.dropdown_open = !watchlist.timeframe.dropdown_open;
                    watchlist.timeframe.dropdown_pos = egui::pos2(tf_dd_btn.rect.left(), tf_dd_btn.rect.bottom() + crate::ui_kit::style::gap_2xs());
                }
                // Dev Inspector — record the active timeframe and its dropdown trigger.
                #[cfg(debug_assertions)]
                {
                    let active_tf = panes[ap].timeframe.clone();
                    crate::dev_inspector::record(crate::dev_inspector::WidgetRecord::from_response(
                        "toolbar.timeframe_picker", "button", &active_tf, &tf_dd_btn, ui,
                    ));
                    crate::dev_inspector::check_contract(
                        "toolbar.timeframe_picker",
                        tf_dd_btn.rect,
                        crate::dev_inspector::layout::Contract::new().touch_target(28.0),
                    );
                }
            }
            // Close the interval group box.
            let interval_x1 = ui.cursor().left();
            group_box(ui, interval_x0, interval_x1, interval_box);

            // Separator between interval and tools — replaced by box-gap when enclosed.
            if bg_enclosed {
                ui.add_space(gap_md());
            } else {
                crate::ui_kit::widgets::Separator::vertical().spacing(4.0).show(ui, t);
            }

            // ── Tools group (own button-section box: drawing → hit) ──
            //
            // RESPONSIVE OVERFLOW
            // ───────────────────
            // This row used to be a fixed-width run of ~7 groups. When the
            // central area shrank (a second pane, or an `egui::SidePanel` such
            // as auto-chart stealing width) the trailing groups were silently
            // CLIPPED — the corpus caught it as
            // `clipped: toolbar.indicators_btn, toolbar.widgets_btn`.
            //
            // The fix mirrors `panels::kit::PanelHeaderTabs` (kit.rs ~489):
            // reserve room for the overflow control up front, lay groups out in
            // priority order while they fit, and once ONE group does not fit
            // that group and every remaining one move into a `»` menu instead
            // of being drawn (and clipped) inline.
            //
            // Group widths can't be known before rendering an imperative egui
            // run, so each group's consumed width is measured as it is drawn and
            // cached in ctx temp data; the next frame budgets with the measured
            // value. This converges in one frame and cannot oscillate: a group
            // that overflows keeps its last measured width, so the decision is
            // stable.
            let tools_box = ButtonGroupBox::begin(ui);
            let tools_x0 = ui.cursor().left();

            let _menu_font = mono_sm();

            // Priority order (inline longest → overflow first). Interval and
            // drawing are the load-bearing controls; magnet + hit-alert are the
            // most expendable; indicators/widgets sit in between. Alt-bar
            // settings only exist in Renko/Range/Tick modes, and when they do
            // they are mode-critical, so they rank just under drawing.
            let alt_visible = matches!(
                panes[ap].candle_mode,
                CandleMode::Renko | CandleMode::RangeBar | CandleMode::TickBar
            );
            let order: Vec<Grp> = Grp::ALL.iter().copied()
                .filter(|g| *g != Grp::AltBar || alt_visible)
                .collect();

            let ctx = ui.ctx().clone();
            let overflow_w = cached_w(&ctx, OVERFLOW_KEY, OVERFLOW_EST_W);
            // Hard right edge: whichever of the toolbar rect / live clip rect is
            // tighter. `WidgetRecord::from_response` compares against the clip
            // rect, so that is the edge the corpus assertion actually uses.
            // The 1px inset keeps a group whose cached width lags its actual
            // width by a fraction of a px from landing exactly on the edge.
            let limit = tb_rect.right().min(ui.clip_rect().right()) - 1.0;

            let mut overflowed: Vec<Grp> = Vec::new();
            // The `!bg_enclosed` hairlines belong to the boundary BEFORE a
            // group, so they are only painted when a group actually follows
            // them inline (no dangling separator in front of the `»`).
            let mut sep_pending = false;
            for (i, g) in order.iter().copied().enumerate() {
                if !overflowed.is_empty() { overflowed.push(g); continue; }
                let x0 = ui.cursor().left();
                // Only pay the `»` reserve when a `»` is actually going to be
                // drawn. If every group still queued fits, the row uses its full
                // width; otherwise hold back the button's own width plus
                // `OVERFLOW_PAD`, which absorbs sub-pixel drift between a cached
                // width and the width the group consumes this frame.
                let rest: f32 = order[i..].iter()
                    .map(|g| cached_w(&ctx, g.key(), g.est_width()))
                    .sum();
                let reserve = if x0 + rest <= limit { 0.0 } else { overflow_w + OVERFLOW_PAD };
                if x0 + cached_w(&ctx, g.key(), g.est_width()) > limit - reserve {
                    overflowed.push(g);
                    continue;
                }
                if sep_pending {
                    crate::ui_kit::widgets::Separator::vertical().spacing(4.0).show(ui, t);
                    sep_pending = false;
                }
                render_group(g, ui, watchlist, panes, ap, t);
                store_w(&ctx, g.key(), ui.cursor().left() - x0);
                if !bg_enclosed && matches!(g, Grp::Drawing | Grp::Widgets) {
                    sep_pending = true;
                }
            }

            // Close the tools group box. `ButtonGroupBox::end` no-ops below 4px,
            // so an entirely overflowed tools run draws no empty enclosure.
            let tools_x1 = ui.cursor().left();
            group_box(ui, tools_x0, tools_x1, tools_box);

            // ── `»` overflow menu ──
            if !overflowed.is_empty() {
                let ox0 = ui.cursor().left();
                let hidden = overflowed.clone();
                let overflow_menu = KitButton::menu("\u{00BB}")
                    .glyph_size(font_lg())
                    .fg(t.accent)
                    .show_menu(ui, t, |ui| {
                        apply_menu_style(ui, t);
                        ui.set_min_width(190.0);
                        for (hi, g) in hidden.iter().copied().enumerate() {
                            if hi > 0 { ui.add_space(gap_xs()); }
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new(g.title())
                                    .size(font_sm()).color(t.dim));
                                ui.add_space(gap_sm());
                                render_group(g, ui, watchlist, panes, ap, t);
                            });
                        }
                    });
                {
                    let sub = format!(
                        "{} control group(s) don't fit — click to use them",
                        overflowed.len(),
                    );
                    Tooltip::rich(move |ui, theme| {
                        ui.label(TextStyle::BodySm
                            .as_rich_cascading("More controls", theme.text()).strong());
                        ui.label(TextStyle::Caption.as_rich_cascading(sub.as_str(), theme.dim()));
                    }).show(ui, &overflow_menu.response, t);
                }
                store_w(&ctx, OVERFLOW_KEY, ui.cursor().left() - ox0);
                #[cfg(debug_assertions)]
                {
                    let n = overflowed.len().to_string();
                    crate::dev_inspector::record(
                        crate::dev_inspector::WidgetRecord::from_response(
                            "toolbar.overflow_btn", "button", &n, &overflow_menu.response, ui,
                        ).with_style("toolbar"),
                    );
                }
            }

            // Consumed here (not inside the indicators group) so it works
            // whether the Overlay menu was reached inline or via `»`.
            if watchlist.pending_overlay_add {
                watchlist.pending_overlay_add = false;
                panes[ap].overlay_editing = true;
                panes[ap].overlay_editing_idx = None;
            }


    // ── Remove-all-widgets confirmation modal (U0-6) ──
    let rw_id = egui::Id::new("confirm_remove_widgets");
    if let Some(pane_idx) = ui.ctx().data(|d| d.get_temp::<usize>(rw_id)) {
        use crate::ui_kit::widgets::{ConfirmDialog, ConfirmOutcome, ConfirmTone};
        let resp = ConfirmDialog::new("Remove all widgets?")
            .id("confirm_remove_widgets_dlg")
            .body("Every widget on this pane will be removed.")
            .confirm("Remove all", ConfirmTone::Danger)
            .show(ui.ctx(), t);
        match resp.outcome {
            ConfirmOutcome::Confirmed => {
                if let Some(p) = panes.get_mut(pane_idx) { p.chart_widgets.clear(); }
                ui.ctx().data_mut(|d| d.remove::<usize>(rw_id));
            }
            ConfirmOutcome::Cancelled => { ui.ctx().data_mut(|d| d.remove::<usize>(rw_id)); }
            ConfirmOutcome::Open => {}
        }
    }
}


// ── Responsive group model ───────────────────────────────────────────────────
//
// The toolbar's control groups, in PRIORITY order (first = kept inline
// longest, last = first to be pushed into the `»` overflow menu). The interval
// picker is not listed: it is rank-0 and always renders inline.

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum Grp {
    /// Drawing-tool menu + object tree + broadcast toggles.
    Drawing,
    /// Renko / RangeBar / TickBar steppers (only present in those modes).
    AltBar,
    Indicators,
    Widgets,
    Magnet,
    Hit,
}

/// Width-cache key for the `»` button itself.
const OVERFLOW_KEY: &str = "overflow";
/// First-frame estimate for the `»` button (converges to the measured width).
const OVERFLOW_EST_W: f32 = 40.0;
/// Extra slack kept beside the `»` reserve so sub-pixel drift between a group's
/// cached width and its live width can never push the `»` past the clip edge.
const OVERFLOW_PAD: f32 = 4.0;

impl Grp {
    const ALL: [Grp; 6] = [
        Grp::Drawing, Grp::AltBar, Grp::Indicators, Grp::Widgets, Grp::Magnet, Grp::Hit,
    ];

    /// Stable width-cache key.
    fn key(self) -> &'static str {
        match self {
            Grp::Drawing    => "drawing",
            Grp::AltBar     => "altbar",
            Grp::Indicators => "indicators",
            Grp::Widgets    => "widgets",
            Grp::Magnet     => "magnet",
            Grp::Hit        => "hit",
        }
    }

    /// Label shown next to the group when it lives in the overflow menu —
    /// the inline form is icon-only, which reads as noise in a vertical list.
    fn title(self) -> &'static str {
        match self {
            Grp::Drawing    => "Drawing",
            Grp::AltBar     => "Bar settings",
            Grp::Indicators => "Indicators",
            Grp::Widgets    => "Widgets",
            Grp::Magnet     => "Magnet snap",
            Grp::Hit        => "Hit alerts",
        }
    }

    /// First-frame width estimate, used only until the group has been measured
    /// once. Deliberately generous so frame 1 errs toward the overflow menu
    /// rather than toward clipping.
    fn est_width(self) -> f32 {
        match self {
            Grp::Drawing    => 104.0,
            Grp::AltBar     => 160.0,
            Grp::Indicators => 44.0,
            Grp::Widgets    => 44.0,
            Grp::Magnet     => 32.0,
            Grp::Hit        => 32.0,
        }
    }
}

fn grp_w_id(key: &str) -> egui::Id { egui::Id::new(("apex_toolbar_grp_w", key)) }

/// Last measured width for `key`, or `default` if it has never been drawn.
fn cached_w(ctx: &egui::Context, key: &str, default: f32) -> f32 {
    ctx.data(|d| d.get_temp::<f32>(grp_w_id(key))).unwrap_or(default)
}

fn store_w(ctx: &egui::Context, key: &str, w: f32) {
    if w.is_finite() && w >= 0.0 {
        ctx.data_mut(|d| d.insert_temp(grp_w_id(key), w));
    }
}

/// Draw one group into `ui`. Identical code path inline and inside the `»`
/// menu — every click handler, dropdown-state flag, tooltip and
/// `dev_inspector::record` call travels with the group.
fn render_group(
    g: Grp,
    ui: &mut egui::Ui,
    watchlist: &mut Watchlist,
    panes: &mut [Chart],
    ap: usize,
    t: &Theme,
) {
    match g {
        Grp::Drawing    => grp_drawing(ui, watchlist, panes, ap, t),
        Grp::AltBar     => grp_alt_bar(ui, panes, ap, t),
        Grp::Indicators => grp_indicators(ui, watchlist, panes, ap, t),
        Grp::Widgets    => grp_widgets(ui, panes, ap, t),
        Grp::Magnet     => grp_magnet(ui, panes, ap, t),
        Grp::Hit        => grp_hit_alert(ui, watchlist, panes, ap, t),
    }
}

// ── Group bodies (moved verbatim out of `render`) ────────────────────────────

fn grp_drawing(
    ui: &mut egui::Ui, watchlist: &mut Watchlist, panes: &mut [Chart], ap: usize, t: &Theme,
) {
    use super::top_nav::apply_menu_style;
    use super::toolbar_btn;
    // ── Draw dropdown ──
    {
        let draw_label = Icon::PENCIL_LINE;
        let has_tool = !panes[ap].draw_tool.is_empty();
        let cur_tool = panes[ap].draw_tool.clone();
        let mut new_tool: Option<String> = None;
        let drawing_menu = KitButton::menu(draw_label)
            .glyph_size(font_lg())
            .fg(if has_tool { t.accent } else { t.dim })
            .show_menu(ui, t, |ui| {
            apply_menu_style(ui, t);
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
            let tool_shortcut = |tool_name: &str| -> Option<String> {
                let action = format!("tool_{}", tool_name);
                watchlist.hotkeys.iter().find(|hk| hk.action == action).map(|hk| hk.key_name.clone())
            };
            for (si, (section, tools)) in sections.iter().enumerate() {
                if si > 0 { ui.separator(); }
                ui.label(egui::RichText::new(*section).monospace().size(font_sm()).color(t.dim));
                for (tool, label) in *tools {
                    let mut item = MenuItem::new(*label);
                    if let Some(key) = tool_shortcut(tool) {
                        item = item.shortcut(key);
                    }
                    if item.show(ui, t).clicked() {
                        new_tool = Some(tool.to_string());
                        ui.close_menu();
                    }
                }
            }
            if !cur.is_empty() {
                ui.separator();
                if MenuItem::new("Cancel Tool").show(ui, t).clicked() {
                    new_tool = Some(String::new());
                    ui.close_menu();
                }
            }
        });
        {
            Tooltip::rich(|ui, theme| {
                ui.label(TextStyle::BodySm.as_rich_cascading("Drawing Tools", theme.text()).strong());
                ui.label(TextStyle::Caption.as_rich_cascading("Lines, channels, fibs, patterns", theme.dim()));
            }).show(ui, &drawing_menu.response, t);
        }
        if let Some(tool) = new_tool {
            panes[ap].draw_tool = tool;
            panes[ap].pending_pt = None; panes[ap].pending_pt2 = None; panes[ap].pending_pts.clear();
        }
        TB_BTN_CLICKED.with(|f| f.set(true));
    }
    // ── Drawing-section toggles ──
    {
        let prev_sp = ui.spacing().item_spacing.x;
        let prev_pad = ui.spacing().button_padding;
        ui.spacing_mut().item_spacing.x = gap_xs();
        ui.spacing_mut().button_padding = egui::vec2(gap_sm(), gap_sm());

        {
            let draw_count = panes[ap].drawings.len();
            let tree_resp = toolbar_btn(ui, Icon::TREE_STRUCTURE, watchlist.object_tree_open, t);
            Tooltip::new("Object Tree").show(ui, &tree_resp, t);
            if draw_count > 0 {
                let painter = ui.painter();
                let r = tree_resp.rect;
                let badge_center = egui::pos2(r.right() - crate::ui_kit::style::gap_2xs(), r.top() + 3.0);
                let badge_r = 5.0_f32;
                painter.circle_filled(badge_center, badge_r, t.accent);
                painter.text(
                    badge_center,
                    egui::Align2::CENTER_CENTER,
                    draw_count.to_string(),
                    crate::ui_kit::style::prop_at(font_4xs()),
                    contrast_fg(t.accent),
                );
            }
            if tree_resp.clicked() {
                watchlist.update_sidebar_state(|s| s.object_tree_open = !s.object_tree_open);
            }
        }

        {
            let bc = watchlist.broadcast_mode;
            let r = toolbar_btn(ui, Icon::BROADCAST, bc, t);
            Tooltip::new("Broadcast — changes apply to all panes").show(ui, &r, t);
            if r.clicked() {
                watchlist.broadcast_mode = !watchlist.broadcast_mode;
                TB_BTN_CLICKED.with(|f| f.set(true));
            }
        }

        ui.spacing_mut().item_spacing.x = prev_sp;
        ui.spacing_mut().button_padding = prev_pad;
    }
}

fn grp_alt_bar(ui: &mut egui::Ui, panes: &mut [Chart], ap: usize, t: &Theme) {
    // Alt chart type settings row
    match panes[ap].candle_mode {
        CandleMode::Renko => {
            let is_auto = panes[ap].alt.renko_brick == 0.0;
            let auto_label = if is_auto { "Auto" } else { "Manual" };
            if KitButton::new(auto_label).variant(KitVariant::Ghost).size(KitSize::Sm)
                .fg(if is_auto { t.accent } else { t.dim }).frameless(true)
                .min_size(egui::vec2(32.0, KitSize::Xs.height())).show(ui, t).clicked() {
                if is_auto {
                    panes[ap].alt.renko_brick = Chart::auto_brick_size(&panes[ap].bars, 0.5);
                } else {
                    panes[ap].alt.renko_brick = 0.0;
                }
                panes[ap].alt.dirty = true;
            }
            if !is_auto {
                let mut val = panes[ap].alt.renko_brick;
                let resp = NumberStepper::new(&mut val).step(0.01).range(0.01..=10000.0).decimals(2).prefix("Brick: ").show(ui, t);
                if resp.changed() {
                    panes[ap].alt.renko_brick = val;
                    panes[ap].alt.dirty = true;
                }
            }
        }
        CandleMode::RangeBar => {
            let is_auto = panes[ap].alt.range_size == 0.0;
            let auto_label = if is_auto { "Auto" } else { "Manual" };
            if KitButton::new(auto_label).variant(KitVariant::Ghost).size(KitSize::Sm)
                .fg(if is_auto { t.accent } else { t.dim }).frameless(true)
                .min_size(egui::vec2(32.0, KitSize::Xs.height())).show(ui, t).clicked() {
                if is_auto {
                    panes[ap].alt.range_size = Chart::auto_brick_size(&panes[ap].bars, 1.0);
                } else {
                    panes[ap].alt.range_size = 0.0;
                }
                panes[ap].alt.dirty = true;
            }
            if !is_auto {
                let mut val = panes[ap].alt.range_size;
                let resp = NumberStepper::new(&mut val).step(0.01).range(0.01..=10000.0).decimals(2).prefix("Range: ").show(ui, t);
                if resp.changed() {
                    panes[ap].alt.range_size = val;
                    panes[ap].alt.dirty = true;
                }
            }
        }
        CandleMode::TickBar => {
            let mut val = panes[ap].alt.tick_count as i32;
            let resp = NumberStepper::new(&mut val).step(10.0).range(1..=100000).prefix("Ticks: ").integer().show(ui, t);
            if resp.changed() {
                panes[ap].alt.tick_count = val.max(1) as u32;
                panes[ap].alt.dirty = true;
            }
        }
        _ => {}
    }
}

fn grp_indicators(
    ui: &mut egui::Ui, watchlist: &mut Watchlist, panes: &mut [Chart], ap: usize, t: &Theme,
) {
    use super::top_nav::{apply_menu_style, publish_swing_leg_mode, publish_toggle};
    // ── Indicators dropdown ──
    let indicators_menu = KitButton::menu(Icon::CHART_LINE)
        // Share the row's one control height (see toolbar_control_h).
        .min_size(egui::vec2(0.0, crate::chart_renderer::ui::style::toolbar_control_h()))
        .glyph_size(font_lg())
        .show_menu(ui, t, |ui| {
        apply_menu_style(ui, t);

    KitButton::menu("MAs").show_menu(ui, t, |ui| {
        apply_menu_style(ui, t);
        let ma_types = [(IndicatorType::SMA, "SMA"), (IndicatorType::EMA, "EMA"), (IndicatorType::WMA, "WMA"),
            (IndicatorType::DEMA, "DEMA"), (IndicatorType::TEMA, "TEMA"), (IndicatorType::VWAP, "VWAP")];
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
                    if r.clicked() { panes[ap].editing_indicator = Some(*eid); }
                    let r = KitButton::icon(Icon::X).variant(KitVariant::MutedIcon).placement(IconPlacement::Toolbar).tone_destructive().show(ui, t);
                    Tooltip::new("Remove indicator").show(ui, &r, t);
                    if r.clicked() {
                        let shift = ui.input(|i| i.modifiers.shift);
                        let fan = shift || watchlist.broadcast_mode;
                        if fan {
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
        for (itype, label) in ma_types {
            if ui.add(SelectableRow::new(label, false).leading_icon(Icon::PLUS)).clicked() {
                let shift = ui.input(|i| i.modifiers.shift);
                let fan = shift || watchlist.broadcast_mode;
                let id = panes[ap].next_indicator_id; panes[ap].next_indicator_id += 1;
                let color_owned = indicator_default_color(panes[ap].indicators.len(), t);
                let new_ind = Indicator::new(id, itype, itype.default_period(), &color_owned);
                panes[ap].indicators.push(new_ind.clone());
                panes[ap].indicator_bar_count = 0;
                panes[ap].editing_indicator = Some(id);
                if fan {
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

    KitButton::menu("Osc").show_menu(ui, t, |ui| {
        apply_menu_style(ui, t);
        let osc_types = [(IndicatorType::RSI, "RSI"), (IndicatorType::MACD, "MACD"),
            (IndicatorType::Stochastic, "Stochastic"), (IndicatorType::CCI, "CCI"),
            (IndicatorType::WilliamsR, "Williams %R"), (IndicatorType::ADX, "ADX"), (IndicatorType::ATR, "ATR")];
        for (itype, label) in osc_types {
            let has = panes[ap].indicators.iter().any(|i| i.kind == itype && i.visible);
            if ui.add(SelectableRow::new(label, has)).clicked() {
                let shift = ui.input(|i| i.modifiers.shift);
                let fan = shift || watchlist.broadcast_mode;
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

    KitButton::menu("Vol").show_menu(ui, t, |ui| {
        apply_menu_style(ui, t);
        let vol = panes[ap].show_volume;
        if ui.add(SelectableRow::new("Volume Bars", vol)).clicked() {
            let shift = ui.input(|i| i.modifiers.shift); let nv = !vol;
            let fan = shift || watchlist.broadcast_mode;
            commands::push(AppCommand::SetChartFlag { pane: ap, flag: ChartFlag::ShowVolume, value: nv });
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
            let active = panes[ap].vp.mode == mode;
            if ui.add(SelectableRow::new(label, active)).clicked() {
                panes[ap].vp.mode = mode; panes[ap].vp.data = None;
            }
        }
    });

    KitButton::menu("Overlay").show_menu(ui, t, |ui| {
        apply_menu_style(ui, t);
        ui.set_min_width(150.0);

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
            if ui.add(SelectableRow::new("VWAP + Bands", vwap)).clicked() { panes[ap].show_vwap_bands = !panes[ap].show_vwap_bands; }
            let sr = panes[ap].show_auto_sr;
            if ui.add(SelectableRow::new("Auto S/R Levels", sr)).clicked() { panes[ap].show_auto_sr = !panes[ap].show_auto_sr; }
        });

        KitButton::menu("Structure").leading_icon(Icon::TREE_STRUCTURE_FILL).trailing_icon(Icon::CARET_RIGHT).show_menu(ui, t, |ui| {
            ui.set_min_width(220.0);
            macro_rules! overlay_toggle {
                ($field:ident, $label:expr) => {
                    let v = panes[ap].$field;
                    if ui.add(SelectableRow::new($label, v)).clicked() { panes[ap].$field = !v; }
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
            // Dividends & splits (ApexData corporate actions). Fetched
            // inline on enable, like the gamma overlay below.
            {
                let on = panes[ap].show_corp_actions;
                if ui.add(SelectableRow::new("Dividends & Splits", on)).clicked() {
                    panes[ap].show_corp_actions = !on;
                    if panes[ap].show_corp_actions && panes[ap].corp_actions.is_empty() {
                        let sym = panes[ap].symbol.clone();
                        panes[ap].corp_actions =
                            crate::chart_renderer::gpu::fetch_corp_actions(&sym);
                    }
                }
            }
            ui.separator();
            let gamma = panes[ap].show_gamma;
            if ui.add(SelectableRow::new("Gamma Levels (GEX)", gamma)).clicked() {
                panes[ap].show_gamma = !panes[ap].show_gamma;
                if panes[ap].show_gamma {
                    // Shared feed-or-synth path (see Chart::populate_gamma).
                    // UI prefers the real feed; synthesizes only if absent.
                    panes[ap].populate_gamma(false);
                }
            }
        });

        KitButton::menu("Regime").leading_icon(Icon::BROADCAST_FILL).trailing_icon(Icon::CARET_RIGHT).show_menu(ui, t, |ui| {
            ui.set_min_width(220.0);
            macro_rules! overlay_toggle {
                ($field:ident, $label:expr) => {
                    let v = panes[ap].$field;
                    if ui.add(SelectableRow::new($label, v)).clicked() { panes[ap].$field = !v; }
                }
            }
            overlay_toggle!(show_momentum_heat, "Momentum Heatmap");
            overlay_toggle!(show_trend_strip, "Trend Alignment Strip");
            overlay_toggle!(show_breadth_tint, "Breadth Tint");
            overlay_toggle!(show_vol_cone, "Volatility Cone");
            overlay_toggle!(show_corr_ribbon, "Correlation Ribbon");
        });

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
                if r.clicked() { remove_idx = Some(oi); }
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

    KitButton::menu("Tools").show_menu(ui, t, |ui| {
        apply_menu_style(ui, t);
        ui.label(egui::RichText::new("DISPLAY").monospace().size(font_sm()).color(color_half(t.dim)));
        let ohlc = panes[ap].ohlc_tooltip;
        if ui.add(SelectableRow::new("OHLC Tooltip", ohlc)).clicked() {
            let shift = ui.input(|i| i.modifiers.shift); let nv = !ohlc;
            let fan = shift || watchlist.broadcast_mode;
            commands::push(AppCommand::SetChartFlag { pane: ap, flag: ChartFlag::OhlcTooltip, value: nv });
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
            commands::push(AppCommand::SetChartFlag { pane: ap, flag: ChartFlag::ShowPatternLabels, value: nv });
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
            commands::push(AppCommand::SetChartFlag { pane: ap, flag: ChartFlag::ShowFootprint, value: nv });
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

    KitButton::menu("Suites").show_menu(ui, t, |ui| {
        apply_menu_style(ui, t);
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
        ui.add(SelectableRow::new("Triangulator (soon)", false).disabled(true));
        ui.add(SelectableRow::new("Auto Target (soon)", false).disabled(true));
    });

    }); // end Indicators outer dropdown
    {
        Tooltip::rich(|ui, theme| {
            ui.label(TextStyle::BodySm.as_rich_cascading("Indicators", theme.text()).strong());
            ui.label(TextStyle::Caption.as_rich_cascading("MAs, Oscillators, Volume, Overlays, Tools, Suites", theme.dim()));
        }).show(ui, &indicators_menu.response, t);
    }
    #[cfg(debug_assertions)]
    {
        let ind_count = panes[ap].indicators.len().to_string();
        crate::dev_inspector::record(
            crate::dev_inspector::WidgetRecord::from_response(
                "toolbar.indicators_btn", "button", &ind_count, &indicators_menu.response, ui,
            ).with_style("toolbar"),
        );
    }
}

fn grp_widgets(ui: &mut egui::Ui, panes: &mut [Chart], ap: usize, t: &Theme) {
    use super::top_nav::apply_menu_style;
    // ── Widgets dropdown ──
    let widgets_menu = KitButton::menu(Icon::CIRCLES_FOUR)
        .min_size(egui::vec2(0.0, crate::chart_renderer::ui::style::toolbar_control_h()))
        .glyph_size(font_lg())
        .show_menu(ui, t, |ui| {
        apply_menu_style(ui, t);
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
                        p.rect_filled(r, radius_sm(), tint(t, Tone::Accent, alpha_ghost()));
                    }

                    let preview_rect = egui::Rect::from_min_size(
                        egui::pos2(r.left() + crate::ui_kit::style::gap_xs(), r.top() + crate::ui_kit::style::gap_xs()), egui::vec2(28.0, 28.0));
                    let preview_bg = tint(t, Tone::Border, alpha_faint());
                    p.rect_filled(preview_rect, radius_sm(), preview_bg);
                    paint_widget_preview(p, preview_rect, kind, t, is_active);

                    let name_x = r.left() + 38.0;
                    p.text(egui::pos2(name_x, r.top() + 10.0), egui::Align2::LEFT_CENTER,
                        kind.label(), crate::ui_kit::style::mono_sm(),
                        if is_active { t.text } else { t.dim });

                    let desc = widget_description(kind);
                    p.text(egui::pos2(name_x, r.top() + 23.0), egui::Align2::LEFT_CENTER,
                        desc, mono_sm(), color_dim(t.dim));

                    if is_active {
                        p.text(egui::pos2(r.right() - crate::ui_kit::style::gap_md(), r.center().y),
                            egui::Align2::CENTER_CENTER, "\u{2713}",
                            crate::ui_kit::style::prop_at(crate::ui_kit::style::font_sm()), t.accent);
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

        // Divider only when there is actually a row below it to divide
        // from — otherwise this painted a trailing hairline at the very
        // bottom of the menu, separating nothing.
        if !panes[ap].chart_widgets.is_empty() {
            ui.add_space(gap_sm());
            ui.separator();
            if ui.add(SelectableRow::new("Remove All Widgets", false).leading_icon(Icon::TRASH)).clicked() {
                // U0-6: defer to a confirmation modal (removes every widget
                // on the pane at once). Pending state lives in egui temp
                // memory keyed by pane index; the modal renders at fn end.
                ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new("confirm_remove_widgets"), ap));
                ui.close_menu();
            }
        }
    });
    {
        Tooltip::rich(|ui, theme| {
            ui.label(TextStyle::BodySm.as_rich_cascading("Widgets", theme.text()).strong());
            ui.label(TextStyle::Caption.as_rich_cascading("Add live data tiles to the chart", theme.dim()));
        }).show(ui, &widgets_menu.response, t);
    }
    #[cfg(debug_assertions)]
    {
        let widget_count = panes[ap].chart_widgets.len().to_string();
        crate::dev_inspector::record(
            crate::dev_inspector::WidgetRecord::from_response(
                "toolbar.widgets_btn", "button", &widget_count, &widgets_menu.response, ui,
            ).with_style("toolbar"),
        );
    }
}

fn grp_magnet(ui: &mut egui::Ui, panes: &mut [Chart], ap: usize, t: &Theme) {
    use super::toolbar_btn;
    // ── Magnet snap ──
    {
        let cur_magnet = panes[ap].magnet;
        let r = toolbar_btn(ui, Icon::MAGNET, cur_magnet, t);
        Tooltip::new("Magnet snap").show(ui, &r, t);
        if r.clicked() {
            crate::chart_renderer::commands::push(
                crate::chart_renderer::commands::AppCommand::SetChartFlag {
                    pane: ap,
                    flag: crate::chart_renderer::commands::ChartFlag::Magnet,
                    value: !cur_magnet,
                },
            );
        }
    }
}

fn grp_hit_alert(
    ui: &mut egui::Ui, watchlist: &mut Watchlist, panes: &mut [Chart], ap: usize, t: &Theme,
) {
    use super::top_nav::publish_toggle;
    use super::toolbar_btn;
    // ── Hit-alert toggle ──
    {
        let cur_hit = panes[ap].hit_highlight;
        let cur_broadcast = watchlist.broadcast_mode;
        let r = toolbar_btn(ui, Icon::LINE_SEGMENT, cur_hit, t);
        Tooltip::new("Hit alerts — trendline / swing flash").show(ui, &r, t);
        if r.clicked() {
            let shift = ui.input(|i| i.modifiers.shift);
            let v = !cur_hit;
            panes[ap].hit_highlight = v;
            publish_toggle(
                watchlist, shift || cur_broadcast,
                crate::state::subscriptions::PaneToggle::HitHighlight, v, ap,
            );
        }
    }
}
