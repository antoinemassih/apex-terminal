//! FormRow — horizontal "label : control" row with fixed-width label gutter.
//!
//! Ported from `src/chart/renderer/ui/inputs/form.rs` to ui_kit so all
//! callers can use it without depending on chart_renderer internals.
//!
//! ```ignore
//! FormRow::new("Username")
//!     .required(true)
//!     .helper_text("3-20 characters")
//!     .show(ui, theme, |ui| {
//!         ui.add(Input::new(&mut username));
//!     });
//!
//! FormRow::new("Email")
//!     .error("Invalid email format")
//!     .show(ui, theme, |ui| { ... });
//! ```
//!
//! Layout: `[leading_space] [label gutter (label_width)] [inner_pad] [body]`
//! followed by an optional helper / error line below the body.
//!
//! When `label_top(true)` the label is rendered on its own row above the body
//! at full width instead.

use egui::{Color32, RichText, Ui, Vec2};

use super::theme::ComponentTheme;
use crate::chart::renderer::ui::style as st;

/// Controls which side of the gutter area the body aligns to.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub enum FormRowAlign {
    #[default]
    Left,
    Right,
}

/// Horizontal label + control row.
#[must_use = "FormRow does nothing until `.show(ui, theme, body)` is called"]
pub struct FormRow<'a> {
    label: &'a str,
    label_width: f32,
    helper: Option<&'a str>,
    error: Option<&'a str>,
    required: bool,
    align: FormRowAlign,
    inner_pad: f32,
    leading_space: f32,
    label_color: Option<Color32>,
    label_top: bool,
}

impl<'a> FormRow<'a> {
    pub fn new(label: &'a str) -> Self {
        Self {
            label,
            label_width: 120.0,
            helper: None,
            error: None,
            required: false,
            align: FormRowAlign::Left,
            inner_pad: st::gap_sm(),
            leading_space: 0.0,
            label_color: None,
            label_top: false,
        }
    }

    /// Width of the label gutter in logical pixels.
    pub fn label_width(mut self, w: f32) -> Self { self.label_width = w; self }
    /// Helper text shown below the body (muted italic). Overridden by `error`.
    pub fn helper_text(mut self, h: &'a str) -> Self { self.helper = Some(h); self }
    /// Error text shown below the body (bear color). Takes precedence over helper.
    pub fn error(mut self, e: &'a str) -> Self { self.error = Some(e); self }
    /// Append a red `*` after the label.
    pub fn required(mut self, r: bool) -> Self { self.required = r; self }
    /// Body alignment within the area right of the gutter.
    pub fn align(mut self, a: FormRowAlign) -> Self { self.align = a; self }
    /// Horizontal pad between gutter and body (or right inset when Right-aligned).
    pub fn inner_pad(mut self, p: f32) -> Self { self.inner_pad = p; self }
    /// Horizontal space before the label gutter (e.g. dialog indent).
    pub fn leading_space(mut self, s: f32) -> Self { self.leading_space = s; self }
    /// Override the label text color.
    pub fn label_color(mut self, c: Color32) -> Self { self.label_color = Some(c); self }
    /// Render the label on its own row above the control instead of left.
    pub fn label_top(mut self, v: bool) -> Self { self.label_top = v; self }

    pub fn show<R, B: FnOnce(&mut Ui) -> R>(
        self,
        ui: &mut Ui,
        theme: &dyn ComponentTheme,
        body: B,
    ) -> R {
        let label_col = self.label_color.unwrap_or_else(|| theme.dim());

        let render_label = |ui: &mut Ui, required: bool| {
            if required {
                ui.label(
                    RichText::new("*")
                        .monospace()
                        .size(st::font_sm())
                        .strong()
                        .color(theme.bear()),
                );
            }
            ui.label(
                RichText::new(self.label)
                    .monospace()
                    .size(st::font_sm())
                    .color(label_col),
            );
        };

        let result = if self.label_top {
            // Label on its own row, body full-width below.
            ui.vertical(|ui| {
                if self.leading_space > 0.0 { ui.add_space(self.leading_space); }
                ui.horizontal(|ui| render_label(ui, self.required));
                ui.add_space(st::gap_xs());
                let inner = body(ui);
                render_sub_text(ui, theme, self.helper, self.error,
                    self.leading_space, 0.0);
                inner
            }).inner
        } else {
            let inner = ui.horizontal(|ui| {
                if self.leading_space > 0.0 { ui.add_space(self.leading_space); }
                // Fixed-width label gutter, right-aligned by default.
                ui.allocate_ui_with_layout(
                    Vec2::new(self.label_width, ui.spacing().interact_size.y),
                    egui::Layout::right_to_left(egui::Align::Center),
                    |ui| render_label(ui, self.required),
                );
                match self.align {
                    FormRowAlign::Left => {
                        ui.add_space(self.inner_pad);
                        body(ui)
                    }
                    FormRowAlign::Right => {
                        ui.with_layout(
                            egui::Layout::right_to_left(egui::Align::Center),
                            |ui| {
                                ui.add_space(self.inner_pad);
                                body(ui)
                            },
                        ).inner
                    }
                }
            }).inner;
            render_sub_text(ui, theme, self.helper, self.error,
                self.leading_space, self.label_width);
            inner
        };

        result
    }
}

/// Render helper or error sub-text below the row.
fn render_sub_text(
    ui: &mut Ui,
    theme: &dyn ComponentTheme,
    helper: Option<&str>,
    error: Option<&str>,
    leading: f32,
    label_w: f32,
) {
    let sub = error.or(helper);
    if let Some(text) = sub {
        let is_error = error.is_some();
        ui.horizontal(|ui| {
            let offset = leading + label_w + st::gap_sm();
            if offset > 0.0 { ui.add_space(offset); }
            let col = if is_error {
                theme.bear()
            } else {
                st::color_alpha(theme.dim(), st::alpha_dim())
            };
            ui.label(
                RichText::new(text)
                    .monospace()
                    .size(st::font_xs())
                    .italics()
                    .color(col),
            );
        });
    }
}
