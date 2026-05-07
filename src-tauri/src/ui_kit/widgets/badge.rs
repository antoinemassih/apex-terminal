//! Badge — small numeric indicator overlaid on icons or row endings.
//! Different from Tag: smaller, usually carries a count or single dot.
//!
//! API:
//!   ui.add(Badge::count(3));               // "3"
//!   ui.add(Badge::count(150).max(99));     // "99+"
//!   ui.add(Badge::dot().tone(TagTone::Warn));

use egui::{Color32, CornerRadius, FontId, Pos2, Response, Sense, Ui, Vec2, Widget};

use super::tag::TagTone;
use super::theme::ComponentTheme;

#[derive(Clone, Copy)]
enum BadgeKind {
    Count(u32),
    Dot,
    Text,
}

#[must_use = "Badge does nothing until `.show(ui, theme)` or `ui.add(badge)` is called"]
pub struct Badge<'a> {
    kind: BadgeKind,
    text: String,
    tone: TagTone,
    max_count: Option<u32>,
    _lt: std::marker::PhantomData<&'a ()>,
}

impl<'a> Badge<'a> {
    pub fn count(n: u32) -> Self {
        Self {
            kind: BadgeKind::Count(n),
            text: String::new(),
            tone: TagTone::Bear,
            max_count: None,
            _lt: std::marker::PhantomData,
        }
    }

    pub fn dot() -> Self {
        Self {
            kind: BadgeKind::Dot,
            text: String::new(),
            tone: TagTone::Accent,
            max_count: None,
            _lt: std::marker::PhantomData,
        }
    }

    pub fn text(s: impl Into<String>) -> Self {
        Self {
            kind: BadgeKind::Text,
            text: s.into(),
            tone: TagTone::Accent,
            max_count: None,
            _lt: std::marker::PhantomData,
        }
    }

    pub fn tone(mut self, t: TagTone) -> Self { self.tone = t; self }
    pub fn max(mut self, max_count: u32) -> Self { self.max_count = Some(max_count); self }

    pub fn show(self, ui: &mut Ui, theme: &dyn ComponentTheme) -> Response {
        // count(0) renders nothing.
        if let BadgeKind::Count(0) = self.kind {
            let (_, r) = ui.allocate_exact_size(Vec2::ZERO, Sense::hover());
            return r;
        }

        let tone_col = self.tone.color(theme);

        // Resolve display text.
        let display = match self.kind {
            BadgeKind::Count(n) => match self.max_count {
                Some(m) if n > m => format!("{}+", m),
                _ => n.to_string(),
            },
            BadgeKind::Dot => String::new(),
            BadgeKind::Text => self.text.clone(),
        };

        let h: f32 = 14.0;

        if matches!(self.kind, BadgeKind::Dot) {
            let size = 8.0;
            let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
            if ui.is_rect_visible(rect) {
                ui.painter().circle_filled(rect.center(), size * 0.5, tone_col);
            }
            return response;
        }

        // NOTE: 10px font intentionally violates the typography scale (which
        // forbids sub-11px). Badges are a documented exception — they are
        // glanceable indicators (counts on icons), not body text. Do not
        // copy this size into other widgets.
        let font_size: f32 = 10.0;
        let galley = ui.fonts(|f| {
            f.layout_no_wrap(display.clone(), FontId::monospace(font_size), Color32::WHITE)
        });
        let text_w = galley.rect.width();

        let pad_x: f32 = 5.0;
        let w = (text_w + pad_x * 2.0).max(h); // pill but at least circular
        let desired = Vec2::new(w, h);
        let (rect, response) = ui.allocate_exact_size(desired, Sense::hover());

        if ui.is_rect_visible(rect) {
            let painter = ui.painter_at(rect);
            let cr = CornerRadius::same((h * 0.5) as u8);
            painter.rect_filled(rect, cr, tone_col);
            painter.text(
                Pos2::new(rect.center().x, rect.center().y),
                egui::Align2::CENTER_CENTER,
                &display,
                FontId::monospace(font_size),
                Color32::WHITE,
            );
        }

        response
    }
}

impl<'a> Widget for Badge<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        let theme = &crate::chart_renderer::gpu::THEMES[0];
        self.show(ui, theme)
    }
}
