//! Builder + impl Widget primitives — inputs family.
//! See ui/widgets/mod.rs for the rationale.

#![allow(dead_code, unused_imports)]

use egui::{Color32, Response, RichText, Sense, Stroke, Ui, Vec2, Widget};
use super::super::style::*;
use crate::chart::renderer::ui::foundation::{InputShell, InputState, InputVariant, Size as FSize, Radius as FRadius};
use super::super::super::gpu::Theme;

#[inline(always)]
fn ft() -> &'static Theme { &crate::chart_renderer::gpu::THEMES[0] }

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
    theme: Option<&'a Theme>,
    variant: InputVariant,
    // Extended knobs
    text_color: Option<Color32>,
    background_color: Option<Color32>,
    explicit_id: Option<egui::Id>,
    margin: Option<egui::Margin>,
    frameless: bool,
    proportional: bool,
    multiline: bool,
    put_at: Option<egui::Rect>,
    horizontal_align: Option<egui::Align>,
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
            text_color: None,
            background_color: None,
            explicit_id: None,
            margin: None,
            frameless: false,
            proportional: false,
            multiline: false,
            put_at: None,
            horizontal_align: None,
        }
    }
    pub fn placeholder(mut self, p: &'a str) -> Self { self.placeholder = p; self }
    pub fn width(mut self, w: f32) -> Self { self.width = Some(w); self }
    pub fn font_size(mut self, s: f32) -> Self { self.font_size = s; self }
    pub fn palette(mut self, accent: Color32, _bear: Color32, dim: Color32) -> Self {
        self.accent = Some(accent); self.dim = Some(dim); self
    }
    pub fn theme(mut self, t: &'a Theme) -> Self {
        self.theme = Some(t);
        self.accent = Some(t.accent);
        self.dim = Some(t.dim);
        self
    }
    pub fn border(mut self, c: Color32) -> Self { self.border = Some(c); self }
    pub(crate) fn variant_internal(mut self, v: InputVariant) -> Self { self.variant = v; self }

    // ── Extended knobs ────────────────────────────────────────────────────────
    /// Explicit text color override (applied via `TextEdit::text_color`).
    pub fn text_color(mut self, c: Color32) -> Self { self.text_color = Some(c); self }
    /// Explicit background fill (applied via `Frame::fill`).
    pub fn background_color(mut self, c: Color32) -> Self { self.background_color = Some(c); self }
    /// Explicit `egui::Id` for the inner `TextEdit` (enables external focus tracking).
    pub fn id(mut self, id: egui::Id) -> Self { self.explicit_id = Some(id); self }
    /// Override the frame inner margin.
    pub fn margin(mut self, m: egui::Margin) -> Self { self.margin = Some(m); self }
    /// Disable the surrounding frame (for inline cell editors).
    pub fn frameless(mut self, v: bool) -> Self { self.frameless = v; self }
    /// Use proportional font instead of monospace (default).
    pub fn proportional(mut self, v: bool) -> Self { self.proportional = v; self }
    /// Enable multiline mode (uses `egui::TextEdit::multiline`).
    pub fn multiline(mut self, v: bool) -> Self { self.multiline = v; self }
    /// Place the widget at an explicit rect via `ui.put`.
    /// When set, `show` returns early using `ui.put(rect, …)` semantics.
    pub fn put_at(mut self, rect: egui::Rect) -> Self { self.put_at = Some(rect); self }
    /// Set the horizontal text alignment inside the `TextEdit` (e.g. `egui::Align::Center` or `egui::Align::RIGHT`).
    pub fn horizontal_align(mut self, a: egui::Align) -> Self { self.horizontal_align = Some(a); self }

    pub fn show(self, ui: &mut Ui) -> Response {
        let resolved_id = self.explicit_id.unwrap_or_else(|| ui.next_auto_id());
        let focused = ui.memory(|m| m.has_focus(resolved_id));
        let desired_width = self.width.unwrap_or_else(|| ui.available_width());
        let font_size = self.font_size;
        let placeholder = self.placeholder;
        let buffer = self.buffer;
        let is_multi = self.multiline;
        let is_prop = self.proportional;
        let text_color = self.text_color;
        let frameless = self.frameless;
        let put_at = self.put_at;
        let h_align = self.horizontal_align;

        let font_id = if is_prop {
            egui::FontId::proportional(font_size)
        } else {
            egui::FontId::monospace(font_size)
        };

        // Helper: build the TextEdit from the shared knobs.
        macro_rules! build_te {
            ($buf:expr) => {{
                let base = if is_multi {
                    egui::TextEdit::multiline($buf)
                } else {
                    egui::TextEdit::singleline($buf)
                };
                let base = base
                    .id(resolved_id)
                    .hint_text(placeholder)
                    .font(egui::FontSelection::FontId(font_id.clone()))
                    .frame(false)
                    .desired_width(desired_width);
                let base = if let Some(tc) = text_color { base.text_color(tc) } else { base };
                let base = if let Some(a) = h_align { base.horizontal_align(a) } else { base };
                base
            }};
        }

        // put_at shortcut — skips all framing and places directly.
        if let Some(rect) = put_at {
            let te = build_te!(buffer);
            return ui.put(rect, te);
        }

        // Compose InputShell when a theme is available — this is the foundation path.
        if let Some(theme) = self.theme {
            let state = if focused { InputState::Focused } else { InputState::Default };
            let mut resp_opt: Option<Response> = None;
            if frameless {
                let te = build_te!(buffer);
                return ui.add(te);
            }
            InputShell::new(theme)
                .variant(self.variant)
                .size(FSize::Sm)
                .radius(FRadius::Sm)
                .state(state)
                .body(|ui| {
                    let te = build_te!(buffer);
                    resp_opt = Some(ui.add(te));
                })
                .show(ui);
            return resp_opt.unwrap_or_else(|| ui.label(""));
        }

        // Fallback: legacy hand-rolled frame for palette()-only callers.
        let accent = self.accent.unwrap_or_else(|| ft().accent);
        let border = self.border.unwrap_or_else(|| ft().toolbar_border);
        let border_color = if focused {
            color_alpha(accent, alpha_active())
        } else {
            color_alpha(border, alpha_line())
        };

        if frameless {
            let te = build_te!(buffer);
            return ui.add(te);
        }

        let inner_margin = self.margin.unwrap_or_else(|| egui::Margin::same(gap_sm() as i8));
        let mut frame = egui::Frame::NONE
            .stroke(Stroke::new(stroke_std(), border_color))
            .inner_margin(inner_margin)
            .corner_radius(radius_sm());
        if let Some(bg) = self.background_color {
            frame = frame.fill(bg);
        }

        let mut resp_opt: Option<Response> = None;
        frame.show(ui, |ui| {
            let te = build_te!(buffer);
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
    pub fn theme(self, t: &Theme) -> Self {
        self.palette(t.accent, t.bear, t.dim)
    }
    pub fn border(mut self, c: Color32) -> Self { self.border = Some(c); self }

    pub fn show(self, ui: &mut Ui) -> Response {
        let accent = self.accent.unwrap_or_else(|| ft().accent);
        let dim = self.dim.unwrap_or_else(|| ft().dim);
        let border = self.border.unwrap_or_else(|| ft().toolbar_border);

        let buf_id = ui.next_auto_id();
        let value = self.value;
        let mut buf = ui.memory_mut(|m| {
            m.data.get_temp_mut_or_insert_with::<String>(buf_id, || value.to_string()).clone()
        });
        let resp = TextInput::new(&mut buf)
            .placeholder(self.placeholder)
            .font_size(self.font_size)
            .palette(accent, ft().bear, dim)
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
    pub fn theme(self, t: &Theme) -> Self {
        self.palette(t.accent, t.bear, t.dim)
    }
    pub fn border(mut self, c: Color32) -> Self { self.border = Some(c); self }

    pub fn show(self, ui: &mut Ui) -> Response {
        let accent = self.accent.unwrap_or_else(|| ft().accent);
        let dim = self.dim.unwrap_or_else(|| ft().dim);
        let border = self.border.unwrap_or_else(|| ft().toolbar_border);
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
    pub fn theme(self, t: &Theme) -> Self {
        self.palette(t.accent, t.bear, t.dim)
    }

    pub fn show(self, ui: &mut Ui) -> Response {
        let label_color = self.label_color.unwrap_or_else(|| ft().dim);
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
    theme: Option<&'a Theme>,
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
    pub fn theme(mut self, t: &'a Theme) -> Self {
        self.theme = Some(t);
        self.accent = Some(t.accent);
        self.dim = Some(t.dim);
        self
    }
    pub fn border(mut self, c: Color32) -> Self { self.border = Some(c); self }

    pub fn show(self, ui: &mut Ui) -> Response {
        let accent = self.accent.unwrap_or_else(|| ft().accent);
        let dim = self.dim.unwrap_or_else(|| ft().dim);
        let border = self.border.unwrap_or_else(|| ft().toolbar_border);

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
    pub fn theme(self, t: &Theme) -> Self {
        Self {
            value: self.value,
            dim: Some(t.dim),
            border: Some(t.toolbar_border),
        }
    }

    /// Body mirrors `components_extra::compact_stepper` byte-for-byte.
    pub fn show(self, ui: &mut Ui) -> i32 {
        let dim = self.dim.unwrap_or_else(|| ft().dim);
        let border = self.border.unwrap_or_else(|| ft().toolbar_border);
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

// ─── Slider ───────────────────────────────────────────────────────────────────

/// Numeric slider — wraps `egui::Slider` with theme + style awareness.
/// Used wherever the legacy code calls `egui::Slider::new(&mut value, range)`.
///
/// ```ignore
/// Slider::new(&mut my_f32, 0.0..=1.0).label("Opacity").suffix("%").theme(t).show(ui);
/// ```
#[must_use = "Slider must be rendered via `.show(ui)`"]
pub struct Slider<'a, T: egui::emath::Numeric> {
    value: &'a mut T,
    range: std::ops::RangeInclusive<T>,
    label: Option<&'a str>,
    step: Option<f64>,
    width: Option<f32>,
    fill_color: Option<Color32>,
    show_value: bool,
    suffix: Option<&'a str>,
    theme: Option<&'a Theme>,
}

impl<'a, T: egui::emath::Numeric> Slider<'a, T> {
    pub fn new(value: &'a mut T, range: std::ops::RangeInclusive<T>) -> Self {
        Self {
            value,
            range,
            label: None,
            step: None,
            width: None,
            fill_color: None,
            show_value: true,
            suffix: None,
            theme: None,
        }
    }

    pub fn label(mut self, l: &'a str) -> Self { self.label = Some(l); self }
    pub fn step(mut self, s: f64) -> Self { self.step = Some(s); self }
    pub fn width(mut self, w: f32) -> Self { self.width = Some(w); self }
    pub fn fill(mut self, c: Color32) -> Self { self.fill_color = Some(c); self }
    pub fn show_value(mut self, s: bool) -> Self { self.show_value = s; self }
    pub fn suffix(mut self, s: &'a str) -> Self { self.suffix = Some(s); self }
    pub fn theme(mut self, t: &'a Theme) -> Self { self.theme = Some(t); self }

    pub fn show(self, ui: &mut Ui) -> Response {
        let st = current();

        // Resolve fill color: explicit > theme accent > style default.
        let fill = self.fill_color
            .or_else(|| self.theme.map(|t| color_alpha(t.accent, alpha_active())))
            .unwrap_or_else(|| ft().accent);

        // Apply optional width constraint.
        if let Some(w) = self.width {
            ui.set_max_width(w);
        }

        let mut slider = egui::Slider::new(self.value, self.range)
            .show_value(self.show_value)
            .handle_shape(egui::style::HandleShape::Circle);

        if let Some(s) = self.step {
            slider = slider.step_by(s);
        }
        if let Some(sfx) = self.suffix {
            slider = slider.suffix(sfx);
        }

        // Style the slider track using theme-aware visuals.
        {
            let visuals = ui.visuals_mut();
            visuals.selection.bg_fill = fill;
            // Use r_md from active style for the handle radius if accessible.
            let _handle_r = st.r_md;
        }

        if let Some(lbl) = self.label {
            ui.label(
                RichText::new(lbl)
                    .monospace()
                    .size(font_sm())
                    .color(self.theme.map(|t| t.dim).unwrap_or_else(|| ft().dim)),
            );
        }

        ui.add(slider)
    }
}

// ─── ColorSwatchPicker ────────────────────────────────────────────────────────

/// A horizontal row of color-dot swatches drawn from a `&[&str]` hex palette,
/// with an optional "auto" (empty-string) button at the right end.
///
/// `value` is a `String` holding the currently-selected hex code, or `""` for auto.
///
/// ```ignore
/// if ColorSwatchPicker::new(&mut ind.color).palette(INDICATOR_COLORS)
///     .theme(t).show(ui)
/// { /* changed */ }
/// ```
#[must_use = "ColorSwatchPicker must be rendered via `.show(ui)`"]
pub struct ColorSwatchPicker<'a> {
    value: &'a mut String,
    palette: &'a [&'a str],
    swatch_size: f32,
    dot_radius: f32,
    auto_button: bool,
    /// Alpha applied to the dot when rendering the fill swatch (0 = full opacity).
    fill_alpha: u8,
    accent: Option<Color32>,
    dim: Option<Color32>,
    theme: Option<&'a Theme>,
}

impl<'a> ColorSwatchPicker<'a> {
    pub fn new(value: &'a mut String) -> Self {
        Self {
            value,
            palette: &[],
            swatch_size: 12.0,
            dot_radius: 3.0,
            auto_button: false,
            fill_alpha: 255,
            accent: None,
            dim: None,
            theme: None,
        }
    }
    pub fn palette(mut self, p: &'a [&'a str]) -> Self { self.palette = p; self }
    /// Size of the hit-rect allocated per swatch (default 12×12).
    pub fn swatch_size(mut self, s: f32) -> Self { self.swatch_size = s; self }
    /// Dot radius when idle (selected dot is 1px larger, default 3.0).
    pub fn dot_radius(mut self, r: f32) -> Self { self.dot_radius = r; self }
    /// Show an "auto" button that clears the selection to `""` (default false).
    pub fn auto_button(mut self, v: bool) -> Self { self.auto_button = v; self }
    /// Alpha channel applied to every dot fill (255 = opaque).
    pub fn fill_alpha(mut self, a: u8) -> Self { self.fill_alpha = a; self }
    pub fn theme(mut self, t: &'a Theme) -> Self {
        self.theme = Some(t);
        self.accent = Some(t.accent);
        self.dim = Some(t.dim);
        self
    }

    /// Returns `true` if the value was changed.
    pub fn show(self, ui: &mut Ui) -> bool {
        use super::super::style::*;
        let accent = self.accent.unwrap_or_else(|| ft().accent);
        let dim = self.dim.unwrap_or_else(|| ft().dim);
        let dot_r = self.dot_radius;
        let sel_dot_r = dot_r + 1.0;
        let sz = self.swatch_size;
        let mut changed = false;
        let value = self.value;
        let fill_alpha = self.fill_alpha;

        let prev_spacing = ui.spacing().item_spacing.x;
        ui.spacing_mut().item_spacing.x = 3.0;

        for &hex in self.palette {
            let col_full = super::super::style::hex_to_color(hex, 1.0);
            let col_draw = if fill_alpha < 255 {
                color_alpha(col_full, fill_alpha)
            } else {
                col_full
            };
            let is_cur = value.as_str() == hex;
            let (r, resp) = ui.allocate_exact_size(egui::vec2(sz, sz), Sense::click());
            if is_cur {
                ui.painter().rect_stroke(r, 2.0,
                    egui::Stroke::new(stroke_std(), col_full), egui::StrokeKind::Outside);
            }
            ui.painter().circle_filled(r.center(), if is_cur { sel_dot_r } else { dot_r }, col_draw);
            if resp.clicked() && !is_cur {
                *value = hex.to_string();
                changed = true;
            }
        }

        if self.auto_button {
            let is_auto = value.is_empty();
            use super::buttons::ChromeBtn;
            let auto_fg = if is_auto { accent } else { dim.gamma_multiply(0.5) };
            let auto_bg = if is_auto { color_alpha(accent, alpha_soft()) } else { Color32::TRANSPARENT };
            if ui.add(ChromeBtn::new(egui::RichText::new("auto").monospace().size(font_xs()).color(auto_fg))
                .fill(auto_bg)
                .corner_radius(r_xs())
                .min_size(egui::vec2(24.0, sz))).clicked() && !is_auto {
                *value = String::new();
                changed = true;
            }
        }

        ui.spacing_mut().item_spacing.x = prev_spacing;
        changed
    }
}

// ─── ThicknessPicker ──────────────────────────────────────────────────────────

/// A connected-pill row for selecting a stroke thickness from a fixed list of
/// `f32` values. Renders as a segmented control (connected pills).
///
/// ```ignore
/// ThicknessPicker::new(&mut ind.thickness)
///     .values(&[0.5, 1.0, 1.5, 2.0, 3.0])
///     .height(18.0)
///     .theme(t)
///     .show(ui);
/// ```
#[must_use = "ThicknessPicker must be rendered via `.show(ui)`"]
pub struct ThicknessPicker<'a> {
    value: &'a mut f32,
    values: &'a [f32],
    height: f32,
    font_size: f32,
    min_btn_w: f32,
    accent: Option<Color32>,
    dim: Option<Color32>,
    border: Option<Color32>,
    theme: Option<&'a Theme>,
}

impl<'a> ThicknessPicker<'a> {
    pub fn new(value: &'a mut f32) -> Self {
        Self {
            value,
            values: &[0.5, 1.0, 1.5, 2.0, 3.0],
            height: 18.0,
            font_size: 8.0,
            min_btn_w: 26.0,
            accent: None,
            dim: None,
            border: None,
            theme: None,
        }
    }
    pub fn values(mut self, v: &'a [f32]) -> Self { self.values = v; self }
    pub fn height(mut self, h: f32) -> Self { self.height = h; self }
    pub fn font_size(mut self, s: f32) -> Self { self.font_size = s; self }
    pub fn min_btn_w(mut self, w: f32) -> Self { self.min_btn_w = w; self }
    pub fn theme(mut self, t: &'a Theme) -> Self {
        self.theme = Some(t);
        self.accent = Some(t.accent);
        self.dim = Some(t.dim);
        self.border = Some(t.toolbar_border);
        self
    }

    /// Returns `true` if the value was changed.
    pub fn show(self, ui: &mut Ui) -> bool {
        use super::super::style::*;
        let accent = self.accent.unwrap_or_else(|| ft().accent);
        let dim = self.dim.unwrap_or_else(|| ft().dim);
        let border = self.border.unwrap_or_else(|| ft().toolbar_border);
        let n = self.values.len();
        let st = current();
        let r_sm = st.r_sm;
        let mut changed = false;
        let value = self.value;

        let prev_spacing = ui.spacing().item_spacing.x;
        ui.spacing_mut().item_spacing.x = 0.0;

        for (i, &th) in self.values.iter().enumerate() {
            let sel = (*value - th).abs() < 0.1;
            let fg = if sel { Color32::WHITE } else { dim.gamma_multiply(0.7) };
            let bg = if sel { color_alpha(accent, alpha_dim()) } else { color_alpha(border, alpha_subtle()) };
            let rounding: egui::CornerRadius = if i == 0 {
                egui::CornerRadius { nw: r_sm, sw: r_sm, ne: 0, se: 0 }
            } else if i == n - 1 {
                egui::CornerRadius { nw: 0, sw: 0, ne: r_sm, se: r_sm }
            } else {
                egui::CornerRadius::ZERO
            };
            let stroke_col = if sel { color_alpha(accent, alpha_heavy()) } else { color_alpha(border, alpha_line()) };
            if ui.add(egui::Button::new(
                    egui::RichText::new(format!("{:.1}", th)).monospace().size(self.font_size).color(fg))
                .fill(bg)
                .corner_radius(rounding)
                .min_size(egui::vec2(self.min_btn_w, self.height))
                .stroke(egui::Stroke::new(stroke_thin(), stroke_col)))
                .clicked() && !sel
            {
                *value = th;
                changed = true;
            }
        }

        ui.spacing_mut().item_spacing.x = prev_spacing;
        changed
    }
}
