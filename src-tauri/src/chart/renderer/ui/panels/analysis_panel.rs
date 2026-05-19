//! Analysis panel — sidebar with subdivided sections, each with its own tab bar.
//!
//! Chrome (outer side panel, header, "+", per-section tab strips, dividers,
//! close-X) is delegated to
//! [`SplitSectionPanel`](crate::ui_kit::widgets::SplitSectionPanel). This
//! module is now responsible only for tab definitions and per-tab body
//! dispatch (RRG / T&S / Scanner / Scripts / Seasonality / Research).

use egui;
use super::super::super::gpu::{Watchlist, Chart, Theme};
use crate::chart_renderer::AnalysisTab;
use crate::ui_kit::widgets::SplitSectionPanel;
use crate::ui_kit::widgets::side_panel_shell::Width;

const ALL_TABS: &[(AnalysisTab, &str)] = &[
    (AnalysisTab::Rrg, "RRG"),
    (AnalysisTab::TimeSales, "T&S"),
    (AnalysisTab::Scanner, "Scanner"),
    (AnalysisTab::Scripts, "Scripts"),
    (AnalysisTab::Seasonality, "Seasonality"),
    (AnalysisTab::Research, "Research"),
];

pub(crate) fn draw(
    ctx: &egui::Context,
    watchlist: &mut Watchlist,
    panes: &mut [Chart],
    ap: usize,
    t: &Theme,
) {
    if !watchlist.analysis_open { return; }

    let mut pending_symbol: Option<String> = None;

    // Snapshot per-section active tab so the body closure can dispatch
    // without holding a borrow into the splits Vec (owned by the widget).
    let tab_snapshot: Vec<AnalysisTab> =
        watchlist.analysis_splits.iter().map(|s| s.tab).collect();

    // Move splits out so the body closure can use `&mut watchlist` freely
    // for the child draw_content panels. Restore after `.show()`.
    let mut splits = std::mem::take(&mut watchlist.analysis_splits);
    let pane_h = crate::chart_renderer::gpu::pane_tabs_header_h(watchlist);
    let pane_font = watchlist.pane_header_size.title_font();

    let resp = SplitSectionPanel::new("analysis_panel", &mut splits)
        .title("ANALYSIS")
        .tabs(ALL_TABS)
        .default_tab(AnalysisTab::Rrg)
        .width(Width::Narrow)
        .resizable(220.0..=480.0)
        .pane_metrics(pane_h, pane_font)
        .show(ctx, t, |ui, t, i, _frac| {
            let tab = tab_snapshot.get(i).copied().unwrap_or(AnalysisTab::Rrg);
            let panel_w = ui.available_width();
            match tab {
                AnalysisTab::Rrg =>
                    super::rrg_panel::draw_content(ui, watchlist, t),
                AnalysisTab::TimeSales => {
                    let sym = if !panes.is_empty() { panes[ap].symbol.clone() } else { String::new() };
                    super::tape_panel::draw_content(ui, watchlist, &sym, t);
                }
                AnalysisTab::Scanner => {
                    super::scanner_panel::draw_content(
                        ui, watchlist, panes, ap, t, &mut pending_symbol, panel_w,
                    );
                }
                AnalysisTab::Scripts =>
                    super::script_panel::draw_content(ui, watchlist, t),
                AnalysisTab::Seasonality =>
                    super::seasonality_panel::draw_content(ui, watchlist, panes, ap, t),
                AnalysisTab::Research =>
                    super::research_panel::draw_content(ui, panes, ap, t),
            }
        });

    watchlist.analysis_splits = splits;
    if resp.close_clicked { watchlist.update_sidebar_state(|s| s.analysis_open = false); }

    if let Some(sym) = pending_symbol {
        if let Some(p) = panes.get_mut(ap) {
            p.pending_symbol_change = Some(sym);
        }
    }
}
