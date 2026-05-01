//! Feed panel — sidebar with subdivided sections, each with its own tab bar.

use egui;
use super::style::*;
use super::widgets;
use super::super::gpu::{Watchlist, Chart, Theme, SplitSection};
use crate::chart_renderer::FeedTab;

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

    egui::SidePanel::right("feed_panel")
        .default_width(300.0)
        .min_width(260.0)
        .max_width(480.0)
        .resizable(true)
        .frame(widgets::frames::PanelFrame::new(t.toolbar_bg, t.toolbar_border).theme(t).build())
        .show(ctx, |ui| {
            // Header
            let header = ui.horizontal(|ui| {
                ui.set_min_height(26.0);
                ui.add(widgets::text::SectionLabel::new("FEED").color(t.accent));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if close_button(ui, t.dim) { watchlist.feed_panel_open = false; }
                    if ui.add(egui::Button::new(egui::RichText::new("+").monospace().size(FONT_SM).color(t.dim))
                        .fill(egui::Color32::TRANSPARENT).min_size(egui::vec2(20.0, 20.0))).clicked() {
                        let used: Vec<FeedTab> = watchlist.feed_splits.iter().map(|s| s.tab).collect();
                        let next = ALL_TABS.iter().find(|(tab, _)| !used.contains(tab))
                            .map(|(tab, _)| *tab).unwrap_or(FeedTab::News);
                        if let Some(last) = watchlist.feed_splits.last_mut() { last.frac *= 0.5; }
                        let frac = watchlist.feed_splits.last().map(|s| s.frac).unwrap_or(1.0);
                        watchlist.feed_splits.push(SplitSection::new(next, frac));
                    }
                });
            });
            let line_y = header.response.rect.max.y;
            ui.painter().line_segment(
                [egui::pos2(ui.min_rect().left(), line_y), egui::pos2(ui.min_rect().right(), line_y)],
                egui::Stroke::new(1.0, color_alpha(t.toolbar_border, ALPHA_MUTED)));

            let available_h = ui.available_height();
            let n = watchlist.feed_splits.len();
            if watchlist.feed_splits.is_empty() {
                watchlist.feed_splits.push(SplitSection::new(FeedTab::News, 1.0));
            }

            let divider_total = n.saturating_sub(1) as f32 * 6.0;
            let tab_bar_total = n as f32 * 28.0;
            let content_h = (available_h - divider_total - tab_bar_total).max(40.0);
            let total_frac: f32 = watchlist.feed_splits.iter().map(|s| s.frac).sum();
            let norm = if total_frac > 0.001 { 1.0 / total_frac } else { 1.0 };
            let heights: Vec<f32> = watchlist.feed_splits.iter()
                .map(|s| (s.frac * norm * content_h).max(30.0)).collect();

            let mut remove_idx: Option<usize> = None;
            let mut divider_drags: Vec<(usize, f32)> = Vec::new();

            for i in 0..n {
                let tab = watchlist.feed_splits[i].tab;
                let h = heights[i];
                let can_close = n > 1;

                ui.horizontal(|ui| {
                    ui.set_min_height(26.0);
                    let mut sel_tab = tab;
                    widgets::tabs::TabBar::new(&mut sel_tab, ALL_TABS)
                        .accent(t.accent)
                        .dim(t.dim.gamma_multiply(0.5))
                        .font_size(FONT_XS)
                        .underline(false)
                        .min_height(22.0)
                        .show(ui);
                    if sel_tab != tab {
                        watchlist.feed_splits[i].tab = sel_tab;
                    }
                    if can_close {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.add(egui::Button::new(egui::RichText::new("\u{00D7}").size(FONT_SM).color(t.dim.gamma_multiply(0.4)))
                                .fill(egui::Color32::TRANSPARENT).min_size(egui::vec2(18.0, 18.0))).clicked() {
                                remove_idx = Some(i);
                            }
                        });
                    }
                });
                ui.painter().line_segment(
                    [egui::pos2(ui.min_rect().left(), ui.min_rect().bottom()),
                     egui::pos2(ui.min_rect().right(), ui.min_rect().bottom())],
                    egui::Stroke::new(0.5, color_alpha(t.toolbar_border, ALPHA_FAINT)));

                egui::ScrollArea::vertical().id_salt(format!("feed_sec_{}", i)).max_height(h).show(ui, |ui| {
                    match tab {
                        FeedTab::News => super::news_panel::draw_content(ui, watchlist, &active_symbol, t),
                        FeedTab::Discord => super::discord_panel::draw_content(ui, watchlist, t),
                        FeedTab::Screenshots => super::screenshot_panel::draw_content(ui, watchlist, t, panes, ap),
                    }
                });

                if i + 1 < n {
                    let d = split_divider(ui, &format!("fdiv_{}", i), t.dim);
                    if d != 0.0 { divider_drags.push((i, d)); }
                }
            }

            if let Some(idx) = remove_idx {
                let removed = watchlist.feed_splits[idx].frac;
                watchlist.feed_splits.remove(idx);
                if !watchlist.feed_splits.is_empty() {
                    let share = removed / watchlist.feed_splits.len() as f32;
                    for s in &mut watchlist.feed_splits { s.frac += share; }
                }
            }
            for (idx, delta) in divider_drags {
                if idx + 1 < watchlist.feed_splits.len() {
                    let fd = delta / available_h.max(1.0);
                    watchlist.feed_splits[idx].frac = (watchlist.feed_splits[idx].frac + fd).clamp(0.05, 0.90);
                    watchlist.feed_splits[idx + 1].frac = (watchlist.feed_splits[idx + 1].frac - fd).clamp(0.05, 0.90);
                }
            }
        });
}
