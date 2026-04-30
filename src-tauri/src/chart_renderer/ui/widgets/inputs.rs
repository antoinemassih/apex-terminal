//! Builder + impl Widget primitives — inputs family.
//! See ui/widgets/mod.rs for the rationale.

#![allow(dead_code, unused_imports)]

use egui::{Color32, Response, RichText, Sense, Stroke, Ui, Vec2, Widget};
use super::super::style::*;
use super::foundation::{InputShell, InputState, InputVariant, Size as FSize, Radius as FRadius};

// ─── TextInput ────────────────────────────────────────────────────────────────

/// Single-line text input with placeholder + optional width/font-size.
/// Replaces `components_extra::text_input_field(...)`.
///
/// ```ignore
/// let resp = TextInput::new(&mut buf).placeholder("Search…").width(200.0).show(ui);
/// ```
#[must_use = "TextInput must be rendered via `.show(ui)`"]
pub struct TextInput<'a, 'b> {
    buffer: &'b mut String,
    placeholder: &'a str,
    width: Option<f32>,
    font_size: f32,
    accent: Option<Color32>,
    dim: Option<Color32>,
    border: Option<Color32>,
    theme: Option<&'a super::super::super::gpu::Theme>,
    variant: InputVariant,
}

impl<'a, 'b> TextInput<'a, 'b> {
    pub fn new(buffer: &'b mut String) -> Self {
        Self {
            buffer,
            placeholder: "",
            width: None,
            font_size: font_sm(),
            accent: None,
            dim: None,
            border: None,
            theme: None,
            variant: InputVariant::Default,
        }
    }
    pub fn placeholder(mut self, p: &'a str) -> Self { self.placeholder = p; self }
    pub fn width(mut self, w: f32) -> Self { self.width = Some(w); self }
    pub fn font_size(mut self, s: f32) -> Self { self.font_size = s; self }
    pub fn palette(mut self, accent: Color32, _bear: Color32, dim: Color32) -> Self {
        self.accent = Some(accent); self.dim = Some(dim); self
    }
    pub fn theme(mut self, t: &'a super::super::super::gpu::Theme) -> Self {
        self.theme = Some(t);
        self.accent = Some(t.accent);
        self.dim = Some(t.dim);
        self
    }
    pub fn border(mut self, c: Color32) -> Self { self.border = Some(c); self }
    pub(crate) fn variant_internal(mut self, v: InputVariant) -> Self { self.variant = v; self }

    pub fn show(self, ui: &mut Ui) -> Response {
        let id = ui.next_auto_id();
        let focused = ui.memory(|m| m.has_focus(id));
        let desired_width = self.width.unwrap_or_else(|| ui.available_width());
        let font_size = self.font_size;
        let placeholder = self.placeholder;
        let buffer = self.buffer;

        // Compose InputShell when a theme is available — this is the foundation path.
        if let Some(theme) = self.theme {
            let state = if focused { InputState::Focused } else { InputState::Default };
            let mut resp_opt: Option<Response> = None;
            InputShell::new(theme)
                .variant(self.variant)
                .size(FSize::Sm)
                .radius(FRadius::Sm)
                .state(state)
                .body(|ui| {
                    let te = egui::TextEdit::singleline(buffer)
                        .id(id)
                        .hint_text(placeholder)
                        .font(egui::FontSelection::FontId(egui::FontId::monospace(font_size)))
                        .frame(false)
                        .desired_width(desired_width);
                    resp_opt = Some(ui.add(te));
                })
                .show(ui);
            return resp_opt.unwrap_or_else(|| ui.label(""));
        }

        // Fallback: legacy hand-rolled frame for palette()-only callers.
        let accent = self.accent.unwrap_or_else(|| Color32::from_rgb(120, 140, 220));
        let border = self.border.unwrap_or_else(|| Color32::from_rgb(80, 80, 90));
        let border_color = if focused {
            color_alpha(accent, alpha_active())
        } else {
            color_alpha(border, alpha_line())
        };
        let frame = egui::Frame::NONE
            .stroke(Stroke::new(1.0, border_color))
            .inner_margin(gap_sm())
            .corner_radius(radius_sm());

        let mut resp_opt: Option<Response> = None;
        frame.show(ui, |ui| {
            let te = egui::TextEdit::singleline(buffer)
                .id(id)
                .hint_text(placeholder)
                .font(egui::FontSelection::FontId(egui::FontId::monospace(font_size)))
                .frame(false)
                .desired_width(desired_width);
            resp_opt = Some(ui.add(te));
        });
        resp_opt.unwrap_or_else(|| ui.label(""))
    }
}

// ─── NumericInput ─────────────────────────────────────────────────────────────

/// Numeric (f32) text input that parses on focus-loss.
/// Replaces `components_extra::numeric_input_field(...)`.
///
/// ```ignore
/// let resp = NumericInput::new(&mut my_f32).placeholder("0.0").show(ui);
/// ```
#[must_use = "NumericInput must be rendered via `.show(ui)`"]
pub struct NumericInput<'a, 'b> {
    value: &'b mut f32,
    placeholder: &'a str,
    width: Option<f32>,
    font_size: f32,
    accent: Option<Color32>,
    dim: Option<Color32>,
    border: Option<Color32>,
}

impl<'a, 'b> NumericInput<'a, 'b> {
    pub fn new(value: &'b mut f32) -> Self {
        Self {
            value,
            placeholder: "",
            width: None,
            font_size: font_sm(),
            accent: None,
            dim: None,
            border: None,
        }
    }
    pub fn placeholder(mut self, p: &'a str) -> Self { self.placeholder = p; self }
    pub fn width(mut self, w: f32) -> Self { self.width = Some(w); self }
    pub fn font_size(mut self, s: f32) -> Self { self.font_size = s; self }
    pub fn palette(mut self, accent: Color32, _bear: Color32, dim: Color32) -> Self {
        self.accent = Some(accent); self.dim = Some(dim); self
    }
    pub fn theme(self, t: &super::super::super::gpu::Theme) -> Self {
        self.palette(t.accent, t.bear, t.dim)
    }
    pub fn border(mut self, c: Color32) -> Self { self.border = Some(c); self }

    pub fn show(self, ui: &mut Ui) -> Response {
        let accent = self.accent.unwrap_or_else(|| Color32::from_rgb(120, 140, 220));
        let dim = self.dim.unwrap_or_else(|| Color32::from_rgb(100, 100, 110));
        let border = self.border.unwrap_or_else(|| Color32::from_rgb(80, 80, 90));

        let buf_id = ui.next_auto_id();
        let value = self.value;
        let mut buf = ui.memory_mut(|m| {
            m.data.get_temp_mut_or_insert_with::<String>(buf_id, || value.to_string()).clone()
        });
        let resp = TextInput::new(&mut buf)
            .placeholder(self.placeholder)
            .font_size(self.font_size)
            .palette(accent, Color32::from_rgb(220, 80, 90), dim)
            .border(border)
            .variant_internal(InputVariant::Numeric);
        let resp = if let Some(w) = self.width { resp.width(w) } else { resp };
        let resp = resp.show(ui);

        ui.memory_mut(|m| {
            *m.data.get_temp_mut_or_insert_with::<String>(buf_id, || value.to_string()) = buf.clone();
        });
        if resp.lost_focus() {
            if let Ok(parsed) = buf.trim().parse::<f32>() {
                *value = parsed;
            }
            ui.memory_mut(|m| {
                *m.data.get_temp_mut_or_insert_with::<String>(buf_id, || value.to_string()) =
                    value.to_string();
            });
        }
        resp
    }
}

// ─── Stepper ──────────────────────────────────────────────────────────────────

/// [-] value [+] stepper. Replaces both `compact_stepper` and `numeric_stepper`.
/// Returns a Response; call `.show(ui)` — also check `.changed()` or use the
/// returned delta via `Stepper::delta_from(resp, ui)`. The value is mutated
/// in-place when buttons are clicked.
///
/// ```ignore
/// Stepper::new(&mut my_u32).range(1, 100).step(1).theme(t).show(ui);
/// ```
#[must_use = "Stepper must be rendered via `.show(ui)`"]
pub struct Stepper<'b> {
    value: &'b mut u32,
    min: u32,
    max: u32,
    step: u32,
    compact: bool,
    accent: Option<Color32>,
    dim: Option<Color32>,
    border: Option<Color32>,
}

impl<'b> Stepper<'b> {
    pub fn new(value: &'b mut u32) -> Self {
        Self {
            value,
            min: u32::MIN,
            max: u32::MAX,
            step: 1,
            compact: false,
            accent: None,
            dim: None,
            border: None,
        }
    }
    pub fn range(mut self, min: u32, max: u32) -> Self { self.min = min; self.max = max; self }
    pub fn step(mut self, s: u32) -> Self { self.step = s; self }
    /// Use 14x14 buttons instead of 18x18 (mirrors `compact_stepper`).
    pub fn compact(mut self) -> Self { self.compact = true; self }
    pub fn palette(mut self, accent: Color32, _bear: Color32, dim: Color32) -> Self {
        self.accent = Some(accent); self.dim = Some(dim); self
    }
    pub fn theme(self, t: &super::super::super::gpu::Theme) -> Self {
        self.palette(t.accent, t.bear, t.dim)
    }
    pub fn border(mut self, c: Color32) -> Self { self.border = Some(c); self }

    pub fn show(self, ui: &mut Ui) -> Response {
        let accent = self.accent.unwrap_or_else(|| Color32::from_rgb(120, 140, 220));
        let dim = self.dim.unwrap_or_else(|| Color32::from_rgb(100, 100, 110));
        let border = self.border.unwrap_or_else(|| Color32::from_rgb(80, 80, 90));
        let compact = self.compact;
        let step = self.step;
        let min = self.min;
        let max = self.max;
        let value = self.value;

        let btn_size = if compact { 14.0_f32 } else { 18.0_f32 };
        let font_size = if compact { font_xs() } else { font_sm() };

        let st = current();
        let cr = egui::CornerRadius::same(st.r_xs as u8);
        let stroke = if st.hairline_borders {
            Stroke::new(st.stroke_std, color_alpha(border, alpha_strong()))
        } else {
            Stroke::new(st.stroke_thin, color_alpha(border, alpha_muted()))
        };

        let mut resp_out: Option<Response> = None;
        ui.horizontal(|ui| {
            let prev = ui.spacing().item_spacing.x;
            ui.spacing_mut().item_spacing.x = gap_xs();

            let mut mk_btn = |ui: &mut Ui, sym: &str| -> Response {
                let txt = if compact {
                    RichText::new(sym).monospace().size(font_size).color(dim)
                } else {
                    RichText::new(sym).monospace().size(font_size).strong().color(dim)
                };
                ui.add(
                    egui::Button::new(txt)
                        .fill(Color32::TRANSPARENT)
                        .stroke(stroke)
                        .corner_radius(cr)
                        .min_size(Vec2::new(btn_size, btn_size)),
                )
            };

            let dec = mk_btn(ui, "-");
            let label_color = if compact { dim } else { accent };
            let label_resp = ui.label(
                RichText::new(value.to_string())
                    .monospace()
                    .size(font_size)
                    .strong()
                    .color(label_color),
            );
            let inc = mk_btn(ui, "+");

            if dec.clicked() {
                *value = value.saturating_sub(step).max(min);
            }
            if inc.clicked() {
                *value = (*value + step).min(max);
            }

            resp_out = Some(dec.union(inc).union(label_resp));
            ui.spacing_mut().item_spacing.x = prev;
        });
        resp_out.unwrap_or_else(|| ui.label(""))
    }
}

// ─── ToggleRow ────────────────────────────────────────────────────────────────

/// Settings-style row: label left, checkbox right.
/// Replaces `components_extra::toggle_row(...)`.
///
/// ```ignore
/// ToggleRow::new("Enable feature", &mut flag).theme(t).show(ui);
/// ```
#[must_use = "ToggleRow must be rendered via `.show(ui)`"]
pub struct ToggleRow<'a, 'b> {
    label: &'a str,
    value: &'b mut bool,
    label_color: Option<Color32>,
}

impl<'a, 'b> ToggleRow<'a, 'b> {
    pub fn new(label: &'a str, value: &'b mut bool) -> Self {
        Self { label, value, label_color: None }
    }
    pub fn label_color(mut self, c: Color32) -> Self { self.label_color = Some(c); self }
    pub fn palette(mut self, _accent: Color32, _bear: Color32, dim: Color32) -> Self {
        self.label_color = Some(dim); self
    }
    pub fn theme(self, t: &super::super::super::gpu::Theme) -> Self {
        self.palette(t.accent, t.bear, t.dim)
    }

    pub fn show(self, ui: &mut Ui) -> Response {
        let label_color = self.label_color.unwrap_or_else(|| Color32::from_rgb(140, 140, 150));
        let label = self.label;
        let value = self.value;

        let mut resp = ui.allocate_response(Vec2::ZERO, Sense::hover());
        ui.horizontal(|ui| {
            let s = style_label_case(label);
            ui.label(
                RichText::new(s)
                    .monospace()
                    .size(font_sm())
                    .color(label_color),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                resp = ui.checkbox(value, "");
            });
        });
        resp
    }
}

// ─── SearchInput ──────────────────────────────────────────────────────────────

/// Search input with magnifier glyph prefix.
/// Replaces `components_extra::search_input(...)`.
///
/// ```ignore
/// SearchInput::new(&mut query).placeholder("Search symbols…").theme(t).show(ui);
/// ```
#[must_use = "SearchInput must be rendered via `.show(ui)`"]
pub struct SearchInput<'a, 'b> {
    buffer: &'b mut String,
    placeholder: &'a str,
    accent: Option<Color32>,
    dim: Option<Color32>,
    border: Option<Color32>,
    theme: Option<&'a super::super::super::gpu::Theme>,
}

impl<'a, 'b> SearchInput<'a, 'b> {
    pub fn new(buffer: &'b mut String) -> Self {
        Self {
            buffer,
            placeholder: "",
            accent: None,
            dim: None,
            border: None,
            theme: None,
        }
    }
    pub fn placeholder(mut self, p: &'a str) -> Self { self.placeholder = p; self }
    pub fn palette(mut self, accent: Color32, _bear: Color32, dim: Color32) -> Self {
        self.accent = Some(accent); self.dim = Some(dim); self
    }
    pub fn theme(mut self, t: &'a super::super::super::gpu::Theme) -> Self {
        self.theme = Some(t);
        self.accent = Some(t.accent);
        self.dim = Some(t.dim);
        self
    }
    pub fn border(mut self, c: Color32) -> Self { self.border = Some(c); self }

    pub fn show(self, ui: &mut Ui) -> Response {
        let accent = self.accent.unwrap_or_else(|| Color32::from_rgb(120, 140, 220));
        let dim = self.dim.unwrap_or_else(|| Color32::from_rgb(100, 100, 110));
        let border = self.border.unwrap_or_else(|| Color32::from_rgb(80, 80, 90));

        let avail = ui.available_width();
        let buffer = self.buffer;
        let placeholder = self.placeholder;

        // Compose InputShell when a theme is available (foundation path).
        if let Some(theme) = self.theme {
            let id = ui.next_auto_id();
            let focused = ui.memory(|m| m.has_focus(id));
            let state = if focused { InputState::Focused } else { InputState::Default };
            let mut resp_out: Option<egui::Response> = None;
            InputShell::new(theme)
                .variant(InputVariant::Search)
                .size(FSize::Sm)
                .radius(FRadius::Sm)
                .state(state)
                .body(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("\u{1F50D}").size(font_sm()).color(dim));
                        let edit = egui::TextEdit::singleline(buffer)
                            .id(id)
                            .desired_width(avail - 36.0)
                            .hint_text(RichText::new(placeholder).color(color_alpha(dim, alpha_muted())))
                            .text_color(accent)
                            .frame(false);
                        resp_out = Some(ui.add(edit));
                    });
                })
                .show(ui);
            return resp_out.expect("SearchInput response");
        }

        // Fallback for palette()-only callers.
        let st = current();
        let frame = egui::Frame::NONE
            .fill(Color32::TRANSPARENT)
            .corner_radius(r_sm_cr())
            .stroke(if st.hairline_borders {
                Stroke::new(st.stroke_std, color_alpha(border, alpha_strong()))
            } else {
                Stroke::new(st.stroke_thin, color_alpha(border, alpha_muted()))
            })
            .inner_margin(egui::Margin {
                left: gap_md() as i8,
                right: gap_md() as i8,
                top: gap_xs() as i8,
                bottom: gap_xs() as i8,
            });

        let mut resp_out: Option<egui::Response> = None;
        frame.show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("\u{1F50D}").size(font_sm()).color(dim));
                let edit = egui::TextEdit::singleline(buffer)
                    .desired_width(avail - 36.0)
                    .hint_text(RichText::new(placeholder).color(color_alpha(dim, alpha_muted())))
                    .text_color(accent)
                    .frame(false);
                resp_out = Some(ui.add(edit));
            });
        });
        resp_out.expect("SearchInput response")
    }
}

// ─── CompactStepper ───────────────────────────────────────────────────────────

/// Builder for `components_extra::compact_stepper(ui, value, dim, border) -> i32`.
///
/// Returns the click delta (`-1`, `0`, `+1`). Display-only — the caller owns
/// the value and applies the delta.  Use [`Stepper`] for in-place mutation
/// over a `&mut u32`.
///
/// ```ignore
/// let delta = CompactStepper::new(&value_str).theme(t).show(ui);
/// if delta != 0 { my_value = (my_value as i32 + delta).max(0) as u32; }
/// ```
#[must_use = "CompactStepper must be rendered via `.show(ui)`"]
pub struct CompactStepper<'a> {
    value: &'a str,
    dim: Option<Color32>,
    border: Option<Color32>,
}

impl<'a> CompactStepper<'a> {
    pub fn new(value: &'a str) -> Self {
        Self { value, dim: None, border: None }
    }
    pub fn dim(mut self, c: Color32) -> Self { self.dim = Some(c); self }
    pub fn border(mut self, c: Color32) -> Self { self.border = Some(c); self }
    pub fn palette(mut self, _accent: Color32, _bear: Color32, dim: Color32) -> Self {
        self.dim = Some(dim); self
    }
    pub fn theme(self, t: &super::super::super::gpu::Theme) -> Self {
        Self {
            value: self.value,
            dim: Some(t.dim),
            border: Some(t.toolbar_border),
        }
    }

    /// Body mirrors `components_extra::compact_stepper` byte-for-byte.
    pub fn show(self, ui: &mut Ui) -> i32 {
        let dim = self.dim.unwrap_or_else(|| Color32::from_rgb(100, 100, 110));
        let border = self.border.unwrap_or_else(|| Color32::from_rgb(80, 80, 90));
        let value = self.value;

        let mut delta = 0;
        ui.horizontal(|ui| {
            let prev = ui.spacing().item_spacing.x;
            ui.spacing_mut().item_spacing.x = gap_xs();

            let st = current();
            let cr = egui::CornerRadius::same(st.r_xs as u8);
            let stroke = if st.hairline_borders {
                Stroke::new(st.stroke_std, color_alpha(border, alpha_strong()))
            } else {
                Stroke::new(st.stroke_thin, color_alpha(border, alpha_muted()))
            };

            let mut mk_btn = |ui: &mut Ui, sym: &str| -> Response {
                ui.add(
                    egui::Button::new(
                        RichText::new(sym).monospace().size(font_xs()).color(dim),
                    )
                    .fill(Color32::TRANSPARENT)
                    .stroke(stroke)
                    .corner_radius(cr)
                    .min_size(Vec2::new(14.0, 14.0)),
                )
            };

            if mk_btn(ui, "-").clicked() { delta = -1; }
            ui.label(
                RichText::new(value)
                    .monospace()
                    .size(font_xs())
                    .color(dim),
            );
            if mk_btn(ui, "+").clicked() { delta = 1; }

            ui.spacing_mut().item_spacing.x = prev;
        });
        delta
    }
}
