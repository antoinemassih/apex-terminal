//! Switch — toggle control, like iOS-style or shadcn Switch.
//!
//! Different from Checkbox in semantics: Switch implies an immediate
//! state change (settings toggle, "Show drafts"); Checkbox implies
//! batch selection ("Apply to all selected orders").
//!
//! Style: rounded-full track + circular thumb. Thumb slides on toggle.
//! Track fills with accent when on.
//!
//! API:
//!   let mut enabled = true;
//!   ui.add(Switch::new(&mut enabled).label("Outside RTH"));

use egui::{Color32, CornerRadius, FontId, Pos2, Response, Sense, Ui, Vec2, Widget};

use super::motion;
use super::theme::ComponentTheme;
use super::tokens::Size;
use crate::ui_kit::tokens as st;
use crate::ui_kit::sx::{palette_ct, Tone};

#[must_use = "Switch does nothing until `.show(ui, theme)` or `ui.add(switch)` is called"]
pub struct Switch<'a> {
    value: &'a mut bool,
    label: Option<String>,
    size: Size,
    disabled: bool,
}

impl<'a> Switch<'a> {
    pub fn new(value: &'a mut bool) -> Self {
        Self { value, label: None, size: Size::Md, disabled: false }
    }

    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Sm or Md only. Xs/Lg fall back to Md.
    pub fn size(mut self, s: Size) -> Self {
        self.size = match s {
            Size::Sm => Size::Sm,
            _ => Size::Md,
        };
        self
    }

    pub fn disabled(mut self, v: bool) -> Self { self.disabled = v; self }

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
        paint_switch(ui, theme, self)
    }
}

impl<'a> Widget for Switch<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        let theme = super::theme::active_theme(ui.ctx());
        self.show(ui, &theme)
    }
}

/// Returns (track_width, track_height, thumb_diameter).
/// Zed-exact: Md = 32×20 with 12px thumb (4px vertical inset).
fn track_dims(size: Size) -> (f32, f32, f32) {
    match size {
        Size::Sm => (26.0, 14.0, 10.0),
        _ => (32.0, 20.0, 12.0),
    }
}

fn paint_switch(ui: &mut Ui, theme: &dyn ComponentTheme, sw: Switch<'_>) -> Response {
    let Switch { value, label, size, disabled } = sw;
    let (tw, th, thumb_d) = track_dims(size);
    let font_size = size.font_size();
    let gap = st::gap_xs();

    // Measure label.
    let label_w = if let Some(s) = &label {
        let galley = ui.fonts(|f| {
            // layout-only: color is discarded, we only read `.rect.width()` below.
            f.layout_no_wrap(s.clone(), FontId::proportional(font_size), Color32::WHITE)
        });
        galley.rect.width() + gap
    } else {
        0.0
    };

    let total_w = tw + label_w;
    let total_h = th.max(font_size + 2.0);

    let sense = if disabled { Sense::hover() } else { Sense::click() };
    let (rect, mut response) = ui.allocate_exact_size(Vec2::new(total_w, total_h), sense);

    if response.clicked() && !disabled {
        *value = !*value;
        response.mark_changed();
    }

    if !ui.is_rect_visible(rect) {
        return response;
    }

    let id = response.id;
    let on = *value;

    // Track rect, vertically centered.
    let track_min = Pos2::new(rect.left(), rect.center().y - th * 0.5);
    let track_rect = egui::Rect::from_min_size(track_min, Vec2::new(tw, th));

    // Animate track color (off -> on).
    let on_t = motion::ease_bool(ui.ctx(), id.with("sw_on"), on, motion::FAST);
    let pal = palette_ct(theme);
    let off_color = st::color_alpha(pal.base(Tone::Dim), 64);
    let on_color = pal.base(Tone::Accent);
    let mut track_color = motion::lerp_color(off_color, on_color, on_t);

    // Thumb position — animated with ease_out_back for a satisfying
    // "snap past and settle" feel (slight overshoot then return).
    // 4px inset from each edge: thumb left edge at 4 (off) or tw - 4 - thumb_d (on).
    let pad = 4.0;
    let x_off = track_rect.left() + pad + thumb_d * 0.5;
    let x_on = track_rect.right() - pad - thumb_d * 0.5;
    // Raw linear progress 0.0 (off) → 1.0 (on), driven by egui's animator.
    // Curve: ease_out_back — overshoots ~5% then settles, matches iOS toggle feel.
    let raw_t = motion::ease_value(ui.ctx(), id.with("sw_thumb"), if on { 1.0 } else { 0.0 }, motion::FAST);
    let eased_t = motion::ease_out_back(raw_t);
    let thumb_x = x_off + (x_on - x_off) * eased_t;
    let thumb_center = Pos2::new(thumb_x, track_rect.center().y);

    let mut thumb_color = st::contrast_fg(on_color);

    if disabled {
        track_color = with_alpha_scale(track_color, 0.5);
        thumb_color = with_alpha_scale(thumb_color, 0.5);
    }

    let painter = ui.painter_at(rect);
    // `switch` key — the track. Thumb colour stays with the widget (it encodes
    // on/off), and the default radius is a true pill (half the track height).
    let (cr, tr_fill, _) = super::theme::resolve_control_chrome(
        ui.ctx(), theme, "switch", th * 0.5, track_color, track_color, 0.0,
    );
    painter.rect_filled(track_rect, cr, tr_fill);
    painter.circle_filled(thumb_center, thumb_d * 0.5, thumb_color);

    // Label.
    if let Some(s) = label {
        let lx = track_rect.right() + gap;
        let ly = rect.center().y;
        let mut text_color = pal.base(Tone::Text);
        if disabled { text_color = with_alpha_scale(text_color, 0.5); }
        painter.text(
            Pos2::new(lx, ly),
            egui::Align2::LEFT_CENTER,
            s,
            FontId::proportional(font_size),
            text_color,
        );
    }

    if response.hovered() && !disabled {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    st::cursor::focus_ring(ui, &response, pal.base(Tone::Accent));

    response
}

#[inline]
fn with_alpha_scale(c: Color32, s: f32) -> Color32 {
    Color32::from_rgba_premultiplied(
        c.r(), c.g(), c.b(),
        ((c.a() as f32) * s.clamp(0.0, 1.0)).round() as u8,
    )
}
