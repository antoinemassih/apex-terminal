//! Auto-Charting side panel — controls ApexSignals' detection layers,
//! methodology, and tuning. A standard rail-hosted `SidePanelShell` panel,
//! toggled from the top-right nav beside Analysis/Signals.
//!
//! All state lives in the global [`AutoDrawConfig`](crate::chart_renderer::gpu::AutoDrawConfig)
//! (persisted). Any change writes the config + re-fetches the active chart so
//! toggles/tuning take effect immediately.

use super::super::super::gpu::{Chart, Theme, Watchlist};
use crate::chart_renderer::ui::panels::side_panel_shell::{RailSlot, SidePanelShell, Width};
use egui;

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
            ui.checkbox(&mut cfg.enabled, "Auto-charting ON");
            ui.separator();
            if cfg.enabled {
                ui.label("Layers");
                ui.checkbox(&mut cfg.trendlines, "Trendlines");
                ui.checkbox(&mut cfg.channels, "Channels");
                ui.checkbox(&mut cfg.levels, "Levels");
                ui.checkbox(&mut cfg.patterns, "Chart patterns");
                ui.checkbox(&mut cfg.candles, "Candlesticks");
                ui.separator();
                ui.label("Pivot method");
                ui.horizontal(|ui| {
                    for m in ["hybrid", "atr", "percent"] {
                        if ui.selectable_label(cfg.pivot_mode == m, m).clicked() {
                            cfg.pivot_mode = m.to_string();
                        }
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
                        if ui.selectable_label(cfg.extend == e, e).clicked() {
                            cfg.extend = e.to_string();
                        }
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
                    if ui.checkbox(&mut on, label).changed() {
                        if on {
                            cfg.methods.push(id.to_string());
                        } else {
                            cfg.methods.retain(|m| m != id);
                        }
                    }
                }
            }

            if cfg != before {
                crate::chart_renderer::gpu::set_auto_draw_config(cfg);
                if !sym.is_empty() {
                    crate::chart_renderer::gpu::fetch_apexsignals_drawings(sym, tf);
                }
            }
        });

    if resp.close_clicked {
        watchlist.update_sidebar_state(|s| s.auto_chart_open = false);
    }
    true
}
