//! Alert badge feed — trading-event badges (order fills, price alerts, signals,
//! errors) rendered as a horizontal scrolling pill-strip in the toolnav.
//!
//! Push alerts from anywhere via [`push`]. Click a badge to dismiss it.
//! Eventually this replaces the toast system for all trading-related events.

use std::collections::VecDeque;
use std::sync::{Mutex, OnceLock};
use std::sync::atomic::{AtomicU64, Ordering};
use egui::{Color32, FontId, Sense};

type Theme = crate::chart_renderer::gpu::Theme;

static NEXT_ID: AtomicU64 = AtomicU64::new(1);
static ALERTS: OnceLock<Mutex<VecDeque<AlertItem>>> = OnceLock::new();

fn store() -> &'static Mutex<VecDeque<AlertItem>> {
    ALERTS.get_or_init(|| Mutex::new(VecDeque::new()))
}

const MAX_ALERTS: usize = 50;

/// Severity / source of the alert — controls badge colour.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AlertKind {
    /// Order fully filled.
    OrderFilled,
    /// Order rejected or cancelled.
    OrderRejected,
    /// Order submitted, awaiting fill.
    OrderPending,
    /// A price-level alert triggered.
    PriceAlert,
    /// A signal fired on a chart.
    Signal,
    /// Hard error (connection loss, broker rejection, etc.).
    Error,
    /// Soft warning.
    Warning,
}

#[derive(Debug, Clone)]
pub struct AlertItem {
    pub id: u64,
    pub kind: AlertKind,
    /// Optional ticker the alert relates to.
    pub symbol: Option<String>,
    /// Short human-readable description.
    pub message: String,
}

/// Push a new alert onto the feed. Oldest entry is dropped when the cap is reached.
pub fn push(kind: AlertKind, symbol: Option<String>, message: impl Into<String>) {
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let mut q = store().lock().unwrap();
    if q.len() >= MAX_ALERTS { q.pop_front(); }
    q.push_back(AlertItem { id, kind, symbol, message: message.into() });
}

/// Dismiss (remove) a single alert by id.
pub fn dismiss(id: u64) {
    store().lock().unwrap().retain(|a| a.id != id);
}

/// Remove all alerts from the feed.
pub fn clear_all() {
    store().lock().unwrap().clear();
}

fn kind_color(kind: AlertKind, t: &Theme) -> Color32 {
    match kind {
        AlertKind::OrderFilled                       => t.bull,
        AlertKind::OrderRejected | AlertKind::Error  => t.bear,
        _                                            => t.accent,
    }
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
        color_alpha,
        BADGE_HEIGHT, BADGE_MIN_WIDTH, BADGE_ACCENT_WIDTH,
        BADGE_DISMISS_WIDTH, BADGE_DISMISS_PADDING,
        BADGE_CORNER_RADIUS, BADGE_TINT_ALPHA,
        ALPHA_SECONDARY_TEXT, ALPHA_INTERACTIVE,
    };

    let alerts: Vec<AlertItem> = store().lock().unwrap().iter().cloned().collect();

    if alerts.is_empty() {
        ui.label(
            egui::RichText::new("No alerts")
                .size(font_xs())
                .color(color_alpha(t.dim, ALPHA_SECONDARY_TEXT)),
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
                        let cr = egui::CornerRadius::same(BADGE_CORNER_RADIUS);

                        // Tinted pill background
                        p.rect_filled(rect, cr, color_alpha(accent, BADGE_TINT_ALPHA));
                        // Left accent bar
                        p.rect_filled(
                            egui::Rect::from_min_size(rect.min, egui::vec2(BADGE_ACCENT_WIDTH, rect.height())),
                            cr,
                            accent,
                        );
                        // Badge text
                        p.text(
                            egui::pos2(rect.left() + BADGE_ACCENT_WIDTH + gap_xs(), rect.center().y),
                            egui::Align2::LEFT_CENTER,
                            &text,
                            FontId::monospace(font_xs()),
                            t.text,
                        );
                        // Dismiss ×
                        p.text(
                            egui::pos2(rect.right() - BADGE_DISMISS_PADDING, rect.center().y),
                            egui::Align2::CENTER_CENTER,
                            "×",
                            FontId::proportional(font_sm()),
                            color_alpha(t.dim, ALPHA_INTERACTIVE),
                        );
                    }

                    if resp.clicked() { to_dismiss = Some(alert.id); }
                    if resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
                }
            });
        });

    if let Some(id) = to_dismiss { dismiss(id); }
}
