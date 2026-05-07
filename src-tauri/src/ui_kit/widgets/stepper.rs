//! Stepper — numbered step indicator for wizards and onboarding.
//!
//! API:
//!   let steps = ["Account", "Connect Broker", "Set Risk", "Done"];
//!   ui.add(Stepper::new(&steps, current_step));
//!
//!   Stepper::new(&steps, 2)
//!     .vertical(true)
//!     .show_labels(true)
//!     .show(ui, theme);

use egui::{Color32, FontId, Pos2, Response, Sense, Stroke, Ui, Vec2, Widget};

use super::theme::ComponentTheme;
use super::tokens::Size;
use crate::chart::renderer::ui::style as st;
use crate::ui_kit::icons::Icon;

#[must_use = "Stepper does nothing until `.show(ui, theme)` or `ui.add(stepper)` is called"]
pub struct Stepper<'a> {
    steps: &'a [&'a str],
    current: usize,
    vertical: bool,
    show_labels: bool,
    size: Size,
}

impl<'a> Stepper<'a> {
    pub fn new(steps: &'a [&'a str], current: usize) -> Self {
        Self {
            steps,
            current,
            vertical: false,
            show_labels: true,
            size: Size::Md,
        }
    }

    pub fn vertical(mut self, v: bool) -> Self { self.vertical = v; self }
    pub fn show_labels(mut self, v: bool) -> Self { self.show_labels = v; self }
    pub fn size(mut self, s: Size) -> Self { self.size = s; self }

    pub fn show(self, ui: &mut Ui, theme: &dyn ComponentTheme) -> Response {
        let Stepper { steps, current, vertical, show_labels, size } = self;
        if steps.is_empty() {
            let (_, r) = ui.allocate_exact_size(Vec2::ZERO, Sense::hover());
            return r;
        }

        let circle_d = match size { Size::Xs => 18.0, Size::Sm => 22.0, Size::Md => 26.0, Size::Lg => 32.0 };
        let label_font = FontId::proportional(size.font_size());
        let num_font = FontId::proportional(circle_d * 0.45);
        let line_thickness = 2.0;

        let accent = theme.accent();
        let dim = theme.dim();
        let text = theme.text();
        let line_completed = accent;
        let line_future = st::color_alpha(dim, 80);

        if vertical {
            paint_vertical(ui, theme, steps, current, show_labels, circle_d, label_font, num_font,
                line_thickness, accent, dim, text, line_completed, line_future)
        } else {
            paint_horizontal(ui, theme, steps, current, show_labels, circle_d, label_font, num_font,
                line_thickness, accent, dim, text, line_completed, line_future)
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn paint_horizontal(
    ui: &mut Ui,
    _theme: &dyn ComponentTheme,
    steps: &[&str],
    current: usize,
    show_labels: bool,
    circle_d: f32,
    label_font: FontId,
    num_font: FontId,
    line_thickness: f32,
    accent: Color32,
    dim: Color32,
    text: Color32,
    line_completed: Color32,
    line_future: Color32,
) -> Response {
    let n = steps.len();
    let avail_w = ui.available_width();
    let label_h = if show_labels { label_font.size + 4.0 } else { 0.0 };
    let h = circle_d + label_h + 4.0;
    let (rect, response) = ui.allocate_exact_size(Vec2::new(avail_w, h), Sense::hover());

    if !ui.is_rect_visible(rect) { return response; }
    let painter = ui.painter_at(rect);
    let cy = rect.top() + circle_d * 0.5;

    // Compute circle centers evenly distributed.
    let centers: Vec<f32> = if n == 1 {
        vec![rect.center().x]
    } else {
        let pad = circle_d * 0.5 + 4.0;
        let left = rect.left() + pad;
        let right = rect.right() - pad;
        let step = (right - left) / (n as f32 - 1.0).max(1.0);
        (0..n).map(|i| left + step * i as f32).collect()
    };

    // Connector lines first.
    for i in 0..n.saturating_sub(1) {
        let c0 = Pos2::new(centers[i] + circle_d * 0.5, cy);
        let c1 = Pos2::new(centers[i + 1] - circle_d * 0.5, cy);
        let col = if i + 1 <= current { line_completed } else { line_future };
        painter.line_segment([c0, c1], Stroke::new(line_thickness, col));
    }

    // Circles + labels.
    for (i, label) in steps.iter().enumerate() {
        let center = Pos2::new(centers[i], cy);
        paint_circle(&painter, center, circle_d, i, current, &num_font, accent, dim);
        if show_labels {
            let col = if i == current { text } else if i < current { text } else { dim };
            painter.text(
                Pos2::new(center.x, rect.top() + circle_d + 4.0),
                egui::Align2::CENTER_TOP,
                label,
                label_font.clone(),
                col,
            );
        }
    }

    response
}

#[allow(clippy::too_many_arguments)]
fn paint_vertical(
    ui: &mut Ui,
    _theme: &dyn ComponentTheme,
    steps: &[&str],
    current: usize,
    show_labels: bool,
    circle_d: f32,
    label_font: FontId,
    num_font: FontId,
    line_thickness: f32,
    accent: Color32,
    dim: Color32,
    text: Color32,
    line_completed: Color32,
    line_future: Color32,
) -> Response {
    let n = steps.len();
    let row_h = circle_d + 8.0;
    let total_h = row_h * n as f32;
    let label_x_offset = circle_d + st::gap_sm();
    let avail_w = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(Vec2::new(avail_w, total_h), Sense::hover());

    if !ui.is_rect_visible(rect) { return response; }
    let painter = ui.painter_at(rect);
    let cx = rect.left() + circle_d * 0.5 + 2.0;

    // Connector lines between rows.
    for i in 0..n.saturating_sub(1) {
        let y0 = rect.top() + row_h * i as f32 + circle_d + 1.0;
        let y1 = rect.top() + row_h * (i + 1) as f32 - 1.0;
        let col = if i + 1 <= current { line_completed } else { line_future };
        painter.line_segment([Pos2::new(cx, y0), Pos2::new(cx, y1)], Stroke::new(line_thickness, col));
    }

    for (i, label) in steps.iter().enumerate() {
        let cy = rect.top() + row_h * i as f32 + circle_d * 0.5;
        let center = Pos2::new(cx, cy);
        paint_circle(&painter, center, circle_d, i, current, &num_font, accent, dim);
        if show_labels {
            let col = if i == current { text } else if i < current { text } else { dim };
            painter.text(
                Pos2::new(rect.left() + label_x_offset, cy),
                egui::Align2::LEFT_CENTER,
                label,
                label_font.clone(),
                col,
            );
        }
    }

    response
}

fn paint_circle(
    painter: &egui::Painter,
    center: Pos2,
    diameter: f32,
    idx: usize,
    current: usize,
    num_font: &FontId,
    accent: Color32,
    dim: Color32,
) {
    let r = diameter * 0.5;
    if idx < current {
        // Completed: filled accent, white check.
        painter.circle_filled(center, r, accent);
        painter.text(
            center,
            egui::Align2::CENTER_CENTER,
            Icon::CHECK,
            FontId::proportional(diameter * 0.55),
            Color32::WHITE,
        );
    } else if idx == current {
        // Current: filled accent, white number, slightly larger ring.
        painter.circle_filled(center, r, accent);
        painter.circle_stroke(center, r + 1.5, Stroke::new(1.5, st::color_alpha(accent, 120)));
        painter.text(
            center,
            egui::Align2::CENTER_CENTER,
            format!("{}", idx + 1),
            num_font.clone(),
            Color32::WHITE,
        );
    } else {
        // Future: transparent fill, dim border, dim number.
        painter.circle_stroke(center, r, Stroke::new(1.0, dim));
        painter.text(
            center,
            egui::Align2::CENTER_CENTER,
            format!("{}", idx + 1),
            num_font.clone(),
            dim,
        );
    }
}

impl<'a> Widget for Stepper<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        let theme = &crate::chart_renderer::gpu::THEMES[0];
        self.show(ui, theme)
    }
}
