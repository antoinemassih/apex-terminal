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
use crate::ui_kit::sx::{palette_ct, Tone};
use super::tokens::{Size, Variant};
use crate::ui_kit::tokens as st;
use crate::ui_kit::text_style::TextStyle;

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
    disabled: bool,
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
            disabled: false,
        }
    }

    pub fn step(mut self, step: f64) -> Self { self.step = Some(step); self }
    pub fn ticks(mut self, ticks: &'a [f64]) -> Self { self.ticks = ticks; self }
    pub fn show_value(mut self, v: bool) -> Self { self.show_value = v; self }
    pub fn label(mut self, text: impl Into<String>) -> Self { self.label = Some(text.into()); self }
    pub fn size(mut self, s: Size) -> Self { self.size = s; self }
    pub fn variant(mut self, v: Variant) -> Self { self.variant = v; self }
    pub fn full_width(mut self) -> Self { self.full_width = true; self }
    pub fn disabled(mut self, d: bool) -> Self { self.disabled = d; self }

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
        paint_slider(ui, theme, self)
    }
}

impl<'a, T: egui::emath::Numeric> Widget for Slider<'a, T> {
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

fn paint_slider<T: egui::emath::Numeric>(
    ui: &mut Ui,
    theme: &dyn ComponentTheme,
    s: Slider<'_, T>,
) -> Response {
    let Slider {
        value, range, step, ticks, show_value, label, size, variant, full_width, disabled,
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
                crate::ui_kit::style::prop_at(label_font),
                st::color_alpha(palette_ct(theme).base(Tone::Text), crate::ui_kit::style::alpha_near_solid()),
            );
            // Allocate the label space.
            // layout-only: only `.rect.width()/.height()` is read.
            let galley = ui.fonts(|f| f.layout_no_wrap(
                text.clone(), crate::ui_kit::style::prop_at(label_font), egui::Color32::PLACEHOLDER));
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
            let sense = if disabled { Sense::hover() } else { Sense::click_and_drag() };
            let (rect, mut response) = ui.allocate_exact_size(row_size, sense);
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
            let dim_mul = if disabled { 0.5 } else { 1.0 };
            let track_bg = st::color_alpha(palette_ct(theme).base(Tone::Dim), 64).gamma_multiply(dim_mul);
            // `slider` key — the TRACK. Its own key rather than reusing
            // `progress`: both are capsule tracks, but a Slider is interactive
            // and carries a thumb, so a style may legitimately want to treat
            // the two differently. Keys are append-only, so this is the point
            // to make that call.
            let (cr, track_fill, _) = super::theme::resolve_control_chrome(
                ui.ctx(), theme, "slider",
                track_h * 0.5, track_bg, track_bg, 0.0,
            );
            painter.rect_filled(track_rect, cr, track_fill);

            // Filled portion.
            let fill_color = variant_color(variant, theme).gamma_multiply(dim_mul);
            let filled = egui::Rect::from_min_max(
                track_rect.min,
                Pos2::new(thumb_x, track_rect.max.y),
            );
            painter.rect_filled(filled, cr, fill_color);

            // Tick marks.
            for &t in ticks.iter() {
                let tnorm = ((t - r_min) / r_span).clamp(0.0, 1.0) as f32;
                let tx = track_rect.left() + tnorm * track_rect.width();
                let ty = track_rect.bottom() + crate::ui_kit::style::gap_2xs();
                painter.line_segment(
                    [Pos2::new(tx, ty), Pos2::new(tx, ty + 4.0)],
                    Stroke::new(st::stroke_std(), st::color_alpha(palette_ct(theme).base(Tone::Dim), crate::ui_kit::style::alpha_active())),
                );
            }

            // Thumb (scale on hover/drag; no animation when disabled).
            let active = !disabled && (response.hovered() || response.dragged());
            let scale_t = motion::ease_bool(ui.ctx(), id.with("sl_hov"), active, motion::FAST);
            let d = thumb_d + scale_t * hover_extra;
            let thumb_bg = palette_ct(theme).base(Tone::Bg).gamma_multiply(dim_mul);
            painter.circle_filled(thumb_center, d * 0.5, thumb_bg);
            painter.circle_stroke(thumb_center, d * 0.5, Stroke::new(st::stroke_thick(), fill_color));

            if !disabled && response.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }

            // Value label to the right.
            if show_value {
                let formatted = format_value(new_val, step, r_max - r_min);
                let painter = ui.painter();
                painter.text(
                    Pos2::new(rect.right() + st::gap_xs(), rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    formatted,
                    if size == Size::Sm || size == Size::Xs {
                        TextStyle::MonoXs.font_id_in(ui)
                    } else {
                        TextStyle::MonoSm.font_id_in(ui)
                    },
                    palette_ct(theme).base(Tone::Text),
                );
                ui.allocate_exact_size(Vec2::new(value_label_w, total_h), Sense::hover());
            }

            crate::ui_kit::tokens::cursor::focus_ring(ui, &response, crate::ui_kit::sx::palette_ct(theme).base(crate::ui_kit::sx::Tone::Accent));

            full_resp = Some(response);
        });
    });

    full_resp.unwrap_or_else(|| ui.allocate_response(Vec2::ZERO, Sense::hover()))
}

/// Format a slider's value with enough precision to be READABLE at that range.
///
/// `span` (range max - min) is required because a fixed decimal count cannot
/// serve every slider. This used to hard-code `{:.2}` whenever no `step` was
/// set, which is fine for `0.5..=6.0` and useless for `0.001..=0.02`: the
/// auto-chart `sensitivity` slider displayed its 0.003 value as **`0.00`**, so
/// the control showed the same text across most of its travel and the user
/// could not read what they had set.
///
/// Caught immediately after converting that panel to this widget — the
/// conversion fixed an alignment defect and introduced a precision one. Worth
/// naming: swapping a component in is not free, and "it looks aligned now" is
/// not the same as "it still says the right thing".
fn format_value(v: f64, step: Option<f64>, span: f64) -> String {
    let step = step.unwrap_or(0.0);
    // Integral values stay integral — `min touches`, `max lines`, `lookback`
    // must never render as `3.00`.
    if step >= 1.0 || (step == 0.0 && (v - v.round()).abs() < 1e-9) {
        return format!("{}", v.round() as i64);
    }
    let decimals = if step > 0.0 {
        // Enough places to show one step.
        (-step.log10().floor()).clamp(0.0, 6.0) as usize
    } else if span > 0.0 {
        // Two significant places relative to the span, so a slider always
        // resolves at least ~1% of its own travel.
        (2.0 - span.log10().floor()).clamp(2.0, 6.0) as usize
    } else {
        2
    };
    format!("{v:.decimals$}")
}

#[cfg(test)]
mod slider_format_tests {
    use super::format_value;

    /// A slider must never render the same text across most of its travel.
    ///
    /// The auto-chart `sensitivity` slider spans 0.001..=0.02. With the old
    /// fixed `{:.2}`, every value below 0.005 displayed as `0.00` — the
    /// control was unreadable for most of its range.
    #[test]
    fn narrow_range_keeps_enough_decimals() {
        let span = 0.02 - 0.001;
        let a = format_value(0.003, None, span);
        let b = format_value(0.006, None, span);
        assert_ne!(a, b, "0.003 and 0.006 rendered identically as {a:?}");
        assert!(a.starts_with("0.003"), "expected 0.003.., got {a:?}");
    }

    /// Integral sliders stay integral — `min touches` must not read `3.00`.
    #[test]
    fn integral_values_render_without_decimals() {
        assert_eq!(format_value(3.0, None, 4.0), "3");
        assert_eq!(format_value(200.0, None, 350.0), "200");
        assert_eq!(format_value(12.0, None, 26.0), "12");
    }

    /// A wide float range does not gain pointless precision.
    #[test]
    fn wide_range_stays_terse() {
        assert_eq!(format_value(2.35, None, 5.5), "2.35");
    }

    /// An explicit step drives precision directly.
    #[test]
    fn step_sets_precision() {
        assert_eq!(format_value(0.25, Some(0.05), 1.0), "0.25");
        assert_eq!(format_value(7.0, Some(1.0), 100.0), "7");
    }
}
