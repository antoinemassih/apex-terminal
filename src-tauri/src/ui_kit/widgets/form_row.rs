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
//!
//! // Password + hint passed through FormRowCx:
//! FormRow::new("Auth Token")
//!     .password(true)
//!     .hint("leave blank if not required")
//!     .show_with_cx(ui, theme, |ui, cx| {
//!         let mut input = Input::new(&mut buf);
//!         if cx.password { input = input.password(true); }
//!         if let Some(h) = cx.hint { input = input.placeholder(h); }
//!         input.show(ui, theme)
//!     });
//! ```
//!
//! Layout: `[leading_space] [label gutter (label_width)] [inner_pad] [body]`
//! followed by an optional helper / error line below the body.
//!
//! When `label_top(true)` the label is rendered on its own row above the body
//! at full width instead.

use egui::{Color32, Ui, Vec2};

use super::theme::ComponentTheme;
use crate::ui_kit::sx::{palette_ct, Tone};
use crate::ui_kit::tokens as st;
use crate::ui_kit::text_style::TextStyle;

/// Controls which side of the gutter area the body aligns to.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub enum FormRowAlign {
    #[default]
    Left,
    Right,
}

/// Row-level hints threaded to the body closure via [`FormRow::show_with_cx`].
///
/// Body primitives can opt-in to honour these: apply `.password(cx.password)`
/// and `.placeholder(cx.hint.unwrap_or(""))` to an [`Input`].
#[derive(Copy, Clone, Debug, Default)]
pub struct FormRowCx<'a> {
    /// When `true`, the body input should mask its content (password mode).
    pub password: bool,
    /// Optional placeholder hint text the body input should display.
    pub hint: Option<&'a str>,
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
    /// Extra vertical space added above the row (ported from legacy `.margins(top, _)`).
    margin_top: f32,
    /// Extra vertical space added below the row (ported from legacy `.margins(_, bottom)`).
    margin_bottom: f32,
    /// When `true` the label text is laid out left-to-right inside the gutter
    /// (settings_panel style) instead of the default right-to-left alignment.
    label_layout_left: bool,
    /// Password-mode hint threaded to the body via [`FormRowCx`].
    password: bool,
    /// Placeholder hint text threaded to the body via [`FormRowCx`].
    hint: Option<&'a str>,
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
            margin_top: 0.0,
            margin_bottom: 0.0,
            label_layout_left: false,
            password: false,
            hint: None,
        }
    }

    /// Width of the label gutter in logical pixels.
    pub fn label_width(mut self, w: f32) -> Self { self.label_width = w; self }
    /// Alias for [`label_width`] — matches the legacy `settings_panel` terminology.
    pub fn gutter(mut self, w: f32) -> Self { self.label_width = w; self }
    /// Helper text shown below the body (muted italic). Overridden by `error`.
    pub fn helper_text(mut self, h: &'a str) -> Self { self.helper = Some(h); self }
    /// Error text shown below the body (bear color). Takes precedence over helper.
    pub fn error(mut self, e: &'a str) -> Self { self.error = Some(e); self }
    /// Append a red `*` after the label.
    pub fn required(mut self, r: bool) -> Self { self.required = r; self }
    /// Body alignment within the area right of the gutter.
    pub fn align(mut self, a: FormRowAlign) -> Self { self.align = a; self }
    /// Alias for [`align`] — matches the legacy `inputs::form::FormRow` API.
    pub fn alignment(mut self, a: FormRowAlign) -> Self { self.align = a; self }
    /// Horizontal pad between gutter and body (or right inset when Right-aligned).
    pub fn inner_pad(mut self, p: f32) -> Self { self.inner_pad = p; self }
    /// Horizontal space before the label gutter (e.g. dialog indent).
    pub fn leading_space(mut self, s: f32) -> Self { self.leading_space = s; self }
    /// Alias for [`leading_space`] — explicit left indent for nested option rows.
    pub fn indent(mut self, s: f32) -> Self { self.leading_space = s; self }
    /// Override the label text color.
    pub fn label_color(mut self, c: Color32) -> Self { self.label_color = Some(c); self }
    /// Render the label on its own row above the control instead of left.
    pub fn label_top(mut self, v: bool) -> Self { self.label_top = v; self }
    /// Custom vertical margins around the row.
    ///
    /// `top` space is inserted before the row; `bottom` space after the row
    /// (and after any sub-text). Mirrors the legacy `.margins(top, bottom)` API.
    pub fn margins(mut self, top: f32, bottom: f32) -> Self {
        self.margin_top = top;
        self.margin_bottom = bottom;
        self
    }
    /// Lay out the label left-to-right inside the gutter (settings_panel style)
    /// instead of the default right-to-left alignment.
    pub fn label_left(mut self, l: bool) -> Self { self.label_layout_left = l; self }
    /// Hint at password mode for the body input — passed through [`FormRowCx`]
    /// when using [`show_with_cx`](Self::show_with_cx).
    pub fn password(mut self, p: bool) -> Self { self.password = p; self }
    /// Placeholder hint text passed through [`FormRowCx`] to the body input
    /// when using [`show_with_cx`](Self::show_with_cx).
    pub fn hint(mut self, h: &'a str) -> Self { self.hint = Some(h); self }

    /// Render the row. The body closure receives only `&mut Ui`.
    ///
    /// Use [`show_with_cx`](Self::show_with_cx) if the body needs to read
    /// `.password` / `.hint` context.
    pub fn show<R, B: FnOnce(&mut Ui) -> R>(
        self,
        ui: &mut Ui,
        theme: &dyn ComponentTheme,
        body: B,
    ) -> R {
        self.show_with_cx(ui, theme, |ui, _cx| body(ui))
    }

    /// Like [`show`](Self::show) but the body receives a [`FormRowCx`] carrying
    /// row-level hints (password mode, placeholder hint text). Body primitives
    /// can opt-in to honour them.
    pub fn show_with_cx<R, B: FnOnce(&mut Ui, FormRowCx<'_>) -> R>(
        self,
        ui: &mut Ui,
        theme: &dyn ComponentTheme,
        body: B,
    ) -> R {
        let label_col = self.label_color.unwrap_or_else(|| palette_ct(theme).base(Tone::Dim));
        let cx = FormRowCx { password: self.password, hint: self.hint };

        let label_layout = if self.label_layout_left {
            egui::Layout::left_to_right(egui::Align::Center)
        } else {
            egui::Layout::right_to_left(egui::Align::Center)
        };

        let render_label = |ui: &mut Ui, required: bool| {
            if required {
                ui.label(
                    TextStyle::MonoSm.as_rich_cascading("*", theme.danger()).strong(),
                );
            }
            ui.label(
                TextStyle::MonoSm.as_rich_cascading(self.label, label_col),
            );
        };

        if self.margin_top > 0.0 { ui.add_space(self.margin_top); }

        let result = if self.label_top {
            // Label on its own row, body full-width below.
            ui.vertical(|ui| {
                if self.leading_space > 0.0 { ui.add_space(self.leading_space); }
                ui.horizontal(|ui| render_label(ui, self.required));
                ui.add_space(st::gap_xs());
                let inner = body(ui, cx);
                render_sub_text(ui, theme, self.helper, self.error,
                    self.leading_space, 0.0);
                inner
            }).inner
        } else {
            let inner = ui.horizontal(|ui| {
                if self.leading_space > 0.0 { ui.add_space(self.leading_space); }
                // Fixed-width label gutter.
                ui.allocate_ui_with_layout(
                    Vec2::new(self.label_width, ui.spacing().interact_size.y),
                    label_layout,
                    |ui| render_label(ui, self.required),
                );
                match self.align {
                    FormRowAlign::Left => {
                        ui.add_space(self.inner_pad);
                        body(ui, cx)
                    }
                    FormRowAlign::Right => {
                        ui.with_layout(
                            egui::Layout::right_to_left(egui::Align::Center),
                            |ui| {
                                ui.add_space(self.inner_pad);
                                body(ui, cx)
                            },
                        ).inner
                    }
                }
            }).inner;
            render_sub_text(ui, theme, self.helper, self.error,
                self.leading_space, self.label_width);
            inner
        };

        if self.margin_bottom > 0.0 { ui.add_space(self.margin_bottom); }

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
                theme.danger()
            } else {
                st::color_alpha(palette_ct(theme).base(Tone::Dim), st::alpha_dim())
            };
            ui.label(
                TextStyle::MonoXs.as_rich_cascading(text, col).italics(),
            );
        });
    }
}
