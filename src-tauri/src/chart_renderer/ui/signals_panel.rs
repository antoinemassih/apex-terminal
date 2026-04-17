//! Signals panel — unified Alerts + Signals sidebar.

use egui;
use super::style::*;
use super::super::gpu::{Watchlist, Chart, Theme};
use crate::chart_renderer::SignalsTab;
use crate::ui_kit::icons::Icon;

pub(crate) fn draw(
    ctx: &egui::Context,
    watchlist: &mut Watchlist,
    panes: &mut [Chart],
    ap: usize,
    t: &Theme,
) {
    if !watchlist.signals_panel_open { return; }

    egui::SidePanel::right("signals_panel")
        .default_width(280.0)
        .min_width(240.0)
        .max_width(420.0)
        .resizable(true)
        .frame(panel_frame(t.toolbar_bg, t.toolbar_border))
        .show(ctx, |ui| {
            // Tab bar row with close button
            let tab_row = ui.horizontal(|ui| {
                ui.set_min_height(26.0);
                tab_bar(ui, &mut watchlist.signals_tab, &[
                    (SignalsTab::Alerts, "Alerts"),
                    (SignalsTab::Signals, "Signals"),
                ], t.accent, t.dim);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if close_button(ui, t.dim) { watchlist.signals_panel_open = false; }
                });
            });
            // Line below tabs
            let line_y = tab_row.response.rect.max.y;
            ui.painter().line_segment(
                [egui::pos2(ui.min_rect().left(), line_y),
                 egui::pos2(ui.min_rect().right(), line_y)],
                egui::Stroke::new(1.0, color_alpha(t.toolbar_border, ALPHA_MUTED)));
            ui.add_space(GAP_SM);

            // Dispatch to tab content
            match watchlist.signals_tab {
                SignalsTab::Alerts => {
                    super::alerts_panel::draw_content(ui, watchlist, panes, ap, t);
                }
                SignalsTab::Signals => {
                    draw_signals_toggles(ui, panes, ap, t);
                }
            }
        });
}

/// Per-signal visibility toggles — the Signals tab body.
/// Lets the user turn on/off each signal overlay independently.
fn draw_signals_toggles(ui: &mut egui::Ui, panes: &mut [Chart], ap: usize, t: &Theme) {
    ui.add_space(GAP_SM);

    // Demo toggle header
    let chart = &mut panes[ap];
    let demo_on = chart.trend_health_score > 0.0 || chart.precursor_active || chart.trade_plan.is_some();
    ui.horizontal(|ui| {
        section_label(ui, "DEMO SIGNALS", t.dim);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let label = if demo_on { "Stop Demo" } else { "Start Demo" };
            let color = if demo_on { t.bear } else { t.accent };
            if small_action_btn(ui, label, color) {
                chart.signal_demo_toggle = true;
            }
        });
    });
    ui.add_space(GAP_SM);
    separator(ui, color_alpha(t.toolbar_border, ALPHA_MUTED));
    ui.add_space(GAP_MD);

    // Signal visibility toggles
    ui.horizontal(|ui| {
        section_label(ui, "VISIBILITY", t.dim);
    });
    ui.add_space(GAP_SM);

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
            ui.add_space(GAP_SM);
            // Checkbox
            let icon = if **flag { Icon::EYE } else { Icon::EYE_SLASH };
            let color = if **flag { t.accent } else { t.dim.gamma_multiply(0.4) };
            if icon_btn(ui, icon, color, FONT_MD).clicked() {
                **flag = !**flag;
            }
            // Label + hint
            ui.vertical(|ui| {
                let label_color = if **flag { TEXT_PRIMARY } else { t.dim.gamma_multiply(0.5) };
                ui.label(egui::RichText::new(*name).monospace().size(FONT_SM).strong().color(label_color));
                ui.label(egui::RichText::new(*hint).monospace().size(FONT_XS).color(t.dim.gamma_multiply(0.5)));
            });
        });
        ui.add_space(GAP_XS);
    }

    // Sync hide_signal_drawings with show_auto_trendlines (they're inverse)
    chart.hide_signal_drawings = !chart.show_auto_trendlines;
}
