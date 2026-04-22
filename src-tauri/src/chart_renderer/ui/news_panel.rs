//! News feed floating window.

use egui;
use super::style::*;
use super::super::gpu::{Watchlist, NewsItem, Theme};

const fn rgb(r: u8, g: u8, b: u8) -> egui::Color32 { egui::Color32::from_rgb(r, g, b) }

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
            .stroke(egui::Stroke::new(STROKE_STD, color_alpha(t.toolbar_border, ALPHA_HEAVY)))
            .corner_radius(RADIUS_LG))
        .show(ctx, |ui| {
            let w = ui.available_width();

            // Header
            ui.horizontal(|ui| {
                ui.add_space(8.0);
                ui.label(egui::RichText::new("NEWS").monospace().size(10.0).strong().color(t.accent));
                ui.add_space(6.0);
                let filter_label = if watchlist.news_filter_symbol { active_symbol } else { "All" };
                let filter_col = if watchlist.news_filter_symbol { t.accent } else { t.dim };
                if ui.add(egui::Button::new(
                    egui::RichText::new(filter_label).monospace().size(9.0).color(filter_col))
                    .fill(color_alpha(filter_col, ALPHA_GHOST))
                    .corner_radius(RADIUS_MD)
                    .stroke(egui::Stroke::new(STROKE_THIN, color_alpha(filter_col, ALPHA_MUTED)))
                    .min_size(egui::vec2(0.0, 16.0))
                ).clicked() {
                    watchlist.news_filter_symbol = !watchlist.news_filter_symbol;
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(6.0);
                    if close_button(ui, t.dim) { close_news = true; }
                });
            });
            ui.add_space(4.0);

            let div_rect = egui::Rect::from_min_size(
                egui::pos2(ui.cursor().min.x, ui.cursor().min.y),
                egui::vec2(w, 1.0),
            );
            ui.painter().rect_filled(div_rect, 0.0, color_alpha(t.toolbar_border, ALPHA_DIM));
            ui.add_space(5.0);

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
                    ui.label(egui::RichText::new("No news for this symbol")
                        .monospace().size(9.0).color(t.dim.gamma_multiply(0.5)));
                });
            }

            for news in &filtered {
                let m = 8.0;
                let item_rect = egui::Rect::from_min_size(
                    egui::pos2(ui.cursor().min.x, ui.cursor().min.y),
                    egui::vec2(w, 52.0),
                );
                let item_resp = ui.allocate_rect(item_rect, egui::Sense::click());
                let bg = if item_resp.hovered() { color_alpha(t.toolbar_border, ALPHA_MUTED) } else { egui::Color32::TRANSPARENT };
                ui.painter().rect_filled(item_rect, 2.0, bg);

                // Headline
                let headline_rect = egui::Rect::from_min_size(
                    egui::pos2(item_rect.min.x + m, item_rect.min.y + 4.0),
                    egui::vec2(w - m * 2.0, 24.0),
                );
                ui.painter().text(
                    headline_rect.left_top(), egui::Align2::LEFT_TOP,
                    &news.headline, egui::FontId::monospace(9.0),
                    egui::Color32::from_gray(230),
                );

                let meta_y = item_rect.min.y + 30.0;

                // Source badge
                let source_col = match news.source.as_str() {
                    "Reuters" => rgb(255, 140, 0),
                    "Bloomberg" => rgb(100, 180, 255),
                    "CNBC" => rgb(0, 180, 120),
                    "Benzinga" => rgb(180, 100, 255),
                    _ => t.dim,
                };
                let source_rect = egui::Rect::from_min_size(egui::pos2(item_rect.min.x + m, meta_y), egui::vec2(50.0, 14.0));
                ui.painter().rect_filled(source_rect, 2.0, color_alpha(source_col, ALPHA_SUBTLE));
                ui.painter().text(source_rect.center(), egui::Align2::CENTER_CENTER, &news.source, egui::FontId::monospace(7.0), source_col);

                // Timestamp
                ui.painter().text(egui::pos2(item_rect.min.x + m + 55.0, meta_y + 7.0), egui::Align2::LEFT_CENTER, &news.timestamp, egui::FontId::monospace(7.0), t.dim.gamma_multiply(0.5));

                // Symbol badge
                let sym_rect = egui::Rect::from_min_size(egui::pos2(item_rect.min.x + m + 95.0, meta_y), egui::vec2(36.0, 14.0));
                ui.painter().rect_filled(sym_rect, 2.0, color_alpha(t.accent, ALPHA_SOFT));
                ui.painter().text(sym_rect.center(), egui::Align2::CENTER_CENTER, &news.symbol, egui::FontId::monospace(7.0), t.accent);

                // Sentiment dot
                let dot_col = match news.sentiment {
                    1 => t.bull, -1 => t.bear, _ => t.dim.gamma_multiply(0.4),
                };
                ui.painter().circle_filled(egui::pos2(item_rect.right() - m - 4.0, meta_y + 7.0), 3.5, dot_col);

                if item_resp.clicked() && !news.url.is_empty() {
                    // TODO: open URL
                }
            }
        });
}
