//! Builder primitives — form layout family.
//!
//! Form layout primitives extracted from patterns seen in `settings_panel.rs`,
//! `hotkey_editor.rs`, `indicator_editor.rs`, `alerts_panel.rs`, and the
//! order-ticket area of `orders_panel.rs`. Each builder is a thin layout
//! wrapper that delegates rendering of the inner control to a closure that
//! receives `&mut Ui`.
//!
//! These additions are NEW only — call sites are not migrated yet (Wave 5).
//! Inner controls should be built from `widgets::{text, inputs}` primitives.

#![allow(dead_code, unused_imports)]

use egui::{Color32, Response, RichText, Stroke, Ui, Vec2};
use super::super::style::*;

// Shorthand for the Theme type used across the codebase.
type Theme = crate::chart_renderer::gpu::Theme;

fn ft() -> &'static Theme {
    &crate::chart_renderer::gpu::THEMES[0]
}

// ─── FormRow ──────────────────────────────────────────────────────────────────

/// Horizontal `label : control` row with a fixed-width label gutter.
///
/// ```ignore
/// FormRow::new("Username").label_width(120.0).show(ui, t, |ui| {
///     ui.add(TextInput::new(&mut state.username));
/// });
/// ```
/// Body alignment within the right-of-gutter region.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum FormRowAlign { Left, Right }

/// Hints passed to the body closure so primitives (e.g. TextInput/TextEdit)
/// can pick up row-level options like password mode and placeholder hint.
#[derive(Copy, Clone, Debug, Default)]
pub struct FormRowCx<'a> {
    pub password: bool,
    pub hint: Option<&'a str>,
}

#[must_use = "FormRow must be rendered with `.show(...)`"]
pub struct FormRow<'a> {
    label: &'a str,
    label_width: f32,
    help: Option<&'a str>,
    required: bool,
    label_color: Option<Color32>,
    leading_space: f32,
    alignment: FormRowAlign,
    inner_pad: f32,
    margin_top: f32,
    margin_bottom: f32,
    password: bool,
    hint: Option<&'a str>,
    label_label_layout_left: bool,
}

impl<'a> FormRow<'a> {
    pub fn new(label: &'a str) -> Self {
        Self {
            label,
            label_width: 120.0,
            help: None,
            required: false,
            label_color: None,
            leading_space: 0.0,
            alignment: FormRowAlign::Left,
            inner_pad: 0.0,
            margin_top: 0.0,
            margin_bottom: 0.0,
            password: false,
            hint: None,
            label_label_layout_left: false,
        }
    }
    pub fn label_width(mut self, w: f32) -> Self { self.label_width = w; self }
    /// Alias for `label_width`, matching the settings_panel terminology.
    pub fn gutter(mut self, w: f32) -> Self { self.label_width = w; self }
    pub fn help(mut self, h: &'a str) -> Self { self.help = Some(h); self }
    pub fn required(mut self, r: bool) -> Self { self.required = r; self }
    pub fn label_color(mut self, c: Color32) -> Self { self.label_color = Some(c); self }
    /// Horizontal pad inserted before the label gutter (e.g. dialog margin).
    pub fn leading_space(mut self, s: f32) -> Self { self.leading_space = s; self }
    /// Alias for [`leading_space`] — explicit left indent for nested option
    /// rows (mirrors the `add_space(m)` pattern used in indicator_editor).
    pub fn indent(mut self, s: f32) -> Self { self.leading_space = s; self }
    /// Body alignment within the area to the right of the gutter.
    pub fn alignment(mut self, a: FormRowAlign) -> Self { self.alignment = a; self }
    /// Pad inserted between the gutter and the body (or, in Right alignment,
    /// the right-edge inset).
    pub fn inner_pad(mut self, p: f32) -> Self { self.inner_pad = p; self }
    /// Custom vertical margins (top, bottom) around the row.
    pub fn margins(mut self, top: f32, bottom: f32) -> Self {
        self.margin_top = top;
        self.margin_bottom = bottom;
        self
    }
    /// Hint at password mode for TextEdit-style bodies (passed through `cx`).
    pub fn password(mut self, p: bool) -> Self { self.password = p; self }
    /// Placeholder hint text passed through `cx` to the body.
    pub fn hint(mut self, h: &'a str) -> Self { self.hint = Some(h); self }
    /// Lay out the label left-to-right inside the gutter (settings_panel style)
    /// instead of the default right-to-left alignment.
    pub fn label_left(mut self, l: bool) -> Self { self.label_label_layout_left = l; self }

    pub fn show<R>(
        self,
        ui: &mut Ui,
        t: &Theme,
        body: impl FnOnce(&mut Ui) -> R,
    ) -> R {
        self.show_with_cx(ui, t, |ui, _cx| body(ui))
    }

    /// Like `show`, but the body receives a `FormRowCx` carrying row-level
    /// hints (password, hint text). Body primitives can opt-in to honor them.
    pub fn show_with_cx<R>(
        self,
        ui: &mut Ui,
        t: &Theme,
        body: impl FnOnce(&mut Ui, FormRowCx<'_>) -> R,
    ) -> R {
        let label_color = self.label_color.unwrap_or(t.dim);
        let cx = FormRowCx { password: self.password, hint: self.hint };
        if self.margin_top > 0.0 { ui.add_space(self.margin_top); }
        let label_layout = if self.label_label_layout_left {
            egui::Layout::left_to_right(egui::Align::Center)
        } else {
            egui::Layout::right_to_left(egui::Align::Center)
        };
        let result = ui.horizontal(|ui| {
            if self.leading_space > 0.0 { ui.add_space(self.leading_space); }
            // Fixed label gutter
            ui.allocate_ui_with_layout(
                Vec2::new(self.label_width, ui.spacing().interact_size.y),
                label_layout,
                |ui| {
                    if self.required {
                        ui.add(RequiredMarker::new().theme(t));
                    }
                    ui.label(
                        RichText::new(self.label)
                            .monospace()
                            .size(font_sm())
                            .color(label_color),
                    );
                },
            );
            match self.alignment {
                FormRowAlign::Left => {
                    let pad = if self.inner_pad > 0.0 { self.inner_pad } else { gap_sm() };
                    ui.add_space(pad);
                    body(ui, cx)
                }
                FormRowAlign::Right => {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if self.inner_pad > 0.0 { ui.add_space(self.inner_pad); }
                        body(ui, cx)
                    }).inner
                }
            }
        });
        let r = result.inner;
        if let Some(h) = self.help {
            ui.horizontal(|ui| {
                ui.add_space(self.leading_space + self.label_width + gap_sm());
                ui.add(HelpText::new(h).theme(t));
            });
        }
        if self.margin_bottom > 0.0 { ui.add_space(self.margin_bottom); }
        r
    }
}

// ─── FieldSet ─────────────────────────────────────────────────────────────────

/// Bordered group with optional title — like an HTML `<fieldset>`.
///
/// ```ignore
/// FieldSet::new("Connection").show(ui, t, |ui| { /* fields */ });
/// ```
#[must_use = "FieldSet must be rendered with `.show(...)`"]
pub struct FieldSet<'a> {
    title: Option<&'a str>,
    inner_margin: f32,
}

impl<'a> FieldSet<'a> {
    pub fn new(title: &'a str) -> Self {
        Self { title: Some(title), inner_margin: gap_lg() }
    }
    pub fn untitled() -> Self {
        Self { title: None, inner_margin: gap_lg() }
    }
    pub fn title(mut self, t: &'a str) -> Self { self.title = Some(t); self }
    pub fn inner_margin(mut self, m: f32) -> Self { self.inner_margin = m; self }

    pub fn show<R>(
        self,
        ui: &mut Ui,
        t: &Theme,
        body: impl FnOnce(&mut Ui) -> R,
    ) -> R {
        let s = current();
        let stroke_w = if s.hairline_borders { s.stroke_std } else { stroke_thin() };
        let border = color_alpha(t.toolbar_border, alpha_muted());
        let frame = egui::Frame::NONE
            .stroke(Stroke::new(stroke_w, border))
            .corner_radius(r_sm_cr())
            .inner_margin(egui::Margin::same(self.inner_margin as i8));

        let resp = frame.show(ui, |ui| {
            if let Some(title) = self.title {
                ui.label(
                    RichText::new(title)
                        .monospace()
                        .size(font_sm())
                        .strong()
                        .color(t.text),
                );
                ui.add_space(gap_sm());
            }
            body(ui)
        });
        resp.inner
    }
}

// ─── FormSection ──────────────────────────────────────────────────────────────

/// Header label + spaced body — non-bordered grouping.
///
/// ```ignore
/// FormSection::new("Display").show(ui, t, |ui| { /* rows */ });
/// ```
#[must_use = "FormSection must be rendered with `.show(...)`"]
pub struct FormSection<'a> {
    title: &'a str,
    spacing: f32,
    title_color: Option<Color32>,
}

impl<'a> FormSection<'a> {
    pub fn new(title: &'a str) -> Self {
        Self { title, spacing: gap_md(), title_color: None }
    }
    pub fn spacing(mut self, s: f32) -> Self { self.spacing = s; self }
    pub fn title_color(mut self, c: Color32) -> Self { self.title_color = Some(c); self }

    pub fn show<R>(
        self,
        ui: &mut Ui,
        t: &Theme,
        body: impl FnOnce(&mut Ui) -> R,
    ) -> R {
        let color = self.title_color.unwrap_or(t.dim);
        ui.label(
            RichText::new(style_label_case(self.title))
                .monospace()
                .size(font_sm())
                .strong()
                .color(color),
        );
        ui.add_space(self.spacing);
        body(ui)
    }
}

// ─── LabeledControl ───────────────────────────────────────────────────────────

/// Vertical layout: label above, control below, optional help + error text.
///
/// ```ignore
/// LabeledControl::new("Quantity")
///     .help("Shares or contracts")
///     .show(ui, t, |ui| ui.add(TextInput::new(&mut qty)));
/// ```
#[must_use = "LabeledControl must be rendered with `.show(...)`"]
pub struct LabeledControl<'a> {
    label: &'a str,
    help: Option<&'a str>,
    error: Option<&'a str>,
    required: bool,
}

impl<'a> LabeledControl<'a> {
    pub fn new(label: &'a str) -> Self {
        Self { label, help: None, error: None, required: false }
    }
    pub fn help(mut self, h: &'a str) -> Self { self.help = Some(h); self }
    pub fn error(mut self, e: &'a str) -> Self { self.error = Some(e); self }
    pub fn required(mut self, r: bool) -> Self { self.required = r; self }

    pub fn show<R>(
        self,
        ui: &mut Ui,
        t: &Theme,
        body: impl FnOnce(&mut Ui) -> R,
    ) -> R {
        let r = ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(self.label)
                        .monospace()
                        .size(font_sm())
                        .color(t.dim),
                );
                if self.required {
                    ui.add(RequiredMarker::new().theme(t));
                }
            });
            ui.add_space(gap_xs());
            let inner = body(ui);
            if let Some(e) = self.error {
                ui.add_space(gap_xs());
                ui.add(ErrorText::new(e).theme(t));
            } else if let Some(h) = self.help {
                ui.add_space(gap_xs());
                ui.add(HelpText::new(h).theme(t));
            }
            inner
        });
        r.inner
    }
}

// ─── HelpText ─────────────────────────────────────────────────────────────────

/// Small dim italic text — typically rendered under a control.
#[must_use = "HelpText must be added with `ui.add(...)` to render"]
pub struct HelpText<'a> {
    text: &'a str,
    color: Option<Color32>,
}

impl<'a> HelpText<'a> {
    pub fn new(text: &'a str) -> Self { Self { text, color: None } }
    pub fn color(mut self, c: Color32) -> Self { self.color = Some(c); self }
    pub fn theme(mut self, t: &Theme) -> Self { self.color = Some(t.dim); self }
}

impl<'a> egui::Widget for HelpText<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        let base = self.color.unwrap_or_else(|| ft().dim);
        let c = color_alpha(base, alpha_dim());
        ui.label(
            RichText::new(self.text)
                .monospace()
                .size(font_xs())
                .italics()
                .color(c),
        )
    }
}

// ─── ErrorText ────────────────────────────────────────────────────────────────

/// Small red text — typically rendered under a control to surface validation
/// errors.
#[must_use = "ErrorText must be added with `ui.add(...)` to render"]
pub struct ErrorText<'a> {
    text: &'a str,
    color: Option<Color32>,
}

impl<'a> ErrorText<'a> {
    pub fn new(text: &'a str) -> Self { Self { text, color: None } }
    pub fn color(mut self, c: Color32) -> Self { self.color = Some(c); self }
    pub fn theme(mut self, t: &Theme) -> Self { self.color = Some(t.bear); self }
}

impl<'a> egui::Widget for ErrorText<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        let c = self.color.unwrap_or_else(|| ft().bear);
        ui.label(
            RichText::new(self.text)
                .monospace()
                .size(font_xs())
                .color(c),
        )
    }
}

// ─── RequiredMarker ───────────────────────────────────────────────────────────

/// Small red asterisk indicating a required field.
#[must_use = "RequiredMarker must be added with `ui.add(...)` to render"]
pub struct RequiredMarker {
    color: Option<Color32>,
}

impl RequiredMarker {
    pub fn new() -> Self { Self { color: None } }
    pub fn color(mut self, c: Color32) -> Self { self.color = Some(c); self }
    pub fn theme(mut self, t: &Theme) -> Self { self.color = Some(t.bear); self }
}

impl Default for RequiredMarker {
    fn default() -> Self { Self::new() }
}

impl egui::Widget for RequiredMarker {
    fn ui(self, ui: &mut Ui) -> Response {
        let c = self.color.unwrap_or_else(|| ft().bear);
        ui.label(
            RichText::new("*")
                .monospace()
                .size(font_sm())
                .strong()
                .color(c),
        )
    }
}

// ─── InlineValidation ─────────────────────────────────────────────────────────

/// Validation state used by [`InlineValidation`].
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ValidationState {
    Ok,
    Error,
    Neutral,
}

/// Leading green-check / red-x icon shown next to a value (e.g. inline next
/// to an input). Renders only an icon glyph; pair it with your value label.
///
/// ```ignore
/// ui.horizontal(|ui| {
///     ui.add(InlineValidation::new(ValidationState::Ok).theme(t));
///     ui.label("Connected");
/// });
/// ```
#[must_use = "InlineValidation must be added with `ui.add(...)` to render"]
pub struct InlineValidation {
    state: ValidationState,
    ok_color: Option<Color32>,
    err_color: Option<Color32>,
    dim_color: Option<Color32>,
}

impl InlineValidation {
    pub fn new(state: ValidationState) -> Self {
        Self { state, ok_color: None, err_color: None, dim_color: None }
    }
    pub fn theme(mut self, t: &Theme) -> Self {
        self.ok_color = Some(t.bull);
        self.err_color = Some(t.bear);
        self.dim_color = Some(t.dim);
        self
    }
    pub fn ok_color(mut self, c: Color32) -> Self { self.ok_color = Some(c); self }
    pub fn err_color(mut self, c: Color32) -> Self { self.err_color = Some(c); self }
    pub fn dim_color(mut self, c: Color32) -> Self { self.dim_color = Some(c); self }
}

impl egui::Widget for InlineValidation {
    fn ui(self, ui: &mut Ui) -> Response {
        let (glyph, color) = match self.state {
            ValidationState::Ok => (
                "✓",
                self.ok_color.unwrap_or_else(|| ft().bull),
            ),
            ValidationState::Error => (
                "✗",
                self.err_color.unwrap_or_else(|| ft().bear),
            ),
            ValidationState::Neutral => (
                "•",
                self.dim_color.unwrap_or_else(|| ft().dim),
            ),
        };
        ui.label(
            RichText::new(glyph)
                .monospace()
                .size(font_sm())
                .strong()
                .color(color),
        )
    }
}

// ─── MeridienOrderTicket (#13) ────────────────────────────────────────────────

/// All mutable order state threaded through `MeridienOrderTicket::show`.
/// Maps 1:1 onto the `Chart` fields; the caller passes `&mut chart.field`
/// references directly.
pub struct OrderTicketState<'a> {
    pub symbol:       &'a str,
    pub is_buy:       &'a mut bool,
    pub order_type_idx: &'a mut usize,
    pub order_tif_idx:  &'a mut usize,
    pub order_qty:      &'a mut u32,
    pub order_market:   &'a mut bool,
    pub limit_price:    &'a mut String,
    pub stop_price:     &'a mut String,
    pub tp_price:       &'a mut String,
    pub sl_price:       &'a mut String,
    pub bracket:        &'a mut bool,
    pub bid:  f32,
    pub last: f32,
    pub ask:  f32,
    pub notional: f32,
    pub buying_power: f32,
    pub slippage_bps: f32,
}

/// Outcome emitted by `MeridienOrderTicket::show`.
pub struct OrderTicketOutcome {
    /// User clicked the REVIEW CTA.
    pub review_clicked: bool,
}

/// Meridien editorial order entry form (#13).
///
/// Replaces the standard compact body when `current().hairline_borders` is true.
/// Call site pattern:
///
/// ```ignore
/// if current().hairline_borders {
///     let outcome = MeridienOrderTicket::new().theme(t).show(ui, &mut state);
///     if outcome.review_clicked { submit_order(); }
///     return;
/// }
/// // … existing compact body …
/// ```
#[must_use = "MeridienOrderTicket must be shown with `.show(ui, state)`"]
pub struct MeridienOrderTicket {
    bg:      Color32,
    text:    Color32,
    dim:     Color32,
    bull:    Color32,
    bear:    Color32,
    accent:  Color32,
    border:  Color32,
    width:   f32,
}

impl MeridienOrderTicket {
    pub fn new() -> Self {
        Self {
            bg:     Color32::from_rgb(24, 20, 16),
            text:   Color32::from_rgb(238, 228, 210),
            dim:    Color32::from_rgb(150, 138, 118),
            bull:   Color32::from_rgb(120, 170, 104),
            bear:   Color32::from_rgb(220, 108, 70),
            accent: Color32::from_rgb(232, 118, 80),
            border: Color32::from_rgb(80, 72, 60),
            width:  0.0, // 0 = fill available
        }
    }
    pub fn width(mut self, w: f32) -> Self { self.width = w; self }
    pub fn theme(mut self, t: &Theme) -> Self {
        self.bg     = t.toolbar_bg;
        self.text   = t.text;
        self.dim    = t.dim;
        self.bull   = t.bull;
        self.bear   = t.bear;
        self.accent = t.accent;
        self.border = t.toolbar_border;
        self
    }

    /// Render the full editorial order ticket. Returns `OrderTicketOutcome`.
    pub fn show(self, ui: &mut Ui, s: &mut OrderTicketState<'_>) -> OrderTicketOutcome {
        let mut review_clicked = false;
        let st = current();
        let panel_w = if self.width > 0.0 { self.width } else { ui.available_width() };
        let label_color = self.dim.gamma_multiply(0.7);
        let border_col  = color_alpha(self.border, 50);
        let hairline_sw = st.stroke_std;

        let section_label_txt = |ui: &mut Ui, txt: &str| {
            ui.label(RichText::new(style_label_case(txt))
                .monospace().size(font_xs())
                .color(label_color));
        };

        let hairline = |ui: &mut Ui| {
            let avail = ui.available_width();
            let (rect, _) = ui.allocate_exact_size(Vec2::new(avail, 1.0), egui::Sense::hover());
            ui.painter().rect_filled(rect, egui::CornerRadius::ZERO, border_col);
            ui.add_space(2.0);
        };

        ui.set_width(panel_w);
        ui.spacing_mut().item_spacing.y = 3.0;
        ui.add_space(4.0);

        // ── Section 1: Header ──────────────────────────────────────────────
        ui.horizontal(|ui| {
            section_label_txt(ui, "Order Ticket");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(RichText::new(s.symbol)
                    .monospace().size(font_md()).strong()
                    .color(self.text));
            });
        });
        ui.add_space(2.0);
        hairline(ui);

        // ── Section 2: BID/LAST/ASK strip ─────────────────────────────────
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            let col_w = panel_w / 3.0;
            for &(label, val, color) in &[
                ("BID",  s.bid,  self.bear),
                ("LAST", s.last, self.text),
                ("ASK",  s.ask,  self.bull),
            ] {
                ui.vertical(|ui| {
                    ui.set_width(col_w);
                    ui.label(RichText::new(label).monospace().size(font_xs())
                        .color(color_alpha(label_color, alpha_active())));
                    ui.label(RichText::new(format!("{:.2}", val)).monospace()
                        .size(font_sm()).color(color));
                });
            }
        });
        ui.add_space(2.0);
        hairline(ui);

        // ── Section 3: BUY / SELL toggle ──────────────────────────────────
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            let half = (panel_w - 4.0) / 2.0;
            for &(label, is_this, color) in &[
                ("BUY",  true,  self.bull),
                ("SELL", false, self.bear),
            ] {
                let active = *s.is_buy == is_this;
                let bg = if active { color_alpha(color, 60) } else { color_alpha(self.border, 20) };
                let fg = if active { color } else { color_alpha(self.dim, alpha_strong()) };
                let (rect, resp) = ui.allocate_exact_size(
                    Vec2::new(half, 22.0), egui::Sense::click());
                if resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
                ui.painter().rect_filled(rect, egui::CornerRadius::ZERO, bg);
                if active {
                    ui.painter().rect_stroke(rect, egui::CornerRadius::ZERO,
                        Stroke::new(hairline_sw, color), egui::StrokeKind::Inside);
                }
                ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER,
                    label, egui::FontId::monospace(font_sm()), fg);
                if resp.clicked() { *s.is_buy = is_this; }
            }
        });
        ui.add_space(2.0);

        // ── Section 4: TYPE ────────────────────────────────────────────────
        let order_types = ["MKT", "LMT", "STP", "STP-LMT", "TRAIL"];
        ui.horizontal(|ui| {
            section_label_txt(ui, "Type");
            ui.add_space(6.0);
            ui.spacing_mut().item_spacing.x = 0.0;
            let seg_w = (panel_w * 0.6 / order_types.len() as f32).max(24.0);
            for (i, &opt) in order_types.iter().enumerate() {
                let sel = *s.order_type_idx == i;
                let fg = if sel { self.text } else { color_alpha(self.dim, alpha_strong()) };
                let bg = if sel { color_alpha(self.accent, 40) } else { egui::Color32::TRANSPARENT };
                if ui.add(egui::Button::new(
                    RichText::new(opt).monospace().size(font_xs()).color(fg))
                    .fill(bg).min_size(Vec2::new(seg_w, 16.0)))
                    .clicked()
                {
                    *s.order_type_idx = i;
                    *s.order_market = i == 0;
                }
            }
        });

        // ── Section 5: TIF ─────────────────────────────────────────────────
        let tifs = ["DAY", "GTC", "IOC"];
        ui.horizontal(|ui| {
            section_label_txt(ui, "TIF");
            ui.add_space(6.0);
            ui.spacing_mut().item_spacing.x = 0.0;
            let seg_w = (panel_w * 0.4 / tifs.len() as f32).max(24.0);
            for (i, &opt) in tifs.iter().enumerate() {
                let sel = *s.order_tif_idx == i;
                let fg = if sel { self.text } else { color_alpha(self.dim, alpha_strong()) };
                let bg = if sel { color_alpha(self.accent, 40) } else { egui::Color32::TRANSPARENT };
                if ui.add(egui::Button::new(
                    RichText::new(opt).monospace().size(font_xs()).color(fg))
                    .fill(bg).min_size(Vec2::new(seg_w, 16.0)))
                    .clicked()
                {
                    *s.order_tif_idx = i;
                }
            }
        });
        ui.add_space(2.0);
        hairline(ui);

        // ── Section 6: QUANTITY hero ───────────────────────────────────────
        section_label_txt(ui, "Quantity");
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            // Hero numeric — large serif
            ui.label(hero_text(&s.order_qty.to_string(), self.text)
                .size(st.font_hero * 0.7));
            // ± stepper
            if ui.small_button("−").clicked() {
                *s.order_qty = s.order_qty.saturating_sub(1).max(1);
            }
            if ui.small_button("+").clicked() {
                *s.order_qty = s.order_qty.saturating_add(1);
            }
            // Preset chips
            ui.add_space(4.0);
            for &preset in &[100u32, 500, 1000] {
                let sel = *s.order_qty == preset;
                let fg = if sel { self.accent } else { color_alpha(self.dim, alpha_strong()) };
                let bg = if sel { color_alpha(self.accent, 25) } else { egui::Color32::TRANSPARENT };
                if ui.add(egui::Button::new(
                    RichText::new(preset.to_string()).monospace().size(font_xs()).color(fg))
                    .fill(bg).min_size(Vec2::new(32.0, 16.0)))
                    .clicked()
                {
                    *s.order_qty = preset;
                }
            }
        });
        ui.add_space(2.0);

        // ── Section 7: LIMIT PRICE ────────────────────────────────────────
        if !*s.order_market {
            section_label_txt(ui, "Limit Price");
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                ui.add_sized(Vec2::new(80.0, 18.0),
                    egui::TextEdit::singleline(s.limit_price)
                        .font(egui::FontId::monospace(font_sm()))
                        .text_color(self.text));
                // BID/LAST/ASK presets
                for &(tag, val) in &[("B", s.bid), ("L", s.last), ("A", s.ask)] {
                    if ui.add(egui::Button::new(
                        RichText::new(tag).monospace().size(font_xs())
                            .color(color_alpha(self.dim, alpha_strong())))
                        .fill(egui::Color32::TRANSPARENT)
                        .min_size(Vec2::new(18.0, 16.0)))
                        .clicked()
                    {
                        *s.limit_price = format!("{:.2}", val);
                    }
                }
            });
            ui.add_space(2.0);
        }
        hairline(ui);

        // ── Section 8: BRACKET ────────────────────────────────────────────
        ui.horizontal(|ui| {
            let brk_col = if *s.bracket { self.accent } else { color_alpha(self.dim, alpha_muted()) };
            if ui.add(egui::Button::new(
                RichText::new(style_label_case("Bracket — Stop + Target"))
                    .monospace().size(font_xs()).color(brk_col))
                .fill(egui::Color32::TRANSPARENT))
                .clicked()
            {
                *s.bracket = !*s.bracket;
            }
        });
        if *s.bracket {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                ui.label(RichText::new("TP").monospace().size(font_xs()).color(self.bull));
                ui.add_sized(Vec2::new(60.0, 16.0),
                    egui::TextEdit::singleline(s.tp_price)
                        .font(egui::FontId::monospace(font_xs()))
                        .text_color(self.text));
                ui.label(RichText::new("SL").monospace().size(font_xs()).color(self.bear));
                ui.add_sized(Vec2::new(60.0, 16.0),
                    egui::TextEdit::singleline(s.sl_price)
                        .font(egui::FontId::monospace(font_xs()))
                        .text_color(self.text));
            });
        }
        ui.add_space(2.0);
        hairline(ui);

        // ── Section 9: META ROW ───────────────────────────────────────────
        let meta_notional   = format!("${:.0}", s.notional);
        let meta_bp         = format!("${:.0}M", s.buying_power / 1_000_000.0);
        let meta_slip       = format!("{:.1} bp", s.slippage_bps);
        let meta: [(&str, &str); 3] = [
            ("Notional",      &meta_notional),
            ("Buying Power",  &meta_bp),
            ("Est. Slippage", &meta_slip),
        ];
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            let col_w = panel_w / 3.0;
            for &(lbl, val_str) in &meta {
                ui.vertical(|ui| {
                    ui.set_width(col_w);
                    section_label_txt(ui, lbl);
                    ui.label(RichText::new(val_str).monospace()
                        .size(font_sm()).color(self.text));
                });
            }
        });
        ui.add_space(4.0);

        // ── Section 10: REVIEW CTA ────────────────────────────────────────
        let side_str = if *s.is_buy { "BUY" } else { "SELL" };
        let cta_color = if *s.is_buy { self.bull } else { self.bear };
        let price_str = if *s.order_market {
            "MKT".to_string()
        } else {
            s.limit_price.clone()
        };
        let cta_label = format!("REVIEW {} · {} @ {}", side_str, s.order_qty, price_str);
        let (cta_rect, cta_resp) = ui.allocate_exact_size(
            Vec2::new(panel_w, 26.0), egui::Sense::click());
        if cta_resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
        let cta_bg = if cta_resp.hovered() {
            color_alpha(cta_color, 80)
        } else {
            color_alpha(cta_color, 55)
        };
        ui.painter().rect_filled(cta_rect, egui::CornerRadius::ZERO, cta_bg);
        ui.painter().text(cta_rect.center(), egui::Align2::CENTER_CENTER,
            &cta_label,
            egui::FontId::monospace(font_sm()),
            self.text);
        if cta_resp.clicked() { review_clicked = true; }

        OrderTicketOutcome { review_clicked }
    }
}

impl Default for MeridienOrderTicket {
    fn default() -> Self { Self::new() }
}

// ─── IndicatorParamRow ────────────────────────────────────────────────────────

/// Horizontal row: `indent → label → DragValue → optional preset chips`.
///
/// Designed for the indicator editor parameter section. Combines the
/// label-gutter pattern of `FormRow` with an inline `DragValue` and a list of
/// preset values displayed as small ChromeBtn chips.
///
/// ```ignore
/// let changed = IndicatorParamRow::new("Period", &mut ind.period as &mut usize)
///     .indent(8.0)
///     .presets(&[9, 20, 50, 100, 200])
///     .theme(t)
///     .show(ui);
/// ```
#[must_use = "IndicatorParamRow must be rendered via `.show(ui)`"]
pub struct IndicatorParamRow<'a> {
    label: &'a str,
    value: &'a mut usize,
    indent: f32,
    presets: &'a [usize],
    range_min: usize,
    range_max: usize,
    speed: f64,
    accent: Option<Color32>,
    dim: Option<Color32>,
    border: Option<Color32>,
    theme: Option<&'a Theme>,
}

impl<'a> IndicatorParamRow<'a> {
    pub fn new(label: &'a str, value: &'a mut usize) -> Self {
        Self {
            label,
            value,
            indent: 0.0,
            presets: &[],
            range_min: 1,
            range_max: 500,
            speed: 0.5,
            accent: None,
            dim: None,
            border: None,
            theme: None,
        }
    }
    pub fn indent(mut self, s: f32) -> Self { self.indent = s; self }
    pub fn presets(mut self, p: &'a [usize]) -> Self { self.presets = p; self }
    pub fn range(mut self, min: usize, max: usize) -> Self { self.range_min = min; self.range_max = max; self }
    pub fn speed(mut self, s: f64) -> Self { self.speed = s; self }
    pub fn theme(mut self, t: &'a Theme) -> Self {
        self.theme = Some(t);
        self.accent = Some(t.accent);
        self.dim = Some(t.dim);
        self.border = Some(t.toolbar_border);
        self
    }

    /// Returns `true` if the value changed.
    pub fn show(self, ui: &mut Ui) -> bool {
        use super::buttons::ChromeBtn;
        let accent = self.accent.unwrap_or_else(|| ft().accent);
        let dim = self.dim.unwrap_or_else(|| ft().dim);
        let border = self.border.unwrap_or_else(|| ft().toolbar_border);
        let value = self.value;
        let mut changed = false;

        ui.horizontal(|ui| {
            if self.indent > 0.0 { ui.add_space(self.indent); }
            ui.label(egui::RichText::new(self.label).monospace().size(font_sm()).color(dim));
            ui.add_space(gap_sm());
            let mut p = *value as i32;
            if ui.add(egui::DragValue::new(&mut p)
                .range(self.range_min as i32..=self.range_max as i32)
                .speed(self.speed)
                .custom_formatter(|v, _| format!("{}", v as i32))).changed()
            {
                *value = (p as usize).max(self.range_min);
                changed = true;
            }
            if !self.presets.is_empty() {
                ui.add_space(gap_md());
                let prev = ui.spacing().item_spacing.x;
                ui.spacing_mut().item_spacing.x = 2.0;
                for &pr in self.presets {
                    let sel = *value == pr;
                    let fg = if sel { accent } else { dim.gamma_multiply(0.5) };
                    if ui.add(ChromeBtn::new(
                            egui::RichText::new(format!("{}", pr)).monospace().size(font_xs()).color(fg))
                        .fill(if sel { color_alpha(accent, alpha_soft()) } else { Color32::TRANSPARENT })
                        .corner_radius(r_xs())
                        .min_size(egui::vec2(22.0, 18.0))).clicked() && !sel
                    {
                        *value = pr;
                        changed = true;
                    }
                }
                ui.spacing_mut().item_spacing.x = prev;
            }
        });
        changed
    }
}

/// Float variant of `IndicatorParamRow` for `f32` parameters (e.g. std-dev, multiplier).
#[must_use = "IndicatorParamRowF must be rendered via `.show(ui)`"]
pub struct IndicatorParamRowF<'a> {
    label: &'a str,
    value: &'a mut f32,
    default: f32,
    indent: f32,
    presets: &'a [f32],
    range_min: f32,
    range_max: f32,
    speed: f64,
    decimals: usize,
    accent: Option<Color32>,
    dim: Option<Color32>,
    border: Option<Color32>,
    theme: Option<&'a Theme>,
}

impl<'a> IndicatorParamRowF<'a> {
    pub fn new(label: &'a str, value: &'a mut f32, default: f32) -> Self {
        Self {
            label, value, default,
            indent: 0.0,
            presets: &[],
            range_min: 0.0,
            range_max: 500.0,
            speed: 0.05,
            decimals: 1,
            accent: None,
            dim: None,
            border: None,
            theme: None,
        }
    }
    pub fn indent(mut self, s: f32) -> Self { self.indent = s; self }
    pub fn presets(mut self, p: &'a [f32]) -> Self { self.presets = p; self }
    pub fn range(mut self, min: f32, max: f32) -> Self { self.range_min = min; self.range_max = max; self }
    pub fn speed(mut self, s: f64) -> Self { self.speed = s; self }
    pub fn decimals(mut self, d: usize) -> Self { self.decimals = d; self }
    pub fn theme(mut self, t: &'a Theme) -> Self {
        self.theme = Some(t);
        self.accent = Some(t.accent);
        self.dim = Some(t.dim);
        self.border = Some(t.toolbar_border);
        self
    }

    /// Returns `true` if the value changed.
    pub fn show(self, ui: &mut Ui) -> bool {
        use super::buttons::ChromeBtn;
        let accent = self.accent.unwrap_or_else(|| ft().accent);
        let dim = self.dim.unwrap_or_else(|| ft().dim);
        let d = self.decimals;
        let value = self.value;
        // Treat 0.0 as "use default"
        if *value <= 0.0 { *value = self.default; }
        let mut changed = false;

        ui.horizontal(|ui| {
            if self.indent > 0.0 { ui.add_space(self.indent); }
            ui.label(egui::RichText::new(self.label).monospace().size(font_sm()).color(dim));
            ui.add_space(gap_sm());
            if ui.add(egui::DragValue::new(value)
                .range(self.range_min..=self.range_max)
                .speed(self.speed)
                .custom_formatter(move |v, _| format!("{:.prec$}", v, prec = d))).changed()
            {
                changed = true;
            }
            if !self.presets.is_empty() {
                ui.add_space(gap_sm());
                let prev = ui.spacing().item_spacing.x;
                ui.spacing_mut().item_spacing.x = 2.0;
                for &pr in self.presets {
                    let sel = (*value - pr).abs() < 0.01;
                    let fg = if sel { accent } else { dim.gamma_multiply(0.5) };
                    if ui.add(ChromeBtn::new(
                            egui::RichText::new(format!("{:.prec$}", pr, prec = d)).monospace().size(font_xs()).color(fg))
                        .fill(if sel { color_alpha(accent, alpha_soft()) } else { Color32::TRANSPARENT })
                        .corner_radius(r_xs())
                        .min_size(egui::vec2(22.0, 18.0))).clicked() && !sel
                    {
                        *value = pr;
                        changed = true;
                    }
                }
                ui.spacing_mut().item_spacing.x = prev;
            }
        });
        changed
    }
}

// ─── ApertureOrderTicket (#aperture) ─────────────────────────────────────────

/// Which design variant to render.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ApertureVariant {
    Aperture,
    Octave,
}

impl Default for ApertureVariant {
    fn default() -> Self { Self::Aperture }
}

/// All mutable order state threaded through `ApertureOrderTicket::show`.
pub struct ApertureOrderState<'a> {
    pub last_price:            f32,
    pub spread:                f32,
    pub order_advanced:        bool,
    pub order_market:          &'a mut bool,
    pub order_type_idx:        &'a mut usize,
    pub order_tif_idx:         &'a mut usize,
    pub order_qty:             &'a mut u32,
    pub order_notional_mode:   &'a mut bool,
    pub order_notional_amount: &'a mut String,
    pub order_limit_price:     &'a mut String,
    pub order_stop_price:      &'a mut String,
    pub order_trail_amt:       &'a mut String,
    pub order_bracket:         &'a mut bool,
    pub order_tp_price:        &'a mut String,
    pub order_sl_price:        &'a mut String,
    pub order_outside_rth:     &'a mut bool,
    pub is_option:             bool,
    pub option_type:           &'a str,
    pub armed:                 bool,
}

/// Action signalled by `ApertureOrderTicket::show`.
#[derive(Clone, Debug, PartialEq)]
pub enum ApertureAction {
    None,
    Buy  { price: f32 },
    Sell { price: f32 },
    TriggerBuy,
    TriggerSell,
}

/// Outcome returned by `ApertureOrderTicket::show`.
pub struct ApertureOrderOutcome {
    pub action: ApertureAction,
}

/// Compact order-entry widget for the Aperture and Octave theme families.
#[must_use = "ApertureOrderTicket must be rendered with `.show(ui, state)`"]
pub struct ApertureOrderTicket {
    variant:        ApertureVariant,
    panel_w:        f32,
    text:           Color32,
    dim:            Color32,
    bull:           Color32,
    bear:           Color32,
    accent:         Color32,
    toolbar_bg:     Color32,
    toolbar_border: Color32,
}

impl ApertureOrderTicket {
    pub fn new() -> Self {
        Self {
            variant:        ApertureVariant::default(),
            panel_w:        0.0,
            text:           Color32::from_rgb(220, 215, 205),
            dim:            Color32::from_rgb(140, 132, 120),
            bull:           Color32::from_rgb(100, 160, 88),
            bear:           Color32::from_rgb(200, 88, 60),
            accent:         Color32::from_rgb(100, 130, 200),
            toolbar_bg:     Color32::from_rgb(28, 26, 24),
            toolbar_border: Color32::from_rgb(60, 56, 50),
        }
    }
    pub fn variant(mut self, v: ApertureVariant) -> Self { self.variant = v; self }
    pub fn panel_width(mut self, w: f32) -> Self { self.panel_w = w; self }
    pub fn theme(mut self, t: &Theme) -> Self {
        self.text           = t.text;
        self.dim            = t.dim;
        self.bull           = t.bull;
        self.bear           = t.bear;
        self.accent         = t.accent;
        self.toolbar_bg     = t.toolbar_bg;
        self.toolbar_border = t.toolbar_border;
        self
    }

    pub fn show(self, ui: &mut Ui, s: &mut ApertureOrderState<'_>) -> ApertureOrderOutcome {
        use super::select::SegmentedControl;
        use super::inputs::Stepper;
        use super::buttons::TradeBtn;

        let panel_w = if self.panel_w > 0.0 { self.panel_w } else { ui.available_width() };
        let pad     = 8.0_f32;
        let adv     = s.order_advanced;
        let last    = s.last_price;
        let spread  = s.spread;
        let _ = self.variant;
        let _ = self.text;
        // Build a minimal theme stub so sub-widgets that accept &Theme can be
        // called without the caller threading a full Theme reference here.
        let t_stub  = aperture_stub_theme_full(
            self.dim, self.bull, self.bear, self.accent,
            self.toolbar_bg, self.toolbar_border);

        let mut action = ApertureAction::None;

        // ── Advanced: order type + TIF + EXT ──────────────────────────────
        if adv {
            ui.horizontal(|ui| {
                ui.add_space(pad);
                const OT_STOCK: &[(usize, &str)] = &[
                    (0, "MKT"), (1, "LMT"), (2, "STP"), (3, "STP-LMT"), (4, "TRAIL"),
                ];
                const OT_OPTION: &[(usize, &str)] = &[
                    (0, "MKT"), (1, "LMT"), (2, "STP"), (3, "STP-LMT"), (4, "TRAIL"), (5, "UND"),
                ];
                let ot_opts = if s.is_option { OT_OPTION } else { OT_STOCK };
                if SegmentedControl::new()
                    .options(ot_opts)
                    .connected_pills(true)
                    .compact(true)
                    .height(18.0)
                    .theme(&t_stub)
                    .show(ui, s.order_type_idx)
                {
                    *s.order_market = *s.order_type_idx == 0;
                }
                ui.add_space(8.0);
                let tif_opts: &[(usize, &str)] = &[(0, "DAY"), (1, "GTC"), (2, "IOC")];
                SegmentedControl::new()
                    .options(tif_opts)
                    .theme(&t_stub)
                    .show(ui, s.order_tif_idx);
                ui.add_space(6.0);
                let rth_amber = egui::Color32::from_rgb(255, 191, 0);
                let rth_fg = if *s.order_outside_rth { rth_amber } else { color_alpha(self.dim, 40) };
                let rth_bg = if *s.order_outside_rth { color_alpha(rth_amber, 30) } else { egui::Color32::TRANSPARENT };
                let rth_stroke = Stroke::new(0.5, if *s.order_outside_rth {
                    color_alpha(rth_amber, 80)
                } else {
                    color_alpha(self.toolbar_border, 40)
                });
                if ui.add(egui::Button::new(
                        egui::RichText::new("EXT").monospace().size(font_xs()).color(rth_fg))
                    .fill(rth_bg).corner_radius(2.0).stroke(rth_stroke)
                    .min_size(egui::vec2(26.0, 18.0)))
                    .on_hover_text("Trade outside regular trading hours")
                    .clicked()
                {
                    *s.order_outside_rth = !*s.order_outside_rth;
                }
            });
            ui.add_space(4.0);
        }

        // ── QTY / $ mode ──────────────────────────────────────────────────
        ui.horizontal(|ui| {
            ui.add_space(pad);
            let mode_opts: &[(bool, &str)] = &[(false, "QTY"), (true, "$")];
            SegmentedControl::new()
                .options(mode_opts)
                .theme(&t_stub)
                .show(ui, s.order_notional_mode);
            if *s.order_notional_mode {
                ui.add_space(4.0);
                let premium = last;
                let mult    = if s.is_option { 100.0_f32 } else { 1.0_f32 };
                ui.add(egui::TextEdit::singleline(s.order_notional_amount)
                    .desired_width(70.0).font(egui::FontId::monospace(9.0)).hint_text("Amount"));
                let notional: f32 = s.order_notional_amount.parse().unwrap_or(0.0);
                let qty = if premium > 0.0 && mult > 0.0 {
                    (notional / (premium * mult)).floor() as i32
                } else { 0 };
                if qty > 0 { *s.order_qty = qty as u32; }
                ui.label(egui::RichText::new(format!("= {} @ {:.2}", qty, premium))
                    .monospace().size(font_sm()).color(color_alpha(self.dim, 60)));
            }
        });
        ui.add_space(2.0);

        // ── QTY stepper + compact price / MKT-LMT ────────────────────────
        ui.horizontal(|ui| {
            ui.add_space(pad);
            ui.spacing_mut().item_spacing.x = 2.0;
            let step = if *s.order_qty >= 100 { 10u32 }
                       else if *s.order_qty >= 10 { 5 }
                       else { 1 };
            if !*s.order_notional_mode {
                Stepper::new(s.order_qty)
                    .step(step).range(1, u32::MAX)
                    .theme(&t_stub)
                    .show(ui);
            } else {
                let _ = ui.add(
                    egui::TextEdit::singleline(&mut format!("{} contracts", s.order_qty))
                        .desired_width(100.0).font(egui::FontId::monospace(9.0))
                        .horizontal_align(egui::Align::Center).interactive(false));
            }
            ui.add_space(4.0);
            let cursor = ui.cursor().min;
            ui.painter().line_segment(
                [egui::pos2(cursor.x, cursor.y), egui::pos2(cursor.x, cursor.y + 20.0)],
                Stroke::new(1.0, color_alpha(self.toolbar_border, 80)));
            ui.add_space(6.0);
            if !adv {
                if *s.order_market {
                    ui.label(egui::RichText::new(format!("{:.2}", last))
                        .monospace().size(font_md()).color(self.dim));
                } else {
                    ui.add(egui::TextEdit::singleline(s.order_limit_price)
                        .desired_width(68.0).font(egui::FontId::monospace(10.0))
                        .hint_text("Price").horizontal_align(egui::Align::RIGHT));
                }
                ui.add_space(2.0);
                let mkt_label = if *s.order_market { "MKT" } else { "LMT" };
                if ui.add(egui::Button::new(
                        egui::RichText::new(mkt_label).monospace().size(font_sm()).strong()
                            .color(if *s.order_market { self.accent } else { self.dim }))
                    .fill(if *s.order_market { color_alpha(self.accent, 35) } else { self.toolbar_bg })
                    .stroke(Stroke::new(0.5, color_alpha(self.toolbar_border, 90))).corner_radius(2.0)
                    .min_size(egui::vec2(30.0, 20.0)))
                    .clicked()
                {
                    *s.order_market = !*s.order_market;
                    if !*s.order_market && s.order_limit_price.is_empty() {
                        *s.order_limit_price = format!("{:.2}", last);
                    }
                }
            } else {
                ui.label(egui::RichText::new(format!("Last {:.2}", last))
                    .monospace().size(font_sm()).color(color_alpha(self.dim, 60)));
            }
        });

        // ── Advanced: per-order-type price fields ─────────────────────────
        if adv {
            let oti = *s.order_type_idx;
            ui.add_space(2.0);
            if oti == 1 || oti == 3 {
                FormRow::new("Limit").leading_space(pad).label_width(32.0).hint("Limit price")
                    .show(ui, &t_stub, |ui| {
                        ui.add(egui::TextEdit::singleline(s.order_limit_price)
                            .desired_width(80.0).font(egui::FontId::monospace(9.0))
                            .horizontal_align(egui::Align::RIGHT));
                    });
            }
            if oti == 2 || oti == 3 {
                FormRow::new("Stop").leading_space(pad).label_width(32.0)
                    .label_color(self.bear).hint("Stop price")
                    .show(ui, &t_stub, |ui| {
                        ui.add(egui::TextEdit::singleline(s.order_stop_price)
                            .desired_width(80.0).font(egui::FontId::monospace(9.0))
                            .horizontal_align(egui::Align::RIGHT));
                    });
            }
            if oti == 4 {
                FormRow::new("Trail").leading_space(pad).label_width(32.0)
                    .label_color(self.accent).hint("Trail amt")
                    .show(ui, &t_stub, |ui| {
                        ui.add(egui::TextEdit::singleline(s.order_trail_amt)
                            .desired_width(80.0).font(egui::FontId::monospace(9.0))
                            .horizontal_align(egui::Align::RIGHT));
                    });
            }
        }

        // ── Advanced: Bracket + TP/SL ─────────────────────────────────────
        if adv {
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                ui.add_space(pad);
                let brk_color = if *s.order_bracket { self.accent } else { color_alpha(self.dim, 50) };
                if ui.add(egui::Button::new(
                        egui::RichText::new("Bracket").monospace().size(font_sm()).color(brk_color))
                    .fill(if *s.order_bracket { color_alpha(self.accent, 25) } else { egui::Color32::TRANSPARENT })
                    .stroke(Stroke::new(STROKE_THIN, color_alpha(self.toolbar_border, ALPHA_DIM)))
                    .corner_radius(2.0).min_size(egui::vec2(0.0, 18.0)))
                    .clicked()
                {
                    *s.order_bracket = !*s.order_bracket;
                }
                if *s.order_bracket {
                    ui.add_space(4.0);
                    ui.label(egui::RichText::new("TP").monospace().size(font_sm()).color(self.bull));
                    ui.add(egui::TextEdit::singleline(s.order_tp_price)
                        .desired_width(52.0).font(egui::FontId::monospace(10.0)).hint_text("Take")
                        .horizontal_align(egui::Align::RIGHT));
                    ui.label(egui::RichText::new("SL").monospace().size(font_sm()).color(self.bear));
                    ui.add(egui::TextEdit::singleline(s.order_sl_price)
                        .desired_width(52.0).font(egui::FontId::monospace(10.0)).hint_text("Stop")
                        .horizontal_align(egui::Align::RIGHT));
                }
            });
        }

        ui.add_space(4.0);

        // ── BUY / SELL ────────────────────────────────────────────────────
        let buy_price = if *s.order_market { last + spread }
            else { s.order_limit_price.parse::<f32>().unwrap_or(last) };
        let sell_price = if *s.order_market { last - spread }
            else { s.order_limit_price.parse::<f32>().unwrap_or(last) };
        ui.horizontal(|ui| {
            ui.add_space(pad);
            ui.spacing_mut().item_spacing.x = 4.0;
            let btn_w = (panel_w - pad * 2.0 - 8.0) / 2.0;
            let is_und = adv && *s.order_type_idx == 5 && s.is_option;
            let buy_label = if is_und {
                format!("BUY {} on UND", s.option_type)
            } else {
                format!("BUY {:.2}", buy_price)
            };
            let sell_label = if is_und {
                format!("SELL {} on UND", s.option_type)
            } else {
                format!("SELL {:.2}", sell_price)
            };
            if ui.add(TradeBtn::new(&buy_label).color(self.bull).width(btn_w)).clicked() {
                action = if is_und { ApertureAction::TriggerBuy }
                         else      { ApertureAction::Buy { price: buy_price } };
            }
            if ui.add(TradeBtn::new(&sell_label).color(self.bear).width(btn_w)).clicked() {
                action = if is_und { ApertureAction::TriggerSell }
                         else      { ApertureAction::Sell { price: sell_price } };
            }
        });
        ui.add_space(6.0);

        ApertureOrderOutcome { action }
    }
}

impl Default for ApertureOrderTicket {
    fn default() -> Self { Self::new() }
}

/// Build a Theme stub for sub-widgets (SegmentedControl, Stepper, FormRow, TradeBtn)
/// from the color fields the Aperture ticket carries.
fn aperture_stub_theme_full(
    dim: Color32, bull: Color32, bear: Color32, accent: Color32,
    toolbar_bg: Color32, toolbar_border: Color32,
) -> Theme {
    Theme {
        name:           "aperture-stub",
        bg:             toolbar_bg,
        bull,
        bear,
        dim,
        toolbar_bg,
        toolbar_border,
        accent,
        text:           Color32::from_rgb(220, 215, 205),
    }
}
