//! Skeleton — placeholder shimmer for content being loaded.
//!
//! Use during async loads: list rows, charts, profile cards.
//!
//! API:
//!   if !data.loaded {
//!       ui.add(Skeleton::rect(120.0, 16.0));
//!       ui.add(Skeleton::text(180.0));        // single line, 16px tall
//!       ui.add(Skeleton::lines(3, 200.0));    // 3 lines
//!       ui.add(Skeleton::circle(40.0));       // avatar placeholder
//!   } else {
//!       paint_real_content(...);
//!   }

use egui::{CornerRadius, Pos2, Response, Sense, Ui, Vec2, Widget};

use super::theme::ComponentTheme;
use crate::chart::renderer::ui::style as st;

#[derive(Clone, Copy)]
enum Shape {
    Rect { w: f32, h: f32 },
    Lines { count: u32, w: f32 },
    Circle { d: f32 },
}

#[must_use = "Skeleton does nothing until `.show(ui, theme)` or `ui.add(skeleton)` is called"]
pub struct Skeleton {
    shape: Shape,
    radius: Option<f32>,
}

impl Skeleton {
    pub fn rect(width: f32, height: f32) -> Self {
        Self { shape: Shape::Rect { w: width, h: height }, radius: None }
    }
    pub fn text(width: f32) -> Self {
        Self { shape: Shape::Rect { w: width, h: st::font_sm() }, radius: None }
    }
    pub fn lines(count: u32, width: f32) -> Self {
        Self { shape: Shape::Lines { count, w: width }, radius: None }
    }
    pub fn circle(diameter: f32) -> Self {
        Self { shape: Shape::Circle { d: diameter }, radius: None }
    }
    pub fn corner_radius(mut self, r: f32) -> Self { self.radius = Some(r); self }

    pub fn show(self, ui: &mut Ui, theme: &dyn ComponentTheme) -> Response {
        match self.shape {
            Shape::Rect { w, h } => paint_rect(ui, theme, w, h, self.radius.unwrap_or(4.0)),
            Shape::Circle { d } => paint_rect(ui, theme, d, d, d * 0.5),
            Shape::Lines { count, w } => paint_lines(ui, theme, count, w, self.radius.unwrap_or(4.0)),
        }
    }
}

impl Widget for Skeleton {
    fn ui(self, ui: &mut Ui) -> Response {
        let theme = &crate::chart_renderer::gpu::THEMES[0];
        self.show(ui, theme)
    }
}

fn paint_rect(ui: &mut Ui, theme: &dyn ComponentTheme, w: f32, h: f32, radius: f32) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(w, h), Sense::hover());
    if !ui.is_rect_visible(rect) { return response; }

    let painter = ui.painter_at(rect);
    let cr = CornerRadius::same(radius as u8);

    // Base.
    painter.rect_filled(rect, cr, st::color_alpha(theme.dim(), 40));

    // Shimmer band, ~1.4s cycle.
    let time = ui.input(|i| i.time);
    let phase = ((time / 1.4) % 1.0) as f32; // 0..1
    let band_w = (w * 0.30).max(20.0);
    let total_travel = w + band_w;
    let x_left = rect.left() - band_w + phase * total_travel;

    // Three vertical strips for a soft gradient feel.
    let band_color_full = st::color_alpha(theme.text(), 24);
    let band_color_soft = st::color_alpha(theme.text(), 12);
    let strips = [
        (-0.5, 0.5, band_color_soft),
        (-0.25, 0.25, band_color_full),
    ];
    for (a, b, color) in strips {
        let s_left = x_left + band_w * (0.5 + a);
        let s_right = x_left + band_w * (0.5 + b);
        let x0 = s_left.max(rect.left());
        let x1 = s_right.min(rect.right());
        if x1 > x0 {
            let strip = egui::Rect::from_min_max(
                Pos2::new(x0, rect.top()),
                Pos2::new(x1, rect.bottom()),
            );
            painter.rect_filled(strip, cr, color);
        }
    }

    ui.ctx().request_repaint();
    response
}

fn paint_lines(ui: &mut Ui, theme: &dyn ComponentTheme, count: u32, w: f32, radius: f32) -> Response {
    let line_h = st::font_sm();
    let gap = st::gap_2xs();
    let total_h = count as f32 * line_h + (count.saturating_sub(1)) as f32 * gap;
    let (outer, response) = ui.allocate_exact_size(Vec2::new(w, total_h), Sense::hover());
    if !ui.is_rect_visible(outer) { return response; }

    for i in 0..count {
        // Last line shorter for visual realism.
        let line_w = if i + 1 == count { w * 0.65 } else { w };
        let y = outer.top() + i as f32 * (line_h + gap);
        let rect = egui::Rect::from_min_size(Pos2::new(outer.left(), y), Vec2::new(line_w, line_h));
        let painter = ui.painter_at(rect);
        let cr = CornerRadius::same(radius as u8);
        painter.rect_filled(rect, cr, st::color_alpha(theme.dim(), 40));

        let time = ui.input(|i| i.time);
        let phase = ((time / 1.4) % 1.0) as f32;
        let band_w = (line_w * 0.30).max(20.0);
        let total_travel = line_w + band_w;
        let x_left = rect.left() - band_w + phase * total_travel;
        let band_color = st::color_alpha(theme.text(), 24);
        let x0 = x_left.max(rect.left());
        let x1 = (x_left + band_w).min(rect.right());
        if x1 > x0 {
            let strip = egui::Rect::from_min_max(
                Pos2::new(x0, rect.top()),
                Pos2::new(x1, rect.bottom()),
            );
            painter.rect_filled(strip, cr, band_color);
        }
    }
    ui.ctx().request_repaint();
    response
}
