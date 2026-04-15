//! Feed panel — unified News + Discord + Screenshots sidebar.

use egui;
use super::style::*;
use super::super::gpu::{Watchlist, Chart, Theme};
use crate::chart_renderer::FeedTab;

pub(crate) fn draw(
    ctx: &egui::Context,
    watchlist: &mut Watchlist,
    panes: &mut [Chart],
    ap: usize,
    t: &Theme,
) {
    if !watchlist.feed_panel_open { return; }

    // Discord background drain needs &Context; do it once per frame regardless of tab.
    super::discord_panel::drain_background(ctx, watchlist);

    let active_symbol = if !panes.is_empty() { panes[ap].symbol.clone() } else { String::new() };

    egui::SidePanel::right("feed_panel")
        .default_width(320.0)
        .min_width(280.0)
        .max_width(480.0)
        .resizable(true)
        .frame(panel_frame(t.toolbar_bg, t.toolbar_border))
        .show(ctx, |ui| {
            // Tab bar row with close button
            let tab_row = ui.horizontal(|ui| {
                ui.set_min_height(26.0);
                tab_bar(ui, &mut watchlist.feed_tab, &[
                    (FeedTab::News, "News"),
                    (FeedTab::Discord, "Discord"),
                    (FeedTab::Screenshots, "Screenshots"),
                ], t.accent, t.dim);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if close_button(ui, t.dim) { watchlist.feed_panel_open = false; }
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
            match watchlist.feed_tab {
                FeedTab::News => {
                    super::news_panel::draw_content(ui, watchlist, &active_symbol, t);
                }
                FeedTab::Discord => {
                    super::discord_panel::draw_content(ui, watchlist, t);
                }
                FeedTab::Screenshots => {
                    super::screenshot_panel::draw_content(ui, watchlist, t, panes, ap);
                }
            }
        });
}
