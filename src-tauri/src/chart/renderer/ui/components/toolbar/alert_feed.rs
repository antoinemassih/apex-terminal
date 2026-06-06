//! Alert badge feed — trading-event badges (order fills, price alerts, signals,
//! errors) rendered as a horizontal scrolling pill-strip in the toolnav.
//!
//! Push alerts from anywhere via [`push`]. Click a badge to dismiss it.
//! Eventually this replaces the toast system for all trading-related events.
//!
//! ## Storage
//! The badge store has been merged into `NotificationManager` (notification.rs).
//! All writes/reads delegate to `notification::push_badge` / `badges_snapshot` /
//! `dismiss_badge`.  `AlertKind` and `AlertItem` now live in `notification.rs`
//! and are re-exported here so that existing `use super::alert_feed::AlertKind`
//! call-sites continue to compile unchanged.

pub use crate::chart_renderer::ui::tools::notification::{AlertKind, AlertItem};

use crate::chart_renderer::ui::tools::notification as notification;
use crate::chart_renderer::ui::style::tint;
use crate::ui_kit::sx::Tone;
use crate::ui_kit::widgets::paint_badge;
use egui::{Color32, FontId, Sense};

type Theme = crate::chart_renderer::gpu::Theme;

/// Push a new alert onto the feed. Oldest entry is dropped when the cap is reached.
pub fn push(kind: AlertKind, symbol: Option<String>, message: impl Into<String>) {
    notification::push_badge(kind, symbol, message);
}

/// Dismiss (remove) a single alert by id.
pub fn dismiss(id: u64) {
    notification::dismiss_badge(id);
}

/// Remove all alerts from the feed.
pub fn clear_all() {
    notification::clear_all_badges();
}

fn kind_color(kind: AlertKind, t: &Theme) -> Color32 {
    kind.severity().color(t)
}

fn kind_tag(kind: AlertKind) -> &'static str {
    match kind {
        AlertKind::OrderFilled   => "FILLED",
        AlertKind::OrderRejected => "REJECTED",
        AlertKind::OrderPending  => "PENDING",
        AlertKind::PriceAlert    => "ALERT",
        AlertKind::Signal        => "SIGNAL",
        AlertKind::Error         => "ERROR",
        AlertKind::Warning       => "WARN",
    }
}

fn badge_text(a: &AlertItem) -> String {
    match &a.symbol {
        Some(s) if !s.is_empty() => format!("{} {} {}", kind_tag(a.kind), s, a.message),
        _                         => format!("{} {}", kind_tag(a.kind), a.message),
    }
}

/// Seed two placeholder alerts so the feed is non-empty on first render.
/// Remove this once real alerts are wired in from the broker / signal pipeline.
fn seed_placeholders() {
    use std::sync::Once;
    static SEEDED: Once = Once::new();
    SEEDED.call_once(|| {
        push(AlertKind::OrderFilled,   Some("AAPL".into()), "100 @ 213.45");
        push(AlertKind::Signal,        Some("SPY".into()),  "Bull cross 5m EMA 9/21");
    });
}

/// Render all pending alerts as horizontal dismissible badges.
///
/// Call from within a left-to-right horizontal layout; consumes all remaining
/// available width via an inner `ScrollArea`. Clicking a badge dismisses it.
pub fn render_badge_feed(ui: &mut egui::Ui, t: &Theme) {
    seed_placeholders();
    use crate::chart_renderer::ui::style::{
        font_xs, font_sm, gap_xs, gap_sm,
        BADGE_HEIGHT, BADGE_MIN_WIDTH, BADGE_ACCENT_WIDTH,
        BADGE_DISMISS_WIDTH, BADGE_DISMISS_PADDING,
        ALPHA_SECONDARY_TEXT, ALPHA_INTERACTIVE,
    };

    let alerts: Vec<AlertItem> = notification::badges_snapshot();

    if alerts.is_empty() {
        ui.label(
            egui::RichText::new("No alerts")
                .size(font_xs())
                .color(tint(t, Tone::Dim, ALPHA_SECONDARY_TEXT)),
        );
        return;
    }

    let mut to_dismiss: Option<u64> = None;

    egui::ScrollArea::horizontal()
        .id_source("alert_feed_scroll")
        .show(ui, |ui| {
            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing.x = gap_sm();

                for alert in &alerts {
                    let accent = kind_color(alert.kind, t);
                    let text   = badge_text(alert);

                    let text_w = ui.fonts(|f| {
                        f.layout_no_wrap(text.clone(), FontId::monospace(font_xs()), t.text)
                            .rect.width()
                    });
                    let badge_w = (BADGE_ACCENT_WIDTH + gap_xs() + text_w + gap_sm() + BADGE_DISMISS_WIDTH)
                        .max(BADGE_MIN_WIDTH);

                    let (rect, resp) = ui.allocate_exact_size(
                        egui::vec2(badge_w, BADGE_HEIGHT),
                        Sense::click(),
                    );

                    if ui.is_rect_visible(rect) {
                        let p = ui.painter();

                        // Tinted pill background + left accent bar + label text
                        paint_badge(p, rect, &text, accent, FontId::monospace(font_xs()), t as &dyn crate::ui_kit::widgets::theme::ComponentTheme);

                        // Dismiss ×
                        p.text(
                            egui::pos2(rect.right() - BADGE_DISMISS_PADDING, rect.center().y),
                            egui::Align2::CENTER_CENTER,
                            "×",
                            FontId::proportional(font_sm()),
                            tint(t, Tone::Dim, ALPHA_INTERACTIVE),
                        );
                    }

                    if resp.clicked() { to_dismiss = Some(alert.id); }
                    if resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
                }
            });
        });

    if let Some(id) = to_dismiss { dismiss(id); }
}
