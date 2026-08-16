//! Badge — small numeric indicator overlaid on icons or row endings.
//! Different from Tag: smaller, usually carries a count or single dot.
//!
//! API:
//!   ui.add(Badge::count(3));               // "3"
//!   ui.add(Badge::count(150).max(99));     // "99+"
//!   ui.add(Badge::dot().tone(TagTone::Warn));

use egui::{Color32, FontId, Pos2, Response, Sense, Ui, Vec2, Widget};

use super::tag::TagTone;
use super::theme::ComponentTheme;
use crate::ui_kit::sx::Sx;

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
    /// Optional raw colour override — if `Some`, this colour is used
    /// instead of resolving `tone` through the theme. Used by legacy
    /// callers (e.g. status_badge wrapper) that compute a colour
    /// dynamically and want pass it through verbatim.
    tone_color_override: Option<egui::Color32>,
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
            tone_color_override: None,
            _lt: std::marker::PhantomData,
        }
    }

    pub fn dot() -> Self {
        Self {
            kind: BadgeKind::Dot,
            text: String::new(),
            tone: TagTone::Accent,
            max_count: None,
            tone_color_override: None,
            _lt: std::marker::PhantomData,
        }
    }

    pub fn text(s: impl Into<String>) -> Self {
        Self {
            kind: BadgeKind::Text,
            text: s.into(),
            tone: TagTone::Accent,
            max_count: None,
            tone_color_override: None,
            _lt: std::marker::PhantomData,
        }
    }

    pub fn tone(mut self, t: TagTone) -> Self { self.tone = t; self }
    pub fn max(mut self, max_count: u32) -> Self { self.max_count = Some(max_count); self }

    /// Override the tone colour with an arbitrary `Color32`. Bypasses the
    /// `TagTone` enum entirely — use only when the caller has a dynamic
    /// colour that doesn't map cleanly to a tone (e.g. status colours
    /// computed from connection state or order state).
    pub fn tone_color(mut self, c: Color32) -> Self { self.tone_color_override = Some(c); self }

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
        // count(0) renders nothing.
        if let BadgeKind::Count(0) = self.kind {
            let (_, r) = ui.allocate_exact_size(Vec2::ZERO, Sense::hover());
            return r;
        }

        let tone_col = self.tone_color_override.unwrap_or_else(|| self.tone.color(theme));

        // Resolve display text.
        let display = match self.kind {
            BadgeKind::Count(n) => match self.max_count {
                Some(m) if n > m => format!("{}+", m),
                _ => n.to_string(),
            },
            BadgeKind::Dot => String::new(),
            BadgeKind::Text => self.text.clone(),
        };

        // Was a bare 14.0 while a `badge.height` token sat with no reader —
        // the slider moved and nothing did. Token default set to 14.0 so an
        // unauthored style renders byte-identically (it was 16.0, which would
        // have made wiring it a silent resize).
        let h: f32 = crate::dt_f32!(badge.height, 14.0);

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
        let font_size: f32 = crate::dt_f32!(badge.font_size, 10.0);
        let galley = ui.fonts(|f| {
            // layout-only: width measurement only; text color is decided below.
            f.layout_no_wrap(display.clone(), crate::ui_kit::style::mono_at(font_size), egui::Color32::PLACEHOLDER)
        });
        let text_w = galley.rect.width();

        let pad_x: f32 = 5.0;
        let w = (text_w + pad_x * 2.0).max(h); // pill but at least circular
        let desired = Vec2::new(w, h);
        let (rect, response) = ui.allocate_exact_size(desired, Sense::hover());

        if ui.is_rect_visible(rect) {
            let painter = ui.painter_at(rect);
            // DS#4: the pill is declared as an Sx solid-fill box.
            // `badge` key. Default is a true pill (half the height); the
            // tone-derived fill stays with the widget.
            super::theme::resolve_sx(ui.ctx(), theme, "badge",
                Sx::new().rounded(h * 0.5).bg_color(tone_col),
            ).paint_box_at(&painter, rect, theme);
            painter.text(
                Pos2::new(rect.center().x, rect.center().y),
                egui::Align2::CENTER_CENTER,
                &display,
                crate::ui_kit::style::mono_at(font_size),
                crate::ui_kit::tokens::contrast_fg(tone_col),
            );
        }

        response
    }
}

impl<'a> Widget for Badge<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        let theme = super::theme::active_theme(ui.ctx());
        self.show(ui, &theme)
    }
}
