//! Foundation shells — the base composables every concrete widget will
//! eventually wrap. Each shell owns spacing/colors/hover/border/radius for one
//! family of UI element so that families on top stay declarative.
//!
//! Wave 4.5b will migrate existing widgets onto these shells.

#![allow(dead_code, unused_imports)]

use egui::{Color32, CornerRadius, Margin, Rect, Response, RichText, Sense, Stroke, StrokeKind, Ui, Vec2};

use super::super::style::*;
use super::interaction::{apply_interaction, HoverTreatment, InteractionState, InteractionTokens};
use super::text_style::TextStyle;
use super::tokens::{Radius, Size};
use super::variants::{ButtonVariant, CardVariant, ChipVariant, InputVariant, RowVariant};

type Theme = super::super::super::gpu::Theme;

// ─── ButtonShell ─────────────────────────────────────────────────────────────

#[must_use = "ButtonShell must be drawn with .show(ui)"]
pub struct ButtonShell<'a> {
    label: &'a str,
    variant: ButtonVariant,
    size: Size,
    radius: Radius,
    icon: Option<&'a str>,
    state: InteractionState,
    theme: &'a Theme,
    tokens: InteractionTokens,
    treatment: Option<ButtonTreatment>,
    active: bool,
    color: Option<Color32>,
    hover_treatment: Option<HoverTreatment>,
    size_explicit: Option<Vec2>,
}

impl<'a> ButtonShell<'a> {
    pub fn new(theme: &'a Theme, label: &'a str) -> Self {
        Self {
            label, variant: ButtonVariant::Secondary, size: Size::Md, radius: Radius::Sm,
            icon: None, state: InteractionState::default(), theme,
            tokens: InteractionTokens::default(),
            treatment: None, active: false, color: None,
            hover_treatment: None, size_explicit: None,
        }
    }
    pub fn variant(mut self, v: ButtonVariant) -> Self { self.variant = v; self }
    pub fn size(mut self, s: Size) -> Self { self.size = s; self }
    pub fn radius(mut self, r: Radius) -> Self { self.radius = r; self }
    pub fn icon(mut self, icon: &'a str) -> Self { self.icon = Some(icon); self }
    pub fn state(mut self, s: InteractionState) -> Self { self.state = s; self }
    pub fn tokens(mut self, t: InteractionTokens) -> Self { self.tokens = t; self }
    /// Per-instance ButtonTreatment override. When unset, defaults to
    /// `style::current().button_treatment`.
    pub fn treatment(mut self, t: ButtonTreatment) -> Self { self.treatment = Some(t); self }
    /// Active-state flag; treatment-specific painting (stripe / fill / black).
    pub fn active(mut self, v: bool) -> Self { self.active = v; self }
    /// Per-instance accent color override (used for fill + hover).
    pub fn color(mut self, c: Color32) -> Self { self.color = Some(c); self }
    /// Override the hover treatment (default depends on variant).
    pub fn hover_treatment(mut self, h: HoverTreatment) -> Self { self.hover_treatment = Some(h); self }
    /// Explicit (width, height) override that bypasses Size's height.
    pub fn size_explicit(mut self, w: f32, h: f32) -> Self { self.size_explicit = Some(Vec2::new(w, h)); self }

    /// Pick the default hover treatment for a given variant.
    fn default_hover(v: ButtonVariant) -> HoverTreatment {
        match v {
            ButtonVariant::Brand | ButtonVariant::Primary | ButtonVariant::Destructive
                => HoverTreatment::WhiteVeil(12),
            _   => HoverTreatment::AccentTint,
        }
    }

    pub fn show(self, ui: &mut Ui) -> Response {
        let _pad = self.size.padding();
        let treatment = self.treatment.unwrap_or_else(|| current().button_treatment);
        let raw_accent = self.color.unwrap_or_else(|| self.variant.fill_color(self.theme));
        // accent_emphasis multiplies brightness/saturation for active elements.
        let accent = if self.active { accent_emphasised(raw_accent) } else { raw_accent };
        let base_fg     = self.variant.fg_color(self.theme);
        let base_border = self.variant.border_color(self.theme);

        // Compute treatment-aware base fill / fg / border (depends on active state).
        let (base_fill, fg_for_text, treatment_border) = match treatment {
            ButtonTreatment::SoftPill => {
                if self.active {
                    (color_alpha(accent, alpha_active()), contrast_fg(accent), color_alpha(accent, alpha_active()))
                } else {
                    (self.variant.fill_color(self.theme), base_fg, base_border)
                }
            }
            ButtonTreatment::OutlineAccent => {
                if self.active {
                    (Color32::TRANSPARENT, accent, color_alpha(accent, alpha_active()))
                } else {
                    (Color32::TRANSPARENT, base_fg, base_border)
                }
            }
            ButtonTreatment::UnderlineActive => {
                (Color32::TRANSPARENT, base_fg, Color32::TRANSPARENT)
            }
            ButtonTreatment::RaisedActive => {
                if self.active {
                    (accent, contrast_fg(accent), color_alpha(accent, alpha_active()))
                } else {
                    (self.variant.fill_color(self.theme), base_fg, base_border)
                }
            }
            ButtonTreatment::BlackFillActive => {
                if self.active {
                    (Color32::BLACK, Color32::WHITE, Color32::BLACK)
                } else {
                    (self.variant.fill_color(self.theme), base_fg, base_border)
                }
            }
        };

        let label_rich = TextStyle::Body.as_rich(self.label, fg_for_text);
        let mut text_label = if let Some(icon) = self.icon {
            RichText::new(format!("{icon} {}", self.label)).size(self.size.font()).color(fg_for_text)
        } else {
            label_rich
        };
        if self.state.disabled {
            let f = self.tokens.disabled_opacity;
            text_label = text_label.color(color_alpha(fg_for_text, (255.0 * f) as u8));
        }

        let min_size = self.size_explicit.unwrap_or(Vec2::new(0.0, self.size.height()));

        let resp = ui.add(
            egui::Button::new(text_label)
                .fill(base_fill)
                .stroke(Stroke::new(stroke_thin(), treatment_border))
                .corner_radius(self.radius.corner())
                .min_size(min_size)
        );

        // Treatment-specific extras (stripe under UnderlineActive when active).
        // Uses current().stroke_bold so style presets can adjust the active emphasis weight.
        if matches!(treatment, ButtonTreatment::UnderlineActive) && self.active {
            let y = resp.rect.bottom() - 1.0;
            ui.painter().line_segment(
                [egui::pos2(resp.rect.left(), y), egui::pos2(resp.rect.right(), y)],
                Stroke::new(current().stroke_bold, accent),
            );
        }

        // Apply hover/focus/press overlay using the per-instance color.
        let mut tokens = self.tokens;
        tokens.hover_treatment = self.hover_treatment
            .unwrap_or_else(|| Self::default_hover(self.variant));
        let st = self.state
            .hovered(resp.hovered())
            .pressed(resp.is_pointer_button_down_on())
            .focused(resp.has_focus());
        let v = apply_interaction(resp.rect, st, accent, &tokens);
        if v.fill != Color32::TRANSPARENT {
            ui.painter().rect_filled(resp.rect, self.radius.corner(), v.fill);
        }
        if v.stroke.width > 0.0 {
            ui.painter().rect_stroke(resp.rect, self.radius.corner(), v.stroke, StrokeKind::Inside);
        }
        if resp.hovered() && !st.disabled {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        resp
    }
}

// ─── RowShell ────────────────────────────────────────────────────────────────

#[must_use = "RowShell must be drawn with .show(ui)"]
pub struct RowShell<'a> {
    theme: &'a Theme,
    variant: RowVariant,
    size: Size,
    primary: &'a str,
    secondary: Option<&'a str>,
    leading: Option<Box<dyn FnOnce(&mut Ui) + 'a>>,
    trailing: Option<Box<dyn FnOnce(&mut Ui) + 'a>>,
    state: InteractionState,
    tokens: InteractionTokens,
    show_divider: bool,
    painter_mode: bool,
    painter_body: Option<Box<dyn FnOnce(&mut Ui, Rect) + 'a>>,
    painter_height: Option<f32>,
}

impl<'a> RowShell<'a> {
    pub fn new(theme: &'a Theme, primary: &'a str) -> Self {
        Self {
            theme, variant: RowVariant::Default, size: Size::Md, primary,
            secondary: None, leading: None, trailing: None,
            state: InteractionState::default(), tokens: InteractionTokens::default(),
            show_divider: false,
            painter_mode: false, painter_body: None, painter_height: None,
        }
    }
    /// Switch the row to painter mode — the shell allocates an exact-size
    /// strip and runs `body(ui, rect)` instead of using slot closures.
    pub fn painter_mode(mut self, v: bool) -> Self { self.painter_mode = v; self }
    /// Body closure used when `painter_mode == true`. Takes the full row rect.
    pub fn painter_body(mut self, f: impl FnOnce(&mut Ui, Rect) + 'a) -> Self {
        self.painter_body = Some(Box::new(f)); self
    }
    /// Optional height override for painter_mode (defaults to Size's height).
    pub fn painter_height(mut self, h: f32) -> Self { self.painter_height = Some(h); self }
    pub fn variant(mut self, v: RowVariant) -> Self { self.variant = v; self }
    pub fn size(mut self, s: Size) -> Self { self.size = s; self }
    pub fn secondary(mut self, s: &'a str) -> Self { self.secondary = Some(s); self }
    pub fn state(mut self, s: InteractionState) -> Self { self.state = s; self }
    pub fn divider(mut self, v: bool) -> Self { self.show_divider = v; self }
    pub fn leading(mut self, f: impl FnOnce(&mut Ui) + 'a) -> Self {
        self.leading = Some(Box::new(f)); self
    }
    pub fn trailing(mut self, f: impl FnOnce(&mut Ui) + 'a) -> Self {
        self.trailing = Some(Box::new(f)); self
    }

    pub fn show(self, ui: &mut Ui) -> Response {
        let pad = self.size.padding();
        let fg = self.variant.fg_color(self.theme);
        let border = self.variant.border_color(self.theme);
        let base_fill = self.variant.fill_color(self.theme);

        // ── Painter-mode escape hatch ────────────────────────────────────
        if self.painter_mode {
            // Use style_row_height() as the density-aware default row height so the
            // `row_height_px` knob in the inspector drives all RowShell painter rows.
            let h = self.painter_height.unwrap_or_else(style_row_height);
            let avail_w = ui.available_width();
            let (rect, click) = ui.allocate_exact_size(
                Vec2::new(avail_w, h),
                Sense::click(),
            );
            // Paint base fill.
            if base_fill != Color32::TRANSPARENT {
                ui.painter().rect_filled(rect, Radius::Sm.corner(), base_fill);
            }
            // Run body — body owns the inner geometry.
            if let Some(body) = self.painter_body { body(ui, rect); }

            let st = self.state
                .hovered(click.hovered())
                .pressed(click.is_pointer_button_down_on())
                .focused(click.has_focus());
            let v = apply_interaction(rect, st, self.theme.accent, &self.tokens);
            if v.fill != Color32::TRANSPARENT {
                ui.painter().rect_filled(rect, Radius::Sm.corner(), v.fill);
            }
            if v.stroke.width > 0.0 {
                ui.painter().rect_stroke(rect, Radius::Sm.corner(), v.stroke, StrokeKind::Inside);
            }
            if self.show_divider {
                let y = rect.bottom();
                ui.painter().line_segment(
                    [egui::pos2(rect.left(), y), egui::pos2(rect.right(), y)],
                    Stroke::new(stroke_hair(), border),
                );
            }
            if click.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
            // Suppress unused warnings.
            let _ = (pad, fg);
            return click;
        }

        let resp = egui::Frame::NONE
            .fill(base_fill)
            .inner_margin(pad)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    if let Some(leading) = self.leading { leading(ui); }
                    ui.label(TextStyle::Body.as_rich(self.primary, fg));
                    if let Some(sec) = self.secondary {
                        ui.label(TextStyle::BodySm.as_rich(sec, self.theme.dim));
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if let Some(trailing) = self.trailing { trailing(ui); }
                    });
                });
            }).response;

        let click = ui.interact(resp.rect, ui.id().with(("row_shell", resp.rect.min.x as i32, resp.rect.min.y as i32)), Sense::click());
        let st = self.state
            .hovered(click.hovered())
            .pressed(click.is_pointer_button_down_on())
            .focused(click.has_focus());
        let v = apply_interaction(resp.rect, st, self.theme.accent, &self.tokens);
        if v.fill != Color32::TRANSPARENT {
            ui.painter().rect_filled(resp.rect, Radius::Sm.corner(), v.fill);
        }
        if v.stroke.width > 0.0 {
            ui.painter().rect_stroke(resp.rect, Radius::Sm.corner(), v.stroke, StrokeKind::Inside);
        }
        if self.show_divider {
            let y = resp.rect.bottom();
            ui.painter().line_segment(
                [egui::pos2(resp.rect.left(), y), egui::pos2(resp.rect.right(), y)],
                Stroke::new(stroke_hair(), border),
            );
        }
        if click.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
        click
    }
}

// ─── CardShell ───────────────────────────────────────────────────────────────

#[must_use = "CardShell must be drawn with .show(ui)"]
pub struct CardShell<'a> {
    theme: Option<&'a Theme>,
    variant: CardVariant,
    size: Size,
    radius: Radius,
    title: Option<&'a str>,
    subtitle: Option<&'a str>,
    body: Option<Box<dyn FnOnce(&mut Ui) + 'a>>,
    footer: Option<Box<dyn FnOnce(&mut Ui) + 'a>>,
    title_style: Option<TextStyle>,
    padding: Option<Margin>,
}

impl<'a> CardShell<'a> {
    pub fn new(theme: &'a Theme) -> Self {
        Self {
            theme: Some(theme), variant: CardVariant::Bordered, size: Size::Md, radius: Radius::Md,
            title: None, subtitle: None, body: None, footer: None,
            title_style: None, padding: None,
        }
    }
    /// Theme-less constructor — falls back to neutral colors.
    pub fn new_themeless() -> Self {
        Self {
            theme: None, variant: CardVariant::Bordered, size: Size::Md, radius: Radius::Md,
            title: None, subtitle: None, body: None, footer: None,
            title_style: None, padding: None,
        }
    }
    pub fn variant(mut self, v: CardVariant) -> Self { self.variant = v; self }
    pub fn size(mut self, s: Size) -> Self { self.size = s; self }
    pub fn radius(mut self, r: Radius) -> Self { self.radius = r; self }
    pub fn title(mut self, t: &'a str) -> Self { self.title = Some(t); self }
    pub fn subtitle(mut self, s: &'a str) -> Self { self.subtitle = Some(s); self }
    pub fn body(mut self, f: impl FnOnce(&mut Ui) + 'a) -> Self {
        self.body = Some(Box::new(f)); self
    }
    pub fn footer(mut self, f: impl FnOnce(&mut Ui) + 'a) -> Self {
        self.footer = Some(Box::new(f)); self
    }
    /// Configure the text style of the title (defaults to HeadingMd).
    pub fn title_style(mut self, t: TextStyle) -> Self { self.title_style = Some(t); self }
    /// Override inner padding (defaults to Size's padding).
    pub fn padding(mut self, m: Margin) -> Self { self.padding = Some(m); self }
    /// Optional theme handle. Pass `None` to render with neutral fallbacks.
    pub fn theme(mut self, t: Option<&'a Theme>) -> Self { self.theme = t; self }

    pub fn show(self, ui: &mut Ui) -> Response {
        let pad = self.padding.unwrap_or_else(|| self.size.padding());
        // Resolve colors with optional theme fallback.
        let (fg, dim_color, fill, border) = if let Some(theme) = self.theme {
            (
                self.variant.fg_color(theme),
                theme.dim,
                self.variant.fill_color(theme),
                self.variant.border_color(theme),
            )
        } else {
            // Neutral fallbacks for theme-less rendering.
            let neutral_fg = Color32::from_gray(220);
            let neutral_dim = Color32::from_gray(150);
            let neutral_bg = match self.variant {
                CardVariant::Ghost => Color32::TRANSPARENT,
                _ => Color32::from_gray(28),
            };
            let neutral_border = match self.variant {
                CardVariant::Ghost => Color32::TRANSPARENT,
                _ => Color32::from_gray(60),
            };
            (neutral_fg, neutral_dim, neutral_bg, neutral_border)
        };

        let settings = current();
        let stroke_width = if settings.hairline_borders { stroke_thin() } else { 0.0 };
        let stroke = if stroke_width > 0.0 && border != Color32::TRANSPARENT {
            Stroke::new(stroke_width, border)
        } else {
            Stroke::NONE
        };

        let mut frame = egui::Frame::NONE
            .fill(fill)
            .inner_margin(pad)
            .stroke(stroke)
            .corner_radius(self.radius.corner());
        if matches!(self.variant, CardVariant::Elevated) && settings.shadows_enabled {
            frame = frame.shadow(egui::epaint::Shadow {
                offset: [0, shadow_offset() as i8],
                blur: shadow_spread() as u8 + 4,
                spread: 1,
                color: Color32::from_black_alpha(shadow_alpha()),
            });
        }
        let title_style = self.title_style.unwrap_or(TextStyle::HeadingMd);
        frame.show(ui, |ui| {
            if let Some(title) = self.title {
                ui.label(title_style.as_rich(title, fg));
            }
            if let Some(sub) = self.subtitle {
                ui.label(TextStyle::BodySm.as_rich(sub, dim_color));
            }
            if let Some(body) = self.body { body(ui); }
            if let Some(footer) = self.footer {
                ui.add_space(gap_md());
                footer(ui);
            }
        }).response
    }
}

// ─── InputShell ──────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputState { Default, Focused, Error, Disabled }

#[must_use = "InputShell must be drawn with .show(ui)"]
pub struct InputShell<'a> {
    theme: &'a Theme,
    variant: InputVariant,
    size: Size,
    radius: Radius,
    state: InputState,
    body: Option<Box<dyn FnOnce(&mut Ui) + 'a>>,
}

impl<'a> InputShell<'a> {
    pub fn new(theme: &'a Theme) -> Self {
        Self { theme, variant: InputVariant::Default, size: Size::Md, radius: Radius::Sm,
               state: InputState::Default, body: None }
    }
    pub fn variant(mut self, v: InputVariant) -> Self { self.variant = v; self }
    pub fn size(mut self, s: Size) -> Self { self.size = s; self }
    pub fn radius(mut self, r: Radius) -> Self { self.radius = r; self }
    pub fn state(mut self, s: InputState) -> Self { self.state = s; self }
    pub fn body(mut self, f: impl FnOnce(&mut Ui) + 'a) -> Self { self.body = Some(Box::new(f)); self }

    pub fn show(self, ui: &mut Ui) -> Response {
        let pad = self.size.padding();
        let border = match self.state {
            InputState::Default  => self.variant.border_color(self.theme),
            InputState::Focused  => color_alpha(self.theme.accent, alpha_strong()),
            InputState::Error    => color_alpha(self.theme.bear, alpha_strong()),
            InputState::Disabled => color_alpha(self.theme.toolbar_border, alpha_muted()),
        };
        let stroke_w = if matches!(self.state, InputState::Focused | InputState::Error) {
            stroke_bold()
        } else {
            stroke_thin()
        };
        let resp = egui::Frame::NONE
            .fill(self.variant.fill_color(self.theme))
            .inner_margin(pad)
            .stroke(Stroke::new(stroke_w, border))
            .corner_radius(self.radius.corner())
            .show(ui, |ui| {
                if let Some(body) = self.body { body(ui); }
            }).response;
        resp
    }
}

// ─── ChipShell ───────────────────────────────────────────────────────────────

#[must_use = "ChipShell must be drawn with .show(ui)"]
pub struct ChipShell<'a> {
    theme: &'a Theme,
    variant: ChipVariant,
    size: Size,
    label: &'a str,
    leading_icon: Option<&'a str>,
    closable: bool,
    state: InteractionState,
    tokens: InteractionTokens,
}

impl<'a> ChipShell<'a> {
    pub fn new(theme: &'a Theme, label: &'a str) -> Self {
        Self { theme, variant: ChipVariant::Subtle, size: Size::Sm, label,
               leading_icon: None, closable: false,
               state: InteractionState::default(), tokens: InteractionTokens::default() }
    }
    pub fn variant(mut self, v: ChipVariant) -> Self { self.variant = v; self }
    pub fn size(mut self, s: Size) -> Self { self.size = s; self }
    pub fn icon(mut self, icon: &'a str) -> Self { self.leading_icon = Some(icon); self }
    pub fn closable(mut self, v: bool) -> Self { self.closable = v; self }
    pub fn state(mut self, s: InteractionState) -> Self { self.state = s; self }

    pub fn show(self, ui: &mut Ui) -> Response {
        let fg = self.variant.fg_color(self.theme);
        let fill = self.variant.fill_color(self.theme);
        let border = self.variant.border_color(self.theme);
        let pad = self.size.padding();

        let resp = egui::Frame::NONE
            .fill(fill)
            .inner_margin(pad)
            .stroke(Stroke::new(stroke_thin(), border))
            .corner_radius(Radius::Pill.corner())
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    if let Some(icon) = self.leading_icon {
                        ui.label(RichText::new(icon).size(self.size.font()).color(fg));
                    }
                    ui.label(TextStyle::BodySm.as_rich(self.label, fg));
                    if self.closable {
                        ui.label(RichText::new("×").size(self.size.font()).color(fg));
                    }
                });
            }).response;

        let st = self.state.hovered(resp.hovered());
        let v = apply_interaction(resp.rect, st, self.theme.accent, &self.tokens);
        if v.fill != Color32::TRANSPARENT {
            ui.painter().rect_filled(resp.rect, Radius::Pill.corner(), v.fill);
        }
        if resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
        resp
    }
}
