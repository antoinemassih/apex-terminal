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
use crate::chart_renderer::ui::foundation::text_style::TextStyle;
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

/// Glyph that identifies the alert kind in the compact ticker (replaces the
/// word to save space). Severity colour still distinguishes good/bad.
fn kind_icon(kind: AlertKind) -> &'static str {
    use crate::ui_kit::icons::Icon;
    match kind {
        AlertKind::OrderFilled   => Icon::CHECK_CIRCLE,
        AlertKind::OrderRejected => Icon::X_CIRCLE,
        AlertKind::OrderPending  => Icon::CLOCK,
        AlertKind::PriceAlert    => Icon::CURRENCY_DOLLAR,
        AlertKind::Signal        => Icon::PULSE,
        AlertKind::Error         => Icon::SHIELD_WARNING,
        AlertKind::Warning       => Icon::WARNING,
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

/// Cubic ease-in-out (0..1) — gentle accelerate/decelerate for the ticker slide.
fn ease_in_out(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    if x < 0.5 {
        4.0 * x * x * x
    } else {
        let f = -2.0 * x + 2.0;
        1.0 - f * f * f / 2.0
    }
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

/// Render the alert ticker — a *stable ticker-tape frame*: the left edge and the
/// right-pinned bell never move. Badges are right-aligned and painted at
/// animated positions, so new alerts slide in within the fixed frame instead of
/// shoving the bell/frame around. Hovering a badge morphs the pill in place into
/// a stable box (frozen — incoming alerts don't move it) showing the full
/// content. Clicking a badge dismisses it; clicking the bell opens history.
pub fn render_badge_feed(ui: &mut egui::Ui, t: &Theme) {
    // Hidden entirely when the user disables toolbar notifications.
    if !notification::routing().toolbar_enabled { return; }
    use std::collections::{HashMap, HashSet};
    use egui::{Id, Rect, Align2, CornerRadius, Stroke, StrokeKind, LayerId, pos2, vec2};
    use crate::chart_renderer::ui::style::{
        font_sm, gap_sm, gap_xs, color_alpha, contrast_fg,
        ALPHA_SECONDARY_TEXT, ALPHA_INTERACTIVE, r_pill,
    };
    use crate::ui_kit::icons::Icon;

    // ── Sizing / timing ──
    // AUDIT 2026-08 — the control ladder, not a literal. These pills are
    // interactive (click to expand, click to dismiss) and were a hardcoded
    // 26.0, which is under the 28px touch minimum the dev-inspector's design
    // audit enforces — it reported all ten of them every frame. `control_h_md`
    // is 28 and is the token every other interactive control in the chrome
    // resolves from, so the badges now scale with density like their
    // neighbours instead of staying pinned two pixels short.
    let badge_h: f32 = crate::ui_kit::style::control_h_md()
        // Floor AFTER density. `control_h_md()` is the ladder rung times the
        // effective density scale, so a compact style (Octave, 0.85x) turned 28
        // into 23.8 and put the badges back under the minimum — the same
        // failure the raw 26.0 had, arrived at from the other direction.
        // Density may compress a control; it may not compress it out of reach.
        .max(crate::ui_kit::style::MIN_TOUCH_TARGET_PX);
    const ACCENT_W:    f32   = 3.0;
    const PAD_L:       f32   = 8.0;
    const PAD_R:       f32   = 6.0;
    const DISMISS_W:   f32   = 14.0;
    const SLOT_W:      f32   = 150.0;   // fixed badge width — uniform ticker slots
    const ICON_W:      f32   = 16.0;    // kind-icon column width
    const AREA_SLOTS:  f32   = 4.5;     // visible window = 4½ badges
    const FULL_SLOTS:  usize = 4;       // fully-visible count (rest → bell)
    const RENDER_EXTRA:usize = 2;       // extra rendered so out-goers slide out
    const FULL_CAP:    usize = 220;     // ceiling on the expanded message
    const PILL_TINT_A: u8    = 18;
    const MSG_A:       u8    = 220;
    const APPEAR_DUR:  f64   = 0.30;    // entrance fade/settle
    const SLIDE_DUR:   f64   = 0.34;    // eased ticker-tape slide
    const EXPAND_DUR:  f32   = 0.16;    // pill → box morph
    const BELL_W:      f32   = 30.0;
    const BOX_W:       f32   = 320.0;
    const HEADER_H:    f32   = 16.0;
    const PAD_V:       f32   = 6.0;

    let alerts: Vec<AlertItem> = notification::badges_snapshot();
    if alerts.is_empty() {
        ui.label(
            TextStyle::BodySm.as_rich_cascading("No alerts", tint(t, Tone::Dim, ALPHA_SECONDARY_TEXT)),
        );
        return;
    }

    let items: Vec<&AlertItem> = alerts.iter().rev().collect(); // newest first
    let by_id: HashMap<u64, &AlertItem> = alerts.iter().map(|a| (a.id, a)).collect();
    let font = crate::ui_kit::style::mono_sm();
    let gapx = gap_sm();
    let ctx  = ui.ctx().clone();
    let now  = ctx.input(|i| i.time);
    let pointer = ctx.input(|i| i.pointer.hover_pos());

    // ── Memory ──
    let spawn_id    = Id::new("alert_feed_spawn");
    let bell_id     = Id::new("alert_feed_bell_open");
    let frozen_id   = Id::new("alert_feed_order");
    let active_id   = Id::new("alert_feed_active");      // currently-expanded badge id
    let boxrect_id  = Id::new("alert_feed_boxrect");     // last frame's expand-box rect
    let expstart_id = Id::new("alert_feed_expstart");    // expand morph start time
    let mut spawn: HashMap<u64, f64> = ui.memory(|m| m.data.get_temp(spawn_id).unwrap_or_default());
    let mut bell_open: bool          = ui.memory(|m| m.data.get_temp(bell_id).unwrap_or(false));
    let frozen_order: Vec<u64>       = ui.memory(|m| m.data.get_temp(frozen_id).unwrap_or_default());
    let active_prev: Option<u64>     = ui.memory(|m| m.data.get_temp(active_id).unwrap_or(None));
    let prev_box: Rect               = ui.memory(|m| m.data.get_temp(boxrect_id).unwrap_or(Rect::NOTHING));
    let exp_start_prev: f64          = ui.memory(|m| m.data.get_temp(expstart_id).unwrap_or(now));

    let slide_id = Id::new("alert_feed_slide");
    let mut slide_state: HashMap<u64, (f32, f32, f64)> =
        ui.memory(|m| m.data.get_temp(slide_id).unwrap_or_default());

    // ── Fixed frame + a fixed 4½-slot window pinned just left of the bell. ──
    let avail = ui.available_width().max(BELL_W + 8.0);
    let (frame_rect, _) = ui.allocate_exact_size(vec2(avail, badge_h), Sense::hover());
    let bell_rect = Rect::from_min_size(pos2(frame_rect.right() - BELL_W, frame_rect.top()), vec2(BELL_W, badge_h));
    let slot_pitch = SLOT_W + gapx;
    let area_right = bell_rect.left() - gapx;
    let area_w = (AREA_SLOTS * slot_pitch - gapx).min((avail - BELL_W - gapx).max(slot_pitch));
    let area_left = (area_right - area_w).max(frame_rect.left());
    let badge_clip = Rect::from_min_max(pos2(area_left, frame_rect.top() - crate::ui_kit::style::gap_2xs()), pos2(area_right, frame_rect.bottom() + crate::ui_kit::style::gap_2xs()));

    // Freeze the running order while a badge is expanded so nothing shifts.
    let freeze = active_prev.map_or(false, |id| by_id.contains_key(&id));

    let overflow = alerts.len().saturating_sub(FULL_SLOTS);

    // Render the newest few (full slots + a couple that slide out, clipped).
    let render_count = (FULL_SLOTS + RENDER_EXTRA).min(items.len());
    let order: Vec<u64> = if freeze {
        frozen_order.iter().copied().filter(|id| by_id.contains_key(id)).collect()
    } else {
        items.iter().take(render_count).map(|a| a.id).collect()
    };

    // Left-anchored fixed slots: order[0] (newest) lands in the fixed first slot.
    let mut target_left: HashMap<u64, f32> = HashMap::new();
    for (k, &id) in order.iter().enumerate() {
        target_left.insert(id, area_left + k as f32 * slot_pitch);
    }

    let mut to_dismiss: Option<u64> = None;
    let mut hovered_now: Option<u64> = None;
    let mut rects: HashMap<u64, Rect> = HashMap::new();
    let mut animating = false;
    // Whether the ticker was already populated last frame — gates the slide-in
    // entrance so the very first paint doesn't slide everything in at once.
    let had_state = !slide_state.is_empty();

    // ── Render badges at animated positions, clipped to the stable frame. ──
    for (k, &id) in order.iter().enumerate() {
        let a = match by_id.get(&id) { Some(a) => *a, None => continue };
        let w = SLOT_W;
        let tgt = target_left[&id];
        // Eased slide: when the slot changes, retarget from the current eased
        // position so motion stays continuous (no snapping / jumpiness). A brand
        // new newest badge slides in from one slot to the left, keeping perfect
        // spacing with the badge it's pushing right (no overlap glitch).
        let (from, start) = match slide_state.get(&id).copied() {
            Some((to_p, from_p, start_p)) if (to_p - tgt).abs() < 0.5 => (from_p, start_p),
            Some((to_p, from_p, start_p)) => {
                let tt = ease_in_out(((now - start_p) / SLIDE_DUR) as f32);
                (from_p + (to_p - from_p) * tt, now)
            }
            None => if had_state && k == 0 { (tgt - slot_pitch, now) } else { (tgt, now) },
        };
        let st = (((now - start) / SLIDE_DUR) as f32).clamp(0.0, 1.0);
        let cur_left = from + (tgt - from) * ease_in_out(st);
        slide_state.insert(id, (tgt, from, start));
        if st < 1.0 { animating = true; }

        let spawn_t = *spawn.entry(id).or_insert(now);
        let appear  = (((now - spawn_t) / APPEAR_DUR) as f32).clamp(0.0, 1.0);
        let app_e   = ease_out(appear);
        if appear < 1.0 { animating = true; }

        let rect = Rect::from_min_size(pos2(cur_left, frame_rect.top()), vec2(w, badge_h));
        rects.insert(id, rect);

        let resp = ui.interact(rect, Id::new(("alert_badge", id)), Sense::click());
        if resp.hovered() { hovered_now = Some(id); ctx.set_cursor_icon(egui::CursorIcon::PointingHand); }
        if resp.clicked() { to_dismiss = Some(id); }

        // Register with the dev inspector — the feed had no widget-tree presence
        // at all, so nothing could assert anything about it. Indexed by slot (not
        // alert id) so the ids are stable across runs; the badge that sits under
        // the edge fade is expected to report as clipped, which is the point.
        #[cfg(debug_assertions)]
        crate::dev_inspector::record(
            crate::dev_inspector::WidgetRecord::from_response(
                format!("alert_feed.badge_{k}"), "button", a.message.as_str(), &resp, ui,
            ).with_style("alert_feed").in_ticker(),
        );

        // Paint the pill (clipped to the frame so off-edge badges are cut, not moved).
        let accent  = kind_color(a.kind, t);
        let sym     = a.symbol.as_deref().filter(|s| !s.is_empty());
        let summary = summarize(&a.message);
        let (_, more) = vital_part(&summary);
        let full    = truncate_ellipsis(&summary, FULL_CAP);
        let p = ui.painter().with_clip_rect(badge_clip);
        let r  = r_pill().nw.min((badge_h * 0.5) as u8);
        let cy = rect.center().y;
        p.rect_filled(rect, CornerRadius::same(r), color_alpha(accent, PILL_TINT_A).gamma_multiply(app_e));
        let bar = Rect::from_min_size(rect.min, vec2(ACCENT_W, rect.height()));
        p.rect_filled(bar, CornerRadius { nw: r, sw: r, ne: 0, se: 0 }, accent.gamma_multiply(app_e));
        // Layout is SOLVED, painting is unchanged.
        //
        // This was a cursor walk: `cx0 = left + ACCENT_W + PAD_L`, paint the
        // icon, `x += ICON_W + gapx`, maybe paint the symbol, `x += text_w +
        // gapx`. Every `+=` is a place the optional symbol can desynchronise
        // the rest of the row, and the walk cannot answer "where does the
        // message start" without having already drawn everything before it.
        //
        // The tree states the shape and hands back rects. The painting below is
        // the SAME code against `solved.rect(..)` instead of a running `x` —
        // deliberately, because this badge carries a fade (`app_e`), a morph,
        // and a clip-rect invariant with its own bug history (see below). Only
        // the geometry moved.
        let solved = {
            use crate::ui_kit::cascade::element::El;
            El::row()
                .gap(gapx)
                .pad_x(0.0)
                .child(El::slot("accent", vec2(ACCENT_W, rect.height())))
                .child(El::slot("icon", vec2(ICON_W, rect.height())).margin_start(PAD_L))
                .child_if(sym.is_some(), El::slot(
                    "sym",
                    vec2(sym.map_or(0.0, |s| text_w(ui, s, &font, t.text)), rect.height()),
                ))
                .child(El::slot("msg", vec2(0.0, rect.height())).grow(1.0))
                .child(El::slot("dismiss", vec2(DISMISS_W, rect.height())).margin_start(PAD_R))
                .solve_in(ui, rect)
        };
        let icon_r = solved.rect("icon");
        p.text(icon_r.center(), Align2::CENTER_CENTER, kind_icon(a.kind),
            crate::ui_kit::style::prop_at(crate::ui_kit::style::font_md()), accent.gamma_multiply(app_e));
        let x = solved.rect("msg").left();
        if let Some(s) = sym {
            p.text(pos2(solved.rect("sym").left(), cy), Align2::LEFT_CENTER, s,
                font.clone(), t.text.gamma_multiply(app_e));
        }
        let tail = gapx + DISMISS_W + PAD_R;
        let ell_w = if more { text_w(ui, "…", &font, t.text) + 4.0 } else { 0.0 };
        let clip_right = rect.right() - tail - ell_w;
        if clip_right > x {
            // `with_clip_rect` REPLACES the clip rect, it does not intersect —
            // so building this painter from `ui.painter()` rather than from `p`
            // dropped `badge_clip` entirely, and the left bound `x` is the text
            // origin, which for a badge sliding off the strip's left edge sits
            // OUTSIDE the strip. The pill background is clipped (it is painted
            // through `p`), but the message text was not: an outbound badge left
            // its label floating on the bare toolbar, overlapping whatever was
            // to the left of the strip. It also escaped the edge-fade below,
            // which only covers `badge_clip`.
            //
            // Intersecting restores the invariant that nothing in this strip
            // paints outside it. Same idiom as `order_row.rs`.
            let text_clip = Rect::from_min_max(
                pos2(x, badge_clip.top()),
                pos2(clip_right.min(area_right), badge_clip.bottom()),
            )
            .intersect(badge_clip);
            let pm = ui.painter().with_clip_rect(text_clip);
            pm.text(pos2(x, cy), Align2::LEFT_CENTER, &full, font.clone(), tint(t, Tone::Dim, MSG_A).gamma_multiply(app_e));
        }
        if more {
            p.text(pos2(rect.right() - tail, cy), Align2::RIGHT_CENTER, "…", font.clone(),
                tint(t, Tone::Dim, ALPHA_INTERACTIVE).gamma_multiply(app_e));
        }
        p.text(pos2(rect.right() - PAD_R - DISMISS_W * 0.5, cy), Align2::CENTER_CENTER, "×",
            crate::ui_kit::style::prop_at(font_sm() + 1.0), tint(t, Tone::Dim, ALPHA_INTERACTIVE).gamma_multiply(app_e));
    }

    // Keep the expansion alive while the pointer is over the badge OR its box.
    let over_box = pointer.map_or(false, |p| prev_box.contains(p));
    let active_now: Option<u64> = hovered_now
        .or(if over_box { active_prev } else { None })
        .filter(|id| by_id.contains_key(id));
    // Morph timer: reset when the active badge changes (timestamp-driven so the
    // pill→box grow actually animates from 0 on first hover).
    let exp_start = if active_now.is_some() && active_now == active_prev { exp_start_prev } else { now };

    // ── Expand: morph the active pill into a stable box (in place, on top). ──
    let mut box_rect = Rect::NOTHING;
    if let (Some(id), Some(&pill)) = (active_now, active_now.and_then(|id| rects.get(&id))) {
        let a = by_id[&id];
        let t_exp = (((now - exp_start) / EXPAND_DUR as f64) as f32).clamp(0.0, 1.0);
        if t_exp < 1.0 { animating = true; }
        let app = ease_out(t_exp);

        let accent  = kind_color(a.kind, t);
        let tag     = kind_tag(a.kind);
        let sym     = a.symbol.as_deref().filter(|s| !s.is_empty());
        let summary = summarize(&a.message);
        let full    = truncate_ellipsis(&summary, FULL_CAP);

        let inner_w = BOX_W - ACCENT_W - PAD_L - PAD_R;
        let mcol = t.text.gamma_multiply(app);
        let galley = ui.fonts(|f| f.layout(full.clone(), font.clone(), mcol, inner_w));
        let target_h = (PAD_V + HEADER_H + gap_xs() + galley.size().y + PAD_V).max(badge_h);

        let box_w = pill.width() + (BOX_W - pill.width()) * app;
        let box_h = badge_h + (target_h - badge_h) * app;
        // Anchor at the pill's left and grow right + down (newest badges sit on
        // the left now). Clamp so the box never spills off the screen edge.
        let screen_r = ctx.screen_rect().right();
        let box_left = pill.left().min((screen_r - 8.0 - box_w).max(4.0)).max(4.0);
        box_rect = Rect::from_min_size(pos2(box_left, pill.top()), vec2(box_w, box_h));

        let fp = ctx.layer_painter(LayerId::new(egui::Order::Foreground, Id::new(("alert_box_layer", id))));
        let r = r_pill().nw.min((badge_h * 0.5) as u8);
        let cr = CornerRadius::same(r);
        // Soft shadow + solid card + border + accent bar (all fade in with the morph).
        fp.rect_filled(box_rect.translate(vec2(0.0, 2.0)).expand(1.0), cr,
            { let s = t.shadow_color; crate::ui_kit::style::color_alpha(s, (70.0 * app) as u8) });
        fp.rect_filled(box_rect, cr, tint(t, Tone::Surface, 252).gamma_multiply(app.max(0.001)));
        fp.rect_stroke(box_rect, cr, Stroke::new(crate::ui_kit::style::stroke_std(), tint(t, Tone::Border, crate::ui_kit::style::alpha_dense()).gamma_multiply(app)), StrokeKind::Inside);
        fp.rect_filled(Rect::from_min_size(box_rect.min, vec2(ACCENT_W, box_rect.height())),
            CornerRadius { nw: r, sw: r, ne: 0, se: 0 }, accent.gamma_multiply(app));

        // Header: icon + type word + symbol (room here, so keep the label).
        let hx = box_rect.left() + ACCENT_W + PAD_L;
        let hy = box_rect.top() + PAD_V + HEADER_H * 0.5;
        fp.text(pos2(hx + ICON_W * 0.5, hy), Align2::CENTER_CENTER, kind_icon(a.kind),
            crate::ui_kit::style::prop_at(crate::ui_kit::style::font_md()), accent.gamma_multiply(app));
        let mut hxx = hx + ICON_W + gapx;
        fp.text(pos2(hxx, hy), Align2::LEFT_CENTER, tag, font.clone(), accent.gamma_multiply(app));
        hxx += text_w(ui, tag, &font, accent) + gapx;
        if let Some(s) = sym {
            fp.text(pos2(hxx, hy), Align2::LEFT_CENTER, s, font.clone(), t.text.gamma_multiply(app));
            hxx += text_w(ui, s, &font, t.text) + gapx;
        }
        let _ = hxx;
        // Body message (wrapped), clipped to the animating box so it reveals as it grows.
        let body_clip = Rect::from_min_max(
            pos2(box_rect.left(), box_rect.top() + PAD_V + HEADER_H),
            pos2(box_rect.right() - PAD_R, box_rect.bottom()),
        );
        let bp = fp.with_clip_rect(body_clip);
        bp.galley(pos2(box_rect.left() + ACCENT_W + PAD_L, box_rect.top() + PAD_V + HEADER_H + gap_xs()), galley, mcol);
    }

    // ── Edge fade over the outgoing badge ────────────────────────────────────
    // The window is deliberately 4½ slots wide (AREA_SLOTS) so a half badge
    // signals "more is sliding in". But a bare clip rect guillotines that badge
    // mid-glyph and lops off its rounded corner, which reads as a rendering bug
    // rather than as a ticker tape. Dissolve the last stretch into the toolbar
    // instead: same information, no hard cut.
    //
    // Painted AFTER the badges (so it covers them) and BEFORE the bell (so the
    // bell stays crisp). A mesh gives a true gradient — stepped rects band
    // visibly against the flat toolbar fill on light styles like Aperture.
    {
        const FADE_W: f32 = 28.0;
        let x1 = area_right;
        let x0 = (x1 - FADE_W).max(area_left);
        if x1 > x0 {
            let (y0, y1) = (badge_clip.top(), badge_clip.bottom());
            let mut mesh = egui::Mesh::default();
            // Transparent on the inboard edge → solid toolbar fill at the clip.
            mesh.colored_vertex(pos2(x0, y0), egui::Color32::TRANSPARENT);
            mesh.colored_vertex(pos2(x0, y1), egui::Color32::TRANSPARENT);
            mesh.colored_vertex(pos2(x1, y0), t.toolbar_bg);
            mesh.colored_vertex(pos2(x1, y1), t.toolbar_bg);
            mesh.add_triangle(0, 1, 2);
            mesh.add_triangle(2, 1, 3);
            ui.painter().with_clip_rect(badge_clip).add(egui::Shape::mesh(mesh));
        }
    }

    // ── Notification bell (pinned, stable) + count + history toggle. ──
    {
        let resp = ui.interact(bell_rect, Id::new("alert_bell"), Sense::click());
        let p = ui.painter().with_clip_rect(frame_rect);
        let glyph_col = if bell_open || overflow > 0 { t.accent } else { tint(t, Tone::Dim, ALPHA_INTERACTIVE) };
        p.text(bell_rect.center(), Align2::CENTER_CENTER, Icon::BELL, crate::ui_kit::style::prop_at(crate::ui_kit::style::font_md()), glyph_col);
        if overflow > 0 {
            let lbl = if overflow > 99 { "99+".to_string() } else { overflow.to_string() };
            let cf  = crate::ui_kit::style::prop_at(font_sm() - 1.0);
            let cw  = (text_w(ui, &lbl, &cf, t.text) + 6.0).max(13.0);
            let cnt = Rect::from_min_size(pos2(bell_rect.center().x + 2.0, bell_rect.top()), vec2(cw, 13.0));
            p.rect_filled(cnt, crate::ui_kit::style::r_md_cr(), t.accent);
            p.text(cnt.center(), Align2::CENTER_CENTER, &lbl, cf, contrast_fg(t.accent));
        }
        if resp.hovered() { ctx.set_cursor_icon(egui::CursorIcon::PointingHand); }
        let resp = resp.on_hover_text(if overflow > 0 {
            format!("{overflow} more — click for history")
        } else {
            "Notification history".to_string()
        });
        if resp.clicked() { bell_open = !bell_open; }
    }

    if animating { ctx.request_repaint(); }

    // ── History popover (all alerts, newest-first). ──
    let mut clear_all_clicked = false;
    if bell_open {
        let screen = ctx.screen_rect();
        const POP_W: f32 = 340.0;
        let left = (bell_rect.right() - POP_W).clamp(8.0, (screen.right() - POP_W - 8.0).max(8.0));
        let top  = bell_rect.bottom() + crate::ui_kit::style::gap_xs_mid();
        let frame = egui::Frame::popup(&ctx.style())
            .fill(tint(t, Tone::Surface, 255))
            .stroke(Stroke::new(crate::ui_kit::style::stroke_std(), tint(t, Tone::Border, 150)))
            .corner_radius(crate::ui_kit::style::r_md_cr())
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
                            if ui.add(egui::Button::new(TextStyle::BodySm.as_rich_cascading("Clear all", t.dim))
                                .fill(egui::Color32::TRANSPARENT)).clicked() {
                                clear_all_clicked = true;
                            }
                        });
                    });
                    ui.add_space(crate::ui_kit::style::gap_xs());
                    egui::ScrollArea::vertical().max_height(360.0).show(ui, |ui| {
                        for &a in &items {
                            let accent = kind_color(a.kind, t);
                            let tag    = kind_tag(a.kind);
                            let msg    = summarize(&a.message);
                            ui.horizontal_wrapped(|ui| {
                                ui.spacing_mut().item_spacing.x = gap_xs();
                                ui.label(TextStyle::Body.as_rich_cascading(kind_icon(a.kind), accent));
                                ui.label(egui::RichText::new(tag).monospace().size(font_sm()).strong().color(accent));
                                if let Some(s) = a.symbol.as_deref().filter(|s| !s.is_empty()) {
                                    ui.label(egui::RichText::new(s).monospace().size(font_sm()).color(t.text));
                                }
                                ui.label(egui::RichText::new(&msg).monospace().size(font_sm()).color(tint(t, Tone::Dim, MSG_A)));
                            });
                            ui.add_space(crate::ui_kit::style::gap_2xs());
                        }
                    });
                });
            });
        let pop_rect = area.response.rect;
        if ctx.input(|i| i.pointer.any_pressed()) {
            if let Some(p) = ctx.input(|i| i.pointer.interact_pos()) {
                if !pop_rect.contains(p) && !bell_rect.contains(p) { bell_open = false; }
            }
        }
    }

    // ── Persist memory + apply actions. ──
    let live: HashSet<u64> = alerts.iter().map(|a| a.id).collect();
    spawn.retain(|k, _| live.contains(k));
    slide_state.retain(|k, _| live.contains(k));
    ui.memory_mut(|m| {
        m.data.insert_temp(spawn_id, spawn);
        m.data.insert_temp(slide_id, slide_state);
        m.data.insert_temp(frozen_id, order);
        m.data.insert_temp(active_id, active_now);
        m.data.insert_temp(boxrect_id, box_rect);
        m.data.insert_temp(expstart_id, exp_start);
    });

    if clear_all_clicked { clear_all(); bell_open = false; }
    else if let Some(id) = to_dismiss { dismiss(id); }

    ui.memory_mut(|m| m.data.insert_temp(bell_id, bell_open));
}
