//! Chart **primitives** — painter-based, theme-styled data visualizations that
//! widget bodies (and the dashboard) compose from. Each takes an absolute `rect`
//! (or `center`+`radius`), the data, a [`ChartStyle`] and the `Theme`. They draw
//! only — no layout, no interaction — so they slot into the overlay render path.
//!
//! Colour comes from the Sx ramps (via [`categorical`] / `shade` / `tint`) so a
//! chart re-tints with the active `ColorScheme`; dimension (stroke, fonts,
//! proportions, dash/dot) comes from [`ChartStyle`] so it re-shapes with the
//! active `StyleSystem`. Wave 1: stat · bars · area/line · histogram · donut ·
//! pie · radar · heatmap · multi-ring.

use std::f32::consts::{PI, TAU};
use egui::{self, Color32, Pos2, Stroke};
use crate::ui_kit::sx::{Tone, Shade};
use crate::chart_renderer::ui::style::*;
use crate::chart_renderer::gpu::Theme;
use super::style::{ChartStyle, FillMode, categorical, stroke_path};

// ── Geometry helpers (y-down screen convention; angle 0 = +x, grows clockwise) ─

#[inline]
fn polar(center: Pos2, r: f32, a: f32) -> Pos2 {
    egui::pos2(center.x + r * a.cos(), center.y + r * a.sin())
}

/// Points along an arc `a0`→`a1` (radians).
fn arc_points(center: Pos2, r: f32, a0: f32, a1: f32, segs: usize) -> Vec<Pos2> {
    let segs = segs.max(2);
    let step = (a1 - a0) / segs as f32;
    (0..=segs).map(|i| polar(center, r, a0 + step * i as f32)).collect()
}

/// Stroke an arc.
fn stroke_arc(p: &egui::Painter, center: Pos2, r: f32, a0: f32, a1: f32, stroke: Stroke) {
    let pts = arc_points(center, r, a0, a1, 48);
    for w in pts.windows(2) { p.line_segment([w[0], w[1]], stroke); }
}

/// Fill a wedge (pie slice) `a0`→`a1` as a triangle fan from `center`.
fn fill_wedge(p: &egui::Painter, center: Pos2, r: f32, a0: f32, a1: f32, color: Color32) {
    let segs = (((a1 - a0).abs() / 0.16).ceil() as usize).max(2);
    let step = (a1 - a0) / segs as f32;
    for i in 0..segs {
        let b0 = a0 + step * i as f32;
        let b1 = a0 + step * (i + 1) as f32;
        p.add(egui::Shape::convex_polygon(
            vec![center, polar(center, r, b0), polar(center, r, b1)],
            color, Stroke::NONE));
    }
}

const TOP: f32 = -PI * 0.5; // 12 o'clock

// ── Stat block ────────────────────────────────────────────────────────────────

/// **Stat** — an eyebrow (metric name, small caps) above a hero number, with an
/// optional sub-label beneath. The "just numbers" widget. Vertically centred in
/// `rect`. `eyebrow`/`sub` may be empty.
pub(crate) fn stat(
    p: &egui::Painter, rect: egui::Rect, eyebrow: &str, value: &str, sub: &str,
    tone: Tone, st: &ChartStyle, t: &Theme,
) {
    let cx = rect.center().x;
    let cy = rect.center().y;
    let value_col = shade(t, tone, Shade::S400);
    if !eyebrow.is_empty() {
        p.text(egui::pos2(cx, cy - st.num_lg * 0.62 - st.eyebrow), egui::Align2::CENTER_CENTER,
            &eyebrow.to_uppercase(), egui::FontId::monospace(st.eyebrow),
            tint(t, Tone::Dim, alpha_line()));
    }
    p.text(egui::pos2(cx, cy), egui::Align2::CENTER_CENTER,
        value, egui::FontId::proportional(st.num_lg), value_col);
    if !sub.is_empty() {
        p.text(egui::pos2(cx, cy + st.num_lg * 0.6 + st.num_sm * 0.4), egui::Align2::CENTER_CENTER,
            sub, egui::FontId::monospace(st.num_sm), tint(t, Tone::Dim, alpha_active()));
    }
}

// ── Bars ────────────────────────────────────────────────────────────────────

/// **Bar chart** — vertical bars from a baseline, rounded tops, faint baseline.
/// `values` are scaled to the tallest; `tone` colours the bars.
pub(crate) fn bars(
    p: &egui::Painter, rect: egui::Rect, values: &[f32], tone: Tone, st: &ChartStyle, t: &Theme,
) {
    if values.is_empty() { return; }
    let max = values.iter().cloned().fold(f32::MIN, f32::max).max(1e-6);
    let n = values.len();
    let gap = st.gap.min(rect.width() / (n as f32 * 2.0));
    let bw = (rect.width() - gap * (n as f32 - 1.0)) / n as f32;
    let base = rect.bottom();
    let col = shade(t, tone, Shade::S400);
    let track = tint(t, Tone::Border, st.track_alpha);
    p.hline(rect.x_range(), base, Stroke::new(st.line_thin, track));
    for (i, &v) in values.iter().enumerate() {
        let x = rect.left() + i as f32 * (bw + gap);
        let h = (v.max(0.0) / max) * rect.height();
        let r = egui::Rect::from_min_size(egui::pos2(x, base - h), egui::vec2(bw, h));
        let cr = egui::CornerRadius { nw: st.corner as u8, ne: st.corner as u8, sw: 0, se: 0 };
        p.rect_filled(r, cr, col);
    }
}

// ── Histogram ─────────────────────────────────────────────────────────────────

/// **Histogram** — contiguous distribution bars (no inter-bar gap), value-shaded
/// so taller bins read hotter. A baseline anchors them.
pub(crate) fn histogram(
    p: &egui::Painter, rect: egui::Rect, values: &[f32], tone: Tone, st: &ChartStyle, t: &Theme,
) {
    if values.is_empty() { return; }
    let max = values.iter().cloned().fold(f32::MIN, f32::max).max(1e-6);
    let n = values.len();
    let bw = rect.width() / n as f32;
    let base = rect.bottom();
    let lo = tint(t, tone, alpha_muted());
    let hi = shade(t, tone, Shade::S400);
    p.hline(rect.x_range(), base, Stroke::new(st.line_thin, tint(t, Tone::Border, st.track_alpha)));
    for (i, &v) in values.iter().enumerate() {
        let frac = (v.max(0.0) / max).clamp(0.0, 1.0);
        let x = rect.left() + i as f32 * bw;
        let h = frac * rect.height();
        let r = egui::Rect::from_min_size(egui::pos2(x + 0.5, base - h), egui::vec2(bw - 1.0, h));
        p.rect_filled(r, 0.0, super::super::kit::lerp_color(lo, hi, frac));
    }
}

// ── Area / line ───────────────────────────────────────────────────────────────

/// **Area / line chart** — a poly-line over the series (pattern-aware) with a
/// soft area fill below it (honours [`FillMode`]). `values` auto-scale to their
/// own min/max with a little headroom.
pub(crate) fn area_line(
    p: &egui::Painter, rect: egui::Rect, values: &[f32], tone: Tone, st: &ChartStyle, t: &Theme,
) {
    if values.len() < 2 { return; }
    let (mut lo, mut hi) = (f32::MAX, f32::MIN);
    for &v in values { lo = lo.min(v); hi = hi.max(v); }
    let span = (hi - lo).max(1e-6);
    let pad = span * 0.12;
    let (lo, hi) = (lo - pad, hi + pad);
    let n = values.len();
    let xs = |i: usize| rect.left() + rect.width() * i as f32 / (n as f32 - 1.0);
    let ys = |v: f32| rect.bottom() - rect.height() * ((v - lo) / (hi - lo)).clamp(0.0, 1.0);
    let pts: Vec<Pos2> = values.iter().enumerate().map(|(i, &v)| egui::pos2(xs(i), ys(v))).collect();
    let col = shade(t, tone, Shade::S400);

    if st.fill != FillMode::Hollow {
        // Area fill: triangle-strip between the curve and the baseline.
        let base = rect.bottom();
        let fill = tint(t, tone, if st.fill == FillMode::Solid { st.fill_alpha } else { st.track_alpha });
        for w in pts.windows(2) {
            p.add(egui::Shape::convex_polygon(
                vec![w[0], w[1], egui::pos2(w[1].x, base), egui::pos2(w[0].x, base)],
                fill, Stroke::NONE));
        }
    }
    stroke_path(p, &pts, col, st, st.line_std);
    // Endpoint dot for a polished read.
    if let Some(last) = pts.last() {
        p.circle_filled(*last, st.line_bold + 1.0, col);
    }
}

// ── Donut (segmented ring) ────────────────────────────────────────────────────

/// **Donut** — a categorical segmented ring. `segments` are relative weights;
/// each gets a [`categorical`] colour. Thickness scales with the radius.
pub(crate) fn donut(
    p: &egui::Painter, center: Pos2, radius: f32, segments: &[f32], st: &ChartStyle, t: &Theme,
) {
    let total: f32 = segments.iter().map(|v| v.max(0.0)).sum::<f32>().max(1e-6);
    let thick = (radius * 0.30).max(4.0);
    let track = tint(t, Tone::Border, st.track_alpha);
    stroke_arc(p, center, radius, 0.0, TAU, Stroke::new(thick, track));
    let mut a = TOP;
    let gap_a = 0.04; // small wedge gap between segments
    for (i, &v) in segments.iter().enumerate() {
        let sweep = TAU * (v.max(0.0) / total);
        if sweep <= gap_a { a += sweep; continue; }
        stroke_arc(p, center, radius, a + gap_a * 0.5, a + sweep - gap_a * 0.5,
            Stroke::new(thick, categorical(t, i)));
        a += sweep;
    }
}

// ── Pie ───────────────────────────────────────────────────────────────────────

/// **Pie** — filled categorical wedges. `segments` are relative weights.
pub(crate) fn pie(
    p: &egui::Painter, center: Pos2, radius: f32, segments: &[f32], _st: &ChartStyle, t: &Theme,
) {
    let total: f32 = segments.iter().map(|v| v.max(0.0)).sum::<f32>().max(1e-6);
    let mut a = TOP;
    for (i, &v) in segments.iter().enumerate() {
        let sweep = TAU * (v.max(0.0) / total);
        fill_wedge(p, center, radius, a, a + sweep, categorical(t, i));
        a += sweep;
    }
    // Subtle hub for a cleaner centre.
    p.circle_filled(center, radius * 0.16, tint(t, Tone::Bg, alpha_scrim()));
}

// ── Radar ─────────────────────────────────────────────────────────────────────

/// **Radar** — a polygon over `values` (each 0..1) on evenly-spaced spokes, with
/// a faint concentric-ring + spoke grid. `tone` colours the polygon.
pub(crate) fn radar(
    p: &egui::Painter, center: Pos2, radius: f32, values: &[f32], tone: Tone, st: &ChartStyle, t: &Theme,
) {
    let n = values.len();
    if n < 3 { return; }
    let grid = tint(t, Tone::Border, st.track_alpha);
    // Concentric rings.
    for k in 1..=3 {
        let rr = radius * k as f32 / 3.0;
        let ring = arc_points(center, rr, 0.0, TAU, n.max(12) * 2);
        for w in ring.windows(2) { p.line_segment([w[0], w[1]], Stroke::new(st.line_thin, grid)); }
    }
    let ang = |i: usize| TOP + TAU * i as f32 / n as f32;
    // Spokes.
    for i in 0..n {
        p.line_segment([center, polar(center, radius, ang(i))], Stroke::new(st.line_thin, grid));
    }
    // Data polygon.
    let col = shade(t, tone, Shade::S400);
    let poly: Vec<Pos2> = (0..n).map(|i| polar(center, radius * values[i].clamp(0.0, 1.0), ang(i))).collect();
    p.add(egui::Shape::convex_polygon(poly.clone(), tint(t, tone, st.fill_alpha), Stroke::NONE));
    let mut ring = poly.clone();
    if let Some(first) = poly.first().copied() { ring.push(first); }
    stroke_path(p, &ring, col, st, st.line_std);
    for v in &poly { p.circle_filled(*v, st.line_bold, col); }
}

// ── Heatmap ───────────────────────────────────────────────────────────────────

/// **Heatmap** — a `rows`×`cols` grid of cells coloured by `values` (row-major,
/// each 0..1) ramped from a faint to a strong `tone`.
pub(crate) fn heatmap(
    p: &egui::Painter, rect: egui::Rect, rows: usize, cols: usize, values: &[f32],
    tone: Tone, st: &ChartStyle, t: &Theme,
) {
    if rows == 0 || cols == 0 { return; }
    let gap = (st.gap * 0.5).min(2.0);
    let cw = (rect.width() - gap * (cols as f32 - 1.0)) / cols as f32;
    let ch = (rect.height() - gap * (rows as f32 - 1.0)) / rows as f32;
    let lo = tint(t, tone, alpha_faint());
    let hi = shade(t, tone, Shade::S400);
    for r in 0..rows {
        for c in 0..cols {
            let v = values.get(r * cols + c).copied().unwrap_or(0.0).clamp(0.0, 1.0);
            let x = rect.left() + c as f32 * (cw + gap);
            let y = rect.top() + r as f32 * (ch + gap);
            let cell = egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(cw, ch));
            p.rect_filled(cell, st.corner.min(cw * 0.25) as u8,
                super::super::kit::lerp_color(lo, hi, v));
        }
    }
}

// ── Heatmap (signed / diverging) ──────────────────────────────────────────────

/// **Signed heatmap** — like [`heatmap`] but each `values` cell is `-1..1`:
/// magnitude drives intensity (ramped from a faint track), sign picks `pos` vs
/// `neg` colour. For momentum / ROC / change grids that are bull-vs-bear.
pub(crate) fn heatmap_signed(
    p: &egui::Painter, rect: egui::Rect, rows: usize, cols: usize, values: &[f32],
    pos: Color32, neg: Color32, st: &ChartStyle, t: &Theme,
) {
    if rows == 0 || cols == 0 { return; }
    // Contiguous cells (gap baked into a 1px inset) so a caller can place labels
    // at `rect.left() + c*cw + cw/2` without knowing internal spacing.
    let cw = rect.width() / cols as f32;
    let ch = rect.height() / rows as f32;
    let track = tint(t, Tone::Border, alpha_faint());
    for r in 0..rows {
        for c in 0..cols {
            let v = values.get(r * cols + c).copied().unwrap_or(0.0).clamp(-1.0, 1.0);
            let col = if v >= 0.0 { pos } else { neg };
            let fill = super::super::kit::lerp_color(track, col, v.abs());
            let x = rect.left() + c as f32 * cw;
            let y = rect.top() + r as f32 * ch;
            let cell = egui::Rect::from_min_size(egui::pos2(x + 0.5, y + 0.5), egui::vec2(cw - 1.0, ch - 1.0));
            p.rect_filled(cell, st.corner.min(cw * 0.2) as u8, fill);
        }
    }
}

// ── Multi-ring ────────────────────────────────────────────────────────────────

/// Radius of ring `i` (0 = outermost) for an `n`-ring [`multiring`] of `radius`.
/// Exposed so callers can place per-ring labels exactly on their ring.
pub(crate) fn multiring_radius(radius: f32, n: usize, i: usize) -> f32 {
    let thick = (radius / (n.max(1) as f32 * 1.7)).max(3.0);
    let step = (radius - thick * 0.5) / n.max(1) as f32;
    radius - i as f32 * step
}

/// **Multi-ring** — concentric value rings, outermost first. Each ring is
/// `(value 0..1, colour)`; it fills from 12 o'clock clockwise over a faint track.
pub(crate) fn multiring_colored(
    p: &egui::Painter, center: Pos2, radius: f32, rings: &[(f32, Color32)], st: &ChartStyle, t: &Theme,
) {
    let n = rings.len();
    if n == 0 { return; }
    let thick = (radius / (n as f32 * 1.7)).max(3.0);
    let track = tint(t, Tone::Border, st.track_alpha);
    for (i, &(v, col)) in rings.iter().enumerate() {
        let rr = multiring_radius(radius, n, i);
        stroke_arc(p, center, rr, 0.0, TAU, Stroke::new(thick, track));
        let frac = v.clamp(0.0, 1.0);
        if frac > 0.001 {
            stroke_arc(p, center, rr, TOP, TOP + TAU * frac, Stroke::new(thick, col));
        }
    }
}

/// [`multiring_colored`] with [`categorical`] colours from the `values`.
pub(crate) fn multiring(
    p: &egui::Painter, center: Pos2, radius: f32, values: &[f32], st: &ChartStyle, t: &Theme,
) {
    let rings: Vec<(f32, Color32)> = values.iter().enumerate()
        .map(|(i, &v)| (v, categorical(t, i))).collect();
    multiring_colored(p, center, radius, &rings, st, t);
}
