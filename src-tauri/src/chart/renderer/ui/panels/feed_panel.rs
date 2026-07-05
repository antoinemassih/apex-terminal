//! Feed panel — sidebar with subdivided sections, each with its own tab bar.
//!
//! Chrome (outer side panel, header, "+", per-section tab strips, dividers,
//! close-X) is delegated to
//! [`SplitSectionPanel`](crate::chart_renderer::ui::panels::split_section_panel::SplitSectionPanel). This
//! module is now responsible only for tab definitions and per-tab body
//! dispatch (News / Discord / Screenshots).

use egui;
use super::super::super::gpu::{Watchlist, Chart, Theme, SplitSection};
use crate::chart_renderer::FeedTab;
use crate::chart_renderer::ui::panels::side_panel_shell::{SidePanelShell, Width, RailSlot};

/// Rail registration — Feed is now a standard `SidePanelShell::tabs` panel.
pub(crate) const RAIL: super::right_rail::RailPanelDef = super::right_rail::RailPanelDef {
    id: "feed",
    is_open: |w| w.feed_panel.open,
    render: |cx, slot| { draw(cx.ctx, cx.watchlist, cx.panes, cx.active_pane, cx.t, Some(slot), None); },
};

fn feed_tab_to_u8(t: FeedTab) -> u8 {
    match t { FeedTab::News => 0, FeedTab::Discord => 1, FeedTab::Screenshots => 2 }
}
fn feed_tab_from_u8(v: u8) -> FeedTab {
    match v { 1 => FeedTab::Discord, 2 => FeedTab::Screenshots, _ => FeedTab::News }
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
    if !is_spawn && !watchlist.feed_panel.open { return false; }

    super::discord_panel::drain_background(ctx, watchlist);
    let active_symbol = if !panes.is_empty() { panes[ap].symbol.clone() } else { String::new() };

    let pane_h = crate::chart_renderer::gpu::pane_tabs_header_h(watchlist);
    let pane_font = watchlist.pane_header_size.title_font();

    let mut active = match instance_tab.as_deref() {
        Some(v) => feed_tab_from_u8(*v),
        None => watchlist.feed_panel.splits.first().map(|s| s.tab).unwrap_or(FeedTab::News),
    };
    let tabs = [
        (FeedTab::News,        "NEWS",        None),
        (FeedTab::Discord,     "DISCORD",     None),
        (FeedTab::Screenshots, "SCREENSHOTS", None),
    ];

    let shell_id = if is_spawn { "feed_inst" } else { "feed_panel" };
    let resp = SidePanelShell::tabs(shell_id, &mut active, &tabs)
        .width(Width::Medium)
        .resizable(260.0..=480.0)
        .pane_metrics(pane_h, pane_font)
        .rail_slot(slot)
        .on_tab_secondary(|ui, tab| {
            if crate::ui_kit::widgets::MenuItem::new("Open as new instance").show(ui, t).clicked() {
                super::right_rail::request_spawn("feed", feed_tab_to_u8(tab));
                ui.close_menu();
            }
        })
        .show(ctx, t, |ui, t, tab| {
            match tab {
                FeedTab::News =>
                    super::news_panel::draw_content(ui, watchlist, &active_symbol, t),
                FeedTab::Discord =>
                    super::discord_panel::draw_content(ui, watchlist, t),
                FeedTab::Screenshots =>
                    super::screenshot_panel::draw_content(ui, watchlist, t, panes, ap),
            }
        });

    // Persist the active tab to its owner (instance store or base panel).
    if let Some(it) = instance_tab { *it = feed_tab_to_u8(active); }
    else if let Some(s) = watchlist.feed_panel.splits.first_mut() { s.tab = active; }
    else { watchlist.feed_panel.splits.push(SplitSection { tab: active, frac: 1.0 }); }
    if resp.close_clicked {
        if is_spawn { spawn_close = true; }
        else { watchlist.update_sidebar_state(|s| s.feed_panel_open = false); }
    }
    spawn_close
}
