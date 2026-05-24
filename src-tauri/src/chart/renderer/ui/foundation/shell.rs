//! Foundation shells — the base composables every concrete widget will
//! eventually wrap. Each shell owns spacing/colors/hover/border/radius for one
//! family of UI element so that families on top stay declarative.
//!
//! Wave 4.5b will migrate existing widgets onto these shells.

#![allow(dead_code, unused_imports)]

use egui::{Color32, Margin, Rect, Response, RichText, Sense, Stroke, StrokeKind, Ui, Vec2};

use super::super::style::*;
use super::interaction::{apply_interaction, InteractionState, InteractionTokens};
use super::text_style::TextStyle;
use crate::ui_kit::widgets::tokens::Size;

// ─── Radius (inlined from former tokens.rs P3.1) ─────────────────────────────
//
// Pill reads `StyleSettings.r_pill` which varies per style preset (e.g.
// Meridien r_pill = 0); the ui_kit equivalent `radius_pill()` is a fixed
// 999.0 constant with no preset awareness. Unifying requires the style-axis
// decision deferred to Phase 5.

/// Radius scale for foundation shells. `Pill` reads the per-preset `r_pill` knob.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Radius { None, Xs, Sm, Md, Lg, Pill }

impl Radius {
    pub fn corner(self) -> egui::CornerRadius {
        let st = current();
        match self {
            Radius::None => egui::CornerRadius::ZERO,
            Radius::Xs   => egui::CornerRadius::same(st.r_xs),
            Radius::Sm   => egui::CornerRadius::same(st.r_sm),
            Radius::Md   => egui::CornerRadius::same(st.r_md),
            Radius::Lg   => egui::CornerRadius::same(st.r_lg),
            Radius::Pill => egui::CornerRadius::same(st.r_pill),
        }
    }
}
use crate::ui_kit::widgets::{CardVariant, RowVariant};

type Theme = super::super::super::gpu::Theme;

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
    theme: &'a Theme,
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
            theme, variant: CardVariant::Bordered, size: Size::Md, radius: Radius::Md,
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

    pub fn show(self, ui: &mut Ui) -> Response {
        let pad = self.padding.unwrap_or_else(|| self.size.padding());
        let theme = self.theme;
        let fg        = self.variant.fg_color(theme);
        let dim_color = theme.dim;
        let fill      = self.variant.fill_color(theme);
        let border    = self.variant.border_color(theme);

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
                color: shadow_color_alpha(theme, shadow_alpha()),
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
