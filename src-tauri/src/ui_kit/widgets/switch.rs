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
use crate::chart::renderer::ui::style as st;

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
        paint_switch(ui, theme, self)
    }
}

impl<'a> Widget for Switch<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        let theme = &crate::chart_renderer::gpu::THEMES[0];
        self.show(ui, theme)
    }
}

fn track_dims(size: Size) -> (f32, f32) {
    match size {
        Size::Sm => (26.0, 14.0),
        _ => (32.0, 18.0),
    }
}

fn paint_switch(ui: &mut Ui, theme: &dyn ComponentTheme, sw: Switch<'_>) -> Response {
    let Switch { value, label, size, disabled } = sw;
    let (tw, th) = track_dims(size);
    let font_size = size.font_size();
    let gap = st::gap_xs();

    // Measure label.
    let label_w = if let Some(s) = &label {
        let galley = ui.fonts(|f| {
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
    let off_color = st::color_alpha(theme.dim(), 64);
    let on_color = theme.accent();
    let mut track_color = motion::lerp_color(off_color, on_color, on_t);

    // Thumb position animation.
    let pad = 2.0;
    let thumb_d = th - 2.0 * pad;
    let x_off = track_rect.left() + pad + thumb_d * 0.5;
    let x_on = track_rect.right() - pad - thumb_d * 0.5;
    let target_x = if on { x_on } else { x_off };
    let thumb_x = motion::ease_value(ui.ctx(), id.with("sw_thumb"), target_x, motion::FAST);
    let thumb_center = Pos2::new(thumb_x, track_rect.center().y);

    let mut thumb_color = Color32::WHITE;

    if disabled {
        track_color = with_alpha_scale(track_color, 0.5);
        thumb_color = with_alpha_scale(thumb_color, 0.5);
    }

    let painter = ui.painter_at(rect);
    let cr = CornerRadius::same((th * 0.5) as u8);
    painter.rect_filled(track_rect, cr, track_color);
    painter.circle_filled(thumb_center, thumb_d * 0.5, thumb_color);

    // Label.
    if let Some(s) = label {
        let lx = track_rect.right() + gap;
        let ly = rect.center().y;
        let mut text_color = theme.text();
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

    response
}

#[inline]
fn with_alpha_scale(c: Color32, s: f32) -> Color32 {
    Color32::from_rgba_premultiplied(
        c.r(), c.g(), c.b(),
        ((c.a() as f32) * s.clamp(0.0, 1.0)).round() as u8,
    )
}
