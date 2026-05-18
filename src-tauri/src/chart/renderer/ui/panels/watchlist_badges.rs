//! Per-row watchlist badges derived from the `corporate_actions` projector.
//!
//! Icons:
//!   🔴 halt within 24h
//!   💰 ex-dividend within 7d
//!   📊 split within 30d
//!   📰 N unread news count in the last 24h
//!
//! Logic is pure (no UI) so it can be tested headless. The UI side just
//! concatenates `badges_for` glyphs into a small text label next to the row.

use crate::data::feeds::apex_data::live_state as projector;
use crate::data::feeds::apex_data::types::{ActionKind, CorporateActionsReading};

pub(crate) const ICON_HALT: &str    = "🔴";
pub(crate) const ICON_DIV: &str     = "💰";
pub(crate) const ICON_SPLIT: &str   = "📊";
pub(crate) const ICON_NEWS: &str    = "📰";

const MS_PER_HOUR: i64 = 3_600_000;
const MS_PER_DAY:  i64 = 24 * MS_PER_HOUR;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Badge {
    pub icon: &'static str,
    pub tooltip: String,
}

/// Compute the badge list for a single ticker given its corporate actions
/// reading and an unread-news count. `now_ms` is provided explicitly so
/// callers (and tests) control the clock.
pub(crate) fn badges_for(
    reading: Option<&CorporateActionsReading>,
    unread_news_24h: u32,
    now_ms: i64,
) -> Vec<Badge> {
    let mut out: Vec<Badge> = Vec::new();
    if let Some(r) = reading {
        for e in &r.events {
            let dt = e.event_ms - now_ms;
            match e.kind {
                ActionKind::Halt if dt >= -MS_PER_HOUR && dt <= MS_PER_DAY => {
                    out.push(Badge { icon: ICON_HALT, tooltip: format!("Halt scheduled: {}", e.detail.trim()) });
                }
                ActionKind::ExDividend if dt >= 0 && dt <= 7 * MS_PER_DAY => {
                    out.push(Badge { icon: ICON_DIV, tooltip: format!("Ex-div within 7d: {}", e.detail.trim()) });
                }
                ActionKind::Split if dt >= 0 && dt <= 30 * MS_PER_DAY => {
                    out.push(Badge { icon: ICON_SPLIT, tooltip: format!("Split within 30d: {}", e.detail.trim()) });
                }
                _ => {}
            }
        }
    }
    if unread_news_24h > 0 {
        out.push(Badge { icon: ICON_NEWS, tooltip: format!("{unread_news_24h} unread news (24h)") });
    }
    out
}

/// Convenience wrapper that fetches via the live_state caches. Returns the
/// rendered badge text + concatenated tooltip; the UI side just paints them.
pub(crate) fn badges_for_ticker(ticker: &str, now_ms: i64) -> (String, String) {
    projector::get_or_fetch_corp_actions(ticker);
    projector::get_or_fetch_news(ticker);
    let reading = projector::get_corp_actions(ticker);

    // Unread news = count of articles in the last 24h. We don't yet track a
    // "read" flag — the projector marks each article with `published_ms`.
    let unread = projector::get_news(ticker)
        .map(|n| n.articles.iter().filter(|a| now_ms - a.published_ms <= MS_PER_DAY).count() as u32)
        .unwrap_or(0);
    let list = badges_for(reading.as_ref(), unread, now_ms);
    let icons = list.iter().map(|b| {
        if b.icon == ICON_NEWS { format!("{}{}", b.icon, unread) } else { b.icon.to_string() }
    }).collect::<Vec<_>>().join(" ");
    let tooltip = list.iter().map(|b| b.tooltip.clone()).collect::<Vec<_>>().join("\n");
    (icons, tooltip)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::feeds::apex_data::types::UpcomingEvent;

    fn ev(kind: ActionKind, event_ms: i64, detail: &str) -> UpcomingEvent {
        UpcomingEvent { kind, event_ms, detail: detail.into() }
    }
    fn reading(events: Vec<UpcomingEvent>) -> CorporateActionsReading {
        CorporateActionsReading { ticker: "T".into(), events, updated_at_ms: 0 }
    }

    #[test]
    fn no_events_no_badges() {
        let out = badges_for(None, 0, 0);
        assert!(out.is_empty());
    }

    #[test]
    fn halt_within_24h_emits_red_circle() {
        let r = reading(vec![ev(ActionKind::Halt, 10 * MS_PER_HOUR, "LULD")]);
        let out = badges_for(Some(&r), 0, 0);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].icon, ICON_HALT);
    }

    #[test]
    fn halt_beyond_24h_suppressed() {
        let r = reading(vec![ev(ActionKind::Halt, 48 * MS_PER_HOUR, "LULD")]);
        assert!(badges_for(Some(&r), 0, 0).is_empty());
    }

    #[test]
    fn ex_div_within_7d_emits_money_bag() {
        let r = reading(vec![ev(ActionKind::ExDividend, 3 * MS_PER_DAY, "0.24/share")]);
        let out = badges_for(Some(&r), 0, 0);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].icon, ICON_DIV);
        assert!(out[0].tooltip.contains("0.24"));
    }

    #[test]
    fn ex_div_beyond_7d_suppressed() {
        let r = reading(vec![ev(ActionKind::ExDividend, 10 * MS_PER_DAY, "x")]);
        assert!(badges_for(Some(&r), 0, 0).is_empty());
    }

    #[test]
    fn ex_div_in_past_suppressed() {
        let r = reading(vec![ev(ActionKind::ExDividend, -MS_PER_DAY, "x")]);
        assert!(badges_for(Some(&r), 0, 0).is_empty());
    }

    #[test]
    fn split_within_30d_emits_chart_icon() {
        let r = reading(vec![ev(ActionKind::Split, 20 * MS_PER_DAY, "2-for-1")]);
        let out = badges_for(Some(&r), 0, 0);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].icon, ICON_SPLIT);
    }

    #[test]
    fn split_beyond_30d_suppressed() {
        let r = reading(vec![ev(ActionKind::Split, 45 * MS_PER_DAY, "2-for-1")]);
        assert!(badges_for(Some(&r), 0, 0).is_empty());
    }

    #[test]
    fn unread_news_emits_news_icon() {
        let out = badges_for(None, 3, 0);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].icon, ICON_NEWS);
        assert!(out[0].tooltip.contains("3"));
    }

    #[test]
    fn earnings_does_not_emit_a_badge_here() {
        // Earnings badge is rendered by the existing watchlist row code
        // (`earnings_days` field); the corp-actions projector shouldn't
        // double-render it. Keep the behavior asserted so we notice if
        // someone adds an ActionKind::Earnings arm later.
        let r = reading(vec![ev(ActionKind::Earnings, MS_PER_DAY, "EPS 1.23")]);
        assert!(badges_for(Some(&r), 0, 0).is_empty());
    }

    #[test]
    fn multiple_events_yield_multiple_badges_in_order() {
        let r = reading(vec![
            ev(ActionKind::Halt, MS_PER_HOUR, "LULD"),
            ev(ActionKind::ExDividend, 2 * MS_PER_DAY, "0.50"),
            ev(ActionKind::Split, 15 * MS_PER_DAY, "3-for-1"),
        ]);
        let out = badges_for(Some(&r), 5, 0);
        assert_eq!(out.len(), 4);
        assert_eq!(out[0].icon, ICON_HALT);
        assert_eq!(out[1].icon, ICON_DIV);
        assert_eq!(out[2].icon, ICON_SPLIT);
        assert_eq!(out[3].icon, ICON_NEWS);
    }
}
