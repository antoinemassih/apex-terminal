//! News feed floating window.
//!
//! Wired to the `news_sentiment` projector via
//! `apex_data::live_state::get_or_fetch_news` + `get_news`. On each frame we:
//!  1. Compute the ticker set = pinned watchlist tickers ∪ {active chart symbol}
//!  2. Kick a background refresh for each (TTL-gated, no thread storms)
//!  3. Merge the cached projector readings into `watchlist.news_items` so the
//!     legacy `NewsItem` shape downstream code knows about keeps working.
//!  4. Render with a sentiment filter dropdown (All / Bullish / Bearish / Neutral).
//! Chrome: `Modal + HeaderStyle::Dialog`. Body: `PanelLoading` while items
//! are still being fetched, `PanelEmpty` when the active filter produces no
//! matches, otherwise a list of `PanelListRow` headlines.

use egui;
use super::super::style::*;
use super::super::chrome::modal::{Modal, Anchor, HeaderStyle, FrameKind};
use super::super::super::gpu::{Watchlist, NewsItem, Theme};
use crate::ui_kit::widgets::{Button, Skeleton};
use crate::ui_kit::widgets::tokens::Variant;
use crate::data::feeds::apex_data::live_state as projector;
use crate::data::feeds::apex_data::types::NewsReading;

/// Sentiment filter selection — `watchlist.news_sentiment_filter` is stored as
/// `i8` (-2 = All, -1 = Bearish, 0 = Neutral, 1 = Bullish) to avoid touching
/// gpu.rs for a new enum.
const FILTER_ALL: i8 = -2;
const FILTER_BEAR: i8 = -1;
const FILTER_NEUT: i8 = 0;
const FILTER_BULL: i8 = 1;

/// Build the ticker union the news panel should fetch:
/// pinned watchlist tickers (stocks only) + the active chart symbol if non-empty.
pub(crate) fn ticker_union(watchlist: &Watchlist, active_symbol: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for sec in &watchlist.sections {
        for item in &sec.items {
            if item.is_option { continue; }
            if !item.pinned { continue; }
            let up = item.symbol.to_uppercase();
            if seen.insert(up.clone()) { out.push(up); }
        }
    }
    if !active_symbol.is_empty() {
        let up = active_symbol.to_uppercase();
        if seen.insert(up.clone()) { out.push(up); }
    }
    out
}

/// Convert a `NewsReading` (projector) into the legacy `NewsItem` shape used
/// by the renderer + tests. `now_ms` is provided so tests can pin the clock.
pub(crate) fn reading_to_item(r: &NewsReading, now_ms: i64) -> NewsItem {
    NewsItem {
        headline: r.headline.clone(),
        source: r.source.clone(),
        timestamp: format_age(now_ms.saturating_sub(r.published_ms)),
        symbol: r.symbol.to_uppercase(),
        sentiment: r.sentiment_tristate(),
        url: r.url.clone(),
    }
}

/// "10m" / "2h" / "3d" style relative-age label.
fn format_age(delta_ms: i64) -> String {
    let s = (delta_ms.max(0) / 1000) as u64;
    if s < 60 { return format!("{s}s"); }
    let m = s / 60;
    if m < 60 { return format!("{m}m"); }
    let h = m / 60;
    if h < 24 { return format!("{h}h"); }
    format!("{}d", h / 24)
}

/// Pull cached projector news for each ticker, sort newest-first, and
/// overwrite `watchlist.news_items`. No-op if the cache is empty (UI keeps
/// showing the previous batch — looks better than a flicker).
fn refresh_from_projector(watchlist: &mut Watchlist, active_symbol: &str, now_ms: i64) {
    let tickers = ticker_union(watchlist, active_symbol);
    if tickers.is_empty() { return; }
    for t in &tickers { projector::get_or_fetch_news(t); }

    let mut all: Vec<NewsReading> = Vec::new();
    for t in &tickers {
        if let Some(resp) = projector::get_news(t) {
            all.extend(resp.articles);
        }
    }
    if all.is_empty() { return; }
    all.sort_by(|a, b| b.published_ms.cmp(&a.published_ms));
    all.truncate(50);
    watchlist.news_items = all.iter().map(|r| reading_to_item(r, now_ms)).collect();
}
use crate::ui_kit::widgets::{PanelEmpty, PanelListRow, PanelLoading};

pub(crate) fn draw(ctx: &egui::Context, watchlist: &mut Watchlist, active_symbol: &str, t: &Theme) {
    if !watchlist.news_open { return; }

    // Trigger projector fetch + populate `news_items`. Cheap when cached.
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0);
    refresh_from_projector(watchlist, active_symbol, now_ms);

    let frame = egui::Frame::popup(&ctx.style())
        .fill(t.toolbar_bg)
        .inner_margin(egui::Margin { left: 0, right: 0, top: 0, bottom: 0 })
        .stroke(egui::Stroke::new(stroke_std(), color_alpha(t.toolbar_border, alpha_heavy())))
        .corner_radius(r_lg_cr());

    let filter_active = watchlist.news_filter_symbol;
    let filter_label = if filter_active { active_symbol } else { "All" };
    let mut toggle_filter = false;
    let mut cycle_sentiment = false;

    // Sentiment chip label.
    let sent_filter = watchlist.news_sentiment_filter;
    let sent_label = match sent_filter {
        FILTER_BULL => "Bull",
        FILTER_BEAR => "Bear",
        FILTER_NEUT => "Neut",
        _           => "Any",
    };

    let resp = Modal::new("NEWS")
        .id("news_feed")
        .ctx(ctx)
        .theme(t)
        .size(egui::vec2(320.0, 440.0))
        .anchor(Anchor::Window { pos: Some(egui::pos2(300.0, 100.0)) })
        .frame_kind(FrameKind::Custom(frame))
        .draggable_header(true)
        .header_style(HeaderStyle::panel())
        .panel_title_actions(|ui| {
            ui.add_space(gap_sm());
            if ui.add(Button::new(filter_label)
                .variant(Variant::Chip)
                .active(filter_active)
                .min_size(egui::vec2(0.0, 16.0))
            ).clicked() {
                toggle_filter = true;
            }
            ui.add_space(gap_xs_mid());
            if ui.add(Button::new(sent_label)
                .variant(Variant::Chip)
                .active(sent_filter != FILTER_ALL)
                .min_size(egui::vec2(0.0, 16.0))
            ).clicked() {
                cycle_sentiment = true;
            }
        })
        .header_style(HeaderStyle::Dialog)
        .separator(false)
        .show(|ui| {
            ui.add_space(gap_xs());
            draw_content(ui, watchlist, active_symbol, t);
        });

    if toggle_filter { watchlist.news_filter_symbol = !watchlist.news_filter_symbol; }
    if cycle_sentiment {
        watchlist.news_sentiment_filter = next_sentiment_filter(watchlist.news_sentiment_filter);
    }
    if resp.closed { watchlist.update_sidebar_state(|s| s.news_open = false); }
}

/// All → Bull → Bear → Neutral → All
pub(crate) fn next_sentiment_filter(cur: i8) -> i8 {
    match cur {
        FILTER_ALL  => FILTER_BULL,
        FILTER_BULL => FILTER_BEAR,
        FILTER_BEAR => FILTER_NEUT,
        _           => FILTER_ALL,
    }
}

/// Returns the subset of `news_items` matching the current symbol + sentiment
/// filters. Public for tests.
pub(crate) fn filtered<'a>(items: &'a [NewsItem], filter_symbol: bool, active_symbol: &str, sentiment: i8) -> Vec<&'a NewsItem> {
    items.iter()
        .filter(|n| !filter_symbol || n.symbol == active_symbol)
        .filter(|n| match sentiment {
            FILTER_ALL  => true,
            FILTER_BULL => n.sentiment > 0,
            FILTER_BEAR => n.sentiment < 0,
            FILTER_NEUT => n.sentiment == 0,
            _ => true,
        })
        .collect()
}

/// Tab body content (no Window wrapper, no header). Used by feed_panel News tab.
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
        .action("filter", filter_tone)
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

            for news in &filtered {
                let resp = super::super::widgets::rows::NewsRow::new(
                    &news.headline, &news.timestamp, &news.source, &news.symbol)
                    .sentiment(news.sentiment)
                    .height(52.0)
                    .theme(t)
                    .show(ui);

                if resp.clicked() && !news.url.is_empty() {
                    let _ = open::that(&news.url);
                }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn item(sym: &str, sent: i8) -> NewsItem {
        NewsItem {
            headline: format!("{sym} story"),
            source: "Reuters".into(),
            timestamp: "1m".into(),
            symbol: sym.into(),
            sentiment: sent,
            url: format!("https://example.com/{sym}"),
        }
    }

    #[test]
    fn sentiment_filter_all_includes_everything() {
        let items = vec![item("AAPL", 1), item("TSLA", -1), item("SPY", 0)];
        let out = filtered(&items, false, "", FILTER_ALL);
        assert_eq!(out.len(), 3);
    }

    #[test]
    fn sentiment_filter_bullish_only() {
        let items = vec![item("AAPL", 1), item("TSLA", -1), item("SPY", 0), item("MSFT", 1)];
        let out = filtered(&items, false, "", FILTER_BULL);
        assert_eq!(out.len(), 2);
        assert!(out.iter().all(|n| n.sentiment > 0));
    }

    #[test]
    fn sentiment_filter_bearish_only() {
        let items = vec![item("AAPL", 1), item("TSLA", -1), item("SPY", 0), item("NVDA", -1)];
        let out = filtered(&items, false, "", FILTER_BEAR);
        assert_eq!(out.len(), 2);
        assert!(out.iter().all(|n| n.sentiment < 0));
    }

    #[test]
    fn sentiment_filter_neutral_only() {
        let items = vec![item("AAPL", 1), item("SPY", 0), item("QQQ", 0)];
        let out = filtered(&items, false, "", FILTER_NEUT);
        assert_eq!(out.len(), 2);
        assert!(out.iter().all(|n| n.sentiment == 0));
    }

    #[test]
    fn symbol_filter_combines_with_sentiment() {
        let items = vec![item("AAPL", 1), item("AAPL", -1), item("TSLA", 1)];
        let out = filtered(&items, true, "AAPL", FILTER_BULL);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].symbol, "AAPL");
    }

    #[test]
    fn next_sentiment_cycles_all_bull_bear_neut_all() {
        assert_eq!(next_sentiment_filter(FILTER_ALL),  FILTER_BULL);
        assert_eq!(next_sentiment_filter(FILTER_BULL), FILTER_BEAR);
        assert_eq!(next_sentiment_filter(FILTER_BEAR), FILTER_NEUT);
        assert_eq!(next_sentiment_filter(FILTER_NEUT), FILTER_ALL);
    }

    #[test]
    fn reading_to_item_maps_sentiment_label() {
        let r = NewsReading {
            symbol: "aapl".into(), headline: "h".into(), source: "Bloomberg".into(),
            url: "https://x".into(), published_ms: 1_000, sentiment: "bullish".into(), score: 0.8,
        };
        let item = reading_to_item(&r, 61_000);
        assert_eq!(item.symbol, "AAPL");
        assert_eq!(item.sentiment, 1);
        assert_eq!(item.timestamp, "1m");
    }

    #[test]
    fn reading_to_item_uses_score_when_label_missing() {
        let r = NewsReading { score: -0.5, ..Default::default() };
        assert_eq!(reading_to_item(&r, 0).sentiment, -1);
        let r2 = NewsReading { score: 0.5, ..Default::default() };
        assert_eq!(reading_to_item(&r2, 0).sentiment, 1);
        let r3 = NewsReading { score: 0.0, ..Default::default() };
        assert_eq!(reading_to_item(&r3, 0).sentiment, 0);
    }
}
