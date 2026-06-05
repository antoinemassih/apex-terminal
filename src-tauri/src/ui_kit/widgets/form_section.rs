//! Form-layout primitives: FormSection (titled non-bordered group),
//! FieldSet (titled bordered group), FormActions (button bar footer).
//!
//! ```ignore
//! FormSection::new("Account")
//!     .helper("Configure your account details")
//!     .show(ui, theme, |ui| {
//!         FormRow::new("Username").show(ui, theme, |ui| { ... });
//!     });
//!
//! FieldSet::new("Connection")
//!     .show(ui, theme, |ui| { ... });
//!
//! FormActions::new()
//!     .primary("Save", || { /* save */ })
//!     .secondary("Cancel", || { /* cancel */ })
//!     .show(ui, theme);
//! ```

use egui::{CornerRadius, Response, RichText, Stroke, Ui};

use super::theme::ComponentTheme;
use crate::ui_kit::sx::{palette_ct, Tone};
use super::Button;
use super::tokens::{Size, Variant};
use crate::ui_kit::tokens as st;

// ── FormSection ───────────────────────────────────────────────────────────────

/// Non-bordered section group with a bold title and optional helper line.
///
/// Use for logical grouping inside a form where a full border would be too
/// heavy. For a bordered group, use [`FieldSet`] instead.
#[must_use = "FormSection does nothing until `.show(ui, theme, body)` is called"]
pub struct FormSection<'a> {
    title: &'a str,
    helper: Option<&'a str>,
    spacing: f32,
}

impl<'a> FormSection<'a> {
    pub fn new(title: &'a str) -> Self {
        Self { title, helper: None, spacing: st::gap_md() }
    }

    /// Dim italic helper text rendered beneath the title.
    pub fn helper(mut self, h: &'a str) -> Self { self.helper = Some(h); self }
    /// Override the gap between the header block and the body.
    pub fn spacing(mut self, s: f32) -> Self { self.spacing = s; self }

    pub fn show<R, B: FnOnce(&mut Ui) -> R>(
        self,
        ui: &mut Ui,
        theme: &dyn ComponentTheme,
        body: B,
    ) -> R {
        ui.label(
            RichText::new(self.title)
                .monospace()
                .size(st::font_md())
                .strong()
                .color(palette_ct(theme).base(Tone::Text)),
        );
        if let Some(h) = self.helper {
            ui.label(
                RichText::new(h)
                    .monospace()
                    .size(st::font_xs())
                    .italics()
                    .color(st::color_alpha(palette_ct(theme).base(Tone::Dim), st::alpha_dim())),
            );
        }
        ui.add_space(self.spacing);
        body(ui)
    }
}

// ── FieldSet ──────────────────────────────────────────────────────────────────

/// Bordered group with an optional inset title — like an HTML `<fieldset>`.
///
/// Use when you want to visually separate a cluster of related controls with a
/// hairline border. For unbordered grouping, use [`FormSection`].
#[must_use = "FieldSet does nothing until `.show(ui, theme, body)` is called"]
pub struct FieldSet<'a> {
    title: Option<&'a str>,
    inner_margin: f32,
}

impl<'a> FieldSet<'a> {
    pub fn new(title: &'a str) -> Self {
        Self { title: Some(title), inner_margin: st::gap_lg() }
    }

    /// Fieldset with no title label.
    pub fn untitled() -> Self {
        Self { title: None, inner_margin: st::gap_lg() }
    }

    /// Override inner padding on all sides.
    pub fn inner_margin(mut self, m: f32) -> Self { self.inner_margin = m; self }

    pub fn show<R, B: FnOnce(&mut Ui) -> R>(
        self,
        ui: &mut Ui,
        theme: &dyn ComponentTheme,
        body: B,
    ) -> R {
        let border_col = st::color_alpha(palette_ct(theme).base(Tone::Border), st::alpha_muted());
        let frame = egui::Frame::NONE
            .stroke(Stroke::new(st::stroke_thin(), border_col))
            .corner_radius(CornerRadius::same(st::radius_sm() as u8))
            .inner_margin(egui::Margin::same(self.inner_margin as i8));

        frame.show(ui, |ui| {
            if let Some(title) = self.title {
                ui.label(
                    RichText::new(title)
                        .monospace()
                        .size(st::font_sm())
                        .strong()
                        .color(palette_ct(theme).base(Tone::Text)),
                );
                ui.add_space(st::gap_sm());
            }
            body(ui)
        }).inner
    }
}

// ── FormActions ───────────────────────────────────────────────────────────────

/// Footer button bar — primary action right-aligned, secondary action left
/// of it.
///
/// ```ignore
/// FormActions::new()
///     .primary("Save", || { /* on save */ })
///     .secondary("Cancel", || { /* on cancel */ })
///     .show(ui, theme);
/// ```
#[must_use = "FormActions does nothing until `.show(ui, theme)` is called"]
pub struct FormActions<'a> {
    primary_label: Option<&'a str>,
    primary_action: Option<Box<dyn FnMut() + 'a>>,
    secondary_label: Option<&'a str>,
    secondary_action: Option<Box<dyn FnMut() + 'a>>,
    primary_disabled: bool,
}

impl<'a> FormActions<'a> {
    pub fn new() -> Self {
        Self {
            primary_label: None,
            primary_action: None,
            secondary_label: None,
            secondary_action: None,
            primary_disabled: false,
        }
    }

    /// Set the primary (right-most) action button.
    pub fn primary<F: FnMut() + 'a>(mut self, label: &'a str, action: F) -> Self {
        self.primary_label = Some(label);
        self.primary_action = Some(Box::new(action));
        self
    }

    /// Set the secondary action button (rendered left of the primary).
    pub fn secondary<F: FnMut() + 'a>(mut self, label: &'a str, action: F) -> Self {
        self.secondary_label = Some(label);
        self.secondary_action = Some(Box::new(action));
        self
    }

    /// Disable (dim) the primary button.
    pub fn primary_disabled(mut self, d: bool) -> Self { self.primary_disabled = d; self }

    /// Render the button bar and return the horizontal strip's `Response`.
    pub fn show(mut self, ui: &mut Ui, theme: &dyn ComponentTheme) -> Response {
        ui.horizontal(|ui| {
            // Push buttons to the right.
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = st::gap_sm();

                // Primary — right-most.
                if let Some(label) = self.primary_label {
                    let resp = Button::new(label)
                        .variant(Variant::Primary)
                        .size(Size::Md)
                        .disabled(self.primary_disabled)
                        .show(ui, theme);
                    if resp.clicked() {
                        if let Some(ref mut f) = self.primary_action { f(); }
                    }
                }

                // Secondary — left of primary.
                if let Some(label) = self.secondary_label {
                    let resp = Button::new(label)
                        .variant(Variant::Secondary)
                        .size(Size::Md)
                        .show(ui, theme);
                    if resp.clicked() {
                        if let Some(ref mut f) = self.secondary_action { f(); }
                    }
                }
            });
        }).response
    }
}

impl<'a> Default for FormActions<'a> {
    fn default() -> Self { Self::new() }
}
