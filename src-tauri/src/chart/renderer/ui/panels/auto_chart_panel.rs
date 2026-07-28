//! Auto-Charting side panel — controls ApexSignals' detection layers,
//! methodology, and tuning. A standard rail-hosted `SidePanelShell` panel,
//! toggled from the top-right nav beside Analysis/Signals.
//!
//! All state lives in the global [`AutoDrawConfig`](crate::chart_renderer::gpu::AutoDrawConfig)
//! (persisted). Any change writes the config + re-fetches the active chart so
//! toggles/tuning take effect immediately.

use super::super::super::gpu::{Chart, Theme, Watchlist};
use crate::chart_renderer::ui::panels::side_panel_shell::{RailSlot, SidePanelShell, Width};
use crate::ui_kit::widgets::{Button, Checkbox}; // WS-G G3: design-system primitives
use egui;

/// Record a panel control into the dev-inspector widget-tree so the harness can
/// assert it renders (rect/label/clip) and click it by id. Debug-only; a no-op
/// in release. `value` carries the control's current state (reflects config).
#[cfg(debug_assertions)]
fn rec_ctrl(id: &str, role: &str, label: &str, resp: &egui::Response, ui: &egui::Ui, value: String) {
    let mut w = crate::dev_inspector::WidgetRecord::from_response(id, role, label, resp, ui);
    w.value = Some(value);
    crate::dev_inspector::record(w);
}
#[cfg(not(debug_assertions))]
#[inline] fn rec_ctrl(_: &str, _: &str, _: &str, _: &egui::Response, _: &egui::Ui, _: String) {}

/// Rail registration — one line in `right_rail::PANELS`.
pub(crate) const RAIL: super::right_rail::RailPanelDef = super::right_rail::RailPanelDef {
    id: "auto_chart",
    is_open: |w| w.auto_chart_open,
    render: |cx, slot| {
        draw(cx.ctx, cx.watchlist, cx.panes, cx.active_pane, cx.t, Some(slot));
    },
};

pub(crate) fn draw(
    ctx: &egui::Context,
    watchlist: &mut Watchlist,
    panes: &mut [Chart],
    ap: usize,
    t: &Theme,
    slot: Option<RailSlot>,
) -> bool {
    if !watchlist.auto_chart_open {
        return false;
    }

    let pane_h = crate::chart_renderer::gpu::pane_tabs_header_h(watchlist);
    let pane_font = watchlist.pane_header_size.title_font();

    let mut active = 0u8;
    let tabs = [(0u8, "AUTO-CHART", None)];
    let resp = SidePanelShell::tabs("auto_chart_panel", &mut active, &tabs)
        .width(Width::Narrow)
        .resizable(220.0..=420.0)
        .pane_metrics(pane_h, pane_font)
        .rail_slot(slot)
        .show(ctx, t, |ui, _t, _tab| {
            let (sym, tf) = if !panes.is_empty() {
                (panes[ap].symbol.clone(), panes[ap].timeframe.clone())
            } else {
                (String::new(), String::new())
            };

            let mut cfg = crate::chart_renderer::gpu::auto_draw_config();
            let before = cfg.clone();

            ui.add_space(6.0);
            let r = Checkbox::new(&mut cfg.enabled).label("Auto-charting ON").show(ui, t);
            rec_ctrl("auto_chart.enabled", "checkbox", "Auto-charting ON", &r, ui, cfg.enabled.to_string());
            ui.separator();
            if cfg.enabled {
                ui.label("Window of operation");
                let r = ui.add(egui::Slider::new(&mut cfg.window, 100..=2000).text("bars back"));
                rec_ctrl("auto_chart.window", "slider", "bars back", &r, ui, cfg.window.to_string());
                let r = Checkbox::new(&mut cfg.anchored_only).label("Anchored only (no floating starts)").show(ui, t);
                rec_ctrl("auto_chart.anchored_only", "checkbox", "Anchored only", &r, ui, cfg.anchored_only.to_string());
                ui.separator();
                ui.label("Layers");
                let r = Checkbox::new(&mut cfg.trendlines).label("Trendlines").show(ui, t);
                rec_ctrl("auto_chart.trendlines", "checkbox", "Trendlines", &r, ui, cfg.trendlines.to_string());
                let r = Checkbox::new(&mut cfg.channels).label("Channels").show(ui, t);
                rec_ctrl("auto_chart.channels", "checkbox", "Channels", &r, ui, cfg.channels.to_string());
                let r = Checkbox::new(&mut cfg.levels).label("Levels").show(ui, t);
                rec_ctrl("auto_chart.levels", "checkbox", "Levels", &r, ui, cfg.levels.to_string());
                let r = Checkbox::new(&mut cfg.patterns).label("Chart patterns").show(ui, t);
                rec_ctrl("auto_chart.patterns", "checkbox", "Chart patterns", &r, ui, cfg.patterns.to_string());
                let r = Checkbox::new(&mut cfg.candles).label("Candlesticks").show(ui, t);
                rec_ctrl("auto_chart.candles", "checkbox", "Candlesticks", &r, ui, cfg.candles.to_string());
                ui.separator();
                ui.label("Pivot method");
                ui.horizontal(|ui| {
                    for m in ["hybrid", "atr", "percent"] {
                        let r = Button::toggle(m, cfg.pivot_mode == m).show(ui, t);
                        rec_ctrl(&format!("auto_chart.pivot.{m}"), "toggle", m, &r, ui, (cfg.pivot_mode == m).to_string());
                        if r.clicked() { cfg.pivot_mode = m.to_string(); }
                    }
                });
                ui.separator();
                ui.label("Tuning");
                ui.add(egui::Slider::new(&mut cfg.atr_k, 0.5..=6.0).text("ATR x"));
                ui.add(egui::Slider::new(&mut cfg.pct, 0.0..=0.05).text("% move"));
                ui.add(egui::Slider::new(&mut cfg.min_touches, 2..=6).text("min touches"));
                ui.add(egui::Slider::new(&mut cfg.max_lines, 4..=30).text("max lines"));
                ui.add(egui::Slider::new(&mut cfg.sensitivity, 0.001..=0.02).text("sensitivity"));
                ui.add(egui::Slider::new(&mut cfg.lookback, 50..=400).text("lookback"));
                ui.add(egui::Slider::new(&mut cfg.swing_window, 2..=12).text("swing window"));

                ui.separator();
                ui.label("Extend lines");
                ui.horizontal(|ui| {
                    for e in ["none", "right", "both", "left"] {
                        let r = Button::toggle(e, cfg.extend == e).show(ui, t);
                        rec_ctrl(&format!("auto_chart.extend.{e}"), "toggle", e, &r, ui, (cfg.extend == e).to_string());
                        if r.clicked() { cfg.extend = e.to_string(); }
                    }
                });

                ui.separator();
                ui.label("Methods (compare — see what works)");
                for (id, label) in [
                    ("wick", "Wick swing-pairs"), ("body", "Body"), ("inner", "Inner"),
                    ("volume", "Volume-weighted"), ("anchored", "Anchored"),
                    ("regression", "Regression"), ("fibfan", "Fib fan"), ("speed", "Speed lines"),
                    ("hough", "Hough transform"), ("ransac", "RANSAC"), ("kalman", "Kalman"),
                    ("kde", "KDE levels"), ("cusum", "CUSUM"), ("wavelet", "Wavelet"),
                    ("pca", "PCA"), ("tls", "Total least squares"), ("svd", "SVD"),
                    ("bayesian", "Bayesian"),
                ] {
                    let mut on = cfg.methods.iter().any(|m| m == id);
                    if Checkbox::new(&mut on).label(label).show(ui, t).changed() {
                        if on {
                            cfg.methods.push(id.to_string());
                        } else {
                            cfg.methods.retain(|m| m != id);
                        }
                    }
                }
            }

            // ── Drawing feedback list ─────────────────────────────────────
            // Show active drawings with Reject button. Rejected lines are
            // hidden immediately and POSTed to the engine for learning.
            if cfg.enabled && !panes.is_empty() {
                let drawings: Vec<(String, String, String)> = panes[ap]
                    .signal_drawings
                    .iter()
                    .filter(|d| !cfg.rejected_drawings.contains(&d.id))
                    .map(|d| (d.id.clone(), d.detection_method.clone(), d.drawing_type.clone()))
                    .collect();

                if !drawings.is_empty() {
                    ui.separator();
                    ui.label(format!("Active drawings ({})", drawings.len()));
                    let red = t.bear;
                    egui::ScrollArea::vertical()
                        .id_source("drawings_scroll")
                        .max_height(180.0)
                        .show(ui, |ui| {
                            let green = t.bull;
                            for (id, method, dtype) in &drawings {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        egui::RichText::new(format!("{} / {}", method, dtype))
                                            .small(),
                                    );
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        if ui.small_button(
                                            egui::RichText::new(crate::ui_kit::icons::Icon::X).color(red)
                                        ).on_hover_text("Reject — hide and log for learning").clicked() {
                                            cfg.rejected_drawings.insert(id.clone());
                                            crate::chart_renderer::gpu::post_drawing_feedback(
                                                id.clone(),
                                                "reject".to_string(),
                                                sym.clone(),
                                                tf.clone(),
                                            );
                                        }
                                        if ui.small_button(
                                            egui::RichText::new(crate::ui_kit::icons::Icon::CHECK).color(green)
                                        ).on_hover_text("Accept — confirm this line is good").clicked() {
                                            crate::chart_renderer::gpu::post_drawing_feedback(
                                                id.clone(),
                                                "accept".to_string(),
                                                sym.clone(),
                                                tf.clone(),
                                            );
                                        }
                                    });
                                });
                            }
                        });

                    if ui.small_button("Clear all rejections").clicked() {
                        cfg.rejected_drawings.clear();
                    }
                }
            }

            if cfg != before {
                crate::chart_renderer::gpu::set_auto_draw_config(cfg);
                if !sym.is_empty() {
                    crate::chart_renderer::gpu::fetch_apexsignals_drawings(sym.clone(), tf.clone());
                }
            }
        });

    if resp.close_clicked {
        watchlist.update_sidebar_state(|s| s.auto_chart_open = false);
    }
    true
}
