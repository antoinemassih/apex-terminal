//! Resizable — split-pane container with a draggable divider.
//!
//! Caller supplies two render callbacks (left/right or top/bottom).
//! Widget owns the divider state and exposes the current split fraction.
//!
//! API:
//!   let mut split: f32 = 0.3;   // left pane = 30% width
//!   Resizable::horizontal(&mut split)
//!     .min_left(150.0)
//!     .min_right(200.0)
//!     .show(ui, theme,
//!       |ui| { /* left pane */ },
//!       |ui| { /* right pane */ });

use egui::{Color32, CornerRadius, CursorIcon, Rect, Response, Sense, Ui, Vec2};

use super::motion;
use super::theme::ComponentTheme;
use crate::chart::renderer::ui::style as st;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Orient {
    Horizontal,
    Vertical,
}

pub struct Resizable<'a> {
    split: &'a mut f32,
    orient: Orient,
    min_a: f32,
    min_b: f32,
    divider: f32,
    snap: f32,
}

impl<'a> Resizable<'a> {
    pub fn horizontal(split: &'a mut f32) -> Self {
        Self {
            split,
            orient: Orient::Horizontal,
            min_a: 80.0,
            min_b: 80.0,
            divider: 4.0,
            snap: 0.0,
        }
    }

    pub fn vertical(split: &'a mut f32) -> Self {
        Self {
            split,
            orient: Orient::Vertical,
            min_a: 80.0,
            min_b: 80.0,
            divider: 4.0,
            snap: 0.0,
        }
    }

    pub fn min_left(mut self, px: f32) -> Self { self.min_a = px; self }
    pub fn min_right(mut self, px: f32) -> Self { self.min_b = px; self }
    pub fn divider_width(mut self, px: f32) -> Self { self.divider = px; self }
    pub fn snap_threshold(mut self, px: f32) -> Self { self.snap = px; self }

    pub fn show<L, R>(
        self,
        ui: &mut Ui,
        theme: &dyn ComponentTheme,
        add_left: L,
        add_right: R,
    ) -> Response
    where
        L: FnOnce(&mut Ui),
        R: FnOnce(&mut Ui),
    {
        let Self { split, orient, min_a, min_b, divider, snap } = self;

        let avail = ui.available_size_before_wrap();
        let total_w = if avail.x.is_finite() && avail.x > 0.0 { avail.x } else { 200.0 };
        let total_h = if avail.y.is_finite() && avail.y > 0.0 { avail.y } else { 200.0 };

        let total_along = match orient {
            Orient::Horizontal => total_w,
            Orient::Vertical => total_h,
        };

        let outer_id = ui.make_persistent_id(("ui_kit_resizable", match orient {
            Orient::Horizontal => "h",
            Orient::Vertical => "v",
        }));

        // Compute split in pixels.
        let frac = split.clamp(0.0, 1.0);
        let mut split_px = frac * total_along;

        // Apply snap-to-collapse: if frac is 0 or 1, leave the pane fully collapsed.
        let collapsed_left = frac <= 0.0001;
        let collapsed_right = frac >= 0.9999;

        if !collapsed_left && !collapsed_right {
            // Clamp by mins.
            let min = min_a;
            let max = total_along - min_b - divider;
            if max > min {
                split_px = split_px.clamp(min, max);
            }
        } else if collapsed_left {
            split_px = 0.0;
        } else {
            split_px = total_along;
        }

        // Allocate the full container.
        let (rect, response) = ui.allocate_exact_size(
            Vec2::new(total_w, total_h),
            Sense::hover(),
        );

        // Compute divider rect and pane rects.
        let (rect_a, rect_div, rect_b) = match orient {
            Orient::Horizontal => {
                let div_x0 = rect.left() + split_px - divider * 0.5;
                let div_x1 = div_x0 + divider;
                let a = Rect::from_min_max(rect.min, egui::Pos2::new(div_x0, rect.bottom()));
                let d = Rect::from_min_max(
                    egui::Pos2::new(div_x0, rect.top()),
                    egui::Pos2::new(div_x1, rect.bottom()),
                );
                let b = Rect::from_min_max(egui::Pos2::new(div_x1, rect.top()), rect.max);
                (a, d, b)
            }
            Orient::Vertical => {
                let div_y0 = rect.top() + split_px - divider * 0.5;
                let div_y1 = div_y0 + divider;
                let a = Rect::from_min_max(rect.min, egui::Pos2::new(rect.right(), div_y0));
                let d = Rect::from_min_max(
                    egui::Pos2::new(rect.left(), div_y0),
                    egui::Pos2::new(rect.right(), div_y1),
                );
                let b = Rect::from_min_max(egui::Pos2::new(rect.left(), div_y1), rect.max);
                (a, d, b)
            }
        };

        // Divider interaction. Expand hit-area slightly for ease of grabbing.
        let hit_pad = 2.0;
        let hit_rect = match orient {
            Orient::Horizontal => rect_div.expand2(Vec2::new(hit_pad, 0.0)),
            Orient::Vertical => rect_div.expand2(Vec2::new(0.0, hit_pad)),
        };
        let div_resp = ui.interact(hit_rect, outer_id.with("div"), Sense::click_and_drag());

        if div_resp.hovered() || div_resp.dragged() {
            ui.ctx().set_cursor_icon(match orient {
                Orient::Horizontal => CursorIcon::ResizeHorizontal,
                Orient::Vertical => CursorIcon::ResizeVertical,
            });
        }

        // Drag updates.
        if div_resp.dragged() {
            if let Some(p) = ui.ctx().pointer_latest_pos() {
                let new_px = match orient {
                    Orient::Horizontal => p.x - rect.left(),
                    Orient::Vertical => p.y - rect.top(),
                };
                let mut new_frac = (new_px / total_along).clamp(0.0, 1.0);

                // Snap-to-collapse near edges.
                if snap > 0.0 {
                    let min_thresh = (min_a - snap) / total_along;
                    let max_thresh = (total_along - min_b + snap) / total_along;
                    if new_frac < min_thresh.max(0.0) {
                        new_frac = 0.0;
                    } else if new_frac > max_thresh.min(1.0) {
                        new_frac = 1.0;
                    } else {
                        // Clamp to mins inside the valid range.
                        let min_f = min_a / total_along;
                        let max_f = (total_along - min_b - divider) / total_along;
                        if max_f > min_f {
                            new_frac = new_frac.clamp(min_f, max_f);
                        }
                    }
                } else {
                    let min_f = min_a / total_along;
                    let max_f = (total_along - min_b - divider) / total_along;
                    if max_f > min_f {
                        new_frac = new_frac.clamp(min_f, max_f);
                    }
                }

                *split = new_frac;
            }
        }

        // Paint divider.
        let active_t = motion::ease_bool(
            ui.ctx(),
            outer_id.with("div_act"),
            div_resp.hovered() || div_resp.dragged(),
            motion::FAST,
        );
        let idle = st::color_alpha(theme.border(), st::ALPHA_STRONG);
        let active = st::color_alpha(theme.accent(), st::ALPHA_HEAVY);
        let div_color = motion::lerp_color(idle, active, active_t);
        ui.painter().rect_filled(rect_div, CornerRadius::ZERO, div_color);

        // Render panes via child UIs constrained to their rects.
        if rect_a.width() > 0.5 && rect_a.height() > 0.5 && !collapsed_left {
            let mut child = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(rect_a)
                    .layout(egui::Layout::top_down(egui::Align::Min)),
            );
            child.set_clip_rect(rect_a);
            add_left(&mut child);
        } else {
            // Mark closure as consumed.
            let _ = add_left;
        }

        if rect_b.width() > 0.5 && rect_b.height() > 0.5 && !collapsed_right {
            let mut child = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(rect_b)
                    .layout(egui::Layout::top_down(egui::Align::Min)),
            );
            child.set_clip_rect(rect_b);
            add_right(&mut child);
        } else {
            let _ = add_right;
        }

        // Suppress unused on Color32 (transparent never used here).
        let _ = Color32::TRANSPARENT;

        response
    }
}
