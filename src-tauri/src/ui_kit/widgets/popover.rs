//! Popover — click-toggled overlay anchored to a trigger element.
//! Used for: dropdowns, settings popups, color pickers, filter menus.
//!
//! Differences from Tooltip:
//!   - Click to open (not hover).
//!   - Click outside or press Escape to close.
//!   - Owns its own open/closed state via &mut bool.
//!   - Larger / richer content; padding gap_sm.

#![allow(dead_code)]

use egui::{Color32, Id, Key, Rect, Stroke, Ui, Vec2};

use super::motion;
use super::placement::{compute as compute_placement, Placement, Side};
use super::theme::ComponentTheme;

use crate::chart_renderer::ui::style::{
    alpha_line, color_alpha, gap_sm, radius_sm, stroke_thin,
};

pub struct Popover<'a> {
    open: Option<&'a mut bool>,
    anchor: Option<Rect>,
    placement: Placement,
    modal: bool,
    close_on_click_outside: bool,
    id: Option<Id>,
}

impl<'a> Popover<'a> {
    pub fn new() -> Self {
        Self {
            open: None,
            anchor: None,
            placement: Placement {
                side: Side::Bottom,
                ..Default::default()
            },
            modal: false,
            close_on_click_outside: true,
            id: None,
        }
    }

    pub fn open(mut self, state: &'a mut bool) -> Self {
        self.open = Some(state);
        self
    }

    pub fn anchor(mut self, rect: Rect) -> Self {
        self.anchor = Some(rect);
        self
    }

    pub fn placement(mut self, p: Placement) -> Self {
        self.placement = p;
        self
    }

    pub fn modal(mut self, v: bool) -> Self {
        self.modal = v;
        self
    }

    pub fn close_on_click_outside(mut self, v: bool) -> Self {
        self.close_on_click_outside = v;
        self
    }

    pub fn id(mut self, id: impl std::hash::Hash) -> Self {
        self.id = Some(Id::new(id));
        self
    }

    pub fn show<R>(
        self,
        ui: &mut Ui,
        theme: &dyn ComponentTheme,
        add_contents: impl FnOnce(&mut Ui) -> R,
    ) -> Option<R> {
        let open_state = self.open.expect("Popover::show requires .open(&mut bool)");
        if !*open_state {
            return None;
        }
        let anchor = self.anchor.expect("Popover::show requires .anchor(rect)");
        let ctx = ui.ctx().clone();

        let id = self
            .id
            .unwrap_or_else(|| Id::new(("apex_popover", anchor.min.x as i32, anchor.min.y as i32)));

        // Animation: scale + alpha (mirrors Modal motion::MED).
        let appear_t = motion::ease_bool(&ctx, id.with("anim"), true, motion::FAST);

        let bg = theme.surface();
        let border = color_alpha(theme.border(), alpha_line());

        // Position based on prior frame's measured size.
        let size_id = id.with("size");
        let prior_size: Vec2 = ctx
            .memory(|m| m.data.get_temp(size_id))
            .unwrap_or(Vec2::new(180.0, 80.0));
        let screen = ctx.screen_rect();
        let (top_left, _side) =
            compute_placement(anchor, prior_size, self.placement, screen);

        let mut popup_rect = Rect::NOTHING;
        let mut result: Option<R> = None;

        let area_resp = egui::Area::new(id)
            .order(egui::Order::Foreground)
            .fixed_pos(top_left)
            .show(&ctx, |ui| {
                ui.set_opacity(appear_t);
                // ui.set_opacity above naturally fades the shadow with appear_t.
                let shadow_rect = Rect::from_min_size(top_left, prior_size);
                super::paint_shadow(
                    ui.painter(),
                    shadow_rect,
                    super::ShadowSpec::md(),
                );
                let frame = egui::Frame::popup(ui.style())
                    .fill(bg)
                    .stroke(Stroke::new(stroke_thin(), border))
                    .corner_radius(radius_sm() + 3.0)
                    .inner_margin(egui::Margin::same(gap_sm() as i8))
                    .shadow(egui::epaint::Shadow {
                        offset: [0, 4],
                        blur: 16,
                        spread: 1,
                        color: Color32::from_black_alpha(80),
                    });
                let inner = frame.show(ui, |ui| add_contents(ui));
                result = Some(inner.inner);
            });

        popup_rect = area_resp.response.rect;
        let measured = popup_rect.size();
        if measured.x > 0.0 && measured.y > 0.0 {
            ctx.memory_mut(|m| m.data.insert_temp(size_id, measured));
        }

        // Escape to close.
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            *open_state = false;
        }

        // Click outside to close.
        if self.close_on_click_outside && *open_state {
            if ctx.input(|i| i.pointer.any_pressed()) {
                if let Some(p) = ctx.input(|i| i.pointer.interact_pos()) {
                    if !popup_rect.contains(p) && !anchor.contains(p) {
                        *open_state = false;
                    }
                }
            }
        }

        result
    }
}

impl<'a> Default for Popover<'a> {
    fn default() -> Self {
        Self::new()
    }
}
