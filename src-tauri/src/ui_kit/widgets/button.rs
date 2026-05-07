//! Unified button — replaces IconBtn/TradeBtn/ChromeBtn/SimpleBtn/ToolbarBtn/
//! ActionButton/PillButton/BrandCtaButton and their free-fn cousins.
//!
//! Variants:
//!   Primary   — accent fill, white text (was: BrandCtaButton, big_action_btn)
//!   Secondary — surface fill + border (was: ToolbarBtn active, ActionButton)
//!   Ghost     — transparent until hover (was: IconBtn idle, SimpleBtn)
//!   Danger    — bear fill (was: TradeBtn sell, side_pane_action_btn warning)
//!   Link      — text-only, underline on hover
//!
//! Buy/sell semantics: use Variant::Primary with a custom tint via
//! `.tint(theme.bull())` or `.tint(theme.bear())`. Or use the convenience
//! `Button::buy(label)` / `Button::sell(label)` constructors.
//!
//! All sizes match Size enum. icon_only(true) collapses padding to be
//! square. loading(true) shows a spinner inline. disabled(true) gates
//! interaction + dims at 50%.

use egui::{Color32, CornerRadius, FontId, Pos2, Rect, Response, RichText, Sense, Stroke, StrokeKind, Ui, Vec2, Widget};

use super::motion;
use super::theme::ComponentTheme;
use super::tokens::{Size, Variant};
use crate::chart::renderer::ui::style as st;

/// Unified button builder. Use [`Button::new`] for a labelled button or
/// [`Button::icon`] for an icon-only one. Compose via the chainable
/// methods, then call [`Button::show`] (preferred — explicit theme) or
/// add through [`egui::Widget`] which falls back to the default theme.
#[must_use = "Button does nothing until `.show(ui, theme)` or `ui.add(button)` is called"]
pub struct Button<'a> {
    label: &'a str,
    leading_icon: Option<&'a str>,
    trailing_icon: Option<&'a str>,
    variant: Variant,
    size: Size,
    icon_only: bool,
    loading: bool,
    disabled: bool,
    active: bool,
    full_width: bool,
    tint: Option<Color32>,
    corner_radius: Option<f32>,
    _marker_tint_bull: bool,
    _marker_tint_bear: bool,
}

impl<'a> Button<'a> {
    pub fn new(label: impl Into<&'a str>) -> Self {
        Self {
            label: label.into(),
            leading_icon: None,
            trailing_icon: None,
            variant: Variant::Primary,
            size: Size::Md,
            icon_only: false,
            loading: false,
            disabled: false,
            active: false,
            full_width: false,
            tint: None,
            corner_radius: None,
            _marker_tint_bull: false,
            _marker_tint_bear: false,
        }
    }

    /// Icon-only convenience: square button, no label. Pass a Phosphor
    /// glyph from `crate::ui_kit::icons::Icon::*`.
    pub fn icon(icon: &'a str) -> Self {
        let mut b = Self::new("");
        b.leading_icon = Some(icon);
        b.icon_only = true;
        b.variant = Variant::Ghost;
        b
    }

    /// Primary button tinted with the bull color — semantic "buy".
    pub fn buy(label: impl Into<&'a str>) -> Self {
        let mut b = Self::new(label);
        b.variant = Variant::Primary;
        b._marker_tint_bull = true;
        b
    }

    /// Primary button tinted with the bear color — semantic "sell".
    pub fn sell(label: impl Into<&'a str>) -> Self {
        let mut b = Self::new(label);
        b.variant = Variant::Primary;
        b._marker_tint_bear = true;
        b
    }

    pub fn variant(mut self, v: Variant) -> Self { self.variant = v; self }
    pub fn size(mut self, s: Size) -> Self { self.size = s; self }
    pub fn icon_only(mut self, v: bool) -> Self { self.icon_only = v; self }
    pub fn loading(mut self, v: bool) -> Self { self.loading = v; self }
    pub fn disabled(mut self, v: bool) -> Self { self.disabled = v; self }
    pub fn active(mut self, v: bool) -> Self { self.active = v; self }
    pub fn full_width(mut self, v: bool) -> Self { self.full_width = v; self }
    pub fn tint(mut self, c: Color32) -> Self { self.tint = Some(c); self }
    pub fn leading_icon(mut self, icon: &'a str) -> Self { self.leading_icon = Some(icon); self }
    pub fn trailing_icon(mut self, icon: &'a str) -> Self { self.trailing_icon = Some(icon); self }
    pub fn corner_radius(mut self, r: f32) -> Self { self.corner_radius = Some(r); self }

    pub fn show(self, ui: &mut Ui, theme: &dyn ComponentTheme) -> Response {
        paint_button(ui, theme, self)
    }
}

impl<'a> Button<'a> {
    #[doc(hidden)]
    fn resolve_tint(&self, theme: &dyn ComponentTheme) -> Option<Color32> {
        if self._marker_tint_bull { return Some(theme.bull()); }
        if self._marker_tint_bear { return Some(theme.bear()); }
        self.tint
    }
}

// Default impl on Widget — uses fallback theme.
impl<'a> Widget for Button<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        // Fallback: first registered chart theme. This keeps non-themed
        // call sites compiling; prefer `.show(ui, theme)` when you have
        // an explicit theme handle.
        let theme = &crate::chart_renderer::gpu::THEMES[0];
        self.show(ui, theme)
    }
}

// ── Internal painting ──────────────────────────────────────────────────

fn paint_button<'a>(ui: &mut Ui, theme: &dyn ComponentTheme, btn: Button<'a>) -> Response {
    let Button {
        label,
        leading_icon,
        trailing_icon,
        variant,
        size,
        icon_only,
        loading,
        disabled,
        active,
        full_width,
        corner_radius,
        ..
    } = btn;
    let tint = btn.resolve_tint(theme);

    let h = size.height();
    let pad_x = size.padding_x();
    let font_size = size.font_size();
    let icon_gap = st::gap_2xs();

    // ── Measure intrinsic width ──
    let mut content_w = 0.0f32;
    if icon_only {
        content_w = font_size * 1.25;
    } else {
        if leading_icon.is_some() || loading {
            content_w += font_size * 1.1 + icon_gap;
        }
        if !label.is_empty() {
            let galley = ui.fonts(|f| {
                f.layout_no_wrap(label.to_string(), FontId::monospace(font_size), Color32::WHITE)
            });
            content_w += galley.rect.width();
        }
        if trailing_icon.is_some() {
            content_w += icon_gap + font_size * 1.1;
        }
    }

    let intrinsic_w = if icon_only { h } else { content_w + 2.0 * pad_x };
    let desired_w = if full_width { ui.available_width().max(intrinsic_w) } else { intrinsic_w };
    let desired = Vec2::new(desired_w, h);

    let sense = if disabled || loading { Sense::hover() } else { Sense::click() };
    let (rect, response) = ui.allocate_exact_size(desired, sense);

    if ui.is_rect_visible(rect) {
        let id = response.id;
        let hovered = response.hovered() && !disabled && !loading;
        let pressed = response.is_pointer_button_down_on() && !disabled && !loading;

        let hover_t = motion::ease_bool(ui.ctx(), id.with("btn_hover"), hovered, motion::FAST);
        let active_t = motion::ease_bool(ui.ctx(), id.with("btn_active"), active, motion::MED);

        // Resolve colors per variant.
        let (idle_bg, hover_bg, active_bg, fg_idle, fg_hover, border_idle, border_active) =
            resolve_palette(theme, variant, tint);

        // Compose backgrounds: idle -> hover -> active.
        let mut bg = motion::lerp_color(idle_bg, hover_bg, hover_t);
        bg = motion::lerp_color(bg, active_bg, active_t);

        // Press snap (instant darken, no animation).
        if pressed {
            bg = darken(bg, 0.12);
        }

        let mut fg = motion::lerp_color(fg_idle, fg_hover, hover_t);
        let border_col = motion::lerp_color(border_idle, border_active, active_t);

        // Disabled: 50% opacity on everything.
        if disabled {
            bg = with_alpha_scale(bg, 0.5);
            fg = with_alpha_scale(fg, 0.5);
        }

        let radius = corner_radius.unwrap_or(default_radius(variant));
        let cr = CornerRadius::same(radius as u8);

        let painter = ui.painter_at(rect);

        // Background.
        if bg.a() > 0 {
            painter.rect_filled(rect, cr, bg);
        }
        // Border (Secondary or active state).
        let border_w = match variant { Variant::Secondary => 1.0, _ => 0.0 };
        if border_col.a() > 0 && (border_w > 0.0 || active_t > 0.001) {
            let w = if border_w > 0.0 { border_w } else { 1.0 };
            painter.rect_stroke(rect, cr, Stroke::new(w, border_col), StrokeKind::Inside);
        }

        // Link underline — fade in on hover only.
        if matches!(variant, Variant::Link) && hover_t > 0.001 {
            let underline = Color32::from_rgba_premultiplied(
                fg.r(), fg.g(), fg.b(),
                ((fg.a() as f32) * hover_t).round() as u8,
            );
            let y = rect.bottom() - 2.0;
            let x0 = rect.left() + pad_x * 0.5;
            let x1 = rect.right() - pad_x * 0.5;
            painter.line_segment([Pos2::new(x0, y), Pos2::new(x1, y)], Stroke::new(1.0, underline));
        }

        // ── Layout content (icon | label | trailing) ──
        let center = rect.center();
        if icon_only {
            // Loading replaces icon.
            if loading {
                paint_spinner(ui, rect, fg);
            } else if let Some(ic) = leading_icon {
                painter.text(
                    center,
                    egui::Align2::CENTER_CENTER,
                    ic,
                    FontId::proportional(font_size * 1.25),
                    fg,
                );
            }
        } else {
            // Compute layout.
            let mut x = rect.left() + pad_x;
            let cy = center.y;
            // Leading: spinner takes priority over leading icon when loading.
            if loading {
                let spin_rect = Rect::from_center_size(Pos2::new(x + font_size * 0.55, cy), Vec2::splat(font_size * 1.1));
                paint_spinner(ui, spin_rect, fg);
                x += font_size * 1.1 + icon_gap;
            } else if let Some(ic) = leading_icon {
                painter.text(
                    Pos2::new(x, cy),
                    egui::Align2::LEFT_CENTER,
                    ic,
                    FontId::proportional(font_size * 1.1),
                    fg,
                );
                x += font_size * 1.1 + icon_gap;
            }
            if !label.is_empty() {
                painter.text(
                    Pos2::new(x, cy),
                    egui::Align2::LEFT_CENTER,
                    label,
                    FontId::monospace(font_size),
                    fg,
                );
            }
            if let Some(ic) = trailing_icon {
                let tx = rect.right() - pad_x;
                painter.text(
                    Pos2::new(tx, cy),
                    egui::Align2::RIGHT_CENTER,
                    ic,
                    FontId::proportional(font_size * 1.1),
                    fg,
                );
            }
        }

        // Cursor.
        if hovered {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
    }

    response
}

fn default_radius(v: Variant) -> f32 {
    match v {
        Variant::Primary | Variant::Secondary | Variant::Danger => 4.0,
        Variant::Ghost => 2.0,
        Variant::Link => 0.0,
    }
}

fn resolve_palette(
    theme: &dyn ComponentTheme,
    variant: Variant,
    tint: Option<Color32>,
) -> (Color32, Color32, Color32, Color32, Color32, Color32, Color32) {
    // Returns (idle_bg, hover_bg, active_bg, fg_idle, fg_hover, border_idle, border_active)
    let accent = tint.unwrap_or_else(|| theme.accent());
    let bear = tint.unwrap_or_else(|| theme.bear());
    let surface = theme.surface();
    let text = theme.text();
    let border = theme.border();
    let white = Color32::WHITE;
    let transparent = Color32::TRANSPARENT;

    match variant {
        Variant::Primary => (
            accent,
            lighten(accent, 0.10),
            darken(accent, 0.08),
            white,
            white,
            transparent,
            st::color_alpha(accent, st::alpha_active()),
        ),
        Variant::Secondary => (
            surface,
            lighten(surface, 0.08),
            st::color_alpha(accent, st::alpha_tint()),
            text,
            text,
            border,
            st::color_alpha(accent, st::alpha_active()),
        ),
        Variant::Ghost => (
            transparent,
            st::color_alpha(text, 18),
            st::color_alpha(accent, st::alpha_tint()),
            text,
            text,
            transparent,
            st::color_alpha(accent, st::alpha_muted()),
        ),
        Variant::Danger => (
            bear,
            lighten(bear, 0.10),
            darken(bear, 0.08),
            white,
            white,
            transparent,
            st::color_alpha(bear, st::alpha_active()),
        ),
        Variant::Link => (
            transparent,
            transparent,
            transparent,
            theme.accent(),
            lighten(theme.accent(), 0.15),
            transparent,
            transparent,
        ),
    }
}

#[inline]
fn darken(c: Color32, amt: f32) -> Color32 {
    let f = (1.0 - amt).clamp(0.0, 1.0);
    Color32::from_rgba_premultiplied(
        ((c.r() as f32) * f) as u8,
        ((c.g() as f32) * f) as u8,
        ((c.b() as f32) * f) as u8,
        c.a(),
    )
}

#[inline]
fn lighten(c: Color32, amt: f32) -> Color32 {
    let lerp = |x: u8| -> u8 {
        let v = x as f32 + (255.0 - x as f32) * amt.clamp(0.0, 1.0);
        v.round().clamp(0.0, 255.0) as u8
    };
    Color32::from_rgba_premultiplied(lerp(c.r()), lerp(c.g()), lerp(c.b()), c.a())
}

#[inline]
fn with_alpha_scale(c: Color32, s: f32) -> Color32 {
    Color32::from_rgba_premultiplied(
        c.r(), c.g(), c.b(),
        ((c.a() as f32) * s.clamp(0.0, 1.0)).round() as u8,
    )
}

fn paint_spinner(ui: &Ui, rect: Rect, color: Color32) {
    // Simple animated rotating arc using ease_value to accumulate angle.
    let id = egui::Id::new(("ui_kit_btn_spinner", rect.center().x as i32, rect.center().y as i32));
    // Drive a value that increments over time; egui repaints continuously
    // while animations are in flight, so we request a repaint.
    let now = ui.input(|i| i.time) as f32;
    let angle = motion::ease_value(ui.ctx(), id, now * 4.0, 0.0); // 4 rad/s
    ui.ctx().request_repaint();

    let center = rect.center();
    let radius = rect.size().min_elem() * 0.4;
    let segments = 12;
    let arc_len = 7; // 7/12 of the circle
    let painter = ui.painter_at(rect);
    for i in 0..arc_len {
        let t = i as f32 / arc_len as f32;
        let a = angle + t * std::f32::consts::TAU * (arc_len as f32 / segments as f32);
        let p0 = center + Vec2::new(a.cos(), a.sin()) * (radius * 0.6);
        let p1 = center + Vec2::new(a.cos(), a.sin()) * radius;
        let alpha = (60.0 + 195.0 * t).round().clamp(0.0, 255.0) as u8;
        let c = Color32::from_rgba_premultiplied(color.r(), color.g(), color.b(), alpha.min(color.a()));
        painter.line_segment([p0, p1], Stroke::new(1.5, c));
    }
}

// ── Visual smoke test gallery ──────────────────────────────────────────

/// Paints all 5 variants × 4 sizes plus icon_only / loading / disabled /
/// full_width / buy / sell. Drop into any `Ui` for visual QA.
///
/// Not wired into a panel — intended as a callable helper for ad-hoc
/// smoke-test windows during development.
pub fn show_button_gallery(ui: &mut Ui, theme: &dyn ComponentTheme) {
    use crate::ui_kit::icons::Icon;
    ui.heading("Button Gallery");
    ui.separator();

    let variants = [
        ("Primary", Variant::Primary),
        ("Secondary", Variant::Secondary),
        ("Ghost", Variant::Ghost),
        ("Danger", Variant::Danger),
        ("Link", Variant::Link),
    ];
    let sizes = [("Xs", Size::Xs), ("Sm", Size::Sm), ("Md", Size::Md), ("Lg", Size::Lg)];

    for (vname, v) in variants {
        ui.label(vname);
        ui.horizontal(|ui| {
            for (sname, s) in sizes {
                let _ = Button::new(sname).variant(v).size(s).show(ui, theme);
            }
        });
        ui.add_space(4.0);
    }

    ui.separator();
    ui.label("Modifiers");
    ui.horizontal(|ui| {
        let _ = Button::icon(Icon::GEAR).size(Size::Md).show(ui, theme);
        let _ = Button::new("Loading").loading(true).show(ui, theme);
        let _ = Button::new("Disabled").disabled(true).show(ui, theme);
        let _ = Button::new("Active").active(true).variant(Variant::Secondary).show(ui, theme);
    });
    ui.add_space(4.0);

    ui.label("Buy / Sell");
    ui.horizontal(|ui| {
        let _ = Button::buy("BUY").size(Size::Lg).show(ui, theme);
        let _ = Button::sell("SELL").size(Size::Lg).show(ui, theme);
    });
    ui.add_space(4.0);

    ui.label("Full width");
    let _ = Button::new("Submit Order").full_width(true).size(Size::Lg).show(ui, theme);

    ui.add_space(4.0);
    ui.label("With icons");
    ui.horizontal(|ui| {
        let _ = Button::new("Save").leading_icon(Icon::CHECK).variant(Variant::Primary).show(ui, theme);
        let _ = Button::new("Next").trailing_icon(Icon::CARET_RIGHT).variant(Variant::Secondary).show(ui, theme);
    });
}
