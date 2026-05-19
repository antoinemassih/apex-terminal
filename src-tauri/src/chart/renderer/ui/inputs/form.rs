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
use crate::ui_kit::icons::Icon;
use crate::ui_kit::widgets::{FormRow, Tooltip};

// Shorthand for the Theme type used across the codebase.
type Theme = crate::chart_renderer::gpu::Theme;

/// Resolve the ambient theme stashed by the render loop. Used by `Widget`
/// impls and `show()` methods that don't receive a `&Theme` argument, so we
/// never have to fall back to `&THEMES[0]` (which would break light themes).
fn ambient_theme(ctx: &egui::Context) -> &'static Theme {
    crate::ui_kit::widgets::theme::active_theme(ctx)
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
        let base = self.color.unwrap_or_else(|| ambient_theme(ui.ctx()).dim);
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
        let c = self.color.unwrap_or_else(|| ambient_theme(ui.ctx()).bear);
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
        let c = self.color.unwrap_or_else(|| ambient_theme(ui.ctx()).bear);
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
        let amb = ambient_theme(ui.ctx());
        let (glyph, color) = match self.state {
            ValidationState::Ok => (
                Icon::CHECK,
                self.ok_color.unwrap_or(amb.bull),
            ),
            ValidationState::Error => (
                "✗",
                self.err_color.unwrap_or(amb.bear),
            ),
            ValidationState::Neutral => (
                "•",
                self.dim_color.unwrap_or(amb.dim),
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
pub struct MeridienOrderTicket<'a> {
    theme:   Option<&'a Theme>,
    bg:      Color32,
    text:    Color32,
    dim:     Color32,
    bull:    Color32,
    bear:    Color32,
    accent:  Color32,
    border:  Color32,
    width:   f32,
}

impl<'a> MeridienOrderTicket<'a> {
    pub fn new() -> Self {
        // Color fields are intentionally TRANSPARENT placeholders. Callers
        // MUST call `.theme(t)` before `.show(...)` — every call site does.
        // Avoids the `&THEMES[0]` light-theme bug.
        let z = Color32::TRANSPARENT;
        Self {
            theme:  None,
            bg:     z,
            text:   z,
            dim:    z,
            bull:   z,
            bear:   z,
            accent: z,
            border: z,
            width:  0.0,
        }
    }
    pub fn width(mut self, w: f32) -> Self { self.width = w; self }
    pub fn theme(mut self, t: &'a Theme) -> Self {
        self.theme  = Some(t);
        self.bg     = t.toolbar_bg;
        self.text   = t.text;
        self.dim    = t.dim;
        self.bull   = t.bull;
        self.bear   = t.bear;
        self.accent = t.accent;
        self.border = t.toolbar_border;
        self
    }

    /// Render the order ticket body — layout matches
    /// `design references/zed/goodLayout.png`:
    ///
    /// - **BID / SPREAD / ASK** card — three columns, tinted backings
    /// - **LIMIT + QUANTITY** form: shared label row, then stepper row
    /// - **Type / TIF** segmented pill rows
    /// - **EST. COST** card with embedded CTA on the right
    pub fn show(self, ui: &mut Ui, s: &mut OrderTicketState<'_>) -> OrderTicketOutcome {
        use crate::ui_kit::widgets::Button;
        use crate::ui_kit::widgets::tokens::Variant;
        use super::stepper::NumericStepper;

        let mut review_clicked = false;
        let theme_for_seg = self.theme;
        let panel_w = if self.width > 0.0 { self.width } else { ui.available_width() };
        let label_color = color_subtle(self.dim);
        let card_bg     = color_alpha(self.border, alpha_subtle());
        let card_radius = radius_md() as u8;

        // ── Layout constants — unified rhythm ─────────────────────────
        let outer_pad      = gap_sm();          // card outer margin from pane edges
        let inner_pad      = gap_sm();          // padding inside cards
        let row_gap        = gap_xs();          // gap between section rows
        let col_gap        = gap_sm();          // gap between left/right columns
        let strip_h        = 48.0_f32;
        let stepper_h      = 26.0_f32;
        let pill_h         = 22.0_f32;
        let cost_card_h    = 52.0_f32;
        let cta_h          = 40.0_f32;
        let cta_w          = 96.0_f32;

        // ── Pad pane content inward from the chrome edges ────────────
        let inner_w = panel_w - outer_pad * 2.0;
        let half_w  = (inner_w - col_gap) / 2.0;

        let section_label = |ui: &mut Ui, txt: &str| {
            ui.label(RichText::new(style_label_case(txt))
                .monospace().size(font_xs()).color(label_color));
        };

        ui.set_width(panel_w);
        ui.spacing_mut().item_spacing.y = row_gap;
        ui.add_space(outer_pad);

        // Active-side accent strip along the left edge of the body content.
        let side_color = if *s.is_buy { self.bull } else { self.bear };
        let body_top = ui.cursor().min.y;
        // Reserved here; final rect drawn after the body is laid out below.
        let _ = (side_color, body_top);

        // Helper: render two equal-width columns with a fixed gap.
        let two_col = |ui: &mut Ui,
                       left: &mut dyn FnMut(&mut Ui),
                       right: &mut dyn FnMut(&mut Ui)| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                ui.add_space(outer_pad);
                ui.allocate_ui_with_layout(
                    Vec2::new(half_w, 0.0),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| { ui.set_width(half_w); left(ui); });
                ui.add_space(col_gap);
                ui.allocate_ui_with_layout(
                    Vec2::new(half_w, 0.0),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| { ui.set_width(half_w); right(ui); });
                ui.add_space(outer_pad);
            });
        };

        // ─── BID / SPREAD / ASK card ───────────────────────────────────
        let bid_w    = (inner_w * 0.40).floor();
        let spread_w = (inner_w * 0.20).floor();
        let ask_w    = inner_w - bid_w - spread_w;
        let strip_top = egui::pos2(ui.cursor().min.x + outer_pad, ui.cursor().min.y);
        let strip_rect = egui::Rect::from_min_size(strip_top, Vec2::new(inner_w, strip_h));

        ui.painter().rect_filled(strip_rect,
            egui::CornerRadius::same(card_radius), card_bg);

        let bid_rect = egui::Rect::from_min_size(strip_top, Vec2::new(bid_w, strip_h));
        let ask_rect = egui::Rect::from_min_size(
            egui::pos2(strip_top.x + bid_w + spread_w, strip_top.y),
            Vec2::new(ask_w, strip_h));

        ui.painter().rect_filled(bid_rect,
            egui::CornerRadius { nw: card_radius, ne: 0, sw: card_radius, se: 0 },
            color_alpha(self.bear, alpha_subtle()));
        ui.painter().rect_filled(ask_rect,
            egui::CornerRadius { nw: 0, ne: card_radius, sw: 0, se: card_radius },
            color_alpha(self.bull, alpha_subtle()));

        // Click BID/ASK to snap the limit price. Brighter tint on hover.
        let bid_resp = ui.interact(bid_rect,
            egui::Id::new(("meridien_bid", strip_top.x as i32, strip_top.y as i32)),
            egui::Sense::click());
        if bid_resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            ui.painter().rect_filled(bid_rect,
                egui::CornerRadius { nw: card_radius, ne: 0, sw: card_radius, se: 0 },
                color_alpha(self.bear, alpha_muted()));
        }
        if bid_resp.clicked() {
            *s.limit_price = format!("{:.2}", s.bid);
            if *s.order_type_idx == 0 {
                *s.order_type_idx = 1; *s.order_market = false;
            }
        }
        let ask_resp = ui.interact(ask_rect,
            egui::Id::new(("meridien_ask", strip_top.x as i32, strip_top.y as i32)),
            egui::Sense::click());
        if ask_resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            ui.painter().rect_filled(ask_rect,
                egui::CornerRadius { nw: 0, ne: card_radius, sw: 0, se: card_radius },
                color_alpha(self.bull, alpha_muted()));
        }
        if ask_resp.clicked() {
            *s.limit_price = format!("{:.2}", s.ask);
            if *s.order_type_idx == 0 {
                *s.order_type_idx = 1; *s.order_market = false;
            }
        }

        // Re-paint the BID/ASK text on top of the hover overlay so labels
        // stay visible during hover (rect_filled is painted last).
        let _ = (bid_resp, ask_resp);

        // BID
        ui.painter().text(
            egui::pos2(bid_rect.left() + inner_pad, bid_rect.center().y - 9.0),
            egui::Align2::LEFT_CENTER, "BID",
            egui::FontId::monospace(font_xs()), self.bear);
        ui.painter().text(
            egui::pos2(bid_rect.left() + inner_pad, bid_rect.center().y + 7.0),
            egui::Align2::LEFT_CENTER, &format!("{:.2}", s.bid),
            egui::FontId::proportional(font_lg()), self.text);

        // SPREAD
        let spread_cx = strip_top.x + bid_w + spread_w * 0.5;
        ui.painter().text(
            egui::pos2(spread_cx, strip_rect.center().y - 5.0),
            egui::Align2::CENTER_CENTER,
            &format!("{:.2}", (s.ask - s.bid).abs()),
            egui::FontId::monospace(font_sm()), self.text);
        ui.painter().text(
            egui::pos2(spread_cx, strip_rect.center().y + 8.0),
            egui::Align2::CENTER_CENTER, "SPREAD",
            egui::FontId::monospace(font_xs()), label_color);

        // ASK
        ui.painter().text(
            egui::pos2(ask_rect.right() - inner_pad, ask_rect.center().y - 9.0),
            egui::Align2::RIGHT_CENTER, "ASK",
            egui::FontId::monospace(font_xs()), self.bull);
        ui.painter().text(
            egui::pos2(ask_rect.right() - inner_pad, ask_rect.center().y + 7.0),
            egui::Align2::RIGHT_CENTER, &format!("{:.2}", s.ask),
            egui::FontId::proportional(font_lg()), self.text);

        ui.allocate_exact_size(Vec2::new(panel_w, strip_h), egui::Sense::hover());

        // ─── LIMIT + QUANTITY label row ───────────────────────────────
        two_col(ui,
            &mut |ui| {
                ui.horizontal(|ui| {
                    section_label(ui, "LIMIT");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(RichText::new(format!("mid {:.2}", s.last))
                            .monospace().size(font_xs()).color(label_color));
                    });
                });
            },
            &mut |ui| {
                ui.horizontal(|ui| {
                    section_label(ui, "QUANTITY");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        for &(pct, lbl) in &[(1.0_f32, "100%"), (0.5, "50%"), (0.25, "25%")] {
                            if ui.add(Button::new(lbl)
                                    .variant(Variant::Chrome)
                                    .min_size(Vec2::new(30.0, 16.0))).clicked() {
                                let bp = s.buying_power.max(0.0);
                                if s.last > 0.0 {
                                    *s.order_qty = (bp * pct / s.last) as u32;
                                }
                            }
                        }
                    });
                });
            });

        // ─── LIMIT + QUANTITY stepper row ─────────────────────────────
        let is_market = *s.order_market;
        two_col(ui,
            &mut |ui| {
                if is_market {
                    // Market order — no editable price; show "AT MARKET" muted.
                    let (rect, _) = ui.allocate_exact_size(
                        Vec2::new(ui.available_width(), stepper_h),
                        egui::Sense::hover());
                    ui.painter().rect_filled(rect,
                        egui::CornerRadius::same(radius_sm() as u8),
                        color_alpha(self.border, alpha_subtle()));
                    ui.painter().rect_stroke(rect,
                        egui::CornerRadius::same(radius_sm() as u8),
                        Stroke::new(stroke_std(), color_alpha(self.border, alpha_line())),
                        egui::StrokeKind::Inside);
                    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER,
                        "AT MARKET",
                        egui::FontId::monospace(font_sm()),
                        label_color);
                } else {
                    let mut stepper = NumericStepper::new(s.limit_price)
                        .prefix("$")
                        .step(0.01)
                        .min(0.0)
                        .height(stepper_h)
                        .decimals(2);
                    if let Some(t) = theme_for_seg { stepper = stepper.theme(t); }
                    stepper.show(ui);
                }
            },
            &mut |ui| {
                let mut stepper = NumericStepper::new(s.order_qty)
                    .step(1.0)
                    .min(1.0)
                    .height(stepper_h);
                if let Some(t) = theme_for_seg { stepper = stepper.theme(t); }
                stepper.show(ui);
            });

        // ─── Type & TIF — pill toggle + caret dropdown ────────────────
        let order_types: [(usize, &str); 5] = [
            (0, "MKT"), (1, "LMT"), (2, "STP"), (3, "STP-LMT"), (4, "TRAIL"),
        ];
        let tifs: [(usize, &str); 4] = [
            (0, "DAY"), (1, "GTC"), (2, "IOC"), (3, "FOK"),
        ];

        let render_pill_with_more = |ui: &mut Ui,
                                     state: &mut usize,
                                     options: &[(usize, &str)],
                                     id_salt: &str,
                                     col_w: f32| {
            let default_lbl = options[0].1;
            let other_idx = if *state == 0 { 1 } else { *state };
            let other_lbl = options.get(other_idx).map(|(_, l)| *l)
                .unwrap_or(options[1].1);
            let is_default = *state == 0;
            let caret_w = 14.0_f32;
            let pill_w = ((col_w - caret_w) / 2.0).floor();
            ui.allocate_ui_with_layout(
                Vec2::new(col_w, pill_h),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    ui.spacing_mut().button_padding = Vec2::new(2.0, 0.0);
                    let dflt = ui.add(Button::new(default_lbl)
                        .variant(Variant::Chrome)
                        .active(is_default)
                        .corner_radius(0.0)
                        .min_size(Vec2::new(pill_w, pill_h)));
                    if dflt.clicked() { *state = 0; }
                    let othr = ui.add(Button::new(other_lbl)
                        .variant(Variant::Chrome)
                        .active(!is_default)
                        .corner_radius(0.0)
                        .min_size(Vec2::new(pill_w, pill_h)));
                    if othr.clicked() && is_default { *state = other_idx; }
                    let caret = ui.add(Button::icon(Icon::CARET_DOWN)
                        .variant(Variant::Chrome)
                        .glyph_size(9.0)
                        .min_size(Vec2::new(caret_w, pill_h))
                        .corner_radius(0.0)
                        .placement(crate::ui_kit::widgets::icon_placement::IconPlacement::ListRow))
                        .on_hover_text("More options");
                    let popup_id = ui.make_persistent_id(("pill_more", id_salt));
                    if caret.clicked() {
                        ui.memory_mut(|m| m.toggle_popup(popup_id));
                    }
                    egui::popup::popup_below_widget(
                        ui,
                        popup_id,
                        &caret,
                        egui::PopupCloseBehavior::CloseOnClickOutside,
                        |ui| {
                            ui.set_min_width(80.0);
                            for (i, lbl) in options.iter().skip(1) {
                                let active = *state == *i;
                                if ui.selectable_label(active,
                                    RichText::new(*lbl).monospace().size(font_sm())).clicked() {
                                    *state = *i;
                                    ui.memory_mut(|m| m.close_popup());
                                }
                            }
                        });
                });
        };

        two_col(ui,
            &mut |ui| {
                let prev = *s.order_type_idx;
                render_pill_with_more(ui, s.order_type_idx, &order_types, "ot_type", half_w);
                if prev != *s.order_type_idx {
                    *s.order_market = *s.order_type_idx == 0;
                }
            },
            &mut |ui| {
                render_pill_with_more(ui, s.order_tif_idx, &tifs, "ot_tif", half_w);
            });

        // ─── EST. COST card with embedded CTA ─────────────────────────
        ui.add_space(row_gap);
        let notional: f32 = s.last * (*s.order_qty as f32);
        let bp_after: f32 = (s.buying_power - notional).max(0.0);
        let cost_top = egui::pos2(ui.cursor().min.x + outer_pad, ui.cursor().min.y);
        let cost_rect = egui::Rect::from_min_size(cost_top, Vec2::new(inner_w, cost_card_h));
        ui.painter().rect_filled(cost_rect,
            egui::CornerRadius::same(card_radius), card_bg);

        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.add_space(outer_pad + inner_pad);
            let cost_text_w = (inner_w - cta_w - inner_pad * 2.0).max(120.0);
            ui.allocate_ui_with_layout(
                Vec2::new(cost_text_w, cost_card_h),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ui.set_width(cost_text_w);
                    ui.spacing_mut().item_spacing.y = 1.0;
                    ui.add_space(gap_xs());
                    section_label(ui, "EST. COST");
                    ui.label(RichText::new(format!("${:.2}", notional))
                        .monospace().size(font_md()).strong().color(self.text));
                    ui.label(RichText::new(format!("@ ${:.2} \u{00B7} BP ${:.0} after",
                            s.last, bp_after))
                        .monospace().size(font_xs()).color(label_color));
                });
            // CTA aligned vertically centered inside card
            ui.allocate_ui_with_layout(
                Vec2::new(cta_w + inner_pad, cost_card_h),
                egui::Layout::right_to_left(egui::Align::Center),
                |ui| {
                    ui.add_space(inner_pad);
                    let side_str = if *s.is_buy { "Buy" } else { "Sell" };
                    let qty_sub  = format!("{}", *s.order_qty);
                    let cta_btn = if *s.is_buy {
                        Button::buy(side_str)
                    } else {
                        Button::sell(side_str)
                    }.sublabel(qty_sub).fg(crate::chart_renderer::ui::style::contrast_fg(side_color));
                    if ui.add(cta_btn.min_size(Vec2::new(cta_w, cta_h))).clicked() {
                        review_clicked = true;
                    }
                });
            ui.add_space(outer_pad);
        });
        ui.allocate_exact_size(Vec2::new(panel_w, 0.0), egui::Sense::hover());

        ui.add_space(outer_pad);

        // Side-color accent strip along the left edge.
        let body_bottom = ui.cursor().min.y;
        let accent_rect = egui::Rect::from_min_size(
            egui::pos2(ui.max_rect().left(), body_top),
            Vec2::new(2.0, (body_bottom - body_top).max(0.0)),
        );
        ui.painter().rect_filled(accent_rect, egui::CornerRadius::ZERO, side_color);

        OrderTicketOutcome { review_clicked }
    }
}

impl<'a> Default for MeridienOrderTicket<'a> {
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
        use crate::ui_kit::widgets::Button;
        use crate::ui_kit::widgets::tokens::{Variant, Size};
        let amb = ambient_theme(ui.ctx());
        let accent = self.accent.unwrap_or(amb.accent);
        let dim = self.dim.unwrap_or(amb.dim);
        let border = self.border.unwrap_or(amb.toolbar_border);
        let value = self.value;
        let mut changed = false;
        let theme = self.theme.unwrap_or(amb);

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
                ui.spacing_mut().item_spacing.x = gap_xs();
                for &pr in self.presets {
                    let sel = *value == pr;
                    let fg = if sel { accent } else { color_half(dim) };
                    let pr_label = format!("{}", pr);
                    if Button::new(pr_label.as_str()).variant(Variant::Chrome).size(Size::Xs).fg(fg)
                        .fill(if sel { color_alpha(accent, alpha_soft()) } else { Color32::TRANSPARENT })
                        .corner_radius(crate::chart_renderer::ui::style::current().r_xs as f32)
                        .min_size(egui::vec2(22.0, row_height_dense())).show(ui, theme).clicked() && !sel
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
        use crate::ui_kit::widgets::Button;
        use crate::ui_kit::widgets::tokens::{Variant, Size};
        let amb = ambient_theme(ui.ctx());
        let accent = self.accent.unwrap_or(amb.accent);
        let dim = self.dim.unwrap_or(amb.dim);
        let d = self.decimals;
        let value = self.value;
        // Treat 0.0 as "use default"
        if *value <= 0.0 { *value = self.default; }
        let mut changed = false;
        let theme = self.theme.unwrap_or(amb);

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
                ui.spacing_mut().item_spacing.x = gap_xs();
                for &pr in self.presets {
                    let sel = (*value - pr).abs() < 0.01;
                    let fg = if sel { accent } else { color_half(dim) };
                    let pr_label = format!("{:.prec$}", pr, prec = d);
                    if Button::new(pr_label.as_str()).variant(Variant::Chrome).size(Size::Xs).fg(fg)
                        .fill(if sel { color_alpha(accent, alpha_soft()) } else { Color32::TRANSPARENT })
                        .corner_radius(crate::chart_renderer::ui::style::current().r_xs as f32)
                        .min_size(egui::vec2(22.0, row_height_dense())).show(ui, theme).clicked() && !sel
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
        use crate::ui_kit::widgets::Button;
        use crate::ui_kit::widgets::tokens::Size as KitSize;
        use crate::ui_kit::widgets::Input;

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
                ui.add_space(gap_lg());
                let tif_opts: &[(usize, &str)] = &[(0, "DAY"), (1, "GTC"), (2, "IOC")];
                SegmentedControl::new()
                    .options(tif_opts)
                    .theme(&t_stub)
                    .show(ui, s.order_tif_idx);
                ui.add_space(gap_md());
                let rth_amber = COLOR_AMBER;
                let rth_fg = if *s.order_outside_rth { rth_amber } else { color_alpha(self.dim, 40) };
                let rth_bg = if *s.order_outside_rth { color_alpha(rth_amber, 30) } else { egui::Color32::TRANSPARENT };
                let rth_stroke = Stroke::new(stroke_thin(), if *s.order_outside_rth {
                    color_alpha(rth_amber, 80)
                } else {
                    color_alpha(self.toolbar_border, 40)
                });
                let r = ui.add(egui::Button::new(
                        egui::RichText::new("EXT").monospace().size(font_xs()).color(rth_fg))
                    .fill(rth_bg).corner_radius(r_xs()).stroke(rth_stroke)
                    .min_size(egui::vec2(26.0, row_height_dense())));
                Tooltip::new("Trade outside regular trading hours").show(ui, &r, &t_stub);
                if r.clicked() {
                    *s.order_outside_rth = !*s.order_outside_rth;
                }
            });
            ui.add_space(gap_sm());
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
                ui.add_space(gap_sm());
                let premium = last;
                let mult    = if s.is_option { 100.0_f32 } else { 1.0_f32 };
                Input::new(s.order_notional_amount)
                    .placeholder("Amount").width(70.0)
                    .show(ui, &t_stub);
                let notional: f32 = s.order_notional_amount.parse().unwrap_or(0.0);
                let qty = if premium > 0.0 && mult > 0.0 {
                    (notional / (premium * mult)).floor() as i32
                } else { 0 };
                if qty > 0 { *s.order_qty = qty as u32; }
                ui.label(egui::RichText::new(format!("= {} @ {:.2}", qty, premium))
                    .monospace().size(font_sm()).color(color_alpha(self.dim, 60)));
            }
        });
        ui.add_space(gap_xs());

        // ── QTY stepper + compact price / MKT-LMT ────────────────────────
        ui.horizontal(|ui| {
            ui.add_space(pad);
            ui.spacing_mut().item_spacing.x = gap_xs();
            let step = if *s.order_qty >= 100 { 10u32 }
                       else if *s.order_qty >= 10 { 5 }
                       else { 1 };
            if !*s.order_notional_mode {
                Stepper::new(s.order_qty)
                    .step(step).range(1, u32::MAX)
                    .theme(&t_stub)
                    .show(ui);
            } else {
                let mut qty_display = format!("{} contracts", s.order_qty);
                Input::new(&mut qty_display)
                    .disabled(true).width(100.0)
                    .horizontal_align(egui::Align::Center)
                    .show(ui, &t_stub);
            }
            ui.add_space(gap_sm());
            let cursor = ui.cursor().min;
            ui.painter().line_segment(
                [egui::pos2(cursor.x, cursor.y), egui::pos2(cursor.x, cursor.y + 20.0)],
                Stroke::new(stroke_std(), color_alpha(self.toolbar_border, 80)));
            ui.add_space(gap_md());
            if !adv {
                if *s.order_market {
                    ui.label(egui::RichText::new(format!("{:.2}", last))
                        .monospace().size(font_md()).color(self.dim));
                } else {
                    Input::new(s.order_limit_price)
                        .placeholder("Price").width(68.0)
                        .horizontal_align(egui::Align::RIGHT)
                        .show(ui, &t_stub);
                }
                ui.add_space(gap_xs());
                let mkt_label = if *s.order_market { "MKT" } else { "LMT" };
                if ui.add(egui::Button::new(
                        egui::RichText::new(mkt_label).monospace().size(font_sm()).strong()
                            .color(if *s.order_market { self.accent } else { self.dim }))
                    .fill(if *s.order_market { color_alpha(self.accent, 35) } else { self.toolbar_bg })
                    .stroke(Stroke::new(stroke_thin(), color_alpha(self.toolbar_border, 90))).corner_radius(r_xs())
                    .min_size(egui::vec2(30.0, row_height_compact())))
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
            ui.add_space(gap_xs());
            if oti == 1 || oti == 3 {
                FormRow::new("Limit").leading_space(pad).label_width(32.0).hint("Limit price")
                    .show(ui, &t_stub, |ui| {
                        Input::new(s.order_limit_price)
                            .width(80.0)
                            .horizontal_align(egui::Align::RIGHT)
                            .show(ui, &t_stub);
                    });
            }
            if oti == 2 || oti == 3 {
                FormRow::new("Stop").leading_space(pad).label_width(32.0)
                    .label_color(self.bear).hint("Stop price")
                    .show(ui, &t_stub, |ui| {
                        Input::new(s.order_stop_price)
                            .width(80.0)
                            .horizontal_align(egui::Align::RIGHT)
                            .show(ui, &t_stub);
                    });
            }
            if oti == 4 {
                FormRow::new("Trail").leading_space(pad).label_width(32.0)
                    .label_color(self.accent).hint("Trail amt")
                    .show(ui, &t_stub, |ui| {
                        Input::new(s.order_trail_amt)
                            .width(80.0)
                            .horizontal_align(egui::Align::RIGHT)
                            .show(ui, &t_stub);
                    });
            }
        }

        // ── Advanced: Bracket + TP/SL ─────────────────────────────────────
        if adv {
            ui.add_space(gap_xs());
            ui.horizontal(|ui| {
                ui.add_space(pad);
                let brk_color = if *s.order_bracket { self.accent } else { color_alpha(self.dim, 50) };
                if ui.add(egui::Button::new(
                        egui::RichText::new("Bracket").monospace().size(font_sm()).color(brk_color))
                    .fill(if *s.order_bracket { color_alpha(self.accent, 25) } else { egui::Color32::TRANSPARENT })
                    .stroke(Stroke::new(STROKE_THIN, color_alpha(self.toolbar_border, ALPHA_DIM)))
                    .corner_radius(r_xs()).min_size(egui::vec2(0.0, row_height_dense())))
                    .clicked()
                {
                    *s.order_bracket = !*s.order_bracket;
                }
                if *s.order_bracket {
                    ui.add_space(gap_sm());
                    ui.label(egui::RichText::new("TP").monospace().size(font_sm()).color(self.bull));
                    Input::new(s.order_tp_price)
                        .placeholder("Take").width(52.0)
                        .horizontal_align(egui::Align::RIGHT)
                        .show(ui, &t_stub);
                    ui.label(egui::RichText::new("SL").monospace().size(font_sm()).color(self.bear));
                    Input::new(s.order_sl_price)
                        .placeholder("Stop").width(52.0)
                        .horizontal_align(egui::Align::RIGHT)
                        .show(ui, &t_stub);
                }
            });
        }

        ui.add_space(gap_sm());

        // ── BUY / SELL ────────────────────────────────────────────────────
        let buy_price = if *s.order_market { last + spread }
            else { s.order_limit_price.parse::<f32>().unwrap_or(last) };
        let sell_price = if *s.order_market { last - spread }
            else { s.order_limit_price.parse::<f32>().unwrap_or(last) };
        ui.horizontal(|ui| {
            ui.add_space(pad);
            ui.spacing_mut().item_spacing.x = gap_sm();
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
            if Button::buy(buy_label.as_str())
                .min_size(egui::vec2(btn_w, btn_trade_height()))
                .size(KitSize::Md)
                .show(ui, &t_stub).clicked() {
                action = if is_und { ApertureAction::TriggerBuy }
                         else      { ApertureAction::Buy { price: buy_price } };
            }
            if Button::sell(sell_label.as_str())
                .min_size(egui::vec2(btn_w, btn_trade_height()))
                .size(KitSize::Md)
                .show(ui, &t_stub).clicked() {
                action = if is_und { ApertureAction::TriggerSell }
                         else      { ApertureAction::Sell { price: sell_price } };
            }
        });
        ui.add_space(gap_md());

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
        border_variant: crate::chart_renderer::gpu::hairline_border_variant(toolbar_bg),
        accent,
        text:           Color32::from_rgb(220, 215, 205),
        warn:               crate::chart_renderer::ui::style::COLOR_AMBER,
        notification_red:   COLOR_LOSS_RED,
        gold:               Color32::from_rgb(255, 193, 37),
        shadow_color:       Color32::from_rgb(0, 0, 0),
        overlay_text:       Color32::from_rgb(240, 240, 250),
        rrg_leading:        Color32::from_rgb(56, 203, 137),
        rrg_improving:      COLOR_INFO_CYAN,
        rrg_weakening:      Color32::from_rgb(230, 200, 50),
        rrg_lagging:        Color32::from_rgb(224, 82, 82),
        cmd_palette:        crate::chart_renderer::gpu::CMD_PALETTE_DEFAULT,
        pinned_row_tint:    Color32::from_rgba_premultiplied(3, 5, 9, 12),
        text_muted:         Color32::from_rgb(180, 180, 195),
        hud_bg:             Color32::from_rgba_premultiplied(12, 12, 18, 230),
        hud_border:         Color32::from_rgb(50, 52, 64),
        element_hover:      crate::chart_renderer::gpu::alpha(Color32::from_rgb(220, 215, 205), 12),
        element_active:     crate::chart_renderer::gpu::alpha(Color32::from_rgb(220, 215, 205), 24),
        element_selected:   crate::chart_renderer::gpu::alpha(accent, 24),
        element_disabled:   crate::chart_renderer::gpu::alpha(dim, 80),
        ghost_hover:        crate::chart_renderer::gpu::alpha(Color32::from_rgb(220, 215, 205), 6),
        ghost_active:       crate::chart_renderer::gpu::alpha(Color32::from_rgb(220, 215, 205), 12),
        icon:               Color32::from_rgb(220, 215, 205),
        icon_muted:         crate::chart_renderer::gpu::alpha(Color32::from_rgb(220, 215, 205), 178),
        icon_disabled:      crate::chart_renderer::gpu::alpha(Color32::from_rgb(220, 215, 205), 102),
        icon_accent:        accent,
    }
}
