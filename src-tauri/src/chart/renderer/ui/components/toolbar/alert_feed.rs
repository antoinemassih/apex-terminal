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

/// Render the alert ticker — newest-first dismissible badges that grow in on
/// arrival, expand in place on rollover (instead of a tooltip), and collapse
/// the overflow into a stacked "+N" count chip so nothing runs offscreen.
///
/// Call from within a left-to-right horizontal layout; occupies the remaining
/// width. Clicking a badge dismisses it; clicking the count chip expands all.
pub fn render_badge_feed(ui: &mut egui::Ui, t: &Theme) {
    seed_placeholders();
    use std::collections::{HashMap, HashSet};
    use egui::{Id, Rect, Align2, CornerRadius, pos2, vec2};
    use crate::chart_renderer::ui::style::{
        font_sm, gap_sm, color_alpha,
        ALPHA_SECONDARY_TEXT, ALPHA_INTERACTIVE,
    };

    // ── Sizing / timing ──
    const BADGE_H:     f32   = 26.0;
    const ACCENT_W:    f32   = 3.0;    // left severity bar
    const PAD_L:       f32   = 8.0;    // pad between accent bar and type label
    const PAD_R:       f32   = 6.0;    // pad right of the dismiss glyph
    const DISMISS_W:   f32   = 14.0;
    const FULL_CAP:    usize = 110;    // hard ceiling on the expanded message
    const PILL_TINT_A: u8    = 18;     // background fill alpha
    const MSG_A:       u8    = 220;    // message text dim alpha
    const APPEAR_DUR:  f64   = 0.22;   // grow-in seconds
    const EXPAND_DUR:  f32   = 0.13;   // hover expand seconds
    const STACK_OFF:   f32   = 8.0;    // peek width of the stacked count chip

    let alerts: Vec<AlertItem> = notification::badges_snapshot();

    if alerts.is_empty() {
        ui.label(
            egui::RichText::new("No alerts")
                .size(font_sm())
                .color(tint(t, Tone::Dim, ALPHA_SECONDARY_TEXT)),
        );
        return;
    }

    // Newest first — new alerts enter at the left and push older ones right.
    let items: Vec<&AlertItem> = alerts.iter().rev().collect();

    let font  = FontId::monospace(font_sm());
    let gapx  = gap_sm();
    let now   = ui.ctx().input(|i| i.time);
    let ell_w = text_w(ui, "…", &font, t.text);

    // ── Per-feed memory: appear-time map + overflow-expanded toggle. ──
    let spawn_id = Id::new("alert_feed_spawn");
    let exp_id   = Id::new("alert_feed_overflow_expanded");
    let mut spawn: HashMap<u64, f64> = ui.memory(|m| m.data.get_temp(spawn_id).unwrap_or_default());
    let mut overflow_expanded: bool  = ui.memory(|m| m.data.get_temp(exp_id).unwrap_or(false));

    // ── Measure every badge once (compact + expanded widths). ──
    struct M<'a> {
        a: &'a AlertItem, accent: Color32, tag: &'static str,
        type_w: f32, sym: Option<&'a str>, sym_w: f32,
        full: String, more: bool, compact_w: f32, expanded_w: f32,
    }
    let measured: Vec<M> = items.iter().map(|&a| {
        let accent  = kind_color(a.kind, t);
        let tag     = kind_tag(a.kind);
        let sym     = a.symbol.as_deref().filter(|s| !s.is_empty());
        let summary = summarize(&a.message);
        let (vital, more) = vital_part(&summary);
        let full    = truncate_ellipsis(&summary, FULL_CAP);
        let type_w  = text_w(ui, tag, &font, accent);
        let sym_w   = sym.map(|s| text_w(ui, s, &font, t.text)).unwrap_or(0.0);
        let vital_w = text_w(ui, &vital, &font, t.text);
        let full_w  = text_w(ui, &full,  &font, t.text);
        let sym_blk = if sym.is_some() { gapx + sym_w } else { 0.0 };
        let base    = ACCENT_W + PAD_L + type_w + sym_blk + gapx;
        let tail    = gapx + DISMISS_W + PAD_R;
        let more_w  = if more { ell_w + 4.0 } else { 0.0 };
        M {
            a, accent, tag, type_w, sym, sym_w, full, more,
            compact_w:  base + vital_w + more_w + tail,
            expanded_w: base + full_w + tail,
        }
    }).collect();

    // ── Fit: how many fit compactly before the count chip takes over? ──
    let feed_rect  = ui.available_rect_before_wrap();
    let viewport_w = feed_rect.width().max(0.0);
    // Clamp a single badge's resting width so one long alert can't eat the whole
    // (often narrow) feed — keeps room for siblings and the "+N" count chip.
    let max_compact = (viewport_w * 0.62).max(120.0);
    let compact_of = |m: &M| m.compact_w.min(max_compact);
    let total: f32 = measured.iter().map(|m| compact_of(m)).sum::<f32>()
        + gapx * measured.len().saturating_sub(1) as f32;
    let had_overflow = total > viewport_w;
    let visible_count = if overflow_expanded || !had_overflow {
        measured.len()
    } else {
        let reserve = STACK_OFF + 40.0 + gapx; // room for the "+N" chip
        let mut used = 0.0_f32;
        let mut k = 0usize;
        for m in &measured {
            let add = if k == 0 { compact_of(m) } else { gapx + compact_of(m) };
            if used + add > viewport_w - reserve { break; }
            used += add;
            k += 1;
        }
        k.max(1)
    };
    let overflow = measured.len().saturating_sub(visible_count);

    // ── Mouse-wheel horizontal scroll (vertical wheel → horizontal). ──
    let hovering_feed = ui.ctx()
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
    let mut toggle_overflow = false;

    egui::ScrollArea::horizontal()
        .id_source("alert_feed_scroll")
        .show(ui, |ui| {
            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing.x = gapx;
                let mut animating = false;

                for m in measured.iter().take(visible_count) {
                    let id = m.a.id;

                    // Appear: grow-in + fade over APPEAR_DUR from first-seen.
                    let spawn_t = *spawn.entry(id).or_insert(now);
                    let appear  = (((now - spawn_t) / APPEAR_DUR) as f32).clamp(0.0, 1.0);
                    let app_e   = ease_out(appear);
                    if appear < 1.0 { animating = true; }

                    // Hover-expand: drive width from last frame's hover (1-frame lag).
                    let hov_id = Id::new(("alert_badge_hov", id));
                    let was_hovered: bool = ui.memory(|mm| mm.data.get_temp(hov_id).unwrap_or(false));
                    let t_exp = ui.ctx().animate_bool_with_time(Id::new(("alert_badge_exp", id)), was_hovered, EXPAND_DUR);
                    if t_exp > 0.0 && t_exp < 1.0 { animating = true; }

                    // Resting width is clamped (see max_compact); expanded width
                    // is capped to the viewport so the in-place reveal never runs
                    // offscreen.
                    let compact_w = m.compact_w.min(max_compact);
                    let exp_w  = m.expanded_w.min((viewport_w - 4.0).max(compact_w));
                    let w      = compact_w + (exp_w - compact_w) * t_exp;
                    let draw_w = (w * app_e).max(2.0);
                    let (rect, resp) = ui.allocate_exact_size(vec2(draw_w, BADGE_H), Sense::click());
                    let hovered_now = resp.hovered();
                    ui.memory_mut(|mm| mm.data.insert_temp(hov_id, hovered_now));

                    if ui.is_rect_visible(rect) {
                        let p  = ui.painter_at(rect);
                        let r  = (BADGE_H * 0.5) as u8;
                        let cy = rect.center().y;

                        // Pill bg + left accent bar (alpha tracks appear).
                        p.rect_filled(rect, CornerRadius::same(r), color_alpha(m.accent, PILL_TINT_A).gamma_multiply(app_e));
                        let bar = Rect::from_min_size(rect.min, vec2(ACCENT_W, rect.height()));
                        p.rect_filled(bar, CornerRadius { nw: r, sw: r, ne: 0, se: 0 }, m.accent.gamma_multiply(app_e));

                        let cx0 = rect.left() + ACCENT_W + PAD_L;
                        // Type — severity colour.
                        p.text(pos2(cx0, cy), Align2::LEFT_CENTER, m.tag, font.clone(), m.accent.gamma_multiply(app_e));
                        let mut x = cx0 + m.type_w + gapx;
                        // Symbol — bright.
                        if let Some(s) = m.sym {
                            p.text(pos2(x, cy), Align2::LEFT_CENTER, s, font.clone(), t.text.gamma_multiply(app_e));
                            x += m.sym_w + gapx;
                        }
                        // Message — dim, clipped to the (animating) box so it reveals as it expands.
                        let tail = gapx + DISMISS_W + PAD_R;
                        let reserve_more = if m.more { (ell_w + 4.0) * (1.0 - t_exp) } else { 0.0 };
                        let clip_right = rect.right() - tail - reserve_more;
                        if clip_right > x {
                            let pm = ui.painter_at(Rect::from_min_max(pos2(x, rect.top()), pos2(clip_right, rect.bottom())));
                            pm.text(pos2(x, cy), Align2::LEFT_CENTER, &m.full, font.clone(), tint(t, Tone::Dim, MSG_A).gamma_multiply(app_e));
                        }
                        // "more" hint while compact.
                        if m.more && t_exp < 0.85 {
                            p.text(pos2(rect.right() - tail, cy), Align2::RIGHT_CENTER, "…", font.clone(),
                                tint(t, Tone::Dim, ALPHA_INTERACTIVE).gamma_multiply((1.0 - t_exp) * app_e));
                        }
                        // Dismiss ×.
                        p.text(pos2(rect.right() - PAD_R - DISMISS_W * 0.5, cy), Align2::CENTER_CENTER, "×",
                            FontId::proportional(font_sm() + 1.0), tint(t, Tone::Dim, ALPHA_INTERACTIVE).gamma_multiply(app_e));
                    }

                    if hovered_now { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
                    if resp.clicked() { to_dismiss = Some(id); }
                }

                // ── Overflow: stacked "+N" count chip (collapsed) / "‹" (expanded). ──
                if overflow > 0 && !overflow_expanded {
                    let label = format!("+{overflow}");
                    let lw = text_w(ui, &label, &font, t.text);
                    let chip_w = lw + 18.0 + STACK_OFF;
                    let (rect, resp) = ui.allocate_exact_size(vec2(chip_w, BADGE_H), Sense::click());
                    if ui.is_rect_visible(rect) {
                        let p = ui.painter();
                        let r = (BADGE_H * 0.5) as u8;
                        let cr = CornerRadius::same(r);
                        // Two cards peeking behind to imply a stack.
                        p.rect_filled(Rect::from_min_size(rect.min + vec2(STACK_OFF, 4.0), vec2(rect.width() - STACK_OFF, rect.height() - 8.0)), cr, tint(t, Tone::Dim, 34));
                        p.rect_filled(Rect::from_min_size(rect.min + vec2(STACK_OFF * 0.5, 2.0), vec2(rect.width() - STACK_OFF, rect.height() - 4.0)), cr, tint(t, Tone::Dim, 64));
                        let front = Rect::from_min_size(rect.min, vec2(rect.width() - STACK_OFF, rect.height()));
                        p.rect_filled(front, cr, color_alpha(t.text, 22));
                        p.text(front.center(), Align2::CENTER_CENTER, &label, font.clone(), t.text);
                    }
                    if resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
                    let resp = resp.on_hover_text(format!("{overflow} more — click to expand"));
                    if resp.clicked() { toggle_overflow = true; }
                } else if overflow_expanded && had_overflow {
                    let (rect, resp) = ui.allocate_exact_size(vec2(26.0, BADGE_H), Sense::click());
                    if ui.is_rect_visible(rect) {
                        let p = ui.painter();
                        let r = (BADGE_H * 0.5) as u8;
                        p.rect_filled(rect, CornerRadius::same(r), color_alpha(t.text, 18));
                        p.text(rect.center(), Align2::CENTER_CENTER, "‹", FontId::proportional(font_sm() + 2.0), t.text);
                    }
                    if resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
                    let resp = resp.on_hover_text("Collapse");
                    if resp.clicked() { toggle_overflow = true; }
                }

                if animating { ui.ctx().request_repaint(); }
            });
        });

    // ── Persist memory (prune appear-times to live ids) + apply actions. ──
    let live: HashSet<u64> = items.iter().map(|a| a.id).collect();
    spawn.retain(|k, _| live.contains(k));
    ui.memory_mut(|m| m.data.insert_temp(spawn_id, spawn));
    if toggle_overflow { overflow_expanded = !overflow_expanded; }
    ui.memory_mut(|m| m.data.insert_temp(exp_id, overflow_expanded));

    if let Some(id) = to_dismiss { dismiss(id); }
}
