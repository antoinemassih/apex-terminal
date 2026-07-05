//! Analysis panel — sidebar with subdivided sections, each with its own tab bar.
//!
//! Chrome (outer side panel, header, "+", per-section tab strips, dividers,
//! close-X) is delegated to
//! [`SplitSectionPanel`](crate::chart_renderer::ui::panels::split_section_panel::SplitSectionPanel). This
//! module is now responsible only for tab definitions and per-tab body
//! dispatch (RRG / T&S / Scanner / Scripts / Seasonality / Research).

use egui;
use super::super::super::gpu::{Watchlist, Chart, Theme, SplitSection};
use crate::chart_renderer::AnalysisTab;
use crate::chart_renderer::ui::panels::side_panel_shell::{SidePanelShell, Width, RailSlot};

/// Rail registration — Analysis is now a standard `SidePanelShell::tabs` panel.
pub(crate) const RAIL: super::right_rail::RailPanelDef = super::right_rail::RailPanelDef {
    id: "analysis",
    is_open: |w| w.analysis.open,
    render: |cx, slot| { draw(cx.ctx, cx.watchlist, cx.panes, cx.active_pane, cx.t, Some(slot), None); },
};

fn analysis_tab_to_u8(t: AnalysisTab) -> u8 {
    match t {
        AnalysisTab::Rrg => 0, AnalysisTab::TimeSales => 1, AnalysisTab::Scanner => 2,
        AnalysisTab::Scripts => 3, AnalysisTab::Seasonality => 4, AnalysisTab::Research => 5,
    }
}
fn analysis_tab_from_u8(v: u8) -> AnalysisTab {
    match v {
        1 => AnalysisTab::TimeSales, 2 => AnalysisTab::Scanner, 3 => AnalysisTab::Scripts,
        4 => AnalysisTab::Seasonality, 5 => AnalysisTab::Research, _ => AnalysisTab::Rrg,
    }
}

pub(crate) fn draw(
    ctx: &egui::Context,
    watchlist: &mut Watchlist,
    panes: &mut [Chart],
    ap: usize,
    t: &Theme,
    slot: Option<RailSlot>,
    instance_tab: Option<&mut u8>,
) -> bool {
    let is_spawn = instance_tab.is_some();
    let mut spawn_close = false;
    if !is_spawn && !watchlist.analysis.open { return false; }

    let mut pending_symbol: Option<String> = None;

    let pane_h = crate::chart_renderer::gpu::pane_tabs_header_h(watchlist);
    let pane_font = watchlist.pane_header_size.title_font();

    let mut active = match instance_tab.as_deref() {
        Some(v) => analysis_tab_from_u8(*v),
        None => watchlist.analysis.splits.first().map(|s| s.tab).unwrap_or(AnalysisTab::Rrg),
    };
    let tabs = [
        (AnalysisTab::Rrg,         "RRG",         None),
        (AnalysisTab::TimeSales,   "T&S",         None),
        (AnalysisTab::Scanner,     "SCANNER",     None),
        (AnalysisTab::Scripts,     "SCRIPTS",     None),
        (AnalysisTab::Seasonality, "SEASONALITY", None),
        (AnalysisTab::Research,    "RESEARCH",    None),
    ];

    let shell_id = if is_spawn { "analysis_inst" } else { "analysis_panel" };
    let resp = SidePanelShell::tabs(shell_id, &mut active, &tabs)
        .width(Width::Narrow)
        .resizable(220.0..=480.0)
        .pane_metrics(pane_h, pane_font)
        .rail_slot(slot)
        .on_tab_secondary(|ui, tab| {
            if crate::ui_kit::widgets::MenuItem::new("Open as new instance").show(ui, t).clicked() {
                super::right_rail::request_spawn("analysis", analysis_tab_to_u8(tab));
                ui.close_menu();
            }
        })
        .show(ctx, t, |ui, t, tab| {
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

    // Persist the active tab to its owner (instance store or base panel).
    if let Some(it) = instance_tab { *it = analysis_tab_to_u8(active); }
    else if let Some(s) = watchlist.analysis.splits.first_mut() { s.tab = active; }
    else { watchlist.analysis.splits.push(SplitSection { tab: active, frac: 1.0 }); }
    if resp.close_clicked {
        if is_spawn { spawn_close = true; }
        else { watchlist.update_sidebar_state(|s| s.analysis_open = false); }
    }

    if let Some(sym) = pending_symbol {
        if let Some(p) = panes.get_mut(ap) {
            p.pending_symbol_change = Some(sym);
        }
    }
    spawn_close
}
