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

/// Smooth ease-out (0..1) for the appear / grow-in animation.
fn ease_out(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    1.0 - (1.0 - x) * (1.0 - x)
}

/// Split a one-line `summary` into the VITAL leading clause (always shown in the
/// compact badge) and a flag for whether extra detail follows (revealed when the
/// badge expands on rollover). Vital = text before the first strong delimiter,
/// else a hard character cap. Keeps the resting ticker terse.
fn vital_part(summary: &str) -> (String, bool) {
    const VITAL_MAX: usize = 16;
    for delim in [" — ", " – ", " - ", " · ", ": ", ". ", ", "] {
        if let Some(idx) = summary.find(delim) {
            let head = summary[..idx].trim();
            if !head.is_empty() && head.chars().count() <= 26 {
                return (head.to_string(), true);
            }
        }
    }
    if summary.chars().count() > VITAL_MAX {
        (summary.chars().take(VITAL_MAX).collect(), true)
    } else {
        (summary.to_string(), false)
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
        push(AlertKind::PriceAlert,    Some("NVDA".into()), "crossed above 920.00 resistance — volume surge 3.2x, breaking the 20-day range high");
    });
}

/// Render the alert ticker: up to a few newest dismissible badges that grow in
/// on arrival; everything beyond what fits rolls up into a notification bell
/// with a count + history popover. Hovering a badge pops a floating toast card
/// (above everything, frozen in place) with the full content. Clicking a badge
/// dismisses it; clicking the bell opens the history.
pub fn render_badge_feed(ui: &mut egui::Ui, t: &Theme) {
    seed_placeholders();
    use std::collections::{HashMap, HashSet};
    use egui::{Id, Rect, Align2, CornerRadius, pos2, vec2};
    use crate::chart_renderer::ui::style::{
        font_sm, font_md, gap_sm, gap_xs, color_alpha, contrast_fg,
        ALPHA_SECONDARY_TEXT, ALPHA_INTERACTIVE,
    };
    use crate::ui_kit::widgets::Tooltip;
    use crate::ui_kit::widgets::placement::{Placement, Side};
    use crate::ui_kit::icons::Icon;

    // ── Sizing / timing ──
    const BADGE_H:     f32   = 26.0;
    const ACCENT_W:    f32   = 3.0;
    const PAD_L:       f32   = 8.0;
    const PAD_R:       f32   = 6.0;
    const DISMISS_W:   f32   = 14.0;
    const MAX_INLINE:  usize = 4;       // newest N shown inline; rest live in the bell
    const MAX_BADGE_W: f32   = 240.0;   // a single badge never dominates the strip
    const FULL_CAP:    usize = 140;     // ceiling on the message shown in the toast
    const PILL_TINT_A: u8    = 18;
    const MSG_A:       u8    = 220;
    const APPEAR_DUR:  f64   = 0.22;
    const BELL_W:      f32   = 30.0;

    let alerts: Vec<AlertItem> = notification::badges_snapshot();

    if alerts.is_empty() {
        ui.label(
            egui::RichText::new("No alerts")
                .size(font_sm())
                .color(tint(t, Tone::Dim, ALPHA_SECONDARY_TEXT)),
        );
        return;
    }

    // Newest first.
    let items: Vec<&AlertItem> = alerts.iter().rev().collect();
    let font = FontId::monospace(font_sm());
    let gapx = gap_sm();
    let now  = ui.ctx().input(|i| i.time);

    let spawn_id = Id::new("alert_feed_spawn");
    let bell_id  = Id::new("alert_feed_bell_open");
    let mut spawn: HashMap<u64, f64> = ui.memory(|m| m.data.get_temp(spawn_id).unwrap_or_default());
    let mut bell_open: bool = ui.memory(|m| m.data.get_temp(bell_id).unwrap_or(false));

    // ── Fit: how many of the newest MAX_INLINE badges fit, leaving room for the
    //    bell? The remainder roll into the bell's count + history. Widths are
    //    measured inline (no closure) so `ui` isn't borrowed across the layout. ──
    let feed_w = ui.available_width().max(0.0);
    let cand = items.len().min(MAX_INLINE);
    let bell_reserve = BELL_W + gapx;
    let mut fit_n = 0usize;
    let mut used = 0.0_f32;
    for &a in items.iter().take(cand) {
        let accent  = kind_color(a.kind, t);
        let tag     = kind_tag(a.kind);
        let sym     = a.symbol.as_deref().filter(|s| !s.is_empty());
        let summary = summarize(&a.message);
        let (vital, more) = vital_part(&summary);
        let type_w  = text_w(ui, tag, &font, accent);
        let sym_w   = sym.map(|s| text_w(ui, s, &font, t.text)).unwrap_or(0.0);
        let vital_w = text_w(ui, &vital, &font, t.text);
        let ell_w   = if more { text_w(ui, "…", &font, t.text) + 4.0 } else { 0.0 };
        let sym_blk = if sym.is_some() { gapx + sym_w } else { 0.0 };
        let w = (ACCENT_W + PAD_L + type_w + sym_blk + gapx + vital_w + ell_w + gapx + DISMISS_W + PAD_R).min(MAX_BADGE_W);
        let add = if fit_n == 0 { w } else { gapx + w };
        if used + add > feed_w - bell_reserve { break; }
        used += add;
        fit_n += 1;
    }
    if fit_n == 0 && cand > 0 { fit_n = 1; } // always show at least the newest
    let overflow = items.len() - fit_n;

    let mut to_dismiss: Option<u64> = None;
    let mut clear_all_clicked = false;
    let mut bell_rect = Rect::NOTHING;

    ui.horizontal_centered(|ui| {
        ui.spacing_mut().item_spacing.x = gapx;
        let mut animating = false;

        for &a in items.iter().take(fit_n) {
            let id      = a.id;
            let accent  = kind_color(a.kind, t);
            let tag     = kind_tag(a.kind);
            let sym     = a.symbol.as_deref().filter(|s| !s.is_empty());
            let summary = summarize(&a.message);
            let (vital, more) = vital_part(&summary);
            let full    = truncate_ellipsis(&summary, FULL_CAP);

            // Appear: grow-in + fade.
            let spawn_t = *spawn.entry(id).or_insert(now);
            let appear  = (((now - spawn_t) / APPEAR_DUR) as f32).clamp(0.0, 1.0);
            let app_e   = ease_out(appear);
            if appear < 1.0 { animating = true; }

            let type_w  = text_w(ui, tag, &font, accent);
            let sym_w   = sym.map(|s| text_w(ui, s, &font, t.text)).unwrap_or(0.0);
            let vital_w = text_w(ui, &vital, &font, t.text);
            let ell_w   = if more { text_w(ui, "…", &font, t.text) + 4.0 } else { 0.0 };
            let sym_blk = if sym.is_some() { gapx + sym_w } else { 0.0 };
            let compact_w = (ACCENT_W + PAD_L + type_w + sym_blk + gapx + vital_w + ell_w + gapx + DISMISS_W + PAD_R).min(MAX_BADGE_W);
            let draw_w = (compact_w * app_e).max(2.0);
            let (rect, resp) = ui.allocate_exact_size(vec2(draw_w, BADGE_H), Sense::click());

            if ui.is_rect_visible(rect) {
                let p  = ui.painter_at(rect);
                let r  = (BADGE_H * 0.5) as u8;
                let cy = rect.center().y;
                p.rect_filled(rect, CornerRadius::same(r), color_alpha(accent, PILL_TINT_A).gamma_multiply(app_e));
                let bar = Rect::from_min_size(rect.min, vec2(ACCENT_W, rect.height()));
                p.rect_filled(bar, CornerRadius { nw: r, sw: r, ne: 0, se: 0 }, accent.gamma_multiply(app_e));

                let cx0 = rect.left() + ACCENT_W + PAD_L;
                p.text(pos2(cx0, cy), Align2::LEFT_CENTER, tag, font.clone(), accent.gamma_multiply(app_e));
                let mut x = cx0 + type_w + gapx;
                if let Some(s) = sym {
                    p.text(pos2(x, cy), Align2::LEFT_CENTER, s, font.clone(), t.text.gamma_multiply(app_e));
                    x += sym_w + gapx;
                }
                let tail = gapx + DISMISS_W + PAD_R;
                let clip_right = rect.right() - tail - ell_w;
                if clip_right > x {
                    let pm = ui.painter_at(Rect::from_min_max(pos2(x, rect.top()), pos2(clip_right, rect.bottom())));
                    pm.text(pos2(x, cy), Align2::LEFT_CENTER, &full, font.clone(), tint(t, Tone::Dim, MSG_A).gamma_multiply(app_e));
                }
                if more {
                    p.text(pos2(rect.right() - tail, cy), Align2::RIGHT_CENTER, "…", font.clone(),
                        tint(t, Tone::Dim, ALPHA_INTERACTIVE).gamma_multiply(app_e));
                }
                p.text(pos2(rect.right() - PAD_R - DISMISS_W * 0.5, cy), Align2::CENTER_CENTER, "×",
                    FontId::proportional(font_sm() + 1.0), tint(t, Tone::Dim, ALPHA_INTERACTIVE).gamma_multiply(app_e));
            }

            if resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }

            // Floating toast card on hover — floats above everything (Tooltip
            // layer), frozen in place, never pushed by incoming alerts. Grows as
            // a 2D box (message wraps) rather than only horizontally.
            let tag2 = tag;
            let accent2 = accent;
            let txt = t.text;
            let sym_owned = sym.map(|s| s.to_string());
            let full_owned = summary.clone();
            Tooltip::rich(move |ui, _th| {
                ui.set_max_width(300.0);
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(tag2).monospace().size(font_sm()).strong().color(accent2));
                    if let Some(s) = &sym_owned {
                        ui.label(egui::RichText::new(s).monospace().size(font_sm()).strong().color(txt));
                    }
                });
                ui.add_space(gap_xs());
                ui.label(egui::RichText::new(&full_owned).monospace().size(font_sm()).color(txt));
            })
            .delay_ms(120)
            .placement(Placement { side: Side::Bottom, ..Default::default() })
            .show(ui, &resp, t as &dyn crate::ui_kit::widgets::theme::ComponentTheme);

            if resp.clicked() { to_dismiss = Some(id); }
        }

        // ── Notification bell + count + history toggle ──
        {
            let (rect, resp) = ui.allocate_exact_size(vec2(BELL_W, BADGE_H), Sense::click());
            bell_rect = rect;
            if ui.is_rect_visible(rect) {
                let p = ui.painter();
                let glyph_col = if bell_open || overflow > 0 { t.accent } else { tint(t, Tone::Dim, ALPHA_INTERACTIVE) };
                p.text(rect.center(), Align2::CENTER_CENTER, Icon::BELL, FontId::proportional(font_md()), glyph_col);
                if overflow > 0 {
                    let lbl = if overflow > 99 { "99+".to_string() } else { overflow.to_string() };
                    let cf  = FontId::proportional(font_sm() - 1.0);
                    let cw  = (text_w(ui, &lbl, &cf, t.text) + 6.0).max(13.0);
                    let cnt = Rect::from_min_size(pos2(rect.center().x + 2.0, rect.top()), vec2(cw, 13.0));
                    p.rect_filled(cnt, CornerRadius::same(6), t.accent);
                    p.text(cnt.center(), Align2::CENTER_CENTER, &lbl, cf, contrast_fg(t.accent));
                }
            }
            if resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
            let resp = resp.on_hover_text(if overflow > 0 {
                format!("{overflow} more — click for history")
            } else {
                "Notification history".to_string()
            });
            if resp.clicked() { bell_open = !bell_open; }
        }

        if animating { ui.ctx().request_repaint(); }
    });

    // ── History popover (all alerts, newest-first) ──
    if bell_open {
        let ctx = ui.ctx().clone();
        let screen = ctx.screen_rect();
        const POP_W: f32 = 340.0;
        let left = (bell_rect.right() - POP_W).clamp(8.0, (screen.right() - POP_W - 8.0).max(8.0));
        let top  = bell_rect.bottom() + 6.0;

        let frame = egui::Frame::popup(&ctx.style())
            .fill(tint(t, Tone::Surface, 255))
            .stroke(egui::Stroke::new(1.0, tint(t, Tone::Border, 150)))
            .corner_radius(egui::CornerRadius::same(8))
            .inner_margin(egui::Margin::same(gap_sm() as i8));

        let area = egui::Area::new(Id::new("alert_history_pop"))
            .order(egui::Order::Foreground)
            .fixed_pos(pos2(left, top))
            .show(&ctx, |ui| {
                frame.show(ui, |ui| {
                    ui.set_width(POP_W - 2.0 * gap_sm());
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("NOTIFICATIONS").monospace().size(font_sm()).strong().color(t.accent));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.add(egui::Button::new(egui::RichText::new("Clear all").size(font_sm()).color(t.dim))
                                .fill(egui::Color32::TRANSPARENT)).clicked() {
                                clear_all_clicked = true;
                            }
                        });
                    });
                    ui.add_space(gap_xs());
                    egui::ScrollArea::vertical().max_height(360.0).show(ui, |ui| {
                        for &a in &items {
                            let accent = kind_color(a.kind, t);
                            let tag    = kind_tag(a.kind);
                            let msg    = summarize(&a.message);
                            ui.horizontal_wrapped(|ui| {
                                ui.spacing_mut().item_spacing.x = gap_xs();
                                ui.label(egui::RichText::new(tag).monospace().size(font_sm()).strong().color(accent));
                                if let Some(s) = a.symbol.as_deref().filter(|s| !s.is_empty()) {
                                    ui.label(egui::RichText::new(s).monospace().size(font_sm()).color(t.text));
                                }
                                ui.label(egui::RichText::new(&msg).monospace().size(font_sm()).color(tint(t, Tone::Dim, MSG_A)));
                            });
                            ui.add_space(2.0);
                        }
                    });
                });
            });

        // Dismiss on a press outside the popover and the bell.
        let pop_rect = area.response.rect;
        if ctx.input(|i| i.pointer.any_pressed()) {
            if let Some(p) = ctx.input(|i| i.pointer.interact_pos()) {
                if !pop_rect.contains(p) && !bell_rect.contains(p) { bell_open = false; }
            }
        }
    }

    // ── Persist memory + apply actions ──
    let live: HashSet<u64> = items.iter().map(|a| a.id).collect();
    spawn.retain(|k, _| live.contains(k));
    ui.memory_mut(|m| m.data.insert_temp(spawn_id, spawn));

    if clear_all_clicked { clear_all(); bell_open = false; }
    else if let Some(id) = to_dismiss { dismiss(id); }

    ui.memory_mut(|m| m.data.insert_temp(bell_id, bell_open));
}
