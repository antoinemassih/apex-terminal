//! CardHeader / CardBody / CardFooter — the card slot skeleton.
//!
//! Promotes the ad-hoc `.lc-card-h` header div + body/footer rhythm that was
//! hand-written in every layout into reusable slots, so every card has the
//! same structural bones. Compose inside a `PanelCard::show()` closure:
//!
//! ```ignore
//! PanelCard::new().show(ui, t, |ui, t| {
//!     CardHeader::new("Watchlist")
//!         .trailing(|ui, t| { Badge::count(12).show(ui, t); })
//!         .divider(true)
//!         .show(ui, t);
//!     CardBody::new().show(ui, t, |ui, t| {
//!         // rows
//!     });
//!     CardFooter::new().show(ui, t, |ui, t| {
//!         // actions
//!     });
//! });
//! ```
//!
//! Mirror of React `CardSlots.tsx` (`CardHeader`, `CardBody`, `CardFooter`).

use egui::{Color32, Frame, Margin, Stroke, Ui};

use crate::ui_kit::style::{font_xs, gap_sm, gap_xs};
use crate::ui_kit::widgets::theme::ComponentTheme;

// ── Design tokens (fallback constants matching --ds-card-* CSS vars) ─────────

/// Height of the card header bar — matches `--ds-card-header-h` (30px default).
pub const CARD_HEADER_H: f32 = 30.0;

/// Horizontal padding inside the card header — matches `--ds-card-pad`.
pub fn card_pad() -> f32 { gap_sm() + gap_xs() } // ~10px

// ── CardHeader ────────────────────────────────────────────────────────────────

/// A horizontal title bar with an optional trailing slot (pill, count, actions).
///
/// Mirrors React `CardHeader` from `CardSlots.tsx`.
pub struct CardHeader<'a, F = fn(&mut Ui, &dyn ComponentTheme)>
where
    F: FnOnce(&mut Ui, &dyn ComponentTheme),
{
    title:   &'a str,
    trailing: Option<F>,
    divider: bool,
    title_color: Option<Color32>,
}

impl<'a> CardHeader<'a, fn(&mut Ui, &dyn ComponentTheme)> {
    /// Card header with title text.
    pub fn new(title: &'a str) -> Self {
        Self { title, trailing: None, divider: true, title_color: None }
    }
}

impl<'a, F: FnOnce(&mut Ui, &dyn ComponentTheme)> CardHeader<'a, F> {
    /// Show a divider line below the header (default: true).
    pub fn divider(mut self, on: bool) -> Self { self.divider = on; self }

    /// Override the title text color (default: `t.dim()`).
    pub fn title_color(mut self, c: Color32) -> Self { self.title_color = Some(c); self }

    /// Draw the header.
    pub fn show<T: ComponentTheme>(self, ui: &mut Ui, t: &T) {
        let h = CARD_HEADER_H;
        let pad = card_pad();

        ui.allocate_ui_with_layout(
            egui::vec2(ui.available_width(), h),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.add_space(pad);

                // Title (left-aligned, truncated).
                let title_color = self.title_color.unwrap_or_else(|| t.dim());
                let font_id = egui::FontId::proportional(font_xs());
                let galley = ui.painter().layout_no_wrap(
                    self.title.to_ascii_uppercase(),
                    font_id.clone(),
                    title_color,
                );
                let (title_rect, _resp) = ui.allocate_exact_size(
                    galley.size(),
                    egui::Sense::hover(),
                );
                ui.painter().galley(title_rect.min, galley, title_color);

                // Trailing (right-aligned — push remaining space, then draw).
                if let Some(trailing) = self.trailing {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(pad);
                        trailing(ui, t);
                    });
                }
            },
        );

        // Bottom divider.
        if self.divider {
            let rect = ui.min_rect();
            let y = rect.bottom();
            let border_color = Color32::from_rgba_premultiplied(
                t.border().r(), t.border().g(), t.border().b(), 80,
            );
            ui.painter().hline(
                rect.left()..=rect.right(),
                y,
                Stroke::new(1.0, border_color),
            );
        }
    }
}

// Extension: attach a trailing closure.
impl<'a> CardHeader<'a, fn(&mut Ui, &dyn ComponentTheme)> {
    /// Attach a trailing widget (right-aligned slot — pill, count, button).
    pub fn trailing<F2: FnOnce(&mut Ui, &dyn ComponentTheme)>(
        self,
        f: F2,
    ) -> CardHeader<'a, F2> {
        CardHeader {
            title:    self.title,
            trailing: Some(f),
            divider:  self.divider,
            title_color: self.title_color,
        }
    }
}

// ── CardBody ──────────────────────────────────────────────────────────────────

/// The main content area of a card, with optional scroll.
///
/// Mirrors React `CardBody` from `CardSlots.tsx`.
pub struct CardBody {
    pad:  bool,
    gap:  f32,
}

impl Default for CardBody {
    fn default() -> Self { Self::new() }
}

impl CardBody {
    pub fn new() -> Self {
        Self { pad: true, gap: gap_xs() }
    }

    /// Apply card padding (default: true). Set false for flush lists or charts.
    pub fn pad(mut self, v: bool) -> Self { self.pad = v; self }

    /// Gap between children (default: `gap_xs()`).
    pub fn gap(mut self, px: f32) -> Self { self.gap = px; self }

    /// Draw the body content area.
    pub fn show<T: ComponentTheme, R>(
        self,
        ui: &mut Ui,
        t: &T,
        body: impl FnOnce(&mut Ui, &T) -> R,
    ) -> R {
        let pad_v = if self.pad { card_pad() } else { 0.0 };
        let gap   = self.gap;

        let mut frame = Frame::NONE.inner_margin(Margin {
            left:   pad_v as i8,
            right:  pad_v as i8,
            top:    (gap / 2.0) as i8,
            bottom: pad_v as i8,
        });

        // Disable the default egui frame since PanelCard already provides background.
        frame.fill = Color32::TRANSPARENT;

        ui.style_mut().spacing.item_spacing.y = gap;
        let out = frame.show(ui, |ui| body(ui, t));
        out.inner
    }
}

// ── CardFooter ────────────────────────────────────────────────────────────────

/// Bottom action bar for a card — full-width horizontal layout.
///
/// Mirrors React `CardFooter` from `CardSlots.tsx`.
pub struct CardFooter {
    divider: bool,
}

impl Default for CardFooter {
    fn default() -> Self { Self::new() }
}

impl CardFooter {
    pub fn new() -> Self { Self { divider: true } }

    /// Show a top divider line (default: true).
    pub fn divider(mut self, on: bool) -> Self { self.divider = on; self }

    /// Draw the footer content area.
    pub fn show<T: ComponentTheme, R>(
        self,
        ui: &mut Ui,
        t: &T,
        body: impl FnOnce(&mut Ui, &T) -> R,
    ) -> R {
        let pad = card_pad();

        // Top divider.
        if self.divider {
            let rect = ui.min_rect();
            let y = rect.top();
            let border_color = Color32::from_rgba_premultiplied(
                t.border().r(), t.border().g(), t.border().b(), 80,
            );
            ui.painter().hline(
                rect.left()..=rect.right(),
                y,
                Stroke::new(1.0, border_color),
            );
        }

        let frame = Frame::NONE.inner_margin(Margin {
            left:   pad as i8,
            right:  pad as i8,
            top:    (gap_xs() / 2.0) as i8,
            bottom: (gap_xs() / 2.0) as i8,
        });

        frame
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.with_layout(
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| body(ui, t),
                )
                .inner
            })
            .inner
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn card_header_new_has_divider_on() {
        let h: CardHeader<'_, fn(&mut Ui, &dyn ComponentTheme)> = CardHeader::new("Test");
        assert!(h.divider, "divider should default to true");
    }

    #[test]
    fn card_body_new_has_pad_on() {
        let b = CardBody::new();
        assert!(b.pad, "pad should default to true");
    }

    #[test]
    fn card_footer_new_has_divider_on() {
        let f = CardFooter::new();
        assert!(f.divider, "divider should default to true");
    }
}
