//! Radio — single-selection within a group.
//!
//! API:
//!   let mut selected_tif: usize = 0;
//!   ui.add(Radio::new(&mut selected_tif, 0).label("DAY"));
//!   ui.add(Radio::new(&mut selected_tif, 1).label("GTC"));
//!   ui.add(Radio::new(&mut selected_tif, 2).label("IOC"));

use egui::{Color32, FontId, Pos2, Response, Sense, Stroke, Ui, Vec2, Widget};

use super::motion;
use super::theme::ComponentTheme;
use super::tokens::Size;
use crate::chart::renderer::ui::style as st;

#[must_use = "Radio does nothing until `.show(ui, theme)` or `ui.add(radio)` is called"]
pub struct Radio<'a, T: PartialEq + Copy> {
    group: &'a mut T,
    this: T,
    label: Option<String>,
    size: Size,
    disabled: bool,
}

impl<'a, T: PartialEq + Copy> Radio<'a, T> {
    pub fn new(group: &'a mut T, this: T) -> Self {
        Self { group, this, label: None, size: Size::Md, disabled: false }
    }

    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Sm or Md only.
    pub fn size(mut self, s: Size) -> Self {
        self.size = match s {
            Size::Sm => Size::Sm,
            _ => Size::Md,
        };
        self
    }

    pub fn disabled(mut self, v: bool) -> Self { self.disabled = v; self }

    pub fn show(self, ui: &mut Ui, theme: &dyn ComponentTheme) -> Response {
        paint_radio(ui, theme, self)
    }
}

impl<'a, T: PartialEq + Copy + 'a> Widget for Radio<'a, T> {
    fn ui(self, ui: &mut Ui) -> Response {
        let theme = &crate::chart_renderer::gpu::THEMES[0];
        self.show(ui, theme)
    }
}

fn diameter(size: Size) -> f32 {
    match size {
        Size::Sm => 14.0,
        _ => 16.0,
    }
}

fn paint_radio<T: PartialEq + Copy>(
    ui: &mut Ui,
    theme: &dyn ComponentTheme,
    r: Radio<'_, T>,
) -> Response {
    let d = diameter(r.size);
    let font_size = r.size.font_size();
    let gap = st::gap_xs();

    let label_w = if let Some(s) = &r.label {
        let galley = ui.fonts(|f| {
            f.layout_no_wrap(s.clone(), FontId::proportional(font_size), Color32::WHITE)
        });
        galley.rect.width() + gap
    } else { 0.0 };

    let total_w = d + label_w;
    let total_h = d.max(font_size + 2.0);

    let sense = if r.disabled { Sense::hover() } else { Sense::click() };
    let (rect, mut response) = ui.allocate_exact_size(Vec2::new(total_w, total_h), sense);

    let selected = *r.group == r.this;

    if response.clicked() && !r.disabled {
        *r.group = r.this;
        response.mark_changed();
    }

    if !ui.is_rect_visible(rect) {
        return response;
    }

    let id = response.id;
    let hovered = response.hovered() && !r.disabled;

    let hover_t = motion::ease_bool(ui.ctx(), id.with("rd_hover"), hovered, motion::FAST);
    let on_t = motion::ease_bool(ui.ctx(), id.with("rd_on"), selected, motion::FAST);

    let center = Pos2::new(rect.left() + d * 0.5, rect.center().y);
    let radius = d * 0.5;

    let accent = theme.accent();
    let border = theme.border();

    // Background fill (transparent off → accent on).
    let off_bg = Color32::TRANSPARENT;
    let on_bg = accent;
    let mut bg = motion::lerp_color(off_bg, on_bg, on_t);

    if !selected && hover_t > 0.001 {
        let hover_bg = st::color_alpha(accent, st::ALPHA_GHOST);
        bg = motion::lerp_color(bg, hover_bg, hover_t);
    } else if selected && hover_t > 0.001 {
        bg = motion::lerp_color(bg, lighten(accent, 0.10), hover_t);
    }

    let border_col = motion::lerp_color(border, accent, on_t);

    let mut bg_final = bg;
    let mut border_final = border_col;
    let mut dot_color = Color32::WHITE;

    if r.disabled {
        bg_final = with_alpha_scale(bg_final, 0.5);
        border_final = with_alpha_scale(border_final, 0.5);
        dot_color = with_alpha_scale(dot_color, 0.5);
    }

    let painter = ui.painter_at(rect);
    painter.circle_filled(center, radius, bg_final);
    painter.circle_stroke(center, radius, Stroke::new(1.0, border_final));

    // Inner dot — diameter = outer - 6px → radius - 3.
    if on_t > 0.001 {
        let inner_r = ((d - 6.0) * 0.5).max(1.0);
        let dot = Color32::from_rgba_premultiplied(
            dot_color.r(), dot_color.g(), dot_color.b(),
            ((dot_color.a() as f32) * on_t).round() as u8,
        );
        painter.circle_filled(center, inner_r, dot);
    }

    // Label.
    if let Some(s) = r.label {
        let lx = center.x + radius + gap;
        let ly = rect.center().y;
        let mut text_color = theme.text();
        if r.disabled { text_color = with_alpha_scale(text_color, 0.5); }
        painter.text(
            Pos2::new(lx, ly),
            egui::Align2::LEFT_CENTER,
            s,
            FontId::proportional(font_size),
            text_color,
        );
    }

    if hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    response
}

#[inline]
fn lighten(c: Color32, amt: f32) -> Color32 {
    let lerp = |x: u8| -> u8 {
        let v = x as f32 + (255.0 - x as f32) * amt.clamp(0.0, 1.0);
        v.round().clamp(0.0, 255.0) as u8
    };
    Color32::from_rgba_premultiplied(lerp(c.r()), lerp(c.g()), lerp(c.b()), c.a())
}

#[inline]
fn with_alpha_scale(c: Color32, s: f32) -> Color32 {
    Color32::from_rgba_premultiplied(
        c.r(), c.g(), c.b(),
        ((c.a() as f32) * s.clamp(0.0, 1.0)).round() as u8,
    )
}
