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
use crate::ui_kit::tokens as st;
use crate::ui_kit::sx::Sx;
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
    /// Map to the unified Sx [`Tone`](crate::ui_kit::sx::Tone) vocabulary so
    /// Tag/Badge resolve through the same palette as the rest of the style
    /// system. This is the single tone vocabulary — there is no separate
    /// widget-layer color list.
    #[inline]
    pub fn to_tone(self) -> crate::ui_kit::sx::Tone {
        use crate::ui_kit::sx::Tone;
        match self {
            TagTone::Neutral => Tone::Dim,
            TagTone::Accent => Tone::Accent,
            TagTone::Bull => Tone::Bull,
            TagTone::Bear => Tone::Bear,
            TagTone::Warn => Tone::Warn,
        }
    }

    /// Resolve the base color through the unified Sx palette. Byte-identical to
    /// the previous direct `theme.dim()/accent()/…` reads (Sx `S500` == the
    /// theme base), but now routed through the one shared color authority.
    pub fn color(&self, theme: &dyn ComponentTheme) -> Color32 {
        crate::ui_kit::sx::palette_ct(theme).base(self.to_tone())
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
    disabled: bool,
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
            disabled: false,
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
    pub fn disabled(mut self, d: bool) -> Self { self.disabled = d; self }

    pub fn show(self, ui: &mut Ui, theme: &dyn ComponentTheme) -> TagResponse {
        let dim_mul = if self.disabled { 0.5 } else { 1.0 };
        let tone_col = self.tone.color(theme).gamma_multiply(dim_mul);
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
            // DS#4: declare the chip box — outline (border-only) or soft fill.
            let chip = Sx::new().rounded(h * 0.5);
            if self.outline {
                chip.border_color(tone_col, st::stroke_std()).paint_box_at(&painter, rect, theme);
            } else {
                chip.bg_color(st::color_alpha(tone_col, 32)).paint_box_at(&painter, rect, theme);
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
                let close_sense = if self.disabled { Sense::hover() } else { Sense::click() };
                let close_resp = ui.interact(close_rect, response.id.with("close"), close_sense);
                let col = if !self.disabled && close_resp.hovered() {
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
                if !self.disabled && close_resp.clicked() { closed = true; }
            }
        }

        TagResponse { response, closed }
    }
}

impl<'a> Widget for Tag<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        let theme = super::theme::active_theme(ui.ctx());
        self.show(ui, &theme).response
    }
}

// ─── paint_pill — painter-level pill for absolute-rect callers ───────────────

/// Fill treatment for [`paint_pill`].
#[derive(Clone, Copy, PartialEq)]
pub enum PillStyle {
    /// Soft tinted fill — the default chip look (matches [`Tag`]).
    Soft,
    /// Fainter tinted fill — secondary / status pills.
    Subtle,
    /// Solid fill with contrast text — strong emphasis (matches [`Badge`]).
    Solid,
    /// Border only, transparent fill.
    Outline,
}

/// **Painter-level pill** — draws a labelled, fully-rounded pill into an absolute
/// `rect` with no `Ui`, for chart overlays + painter-driven cards that can't use
/// the `Ui`-based [`Tag`] / [`Badge`]. `color` is the tone; it shares the Sx box
/// renderer so it matches the chip widgets, and the label is centred in `font`.
pub fn paint_pill(
    p: &egui::Painter, rect: egui::Rect, label: &str, color: Color32,
    style: PillStyle, font: FontId, t: &dyn ComponentTheme,
) {
    let r = rect.height() * 0.5;
    match style {
        PillStyle::Soft    => Sx::new().rounded(r).bg_color(st::color_alpha(color, 32)).paint_box_at(p, rect, t),
        PillStyle::Subtle  => Sx::new().rounded(r).bg_color(st::color_alpha(color, 20)).paint_box_at(p, rect, t),
        PillStyle::Solid   => Sx::new().rounded(r).bg_color(color).paint_box_at(p, rect, t),
        PillStyle::Outline => Sx::new().rounded(r).border_color(color, st::stroke_std()).paint_box_at(p, rect, t),
    }
    let text_col = if style == PillStyle::Solid { st::contrast_fg(color) } else { color };
    p.text(rect.center(), egui::Align2::CENTER_CENTER, label, font, text_col);
}
