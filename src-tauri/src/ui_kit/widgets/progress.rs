//! Progress — linear or circular progress indicator.
//!
//! Linear: thin horizontal bar with filled portion.
//! Circular: rotating arc.
//!
//! Both support determinate (specific %) and indeterminate (animated).
//!
//! API:
//!   ui.add(Progress::linear(0.65));            // 65%
//!   ui.add(Progress::linear_indeterminate());
//!   ui.add(Progress::circular(0.5).size(Size::Lg));
//!   ui.add(Progress::circular_indeterminate());

use egui::{Color32, Pos2, Rect, Response, Sense, Stroke, Ui, Vec2, Widget};

use super::theme::ComponentTheme;
use crate::ui_kit::sx::{palette_ct, Tone};
use super::tokens::{Size, Variant};
use crate::ui_kit::tokens as st;

#[derive(Clone, Copy)]
enum Shape { Linear, Circular }

#[must_use = "Progress does nothing until `.show(ui, theme)` or `ui.add(progress)` is called"]
pub struct Progress {
    shape: Shape,
    t: f32,
    indeterminate: bool,
    size: Size,
    variant: Variant,
}

impl Progress {
    pub fn linear(t: f32) -> Self {
        Self { shape: Shape::Linear, t: t.clamp(0.0, 1.0), indeterminate: false, size: Size::Md, variant: Variant::Primary }
    }
    pub fn linear_indeterminate() -> Self {
        Self { shape: Shape::Linear, t: 0.0, indeterminate: true, size: Size::Md, variant: Variant::Primary }
    }
    pub fn circular(t: f32) -> Self {
        Self { shape: Shape::Circular, t: t.clamp(0.0, 1.0), indeterminate: false, size: Size::Md, variant: Variant::Primary }
    }
    pub fn circular_indeterminate() -> Self {
        Self { shape: Shape::Circular, t: 0.0, indeterminate: true, size: Size::Md, variant: Variant::Primary }
    }

    pub fn size(mut self, s: Size) -> Self { self.size = s; self }
    pub fn variant(mut self, v: Variant) -> Self { self.variant = v; self }

    pub fn show(self, ui: &mut Ui, theme: &dyn ComponentTheme) -> Response {
        // Build the ctx from the UI so it carries the AMBIENT RecipeSet.
        // `StyleCtx::from_theme` would hand this widget an empty set — see
        // `ctx.rs` for why that shim must never be used inside a `show`.
        let sctx = super::ctx::StyleCtx::from_ui(theme, ui);
        self.show_ctx(ui, &sctx)
    }

    /// [`StyleCtx`](super::ctx::StyleCtx) entry point.
    ///
    /// Callers that need per-call-site token overrides or an explicit
    /// `RecipeSet` construct a `StyleCtx` and call this directly; `show`
    /// delegates here with the ambient one.
    pub fn show_ctx(self, ui: &mut Ui, sctx: &super::ctx::StyleCtx<'_>) -> Response {
        let theme = sctx.theme();
        match self.shape {
            Shape::Linear => paint_linear(ui, theme, self),
            Shape::Circular => paint_circular(ui, theme, self),
        }
    }
}

impl Widget for Progress {
    fn ui(self, ui: &mut Ui) -> Response {
        let theme = super::theme::active_theme(ui.ctx());
        self.show(ui, &theme)
    }
}

fn variant_color(variant: Variant, theme: &dyn ComponentTheme) -> Color32 {
    match variant {
        Variant::Primary => palette_ct(theme).base(Tone::Accent),
        Variant::Danger => palette_ct(theme).base(Tone::Bear),
        _ => palette_ct(theme).base(Tone::Accent),
    }
}

fn paint_linear(ui: &mut Ui, theme: &dyn ComponentTheme, p: Progress) -> Response {
    let h = match p.size { Size::Xs | Size::Sm => 4.0, Size::Md => 6.0, Size::Lg | Size::Xl => 8.0 };
    let avail = ui.available_width();
    let w = if avail > 220.0 { 200.0 } else { avail.max(60.0) };
    let (rect, response) = ui.allocate_exact_size(Vec2::new(w, h), Sense::hover());
    if !ui.is_rect_visible(rect) { return response; }

    let painter = ui.painter_at(rect);
    // `progress` key — the TRACK. The fill colour encodes the variant and
    // stays with the widget. Default radius is a true pill (half the height).
    let track_col = st::color_alpha(palette_ct(theme).base(Tone::Dim), 64);
    let (cr, track_fill, _) = super::theme::resolve_control_chrome(
        ui.ctx(), theme, "progress", h * 0.5, track_col, track_col, 0.0,
    );
    painter.rect_filled(rect, cr, track_fill);

    let fill = variant_color(p.variant, theme);

    if p.indeterminate {
        // Phase 0..1.5 driven by wall-clock; 1.4s period.
        let time = ui.input(|i| i.time);
        let phase = ((time / 1.4) % 1.0) as f32; // 0..1
        let seg_w = w * 0.30;
        let total_travel = w + seg_w;
        let x_left = rect.left() - seg_w + phase * total_travel;
        let x0 = x_left.max(rect.left());
        let x1 = (x_left + seg_w).min(rect.right());
        if x1 > x0 {
            let seg = egui::Rect::from_min_max(
                Pos2::new(x0, rect.top()),
                Pos2::new(x1, rect.bottom()),
            );
            painter.rect_filled(seg, cr, fill);
        }
        ui.ctx().request_repaint();
    } else {
        let filled = egui::Rect::from_min_size(rect.min, Vec2::new(w * p.t, h));
        painter.rect_filled(filled, cr, fill);
    }

    response
}

fn paint_circular(ui: &mut Ui, theme: &dyn ComponentTheme, p: Progress) -> Response {
    let diameter = match p.size { Size::Xs => 16.0, Size::Sm => 22.0, Size::Md => 28.0, Size::Lg | Size::Xl => 34.0 };
    let stroke_w = match p.size { Size::Xs | Size::Sm => 1.5, Size::Md => 2.0, Size::Lg | Size::Xl => 2.5 };
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(diameter), Sense::hover());
    if !ui.is_rect_visible(rect) { return response; }

    let painter = ui.painter_at(rect);
    let color = variant_color(p.variant, theme);

    if p.indeterminate {
        let time = ui.input(|i| i.time);
        paint_worm2(&painter, rect, stroke_w, color, time);
        ui.ctx().request_repaint();
    } else {
        let center = rect.center();
        let radius = diameter * 0.5 - stroke_w * 0.5;
        painter.circle_stroke(center, radius, Stroke::new(stroke_w, st::color_alpha(palette_ct(theme).base(Tone::Dim), 64)));
        let span = p.t * 360.0;
        if span > 0.0 {
            draw_arc(&painter, center, radius, -90.0, span, stroke_w, color);
        }
    }

    response
}

fn draw_arc(
    painter: &egui::Painter,
    center: Pos2,
    radius: f32,
    start_deg: f32,
    span_deg: f32,
    stroke_w: f32,
    color: Color32,
) {
    // Approximate arc with line segments.
    let segments = ((span_deg.abs() / 6.0) as usize).max(6);
    let mut points = Vec::with_capacity(segments + 1);
    for i in 0..=segments {
        let t = i as f32 / segments as f32;
        let deg = start_deg + span_deg * t;
        let rad = deg.to_radians();
        points.push(Pos2::new(
            center.x + rad.cos() * radius,
            center.y + rad.sin() * radius,
        ));
    }
    painter.add(egui::Shape::line(points, Stroke::new(stroke_w, color)));
}

// Worm 2: ghost outlines of the three logo shapes with traveling segments.
// Logo lives in a 24×24 viewBox; rect is the actual pixel square.
fn paint_worm2(painter: &egui::Painter, rect: Rect, stroke_w: f32, color: Color32, time: f64) {
    let scale = rect.width() / 24.0;
    let ghost = crate::ui_kit::style::color_alpha(color, 36);
    let to_s = |x: f32, y: f32| Pos2::new(rect.min.x + x * scale, rect.min.y + y * scale);

    // --- Dot: small circle top-left (cx 6.159, cy 6.290, r 3.564) ---
    let dot_c = to_s(6.15852, 6.28967);
    let dot_r = (3.5638 * scale - stroke_w * 0.5).max(1.0);
    painter.circle_stroke(dot_c, dot_r, Stroke::new(stroke_w, ghost));
    {
        let phase = ((time / 1.1) % 1.0) as f32;
        draw_arc(painter, dot_c, dot_r, phase * 360.0 - 90.0, 198.0, stroke_w, color);
    }

    // --- Ring: large circle bottom-right (cx 15.577, cy 15.708, r 5.855) ---
    let ring_c = to_s(15.577, 15.7082);
    let ring_r = (5.85481 * scale - stroke_w * 0.5).max(1.0);
    painter.circle_stroke(ring_c, ring_r, Stroke::new(stroke_w, ghost));
    {
        let phase = ((time / 2.6) % 1.0) as f32;
        draw_arc(painter, ring_c, ring_r, phase * 360.0 - 90.0, 136.8, stroke_w, color);
    }

    // --- Boomerang arc: sampled bezier path ---
    let arc_pts = boomerang_pts(scale, rect.min);
    painter.add(egui::Shape::line(arc_pts.clone(), Stroke::new(stroke_w, ghost)));
    {
        let n = arc_pts.len();
        let worm_len = ((n as f32 * 0.42) as usize).max(2);
        let phase = ((time / 1.7) % 1.0) as f32;
        let i0 = (phase * n as f32) as usize % n;
        let pts: Vec<Pos2> = (0..=worm_len).map(|k| arc_pts[(i0 + k) % n]).collect();
        if pts.len() >= 2 {
            painter.add(egui::Shape::line(pts, Stroke::new(stroke_w, color)));
        }
    }
}

// Sample the closed boomerang SVG path into a polyline (24×24 viewBox coords scaled to pixels).
fn boomerang_pts(scale: f32, origin: Pos2) -> Vec<Pos2> {
    let s = |x: f32, y: f32| Pos2::new(origin.x + x * scale, origin.y + y * scale);
    let cb = |p0: [f32; 2], p1: [f32; 2], p2: [f32; 2], p3: [f32; 2], t: f32| -> Pos2 {
        let u = 1.0 - t;
        s(
            u*u*u*p0[0] + 3.0*u*u*t*p1[0] + 3.0*u*t*t*p2[0] + t*t*t*p3[0],
            u*u*u*p0[1] + 3.0*u*u*t*p1[1] + 3.0*u*t*t*p2[1] + t*t*t*p3[1],
        )
    };
    let n = 7usize; // samples per bezier curve
    let mut pts: Vec<Pos2> = Vec::with_capacity(64);

    // Top-right rounded end
    for i in 0..n { pts.push(cb([13.6456,3.38161],[15.5131,1.51417],[18.5651,1.53812],[20.4625,3.43547], i as f32/n as f32)); }
    // Right outer curve
    for i in 0..n { pts.push(cb([20.4625,3.43547],[22.3595,5.33285],[22.3837,8.385],[20.5163,10.2525], i as f32/n as f32)); }
    // Short corner transition + long diagonal to bottom-left
    pts.push(s(20.3209, 10.4355));
    pts.push(s(20.4293, 10.5439));
    pts.push(s(10.4567, 20.5166));
    // Bottom-left rounded end
    for i in 0..n { pts.push(cb([10.4353,20.538],[8.52338,22.45],[5.42336,22.4505],[3.51134,20.5387], i as f32/n as f32)); }
    // Left outer curve
    for i in 0..n { pts.push(cb([3.51134,20.5387],[1.59935,18.6267],[1.59935,15.526],[3.51134,13.614], i as f32/n as f32)); }
    // Long diagonal back to top-right
    pts.push(s(13.1263, 3.99895));
    // Closing curve back to start
    for i in 0..=n { pts.push(cb([13.1263,3.99895],[13.2793,3.78238],[13.4519,3.57531],[13.6456,3.38161], i as f32/n as f32)); }

    // Resample to uniform arc-length so the travelling worm moves at constant
    // speed and doesn't jump across the two long diagonal edges above.
    resample_loop_uniform(&pts, 96)
}

/// Resample a closed polyline to `out_n` points evenly spaced by arc length.
fn resample_loop_uniform(pts: &[Pos2], out_n: usize) -> Vec<Pos2> {
    if pts.len() < 2 || out_n < 2 { return pts.to_vec(); }
    let mut cum = Vec::with_capacity(pts.len());
    cum.push(0.0f32);
    for i in 1..pts.len() { cum.push(cum[i - 1] + pts[i - 1].distance(pts[i])); }
    let total = *cum.last().unwrap();
    if total <= 1e-4 { return pts.to_vec(); }
    let mut out = Vec::with_capacity(out_n);
    let mut seg = 0usize;
    for j in 0..out_n {
        let target = total * j as f32 / out_n as f32;
        while seg + 1 < pts.len() && cum[seg + 1] < target { seg += 1; }
        let seg_len = cum[seg + 1] - cum[seg];
        let local = if seg_len > 1e-6 { (target - cum[seg]) / seg_len } else { 0.0 };
        out.push(pts[seg] + (pts[seg + 1] - pts[seg]) * local);
    }
    out
}
