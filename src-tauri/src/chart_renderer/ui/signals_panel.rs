//! Signals panel — unified Alerts + Signals sidebar.

use egui;
use super::style::*;
use super::super::gpu::{Watchlist, Chart, Theme};
use crate::chart_renderer::SignalsTab;

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
                    ui.label("Signals — coming soon");
                }
            }
        });
}
