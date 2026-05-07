//! Tooltip — hover-triggered, delayed, rich-content overlay.
//!
//! API:
//! ```ignore
//!   Tooltip::new("Buy market order")
//!       .delay_ms(400)
//!       .placement(Placement { side: Side::Top, ..Default::default() })
//!       .show(ui, &response, theme);
//!
//!   // Rich content:
//!   Tooltip::rich(|ui, theme| { /* paint */ })
//!       .show(ui, &response, theme);
//! ```
//!
//! Default delay: 400ms. Fade-in: motion::FAST. Disappears immediately on
//! hover-out (no fade-out — feels snappier).

#![allow(dead_code)]

use egui::{Color32, Id, Pos2, Rect, Response, Stroke, Ui, Vec2};

use super::motion;
use super::placement::{compute as compute_placement, Placement, Side};
use super::theme::ComponentTheme;
use super::PolishedLabel;
use super::tokens::Size as KitSize;

use crate::chart_renderer::ui::style::{
    alpha_line, alpha_strong, color_alpha, gap_sm, gap_xs, radius_sm, stroke_thin,
};

const DEFAULT_DELAY_MS: u64 = 400;
const MAX_WIDTH: f32 = 280.0;

type RichFn<'a> = Box<dyn FnOnce(&mut Ui, &dyn ComponentTheme) + 'a>;

enum Content<'a> {
    Text(String),
    Rich(RichFn<'a>),
}

pub struct Tooltip<'a> {
    content: Content<'a>,
    delay_ms: u64,
    placement: Placement,
}

impl<'a> Tooltip<'a> {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            content: Content::Text(text.into()),
            delay_ms: DEFAULT_DELAY_MS,
            placement: Placement {
                side: Side::Top,
                ..Default::default()
            },
        }
    }

    pub fn rich(content: impl FnOnce(&mut Ui, &dyn ComponentTheme) + 'a) -> Self {
        Self {
            content: Content::Rich(Box::new(content)),
            delay_ms: DEFAULT_DELAY_MS,
            placement: Placement {
                side: Side::Top,
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

    pub fn instant(mut self) -> Self {
        self.delay_ms = 0;
        self
    }

    pub fn show(self, ui: &mut Ui, response: &Response, theme: &dyn ComponentTheme) {
        let ctx = ui.ctx().clone();
        let id = response.id.with("apex_tooltip");
        let hover_start_id = id.with("hover_start");

        // Track hover-start time in memory.
        let now = ctx.input(|i| i.time);
        let hovered = response.hovered();

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
            None
        };

        let elapsed_ms = hover_start
            .map(|t| ((now - t) * 1000.0) as u64)
            .unwrap_or(0);

        let visible = hovered && elapsed_ms >= self.delay_ms;
        if !visible {
            return;
        }

        // Request continuous repaint while waiting / animating in.
        ctx.request_repaint();

        let appear_t = motion::ease_bool(&ctx, id.with("anim"), true, motion::FAST);

        let bg = theme.surface();
        let border = color_alpha(theme.border(), alpha_line());
        let fg = theme.text();

        // Pre-compute estimated size by laying the content into a probe Area
        // off-screen — but for simplicity, position via Area + compute on the
        // post-frame rect; egui Areas accept fixed_pos based on prior frame.
        let placed_id = id.with("rect");
        let prior_size: Vec2 = ctx
            .memory(|m| m.data.get_temp(placed_id))
            .unwrap_or(Vec2::new(80.0, 24.0));

        let screen = ctx.screen_rect();
        let (top_left, _side) =
            compute_placement(response.rect, prior_size, self.placement, screen);

        let area_resp = egui::Area::new(id)
            .order(egui::Order::Tooltip)
            .interactable(false)
            .fixed_pos(top_left)
            .show(&ctx, |ui| {
                ui.set_opacity(appear_t);
                // Drop shadow behind the panel — uses the prior-frame
                // measured size so position matches what we're about to paint.
                // ui.set_opacity above naturally fades the shadow with appear_t.
                let shadow_rect = Rect::from_min_size(top_left, prior_size);
                super::paint_shadow_gpu(
                    ui.painter(),
                    shadow_rect,
                    super::ShadowSpec::sm().color(Color32::from_black_alpha(48)),
                );
                let frame = egui::Frame::popup(ui.style())
                    .fill(bg)
                    .stroke(Stroke::new(stroke_thin(), border))
                    .corner_radius(radius_sm())
                    .inner_margin(egui::Margin::symmetric(gap_sm() as i8, gap_xs() as i8))
                    .shadow(egui::epaint::Shadow {
                        offset: [0, 2],
                        blur: 8,
                        spread: 0,
                        color: Color32::from_black_alpha(60),
                    });
                frame.show(ui, |ui| {
                    ui.set_max_width(MAX_WIDTH);
                    match self.content {
                        Content::Text(s) => {
                            PolishedLabel::new(s)
                                .size(KitSize::Xs)
                                .color(fg)
                                .show(ui, theme);
                        }
                        Content::Rich(f) => {
                            f(ui, theme);
                        }
                    }
                });
            });

        // Persist measured size for next frame.
        let measured = area_resp.response.rect.size();
        if measured.x.is_finite() && measured.y.is_finite() && measured.x > 0.0 {
            ctx.memory_mut(|m| m.data.insert_temp(placed_id, measured));
        }

        // Suppress unused warnings for borrowed values.
        let _ = (Pos2::ZERO, Rect::NOTHING, alpha_strong());
    }
}

// ─── Painter-mode tooltip chrome ─────────────────────────────────────────────
//
// `paint_tooltip_card` is a *paint-only* helper for absolute-positioned
// tooltips that don't have an `egui::Response` to anchor to (chart-canvas
// crosshair tooltips, measure overlays, painter-mode bubbles). It paints the
// same chrome (shadow + bg + top bevel + hairline border) as the flow-mode
// `Tooltip` widget, sourcing every alpha / radius / stroke from the active
// `StyleSettings` so the visual stays in lockstep with the rest of the kit.
//
// Pure paint — no allocation, no animation state, no per-frame compute beyond
// what each call site was already doing inline. Callers paint their text /
// content on top of the card afterward.
//
// Performance: one `style::current()` lookup, one `contrast_fg()` call, and
// 2-4 painter ops (shadow + bg + optional bevel + border). The crosshair
// site that previously inlined the same operations is net-equal — the helper
// removes ~12 LOC of inline arithmetic but doesn't add a single new draw call.

pub fn paint_tooltip_card(
    painter: &egui::Painter,
    rect: egui::Rect,
    theme: &dyn ComponentTheme,
) {
    use crate::chart_renderer::ui::style::{
        alpha_line, contrast_fg, current, shadow_alpha, shadow_offset, stroke_thin,
    };
    let st = current();
    let cr_u8 = st.r_md;
    let cr = egui::CornerRadius::same(cr_u8);

    // Drop shadow
    if st.shadows_enabled {
        painter.rect_filled(
            rect.translate(egui::vec2(0.0, shadow_offset())).expand(1.0),
            cr,
            egui::Color32::from_rgba_unmultiplied(0, 0, 0, shadow_alpha()),
        );
    }

    // Surface fill — theme.surface() at near-solid alpha so the chart bleeds
    // through faintly behind text, matching the previous 240-alpha fidelity.
    let surf = theme.surface();
    painter.rect_filled(
        rect,
        cr,
        egui::Color32::from_rgba_unmultiplied(surf.r(), surf.g(), surf.b(), 240),
    );

    // Top bevel — only when corners are visible (Meridien / Octave have
    // cr_u8 == 0 and skip this). Color depends on theme luminance: light
    // themes get a darker bevel, dark themes a faint white highlight.
    if cr_u8 > 0 {
        let dark_theme = contrast_fg(theme.bg()) == egui::Color32::WHITE;
        let bevel_alpha = if dark_theme { 8 } else { 30 };
        painter.rect_filled(
            egui::Rect::from_min_max(rect.min, egui::pos2(rect.right(), rect.top() + 1.0)),
            egui::CornerRadius { nw: cr_u8, ne: cr_u8, sw: 0, se: 0 },
            egui::Color32::from_rgba_unmultiplied(255, 255, 255, bevel_alpha),
        );
    }

    // Outer border — single source for stroke width and alpha.
    let stroke_w = if st.hairline_borders { st.stroke_std } else { stroke_thin() };
    let border_col = crate::chart_renderer::ui::style::color_alpha(theme.border(), alpha_line());
    painter.rect_stroke(
        rect,
        cr,
        egui::Stroke::new(stroke_w, border_col),
        egui::StrokeKind::Outside,
    );
}
