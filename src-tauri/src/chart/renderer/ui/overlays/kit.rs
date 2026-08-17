//! Overlay Kit — the shared, Sx-styled drawing primitives that on-chart overlay
//! widgets compose their bodies from, instead of each one hand-painting.
//!
//! Before this module these helpers were scattered through the 3,500-line
//! `chart_widgets.rs` and used inconsistently (16 `hero_number` calls, 17
//! `sub_label`, …). Centralising them here is step 1 of the overlay system:
//! a widget body should read as a few kit calls (`radial_gauge(...)`,
//! `progress_bar(...)`), with colours resolved from Sx tones so overlays match
//! the active theme automatically.
//!
//! All primitives are **painter-based** — they draw directly into the chart
//! canvas `Painter` at an absolute rect/pos, matching the overlay render path.

use egui::{self, Color32, Stroke};
use crate::ui_kit::sx::Tone;
use crate::ui_kit::icons::Icon;
use crate::chart_renderer::ui::style::*;
use crate::chart_renderer::gpu::Theme;

/// Height of the floating overlay header bar (above the card). Shared by
/// [`overlay_card_header`] and [`overlay_header_ctx_rect`] so the popup anchor
/// and the painted button can't drift apart.
pub(crate) const OVERLAY_HEADER_H: f32 = 26.0;

// ── Geometry ──────────────────────────────────────────────────────────────────

/// Stroke an arc from `start`→`end` radians around `center` (y-up convention).
pub(crate) fn draw_arc(
    p: &egui::Painter, center: egui::Pos2, radius: f32, start: f32, end: f32,
    stroke: Stroke, segments: usize,
) {
    if segments < 2 { return; }
    let step = (end - start) / segments as f32;
    let points: Vec<egui::Pos2> = (0..=segments)
        .map(|i| {
            let a = start + step * i as f32;
            egui::pos2(center.x + radius * a.cos(), center.y - radius * a.sin())
        })
        .collect();
    for pair in points.windows(2) {
        p.line_segment([pair[0], pair[1]], stroke);
    }
}

/// Linear RGB lerp `a`→`b` by `t` (0..1), forced OPAQUE. Used for value-driven
/// colour ramps, whose endpoints are `tint(...)` colours and whose fills must
/// land solid.
///
/// Shares `motion::lerp_channels`; only the alpha rule is local. It used to
/// carry its own channel loop, which TRUNCATED where the other three rounded —
/// so a ramp here could sit one 1/255 step below the same ramp drawn by a
/// widget. Rounding now, which is the change; it is imperceptible and it is
/// consistent.
///
/// The `#[allow(dead_code)]` this carried ("callers migrate off chart_widgets'
/// local copy") suppressed nothing: it has three callers in `viz/charts.rs`,
/// and `chart_widgets` had already migrated — to `motion`, not to here.
pub(crate) fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    crate::ui_kit::widgets::motion::lerp_color(a, b, t).to_opaque()
}

// ── Text ──────────────────────────────────────────────────────────────────────

/// Hero number — the focal large proportional value at the centre of a widget.
pub(crate) fn hero_number(p: &egui::Painter, pos: egui::Pos2, text: &str, color: Color32) {
    p.text(pos, egui::Align2::CENTER_CENTER, text,
        crate::ui_kit::style::prop_at(font_display_md()), color);
}

/// Even larger hero for primary KPIs.
#[allow(dead_code)] // kit primitive — for the KPI-style overlays migrating next
pub(crate) fn hero_number_lg(p: &egui::Painter, pos: egui::Pos2, text: &str, color: Color32) {
    p.text(pos, egui::Align2::CENTER_CENTER, text,
        crate::ui_kit::style::prop_at(font_display_lg()), color);
}

/// Small uppercase mono label — editorial caption under a hero value.
pub(crate) fn sub_label(p: &egui::Painter, pos: egui::Pos2, text: &str, color: Color32) {
    p.text(pos, egui::Align2::CENTER_CENTER, text, crate::ui_kit::style::mono_at(FONT_XS),
        crate::ui_kit::style::color_alpha(color, 170));
}

// ── Composite primitives (compose the above — what bodies should call) ────────

/// Donut ring gauge — thick value arc over a full-circle track.
pub(crate) fn donut_ring(
    p: &egui::Painter, center: egui::Pos2, radius: f32, thickness: f32,
    value: f32, max: f32, color: Color32, track_color: Color32,
) {
    let segs = 48;
    let tau = std::f32::consts::TAU;
    draw_arc(p, center, radius, 0.0, tau, egui::Stroke::new(thickness, track_color), segs);
    let frac = (value / max).clamp(0.0, 1.0);
    let start = -std::f32::consts::FRAC_PI_2;
    for i in 0..segs {
        let t0 = i as f32 / segs as f32;
        if t0 >= frac { break; }
        let t1 = ((i + 1) as f32 / segs as f32).min(frac);
        let a0 = start + t0 * tau;
        let a1 = start + t1 * tau;
        let p0 = egui::pos2(center.x + radius * a0.cos(), center.y + radius * a0.sin());
        let p1 = egui::pos2(center.x + radius * a1.cos(), center.y + radius * a1.sin());
        p.line_segment([p0, p1], egui::Stroke::new(thickness, color));
    }
}

/// **The dominant overlay shape**: a donut ring + centred hero number + an
/// uppercase caption below — one call for the trend / momentum / exit /
/// conviction-style gauges. `frac` is 0..1 (the ring fill); `color` is the
/// value tone; the track is the theme border tone.
pub(crate) fn radial_gauge(
    p: &egui::Painter, center: egui::Pos2, radius: f32,
    frac: f32, hero: &str, caption: &str, color: Color32, t: &Theme,
) {
    let track = tint(t, Tone::Border, alpha_muted());
    donut_ring(p, center, radius, 6.0, frac.clamp(0.0, 1.0), 1.0, color, track);
    hero_number(p, center, hero, color);
    sub_label(p, egui::pos2(center.x, center.y + radius + 12.0), caption, color);
}

/// Gauge variant with the value + caption **stacked inside** the ring (vs
/// [`radial_gauge`]'s caption below it) — a thicker 8px ring, `font_xl` value
/// and a tiny `FONT_2XS` caption. For the sentiment / liquidity-style gauges.
pub(crate) fn radial_gauge_stacked(
    p: &egui::Painter, center: egui::Pos2, radius: f32,
    frac: f32, value: &str, caption: &str, color: Color32, t: &Theme,
) {
    let track = tint(t, Tone::Border, alpha_muted());
    donut_ring(p, center, radius, 8.0, frac.clamp(0.0, 1.0), 1.0, color, track);
    p.text(egui::pos2(center.x, center.y - 4.0), egui::Align2::CENTER_CENTER,
        value, crate::ui_kit::style::prop_at(crate::ui_kit::style::font_xl()), color);
    p.text(egui::pos2(center.x, center.y + 14.0), egui::Align2::CENTER_CENTER,
        caption, crate::ui_kit::style::mono_at(FONT_2XS), color);
}

/// Big value + caption (no ring), centred at `center` — the non-gauge "stat".
#[allow(dead_code)] // kit primitive — for the stat-style overlays migrating next
pub(crate) fn stat(p: &egui::Painter, center: egui::Pos2, value: &str, caption: &str, color: Color32) {
    hero_number(p, center, value, color);
    sub_label(p, egui::pos2(center.x, center.y + 22.0), caption, color);
}

/// A labelled metric row: dim `label` (left) + `value` (right) + a thin
/// progress bar below at `frac` 0..1. The breadth / greeks-style overlay row.
/// `rect.top()` is the text baseline; the bar sits ~11px under it.
pub(crate) fn metric_row(
    p: &egui::Painter, rect: egui::Rect, label: &str, value: &str,
    frac: f32, color: Color32, t: &Theme,
) {
    p.text(egui::pos2(rect.left(), rect.top()), egui::Align2::LEFT_TOP,
        label, crate::ui_kit::style::mono_at(FONT_2XS),
        crate::ui_kit::style::color_alpha(color, crate::ui_kit::style::alpha_heavy()));
    p.text(egui::pos2(rect.right(), rect.top()), egui::Align2::RIGHT_TOP,
        value, crate::ui_kit::style::mono_at(FONT_SM), color);
    let bar_y = rect.top() + 11.0;
    let track = egui::Rect::from_min_size(egui::pos2(rect.left(), bar_y), egui::vec2(rect.width(), 3.0));
    p.rect_filled(track, r_pill(), tint(t, Tone::Border, alpha_faint()));
    let fw = rect.width() * frac.clamp(0.0, 1.0);
    if fw > 0.5 {
        let fill = egui::Rect::from_min_size(egui::pos2(rect.left(), bar_y), egui::vec2(fw, 3.0));
        p.rect_filled(fill, r_pill(), color_alpha(color, alpha_dim()));
    }
}

// ── Card shell (the chrome every overlay widget shares) ───────────────────────

/// The shared overlay **card shell** — drop shadow, sentiment-tinted background,
/// a top bevel highlight and the border — painted *under* every widget body.
///
/// This centralises what used to be ~40 lines of inline magic-alpha painting in
/// `chart_widgets`, so all overlay cards share one shell and one set of Sx tones.
/// `sentiment` is the widget's data state in `-1..=1` (drives the subtle green↔red
/// background tint); `card` is the body rect (the header floats above it).
pub(crate) fn overlay_card_frame(
    p: &egui::Painter, card: egui::Rect, sentiment: f32, t: &Theme,
) {
    // Drop shadow — two stacked soft rects offset downward.
    p.rect_filled(card.translate(egui::vec2(0.0, 3.0)).expand(2.0),
        r_lg_cr(), color_alpha(t.shadow_color, crate::ui_kit::style::alpha_soft()));
    p.rect_filled(card.translate(egui::vec2(0.0, 1.5)).expand(1.0),
        r_lg_cr(), color_alpha(t.shadow_color, crate::ui_kit::style::alpha_faint()));

    // Sentiment-driven background tint — pastel on light themes, a faint shift of
    // the toolbar surface on dark themes so the card reads against the chart.
    let is_light = t.is_light();
    let (sr, sg, sb) = if is_light {
        match sentiment {
            s if s > 0.6  => (200, 235, 200),  // soft green
            s if s > 0.2  => (215, 235, 210),  // sage
            s if s > -0.2 => (238, 238, 234),  // warm neutral
            s if s > -0.6 => (240, 225, 195),  // warm amber
            _             => (240, 210, 205),  // soft rose
        }
    } else {
        match sentiment {
            s if s > 0.6  => (t.bull.r() / 3 + t.toolbar_bg.r() * 2 / 3, t.bull.g() / 3 + t.toolbar_bg.g() * 2 / 3, t.bull.b() / 3 + t.toolbar_bg.b() * 2 / 3),
            s if s > 0.2  => (t.toolbar_bg.r().saturating_add(8), t.toolbar_bg.g().saturating_add(12), t.toolbar_bg.b().saturating_add(6)),
            s if s > -0.2 => (t.toolbar_bg.r().saturating_add(5), t.toolbar_bg.g().saturating_add(5), t.toolbar_bg.b().saturating_add(5)),
            s if s > -0.6 => (t.toolbar_bg.r().saturating_add(15), t.toolbar_bg.g().saturating_add(10), t.toolbar_bg.b()),
            _             => (t.bear.r() / 4 + t.toolbar_bg.r() * 3 / 4, t.bear.g() / 4 + t.toolbar_bg.g() * 3 / 4, t.bear.b() / 4 + t.toolbar_bg.b() * 3 / 4),
        }
    };
    p.rect_filled(card, r_lg_cr(), Color32::from_rgb(sr, sg, sb));

    // Top bevel highlight — a 1px lighter line along the top edge.
    let r_lg_u8 = r_lg_cr().nw;
    p.rect_filled(
        egui::Rect::from_min_max(card.min, egui::pos2(card.right(), card.top() + 1.0)),
        egui::CornerRadius { nw: r_lg_u8, ne: r_lg_u8, sw: 0, se: 0 },
        Color32::from_rgba_unmultiplied(255, 255, 255, if is_light { 50 } else { 10 }));

    // Border.
    p.rect_stroke(card, r_lg_cr(),
        Stroke::new(stroke_std(), tint(t, Tone::Border, if is_light { 50 } else { 30 })),
        egui::StrokeKind::Outside);
}

/// Draws the floating **header bar** above a hovered card — the kind icon +
/// label, an optional lock glyph, and the context-menu (`⋯`) + mode-toggle
/// buttons — and returns the two button hit-rects `(ctx_btn, mode_btn)` for the
/// caller's interaction routing. Painter-only: the caller owns click handling
/// and cursor feedback (the buttons' hover *visuals* are driven by `ptr` here).
pub(crate) fn overlay_card_header(
    p: &egui::Painter, card: egui::Rect, card_w: f32,
    icon: &str, label: &str, locked: bool, mode_icon: &str,
    ptr: Option<egui::Pos2>, t: &Theme,
) -> (egui::Rect, egui::Rect) {
    let hdr = egui::Rect::from_min_size(
        egui::pos2(card.left(), card.top() - OVERLAY_HEADER_H - 2.0),
        egui::vec2(card_w, OVERLAY_HEADER_H));
    // Header background + a hairline divider along its bottom edge.
    let hdr_r = r_lg_cr().nw;
    p.rect_filled(hdr,
        egui::CornerRadius { nw: hdr_r, ne: hdr_r, sw: 0, se: 0 },
        crate::ui_kit::style::color_alpha(t.toolbar_bg, 230));
    p.line_segment(
        [egui::pos2(hdr.left() + crate::ui_kit::style::gap_xs(), hdr.bottom()), egui::pos2(hdr.right() - crate::ui_kit::style::gap_xs(), hdr.bottom())],
        Stroke::new(stroke_thin(), tint(t, Tone::Border, alpha_muted())));
    // Icon | label | lock glyph — DECLARED, so the lock follows the label
    // instead of being placed at `24.0 + label.len() as f32 * 7.0 + 6.0`.
    //
    // That expression guessed the label's width from its character count, at
    // 7px each. The label is painted in `mono_xs()`, so 7 was a guess at one
    // font's advance and would be wrong the moment the mono face or its size
    // changed — and it sat next to a pinned `24.0` icon column that had to
    // agree with the icon's actual width by hand. As siblings, both
    // relationships are the row's.
    crate::ui_kit::cascade::El::row()
        .child(
            crate::ui_kit::cascade::El::text_with_font(
                icon,
                crate::ui_kit::style::prop_at(crate::ui_kit::style::font_md()),
            )
            .color(t.accent)
            .fixed(24.0 - crate::ui_kit::style::gap_sm()),
        )
        .child(
            crate::ui_kit::cascade::El::text_with_font(label, crate::ui_kit::style::mono_xs())
                .color(t.text),
        )
        .child_if(
            locked,
            crate::ui_kit::cascade::El::text_with_font(
                "\u{1F512}",
                crate::ui_kit::style::prop_at(crate::ui_kit::style::font_xs()),
            )
            .color(color_half(t.dim))
            .margin_start(6.0),
        )
        .show_with(
            p,
            t,
            egui::Rect::from_min_max(
                egui::pos2(hdr.left() + crate::ui_kit::style::gap_sm(), hdr.top()),
                egui::pos2(hdr.right(), hdr.bottom()),
            ),
        );

    let btn_w = 32.0;
    let btn_h = 22.0;

    // One header button: hover-lit fill + stroke + glyph. Returns its rect.
    let header_btn = |center_x: f32, glyph: &str| -> egui::Rect {
        let r = egui::Rect::from_center_size(
            egui::pos2(center_x, hdr.center().y), egui::vec2(btn_w, btn_h));
        let hov = ptr.map(|q| r.contains(q)).unwrap_or(false);
        p.rect_filled(r, r_sm_cr(),
            if hov { tint(t, Tone::Accent, alpha_line()) } else { tint(t, Tone::Border, alpha_subtle()) });
        p.rect_stroke(r, r_sm_cr(),
            Stroke::new(stroke_thin(), if hov { t.accent } else { tint(t, Tone::Border, alpha_muted()) }),
            egui::StrokeKind::Outside);
        p.text(r.center(), egui::Align2::CENTER_CENTER,
            glyph, crate::ui_kit::style::prop_at(crate::ui_kit::style::font_lg()), if hov { t.accent } else { t.dim });
        r
    };

    let ctx_rect = header_btn(hdr.right() - btn_w - 20.0, Icon::DOTS_THREE);
    let tog_rect = header_btn(hdr.right() - 18.0, mode_icon);
    (ctx_rect, tog_rect)
}

/// The header **context (⋯) button** rect for a card of width `card_w` at
/// `card`. Must reproduce exactly the rect [`overlay_card_header`] paints, so the
/// context-menu popup can anchor to it — otherwise the very click that opens the
/// menu is seen as an "outside" click and the popup closes the same frame.
pub(crate) fn overlay_header_ctx_rect(card: egui::Rect, card_w: f32) -> egui::Rect {
    let hdr = egui::Rect::from_min_size(
        egui::pos2(card.left(), card.top() - OVERLAY_HEADER_H - 2.0),
        egui::vec2(card_w, OVERLAY_HEADER_H));
    let btn_w = 32.0;
    let btn_h = 22.0;
    egui::Rect::from_center_size(
        egui::pos2(hdr.right() - btn_w - 20.0, hdr.center().y), egui::vec2(btn_w, btn_h))
}

/// Horizontal progress / ratio bar (track + fill) at `rect`, `frac` 0..1.
/// Pill-rounded (corner = height/2). Used by the volatility / ratio widgets.
pub(crate) fn progress_bar(
    p: &egui::Painter, rect: egui::Rect, frac: f32, color: Color32, t: &Theme,
) {
    let cr = egui::CornerRadius::same((rect.height() * 0.5) as u8);
    p.rect_filled(rect, cr, tint(t, Tone::Border, alpha_muted()));
    let w = (rect.width() * frac.clamp(0.0, 1.0)).max(0.0);
    if w > 0.5 {
        let fill = egui::Rect::from_min_size(rect.min, egui::vec2(w, rect.height()));
        p.rect_filled(fill, cr, color);
    }
}
