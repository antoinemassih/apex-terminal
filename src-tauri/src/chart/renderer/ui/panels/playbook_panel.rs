//! Playbook panel — standalone sidebar for trade idea plays.

use egui;
use super::super::super::gpu::{Watchlist, Chart, Theme};
use crate::chart_renderer::ui::panels::side_panel_shell::{SidePanelShell, Width};

pub(crate) fn draw(
    ctx: &egui::Context,
    watchlist: &mut Watchlist,
    panes: &mut [Chart],
    ap: usize,
    t: &Theme,
) {
    if !watchlist.playbook_panel_open { return; }

    let pane_h    = crate::chart_renderer::gpu::pane_tabs_header_h(watchlist);
    let pane_font = watchlist.pane_header_size.title_font();
    let resp = SidePanelShell::new("playbook_panel", "PLAYBOOK")
        .width(Width::Narrow)
        .resizable(240.0..=440.0)
        .pane_metrics(pane_h, pane_font)
        .show(ctx, t, |ui, t| {
            super::plays_panel::draw_content(ui, watchlist, panes, ap, t);
        });
    if resp.close_clicked { watchlist.update_sidebar_state(|s| s.playbook_panel_open = false); }
}
