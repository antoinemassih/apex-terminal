//! HoverCard — rich content card that appears on hover (not click).
//! Halfway between Tooltip and Popover.
//!
//! Use case: hover over a symbol → show a card with mini chart + key
//! stats. Hover over a user avatar → show profile preview.

#![allow(dead_code)]

use egui::{Color32, Id, Rect, Response, Stroke, Ui, Vec2};

use super::motion;
use super::placement::{compute as compute_placement, Placement, Side};
use super::theme::ComponentTheme;

use crate::chart_renderer::ui::style::{
    alpha_line, color_alpha, gap_sm, radius_sm, stroke_thin,
};

const DEFAULT_DELAY_MS: u64 = 600;

pub struct HoverCard {
    delay_ms: u64,
    placement: Placement,
}

impl HoverCard {
    pub fn new() -> Self {
        Self {
            delay_ms: DEFAULT_DELAY_MS,
            placement: Placement {
                side: Side::Bottom,
                ..Default::default()
            },
        }
    }

    pub fn delay_ms(mut self, ms: u64) -> Self {
        self.delay_ms = ms;
        self
    }

    pub fn placement(mut self, p: Placement) -> Self {
        self.placement = p;
        self
    }

    pub fn show(
        self,
        ui: &mut Ui,
        response: &Response,
        theme: &dyn ComponentTheme,
        add_contents: impl FnOnce(&mut Ui),
    ) -> bool {
        let ctx = ui.ctx().clone();
        let id = response.id.with("apex_hover_card");
        let hover_start_id = id.with("hover_start");
        let rect_id = id.with("rect");

        let now = ctx.input(|i| i.time);
        let pointer = ctx.input(|i| i.pointer.interact_pos());

        // Determine if pointer is over either trigger or previously-shown card.
        let prior_card_rect: Option<Rect> = ctx.memory(|m| m.data.get_temp(rect_id));
        let over_trigger = response.hovered();
        let over_card = match (pointer, prior_card_rect) {
            (Some(p), Some(r)) => r.contains(p),
            _ => false,
        };
        let hovered = over_trigger || over_card;

        let hover_start: Option<f64> = ctx.memory(|m| m.data.get_temp(hover_start_id));
        let hover_start = if hovered {
            match hover_start {
                Some(t) => Some(t),
                None => {
                    ctx.memory_mut(|m| m.data.insert_temp(hover_start_id, now));
                    Some(now)
                }
            }
        } else {
            if hover_start.is_some() {
                ctx.memory_mut(|m| m.data.remove::<f64>(hover_start_id));
            }
            // Clear stored card rect so it doesn't keep us "hovered" forever.
            if prior_card_rect.is_some() {
                ctx.memory_mut(|m| m.data.remove::<Rect>(rect_id));
            }
            None
        };

        let elapsed_ms = hover_start
            .map(|t| ((now - t) * 1000.0) as u64)
            .unwrap_or(0);
        let visible = hovered && elapsed_ms >= self.delay_ms;
        if !visible {
            return false;
        }

        ctx.request_repaint();

        let appear_t = motion::ease_bool(&ctx, id.with("anim"), true, motion::FAST);

        let bg = theme.surface();
        let border = color_alpha(theme.border(), alpha_line());

        let size_id = id.with("size");
        let prior_size: Vec2 = ctx
            .memory(|m| m.data.get_temp(size_id))
            .unwrap_or(Vec2::new(220.0, 120.0));
        let screen = ctx.screen_rect();
        let (top_left, _side) =
            compute_placement(response.rect, prior_size, self.placement, screen);

        let area_resp = egui::Area::new(id)
            .order(egui::Order::Foreground)
            .fixed_pos(top_left)
            .show(&ctx, |ui| {
                ui.set_opacity(appear_t);
                let frame = egui::Frame::popup(ui.style())
                    .fill(bg)
                    .stroke(Stroke::new(stroke_thin(), border))
                    .corner_radius(radius_sm() + 3.0)
                    .inner_margin(egui::Margin::same(gap_sm() as i8))
                    .shadow(egui::epaint::Shadow {
                        offset: [0, 4],
                        blur: 16,
                        spread: 1,
                        color: Color32::from_black_alpha(70),
                    });
                frame.show(ui, |ui| add_contents(ui));
            });

        let card_rect = area_resp.response.rect;
        let measured = card_rect.size();
        if measured.x > 0.0 && measured.y > 0.0 {
            ctx.memory_mut(|m| m.data.insert_temp(size_id, measured));
            ctx.memory_mut(|m| m.data.insert_temp(rect_id, card_rect));
        }
        let _ = Color32::TRANSPARENT;
        true
    }
}

impl Default for HoverCard {
    fn default() -> Self {
        Self::new()
    }
}
