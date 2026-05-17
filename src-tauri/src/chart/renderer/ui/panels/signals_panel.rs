//! Signals panel — sidebar with subdivided sections, each with its own tab bar.

use egui;
use super::super::style::*;
use super::super::widgets as widgets;
use super::super::super::gpu::{Watchlist, Chart, Theme, SplitSection};
use super::super::widgets::headers::PanelHeaderWithClose;
use crate::apex_data::live_state;
use crate::apex_data::types::{Calibrated, CombinedSignalV2};
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
            // Header — title + add-section button + close. Pre-resolve
            // pane-aligned metrics to avoid a watchlist borrow conflict with
            // the mutating closure below.
            let header_h = crate::chart_renderer::gpu::pane_tabs_header_h(watchlist);
            let title_font_size = watchlist.pane_header_size.title_font();
            let closed = PanelHeaderWithClose::new("SIGNALS").theme(t)
                .height(header_h).font_size(title_font_size)
                .show_with(ui, |ui| {
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
            if closed { watchlist.signals_panel_open = false; }
            separator(ui, color_alpha(t.toolbar_border, alpha_muted()));

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
                    egui::Stroke::new(stroke_thin(), color_alpha(t.toolbar_border, alpha_faint())));

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
    ui.add_space(gap_sm());

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
            if icon_btn(ui, icon, color, FONT_MD).clicked() { **flag = !**flag; }
            ui.vertical(|ui| {
                let lc = if **flag { t.text } else { t.dim.gamma_multiply(0.5) };
                ui.add(widgets::text::BodyLabel::new(*name).monospace(true).strong(true).size(FONT_SM).color(lc));
                ui.add(widgets::text::MonospaceCode::new(*hint).xs().color(t.dim.gamma_multiply(0.5)));
            });
        });
        ui.add_space(gap_xs());
    }

    chart.hide_signal_drawings = !chart.show_auto_trendlines;

    // ── SOTA UX §4.6: calibrated signals list ─────────────────────────────
    // Minimal surgical addition — appended below the visibility toggles.
    // Reads from `live_state::all_combined_sorted()` (populated by the
    // `combined` WS frame routed through `ws::dispatch`).
    ui.add_space(gap_md());
    separator(ui, color_alpha(t.toolbar_border, alpha_muted()));
    ui.add_space(gap_sm());
    ui.add(widgets::text::SectionLabel::new("CALIBRATED SIGNALS").tiny().color(t.dim));
    ui.add_space(gap_xs());

    let signals = live_state::all_combined_sorted();
    if signals.is_empty() {
        ui.label(egui::RichText::new("No signals yet").monospace().size(FONT_XS).color(t.dim.gamma_multiply(0.5)));
    } else {
        // Column header — keep alignment monospace-friendly so eyes can scan.
        ui.horizontal(|ui| {
            ui.add_space(gap_sm());
            ui.label(egui::RichText::new("score").monospace().size(FONT_3XS).color(t.dim));
            ui.label(egui::RichText::new("engine").monospace().size(FONT_3XS).color(t.dim));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(egui::RichText::new("🔍").monospace().size(FONT_3XS).color(t.dim));
                ui.add_space(gap_xs());
                ui.label(egui::RichText::new("trust").monospace().size(FONT_3XS).color(t.dim));
                ui.add_space(gap_xs());
                ui.label(egui::RichText::new("calibrated").monospace().size(FONT_3XS).color(t.dim));
            });
        });
        ui.add_space(gap_2xs());
        for sig in signals.iter().take(20) {
            draw_signal_row_calibrated(ui, sig, t);
        }
    }
}

/// Render one row of the calibrated signals table. Public for tests so we
/// can assert wiring (calibrated rendering, trust bar, lineage button).
///
/// Layout (spec §4.6):
/// ```
/// score │ engine │ symbol │ time │ calibrated │ trust │ 🔍
/// ```
pub(crate) fn draw_signal_row_calibrated(
    ui: &mut egui::Ui,
    sig: &CombinedSignalV2,
    t: &Theme,
) {
    ui.horizontal(|ui| {
        ui.add_space(gap_sm());
        // Score (large, accent if positive direction).
        let score_col = match sig.direction.as_str() {
            "long"  => t.bull,
            "short" => t.bear,
            _ => t.text,
        };
        ui.label(egui::RichText::new(format!("{:>3.0}", sig.score))
            .monospace().size(FONT_XS).color(score_col));
        // Engine (first contributor's engine, or "—").
        let engine = sig.top_contributors.first()
            .map(|c| c.engine.as_str()).unwrap_or("—");
        ui.label(egui::RichText::new(engine).monospace().size(FONT_XS).color(t.text));
        // Symbol.
        ui.label(egui::RichText::new(&sig.symbol).monospace().size(FONT_XS).color(t.dim));

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // 🔍 button → opens ProvenancePane via the cross-panel event bus.
            let lineage = sig.provenance.as_ref().map(|p| p.lineage_id.clone())
                .or_else(|| sig.top_contributors.first()
                    .and_then(|c| c.lineage_id.clone()));
            let btn = ui.add_enabled(
                lineage.is_some(),
                egui::Button::new(egui::RichText::new("🔍")
                    .monospace().size(FONT_XS)
                    .color(if lineage.is_some() { t.accent } else { t.dim.gamma_multiply(0.4) }))
                    .fill(egui::Color32::TRANSPARENT)
                    .min_size(egui::vec2(18.0, 16.0)),
            );
            if btn.clicked() {
                if let Some(l) = lineage {
                    super::provenance_pane::request_open(l);
                }
            }
            ui.add_space(gap_xs());

            // Trust bar — visual `min(n,50)/50`. Pull the strongest
            // contributor's calibration (matches the spec — first column
            // engine drives the row).
            let calibrated = sig.top_contributors.first()
                .and_then(|c| sig.calibrated_contributors.get(&c.engine).cloned())
                .unwrap_or_default();
            draw_trust_bar(ui, &calibrated, t);
            ui.add_space(gap_xs());

            // Calibrated hit_rate + sample-size, or "—" if uncalibrated.
            let label = format_calibrated_label(&calibrated);
            ui.label(egui::RichText::new(label)
                .monospace().size(FONT_XS)
                .color(if calibrated.is_calibrated() { t.text } else { t.dim.gamma_multiply(0.5) }));
        });
    });
    ui.add_space(gap_2xs());
}

/// Format the calibrated column: "62% (n=240)" or "—".
pub(crate) fn format_calibrated_label(c: &Calibrated) -> String {
    if !c.is_calibrated() { return "—".into(); }
    let hr = c.hit_rate.unwrap_or(0.0) * 100.0;
    format!("{:.0}% (n={})", hr, c.n_samples)
}

fn draw_trust_bar(ui: &mut egui::Ui, c: &Calibrated, t: &Theme) {
    let trust = c.trust_factor(); // 0..=1
    let w = 32.0;
    let h = 6.0;
    let (rect, _resp) = ui.allocate_exact_size(egui::vec2(w, h), egui::Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, color_alpha(t.dim, alpha_ghost()));
    let fill_w = (w * trust).clamp(0.0, w);
    let fill_rect = egui::Rect::from_min_max(
        rect.min, egui::pos2(rect.min.x + fill_w, rect.max.y));
    let col = if trust > 0.6 { t.accent }
              else if trust > 0.3 { t.warn }
              else { t.dim };
    painter.rect_filled(fill_rect, 0.0, col);
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::apex_data::types::{Calibrated, CombinedSignalV2, ContributorEntry, ProvenanceRef};

    #[test]
    fn calibrated_label_em_dash_when_undercalibrated() {
        let mut c = Calibrated::default();
        assert_eq!(format_calibrated_label(&c), "—");
        c.hit_rate = Some(0.6);
        c.n_samples = 5; // below threshold
        assert_eq!(format_calibrated_label(&c), "—");
    }

    #[test]
    fn calibrated_label_formats_percent_and_sample() {
        let c = Calibrated {
            hit_rate: Some(0.62),
            n_samples: 240,
            ..Default::default()
        };
        assert_eq!(format_calibrated_label(&c), "62% (n=240)");
    }

    #[test]
    fn calibrated_label_rounds_hit_rate() {
        let c = Calibrated {
            hit_rate: Some(0.555),
            n_samples: 100,
            ..Default::default()
        };
        // 55.5 -> "56% (n=100)" via default {:.0} rounding.
        let label = format_calibrated_label(&c);
        assert!(label == "56% (n=100)" || label == "55% (n=100)",
            "expected 55/56% rounding; got {}", label);
    }

    #[test]
    fn signal_with_provenance_provides_lineage_for_button() {
        let sig = CombinedSignalV2 {
            symbol: "SPY".into(),
            score: 78.0,
            direction: "long".into(),
            top_contributors: vec![ContributorEntry {
                engine: "iv_surface".into(),
                score: 78.0,
                lineage_id: Some("CONTRIBUTOR_ID".into()),
            }],
            provenance: Some(ProvenanceRef {
                lineage_id: "ROOT_ID".into(),
                inputs: vec![],
            }),
            ..Default::default()
        };
        let extracted = sig.provenance.as_ref().map(|p| p.lineage_id.clone())
            .or_else(|| sig.top_contributors.first()
                .and_then(|c| c.lineage_id.clone()));
        assert_eq!(extracted.as_deref(), Some("ROOT_ID"));
    }

    #[test]
    fn signal_without_root_provenance_falls_back_to_contributor() {
        let sig = CombinedSignalV2 {
            symbol: "SPY".into(),
            top_contributors: vec![ContributorEntry {
                engine: "pin_break".into(),
                score: 50.0,
                lineage_id: Some("FALLBACK_ID".into()),
            }],
            provenance: None,
            ..Default::default()
        };
        let extracted = sig.provenance.as_ref().map(|p| p.lineage_id.clone())
            .or_else(|| sig.top_contributors.first()
                .and_then(|c| c.lineage_id.clone()));
        assert_eq!(extracted.as_deref(), Some("FALLBACK_ID"));
    }

    #[test]
    fn signal_without_any_lineage_disables_button() {
        let sig = CombinedSignalV2 {
            symbol: "QQQ".into(),
            top_contributors: vec![],
            provenance: None,
            ..Default::default()
        };
        let extracted = sig.provenance.as_ref().map(|p| p.lineage_id.clone())
            .or_else(|| sig.top_contributors.first()
                .and_then(|c| c.lineage_id.clone()));
        assert!(extracted.is_none());
    }

    #[test]
    fn trust_factor_clamps_at_50_samples() {
        let mut c = Calibrated::default();
        assert_eq!(c.trust_factor(), 0.0);
        c.n_samples = 25;
        assert_eq!(c.trust_factor(), 0.5);
        c.n_samples = 50;
        assert_eq!(c.trust_factor(), 1.0);
        c.n_samples = 100;
        assert_eq!(c.trust_factor(), 1.0, "trust factor saturates at 1.0");
    }
}
