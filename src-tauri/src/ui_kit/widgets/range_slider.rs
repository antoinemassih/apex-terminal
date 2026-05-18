//! RangeSlider — two-thumb slider for selecting a [min, max] sub-range.
//!
//! ```ignore
//! let mut range = (10.0_f32, 90.0_f32);
//! RangeSlider::new(&mut range, 0.0..=100.0)
//!     .step(1.0)
//!     .show_value(true)
//!     .show(ui, theme);
//! ```
//!
//! Mirrors the single-thumb [`Slider`] track + thumb dimensions and motion.
//! The filled segment between the two thumbs uses `theme.accent()` at
//! `alpha_active()`. On drag, the widget automatically picks whichever
//! thumb is closer to the pointer; the two thumbs cannot cross.

use egui::{Color32, CornerRadius, Pos2, Response, Sense, Stroke, Ui, Vec2};
use std::ops::RangeInclusive;

use super::motion;
use super::theme::ComponentTheme;
use super::tokens::Size;
use crate::chart::renderer::ui::style as st;

/// Two-thumb range slider. Call [`RangeSlider::show`] to render it.
#[must_use = "RangeSlider does nothing until `.show(ui, theme)` is called"]
pub struct RangeSlider<'a, T: egui::emath::Numeric> {
    value: &'a mut (T, T),
    range: RangeInclusive<T>,
    step: Option<f64>,
    show_value: bool,
    label: Option<&'a str>,
    size: Size,
    full_width: bool,
    disabled: bool,
}

impl<'a, T: egui::emath::Numeric> RangeSlider<'a, T> {
    /// Create a new RangeSlider bound to `value = (lo, hi)` within `range`.
    pub fn new(value: &'a mut (T, T), range: RangeInclusive<T>) -> Self {
        Self {
            value,
            range,
            step: None,
            show_value: false,
            label: None,
            size: Size::Md,
            full_width: false,
            disabled: false,
        }
    }

    /// Snap values to multiples of `s` from the range start.
    pub fn step(mut self, s: f64) -> Self { self.step = Some(s); self }
    /// Show `"{lo} – {hi}"` centered above the track.
    pub fn show_value(mut self, v: bool) -> Self { self.show_value = v; self }
    /// Optional text label rendered above the track.
    pub fn label(mut self, l: &'a str) -> Self { self.label = Some(l); self }
    /// Widget size tier.
    pub fn size(mut self, s: Size) -> Self { self.size = s; self }
    /// Expand the track to fill the available width.
    pub fn full_width(mut self) -> Self { self.full_width = true; self }
    /// Disable interaction + dim appearance.
    pub fn disabled(mut self, d: bool) -> Self { self.disabled = d; self }

    /// Render and return the combined response.
    pub fn show(self, ui: &mut Ui, theme: &dyn ComponentTheme) -> Response {
        paint_range_slider(ui, theme, self)
    }
}

// ── internal painter ──────────────────────────────────────────────────────────

fn snap(v: f64, r_min: f64, step: Option<f64>) -> f64 {
    match step {
        Some(s) if s > 0.0 => r_min + ((v - r_min) / s).round() * s,
        _ => v,
    }
}

fn paint_range_slider<T: egui::emath::Numeric>(
    ui: &mut Ui,
    theme: &dyn ComponentTheme,
    s: RangeSlider<'_, T>,
) -> Response {
    let RangeSlider { value, range, step, show_value, label, size, full_width, disabled } = s;

    let track_h  = match size { Size::Sm | Size::Xs => 4.0, _ => 6.0 };
    let thumb_d  = match size { Size::Sm | Size::Xs => 14.0, _ => 18.0 };
    let hover_ex = 2.0;
    let total_h  = thumb_d + hover_ex + 2.0;

    let mut full_resp: Option<Response> = None;

    ui.vertical(|ui| {
        // ── optional label ────────────────────────────────────────────────
        if let Some(text) = label {
            let galley = ui.fonts(|f| {
                f.layout_no_wrap(
                    text.to_string(),
                    egui::FontId::proportional(st::font_xs()),
                    Color32::WHITE, // layout-only: only `.rect.width()/.height()` is read
                )
            });
            let lbl_pos = ui.cursor().min;
            ui.painter().text(
                lbl_pos,
                egui::Align2::LEFT_TOP,
                text,
                egui::FontId::proportional(st::font_xs()),
                st::color_alpha(theme.text(), 180),
            );
            ui.allocate_exact_size(
                Vec2::new(galley.rect.width(), galley.rect.height() + 2.0),
                Sense::hover(),
            );
        }

        // ── optional value label ──────────────────────────────────────────
        if show_value {
            let lo_f = value.0.to_f64();
            let hi_f = value.1.to_f64();
            let text = format!("{} – {}", format_val(lo_f, step), format_val(hi_f, step));
            let lbl_pos = ui.cursor().min;
            let galley = ui.fonts(|f| {
                f.layout_no_wrap(
                    text.clone(),
                    egui::FontId::proportional(st::font_xs()),
                    Color32::WHITE, // layout-only: only `.rect.width()/.height()` is read
                )
            });
            ui.painter().text(
                lbl_pos,
                egui::Align2::LEFT_TOP,
                &text,
                egui::FontId::proportional(st::font_xs()),
                st::color_alpha(theme.text(), 180),
            );
            ui.allocate_exact_size(
                Vec2::new(galley.rect.width(), galley.rect.height() + 2.0),
                Sense::hover(),
            );
        }

        ui.horizontal(|ui| {
            let avail = ui.available_width();
            let track_w = if full_width || avail < 240.0 {
                avail.max(80.0)
            } else {
                200.0
            };

            let row_size = Vec2::new(track_w, total_h);
            let sense = if disabled { Sense::hover() } else { Sense::click_and_drag() };
            let (rect, mut response) = ui.allocate_exact_size(row_size, sense);

            let id_lo = response.id.with("rs_lo");
            let id_hi = response.id.with("rs_hi");

            let track_y = rect.center().y;
            let track_rect = egui::Rect::from_min_size(
                Pos2::new(rect.left() + thumb_d * 0.5, track_y - track_h * 0.5),
                Vec2::new(rect.width() - thumb_d, track_h),
            );

            let r_min = range.start().to_f64();
            let r_max = range.end().to_f64();
            let r_span = (r_max - r_min).max(f64::EPSILON);

            let mut lo = value.0.to_f64().clamp(r_min, r_max);
            let mut hi = value.1.to_f64().clamp(lo, r_max);

            // ── interaction ──────────────────────────────────────────────
            if !disabled && (response.dragged() || response.clicked()) {
                if let Some(ptr) = response.interact_pointer_pos() {
                    let t = ((ptr.x - track_rect.left()) / track_rect.width().max(1.0)) as f64;
                    let ptr_val = (r_min + t.clamp(0.0, 1.0) * r_span).clamp(r_min, r_max);

                    // pick the closer thumb
                    let dist_lo = (ptr_val - lo).abs();
                    let dist_hi = (ptr_val - hi).abs();
                    if dist_lo <= dist_hi {
                        lo = snap(ptr_val, r_min, step).clamp(r_min, hi);
                    } else {
                        hi = snap(ptr_val, r_min, step).clamp(lo, r_max);
                    }

                    let new_lo = T::from_f64(lo);
                    let new_hi = T::from_f64(hi);
                    if new_lo.to_f64() != value.0.to_f64()
                        || new_hi.to_f64() != value.1.to_f64()
                    {
                        value.0 = new_lo;
                        value.1 = new_hi;
                        response.mark_changed();
                    }
                }
            }

            // normalised positions
            let lo_n = ((lo - r_min) / r_span).clamp(0.0, 1.0) as f32;
            let hi_n = ((hi - r_min) / r_span).clamp(0.0, 1.0) as f32;
            let lo_x = track_rect.left() + lo_n * track_rect.width();
            let hi_x = track_rect.left() + hi_n * track_rect.width();

            let painter = ui.painter_at(rect);

            // ── track background ─────────────────────────────────────────
            let track_bg = st::color_alpha(theme.dim(), 64);
            let cr = CornerRadius::same((track_h * 0.5) as u8);
            painter.rect_filled(track_rect, cr, track_bg);

            // ── filled segment between thumbs ─────────────────────────────
            let fill_col = if disabled {
                st::color_alpha(theme.accent(), st::alpha_muted())
            } else {
                st::color_alpha(theme.accent(), st::alpha_active())
            };
            let fill_rect = egui::Rect::from_min_max(
                Pos2::new(lo_x, track_rect.min.y),
                Pos2::new(hi_x, track_rect.max.y),
            );
            painter.rect_filled(fill_rect, cr, fill_col);

            // ── thumbs ────────────────────────────────────────────────────
            let hov = response.hovered() || response.dragged();
            let sc_lo = motion::ease_bool(ui.ctx(), id_lo, hov, motion::FAST);
            let sc_hi = motion::ease_bool(ui.ctx(), id_hi, hov, motion::FAST);

            for (center_x, scale_t) in [(lo_x, sc_lo), (hi_x, sc_hi)] {
                let center = Pos2::new(center_x, track_y);
                let d = thumb_d + scale_t * hover_ex;
                painter.circle_filled(center, d * 0.5, theme.bg());
                painter.circle_stroke(
                    center,
                    d * 0.5,
                    Stroke::new(st::stroke_thick(), fill_col),
                );
            }

            if response.hovered() && !disabled {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }

            full_resp = Some(response);
        });
    });

    full_resp.unwrap_or_else(|| ui.allocate_response(Vec2::ZERO, Sense::hover()))
}

fn format_val(v: f64, step: Option<f64>) -> String {
    let s = step.unwrap_or(0.0);
    if s >= 1.0 || (s == 0.0 && (v - v.round()).abs() < 1e-9) {
        format!("{}", v.round() as i64)
    } else if s >= 0.1 || s == 0.0 {
        format!("{:.2}", v)
    } else {
        format!("{:.3}", v)
    }
}
