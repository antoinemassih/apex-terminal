//! Playbook panel — standalone sidebar for trade idea plays.

use egui;
use super::super::super::gpu::{Watchlist, Chart, Theme};
use crate::chart_renderer::ui::panels::side_panel_shell::{SidePanelShell, Width};

/// Rail registration — see [`super::right_rail`].
pub(crate) const RAIL: super::right_rail::RailPanelDef = super::right_rail::RailPanelDef {
    id: "playbook",
    is_open: |w| w.playbook_panel_open,
    render: |cx, slot| draw(cx.ctx, cx.watchlist, cx.panes, cx.active_pane, cx.t, Some(slot)),
};

pub(crate) fn draw(
    ctx: &egui::Context,
    watchlist: &mut Watchlist,
    panes: &mut [Chart],
    ap: usize,
    t: &Theme,
    slot: Option<super::side_panel_shell::RailSlot>,
) {
    if !watchlist.playbook_panel_open { return; }

    let pane_h    = crate::chart_renderer::gpu::pane_tabs_header_h(watchlist);
    let pane_font = watchlist.pane_header_size.title_font();
    let resp = SidePanelShell::new("playbook_panel", "PLAYBOOK")
        .width(Width::Narrow)
        .resizable(240.0..=440.0)
        .pane_metrics(pane_h, pane_font)
        .rail_slot(slot)
        .show(ctx, t, |ui, t| {
            super::plays_panel::draw_content(ui, watchlist, panes, ap, t);
        });
    if resp.close_clicked { watchlist.update_sidebar_state(|s| s.playbook_panel_open = false); }
}
