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

use egui::{Color32, CornerRadius, Pos2, Response, Sense, Stroke, Ui, Vec2, Widget};

use super::theme::ComponentTheme;
use super::tokens::{Size, Variant};
use crate::chart::renderer::ui::style as st;

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
        match self.shape {
            Shape::Linear => paint_linear(ui, theme, self),
            Shape::Circular => paint_circular(ui, theme, self),
        }
    }
}

impl Widget for Progress {
    fn ui(self, ui: &mut Ui) -> Response {
        let theme = &crate::chart_renderer::gpu::THEMES[0];
        self.show(ui, theme)
    }
}

fn variant_color(variant: Variant, theme: &dyn ComponentTheme) -> Color32 {
    match variant {
        Variant::Primary => theme.accent(),
        Variant::Danger => theme.bear(),
        _ => theme.accent(),
    }
}

fn paint_linear(ui: &mut Ui, theme: &dyn ComponentTheme, p: Progress) -> Response {
    let h = match p.size { Size::Xs | Size::Sm => 4.0, Size::Md => 6.0, Size::Lg => 8.0 };
    let avail = ui.available_width();
    let w = if avail > 220.0 { 200.0 } else { avail.max(60.0) };
    let (rect, response) = ui.allocate_exact_size(Vec2::new(w, h), Sense::hover());
    if !ui.is_rect_visible(rect) { return response; }

    let painter = ui.painter_at(rect);
    let cr = CornerRadius::same((h * 0.5) as u8);
    painter.rect_filled(rect, cr, st::color_alpha(theme.dim(), 64));

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
    let diameter = match p.size { Size::Xs => 16.0, Size::Sm => 22.0, Size::Md => 28.0, Size::Lg => 34.0 };
    let stroke_w = match p.size { Size::Xs | Size::Sm => 2.0, Size::Md => 3.0, Size::Lg => 4.0 };
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(diameter), Sense::hover());
    if !ui.is_rect_visible(rect) { return response; }

    let painter = ui.painter_at(rect);
    let center = rect.center();
    let radius = diameter * 0.5 - stroke_w * 0.5;

    // Track full circle.
    painter.circle_stroke(center, radius, Stroke::new(stroke_w, st::color_alpha(theme.dim(), 64)));

    let color = variant_color(p.variant, theme);

    if p.indeterminate {
        // 1 rev/sec from wall-clock.
        let time = ui.input(|i| i.time);
        let phase = (time % 1.0) as f32;
        let start_deg = phase * 360.0 - 90.0;
        draw_arc(&painter, center, radius, start_deg, 90.0, stroke_w, color);
        ui.ctx().request_repaint();
    } else {
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
