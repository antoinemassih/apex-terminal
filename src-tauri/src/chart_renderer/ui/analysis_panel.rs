//! Analysis panel — unified RRG, T&S, Scanner, Scripts sidebar.

use egui;
use super::style::*;
use super::super::gpu::{Watchlist, Chart, Theme};
use crate::chart_renderer::AnalysisTab;

pub(crate) fn draw(
    ctx: &egui::Context,
    watchlist: &mut Watchlist,
    panes: &mut [Chart],
    ap: usize,
    t: &Theme,
) {
    if !watchlist.analysis_open { return; }

    // Deferred actions from scanner tab
    let mut pending_symbol: Option<String> = None;

    egui::SidePanel::right("analysis_panel")
        .default_width(280.0)
        .min_width(220.0)
        .max_width(480.0)
        .resizable(true)
        .frame(panel_frame(t.toolbar_bg, t.toolbar_border))
        .show(ctx, |ui| {
            let panel_w = ui.available_width();

            // Tab bar row with close button
            let tab_row = ui.horizontal(|ui| {
                ui.set_min_height(26.0);
                tab_bar(ui, &mut watchlist.analysis_tab, &[
                    (AnalysisTab::Rrg, "RRG"),
                    (AnalysisTab::TimeSales, "T&S"),
                    (AnalysisTab::Scanner, "Scanner"),
                    (AnalysisTab::Scripts, "Scripts"),
                ], t.accent, t.dim);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if close_button(ui, t.dim) { watchlist.analysis_open = false; }
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
            match watchlist.analysis_tab {
                AnalysisTab::Rrg => {
                    super::rrg_panel::draw_content(ui, watchlist, t);
                }
                AnalysisTab::TimeSales => {
                    let sym = if !panes.is_empty() { panes[ap].symbol.clone() } else { String::new() };
                    super::tape_panel::draw_content(ui, watchlist, &sym, t);
                }
                AnalysisTab::Scanner => {
                    super::scanner_panel::draw_content(ui, watchlist, panes, ap, t, &mut pending_symbol, panel_w);
                }
                AnalysisTab::Scripts => {
                    super::script_panel::draw_content(ui, watchlist, t);
                }
            }
        });

    // Apply deferred symbol changes from scanner tab
    if let Some(sym) = pending_symbol {
        if let Some(p) = panes.get_mut(ap) {
            p.pending_symbol_change = Some(sym);
        }
    }
}
