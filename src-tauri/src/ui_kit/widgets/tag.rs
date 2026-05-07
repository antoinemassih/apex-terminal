//! Tag — colored label for categories, status, filters. Like a chip
//! but non-interactive by default. Closable variant for filter chips.
//!
//! Tones map to the 6-color palette; opacity supplies hierarchy.
//!
//! API:
//!   ui.add(Tag::new("Filled").tone(TagTone::Bull));
//!   ui.add(Tag::new("Day").tone(TagTone::Neutral).size(Size::Xs));
//!   let r = Tag::new("Tech").closable(true).show(ui, theme);
//!   if r.closed { /* remove */ }

use egui::{Color32, CornerRadius, FontId, Pos2, Response, Sense, Stroke, StrokeKind, Ui, Vec2, Widget};

use super::theme::ComponentTheme;
use super::tokens::Size;
use crate::chart::renderer::ui::style as st;
use crate::ui_kit::icons::Icon;

/// Tone palette for Tag/Badge — each tone maps to one color in the
/// project's 6-color palette (accent / bull / bear / warn / dim/text-on-surface).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TagTone {
    /// Theme dim — neutral chip on surface.
    #[default]
    Neutral,
    /// Theme accent — informational / selected.
    Accent,
    /// Theme bull — positive / filled / long.
    Bull,
    /// Theme bear — negative / rejected / short.
    Bear,
    /// Theme warn — caution / pending.
    Warn,
}

impl TagTone {
    pub fn color(&self, theme: &dyn ComponentTheme) -> Color32 {
        match self {
            TagTone::Neutral => theme.dim(),
            TagTone::Accent => theme.accent(),
            TagTone::Bull => theme.bull(),
            TagTone::Bear => theme.bear(),
            TagTone::Warn => theme.warn(),
        }
    }
}

#[must_use = "Tag does nothing until `.show(ui, theme)` or `ui.add(tag)` is called"]
pub struct Tag<'a> {
    label: String,
    tone: TagTone,
    size: Size,
    closable: bool,
    dot: bool,
    outline: bool,
    _lt: std::marker::PhantomData<&'a ()>,
}

pub struct TagResponse {
    pub response: Response,
    pub closed: bool,
}

impl<'a> Tag<'a> {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            tone: TagTone::Neutral,
            size: Size::Sm,
            closable: false,
            dot: false,
            outline: false,
            _lt: std::marker::PhantomData,
        }
    }

    pub fn tone(mut self, t: TagTone) -> Self { self.tone = t; self }
    /// Tag size: Xs/Sm only. Md/Lg are clamped to Sm — they're too chunky for chips.
    pub fn size(mut self, s: Size) -> Self {
        self.size = match s {
            Size::Xs => Size::Xs,
            _ => Size::Sm,
        };
        self
    }
    pub fn closable(mut self, v: bool) -> Self { self.closable = v; self }
    pub fn dot(mut self, v: bool) -> Self { self.dot = v; self }
    pub fn outline(mut self, v: bool) -> Self { self.outline = v; self }

    pub fn show(self, ui: &mut Ui, theme: &dyn ComponentTheme) -> TagResponse {
        let tone_col = self.tone.color(theme);
        let font_size = match self.size { Size::Xs => st::font_xs() - 1.0, _ => st::font_xs() };
        let pad_x = st::gap_xs();
        let pad_y: f32 = 2.0;
        let icon_gap = st::gap_2xs();

        // Measure label.
        let galley = ui.fonts(|f| {
            f.layout_no_wrap(self.label.clone(), FontId::proportional(font_size), tone_col)
        });
        let label_w = galley.rect.width();
        let label_h = galley.rect.height();

        let dot_size: f32 = 6.0;
        let close_size: f32 = 8.0;

        let mut content_w = label_w;
        if self.dot { content_w += dot_size + icon_gap; }
        if self.closable { content_w += icon_gap + close_size; }

        let h = (label_h + pad_y * 2.0).max(match self.size { Size::Xs => 14.0, _ => 16.0 });
        let w = content_w + pad_x * 2.0;
        let desired = Vec2::new(w, h);
        let (rect, response) = ui.allocate_exact_size(desired, Sense::click());

        let mut closed = false;

        if ui.is_rect_visible(rect) {
            let painter = ui.painter_at(rect);
            let radius = (h * 0.5) as u8;
            let cr = CornerRadius::same(radius);

            if self.outline {
                painter.rect_stroke(rect, cr, Stroke::new(1.0, tone_col), StrokeKind::Inside);
            } else {
                let bg = st::color_alpha(tone_col, 32);
                painter.rect_filled(rect, cr, bg);
            }

            let cy = rect.center().y;
            let mut x = rect.left() + pad_x;

            if self.dot {
                let center = Pos2::new(x + dot_size * 0.5, cy);
                painter.circle_filled(center, dot_size * 0.5, tone_col);
                x += dot_size + icon_gap;
            }

            painter.text(
                Pos2::new(x, cy),
                egui::Align2::LEFT_CENTER,
                &self.label,
                FontId::proportional(font_size),
                tone_col,
            );
            x += label_w;

            if self.closable {
                x += icon_gap;
                let close_center = Pos2::new(x + close_size * 0.5, cy);
                let close_rect = egui::Rect::from_center_size(close_center, Vec2::splat(close_size + 4.0));
                let close_resp = ui.interact(close_rect, response.id.with("close"), Sense::click());
                let col = if close_resp.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    st::color_alpha(tone_col, 255)
                } else {
                    st::color_alpha(tone_col, 200)
                };
                painter.text(
                    close_center,
                    egui::Align2::CENTER_CENTER,
                    Icon::X,
                    FontId::proportional(close_size),
                    col,
                );
                if close_resp.clicked() { closed = true; }
            }
        }

        TagResponse { response, closed }
    }
}

impl<'a> Widget for Tag<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        let theme = &crate::chart_renderer::gpu::THEMES[0];
        self.show(ui, theme).response
    }
}
