//! News feed floating window.

use egui;
use super::super::style::*;
use super::super::widgets as widgets_compat;
// widgets alias
#[allow(unused_imports)]
use super::super::widgets as widgets;
use super::super::widgets::buttons::ChromeBtn;
use super::super::widgets::headers::PanelHeaderWithClose;
use super::super::super::gpu::{Watchlist, NewsItem, Theme};

pub(crate) fn draw(ctx: &egui::Context, watchlist: &mut Watchlist, active_symbol: &str, t: &Theme) {
    if !watchlist.news_open { return; }

    let mut close_news = false;
    egui::Window::new("news_feed")
        .default_pos(egui::pos2(300.0, 100.0))
        .default_size(egui::vec2(280.0, 400.0))
        .resizable(true)
        .movable(true)
        .title_bar(false)
        .frame(egui::Frame::popup(&ctx.style())
            .fill(t.toolbar_bg)
            .inner_margin(egui::Margin { left: 0, right: 0, top: 0, bottom: 0 })
            .stroke(egui::Stroke::new(stroke_std(), color_alpha(t.toolbar_border, alpha_heavy())))
            .corner_radius(r_lg_cr()))
        .show(ctx, |ui| {
            let w = ui.available_width();

            // Header — title + filter chip (next to title) + close
            let closed = PanelHeaderWithClose::new("NEWS").theme(t).show_with_title_actions(ui, |ui| {
                ui.add_space(8.0);
                let filter_label = if watchlist.news_filter_symbol { active_symbol } else { "All" };
                let filter_col = if watchlist.news_filter_symbol { t.accent } else { t.dim };
                if ui.add(ChromeBtn::new(
                    egui::RichText::new(filter_label).monospace().size(font_sm_tight()).color(filter_col))
                    .fill(color_alpha(filter_col, alpha_ghost()))
                    .corner_radius(r_md_cr())
                    .stroke(egui::Stroke::new(stroke_thin(), color_alpha(filter_col, alpha_muted())))
                    .min_size(egui::vec2(0.0, 16.0))
                ).clicked() {
                    watchlist.news_filter_symbol = !watchlist.news_filter_symbol;
                }
            });
            if closed { close_news = true; }
            ui.add_space(4.0);

            let div_rect = egui::Rect::from_min_size(
                egui::pos2(ui.cursor().min.x, ui.cursor().min.y),
                egui::vec2(w, 1.0),
            );
            ui.painter().rect_filled(div_rect, 0.0, color_alpha(t.toolbar_border, alpha_dim()));
            ui.add_space(4.0);

            draw_content(ui, watchlist, active_symbol, t);
        });
    if close_news { watchlist.news_open = false; }
}

/// Tab body content (no Window wrapper, no header). Used by feed_panel News tab.
pub(crate) fn draw_content(ui: &mut egui::Ui, watchlist: &mut Watchlist, active_symbol: &str, t: &Theme) {
    let w = ui.available_width();
    egui::ScrollArea::vertical()
        .id_salt("news_items")
        .show(ui, |ui| {
            ui.set_min_width(w - 4.0);
            let filtered: Vec<&NewsItem> = watchlist.news_items.iter()
                .filter(|n| !watchlist.news_filter_symbol || n.symbol == active_symbol)
                .collect();

            if filtered.is_empty() {
                ui.add_space(20.0);
                ui.vertical_centered(|ui| {
                    ui.add(widgets::text::MonospaceCode::new("No news for this symbol").xs().color(t.dim).gamma(0.5));
                });
            }

            for news in &filtered {
                let resp = widgets::rows::NewsRow::new(
                    &news.headline, &news.timestamp, &news.source, &news.symbol)
                    .sentiment(news.sentiment)
                    .height(52.0)
                    .theme(t)
                    .show(ui);

                if resp.clicked() && !news.url.is_empty() {
                    // TODO: open URL
                }
            }
        });
}
