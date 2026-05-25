//! `CountChip` — muted rounded count pill for section headers.
//!
//! Distinct from `Badge::count(n)` (which is a bold overlay-style accent dot
//! intended for notification counts on icons) and from `Tag` (which is for
//! semantic labels with optional close). `CountChip` is a low-prominence
//! `(N)` indicator that sits inline next to a section title — e.g.
//! "INDICATORS (5)" / "ALERTS (12)".
//!
//! Audit-identified gap: `indicators_panel.rs:894-904` and ~4 other section
//! header sites paint a rounded rect + centered count text inline because
//! `Badge` was visually too prominent and `Tag` carries unwanted semantic tone.
//!
//! ### Usage
//!
//! ```ignore
//! ui.horizontal(|ui| {
//!     ui.label("INDICATORS");
//!     CountChip::new(active_count).show(ui, theme);
//! });
//! ```
//!
//! Color defaults to `theme.dim()` at alpha ~38 fill, dim text. Override via
//! `.tone(CountChipTone::Accent)` for a more prominent count (e.g. unread).

use egui::{Color32, FontId, Sense, Stroke, Ui, Vec2};

use super::theme::ComponentTheme;
use crate::ui_kit::tokens as st;

/// Visual prominence tier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum CountChipTone {
    #[default]
    /// Muted gray pill (dim fg + dim bg @ low alpha). Default for section counters.
    Muted,
    /// Accent-tinted (call attention). Use sparingly.
    Accent,
    /// Bull / positive count.
    Bull,
    /// Bear / negative count.
    Bear,
    /// Warn / pending count.
    Warn,
}

impl CountChipTone {
    fn colors(self, t: &dyn ComponentTheme) -> (Color32, Color32) {
        // (fg, fill_base) — fill is then alpha-shifted at paint time.
        match self {
            CountChipTone::Muted  => (t.dim(),    t.text()),
            CountChipTone::Accent => (t.accent(), t.accent()),
            CountChipTone::Bull   => (t.bull(),   t.bull()),
            CountChipTone::Bear   => (t.bear(),   t.bear()),
            CountChipTone::Warn   => (t.warn(),   t.warn()),
        }
    }
}

// Helper: forward dyn-erased ComponentTheme to the tone-color helpers without
// trait-object lifetime fights. Both `show` and `paint_at` need it.
struct ThemeBox<'a, T: ComponentTheme + ?Sized>(&'a T);
impl<'a, T: ComponentTheme + ?Sized> ComponentTheme for ThemeBox<'a, T> {
    fn accent(&self) -> Color32 { self.0.accent() }
    fn bull(&self) -> Color32 { self.0.bull() }
    fn bear(&self) -> Color32 { self.0.bear() }
    fn text(&self) -> Color32 { self.0.text() }
    fn dim(&self) -> Color32 { self.0.dim() }
    fn border(&self) -> Color32 { self.0.border() }
    fn border_variant(&self) -> Color32 { self.0.border_variant() }
    fn warn(&self) -> Color32 { self.0.warn() }
    fn bg(&self) -> Color32 { self.0.bg() }
    fn surface(&self) -> Color32 { self.0.surface() }
    fn element_hover(&self) -> Color32 { self.0.element_hover() }
    fn element_active(&self) -> Color32 { self.0.element_active() }
    fn element_selected(&self) -> Color32 { self.0.element_selected() }
    fn element_disabled(&self) -> Color32 { self.0.element_disabled() }
    fn ghost_hover(&self) -> Color32 { self.0.ghost_hover() }
    fn ghost_active(&self) -> Color32 { self.0.ghost_active() }
    fn icon(&self) -> Color32 { self.0.icon() }
    fn icon_muted(&self) -> Color32 { self.0.icon_muted() }
    fn icon_disabled(&self) -> Color32 { self.0.icon_disabled() }
    fn icon_accent(&self) -> Color32 { self.0.icon_accent() }
}

#[must_use = "CountChip does nothing until `.show(ui, theme)` is called"]
pub struct CountChip {
    count: u32,
    tone: CountChipTone,
    /// Cap displayed count at this value; values above show as `"{max}+"`.
    max: Option<u32>,
    /// Override the chip fg color (defaults to tone fg).
    fg: Option<Color32>,
}

impl CountChip {
    pub fn new(count: u32) -> Self {
        Self { count, tone: CountChipTone::Muted, max: None, fg: None }
    }

    pub fn tone(mut self, tone: CountChipTone) -> Self { self.tone = tone; self }
    pub fn max(mut self, max: u32) -> Self { self.max = Some(max); self }
    pub fn fg(mut self, c: Color32) -> Self { self.fg = Some(c); self }

    /// Resolved display text after applying `.max()` clamp. Exposed for tests
    /// and for code that wants the same label without painting.
    pub fn display_text(&self) -> String {
        match self.max {
            Some(max) if self.count > max => format!("{}+", max),
            _ => format!("{}", self.count),
        }
    }

    /// Paint into an existing painter at an absolute rect. For chrome that
    /// already runs in painter mode (chart-pane HUDs, panel-section painter
    /// headers like indicators_panel.rs:870-906). Pre-computes the rect's
    /// width from text + padding so callers can lay out the chip within a
    /// row. Returns the painted rect.
    pub fn paint_at<T: ComponentTheme + ?Sized>(
        self,
        painter: &egui::Painter,
        right_anchor: egui::Pos2,
        theme: &T,
    ) -> egui::Rect {
        let text = self.display_text();
        let theme_dyn: &dyn ComponentTheme = &ThemeBox(theme);
        let (tone_fg, tone_fill_base) = self.tone.colors(theme_dyn);
        let fg = self.fg.unwrap_or(tone_fg);
        let fill = st::color_alpha(tone_fill_base, 38);

        let font = FontId::monospace(st::font_xs());
        let pad_x = st::gap_xs();
        let h: f32 = 14.0;
        let galley = painter.layout_no_wrap(text.clone(), font.clone(), fg);
        let w = galley.size().x + pad_x * 2.0;
        let rect = egui::Rect::from_min_size(
            egui::pos2(right_anchor.x - w, right_anchor.y - h * 0.5),
            Vec2::new(w, h),
        );
        painter.rect_filled(rect, st::r_sm_cr(), fill);
        painter.rect_stroke(
            rect,
            st::r_sm_cr(),
            Stroke::new(st::stroke_thin(), st::color_alpha(tone_fg, 60)),
            egui::StrokeKind::Inside,
        );
        painter.text(rect.center(), egui::Align2::CENTER_CENTER, &text, font, fg);
        rect
    }

    pub fn show<T: ComponentTheme + ?Sized>(self, ui: &mut Ui, theme: &T) -> egui::Response {
        let text = self.display_text();

        let theme_dyn: &dyn ComponentTheme = &ThemeBox(theme);
        let (tone_fg, tone_fill_base) = self.tone.colors(theme_dyn);
        let fg = self.fg.unwrap_or(tone_fg);
        let fill = st::color_alpha(tone_fill_base, 38);

        // Layout: measure → allocate → paint.
        let font = FontId::monospace(st::font_xs());
        let pad_x = st::gap_xs();
        let h: f32 = 14.0;
        let galley = ui.fonts(|f| f.layout_no_wrap(text.clone(), font.clone(), fg));
        let w = galley.size().x + pad_x * 2.0;
        let (rect, response) = ui.allocate_exact_size(Vec2::new(w, h), Sense::hover());
        if ui.is_rect_visible(rect) {
            let painter = ui.painter_at(rect);
            // Rounded pill — radius_sm() looks right at 14px height.
            painter.rect_filled(rect, st::r_sm_cr(), fill);
            painter.rect_stroke(
                rect,
                st::r_sm_cr(),
                Stroke::new(st::stroke_thin(), st::color_alpha(tone_fg, 60)),
                egui::StrokeKind::Inside,
            );
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                &text,
                font,
                fg,
            );
        }
        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_text_no_max() {
        assert_eq!(CountChip::new(0).display_text(), "0");
        assert_eq!(CountChip::new(5).display_text(), "5");
        assert_eq!(CountChip::new(247).display_text(), "247");
    }

    #[test]
    fn display_text_under_max_unchanged() {
        assert_eq!(CountChip::new(5).max(9).display_text(), "5");
        assert_eq!(CountChip::new(9).max(9).display_text(), "9");
        assert_eq!(CountChip::new(99).max(99).display_text(), "99");
    }

    #[test]
    fn display_text_over_max_appends_plus() {
        assert_eq!(CountChip::new(10).max(9).display_text(), "9+");
        assert_eq!(CountChip::new(100).max(99).display_text(), "99+");
        assert_eq!(CountChip::new(9999).max(99).display_text(), "99+");
    }

    #[test]
    fn builder_chain_compiles_for_every_tone() {
        // Smoke-test that every public API entry chains without compile errors.
        let _ = CountChip::new(1).tone(CountChipTone::Muted);
        let _ = CountChip::new(1).tone(CountChipTone::Accent);
        let _ = CountChip::new(1).tone(CountChipTone::Bull);
        let _ = CountChip::new(1).tone(CountChipTone::Bear);
        let _ = CountChip::new(1).tone(CountChipTone::Warn);
        let _ = CountChip::new(1).max(99).fg(Color32::WHITE);
    }
}
