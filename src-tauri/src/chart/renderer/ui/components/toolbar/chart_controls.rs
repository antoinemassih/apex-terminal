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
use crate::chart_renderer::gpu::{
    Chart, Watchlist, Theme,
    TB_BTN_CLICKED,
    CandleMode, VolumeProfileMode,
    IndicatorType, IndicatorCategory, Indicator,
    EventMarker, DarkPoolPrint,
    indicator_default_color, GammaLevel,
    widget_description, paint_widget_preview,
};
use crate::chart_renderer::ui::style::{
    color_alpha, color_subtle, color_half, color_dim, hex_to_color, segmented_control,
    contrast_fg,
    alpha_faint, alpha_ghost,
    font_4xs, font_xs, font_sm, font_lg,
    mono_sm,
    gap_xs, gap_sm, gap_md, gap_lg, gap_xl,
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
        apply_menu_style, paint_nav_col_tint, publish_swing_leg_mode,
        publish_toggle, ALL_TIMEFRAMES, tf_to_secs,
    };
    use super::toolbar_btn;

    if panes.is_empty() { return; }
    let ap = ap.min(panes.len() - 1);
    ui.spacing_mut().item_spacing.x = 0.0;
    ui.spacing_mut().button_padding = egui::vec2(gap_md(), gap_sm());
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
                let tf_dd_btn = toolbar_btn(ui, Icon::CARET_DOWN, watchlist.timeframe_dropdown_open, t);
                Tooltip::new("Timeframe picker").show(ui, &tf_dd_btn, t);
                if tf_dd_btn.clicked() {
                    watchlist.timeframe_dropdown_open = !watchlist.timeframe_dropdown_open;
                    watchlist.timeframe_dropdown_pos = egui::pos2(tf_dd_btn.rect.left(), tf_dd_btn.rect.bottom() + 2.0);
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
            let tools_box = ButtonGroupBox::begin(ui);
            let tools_x0 = ui.cursor().left();

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
                paint_nav_col_tint(ui, tb_rect, drawing_menu.response.rect, t,
                    drawing_menu.response.hovered(), has_tool, "drawing");
                {
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

            if !bg_enclosed { crate::ui_kit::widgets::Separator::vertical().spacing(4.0).show(ui, t); }

            let _menu_font = mono_sm();

            // Alt chart type settings row
            match panes[ap].candle_mode {
                CandleMode::Renko => {
                    let is_auto = panes[ap].alt.renko_brick == 0.0;
                    let auto_label = if is_auto { "Auto" } else { "Manual" };
                    if KitButton::new(auto_label).variant(KitVariant::Ghost).size(KitSize::Sm)
                        .fg(if is_auto { t.accent } else { t.dim }).frameless(true)
                        .min_size(egui::vec2(32.0, 16.0)).show(ui, t).clicked() {
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
                        .min_size(egui::vec2(32.0, 16.0)).show(ui, t).clicked() {
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

            // ── Indicators dropdown ──
            let indicators_menu = KitButton::menu(Icon::CHART_LINE)
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
                    ui.separator();
                    let gamma = panes[ap].show_gamma;
                    if ui.add(SelectableRow::new("Gamma Levels (GEX)", gamma)).clicked() {
                        panes[ap].show_gamma = !panes[ap].show_gamma;
                        if panes[ap].show_gamma && panes[ap].gamma_levels.is_empty() {
                            // Real gamma/regime feed (gamma_feed_service / ApexSignals).
                            let gamma_sym = panes[ap].symbol.clone();
                            if let Some((levels, zero, cw, pw, _regime)) =
                                crate::chart_renderer::gpu::fetch_gamma_from_feed(&gamma_sym)
                            {
                                panes[ap].gamma_levels = levels;
                                panes[ap].gamma_zero = zero;
                                panes[ap].gamma_call_wall = cw;
                                panes[ap].gamma_put_wall = pw;
                                if let Some(last_bar) = panes[ap].bars.last() {
                                    panes[ap].gamma_hvl = last_bar.close;
                                }
                            } else if let Some(last_bar) = panes[ap].bars.last() {
                                let price = last_bar.close;
                                let step = if price > 200.0 { 5.0 } else if price > 50.0 { 2.5 } else { 1.0 };
                                let mut levels: Vec<GammaLevel> = vec![];
                                for i in -15..=15_i32 {
                                    let level_price = (price / step).round() * step + i as f32 * step;
                                    let dist = i.abs() as f32;
                                    let gex = if dist < 5.0 { (500.0 - dist * 80.0) * (1.0 + 0.3 * (level_price * 7.3).sin()) }
                                    else { (-100.0 - (dist - 5.0) * 50.0) * (1.0 + 0.2 * (level_price * 3.1).sin()) };
                                    levels.push(GammaLevel { price: level_price, exposure: gex });
                                }
                                let max_pos = levels.iter().filter(|l| l.exposure > 0.0).max_by(|a, b| a.exposure.partial_cmp(&b.exposure).unwrap_or(std::cmp::Ordering::Equal));
                                let max_neg = levels.iter().filter(|l| l.exposure < 0.0).min_by(|a, b| a.exposure.partial_cmp(&b.exposure).unwrap_or(std::cmp::Ordering::Equal));
                                panes[ap].gamma_call_wall = max_pos.map_or(price + 10.0 * step, |l| l.price);
                                panes[ap].gamma_put_wall  = max_neg.map_or(price - 10.0 * step, |l| l.price);
                                let mut zero = price;
                                for w in levels.windows(2) { if w[0].exposure >= 0.0 && w[1].exposure < 0.0 { zero = (w[0].price + w[1].price) / 2.0; break; } }
                                panes[ap].gamma_zero = zero;
                                panes[ap].gamma_hvl  = max_pos.map_or(price, |l| l.price);
                                panes[ap].gamma_levels = levels;
                            }
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
                ui.add(SelectableRow::new("Triangulator", false).disabled(true));
                ui.add(SelectableRow::new("Auto Target", false).disabled(true));
            });

            }); // end Indicators outer dropdown
            paint_nav_col_tint(ui, tb_rect, indicators_menu.response.rect, t,
                indicators_menu.response.hovered(), false, "indicators");
            {
                Tooltip::rich(|ui, theme| {
                    ui.label(egui::RichText::new("Indicators").size(font_sm()).strong().color(theme.text()));
                    ui.label(egui::RichText::new("MAs, Oscillators, Volume, Overlays, Tools, Suites").size(font_xs()).color(theme.dim()));
                }).show(ui, &indicators_menu.response, t);
            }

            if watchlist.pending_overlay_add {
                watchlist.pending_overlay_add = false;
                panes[ap].overlay_editing = true;
                panes[ap].overlay_editing_idx = None;
            }

            // ── Widgets dropdown ──
            let widgets_menu = KitButton::menu(Icon::CIRCLES_FOUR)
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
                                p.rect_filled(r, 4.0, tint(t, Tone::Accent, alpha_ghost()));
                            }

                            let preview_rect = egui::Rect::from_min_size(
                                egui::pos2(r.left() + 4.0, r.top() + 4.0), egui::vec2(28.0, 28.0));
                            let preview_bg = tint(t, Tone::Border, alpha_faint());
                            p.rect_filled(preview_rect, 4.0, preview_bg);
                            paint_widget_preview(p, preview_rect, kind, t, is_active);

                            let name_x = r.left() + 38.0;
                            p.text(egui::pos2(name_x, r.top() + 10.0), egui::Align2::LEFT_CENTER,
                                kind.label(), egui::FontId::monospace(font_sm()),
                                if is_active { t.text } else { t.dim });

                            let desc = widget_description(kind);
                            p.text(egui::pos2(name_x, r.top() + 23.0), egui::Align2::LEFT_CENTER,
                                desc, mono_sm(), color_dim(t.dim));

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
                Tooltip::rich(|ui, theme| {
                    ui.label(egui::RichText::new("Widgets").size(font_sm()).strong().color(theme.text()));
                    ui.label(egui::RichText::new("Add live data tiles to the chart").size(font_xs()).color(theme.dim()));
                }).show(ui, &widgets_menu.response, t);
            }

            if !bg_enclosed { crate::ui_kit::widgets::Separator::vertical().spacing(4.0).show(ui, t); }

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
            // Close the tools group box.
            let tools_x1 = ui.cursor().left();
            group_box(ui, tools_x0, tools_x1, tools_box);
}
