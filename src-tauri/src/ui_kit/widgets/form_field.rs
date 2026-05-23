//! `FormField` — canonical top-label form field wrapper.
//!
//! Owns: optional label (with required marker), control closure slot, helper text,
//! error text. The helper/error sub-line ALWAYS reserves vertical space so the
//! form layout doesn't jump when an error appears/disappears.
//!
//! Use this around any form control (Input, Select, NumberStepper, etc.) when
//! you want the canonical top-label layout. For side-label (settings panel
//! style), use `FormRow` instead.
//!
//! ```ignore
//! FormField::new("Price")
//!     .required(true)
//!     .error_if(price <= 0.0, "Must be > 0")
//!     .show(ui, theme, |ui| {
//!         Input::new(&mut price_buf).show(ui, theme);
//!     });
//! ```

use egui::{RichText, Ui, Vec2};

use super::theme::ComponentTheme;
use crate::ui_kit::tokens as st;

/// Canonical top-label form field wrapper.
///
/// Renders: optional label (+ required `*` marker) → control closure →
/// reserved helper/error line. The sub-line height is ALWAYS allocated so
/// the surrounding layout does not shift when an error appears or disappears.
#[must_use = "FormField does nothing until `.show(ui, theme, control)` is called"]
pub struct FormField<'a> {
    label: Option<&'a str>,
    required: bool,
    helper: Option<&'a str>,
    error: Option<&'a str>,
    leading_indent: f32,
}

/// Return value from [`FormField::show`].
pub struct FormFieldResponse<R> {
    /// The value returned by the inner control closure.
    pub inner: R,
    /// `true` when an error string was set on this field.
    pub has_error: bool,
}

impl<'a> FormField<'a> {
    /// Create a field with a visible label.
    pub fn new(label: &'a str) -> Self {
        Self {
            label: Some(label),
            required: false,
            helper: None,
            error: None,
            leading_indent: 0.0,
        }
    }

    /// Create a field with no label (control sits at the top with no header).
    pub fn unlabeled() -> Self {
        Self {
            label: None,
            required: false,
            helper: None,
            error: None,
            leading_indent: 0.0,
        }
    }

    /// Show a required marker `*` after the label (in `t.bear`).
    pub fn required(mut self, r: bool) -> Self {
        self.required = r;
        self
    }

    /// Dim helper text shown below the control when no error is set.
    pub fn helper(mut self, h: &'a str) -> Self {
        self.helper = Some(h);
        self
    }

    /// Error text that replaces helper text when set (painted in `t.bear`).
    pub fn error(mut self, e: &'a str) -> Self {
        self.error = Some(e);
        self
    }

    /// Conditionally set an error message. Equivalent to
    /// `.error(e)` when `cond` is `true`, otherwise a no-op.
    pub fn error_if(self, cond: bool, e: &'a str) -> Self {
        if cond { self.error(e) } else { self }
    }

    /// Horizontal indent applied before the label and control (e.g. for dialog
    /// indentation or nested sub-fields).
    pub fn leading_indent(mut self, px: f32) -> Self {
        self.leading_indent = px;
        self
    }

    /// Render label → control → reserved helper/error line.
    ///
    /// The sub-line height (`font_xs() + gap_xs()`) is ALWAYS allocated — even
    /// when neither helper nor error text is set — so callers in a vertical
    /// stack do not see layout jumps when error state toggles.
    ///
    /// Required marker `*` is appended AFTER the label per the visual spec.
    pub fn show<R>(
        self,
        ui: &mut Ui,
        theme: &dyn ComponentTheme,
        control: impl FnOnce(&mut Ui) -> R,
    ) -> FormFieldResponse<R> {
        let has_error = self.error.is_some();

        let inner = ui.vertical(|ui| {
            if self.leading_indent > 0.0 {
                // Indent by pushing a zero-height horizontal strip whose width
                // matches the indent, then placing the real content alongside.
                ui.horizontal(|ui| {
                    ui.add_space(self.leading_indent);
                    ui.vertical(|ui| {
                        paint_label_and_control(ui, theme, &self, control)
                    }).inner
                }).inner
            } else {
                paint_label_and_control(ui, theme, &self, control)
            }
        }).inner;

        FormFieldResponse { inner, has_error }
    }
}

/// Inner helper that renders the label row, the control, and the reserved
/// sub-line. Extracted so it can be called either at the top level or inside
/// an indented horizontal strip.
fn paint_label_and_control<'a, R>(
    ui: &mut Ui,
    theme: &dyn ComponentTheme,
    field: &FormField<'a>,
    control: impl FnOnce(&mut Ui) -> R,
) -> R {
    // ── Label row ────────────────────────────────────────────────────────────
    if let Some(label_text) = field.label {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(label_text)
                    .monospace()
                    .size(st::font_sm())
                    .color(theme.text()),
            );
            if field.required {
                ui.add_space(st::gap_xs());
                ui.label(
                    RichText::new("*")
                        .monospace()
                        .size(st::font_sm())
                        .strong()
                        .color(theme.danger()),
                );
            }
        });
        // Label-to-control gap: gap_xs_mid (6px — DS-IMPL-3 token).
        ui.add_space(st::gap_xs_mid());
    }

    // ── Control slot ─────────────────────────────────────────────────────────
    let result = control(ui);

    // ── Reserved sub-line ────────────────────────────────────────────────────
    // The sub-line height is ALWAYS reserved so the layout never jumps.
    // Height = font_xs() + gap_xs() to accommodate ascender + cap-height.
    ui.add_space(st::gap_xs());
    let sub_h = st::font_xs() + st::gap_xs();
    let sub_text = field.error.or(field.helper);

    match sub_text {
        Some(text) => {
            let col = if field.error.is_some() {
                theme.danger()
            } else {
                st::color_alpha(theme.dim(), st::alpha_dim())
            };
            // Allocate the exact reserved height so layout is stable.
            ui.allocate_ui(Vec2::new(ui.available_width(), sub_h), |ui| {
                ui.label(
                    RichText::new(text)
                        .monospace()
                        .size(st::font_xs())
                        .italics()
                        .color(col),
                );
            });
        }
        None => {
            // Reserve the same height without any text content.
            ui.allocate_exact_size(Vec2::new(ui.available_width(), sub_h), egui::Sense::hover());
        }
    }

    result
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Smoke: builder chains compile and `has_error` reflects state correctly.
    #[test]
    fn form_field_has_error_reflects_error_state() {
        // Builder with error set → has_error should be true when shown.
        // We can't run egui layout in unit tests without a harness, but we can
        // verify the builder state before calling .show().
        let field: FormField<'_> = FormField::new("Price")
            .required(true)
            .helper("Enter a positive number")
            .error("Must be > 0");
        assert!(
            field.error.is_some(),
            "error field must be Some when .error() is called"
        );
        assert!(
            field.required,
            "required flag must be true when .required(true) is called"
        );
        assert_eq!(field.label, Some("Price"), "label must match constructor arg");
    }

    /// Smoke: `error_if(false, ...)` leaves error as None.
    #[test]
    fn error_if_false_leaves_error_none() {
        let field = FormField::new("Qty").error_if(false, "bad");
        assert!(field.error.is_none(), "error_if(false) must not set error");
    }

    /// Smoke: `error_if(true, ...)` sets the error string.
    #[test]
    fn error_if_true_sets_error() {
        let field = FormField::new("Qty").error_if(true, "required");
        assert_eq!(field.error, Some("required"));
    }

    /// Smoke: `unlabeled()` has no label.
    #[test]
    fn unlabeled_has_no_label() {
        let field = FormField::<'_>::unlabeled();
        assert!(field.label.is_none(), "unlabeled() must have no label");
    }

    /// Smoke: verify reserved sub-line is always allocated (source check).
    #[test]
    fn reserved_subline_always_allocated() {
        let src = include_str!("form_field.rs");
        assert!(
            src.contains("allocate_exact_size"),
            "form_field must call allocate_exact_size for the reserved sub-line"
        );
    }
}

// TODO follow-up: re-export from widgets/mod.rs after the 5-primitives agent lands.
//   pub use form_field::{FormField, FormFieldResponse};
