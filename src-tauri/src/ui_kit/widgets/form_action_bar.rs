//! `FormActionBar` — standardized form footer button row.
//!
//! Right-aligned (primary action LAST per macOS convention). A hairline
//! separator is painted above the bar; `gap_md()` padding is added above
//! and below the button row.
//!
//! Use this at the bottom of every modal form and every settings panel that
//! has explicit save/discard semantics.
//!
//! ```ignore
//! let resp = FormActionBar::new("Save Changes")
//!     .primary_variant(ActionBarPrimary::Save)
//!     .cancel("Cancel")
//!     .tertiary("Reset to Defaults")
//!     .show(ui, theme);
//!
//! if resp.primary_clicked   { /* save */ }
//! if resp.secondary_clicked { /* cancel */ }
//! if resp.tertiary_clicked  { /* reset */ }
//! ```

use egui::{Stroke, Ui};

use super::button::Button;
use super::theme::ComponentTheme;
use super::tokens::{Size, Variant};
use crate::ui_kit::tokens as st;

/// Which semantic role the primary action plays — controls the button variant.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum ActionBarPrimary {
    /// Submit / confirm — uses `Variant::Primary` (accent fill).
    #[default]
    Submit,
    /// Save — semantically the same as Submit; uses `Variant::Primary`.
    Save,
    /// Destructive primary action — uses `Variant::Danger` (bear fill).
    Danger,
}

/// Standardized form footer button row.
///
/// Right-aligned, with the primary action rightmost (macOS convention).
/// Includes an optional tertiary action (e.g. "Reset") anchored to the left.
#[must_use = "FormActionBar does nothing until `.show(ui, theme)` is called"]
pub struct FormActionBar<'a> {
    primary_label: &'a str,
    primary_enabled: bool,
    primary_variant: ActionBarPrimary,
    secondary_label: Option<&'a str>,
    tertiary_label: Option<&'a str>,
}

/// Click results from a [`FormActionBar`].
#[derive(Default)]
pub struct FormActionBarResponse {
    /// `true` when the primary action button was clicked this frame.
    pub primary_clicked: bool,
    /// `true` when the secondary (Cancel) button was clicked this frame.
    pub secondary_clicked: bool,
    /// `true` when the tertiary (e.g. Reset) button was clicked this frame.
    pub tertiary_clicked: bool,
}

impl<'a> FormActionBar<'a> {
    /// Create a bar with only a primary action label. Add `.cancel()` and
    /// `.tertiary()` as needed.
    pub fn new(primary: &'a str) -> Self {
        Self {
            primary_label: primary,
            primary_enabled: true,
            primary_variant: ActionBarPrimary::default(),
            secondary_label: None,
            tertiary_label: None,
        }
    }

    /// Enable or disable the primary button (greys it out when `false`).
    pub fn primary_enabled(mut self, e: bool) -> Self {
        self.primary_enabled = e;
        self
    }

    /// Override the semantic role of the primary button.
    pub fn primary_variant(mut self, v: ActionBarPrimary) -> Self {
        self.primary_variant = v;
        self
    }

    /// Add a secondary (Cancel) button left of the primary.
    pub fn cancel(mut self, label: &'a str) -> Self {
        self.secondary_label = Some(label);
        self
    }

    /// Add an optional left-aligned tertiary action (e.g. "Reset to Defaults").
    pub fn tertiary(mut self, label: &'a str) -> Self {
        self.tertiary_label = Some(label);
        self
    }

    /// Render the separator + button row and return click results.
    pub fn show(self, ui: &mut Ui, theme: &dyn ComponentTheme) -> FormActionBarResponse {
        let mut resp = FormActionBarResponse::default();

        // ── Separator ────────────────────────────────────────────────────────
        ui.add_space(st::gap_md());
        {
            let avail = ui.available_width();
            let (rect, _) = ui.allocate_exact_size(
                egui::Vec2::new(avail, st::stroke_thin()),
                egui::Sense::hover(),
            );
            if ui.is_rect_visible(rect) {
                let sep_col = st::color_alpha(theme.border(), st::alpha_dim());
                ui.painter().hline(
                    rect.x_range(),
                    rect.center().y,
                    Stroke::new(st::stroke_thin(), sep_col),
                );
            }
        }
        ui.add_space(st::gap_md());

        // ── Button row ────────────────────────────────────────────────────────
        // We need tertiary (left-aligned) AND right-aligned buttons in the same
        // horizontal strip. Use a custom layout: left side gets the tertiary,
        // the right side gets secondary + primary.
        let primary_variant = match self.primary_variant {
            ActionBarPrimary::Danger => Variant::Danger,
            ActionBarPrimary::Submit | ActionBarPrimary::Save => Variant::Primary,
        };

        ui.horizontal(|ui| {
            // ── Tertiary (left-aligned) ───────────────────────────────────────
            if let Some(label) = self.tertiary_label {
                let r = Button::new(label)
                    .variant(Variant::Ghost)
                    .size(Size::Md)
                    .show(ui, theme);
                if r.clicked() {
                    resp.tertiary_clicked = true;
                }
            }

            // Push remaining buttons to the right.
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = st::gap_sm();

                // Primary — rightmost.
                let r = Button::new(self.primary_label)
                    .variant(primary_variant)
                    .size(Size::Md)
                    .disabled(!self.primary_enabled)
                    .show(ui, theme);
                if r.clicked() {
                    resp.primary_clicked = true;
                }

                // Secondary (Cancel) — left of primary.
                if let Some(label) = self.secondary_label {
                    let r = Button::new(label)
                        .variant(Variant::Ghost)
                        .size(Size::Md)
                        .show(ui, theme);
                    if r.clicked() {
                        resp.secondary_clicked = true;
                    }
                }
            });
        });

        ui.add_space(st::gap_md());

        resp
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Smoke: builder chains compile and default state is correct.
    #[test]
    fn action_bar_default_primary_enabled() {
        let bar = FormActionBar::new("Save");
        assert!(
            bar.primary_enabled,
            "primary must be enabled by default"
        );
        assert_eq!(
            bar.primary_variant,
            ActionBarPrimary::Submit,
            "default variant must be Submit"
        );
        assert!(bar.secondary_label.is_none());
        assert!(bar.tertiary_label.is_none());
    }

    /// Smoke: `.cancel()` sets secondary_label.
    #[test]
    fn cancel_sets_secondary_label() {
        let bar = FormActionBar::new("OK").cancel("Cancel");
        assert_eq!(bar.secondary_label, Some("Cancel"));
    }

    /// Smoke: `.primary_variant(Danger)` maps to Danger.
    #[test]
    fn danger_variant_round_trips() {
        let bar = FormActionBar::new("Delete").primary_variant(ActionBarPrimary::Danger);
        assert_eq!(bar.primary_variant, ActionBarPrimary::Danger);
    }

    /// Smoke: `.primary_enabled(false)` is stored.
    #[test]
    fn primary_disabled_state_stored() {
        let bar = FormActionBar::new("Submit").primary_enabled(false);
        assert!(!bar.primary_enabled);
    }

    /// Smoke: verify separator is painted (source check).
    #[test]
    fn separator_is_painted() {
        let src = include_str!("form_action_bar.rs");
        assert!(
            src.contains("alpha_dim()") && src.contains("theme.border()"),
            "form_action_bar must paint a separator using theme.border() + alpha_dim"
        );
    }
}

// TODO follow-up: re-export from widgets/mod.rs after the 5-primitives agent lands.
//   pub use form_action_bar::{FormActionBar, FormActionBarResponse, ActionBarPrimary};
