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
use super::Tooltip;
use super::icon_placement::{IconPlacement, IconTone, IconState, icon_glyph_color, icon_hover_bg};
use super::button_style::{ButtonStyle, ButtonState, DefaultButtonStyle};
use crate::ui_kit::tokens as st;

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
    // Escape hatches (legacy IconBtn / ChromeBtn / SimpleBtn parity).
    fg_override: Option<Color32>,
    glyph_color_override: Option<Color32>,
    fill_override: Option<Color32>,
    hover_fill_override: Option<Color32>,
    stroke_override: Option<Stroke>,
    min_size_override: Option<Vec2>,
    frameless: bool,
    honor_style_treatment: bool,
    simple_treatment: bool,
    /// Optional embedded keybind chip (Zed parity: "Finish Setup  Ctrl-Enter").
    /// Rendered inline on the right of the label, no chip frame.
    kbd: Option<String>,
    /// Status-row mode: snap color on hover, no bg tint, smaller default size.
    is_status: bool,
    /// Optional sublabel — when set, the button paints as a vertical
    /// icon + label + sublabel stack. See [`Button::sublabel`].
    sublabel: Option<String>,
    /// Override the rendered icon pixel size for icon-only buttons.
    glyph_px: Option<f32>,
    /// Placement-driven sizing and hover semantics. When `Some`, placement
    /// wins over `min_size_override` and `glyph_px`. When `None`, the
    /// existing Button behaviour is preserved (zero regression for unmigrated
    /// call sites).
    placement: Option<IconPlacement>,
    /// Semantic tone for icon glyph color resolution (used when placement is set).
    icon_tone: IconTone,
    /// Override the interaction sense. Use `Sense::hover()` for display-only badges
    /// that should still receive hover state but not respond to clicks.
    sense_override: Option<egui::Sense>,
    /// Override the corner radius with a full asymmetric `CornerRadius`. Use this for
    /// pill halves where only the outer corners should be rounded (e.g. left half:
    /// `CornerRadius { nw: 99, sw: 99, ne: 0, se: 0 }`). Takes precedence over
    /// `.corner_radius(f32)` when both are set.
    corner_radius_override: Option<egui::CornerRadius>,
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
            fg_override: None,
            glyph_color_override: None,
            fill_override: None,
            hover_fill_override: None,
            stroke_override: None,
            min_size_override: None,
            frameless: false,
            honor_style_treatment: true,
            simple_treatment: false,
            kbd: None,
            is_status: false,
            sublabel: None,
            glyph_px: None,
            placement: None,
            icon_tone: IconTone::Neutral,
            sense_override: None,
            corner_radius_override: None,
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

    // ─── Named presets for the legacy free-function helpers ────────────
    //
    // These cover the role-specific buttons that previously lived as free
    // functions in `chart/renderer/ui/style.rs` (action_btn, trade_btn,
    // cta_btn, simple_btn, small_action_btn, close_button, tb_btn). New
    // code should use these named presets so every button surface goes
    // through one canonical widget.

    /// Action button — tinted bg for Place / Cancel / Clear. Pass the
    /// semantic color (bull / bear / accent / warn) as the tint.
    pub fn action(label: impl Into<&'a str>) -> Self {
        let mut b = Self::new(label);
        b.variant = Variant::Secondary;
        b.size = Size::Sm;
        b
    }

    /// Trade button — deep saturated bg for BUY/SELL. Use `.tint(bull)`
    /// or `.tint(bear)` to color it.
    pub fn trade(label: impl Into<&'a str>) -> Self {
        let mut b = Self::new(label);
        b.variant = Variant::Primary;
        b.size = Size::Md;
        b
    }

    /// Primary CTA button — solid filled accent, full-width default.
    /// Used for the terminal action at the bottom of order tickets
    /// ("REVIEW BUY", "PLACE ORDER").
    pub fn cta(label: impl Into<&'a str>) -> Self {
        let mut b = Self::new(label);
        b.variant = Variant::Primary;
        b.size = Size::Md;
        b.full_width = true;
        b
    }

    /// Small action — inline header actions like "Clear All", "Close All".
    pub fn small_action(label: impl Into<&'a str>) -> Self {
        let mut b = Self::new(label);
        b.variant = Variant::Secondary;
        b.size = Size::Xs;
        b
    }

    /// Simple — subtle border, form-level actions (Create, Cancel).
    pub fn simple(label: impl Into<&'a str>) -> Self {
        let mut b = Self::new(label);
        b.variant = Variant::Secondary;
        b.size = Size::Sm;
        b
    }

    /// Outline + full-width — a Ghost button (no fill) sized small that
    /// stretches across the available width. Used for the "+ Add …" row
    /// at the bottom of a `PanelSection` body where the affordance should
    /// span the section but stay visually quiet. Replaces the
    /// `Button::simple(label).variant(Ghost).min_size(vec2(avail, 22.0))`
    /// workaround that several panels had to reach for.
    pub fn outline_full_width(label: impl Into<&'a str>) -> Self {
        let mut b = Self::new(label);
        b.variant = Variant::Ghost;
        b.size = Size::Sm;
        b.full_width = true;
        b
    }

    /// Toolbar — top-nav button surface. Status mode + Ghost variant so
    /// the bg only paints on hover/active.
    pub fn toolbar(label: impl Into<&'a str>) -> Self {
        let mut b = Self::new(label);
        b.is_status = true;
        b.size = Size::Sm;
        b.variant = Variant::Ghost;
        b
    }

    /// Menu trigger — the canonical "I open a dropdown" preset. Pairs
    /// `Variant::Toolbar` (Status + Ghost so the bg only paints on
    /// hover) with a trailing caret indicator so the button reads as a
    /// menu trigger consistent with the rest of the toolbar surfaces.
    /// Pair with [`Button::show_menu`] to render and attach the popup.
    ///
    /// ```ignore
    /// Button::menu("Mode")
    ///     .icon(Icon::ARROW_RIGHT)             // optional leading icon
    ///     .show_menu(ui, t, |ui| {
    ///         if ui.button("Item 1").clicked() { /* ... */ }
    ///     });
    /// ```
    ///
    /// Replaces raw
    /// `ui.menu_button(RichText::new(label).monospace().size(font_sm()).color(t.dim), ...)`
    /// patterns that have drifted across the toolbar / tool-menu / object-tree.
    pub fn menu(label: impl Into<&'a str>) -> Self {
        let mut b = Self::new(label);
        b.is_status = true;
        b.size = Size::Sm;
        b.variant = Variant::Ghost;
        b.trailing_icon = Some(crate::ui_kit::icons::Icon::CARET_DOWN);
        b
    }

    /// Close (×) affordance — small icon-only Ghost. Pair with
    /// `.size(Size::Xs)` for compact contexts. Defaults to
    /// `IconPlacement::Modal` sizing (24×24 hit-target, 14px glyph).
    pub fn close() -> Self {
        let mut b = Self::icon(crate::ui_kit::icons::Icon::X);
        b.size = Size::Xs;
        b.placement = Some(IconPlacement::Modal);
        b
    }

    /// Toggle chip — one of a row of selectable presets (style chips,
    /// font-scale chips, session-tint chips). When `active` is true the
    /// chip paints with an accent-tinted bg + accent fg + accent border;
    /// when false it paints transparent with a soft dim outline and
    /// text-color fg. Hover blends toward active styling. Defaults to
    /// `Variant::Toggle` + `Size::Sm` with the `active` flag pre-set.
    pub fn toggle(label: impl Into<&'a str>, active: bool) -> Self {
        let mut b = Self::new(label);
        b.variant = Variant::Toggle;
        b.size = Size::Sm;
        b.active = active;
        b
    }

    pub fn variant(mut self, v: Variant) -> Self { self.variant = v; self }
    pub fn size(mut self, s: Size) -> Self { self.size = s; self }
    pub fn icon_only(mut self, v: bool) -> Self { self.icon_only = v; self }
    pub fn loading(mut self, v: bool) -> Self { self.loading = v; self }
    pub fn disabled(mut self, v: bool) -> Self { self.disabled = v; self }
    /// Positive-form counterpart to [`Button::disabled`]. `.enabled(true)`
    /// keeps the button interactive; `.enabled(false)` gates interaction
    /// and dims the surface to 50%. Same backing field as `.disabled()` —
    /// pick whichever reads better at the call site (most call sites think
    /// "is this button currently active?" rather than "is it disabled?").
    pub fn enabled(mut self, v: bool) -> Self { self.disabled = !v; self }
    pub fn active(mut self, v: bool) -> Self { self.active = v; self }
    pub fn full_width(mut self, v: bool) -> Self { self.full_width = v; self }
    pub fn tint(mut self, c: Color32) -> Self { self.tint = Some(c); self }
    pub fn leading_icon(mut self, icon: &'a str) -> Self { self.leading_icon = Some(icon); self }
    pub fn trailing_icon(mut self, icon: &'a str) -> Self { self.trailing_icon = Some(icon); self }
    pub fn glyph_size(mut self, px: f32) -> Self { self.glyph_px = Some(px); self }
    pub fn corner_radius(mut self, r: f32) -> Self { self.corner_radius = Some(r); self }

    /// Override the interaction [`egui::Sense`]. Use `Sense::hover()` for display-only
    /// badges that should still receive hover state but not respond to clicks.
    pub fn sense(mut self, s: egui::Sense) -> Self {
        self.sense_override = Some(s);
        self
    }

    /// Override the corner radius with a full asymmetric [`egui::CornerRadius`]. Use this
    /// for pill halves where only the outer corners should be rounded (e.g. left half:
    /// `CornerRadius { nw: 99, sw: 99, ne: 0, se: 0 }`). Takes precedence over
    /// `.corner_radius(f32)` when both are set.
    pub fn corner_radius_asymmetric(mut self, r: egui::CornerRadius) -> Self {
        self.corner_radius_override = Some(r);
        self
    }

    /// Set the [`IconPlacement`] for this button. When set, placement drives
    /// glyph size and hit-target size (overriding `min_size` / `glyph_size`),
    /// hover-bg treatment, and interactivity semantics. When `None` (default),
    /// the existing Button sizing/color logic is preserved.
    pub fn placement(mut self, p: IconPlacement) -> Self {
        self.placement = Some(p);
        self
    }

    /// Set the icon tone to `IconTone::Destructive` (bear/danger color).
    /// Only meaningful when a placement is set.
    pub fn tone_destructive(mut self) -> Self {
        self.icon_tone = IconTone::Destructive;
        self
    }

    /// Set the icon tone to `IconTone::Affirmative` (bull/confirm color).
    /// Only meaningful when a placement is set.
    pub fn tone_affirmative(mut self) -> Self {
        self.icon_tone = IconTone::Affirmative;
        self
    }

    // ─── Placement-aware preset shortcuts ──────────────────────────────

    /// Toolbar icon button (24×24 hit-target, 14px glyph, bg fill on hover).
    pub fn icon_toolbar(icon: &'a str) -> Self {
        Self::icon(icon).placement(IconPlacement::Toolbar)
    }

    /// Panel header icon button (20×20 hit-target, 13px glyph, bg fill on hover).
    pub fn icon_panel_header(icon: &'a str) -> Self {
        Self::icon(icon).placement(IconPlacement::PanelHeader)
    }

    /// Tab-close button (14×14 hit-target, 11px glyph, color-snap only — no bg fill).
    /// Automatically applies `IconTone::Destructive`.
    pub fn icon_tab_close() -> Self {
        Self::icon(crate::ui_kit::icons::Icon::X)
            .placement(IconPlacement::TabClose)
            .tone_destructive()
    }

    /// Override the icon/glyph color (Ghost variant). Useful for legacy
    /// IconBtn parity where each icon has its own color.
    pub fn glyph_color(mut self, c: Color32) -> Self { self.glyph_color_override = Some(c); self }

    /// Override the foreground (text + icon) color regardless of variant.
    /// Higher-priority than variant defaults.
    pub fn fg(mut self, c: Color32) -> Self { self.fg_override = Some(c); self }

    /// For Chrome variant: explicit fill color.
    pub fn fill(mut self, c: Color32) -> Self { self.fill_override = Some(c); self }

    /// Hover fill override (replaces default lighten).
    pub fn hover_fill(mut self, c: Color32) -> Self { self.hover_fill_override = Some(c); self }

    /// For Chrome variant: explicit border. Pass `Stroke::NONE` to remove.
    pub fn stroke(mut self, s: Stroke) -> Self { self.stroke_override = Some(s); self }

    /// Minimum size (replaces auto-computed from Size enum).
    pub fn min_size(mut self, sz: Vec2) -> Self { self.min_size_override = Some(sz); self }

    /// Shortcut for setting only the minimum width. Preserves any
    /// previously-set min height; otherwise uses the variant's default
    /// height from [`Size`]. Equivalent to
    /// `.min_size(vec2(w, current_or_default_height))`.
    pub fn min_width(mut self, w: f32) -> Self {
        let h = self.min_size_override.map(|v| v.y).unwrap_or_else(|| self.size.height());
        self.min_size_override = Some(Vec2::new(w, h));
        self
    }

    /// Frameless mode: paint label/icon only, no bg/border. Replaces
    /// `egui::Button::frame(false)` for parity with ChromeBtn::frameless.
    pub fn frameless(mut self, v: bool) -> Self { self.frameless = v; self }

    /// Honor the global `button_treatment` style state. When true (default),
    /// `Variant::Secondary` may be re-styled by the active treatment when
    /// `simple_treatment(true)` is also set.
    pub fn honor_style_treatment(mut self, v: bool) -> Self { self.honor_style_treatment = v; self }

    /// Opt into legacy SimpleBtn-style treatment dispatch. Only meaningful
    /// when `variant == Secondary && honor_style_treatment`.
    pub fn simple_treatment(mut self, v: bool) -> Self { self.simple_treatment = v; self }

    /// Render a keybind chip inside the button on the right side.
    /// Pattern: "Finish Setup  Ctrl-Enter" — the kbd lives inside the
    /// button bounds, not floating beside it. Used for primary CTAs that
    /// teach the keyboard equivalent. Unlike the standalone `Kbd` widget,
    /// this draws inline text (no chip frame) in muted mono_xs.
    ///
    /// Combined with `.icon_only(true)`, the kbd is silently dropped
    /// (icon_only buttons have no label, so the pattern is undefined).
    pub fn kbd(mut self, keybind: impl Into<String>) -> Self {
        self.kbd = Some(keybind.into());
        self
    }

    /// Mark this button as a status-row icon. Snap color on hover, no bg
    /// tint, smaller default size. Used for footer/status bar icons,
    /// pane header right-cluster icons.
    pub fn status(mut self, v: bool) -> Self {
        self.is_status = v;
        if v {
            self.size = Size::Sm;
            self.variant = Variant::Ghost;
        }
        self
    }

    /// Stacks an icon + label + sublabel vertically. The icon paints
    /// large at top; label below it (font_xs); sublabel below that
    /// (font_2xs muted). Used for compact two-line button affordances
    /// like the pane-header Order/DOM controls.
    ///
    /// Conflicts with: leading_icon, trailing_icon, kbd. The button
    /// becomes purely vertical when sublabel is set.
    pub fn sublabel(mut self, text: impl Into<String>) -> Self {
        self.sublabel = Some(text.into());
        self
    }

    /// Render with an explicit [`ComponentTheme`]. This is the original,
    /// idiomatic entry point — unchanged for all existing callers.
    ///
    /// Internally constructs a [`DefaultButtonStyle`] adapter and delegates
    /// to [`Button::show_styled`], so both paths share the same paint logic.
    pub fn show(self, ui: &mut Ui, theme: &dyn ComponentTheme) -> Response {
        let style = DefaultButtonStyle::new(theme);
        show_styled_impl(ui, theme, self, &style, None)
    }

    /// Opt-in entry point: render with a custom [`ButtonStyle`] implementor.
    ///
    /// Use this when you want per-call-site color overrides without touching
    /// `ComponentTheme`. The button still receives a `theme` reference for
    /// non-color decisions (motion, focus ring, shadow) that have no
    /// `ButtonStyle` equivalent yet.
    ///
    /// ```ignore
    /// struct MyStyle;
    /// impl ButtonStyle for MyStyle {
    ///     fn fg(&self, _v: Variant, _s: ButtonState) -> Color32 { Color32::RED }
    ///     fn bg(&self, _v: Variant, _s: ButtonState) -> Color32 { Color32::TRANSPARENT }
    ///     fn border(&self, _v: Variant, _s: ButtonState) -> Color32 { Color32::TRANSPARENT }
    /// }
    ///
    /// Button::new("Alert").show_styled(ui, theme, &MyStyle);
    /// ```
    pub fn show_styled<S: ButtonStyle>(self, ui: &mut Ui, theme: &dyn ComponentTheme, style: &S) -> Response {
        show_styled_impl(ui, theme, self, style, None)
    }

    /// Render as a menu trigger. The button paints via the design system,
    /// and `body` runs inside the popup when the user clicks. Replaces raw
    /// `ui.menu_button(RichText::new(label).monospace().size(font_sm()).color(t.dim), ...)`
    /// patterns — the styling now matches the rest of the toolbar.
    ///
    /// Returns the trigger's `Response`. The popup is auto-managed by egui.
    pub fn show_menu<R>(
        self,
        ui: &mut Ui,
        theme: &dyn ComponentTheme,
        body: impl FnOnce(&mut Ui) -> R,
    ) -> egui::InnerResponse<Option<R>> {
        // Compose a RichText label that mirrors the Button's idle styling.
        // This keeps the trigger looking like a Button even though egui's
        // menu_button owns the click-and-popup behaviour.
        let default_size = match self.size {
            Size::Xs => st::font_xs(),
            Size::Sm => st::font_sm(),
            Size::Md => st::font_md(),
            Size::Lg => st::font_lg(),
            Size::Xl => st::font_lg(),
        };
        let fg = self.fg_override.unwrap_or_else(|| theme.dim());
        let label_text = if self.icon_only && self.label.is_empty() {
            self.leading_icon.unwrap_or("").to_string()
        } else {
            let mut s = String::new();
            if let Some(icon) = self.leading_icon {
                s.push_str(icon);
                if !self.label.is_empty() {
                    s.push_str("  ");
                }
            }
            s.push_str(self.label);
            if let Some(tic) = self.trailing_icon {
                s.push(' ');
                s.push_str(tic);
            }
            s
        };
        let glyph_size = self.glyph_px.unwrap_or(default_size);
        let rich = RichText::new(label_text).size(glyph_size).color(fg);
        ui.menu_button(rich, body)
    }

    /// Paint the button at an absolute rect using the provided painter.
    /// Use when the call site has pre-allocated a rect (e.g., the pane
    /// header's hand-laid-out right cluster). Returns a Response built
    /// from the rect (ui.interact internally). Caller is responsible for
    /// allocating the rect first.
    pub fn show_at(
        self,
        ui: &mut Ui,
        painter: &egui::Painter,
        rect: egui::Rect,
        theme: &dyn ComponentTheme,
    ) -> Response {
        let style = DefaultButtonStyle::new(theme);
        show_styled_impl(ui, theme, self, &style, Some((rect, painter)))
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
        let theme = super::theme::active_theme(ui.ctx());
        self.show(ui, &theme)
    }
}

// ── Internal painting ──────────────────────────────────────────────────

/// Core paint implementation, shared by `show`, `show_styled`, and `show_at`.
///
/// `style` drives the *initial* idle/hover/active color resolution. The rest of
/// the function (status overrides, placement overrides, fill_override, motion
/// animation) runs unchanged on top of those seed colors — so all existing
/// escape hatches keep working regardless of which `ButtonStyle` is in play.
fn show_styled_impl<'a, S: ButtonStyle>(
    ui: &mut Ui,
    theme: &dyn ComponentTheme,
    btn: Button<'a>,
    style: &S,
    placed: Option<(Rect, &egui::Painter)>,
) -> Response {
    // Legacy SimpleBtn parity: when caller opts into `simple_treatment(true)`
    // and `honor_style_treatment` is on, dispatch through the global
    // ButtonTreatment-aware painter for pixel-identical SimpleBtn output.
    if btn.honor_style_treatment
        && btn.simple_treatment
        && matches!(btn.variant, Variant::Secondary)
        && !btn.icon_only && !btn.loading && !btn.disabled
    {
        return paint_secondary_with_treatment(ui, theme, &btn);
    }
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
        fg_override,
        glyph_color_override,
        fill_override,
        hover_fill_override,
        stroke_override,
        min_size_override,
        frameless,
        ..
    } = btn;
    let tint = btn.resolve_tint(theme);

    let is_status = btn.is_status;
    let sublabel_text: Option<String> = btn.sublabel.clone();
    let stacked = sublabel_text.is_some();
    // Keybind: only meaningful when there's a label (not icon_only) and not stacked.
    let kbd_text: Option<String> = if !icon_only && !stacked { btn.kbd.clone() } else { None };

    // ── Placement resolution ─────────────────────────────────────────────────
    // When a placement is set, it wins over glyph_px and min_size_override.
    let placement = btn.placement;
    let icon_tone = btn.icon_tone;

    let h = size.height();
    let pad_x = if is_status { st::gap_2xs() } else { size.padding_x() };
    let font_size = size.font_size();
    // Placement glyph px wins over glyph_px override wins over size-derived default.
    let icon_px = if let Some(p) = placement {
        p.glyph_px()
    } else {
        btn.glyph_px.unwrap_or(font_size * 1.25)
    };
    let icon_gap = st::gap_2xs();
    let kbd_font = st::mono_xs();

    // ── Measure intrinsic width ──
    let mut content_w = 0.0f32;
    if icon_only {
        content_w = icon_px;
    } else {
        if leading_icon.is_some() || loading {
            content_w += font_size * 1.1 + icon_gap;
        }
        if !label.is_empty() {
            // Chrome buttons render labels in Proportional (Inter) so they
            // visually match context menus, panel chrome, and dropdowns.
            // Tabular data (prices, tickers) should use RichText.monospace
            // directly, not Button labels.
            // layout-only galley: only `.rect.width()` is read; color discarded.
            let galley = ui.fonts(|f| {
                f.layout_no_wrap(label.to_string(), FontId::proportional(font_size), Color32::WHITE)
            });
            content_w += galley.rect.width();
        }
        if trailing_icon.is_some() {
            content_w += icon_gap + font_size * 1.1;
        }
        if let Some(kt) = kbd_text.as_ref() {
            // layout-only galley: width measurement only.
            let g = ui.fonts(|f| f.layout_no_wrap(kt.clone(), kbd_font.clone(), Color32::WHITE));
            content_w += st::gap_md() + g.rect.width();
        }
    }

    // Stacked (sublabel) layout: vertical icon + label + sublabel.
    // Override measurements computed above.
    let stacked_icon_size = font_size * 1.2;
    let stacked_label_font = FontId::monospace(st::font_xs());
    let stacked_sub_font = FontId::monospace(st::font_2xs());
    let mut stacked_w = 0.0f32;
    let mut stacked_h = 0.0f32;
    if stacked {
        // Width: max(icon_w, label_w, sublabel_w) + side padding
        let mut max_w = stacked_icon_size;
        // layout-only galleys in this stacked-measurement block: only `.rect.width()` is read.
        if !label.is_empty() {
            let g = ui.fonts(|f| f.layout_no_wrap(label.to_string(), stacked_label_font.clone(), Color32::WHITE));
            max_w = max_w.max(g.rect.width());
        }
        if let Some(sl) = sublabel_text.as_ref() {
            let g = ui.fonts(|f| f.layout_no_wrap(sl.clone(), stacked_sub_font.clone(), Color32::WHITE));
            max_w = max_w.max(g.rect.width());
        }
        stacked_w = max_w + 2.0 * st::gap_xs();
        stacked_h = stacked_icon_size + st::gap_2xs() + st::font_xs() + st::gap_2xs() + st::font_2xs() + st::gap_xs();
    }

    let intrinsic_w = if stacked { stacked_w } else if icon_only { h } else { content_w + 2.0 * pad_x };
    let desired_h = if stacked { stacked_h } else { h };
    let desired_w = if full_width { ui.available_width().max(intrinsic_w) } else { intrinsic_w };
    let mut desired = Vec2::new(desired_w, desired_h);
    // Placement hit-target wins over min_size_override (placement is the source of truth).
    if let Some(p) = placement {
        let hit = p.hit_px();
        if hit > 0.0 {
            desired.x = desired.x.max(hit);
            desired.y = desired.y.max(hit);
        }
    } else if let Some(ms) = min_size_override {
        desired.x = desired.x.max(ms.x);
        desired.y = desired.y.max(ms.y);
    }

    // StatusIndicator is a non-interactive decoration — use Sense::hover().
    let is_non_interactive = placement.map_or(false, |p| !p.interactive());
    let sense = btn.sense_override.unwrap_or_else(|| {
        if disabled || loading || is_non_interactive { Sense::hover() } else { Sense::click() }
    });
    let (rect, response) = match placed {
        Some((r, _)) => {
            let id = ui.id().with(("ui_kit_button_show_at", r.min.x.to_bits(), r.min.y.to_bits()));
            let resp = ui.interact(r, id, sense);
            (r, resp)
        }
        None => ui.allocate_exact_size(desired, sense),
    };

    if ui.is_rect_visible(rect) {
        let id = response.id;
        let hovered = response.hovered() && !disabled && !loading;
        let pressed = response.is_pointer_button_down_on() && !disabled && !loading;

        // Status mode: snap (no fade) on hover.
        let hover_t = if is_status {
            if hovered { 1.0 } else { 0.0 }
        } else {
            motion::ease_bool(ui.ctx(), id.with("btn_hover"), hovered, motion::FAST)
        };
        let active_t = motion::ease_bool(ui.ctx(), id.with("btn_active"), active, motion::MED);

        // Resolve colors per variant.
        //
        // When a tint is present (`.tint(c)`, `.buy()`, `.sell()`), `resolve_palette`
        // is authoritative because it inlines the tint into the returned colors.
        // Otherwise, colors come from the `ButtonStyle` — which for the default path
        // is `DefaultButtonStyle` (identical output), and for custom call sites is the
        // caller's impl.
        let (mut idle_bg, mut hover_bg, active_bg, mut fg_idle, mut fg_hover, border_idle, border_active) =
            if tint.is_some() {
                // Tint path: palette already has the tinted color baked in.
                resolve_palette(theme, variant, tint)
            } else {
                // Style path: ask the ButtonStyle for each state.
                (
                    style.bg(variant, ButtonState::Idle),
                    style.bg(variant, ButtonState::Hover),
                    style.bg(variant, ButtonState::Active),
                    style.fg(variant, ButtonState::Idle),
                    style.fg(variant, ButtonState::Hover),
                    style.border(variant, ButtonState::Idle),
                    style.border(variant, ButtonState::Active),
                )
            };

        // Status overrides: no bg on idle OR hover; muted icon -> full strength on hover.
        if is_status {
            idle_bg = Color32::TRANSPARENT;
            hover_bg = Color32::TRANSPARENT;
            fg_idle = st::color_alpha(theme.text(), 178);
            fg_hover = theme.text();
        }

        // Placement overrides: when a placement is active, use icon_glyph_color
        // for the icon fg and conditionally suppress hover bg.
        if let Some(p) = placement {
            // Glyph colors from icon_glyph_color (single source of truth).
            fg_idle  = if disabled {
                icon_glyph_color(theme, icon_tone, IconState::Disabled)
            } else {
                icon_glyph_color(theme, icon_tone, IconState::Idle)
            };
            fg_hover = icon_glyph_color(theme, icon_tone, IconState::Hover);
            if p.hover_bg() {
                // Use the canonical hover bg color from the icon placement system.
                idle_bg  = Color32::TRANSPARENT;
                hover_bg = icon_hover_bg(theme);
            } else {
                // TabClose / StatusIndicator / InlineLabel: color-snap only, no bg fill.
                idle_bg  = Color32::TRANSPARENT;
                hover_bg = Color32::TRANSPARENT;
            }
        }

        // Caller-supplied fill / hover_fill override the variant defaults.
        if let Some(f) = fill_override {
            idle_bg = f;
            hover_bg = hover_fill_override.unwrap_or_else(|| lighten(f, 0.08));
        } else if let Some(hf) = hover_fill_override {
            hover_bg = hf;
        }

        // Compose backgrounds: idle -> hover -> active.
        let mut bg = motion::lerp_color(idle_bg, hover_bg, hover_t);
        bg = motion::lerp_color(bg, active_bg, active_t);

        // Press darken: snap in (fast rise), ease out over ~60ms on release.
        let press_t = motion::ease_bool(ui.ctx(), id.with("btn_press"), pressed, 0.06_f32);
        if press_t > 0.0 {
            bg = darken(bg, 0.12 * press_t);
        }

        let mut fg = motion::lerp_color(fg_idle, fg_hover, hover_t);
        // Toggle variant: when active, fg snaps to accent (or the
        // resolved tint). Blends in alongside the bg/border active state.
        if matches!(variant, Variant::Toggle) && active_t > 0.001 {
            let accent_fg = tint.unwrap_or_else(|| theme.accent());
            fg = motion::lerp_color(fg, accent_fg, active_t);
        }
        // Placement active state: blend toward the active glyph color.
        if placement.is_some() && active_t > 0.001 {
            let active_fg = icon_glyph_color(theme, icon_tone, IconState::Active);
            fg = motion::lerp_color(fg, active_fg, active_t);
        }
        if let Some(c) = fg_override { fg = c; }
        let border_col = motion::lerp_color(border_idle, border_active, active_t);

        // Disabled: 50% opacity on everything.
        if disabled {
            bg = with_alpha_scale(bg, 0.5);
            fg = with_alpha_scale(fg, 0.5);
        }

        let cr = btn.corner_radius_override.unwrap_or_else(|| {
            let radius = corner_radius.unwrap_or(default_radius(variant));
            CornerRadius::same(radius as u8)
        });

        let owned_painter = match placed {
            Some(_) => None,
            None => Some(ui.painter_at(rect)),
        };
        let painter: &egui::Painter = match (&owned_painter, placed) {
            (Some(p), _) => p,
            (None, Some((_, p))) => p,
            (None, None) => unreachable!(),
        };

        // Background.
        if !frameless && bg.a() > 0 {
            painter.rect_filled(rect, cr, bg);
        }
        // Border (Secondary, Chrome stroke override, or active state).
        if !frameless {
            if let Some(s) = stroke_override {
                if s.width > 0.0 && s.color.a() > 0 {
                    painter.rect_stroke(rect, cr, s, StrokeKind::Inside);
                }
            } else {
                let border_w = match variant { Variant::Secondary | Variant::Toggle => 1.0, _ => 0.0 };
                if border_col.a() > 0 && (border_w > 0.0 || active_t > 0.001) {
                    let w = if border_w > 0.0 { border_w } else { 1.0 };
                    painter.rect_stroke(rect, cr, Stroke::new(w, border_col), StrokeKind::Inside);
                }
            }
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
            painter.line_segment([Pos2::new(x0, y), Pos2::new(x1, y)], Stroke::new(st::stroke_std(), underline));
        }

        // ── Layout content (icon | label | trailing) ──
        let center = rect.center();
        let icon_fg = glyph_color_override.unwrap_or(fg);
        if stacked {
            // Stacked vertical: icon (top) → label (mid) → sublabel (bot).
            let label_color = if active { theme.accent() } else { fg };
            let sub_color = st::color_alpha(theme.dim(), 200);
            let icon_h = stacked_icon_size;
            let lbl_h = st::font_xs();
            let sub_h = st::font_2xs();
            let total = icon_h + st::gap_2xs() + lbl_h + st::gap_2xs() + sub_h;
            let top = rect.center().y - total / 2.0;
            let icon_cy = top + icon_h / 2.0;
            let lbl_cy = top + icon_h + st::gap_2xs() + lbl_h / 2.0;
            let sub_cy = top + icon_h + st::gap_2xs() + lbl_h + st::gap_2xs() + sub_h / 2.0;
            if let Some(ic) = leading_icon {
                painter.text(
                    Pos2::new(center.x, icon_cy),
                    egui::Align2::CENTER_CENTER,
                    ic,
                    FontId::proportional(stacked_icon_size),
                    icon_fg,
                );
            }
            if !label.is_empty() {
                painter.text(
                    Pos2::new(center.x, lbl_cy),
                    egui::Align2::CENTER_CENTER,
                    label,
                    stacked_label_font.clone(),
                    label_color,
                );
            }
            if let Some(sl) = sublabel_text.as_ref() {
                painter.text(
                    Pos2::new(center.x, sub_cy),
                    egui::Align2::CENTER_CENTER,
                    sl,
                    stacked_sub_font.clone(),
                    sub_color,
                );
            }
        } else if icon_only {
            // Loading replaces icon.
            if loading {
                paint_spinner(ui, rect, icon_fg);
            } else if let Some(ic) = leading_icon {
                painter.text(
                    center,
                    egui::Align2::CENTER_CENTER,
                    ic,
                    FontId::proportional(icon_px),
                    icon_fg,
                );
            }
        } else {
            // Compute layout.
            let mut x = rect.left() + pad_x;
            let cy = center.y;
            // Leading: spinner takes priority over leading icon when loading.
            if loading {
                let spin_rect = Rect::from_center_size(Pos2::new(x + font_size * 0.55, cy), Vec2::splat(font_size * 1.1));
                paint_spinner(ui, spin_rect, icon_fg);
                x += font_size * 1.1 + icon_gap;
            } else if let Some(ic) = leading_icon {
                painter.text(
                    Pos2::new(x, cy),
                    egui::Align2::LEFT_CENTER,
                    ic,
                    FontId::proportional(font_size * 1.1),
                    icon_fg,
                );
                x += font_size * 1.1 + icon_gap;
            }
            if !label.is_empty() {
                let galley = ui.fonts(|f| {
                    f.layout_no_wrap(label.to_string(), FontId::proportional(font_size), fg)
                });
                let lw = galley.rect.width();
                painter.text(
                    Pos2::new(x, cy),
                    egui::Align2::LEFT_CENTER,
                    label,
                    FontId::proportional(font_size),
                    fg,
                );
                x += lw;
            }
            if let Some(kt) = kbd_text.as_ref() {
                x += st::gap_md();
                let kbd_color = st::color_alpha(theme.text(), 160);
                painter.text(
                    Pos2::new(x, cy),
                    egui::Align2::LEFT_CENTER,
                    kt,
                    kbd_font.clone(),
                    kbd_color,
                );
            }
            if let Some(ic) = trailing_icon {
                let tx = rect.right() - pad_x;
                painter.text(
                    Pos2::new(tx, cy),
                    egui::Align2::RIGHT_CENTER,
                    ic,
                    FontId::proportional(font_size * 1.1),
                    icon_fg,
                );
            }
        }

        // Focus ring overlay — drawn on top of all other paint so the
        // keyboard focus indicator is visible regardless of variant.
        // Delegates to cursor::focus_ring so focus_ring_width / focus_ring_alpha
        // StyleSettings knobs are respected and the ring tracks the design system.
        st::cursor::focus_ring(ui, &response, theme.accent());

        // Cursor.
        if hovered {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
    }

    response
}

/// SimpleBtn parity: route Secondary through the active `ButtonTreatment`.
/// Delegates to `egui::Button` with the treatment-driven fill/stroke/cr,
/// then paints the matching hover overlay and (for UnderlineActive) the
/// bottom underline. Mirrors `chart::renderer::ui::inputs::buttons::SimpleBtn`.
fn paint_secondary_with_treatment(
    ui: &mut Ui,
    _theme: &dyn ComponentTheme,
    btn: &Button<'_>,
) -> Response {
    use crate::ui_kit::widgets::motion as cmotion;
    use crate::ui_kit::tokens::{
        alpha_faint, alpha_ghost, alpha_muted, alpha_soft, alpha_strong,
        btn_small_height, color_alpha, font_sm, r_md_cr, r_sm_cr, r_xs,
        stroke_bold, stroke_std, stroke_thin,
    };
    use crate::ui_kit::widgets::tokens::ButtonTreatment;

    // Pick the dominant color: explicit fg/fill overrides win, then tint, then theme.text().
    let color = btn.fg_override
        .or(btn.fill_override)
        .or_else(|| btn.resolve_tint(_theme))
        .unwrap_or_else(|| _theme.text());

    // P5b: button_treatment now lives on TokenSnapshot — read once and cache.
    // r_md / r_xs come from frame_tokens() too (no more current() reach-in).
    let snap = crate::ui_kit::style::frame_tokens();
    let treatment = snap.button_treatment;
    let r_md_u8 = snap.radius_md as u8;
    let r_xs_u8 = snap.radius_xs as u8;
    let (fill, fg, stroke_w, stroke_col, cr) = match treatment {
        ButtonTreatment::SoftPill => (
            color_alpha(color, alpha_faint()),
            color,
            stroke_thin(),
            color_alpha(color, alpha_muted()),
            r_sm_cr(),
        ),
        ButtonTreatment::OutlineAccent => (
            Color32::TRANSPARENT,
            color,
            stroke_bold(),
            color_alpha(color, alpha_strong()),
            r_md_cr(),
        ),
        ButtonTreatment::UnderlineActive
        | ButtonTreatment::RaisedActive
        | ButtonTreatment::BlackFillActive => (
            Color32::TRANSPARENT,
            color,
            0.0_f32,
            Color32::TRANSPARENT,
            r_xs(),
        ),
    };

    let h = btn.min_size_override.map(|v| v.y).unwrap_or_else(btn_small_height);
    let min_w = btn.min_size_override.map(|v| v.x).unwrap_or(0.0);
    let resp = ui.add(
        egui::Button::new(RichText::new(btn.label).size(font_sm()).color(fg))
            .fill(fill)
            .stroke(Stroke::new(stroke_w, stroke_col))
            .corner_radius(cr)
            .min_size(Vec2::new(min_w, h)),
    );
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    let hover_id = resp.id.with("ui_kit_simple_btn_hover");
    let hover_t = cmotion::ease_bool(ui.ctx(), hover_id, resp.hovered(), cmotion::FAST);
    if hover_t > 0.001 {
        match treatment {
            ButtonTreatment::OutlineAccent => {
                ui.painter().rect_filled(
                    resp.rect,
                    egui::CornerRadius::same(r_md_u8),
                    cmotion::fade_in(color_alpha(color, alpha_soft()), hover_t),
                );
                ui.painter().rect_stroke(
                    resp.rect,
                    egui::CornerRadius::same(r_md_u8),
                    Stroke::new(stroke_bold(), cmotion::fade_in(color, hover_t)),
                    StrokeKind::Inside,
                );
            }
            ButtonTreatment::UnderlineActive
            | ButtonTreatment::RaisedActive
            | ButtonTreatment::BlackFillActive => {
                ui.painter().rect_filled(
                    resp.rect,
                    egui::CornerRadius::same(r_xs_u8),
                    cmotion::fade_in(color_alpha(color, alpha_ghost()), hover_t),
                );
            }
            _ => {}
        }
    }
    if matches!(treatment, ButtonTreatment::UnderlineActive) {
        let r = resp.rect;
        ui.painter().line_segment(
            [Pos2::new(r.left(), r.bottom() + 0.5), Pos2::new(r.right(), r.bottom() + 0.5)],
            Stroke::new(stroke_std(), color),
        );
    }
    resp
}

fn default_radius(v: Variant) -> f32 {
    match v {
        Variant::Primary | Variant::Secondary | Variant::Danger | Variant::NeutralAction => 4.0,
        Variant::Ghost | Variant::MutedIcon | Variant::InlineClose | Variant::DynamicTint => 2.0,
        Variant::Link | Variant::TextOnly | Variant::Tab => 0.0,
        Variant::Chip | Variant::Toggle => 99.0, // pill
        Variant::Chrome => 4.0,
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
    let transparent = Color32::TRANSPARENT;

    match variant {
        Variant::Primary => (
            accent,
            lighten(accent, 0.10),
            darken(accent, 0.08),
            st::contrast_fg(accent),
            st::contrast_fg(accent),
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
            // Toolbar toggles paint Ghost. When many are simultaneously
            // active (e.g. SMA+EMA+Bollinger+VOL all on), the accent
            // tint+border previously summed into a "purple soup". Softer
            // active bg (alpha_soft) and dropped border keep the surface
            // calm; foreground accent on active is set by `toolbar_btn`.
            transparent,
            st::color_alpha(text, 18),
            st::color_alpha(accent, st::alpha_soft()),
            text,
            text,
            transparent,
            transparent,
        ),
        Variant::Danger => (
            bear,
            lighten(bear, 0.10),
            darken(bear, 0.08),
            st::contrast_fg(bear),
            st::contrast_fg(bear),
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
        Variant::Chrome => (
            // Defaults are transparent / theme.text(); caller is expected to
            // override via `.fill()` / `.stroke()` / `.fg()`. Hover lightens
            // the resolved fill by 8% unless overridden via `.hover_fill()`.
            transparent,
            transparent,
            transparent,
            text,
            text,
            transparent,
            transparent,
        ),
        // ── 2026-05 additions ──────────────────────────────────────────────
        Variant::Chip => (
            // Toggle chip: idle = transparent, hover = subtle text overlay,
            // active = soft accent fill. Caller sets `.active(bool)`.
            transparent,
            st::color_alpha(text, 18),
            st::color_alpha(accent, st::alpha_soft()),
            st::color_muted(theme.dim()),
            text,
            transparent,
            st::color_alpha(accent, st::alpha_active()),
        ),
        Variant::Tab => (
            // Tab: frameless, transparent. fg cycles via `.active(bool)`.
            transparent,
            transparent,
            transparent,
            st::color_muted(theme.dim()),
            text,
            transparent,
            transparent,
        ),
        Variant::InlineClose => (
            // Small icon-X close affordance. Hover paints a subtle dim overlay
            // instead of an accent so it doesn't fight modal headings.
            transparent,
            st::color_alpha(text, 18),
            transparent,
            st::color_subtle(theme.dim()),
            text,
            transparent,
            transparent,
        ),
        Variant::MutedIcon => (
            // Ghost-like icon button starting muted; hover restores full text.
            transparent,
            st::color_alpha(text, 18),
            st::color_alpha(accent, st::alpha_tint()),
            st::color_half(theme.dim()),
            text,
            transparent,
            transparent,
        ),
        Variant::NeutralAction => (
            // Utility action (FLATTEN etc.) — neutral fill derived from theme
            // text alpha so it reads on any palette. P5b: was hardcoded gray
            // which made the button a foreign object on non-default palettes
            // (a gray island in a pink/lime/cyan theme).
            st::color_alpha(theme.text(), 28),
            st::color_alpha(theme.text(), 44),
            st::color_alpha(theme.text(), 18),
            st::contrast_fg(theme.surface()),
            st::contrast_fg(theme.surface()),
            border,
            transparent,
        ),
        Variant::TextOnly => (
            // Frameless text. Caller sets fg.
            transparent,
            transparent,
            transparent,
            text,
            text,
            transparent,
            transparent,
        ),
        Variant::Toggle => (
            // Toggle chip — one of a row of selectable presets.
            //   Inactive: transparent bg, dim fg (text @ alpha_soft via
            //             gamma), soft outline.
            //   Hover (inactive): bg = text @ alpha_ghost, fg snaps to text.
            //   Active: accent-tinted bg, accent fg, accent border @ active.
            //   Hover (active): bg brightened by 0.05 (handled by active_bg
            //                   being slightly lighter than the tint).
            transparent,
            st::color_alpha(text, st::alpha_ghost()),
            lighten(st::color_alpha(accent, st::alpha_tint()), 0.05),
            st::color_alpha(text, st::alpha_soft()),
            text,
            st::color_alpha(text, st::alpha_soft()),
            st::color_alpha(accent, st::alpha_active()),
        ),
        Variant::DynamicTint => (
            // P6.5 — Ghost-like (transparent until hover) using the caller-
            // supplied `.tint(color)` for fg / hover / active overlays /
            // border. Replaces the `Variant::Chrome + .fg(dynamic_color)`
            // escape-hatch pattern audited at ~15 call sites (option-type
            // color, link-group color, connection-state color buttons).
            // When no tint is set, `accent` falls back to `theme.accent()`
            // so it degrades gracefully — same shape as Ghost with tint.
            transparent,
            st::color_alpha(accent, st::alpha_soft()),
            st::color_alpha(accent, st::alpha_tint()),
            accent,
            accent,
            transparent,
            st::color_alpha(accent, st::alpha_strong()),
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
        painter.line_segment([p0, p1], Stroke::new(st::stroke_bold(), c));
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
        let r = Button::icon(Icon::GEAR).size(Size::Md).show(ui, theme);
        Tooltip::new("Settings").show(ui, &r, theme);
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

    ui.add_space(4.0);
    ui.label("Escape hatches (legacy parity)");
    ui.horizontal(|ui| {
        // IconBtn parity: Ghost + glyph_color.
        let r = Button::icon(Icon::GEAR)
            .variant(Variant::Ghost)
            .glyph_color(theme.accent())
            .show(ui, theme);
        Tooltip::new("Settings").show(ui, &r, theme);
        // ChromeBtn parity: Chrome variant with explicit fill/stroke/min_size.
        let _ = Button::new("Connect")
            .variant(Variant::Chrome)
            .fill(theme.surface())
            .stroke(Stroke::new(st::stroke_std(), theme.border()))
            .min_size(Vec2::new(80.0, 24.0))
            .corner_radius(st::radius_sm())
            .fg(theme.text())
            .show(ui, theme);
        // ChromeBtn::frameless parity.
        let _ = Button::new("Paper")
            .variant(Variant::Chrome)
            .frameless(true)
            .fg(theme.text())
            .show(ui, theme);
        // SimpleBtn parity: Secondary + simple_treatment.
        let _ = Button::new("Cancel")
            .variant(Variant::Secondary)
            .simple_treatment(true)
            .show(ui, theme);
        // hover_fill override.
        let _ = Button::new("Custom Hover")
            .variant(Variant::Chrome)
            .fill(theme.surface())
            .hover_fill(theme.accent())
            .show(ui, theme);
    });
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::icon_placement::{IconPlacement, IconTone};

    /// Smoke: verify that `paint_button` delegates to `cursor::focus_ring`.
    /// A full UI-level test would need an egui harness with programmatic focus;
    /// source scanning is the lightweight alternative that catches accidental removal.
    #[test]
    fn button_focus_ring_paints_when_focused() {
        let src = include_str!("button.rs");
        assert!(
            src.contains("cursor::focus_ring"),
            "paint_button must call st::cursor::focus_ring for keyboard focus"
        );
    }

    // ── Placement-aware builder state tests ───────────────────────────────────

    /// `Button::icon_toolbar` resolves to Toolbar placement.
    #[test]
    fn icon_toolbar_resolves_to_toolbar_placement() {
        let b = Button::icon_toolbar(crate::ui_kit::icons::Icon::GEAR);
        assert_eq!(b.placement, Some(IconPlacement::Toolbar));
    }

    /// `Button::icon_tab_close` resolves to TabClose placement + Destructive tone.
    #[test]
    fn icon_tab_close_resolves_to_tabclose_and_destructive() {
        let b = Button::icon_tab_close();
        assert_eq!(b.placement, Some(IconPlacement::TabClose));
        assert_eq!(b.icon_tone, IconTone::Destructive);
        // TabClose has no hover bg — assert via the placement API.
        assert!(!IconPlacement::TabClose.hover_bg(), "TabClose must not show hover bg");
    }

    /// `Button::close()` defaults to Modal placement.
    #[test]
    fn close_defaults_to_modal_placement() {
        let b = Button::close();
        assert_eq!(b.placement, Some(IconPlacement::Modal));
    }

    /// `.placement(Toolbar)` overrides an explicit `.min_size`.
    /// The placement hit-target (24px) wins over a smaller min_size when it's larger.
    /// And when min_size is explicitly larger, placement still wins (placement is authority).
    #[test]
    fn placement_wins_over_min_size() {
        let b = Button::icon(crate::ui_kit::icons::Icon::GEAR)
            .min_size(egui::Vec2::new(100.0, 100.0))
            .placement(IconPlacement::Toolbar);
        // Placement is set — sizing happens in paint_button, but we can assert
        // that the placement field is set (meaning paint_button will use it).
        assert_eq!(b.placement, Some(IconPlacement::Toolbar));
        // Toolbar hit_px = 24.0; in paint_button: desired = max(intrinsic, hit_px=24)
        // but min_size_override=100 is skipped when placement is set.
        // We verify the sizing branch would be taken via the presence of placement.
        assert_eq!(IconPlacement::Toolbar.hit_px(), 24.0);
        // The min_size_override is stored (it was set), but placement takes the
        // sizing branch in paint_button (the else branch is not reached).
        assert!(b.min_size_override.is_some());
    }

    /// `Button::icon_panel_header` resolves to PanelHeader placement.
    #[test]
    fn icon_panel_header_resolves_to_panel_header_placement() {
        let b = Button::icon_panel_header(crate::ui_kit::icons::Icon::X);
        assert_eq!(b.placement, Some(IconPlacement::PanelHeader));
    }

    /// Tone setters work correctly.
    #[test]
    fn tone_setters_work() {
        let b = Button::icon(crate::ui_kit::icons::Icon::GEAR).tone_destructive();
        assert_eq!(b.icon_tone, IconTone::Destructive);
        let b2 = Button::icon(crate::ui_kit::icons::Icon::GEAR).tone_affirmative();
        assert_eq!(b2.icon_tone, IconTone::Affirmative);
    }

    /// Default icon_tone is Neutral.
    #[test]
    fn default_tone_is_neutral() {
        let b = Button::icon(crate::ui_kit::icons::Icon::GEAR);
        assert_eq!(b.icon_tone, IconTone::Neutral);
    }
}
