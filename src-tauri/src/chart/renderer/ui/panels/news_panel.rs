//! News feed floating window.

use egui;
use super::super::style::*;
use super::super::widgets as widgets;
use super::super::widgets::modal::{Modal, Anchor, HeaderStyle, FrameKind};
use super::super::super::gpu::{Watchlist, NewsItem, Theme};
use crate::ui_kit::widgets::{Button, Skeleton};
use crate::ui_kit::widgets::tokens::Variant;

pub(crate) fn draw(ctx: &egui::Context, watchlist: &mut Watchlist, active_symbol: &str, t: &Theme) {
    if !watchlist.news_open { return; }

    let frame = egui::Frame::popup(&ctx.style())
        .fill(t.toolbar_bg)
        .inner_margin(egui::Margin { left: 0, right: 0, top: 0, bottom: 0 })
        .stroke(egui::Stroke::new(stroke_std(), color_alpha(t.toolbar_border, alpha_heavy())))
        .corner_radius(r_lg_cr());

    let filter_active = watchlist.news_filter_symbol;
    let filter_label = if filter_active { active_symbol } else { "All" };
    let filter_col = if filter_active { t.accent } else { t.dim };
    let mut toggle_filter = false;

    let resp = Modal::new("NEWS")
        .id("news_feed")
        .ctx(ctx)
        .theme(t)
        .size(egui::vec2(280.0, 400.0))
        .anchor(Anchor::Window { pos: Some(egui::pos2(300.0, 100.0)) })
        .frame_kind(FrameKind::Custom(frame))
        .draggable_header(true)
        .header_style(HeaderStyle::panel())
        .panel_title_actions(|ui| {
            ui.add_space(8.0);
            if ui.add(Button::new(filter_label)
                .variant(Variant::Chrome)
                .fg(filter_col)
                .fill(color_alpha(filter_col, alpha_ghost()))
                .corner_radius(current().r_md as f32)
                .stroke(egui::Stroke::new(stroke_thin(), color_alpha(filter_col, alpha_muted())))
                .min_size(egui::vec2(0.0, 16.0))
                .frameless(true)
            ).clicked() {
                toggle_filter = true;
            }
        })
        .separator(false)
        .show(|ui| {
            let w = ui.available_width();
            ui.add_space(4.0);
            let div_rect = egui::Rect::from_min_size(
                egui::pos2(ui.cursor().min.x, ui.cursor().min.y),
                egui::vec2(w, 1.0),
            );
            ui.painter().rect_filled(div_rect, 0.0, color_alpha(t.toolbar_border, alpha_dim()));
            ui.add_space(4.0);

            draw_content(ui, watchlist, active_symbol, t);
        });

    if toggle_filter { watchlist.news_filter_symbol = !watchlist.news_filter_symbol; }
    if resp.closed { watchlist.news_open = false; }
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
                ui.add_space(8.0);
                let row_w = (w - 16.0).max(80.0);
                for _ in 0..4 {
                    ui.add_space(4.0);
                    Skeleton::text(row_w).show(ui, t);
                    ui.add_space(2.0);
                    Skeleton::text(row_w * 0.55).show(ui, t);
                    ui.add_space(8.0);
                }
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
