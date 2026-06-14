//! Horizontal and vertical hairline dividers.

use super::super::style::*;
use egui::{self, Color32, Pos2, Sense, Stroke, Ui, Vec2};

/// Snap a 1px-ish divider to the physical pixel grid so it renders crisp at
/// fractional DPI (HiDPI) instead of blurring across two rows. Returns the
/// snapped centerline coordinate and an integer-physical stroke width: odd
/// physical width → centered on a half-pixel, even → on an integer pixel.
/// UI-chrome only — never used for the chart GPU pipeline.
#[inline]
pub fn crisp(coord: f32, w_logical: f32, ppp: f32) -> (f32, f32) {
    let w_phys = (w_logical * ppp).round().max(1.0);
    let half = if (w_phys as i32) & 1 == 1 { 0.5 } else { 0.0 };
    let snapped = ((coord * ppp).round() + half) / ppp;
    (snapped, w_phys / ppp)
}

/// Horizontal hairline — width matches available width.
pub fn hairline(ui: &mut Ui, color: Color32) {
    let st = current();
    let rect = ui.available_rect_before_wrap();
    let ppp = ui.ctx().pixels_per_point();
    let (y, w) = crisp(ui.cursor().min.y, st.stroke_std, ppp);
    ui.painter().line_segment(
        [Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
        Stroke::new(w, color),
    );
    ui.add_space(gap_xs());
}

/// Vertical hairline divider — for inline horizontal layouts.
pub fn v_hairline(ui: &mut Ui, color: Color32, height: f32) {
    let st = current();
    let (rect, _resp) = ui.allocate_exact_size(Vec2::new(1.0, height), Sense::hover());
    let ppp = ui.ctx().pixels_per_point();
    let (x, w) = crisp(rect.center().x, st.stroke_std, ppp);
    ui.painter().line_segment(
        [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
        Stroke::new(w, color),
    );
}
