//! Feed panel — sidebar with subdivided sections, each with its own tab bar.
//!
//! Chrome (outer side panel, header, "+", per-section tab strips, dividers,
//! close-X) is delegated to
//! [`SplitSectionPanel`](crate::chart_renderer::ui::panels::split_section_panel::SplitSectionPanel). This
//! module is now responsible only for tab definitions and per-tab body
//! dispatch (News / Discord / Screenshots).

use egui;
use super::super::super::gpu::{Watchlist, Chart, Theme};
use crate::chart_renderer::FeedTab;
use crate::chart_renderer::ui::panels::split_section_panel::SplitSectionPanel;
use crate::chart_renderer::ui::panels::side_panel_shell::Width;

const ALL_TABS: &[(FeedTab, &str)] = &[
    (FeedTab::News, "News"),
    (FeedTab::Discord, "Discord"),
    (FeedTab::Screenshots, "Screenshots"),
];

pub(crate) fn draw(
    ctx: &egui::Context,
    watchlist: &mut Watchlist,
    panes: &mut [Chart],
    ap: usize,
    t: &Theme,
) {
    if !watchlist.feed_panel_open { return; }

    super::discord_panel::drain_background(ctx, watchlist);
    let active_symbol = if !panes.is_empty() { panes[ap].symbol.clone() } else { String::new() };

    // Snapshot per-section active tab so the body closure can dispatch
    // without holding a borrow into the splits Vec (owned by the widget).
    let tab_snapshot: Vec<FeedTab> =
        watchlist.feed_splits.iter().map(|s| s.tab).collect();

    // Move splits out so the body closure can use `&mut watchlist` freely
    // for the child draw_content panels. Restore after `.show()`.
    let mut splits = std::mem::take(&mut watchlist.feed_splits);
    let pane_h = crate::chart_renderer::gpu::pane_tabs_header_h(watchlist);
    let pane_font = watchlist.pane_header_size.title_font();

    let resp = SplitSectionPanel::new("feed_panel", &mut splits)
        .title("FEED")
        .tabs(ALL_TABS)
        .default_tab(FeedTab::News)
        .width(Width::Medium)
        .resizable(260.0..=480.0)
        .pane_metrics(pane_h, pane_font)
        .show(ctx, t, |ui, t, i, _frac| {
            let tab = tab_snapshot.get(i).copied().unwrap_or(FeedTab::News);
            match tab {
                FeedTab::News =>
                    super::news_panel::draw_content(ui, watchlist, &active_symbol, t),
                FeedTab::Discord =>
                    super::discord_panel::draw_content(ui, watchlist, t),
                FeedTab::Screenshots =>
                    super::screenshot_panel::draw_content(ui, watchlist, t, panes, ap),
            }
        });

    watchlist.feed_splits = splits;
    if resp.close_clicked { watchlist.update_sidebar_state(|s| s.feed_panel_open = false); }
}
