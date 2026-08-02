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

use egui::{Color32, Pos2, Rect, Response, Stroke, Ui, Vec2};

use super::motion;
use super::placement::{compute as compute_placement, Placement, Side};
use super::theme::ComponentTheme;
use crate::ui_kit::sx::{palette_ct, Tone};
use super::PolishedLabel;
use super::tokens::Size as KitSize;

use crate::ui_kit::tokens::{
    alpha_line, alpha_strong, color_alpha, elevate, gap_sm, gap_xs, radius_sm, stroke_thin,
    ELEVATE_RAISED,
};

const DEFAULT_DELAY_MS: u64 = (motion::DELAY_TOOLTIP * 1000.0) as u64;
const MAX_WIDTH: f32 = 280.0;

type RichFn<'a> = Box<dyn FnOnce(&mut Ui, &dyn ComponentTheme) + 'a>;

enum Content<'a> {
    Text(String),
    Rich(RichFn<'a>),
}

#[must_use = "Widget does nothing until `.show(ui, theme)` or `ui.add(widget)` is called"]
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

        // elevation_2: tooltip is a mid-tier overlay (elevate(bg, ELEVATE_RAISED)).
        // Direction-aware: dark bg → lighter, light bg → darker (was bg × 0.88,
        // which collapsed on near-black themes). ComponentTheme::bg() is
        // sufficient; no concrete &Theme needed.
        let bg = elevate(palette_ct(theme).base(Tone::Bg), ELEVATE_RAISED);
        let border = color_alpha(palette_ct(theme).base(Tone::Border), alpha_line());
        let fg = palette_ct(theme).base(Tone::Text);

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
                    super::ShadowSpec::sm_themed(theme).color({
                        let s = theme.shadow_color();
                        Color32::from_rgba_unmultiplied(s.r(), s.g(), s.b(), 48)
                    }),
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
                        color: {
                            let s = theme.shadow_color();
                            Color32::from_rgba_unmultiplied(s.r(), s.g(), s.b(), 60)
                        },
                    });
                frame.show(ui, |ui| {
                    ui.set_max_width(MAX_WIDTH);
                    match self.content {
                        Content::Text(s) => {
                            PolishedLabel::new(s)
                                .size(KitSize::Sm)
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
    use crate::ui_kit::tokens::{
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
            {
                let s = theme.shadow_color();
                egui::Color32::from_rgba_unmultiplied(s.r(), s.g(), s.b(), shadow_alpha())
            },
        );
    }

    // Surface fill — elevation_2 (elevate(bg, ELEVATE_RAISED)) at near-solid
    // alpha so the chart bleeds through faintly behind text, matching the
    // previous 240-alpha fidelity while applying the correct depth tier.
    let surf = elevate(palette_ct(theme).base(Tone::Bg), ELEVATE_RAISED);
    painter.rect_filled(
        rect,
        cr,
        egui::Color32::from_rgba_unmultiplied(surf.r(), surf.g(), surf.b(), 240),
    );

    // Top bevel — only when corners are visible (Meridien / Octave have
    // cr_u8 == 0 and skip this). Color depends on theme luminance: light
    // themes get a darker bevel, dark themes a faint white highlight.
    if cr_u8 > 0 {
        let dark_theme = contrast_fg(palette_ct(theme).base(Tone::Bg)) == egui::Color32::WHITE;
        let bevel_alpha = if dark_theme { 8 } else { 30 };
        painter.rect_filled(
            egui::Rect::from_min_max(rect.min, egui::pos2(rect.right(), rect.top() + 1.0)),
            egui::CornerRadius { nw: cr_u8, ne: cr_u8, sw: 0, se: 0 },
            egui::Color32::from_rgba_unmultiplied(255, 255, 255, bevel_alpha),
        );
    }

    // Outer border — single source for stroke width and alpha.
    let stroke_w = if st.hairline_borders { st.stroke_std } else { stroke_thin() };
    let border_col = crate::ui_kit::tokens::color_alpha(palette_ct(theme).base(Tone::Border), alpha_line());
    painter.rect_stroke(
        rect,
        cr,
        egui::Stroke::new(stroke_w, border_col),
        egui::StrokeKind::Outside,
    );
}

// ─── PainterTooltip — reusable multi-line chart-canvas tooltip ────────────────
//
// Chart-canvas tooltips (OHLC readout, measure overlay, event-marker callouts)
// are positioned by absolute pixel coordinates and have no `egui::Response` to
// anchor a flow-mode `Tooltip` to. `PainterTooltip` gives them the same chrome
// as `paint_tooltip_card` plus consistent token-driven multi-line text layout,
// so chart tooltips visually match the rest of the application.
//
// Each line is `(text, color)`. A line whose text is exactly `"---"` renders
// as a hairline separator rule instead of text.
//
// Performance: pure paint. `measure()` is arithmetic only. `paint()` issues
// one `paint_tooltip_card` (2-4 ops) plus one draw per line — identical to the
// inline loops these call sites used before. No allocation, no per-frame state.

/// Multi-line painter-mode tooltip for chart-canvas overlays.
pub struct PainterTooltip<'a> {
    lines: &'a [(String, Color32)],
    width: f32,
}

impl<'a> PainterTooltip<'a> {
    /// Default content width when the caller doesn't override it.
    pub const DEFAULT_WIDTH: f32 = 170.0;

    pub fn new(lines: &'a [(String, Color32)]) -> Self {
        Self { lines, width: Self::DEFAULT_WIDTH }
    }

    /// Override the fixed content width.
    pub fn width(mut self, w: f32) -> Self {
        self.width = w;
        self
    }

    /// Per-line height — `gap_md` token.
    fn line_h() -> f32 {
        crate::ui_kit::tokens::gap_md()
    }

    /// Vertical padding inside the card — `gap_xs` token.
    fn pad_v() -> f32 {
        crate::ui_kit::tokens::gap_xs()
    }

    /// Left text inset (reads tighter than `gap_sm`).
    fn pad_label() -> f32 {
        6.0
    }

    /// Measured outer size of the tooltip card for the current line set.
    /// Use this to position the card (e.g. flip left when it would overflow
    /// the chart's right edge) before calling `paint`.
    pub fn measure(&self) -> Vec2 {
        let h = self.lines.len() as f32 * Self::line_h() + Self::pad_v() * 2.0;
        Vec2::new(self.width, h)
    }

    /// Paint the tooltip with its top-left at `top_left`. Returns the painted
    /// rect so callers can chain further overlay work if needed.
    pub fn paint(
        &self,
        painter: &egui::Painter,
        top_left: Pos2,
        theme: &dyn ComponentTheme,
    ) -> Rect {
        use crate::ui_kit::tokens::{
            alpha_tint, color_alpha, font_sm, stroke_thin,
        };
        let rect = Rect::from_min_size(top_left, self.measure());
        paint_tooltip_card(painter, rect, theme);

        let line_h = Self::line_h();
        let pad_v = Self::pad_v();
        let pad_label = Self::pad_label();
        let font = egui::FontId::monospace(font_sm());
        let sep_color = color_alpha(palette_ct(theme).base(Tone::Text), alpha_tint());

        for (i, (line, col)) in self.lines.iter().enumerate() {
            let cy = rect.top() + pad_v + i as f32 * line_h + line_h / 2.0;
            if line == "---" {
                painter.line_segment(
                    [
                        egui::pos2(rect.left() + pad_v, cy),
                        egui::pos2(rect.right() - pad_v, cy),
                    ],
                    Stroke::new(stroke_thin(), sep_color),
                );
            } else {
                painter.text(
                    egui::pos2(rect.left() + pad_label, cy),
                    egui::Align2::LEFT_CENTER,
                    line,
                    font.clone(),
                    *col,
                );
            }
        }
        rect
    }
}
