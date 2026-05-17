//! News feed floating window.
//!
//! Chrome: `Modal + HeaderStyle::Dialog`. Body: `PanelLoading` while items
//! are still being fetched, `PanelEmpty` when the active filter produces no
//! matches, otherwise a list of `PanelListRow` headlines.

use egui;
use super::super::style::*;
use super::super::widgets::modal::{Modal, Anchor, HeaderStyle, FrameKind};
use super::super::super::gpu::{Watchlist, NewsItem, Theme};
use crate::ui_kit::widgets::{PanelEmpty, PanelListRow, PanelLoading};

pub(crate) fn draw(ctx: &egui::Context, watchlist: &mut Watchlist, active_symbol: &str, t: &Theme) {
    if !watchlist.news_open { return; }

    let frame = egui::Frame::popup(&ctx.style())
        .fill(t.toolbar_bg)
        .inner_margin(egui::Margin { left: 0, right: 0, top: 0, bottom: 0 })
        .stroke(egui::Stroke::new(stroke_std(), color_alpha(t.toolbar_border, alpha_heavy())))
        .corner_radius(r_lg_cr());

    let resp = Modal::new("NEWS")
        .id("news_feed")
        .ctx(ctx)
        .theme(t)
        .size(egui::vec2(320.0, 440.0))
        .anchor(Anchor::Window { pos: Some(egui::pos2(300.0, 100.0)) })
        .frame_kind(FrameKind::Custom(frame))
        .draggable_header(true)
        .header_style(HeaderStyle::Dialog)
        .separator(false)
        .show(|ui| {
            ui.add_space(gap_xs());
            draw_content(ui, watchlist, active_symbol, t);
        });

    if resp.closed { watchlist.news_open = false; }
}

/// Tab body content (no Window wrapper, no header). Used by the floating
/// modal above and by the feed_panel News tab.
pub(crate) fn draw_content(ui: &mut egui::Ui, watchlist: &mut Watchlist, active_symbol: &str, t: &Theme) {
    let active_label = if watchlist.news_filter_symbol { active_symbol } else { "All" };
    let mut toggle_filter = false;

    let section_title = if watchlist.news_filter_symbol { "HEADLINES" } else { "HEADLINES · ALL" };
    let filter_tone = if watchlist.news_filter_symbol {
        crate::ui_kit::widgets::PanelTone::Accent
    } else {
        crate::ui_kit::widgets::PanelTone::Default
    };

    let resp = crate::ui_kit::widgets::PanelSection::new(section_title)
        .meta(active_label.to_string())
        .action(("filter", filter_tone), |_, _| {})
        .show(ui, t, |ui, t| {
            let filtered: Vec<&NewsItem> = watchlist.news_items.iter()
                .filter(|n| !watchlist.news_filter_symbol || n.symbol == active_symbol)
                .collect();

            if watchlist.news_items.is_empty() {
                PanelLoading::new().reason("Fetching news").show(ui, t);
                return;
            }
            if filtered.is_empty() {
                PanelEmpty::new("No headlines")
                    .hint("Try toggling the symbol filter")
                    .show(ui, t);
                return;
            }

            egui::ScrollArea::vertical()
                .id_salt("news_items")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for (idx, news) in filtered.iter().enumerate() {
                        let id = format!("news_{idx}");
                        let primary = news.headline.clone();
                        let secondary = format!("{} · {} · {}", news.source, news.symbol, news.timestamp);
                        let sentiment = news.sentiment;
                        let row = PanelListRow::new(&id)
                            .dense(false)
                            .primary(&primary)
                            .secondary(&secondary)
                            .leading(move |ui, t| {
                                let color = match sentiment {
                                    s if s > 0 => t.bull,
                                    s if s < 0 => t.bear,
                                    _          => t.dim,
                                };
                                let (r, _) = ui.allocate_exact_size(egui::vec2(6.0, 6.0), egui::Sense::hover());
                                ui.painter().circle_filled(r.center(), 3.0, color);
                            })
                            .show(ui, t);
                        if row.clicked() && !news.url.is_empty() {
                            // TODO: open URL
                        }
                    }
                });
        });

    if resp.action_clicked { toggle_filter = true; }
    if toggle_filter { watchlist.news_filter_symbol = !watchlist.news_filter_symbol; }
}
