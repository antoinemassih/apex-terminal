//! Playbook panel — standalone sidebar for trade idea plays.

use egui;
use super::super::style::*;
use super::super::super::gpu::{Watchlist, Chart, Theme};
use super::super::widgets::frames::PanelFrame;
use super::super::widgets::headers::PanelHeaderWithClose;

pub(crate) fn draw(
    ctx: &egui::Context,
    watchlist: &mut Watchlist,
    panes: &mut [Chart],
    ap: usize,
    t: &Theme,
) {
    if !watchlist.playbook_panel_open { return; }

    egui::SidePanel::right("playbook_panel")
        .default_width(280.0)
        .min_width(240.0)
        .max_width(440.0)
        .resizable(true)
        .frame(PanelFrame::new(t.toolbar_bg, t.toolbar_border).build())
        .show(ctx, |ui| {
            if PanelHeaderWithClose::new("PLAYBOOK").theme(t).show(ui) {
                watchlist.playbook_panel_open = false;
            }
            separator(ui, color_alpha(t.toolbar_border, alpha_muted()));
            ui.add_space(gap_sm());

            super::plays_panel::draw_content(ui, watchlist, panes, ap, t);
        });
}
