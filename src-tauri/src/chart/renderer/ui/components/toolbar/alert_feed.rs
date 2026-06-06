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

/// Collapse newlines and runs of whitespace into single spaces so the compact
/// ticker shows a one-line summary regardless of how the raw message is formatted.
fn summarize(msg: &str) -> String {
    msg.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Truncate to at most `max` characters (char-boundary safe), appending an
/// ellipsis when the text was cut. This is the per-badge length cap.
fn truncate_ellipsis(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}

/// Measure the rendered width of `s` in `font` (layout-only; width read only).
fn text_w(ui: &egui::Ui, s: &str, font: &FontId, col: Color32) -> f32 {
    ui.fonts(|f| f.layout_no_wrap(s.to_string(), font.clone(), col).rect.width())
}

/// Seed two placeholder alerts so the feed is non-empty on first render.
/// Remove this once real alerts are wired in from the broker / signal pipeline.
fn seed_placeholders() {
    use std::sync::Once;
    static SEEDED: Once = Once::new();
    SEEDED.call_once(|| {
        push(AlertKind::OrderFilled,   Some("AAPL".into()), "100 @ 213.45");
        push(AlertKind::Signal,        Some("SPY".into()),  "Bull cross 5m EMA 9/21");
        push(AlertKind::PriceAlert,    Some("NVDA".into()), "crossed above 920.00 resistance — volume surge 3.2x, breaking the 20-day range high");
    });
}

/// Render all pending alerts as horizontal dismissible badges.
///
/// Call from within a left-to-right horizontal layout; consumes all remaining
/// available width via an inner `ScrollArea`. Clicking a badge dismisses it.
pub fn render_badge_feed(ui: &mut egui::Ui, t: &Theme) {
    seed_placeholders();
    use crate::chart_renderer::ui::style::{
        font_sm, gap_sm, color_alpha,
        ALPHA_SECONDARY_TEXT, ALPHA_INTERACTIVE,
    };

    // ── Ticker sizing — taller + larger font than the old 20px/xs strip. ──
    const BADGE_H:        f32   = 26.0;
    const ACCENT_W:       f32   = 3.0;   // left severity bar
    const PAD_L:          f32   = 8.0;   // pad between accent bar and type label
    const PAD_R:          f32   = 6.0;   // pad right of the dismiss glyph
    const DISMISS_W:      f32   = 14.0;
    const MAX_MSG_CHARS:  usize = 38;    // per-badge message cap
    const PILL_TINT_A:    u8    = 18;    // background fill alpha
    const MSG_A:          u8    = 220;   // message text dim alpha

    let alerts: Vec<AlertItem> = notification::badges_snapshot();

    if alerts.is_empty() {
        ui.label(
            egui::RichText::new("No alerts")
                .size(font_sm())
                .color(tint(t, Tone::Dim, ALPHA_SECONDARY_TEXT)),
        );
        return;
    }

    let font = FontId::monospace(font_sm());

    // ── Mouse-wheel horizontal scroll ──
    // A horizontal `ScrollArea` only consumes horizontal wheel delta; translate
    // the (far more common) vertical wheel into horizontal scroll while the
    // pointer is over the feed, so the strip scrolls with a normal wheel.
    let feed_rect = ui.available_rect_before_wrap();
    let hovering_feed = ui
        .ctx()
        .input(|i| i.pointer.hover_pos().is_some_and(|p| feed_rect.contains(p)));
    if hovering_feed {
        ui.ctx().input_mut(|i| {
            let dy = i.smooth_scroll_delta.y;
            if dy != 0.0 { i.smooth_scroll_delta.x += dy; i.smooth_scroll_delta.y = 0.0; }
            let ry = i.raw_scroll_delta.y;
            if ry != 0.0 { i.raw_scroll_delta.x += ry; i.raw_scroll_delta.y = 0.0; }
        });
    }

    let mut to_dismiss: Option<u64> = None;

    egui::ScrollArea::horizontal()
        .id_source("alert_feed_scroll")
        .show(ui, |ui| {
            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing.x = gap_sm();

                for alert in &alerts {
                    let accent     = kind_color(alert.kind, t);
                    let type_label = kind_tag(alert.kind);
                    let sym        = alert.symbol.as_deref().filter(|s| !s.is_empty());
                    let summary    = summarize(&alert.message);
                    let msg        = truncate_ellipsis(&summary, MAX_MSG_CHARS);

                    // ── Measure each piece up front (before any painter borrow). ──
                    let type_w = text_w(ui, type_label, &font, accent);
                    let sym_tw = sym.map(|s| text_w(ui, s, &font, t.text));
                    let msg_tw = if msg.is_empty() { 0.0 } else { text_w(ui, &msg, &font, t.text) };

                    let sym_block = sym_tw.map(|w| w + gap_sm()).unwrap_or(0.0);
                    let msg_block = if msg.is_empty() { 0.0 } else { msg_tw + gap_sm() };
                    let badge_w = ACCENT_W + PAD_L + type_w + sym_block + msg_block
                        + gap_sm() + DISMISS_W + PAD_R;

                    let (rect, resp) =
                        ui.allocate_exact_size(egui::vec2(badge_w, BADGE_H), Sense::click());

                    if ui.is_rect_visible(rect) {
                        let p = ui.painter();
                        let r = (rect.height() * 0.5) as u8;

                        // Soft tinted pill background.
                        p.rect_filled(rect, egui::CornerRadius::same(r), color_alpha(accent, PILL_TINT_A));
                        // Left severity accent bar (square inner corners).
                        let bar = egui::Rect::from_min_size(rect.min, egui::vec2(ACCENT_W, rect.height()));
                        p.rect_filled(bar, egui::CornerRadius { nw: r, sw: r, ne: 0, se: 0 }, accent);

                        let cy = rect.center().y;
                        let mut x = rect.left() + ACCENT_W + PAD_L;

                        // Type — colored, the "kind" split out from the message.
                        p.text(egui::pos2(x, cy), egui::Align2::LEFT_CENTER, type_label, font.clone(), accent);
                        x += type_w + gap_sm();

                        // Symbol — bright.
                        if let (Some(s), Some(w)) = (sym, sym_tw) {
                            p.text(egui::pos2(x, cy), egui::Align2::LEFT_CENTER, s, font.clone(), t.text);
                            x += w + gap_sm();
                        }

                        // Message — dim, summarized + capped.
                        if !msg.is_empty() {
                            p.text(egui::pos2(x, cy), egui::Align2::LEFT_CENTER, &msg, font.clone(), tint(t, Tone::Dim, MSG_A));
                        }

                        // Dismiss ×.
                        p.text(
                            egui::pos2(rect.right() - PAD_R - DISMISS_W * 0.5, cy),
                            egui::Align2::CENTER_CENTER,
                            "×",
                            FontId::proportional(font_sm() + 1.0),
                            tint(t, Tone::Dim, ALPHA_INTERACTIVE),
                        );
                    }

                    if resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }

                    // Rollover — show the full, untruncated content (type + symbol + raw message).
                    let resp = resp.on_hover_ui(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(type_label).monospace().size(font_sm()).strong().color(accent));
                            if let Some(s) = sym {
                                ui.label(egui::RichText::new(s).monospace().size(font_sm()).strong().color(t.text));
                            }
                        });
                        ui.label(egui::RichText::new(alert.message.as_str()).monospace().size(font_sm()).color(t.text));
                    });

                    if resp.clicked() { to_dismiss = Some(alert.id); }
                }
            });
        });

    if let Some(id) = to_dismiss { dismiss(id); }
}
