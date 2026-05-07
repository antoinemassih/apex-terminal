//! Checkbox — boolean or tri-state selection.
//!
//! Different from Switch: implies "I will apply this later" (batch
//! selection in lists, settings forms with a Save button). Switch is
//! immediate.
//!
//! Tri-state: Indeterminate represents partial selection in lists.
//!
//! API:
//!   let mut selected = true;
//!   ui.add(Checkbox::new(&mut selected).label("Allow shorts"));
//!
//!   ui.add(Checkbox::tri(&mut tri_state).label("Select all"));

use egui::{Color32, CornerRadius, FontId, Pos2, Response, Sense, Stroke, StrokeKind, Ui, Vec2, Widget};

use super::motion;
use super::theme::ComponentTheme;
use super::tokens::Size;
use crate::chart::renderer::ui::style as st;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CheckState {
    #[default]
    Off,
    On,
    Indeterminate,
}

enum CbValue<'a> {
    Bool(&'a mut bool),
    Tri(&'a mut CheckState),
}

#[must_use = "Checkbox does nothing until `.show(ui, theme)` or `ui.add(checkbox)` is called"]
pub struct Checkbox<'a> {
    value: CbValue<'a>,
    label: Option<String>,
    size: Size,
    disabled: bool,
}

impl<'a> Checkbox<'a> {
    pub fn new(value: &'a mut bool) -> Self {
        Self { value: CbValue::Bool(value), label: None, size: Size::Md, disabled: false }
    }

    pub fn tri(value: &'a mut CheckState) -> Self {
        Self { value: CbValue::Tri(value), label: None, size: Size::Md, disabled: false }
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
        paint_checkbox(ui, theme, self)
    }
}

impl<'a> Widget for Checkbox<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        let theme = &crate::chart_renderer::gpu::THEMES[0];
        self.show(ui, theme)
    }
}

fn box_size(size: Size) -> f32 {
    match size {
        Size::Sm => 14.0,
        _ => 16.0,
    }
}

fn current_state(v: &CbValue<'_>) -> CheckState {
    match v {
        CbValue::Bool(b) => if **b { CheckState::On } else { CheckState::Off },
        CbValue::Tri(s) => **s,
    }
}

fn cycle(v: &mut CbValue<'_>) {
    match v {
        CbValue::Bool(b) => { **b = !**b; }
        CbValue::Tri(s) => {
            **s = match **s {
                CheckState::Off => CheckState::On,
                CheckState::On => CheckState::Indeterminate,
                CheckState::Indeterminate => CheckState::Off,
            };
        }
    }
}

fn paint_checkbox(ui: &mut Ui, theme: &dyn ComponentTheme, mut cb: Checkbox<'_>) -> Response {
    let bs = box_size(cb.size);
    let font_size = cb.size.font_size();
    let gap = st::gap_xs();

    let label_w = if let Some(s) = &cb.label {
        let galley = ui.fonts(|f| {
            f.layout_no_wrap(s.clone(), FontId::proportional(font_size), Color32::WHITE)
        });
        galley.rect.width() + gap
    } else { 0.0 };

    let total_w = bs + label_w;
    let total_h = bs.max(font_size + 2.0);

    let sense = if cb.disabled { Sense::hover() } else { Sense::click() };
    let (rect, mut response) = ui.allocate_exact_size(Vec2::new(total_w, total_h), sense);

    if response.clicked() && !cb.disabled {
        cycle(&mut cb.value);
        response.mark_changed();
    }

    if !ui.is_rect_visible(rect) {
        return response;
    }

    let id = response.id;
    let state = current_state(&cb.value);
    let is_marked = !matches!(state, CheckState::Off);
    let hovered = response.hovered() && !cb.disabled;

    let hover_t = motion::ease_bool(ui.ctx(), id.with("cb_hover"), hovered, motion::FAST);
    let on_t = motion::ease_bool(ui.ctx(), id.with("cb_on"), is_marked, motion::FAST);

    // Box rect, vertically centered.
    let box_min = Pos2::new(rect.left(), rect.center().y - bs * 0.5);
    let box_rect = egui::Rect::from_min_size(box_min, Vec2::splat(bs));

    let accent = theme.accent();
    let border = theme.border();

    // Compute fill / border.
    let off_bg = Color32::TRANSPARENT;
    let on_bg = accent;
    let mut bg = motion::lerp_color(off_bg, on_bg, on_t);

    // Hover tint when off (additive ghost), brighter accent when on.
    if !is_marked && hover_t > 0.001 {
        let hover_bg = st::color_alpha(accent, st::ALPHA_GHOST);
        bg = motion::lerp_color(bg, hover_bg, hover_t);
    } else if is_marked && hover_t > 0.001 {
        bg = motion::lerp_color(bg, lighten(accent, 0.10), hover_t);
    }

    let border_col = motion::lerp_color(border, accent, on_t);

    let mut fg_mark = Color32::WHITE;
    let (mut bg_final, mut border_final) = (bg, border_col);

    if cb.disabled {
        bg_final = with_alpha_scale(bg_final, 0.5);
        border_final = with_alpha_scale(border_final, 0.5);
        fg_mark = with_alpha_scale(fg_mark, 0.5);
    }

    let painter = ui.painter_at(rect);
    let cr = CornerRadius::same(3);
    painter.rect_filled(box_rect, cr, bg_final);
    painter.rect_stroke(box_rect, cr, Stroke::new(1.0, border_final), StrokeKind::Inside);

    // Mark — only when state implies one.
    match state {
        CheckState::On => {
            // Hand-drawn checkmark: two line segments.
            let c = box_rect.center();
            let s = bs;
            let p1 = Pos2::new(c.x - s * 0.25, c.y + s * 0.02);
            let p2 = Pos2::new(c.x - s * 0.05, c.y + s * 0.20);
            let p3 = Pos2::new(c.x + s * 0.28, c.y - s * 0.18);
            let stroke = Stroke::new(1.6, fg_mark);
            painter.line_segment([p1, p2], stroke);
            painter.line_segment([p2, p3], stroke);
        }
        CheckState::Indeterminate => {
            let c = box_rect.center();
            let s = bs;
            let p1 = Pos2::new(c.x - s * 0.28, c.y);
            let p2 = Pos2::new(c.x + s * 0.28, c.y);
            painter.line_segment([p1, p2], Stroke::new(1.8, fg_mark));
        }
        CheckState::Off => {}
    }

    // Label.
    if let Some(s) = cb.label {
        let lx = box_rect.right() + gap;
        let ly = rect.center().y;
        let mut text_color = theme.text();
        if cb.disabled { text_color = with_alpha_scale(text_color, 0.5); }
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
