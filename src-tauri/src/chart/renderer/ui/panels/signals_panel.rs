//! Signals panel — sidebar with subdivided sections, each with its own tab bar.
//!
//! Chrome (outer side panel, header, "+", tab strips, dividers, close-X)
//! is delegated to [`SplitSectionPanel`](crate::ui_kit::widgets::SplitSectionPanel).
//! This module is now responsible only for tab definitions and per-tab
//! body dispatch.

use egui;
use super::super::style::*;
use super::super::widgets as widgets;
use super::super::super::gpu::{Watchlist, Chart, Theme};
use crate::chart_renderer::SignalsTab;
use crate::ui_kit::icons::Icon;
use crate::ui_kit::widgets::side_panel_shell::Width;
use crate::ui_kit::widgets::{Button, SplitSectionPanel};

const ALL_TABS: &[(SignalsTab, &str)] = &[
    (SignalsTab::Alerts, "Alerts"),
    (SignalsTab::Signals, "Signals"),
];

pub(crate) fn draw(
    ctx: &egui::Context,
    watchlist: &mut Watchlist,
    panes: &mut [Chart],
    ap: usize,
    t: &Theme,
) {
    if !watchlist.signals_panel_open { return; }

    // Snapshot the active tab per section so the body closure can dispatch
    // without holding a borrow into `watchlist.signals_splits` (which the
    // widget owns mutably for the duration of `.show()`).
    let tab_snapshot: Vec<SignalsTab> =
        watchlist.signals_splits.iter().map(|s| s.tab).collect();

    // Move splits out of watchlist so the body closure can take `&mut watchlist`
    // freely (child draw_content panels require it). Restore after `.show()`.
    let mut splits = std::mem::take(&mut watchlist.signals_splits);
    let pane_h = crate::chart_renderer::gpu::pane_tabs_header_h(watchlist);
    let pane_font = watchlist.pane_header_size.title_font();

    let resp = SplitSectionPanel::new("signals_panel", &mut splits)
        .title("SIGNALS")
        .tabs(ALL_TABS)
        .default_tab(SignalsTab::Alerts)
        .width(Width::Narrow)
        .resizable(240.0..=420.0)
        .pane_metrics(pane_h, pane_font)
        .show(ctx, t, |ui, t, i, _frac| {
            let tab = tab_snapshot.get(i).copied().unwrap_or(SignalsTab::Alerts);
            match tab {
                SignalsTab::Alerts =>
                    super::alerts_panel::draw_content(ui, watchlist, panes, ap, t),
                SignalsTab::Signals =>
                    draw_signals_toggles(ui, panes, ap, t),
            }
        });

    watchlist.signals_splits = splits;
    if resp.close_clicked { watchlist.signals_panel_open = false; }
}

/// Per-signal visibility toggles.
fn draw_signals_toggles(ui: &mut egui::Ui, panes: &mut [Chart], ap: usize, t: &Theme) {
    ui.add_space(gap_sm());

    let chart = &mut panes[ap];
    let demo_on = chart.trend_health_score > 0.0 || chart.precursor_active || chart.trade_plan.is_some();
    ui.horizontal(|ui| {
        ui.add(widgets::text::SectionLabel::new("DEMO SIGNALS").tiny().color(t.dim));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let label = if demo_on { "Stop Demo" } else { "Start Demo" };
            let color = if demo_on { t.bear } else { t.accent };
            if Button::small_action(label).tint(color).show(ui, t).clicked() { chart.signal_demo_toggle = true; }
        });
    });
    ui.add_space(gap_sm());
    separator(ui, color_alpha(t.toolbar_border, alpha_muted()));
    ui.add_space(gap_md());

    ui.add(widgets::text::SectionLabel::new("VISIBILITY").tiny().color(t.dim));
    ui.add_space(gap_sm());

    let toggles: &mut [(&str, &str, &mut bool)] = &mut [
        ("Trend Health",    "Momentum & regime gauge (top-right)", &mut chart.show_trend_health),
        ("Exit Gauge",      "Position exit urgency indicator",     &mut chart.show_exit_gauge),
        ("Precursor",       "Unusual options activity badge",      &mut chart.show_precursor),
        ("Signal Zones",    "Supply / demand / FVG zones",         &mut chart.show_signal_zones),
        ("Trade Plan",      "Entry / target / stop overlay",       &mut chart.show_trade_plan),
        ("Change Points",   "Regime-change markers on time axis",  &mut chart.show_change_points),
        ("VIX Alert",       "VIX expiry warning card",             &mut chart.show_vix_alert),
        ("Pattern Labels",  "Candlestick patterns from ApexSignals", &mut chart.show_pattern_labels),
        ("Auto Trendlines", "Signal drawings (auto trendlines)",   &mut chart.show_auto_trendlines),
        ("Hit Highlight",   "Flash indicators/drawings on price touch", &mut chart.hit_highlight),
        ("Divergences",     "RSI/MACD divergence overlays",        &mut chart.show_divergences),
        ("Dark Pool",       "Dark pool prints overlay",            &mut chart.show_darkpool),
        ("Gamma",           "Gamma exposure levels",               &mut chart.show_gamma),
        ("Events",          "Calendar event markers",              &mut chart.show_events),
    ];

    for (name, hint, flag) in toggles {
        ui.horizontal(|ui| {
            ui.add_space(gap_sm());
            let icon = if **flag { Icon::EYE } else { Icon::EYE_SLASH };
            let color = if **flag { t.accent } else { t.dim.gamma_multiply(0.4) };
            if icon_btn(ui, icon, color, font_md()).clicked() { **flag = !**flag; }
            ui.vertical(|ui| {
                let lc = if **flag { t.text } else { t.dim.gamma_multiply(0.5) };
                ui.add(widgets::text::BodyLabel::new(*name).monospace(true).strong(true).size(font_sm()).color(lc));
                ui.add(widgets::text::MonospaceCode::new(*hint).xs().color(t.dim.gamma_multiply(0.5)));
            });
        });
        ui.add_space(gap_xs());
    }

    chart.hide_signal_drawings = !chart.show_auto_trendlines;
}
