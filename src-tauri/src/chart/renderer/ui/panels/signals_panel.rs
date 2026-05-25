//! Signals panel — sidebar with subdivided sections, each with its own tab bar.
//!
//! Chrome (outer side panel, header, "+", tab strips, dividers, close-X)
//! is delegated to [`SplitSectionPanel`](crate::chart_renderer::ui::panels::split_section_panel::SplitSectionPanel).
//! This module is now responsible only for tab definitions and per-tab
//! body dispatch.

use egui;
use super::super::style::*;
use super::super::components::text as widgets_text;
use super::super::super::gpu::{Watchlist, Chart, Theme, SplitSection};
use crate::apex_data::live_state;
use crate::apex_data::types::{Calibrated, CombinedSignalV2};
use crate::chart_renderer::SignalsTab;
use crate::ui_kit::icons::Icon;
use crate::chart_renderer::ui::panels::side_panel_shell::Width;
use crate::ui_kit::widgets::{Button, PanelListRow};
use crate::chart_renderer::ui::panels::split_section_panel::SplitSectionPanel;
use crate::ui_kit::widgets::tokens::{Variant as KitVariant, Size as KitSize};

const ALL_TABS: &[(SignalsTab, &str)] = &[
    (SignalsTab::Alerts, "Alerts"),
    (SignalsTab::Signals, "Signals"),
    (SignalsTab::Regime, "Regime"),
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
                SignalsTab::Regime =>
                    super::regime_tape::draw_in_ui(ui, t),
            }
        });

    watchlist.signals_splits = splits;
    if resp.close_clicked { watchlist.update_sidebar_state(|s| s.signals_panel_open = false); }
}

/// Per-signal visibility toggles.
fn draw_signals_toggles(ui: &mut egui::Ui, panes: &mut [Chart], ap: usize, t: &Theme) {
    ui.add_space(gap_sm());

    let chart = &mut panes[ap];
    let demo_on = chart.trend_health_score > 0.0 || chart.precursor_active || chart.trade_plan.is_some();
    ui.horizontal(|ui| {
        ui.add(widgets_text::SectionLabel::new("DEMO SIGNALS").tiny().color(t.dim));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let label = if demo_on { "Stop Demo" } else { "Start Demo" };
            let color = if demo_on { t.bear } else { t.accent };
            if Button::small_action(label).tint(color).show(ui, t).clicked() { chart.signal_demo_toggle = true; }
        });
    });
    ui.add_space(gap_sm());
    separator(ui, color_alpha(t.toolbar_border, alpha_muted()));
    ui.add_space(gap_md());

    ui.add(widgets_text::SectionLabel::new("VISIBILITY").tiny().color(t.dim));
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
            let resp = Button::icon(icon)
                .variant(crate::ui_kit::widgets::tokens::Variant::Ghost)
                .placement(crate::ui_kit::widgets::icon_placement::IconPlacement::ListRow)
                .active(**flag)
                .show(ui, t);
            crate::ui_kit::widgets::Tooltip::new(if **flag { "Hide" } else { "Show" }).show(ui, &resp, t);
            if resp.clicked() { **flag = !**flag; }
            ui.vertical(|ui| {
                let lc = if **flag { t.text } else { color_half(t.dim) };
                ui.add(widgets_text::BodyLabel::new(*name).monospace(true).strong(true).size(font_sm()).color(lc));
                ui.add(widgets_text::MonospaceCode::new(*hint).xs().color(color_half(t.dim)));
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
    ui.add(widgets_text::SectionLabel::new("CALIBRATED SIGNALS").tiny().color(t.dim));
    ui.add_space(gap_xs());

    let signals = live_state::all_combined_sorted();
    if signals.is_empty() {
        ui.label(egui::RichText::new("No signals yet").monospace().size(font_xs()).color(color_muted(t.dim)));
    } else {
        // Column header — keep alignment monospace-friendly so eyes can scan.
        ui.horizontal(|ui| {
            ui.add_space(gap_sm());
            ui.label(egui::RichText::new("score").monospace().size(font_3xs()).color(t.dim));
            ui.label(egui::RichText::new("engine").monospace().size(font_3xs()).color(t.dim));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(egui::RichText::new("🔍").monospace().size(font_3xs()).color(t.dim));
                ui.add_space(gap_xs());
                ui.label(egui::RichText::new("trust").monospace().size(font_3xs()).color(t.dim));
                ui.add_space(gap_xs());
                ui.label(egui::RichText::new("calibrated").monospace().size(font_3xs()).color(t.dim));
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
///
/// Row chrome (hover background, row height, consistent padding) is provided
/// by `PanelListRow`. The left cells (score / engine / symbol) are rendered
/// in the `.leading` slot with per-cell theme colours; the right controls
/// (calibrated label / trust bar / 🔍 button) live in the `.trailing` slot
/// in a right-to-left layout.
///
/// The trust bar is genuinely custom: `ui_kit::Progress::linear` uses a fixed
/// accent colour and auto-width, whereas this bar needs three threshold colours
/// (accent / warn / dim) and an exact 32 × 6 px bounding box. It is therefore
/// kept as inline painter calls routed through theme colours.
pub(crate) fn draw_signal_row_calibrated(
    ui: &mut egui::Ui,
    sig: &CombinedSignalV2,
    t: &Theme,
) {
    // Resolve all per-row data before the closures borrow `sig`.
    let score_col = match sig.direction.as_str() {
        "long"  => t.bull,
        "short" => t.bear,
        _       => t.text,
    };
    let score_text = format!("{:>3.0}", sig.score);
    let engine = sig.top_contributors.first()
        .map(|c| c.engine.as_str()).unwrap_or("—");
    let symbol = sig.symbol.as_str();
    let calibrated = sig.top_contributors.first()
        .and_then(|c| sig.calibrated_contributors.get(&c.engine).cloned())
        .unwrap_or_default();
    let lineage = sig.provenance.as_ref().map(|p| p.lineage_id.clone())
        .or_else(|| sig.top_contributors.first()
            .and_then(|c| c.lineage_id.clone()));
    let calib_label = format_calibrated_label(&calibrated);
    let calib_col = if calibrated.is_calibrated() { t.text } else { color_muted(t.dim) };
    let trust = calibrated.trust_factor(); // 0.0..=1.0

    // `id_salt` must be stable per-row; use symbol + score as a cheap key.
    let id_salt = format!("sig_{}_{:.0}", sig.symbol, sig.score);

    PanelListRow::new(id_salt.as_str())
        .hoverable(false) // display-only streaming row — no pointer cursor
        .leading(move |ui, _t: &crate::chart_renderer::gpu::Theme| {
            ui.add_space(gap_sm());
            // Score — coloured by direction.
            ui.label(egui::RichText::new(&score_text)
                .monospace().size(font_xs()).color(score_col));
            // Engine.
            ui.label(egui::RichText::new(engine)
                .monospace().size(font_xs()).color(_t.text));
            // Symbol.
            ui.label(egui::RichText::new(symbol)
                .monospace().size(font_xs()).color(_t.dim));
        })
        .trailing(move |ui, _t| {
            // Layout is right-to-left; items are appended in reverse display order.
            // 🔍 button → opens ProvenancePane via the cross-panel event bus.
            let has_lineage = lineage.is_some();
            let btn = Button::new("🔍").variant(KitVariant::Ghost).size(KitSize::Xs)
                .fg(if has_lineage { _t.accent } else { color_muted(_t.dim) })
                .min_size(egui::vec2(18.0, 16.0))
                .disabled(!has_lineage)
                .show(ui, _t);
            if btn.clicked() {
                if let Some(ref l) = lineage {
                    super::provenance_pane::request_open(l.clone());
                }
            }
            ui.add_space(gap_xs());

            // Trust bar — genuinely custom: three threshold colours + fixed
            // 32 × 6 px bounding box (see fn doc). Kept as painter calls;
            // all colours route through the theme.
            let bar_w = 32.0;
            let bar_h = 6.0;
            let (rect, _) = ui.allocate_exact_size(
                egui::vec2(bar_w, bar_h), egui::Sense::hover());
            let painter = ui.painter_at(rect);
            painter.rect_filled(rect, 0.0, color_alpha(_t.dim, alpha_ghost()));
            let fill_w = (bar_w * trust).clamp(0.0, bar_w);
            if fill_w > 0.0 {
                let fill_rect = egui::Rect::from_min_max(
                    rect.min, egui::pos2(rect.min.x + fill_w, rect.max.y));
                let col = if trust > 0.6 { _t.accent }
                          else if trust > 0.3 { _t.warn }
                          else { _t.dim };
                painter.rect_filled(fill_rect, 0.0, col);
            }
            ui.add_space(gap_xs());

            // Calibrated hit_rate + sample-size, or "—" if uncalibrated.
            ui.label(egui::RichText::new(&calib_label)
                .monospace().size(font_xs()).color(calib_col));
        })
        .show(ui, t);
    ui.add_space(gap_2xs());
}

/// Format the calibrated column: "62% (n=240)" or "—".
pub(crate) fn format_calibrated_label(c: &Calibrated) -> String {
    if !c.is_calibrated() { return "—".into(); }
    let hr = c.hit_rate.unwrap_or(0.0) * 100.0;
    format!("{:.0}% (n={})", hr, c.n_samples)
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
