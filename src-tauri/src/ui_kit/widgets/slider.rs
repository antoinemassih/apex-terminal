//! Slider — themed wrapper around egui::Slider for visual consistency.
//!
//! Adds: range presets, step snapping, value formatting, themed track + thumb,
//! optional tick marks, color-coded variants (accent/bull/bear).
//!
//! API:
//!   let mut value = 50.0;
//!   ui.add(Slider::new(&mut value, 0.0..=100.0));
//!
//!   Slider::new(&mut qty, 1.0..=1000.0)
//!     .step(1.0)
//!     .ticks(&[100.0, 250.0, 500.0])
//!     .show_value(true)
//!     .label("Qty")
//!     .show(ui, theme);

use egui::{Color32, CornerRadius, Pos2, Response, Sense, Stroke, Ui, Vec2, Widget};

use super::motion;
use super::theme::ComponentTheme;
use super::tokens::{Size, Variant};
use crate::chart::renderer::ui::style as st;

#[must_use = "Slider does nothing until `.show(ui, theme)` or `ui.add(slider)` is called"]
pub struct Slider<'a, T: egui::emath::Numeric> {
    value: &'a mut T,
    range: std::ops::RangeInclusive<T>,
    step: Option<f64>,
    ticks: &'a [f64],
    show_value: bool,
    label: Option<String>,
    size: Size,
    variant: Variant,
    full_width: bool,
}

impl<'a, T: egui::emath::Numeric> Slider<'a, T> {
    pub fn new(value: &'a mut T, range: std::ops::RangeInclusive<T>) -> Self {
        Self {
            value,
            range,
            step: None,
            ticks: &[],
            show_value: false,
            label: None,
            size: Size::Md,
            variant: Variant::Primary,
            full_width: false,
        }
    }

    pub fn step(mut self, step: f64) -> Self { self.step = Some(step); self }
    pub fn ticks(mut self, ticks: &'a [f64]) -> Self { self.ticks = ticks; self }
    pub fn show_value(mut self, v: bool) -> Self { self.show_value = v; self }
    pub fn label(mut self, text: impl Into<String>) -> Self { self.label = Some(text.into()); self }
    pub fn size(mut self, s: Size) -> Self { self.size = s; self }
    pub fn variant(mut self, v: Variant) -> Self { self.variant = v; self }
    pub fn full_width(mut self) -> Self { self.full_width = true; self }

    pub fn show(self, ui: &mut Ui, theme: &dyn ComponentTheme) -> Response {
        paint_slider(ui, theme, self)
    }
}

impl<'a, T: egui::emath::Numeric> Widget for Slider<'a, T> {
    fn ui(self, ui: &mut Ui) -> Response {
        let theme = &crate::chart_renderer::gpu::THEMES[0];
        self.show(ui, theme)
    }
}

fn variant_color(variant: Variant, theme: &dyn ComponentTheme) -> Color32 {
    match variant {
        Variant::Primary => theme.accent(),
        Variant::Danger => theme.bear(),
        Variant::Secondary | Variant::Ghost | Variant::Link => theme.accent(),
    }
}

fn paint_slider<T: egui::emath::Numeric>(
    ui: &mut Ui,
    theme: &dyn ComponentTheme,
    s: Slider<'_, T>,
) -> Response {
    let Slider {
        value, range, step, ticks, show_value, label, size, variant, full_width,
    } = s;

    let track_h = match size { Size::Sm | Size::Xs => 4.0, _ => 6.0 };
    let thumb_d = match size { Size::Sm | Size::Xs => 14.0, _ => 18.0 };
    let hover_extra = 2.0;
    let total_h = thumb_d + hover_extra + 2.0;
    let label_font = st::font_xs();

    // Vertical layout: optional label on top, then [track row] + value to the right.
    let mut full_resp: Option<Response> = None;
    ui.vertical(|ui| {
        if let Some(text) = &label {
            ui.painter().text(
                ui.cursor().min,
                egui::Align2::LEFT_TOP,
                text,
                egui::FontId::proportional(label_font),
                st::color_alpha(theme.text(), 180),
            );
            // Allocate the label space.
            let galley = ui.fonts(|f| f.layout_no_wrap(
                text.clone(), egui::FontId::proportional(label_font), Color32::WHITE));
            ui.allocate_exact_size(Vec2::new(galley.rect.width(), galley.rect.height() + 2.0), Sense::hover());
        }

        ui.horizontal(|ui| {
            // Compute available width.
            let value_label_w = if show_value { 50.0 } else { 0.0 };
            let avail = ui.available_width();
            let track_w = if full_width || avail < 240.0 {
                (avail - value_label_w - st::gap_xs()).max(80.0)
            } else {
                200.0
            };

            let row_size = Vec2::new(track_w, total_h);
            let (rect, mut response) = ui.allocate_exact_size(row_size, Sense::click_and_drag());
            let id = response.id;

            // Track rect (centered vertically in row).
            let track_y = rect.center().y;
            let track_rect = egui::Rect::from_min_size(
                Pos2::new(rect.left() + thumb_d * 0.5, track_y - track_h * 0.5),
                Vec2::new(rect.width() - thumb_d, track_h),
            );

            let r_min = range.start().to_f64();
            let r_max = range.end().to_f64();
            let r_span = (r_max - r_min).max(f64::EPSILON);

            let cur = value.to_f64().clamp(r_min, r_max);

            // Drag/click handling.
            let mut new_val = cur;
            if response.dragged() || response.clicked() {
                if let Some(ptr) = response.interact_pointer_pos() {
                    let t = ((ptr.x - track_rect.left()) / track_rect.width().max(1.0)) as f64;
                    let t = t.clamp(0.0, 1.0);
                    new_val = r_min + t * r_span;
                    if let Some(stp) = step {
                        if stp > 0.0 {
                            new_val = r_min + ((new_val - r_min) / stp).round() * stp;
                        }
                    }
                    new_val = new_val.clamp(r_min, r_max);
                    if (new_val - cur).abs() > f64::EPSILON {
                        *value = T::from_f64(new_val);
                        response.mark_changed();
                    }
                }
            }

            let cur_norm = ((new_val - r_min) / r_span).clamp(0.0, 1.0) as f32;
            let thumb_x = track_rect.left() + cur_norm * track_rect.width();
            let thumb_center = Pos2::new(thumb_x, track_y);

            let painter = ui.painter_at(rect);

            // Track background.
            let track_bg = st::color_alpha(theme.dim(), 64);
            let cr = CornerRadius::same((track_h * 0.5) as u8);
            painter.rect_filled(track_rect, cr, track_bg);

            // Filled portion.
            let fill_color = variant_color(variant, theme);
            let filled = egui::Rect::from_min_max(
                track_rect.min,
                Pos2::new(thumb_x, track_rect.max.y),
            );
            painter.rect_filled(filled, cr, fill_color);

            // Tick marks.
            for &t in ticks.iter() {
                let tnorm = ((t - r_min) / r_span).clamp(0.0, 1.0) as f32;
                let tx = track_rect.left() + tnorm * track_rect.width();
                let ty = track_rect.bottom() + 2.0;
                painter.line_segment(
                    [Pos2::new(tx, ty), Pos2::new(tx, ty + 4.0)],
                    Stroke::new(1.0, st::color_alpha(theme.dim(), 100)),
                );
            }

            // Thumb (scale on hover/drag).
            let active = response.hovered() || response.dragged();
            let scale_t = motion::ease_bool(ui.ctx(), id.with("sl_hov"), active, motion::FAST);
            let d = thumb_d + scale_t * hover_extra;
            painter.circle_filled(thumb_center, d * 0.5, theme.bg());
            painter.circle_stroke(thumb_center, d * 0.5, Stroke::new(2.0, fill_color));

            if response.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }

            // Value label to the right.
            if show_value {
                let formatted = format_value(new_val, step);
                let painter = ui.painter();
                painter.text(
                    Pos2::new(rect.right() + st::gap_xs(), rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    formatted,
                    if size == Size::Sm || size == Size::Xs { st::mono_xs() } else { st::mono_sm() },
                    theme.text(),
                );
                ui.allocate_exact_size(Vec2::new(value_label_w, total_h), Sense::hover());
            }

            full_resp = Some(response);
        });
    });

    full_resp.unwrap_or_else(|| ui.allocate_response(Vec2::ZERO, Sense::hover()))
}

fn format_value(v: f64, step: Option<f64>) -> String {
    let step = step.unwrap_or(0.0);
    if step >= 1.0 || (step == 0.0 && (v - v.round()).abs() < 1e-9) {
        format!("{}", v.round() as i64)
    } else if step >= 0.1 || step == 0.0 {
        format!("{:.2}", v)
    } else {
        format!("{:.3}", v)
    }
}
