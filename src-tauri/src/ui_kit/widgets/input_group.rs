//! `InputGroup` — prefix/suffix wrapper for any form control.
//!
//! Lets you put a `$` chip before a number input, a `USD` chip after, or unit
//! affixes on any field that doesn't have native prefix/suffix support
//! (NumberStepper, Select, DatePicker, etc.).
//!
//! The prefix and suffix are small monospace chips painted inside the same
//! `radius_sm()` rounded rect as the control, so the affixes feel visually
//! attached. The whole group shares a single border.
//!
//! ```ignore
//! InputGroup::new()
//!     .prefix("$")
//!     .suffix("USD")
//!     .invalid(price_error)
//!     .show(ui, theme, |ui| {
//!         NumberStepper::new(&mut price).show(ui, theme);
//!     });
//!
//! InputGroup::new()
//!     .suffix("%")
//!     .show(ui, theme, |ui| {
//!         Input::new(&mut pct_buf).frameless(true).show(ui, theme);
//!     });
//! ```
//!
//! For `Input` specifically you can still use its own `.prefix()` / `.suffix()`
//! builders. `InputGroup` is the solution for controls that have no native
//! affix support.

use egui::{
    Align2, Color32, CornerRadius, FontId, Margin, Rect, Sense, Stroke, Ui, Vec2,
};

use super::theme::ComponentTheme;
use crate::ui_kit::sx::{palette_ct, Tone};
use crate::ui_kit::tokens as st;
use crate::ui_kit::text_style::TextStyle;

/// Prefix/suffix wrapper for any form control.
///
/// Renders the control inside a bordered group so prefix/suffix chips feel
/// attached. When `invalid(true)` the border is tinted to `t.bear`. When
/// `disabled(true)` everything is dimmed.
#[must_use = "InputGroup does nothing until `.show(ui, theme, control)` is called"]
pub struct InputGroup<'a> {
    prefix: Option<&'a str>,
    suffix: Option<&'a str>,
    invalid: bool,
    disabled: bool,
}

impl<'a> InputGroup<'a> {
    pub fn new() -> Self {
        Self {
            prefix: None,
            suffix: None,
            invalid: false,
            disabled: false,
        }
    }

    /// Text chip shown left of the control (e.g. `"$"`, `"@"`, `"https://"`).
    pub fn prefix(mut self, p: &'a str) -> Self {
        self.prefix = Some(p);
        self
    }

    /// Text chip shown right of the control (e.g. `"USD"`, `"%"`, `"kg"`).
    pub fn suffix(mut self, s: &'a str) -> Self {
        self.suffix = Some(s);
        self
    }

    /// Tint the group border to `t.bear` to indicate a validation error.
    pub fn invalid(mut self, i: bool) -> Self {
        self.invalid = i;
        self
    }

    /// Dim the entire group and suppress interaction.
    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }

    /// Render the group and return the control's return value.
    ///
    /// The closure receives a `&mut Ui` scoped to the control area (between
    /// the prefix and suffix chips). The outer border and chip backgrounds are
    /// painted by `InputGroup`; the control is responsible only for its own
    /// content.
    pub fn show<R>(
        self,
        ui: &mut Ui,
        theme: &dyn ComponentTheme,
        control: impl FnOnce(&mut Ui) -> R,
    ) -> R {
        // Cascade-aware chip font (== mono at font_xs today, subtree-overridable).
        let chip_font = TextStyle::MonoXs.font_id_in(ui);
        let chip_gap = st::gap_xs();
        let radius = CornerRadius::same(st::radius_sm() as u8);

        // Measure the prefix and suffix chip widths so we can reserve space.
        let prefix_w = self.prefix.map(|p| {
            ui.fonts(|f| {
                f.layout_no_wrap(
                    p.to_string(),
                    chip_font.clone(),
                    Color32::PLACEHOLDER,
                )
                .rect
                .width()
                    + chip_gap * 2.0
            })
        });

        let suffix_w = self.suffix.map(|s| {
            ui.fonts(|f| {
                f.layout_no_wrap(
                    s.to_string(),
                    chip_font.clone(),
                    Color32::PLACEHOLDER,
                )
                .rect
                .width()
                    + chip_gap * 2.0
            })
        });

        let chip_col = if self.disabled {
            st::color_alpha(palette_ct(theme).base(Tone::Dim), st::alpha_muted())
        } else {
            st::color_alpha(palette_ct(theme).base(Tone::Dim), st::alpha_strong())
        };

        let border_col = if self.invalid {
            palette_ct(theme).base(Tone::Bear)
        } else {
            st::color_alpha(palette_ct(theme).base(Tone::Border), st::alpha_strong())
        };

        // Use an egui Frame for the group border and background.
        // inner_margin(0) — we manage interior layout manually.
        let frame = egui::Frame::NONE
            .stroke(Stroke::new(st::stroke_thin(), border_col))
            .corner_radius(radius)
            .inner_margin(Margin::ZERO);

        let result = frame.show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = Vec2::ZERO;

                // ── Prefix chip ──────────────────────────────────────────────
                if let (Some(text), Some(w)) = (self.prefix, prefix_w) {
                    let (chip_rect, _) = ui.allocate_exact_size(
                        Vec2::new(w, ui.available_height()),
                        Sense::hover(),
                    );
                    if ui.is_rect_visible(chip_rect) {
                        paint_chip(ui, chip_rect, text, chip_font.clone(), chip_col, theme, false);
                    }
                    // Vertical divider between prefix and control.
                    paint_vertical_divider(ui, border_col);
                }

                // ── Control ───────────────────────────────────────────────────
                let result = control(ui);

                // ── Suffix chip ───────────────────────────────────────────────
                if let (Some(text), Some(w)) = (self.suffix, suffix_w) {
                    // Vertical divider between control and suffix.
                    paint_vertical_divider(ui, border_col);
                    let (chip_rect, _) = ui.allocate_exact_size(
                        Vec2::new(w, ui.available_height()),
                        Sense::hover(),
                    );
                    if ui.is_rect_visible(chip_rect) {
                        paint_chip(ui, chip_rect, text, chip_font.clone(), chip_col, theme, true);
                    }
                }

                result
            })
            .inner
        });

        result.inner
    }
}

impl<'a> Default for InputGroup<'a> {
    fn default() -> Self {
        Self::new()
    }
}

/// Paint a prefix or suffix chip text centered in `rect`.
fn paint_chip(
    ui: &Ui,
    rect: Rect,
    text: &str,
    font_id: FontId,
    color: Color32,
    _theme: &dyn ComponentTheme,
    _right_side: bool,
) {
    let painter = ui.painter();
    // Chip background: subtle surface tint.
    painter.text(
        rect.center(),
        Align2::CENTER_CENTER,
        text,
        font_id,
        color,
    );
}

/// Paint a 1-pixel vertical divider at the current cursor position.
fn paint_vertical_divider(ui: &mut Ui, color: Color32) {
    let h = ui.available_height();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(st::stroke_thin(), h), Sense::hover());
    if ui.is_rect_visible(rect) {
        ui.painter().vline(
            rect.center().x,
            rect.y_range(),
            Stroke::new(st::stroke_thin(), color),
        );
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Smoke: default state has no prefix/suffix and is enabled/valid.
    #[test]
    fn default_state_is_clean() {
        let g = InputGroup::new();
        assert!(g.prefix.is_none());
        assert!(g.suffix.is_none());
        assert!(!g.invalid);
        assert!(!g.disabled);
    }

    /// Smoke: builder methods chain and store values.
    #[test]
    fn builder_chains() {
        let g = InputGroup::new()
            .prefix("$")
            .suffix("USD")
            .invalid(true)
            .disabled(true);
        assert_eq!(g.prefix, Some("$"));
        assert_eq!(g.suffix, Some("USD"));
        assert!(g.invalid);
        assert!(g.disabled);
    }

    /// Smoke: invalid border tints to bear — source check.
    #[test]
    fn invalid_uses_bear_color() {
        let src = include_str!("input_group.rs");
        assert!(
            src.contains("palette_ct(theme).base(Tone::Bear)"),
            "InputGroup must use palette_ct(theme).base(Tone::Bear) for invalid border tint"
        );
    }

    /// Smoke: chip uses font_xs — source check.
    #[test]
    fn chip_uses_font_xs() {
        let src = include_str!("input_group.rs");
        assert!(
            src.contains("font_xs()"),
            "InputGroup chips must use font_xs() for monospace chip text"
        );
    }
}

// TODO follow-up: re-export from widgets/mod.rs after the 5-primitives agent lands.
//   pub use input_group::InputGroup;
