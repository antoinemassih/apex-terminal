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
        let base = self.color.unwrap_or(Color32::from_rgb(120, 120, 130));
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
        let c = self.color.unwrap_or(Color32::from_rgb(220, 80, 80));
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
        let c = self.color.unwrap_or(Color32::from_rgb(220, 80, 80));
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
                self.ok_color.unwrap_or(Color32::from_rgb(80, 200, 120)),
            ),
            ValidationState::Error => (
                "✗",
                self.err_color.unwrap_or(Color32::from_rgb(220, 80, 80)),
            ),
            ValidationState::Neutral => (
                "•",
                self.dim_color.unwrap_or(Color32::from_rgb(150, 150, 160)),
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
