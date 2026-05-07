//! Horizontal and vertical hairline dividers.

use super::super::style::*;
use egui::{self, Color32, Pos2, Sense, Stroke, Ui, Vec2};

/// Horizontal hairline — width matches available width.
pub fn hairline(ui: &mut Ui, color: Color32) {
    let st = current();
    let rect = ui.available_rect_before_wrap();
    let y = ui.cursor().min.y;
    ui.painter().line_segment(
        [Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
        Stroke::new(st.stroke_std, color),
    );
    ui.add_space(4.0);
}

/// Vertical hairline divider — for inline horizontal layouts.
pub fn v_hairline(ui: &mut Ui, color: Color32, height: f32) {
    let st = current();
    let (rect, _resp) = ui.allocate_exact_size(Vec2::new(1.0, height), Sense::hover());
    ui.painter().line_segment(
        [
            Pos2::new(rect.center().x, rect.top()),
            Pos2::new(rect.center().x, rect.bottom()),
        ],
        Stroke::new(st.stroke_std, color),
    );
}
