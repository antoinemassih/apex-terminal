//! Signals panel — sidebar with subdivided sections, each with its own tab bar.

use egui;
use super::style::*;
use super::widgets;
use super::super::gpu::{Watchlist, Chart, Theme, SplitSection};
use crate::chart_renderer::SignalsTab;
use crate::ui_kit::icons::Icon;

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

    egui::SidePanel::right("signals_panel")
        .default_width(260.0)
        .min_width(240.0)
        .max_width(420.0)
        .resizable(true)
        .frame(widgets::frames::PanelFrame::new(t.toolbar_bg, t.toolbar_border).theme(t).build())
        .show(ctx, |ui| {
            // Header
            let header = ui.horizontal(|ui| {
                ui.set_min_height(26.0);
                ui.label(egui::RichText::new("SIGNALS").monospace().size(FONT_SM).strong().color(t.accent));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if close_button(ui, t.dim) { watchlist.signals_panel_open = false; }
                    if ui.add(egui::Button::new(egui::RichText::new("+").monospace().size(FONT_SM).color(t.dim))
                        .fill(egui::Color32::TRANSPARENT).min_size(egui::vec2(20.0, 20.0))).clicked() {
                        let used: Vec<SignalsTab> = watchlist.signals_splits.iter().map(|s| s.tab).collect();
                        let next = ALL_TABS.iter().find(|(tab, _)| !used.contains(tab))
                            .map(|(tab, _)| *tab).unwrap_or(SignalsTab::Alerts);
                        if let Some(last) = watchlist.signals_splits.last_mut() { last.frac *= 0.5; }
                        let frac = watchlist.signals_splits.last().map(|s| s.frac).unwrap_or(1.0);
                        watchlist.signals_splits.push(SplitSection::new(next, frac));
                    }
                });
            });
            let line_y = header.response.rect.max.y;
            ui.painter().line_segment(
                [egui::pos2(ui.min_rect().left(), line_y), egui::pos2(ui.min_rect().right(), line_y)],
                egui::Stroke::new(1.0, color_alpha(t.toolbar_border, ALPHA_MUTED)));

            let available_h = ui.available_height();
            let n = watchlist.signals_splits.len();
            if watchlist.signals_splits.is_empty() {
                watchlist.signals_splits.push(SplitSection::new(SignalsTab::Alerts, 1.0));
            }

            let divider_total = n.saturating_sub(1) as f32 * 6.0;
            let tab_bar_total = n as f32 * 28.0;
            let content_h = (available_h - divider_total - tab_bar_total).max(40.0);
            let total_frac: f32 = watchlist.signals_splits.iter().map(|s| s.frac).sum();
            let norm = if total_frac > 0.001 { 1.0 / total_frac } else { 1.0 };
            let heights: Vec<f32> = watchlist.signals_splits.iter()
                .map(|s| (s.frac * norm * content_h).max(30.0)).collect();

            let mut remove_idx: Option<usize> = None;
            let mut divider_drags: Vec<(usize, f32)> = Vec::new();

            for i in 0..n {
                let tab = watchlist.signals_splits[i].tab;
                let h = heights[i];
                let can_close = n > 1;

                ui.horizontal(|ui| {
                    ui.set_min_height(26.0);
                    for (t_val, t_label) in ALL_TABS {
                        let sel = tab == *t_val;
                        let fg = if sel { t.accent } else { t.dim.gamma_multiply(0.5) };
                        if ui.add(egui::Button::new(egui::RichText::new(*t_label).monospace().size(FONT_XS).color(fg))
                            .fill(egui::Color32::TRANSPARENT).stroke(egui::Stroke::NONE)
                            .min_size(egui::vec2(0.0, 22.0))).clicked() {
                            watchlist.signals_splits[i].tab = *t_val;
                        }
                    }
                    if can_close {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.add(egui::Button::new(egui::RichText::new("\u{00D7}").size(FONT_SM).color(t.dim.gamma_multiply(0.4)))
                                .fill(egui::Color32::TRANSPARENT).min_size(egui::vec2(18.0, 18.0))).clicked() {
                                remove_idx = Some(i);
                            }
                        });
                    }
                });
                ui.painter().line_segment(
                    [egui::pos2(ui.min_rect().left(), ui.min_rect().bottom()),
                     egui::pos2(ui.min_rect().right(), ui.min_rect().bottom())],
                    egui::Stroke::new(0.5, color_alpha(t.toolbar_border, ALPHA_FAINT)));

                egui::ScrollArea::vertical().id_salt(format!("sig_sec_{}", i)).max_height(h).show(ui, |ui| {
                    match tab {
                        SignalsTab::Alerts => super::alerts_panel::draw_content(ui, watchlist, panes, ap, t),
                        SignalsTab::Signals => draw_signals_toggles(ui, panes, ap, t),
                    }
                });

                if i + 1 < n {
                    let d = split_divider(ui, &format!("sdiv_{}", i), t.dim);
                    if d != 0.0 { divider_drags.push((i, d)); }
                }
            }

            if let Some(idx) = remove_idx {
                let removed = watchlist.signals_splits[idx].frac;
                watchlist.signals_splits.remove(idx);
                if !watchlist.signals_splits.is_empty() {
                    let share = removed / watchlist.signals_splits.len() as f32;
                    for s in &mut watchlist.signals_splits { s.frac += share; }
                }
            }
            for (idx, delta) in divider_drags {
                if idx + 1 < watchlist.signals_splits.len() {
                    let fd = delta / available_h.max(1.0);
                    watchlist.signals_splits[idx].frac = (watchlist.signals_splits[idx].frac + fd).clamp(0.05, 0.90);
                    watchlist.signals_splits[idx + 1].frac = (watchlist.signals_splits[idx + 1].frac - fd).clamp(0.05, 0.90);
                }
            }
        });
}

/// Per-signal visibility toggles.
fn draw_signals_toggles(ui: &mut egui::Ui, panes: &mut [Chart], ap: usize, t: &Theme) {
    ui.add_space(GAP_SM);

    let chart = &mut panes[ap];
    let demo_on = chart.trend_health_score > 0.0 || chart.precursor_active || chart.trade_plan.is_some();
    ui.horizontal(|ui| {
        ui.add(widgets::text::SectionLabel::new("DEMO SIGNALS").tiny().color(t.dim));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let label = if demo_on { "Stop Demo" } else { "Start Demo" };
            let color = if demo_on { t.bear } else { t.accent };
            if small_action_btn(ui, label, color) { chart.signal_demo_toggle = true; }
        });
    });
    ui.add_space(GAP_SM);
    separator(ui, color_alpha(t.toolbar_border, ALPHA_MUTED));
    ui.add_space(GAP_MD);

    ui.add(widgets::text::SectionLabel::new("VISIBILITY").tiny().color(t.dim));
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
            let icon = if **flag { Icon::EYE } else { Icon::EYE_SLASH };
            let color = if **flag { t.accent } else { t.dim.gamma_multiply(0.4) };
            if icon_btn(ui, icon, color, FONT_MD).clicked() { **flag = !**flag; }
            ui.vertical(|ui| {
                let lc = if **flag { t.text } else { t.dim.gamma_multiply(0.5) };
                ui.label(egui::RichText::new(*name).monospace().size(FONT_SM).strong().color(lc));
                ui.add(widgets::text::MonospaceCode::new(*hint).xs().color(t.dim.gamma_multiply(0.5)));
            });
        });
        ui.add_space(GAP_XS);
    }

    chart.hide_signal_drawings = !chart.show_auto_trendlines;
}
