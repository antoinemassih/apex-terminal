//! Design-system menu item — the canonical primitive for every dropdown row.
//!
//! `MenuItem` replaces raw `ui.button(...)` / `egui::RichText::new(...).color(...)` calls
//! inside `egui::context_menu` and `menu_button` closures. The row sizes to its
//! content (so the menu hugs its widest item, never the screen), and on hover draws
//! the same themed treatment a native egui menu button gets — toolbar-border fill,
//! accent-tinted border, rounded corners — eased in like `ui_kit::Button`. Idle label
//! text is dim and brightens on hover. A pointer-hand cursor is applied when enabled.
//! Surfaces a plain `egui::Response` so callers do
//! `if MenuItem::new(...).show(ui, t).clicked() { ... }`.
//!
//! ## Variants (via builder methods)
//! - `.icon(Icon::*)` — leading glyph (Phosphor Bold)
//! - `.tint(Color32)` — colours label + icon (bull / bear / accent / warn)
//! - `.enabled(bool)` — disabled rows: dimmed, non-interactive, no pointer cursor
//! - `.shortcut(str)` — right-aligned dim hint (e.g. "Ctrl+Z")
//! - `.submenu(bool)` — trailing ▸ arrow indicator; does NOT open a submenu itself
//! - `.selected(bool)` — leading check-mark when true
//!
//! ## When to use a different widget
//! - Use `ui.menu_button(...)` / `Button::menu(...)` for items that open a nested submenu.
//! - Use `ui_kit::Button` everywhere outside a menu context.
//! - Use `ui.label(...)` + `ui.separator()` for section headers and dividers inside menus.

use egui::{Color32, FontId, Pos2, Rect, Response, Sense, Stroke, Ui, Vec2};

use super::motion;
use super::theme::ComponentTheme;
use crate::chart::renderer::ui::style as st;
use crate::ui_kit::icons::Icon;

/// Row height for menu items — matches egui's native menu-button height as configured
/// by `setup_theme`. 26 px is the standard interact_size.y across all active themes.
const MENU_ROW_H: f32 = 26.0;

/// Horizontal padding inside the row (left edge to icon/label, label to right edge).
const PAD_X: f32 = 8.0;

/// Gap between icon glyph and label text.
const ICON_GAP: f32 = 5.0;

/// Font size for the label — matches `font_sm()` used by egui menu buttons.
const LABEL_FONT: f32 = 11.0;

/// Font size for the shortcut hint and the submenu arrow.
const HINT_FONT: f32 = 10.0;

/// Font size for the leading icon glyph.
const ICON_FONT: f32 = 13.0;

/// Font size for the leading check-mark.
const CHECK_FONT: f32 = 11.0;

/// Opacity multiplier for disabled rows.
const DISABLED_ALPHA: f32 = 0.38;

/// Builder for a single-row menu item.
///
/// Construct via [`MenuItem::new`], chain builder methods, then call [`MenuItem::show`].
/// The returned [`egui::Response`] carries `.clicked()` for action dispatch.
#[must_use = "MenuItem does nothing until `.show(ui, theme)` is called"]
pub struct MenuItem<'a> {
    label: String,
    icon: Option<&'a str>,
    tint: Option<Color32>,
    enabled: bool,
    shortcut: Option<String>,
    submenu: bool,
    selected: bool,
}

impl<'a> MenuItem<'a> {
    /// Create a new menu-item row with `label` as its primary text.
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            icon: None,
            tint: None,
            enabled: true,
            shortcut: None,
            submenu: false,
            selected: false,
        }
    }

    /// Optional leading Phosphor Bold icon glyph (e.g. `Icon::TRASH`).
    pub fn icon(mut self, glyph: &'a str) -> Self {
        self.icon = Some(glyph);
        self
    }

    /// Tint the label and icon with `color`. Use theme semantics:
    /// `t.bull` for buy/positive, `t.bear` for destructive, `t.accent` for neutral CTAs.
    pub fn tint(mut self, color: Color32) -> Self {
        self.tint = Some(color);
        self
    }

    /// When `false`, the item is dimmed and non-interactive (no pointer cursor, no click).
    pub fn enabled(mut self, v: bool) -> Self {
        self.enabled = v;
        self
    }

    /// Right-aligned shortcut hint text (e.g. `"Ctrl+Z"`). Rendered in dim mono.
    pub fn shortcut(mut self, hint: impl Into<String>) -> Self {
        self.shortcut = Some(hint.into());
        self
    }

    /// When `true`, a trailing ▸ glyph is rendered to signal a nested submenu.
    /// Does NOT open the submenu itself — pair with `ui.menu_button(...)` for that.
    pub fn submenu(mut self, v: bool) -> Self {
        self.submenu = v;
        self
    }

    /// When `true`, a leading ✓ check-mark is rendered (active/selected state).
    pub fn selected(mut self, v: bool) -> Self {
        self.selected = v;
        self
    }

    /// Render the menu item into `ui` using colors from `theme`.
    /// Returns the interaction [`Response`]; check `.clicked()` to dispatch the action.
    pub fn show(self, ui: &mut Ui, theme: &dyn ComponentTheme) -> Response {
        paint_menu_item(ui, theme, self)
    }
}

// ── Internal rendering ─────────────────────────────────────────────────────────

fn paint_menu_item(ui: &mut Ui, theme: &dyn ComponentTheme, item: MenuItem<'_>) -> Response {
    // ── Measure content width so the menu hugs its content, not the screen ──
    let label_galley = ui.painter().layout_no_wrap(
        item.label.clone(),
        FontId::proportional(LABEL_FONT),
        Color32::WHITE,
    );
    let mut content_w = PAD_X * 2.0 + label_galley.size().x;
    content_w += CHECK_FONT + ICON_GAP; // leading check slot — always reserved
    if item.icon.is_some() {
        content_w += ICON_FONT + ICON_GAP;
    }
    if item.submenu {
        content_w += HINT_FONT + ICON_GAP;
    }
    if let Some(ref hint) = item.shortcut {
        let g = ui.painter().layout_no_wrap(
            hint.clone(),
            FontId::monospace(HINT_FONT),
            Color32::WHITE,
        );
        content_w += g.size().x + ICON_GAP * 2.0;
    }

    // Allocate the CONTENT width — this drives the menu to size to its widest
    // row. Interact + paint across the settled full menu width so every row's
    // hover band is uniform (the menu's width settles in one frame).
    let sense = if item.enabled { Sense::click() } else { Sense::hover() };
    let band_w = ui.available_width().max(content_w);
    let (rect, alloc_resp) =
        ui.allocate_exact_size(Vec2::new(content_w, MENU_ROW_H), Sense::hover());
    let band = Rect::from_min_size(rect.min, Vec2::new(band_w, MENU_ROW_H));
    let response = ui.interact(band, alloc_resp.id, sense);

    if ui.is_rect_visible(band) {
        let id = response.id;
        let hovered = response.hovered() && item.enabled;
        let pressed = response.is_pointer_button_down_on() && item.enabled;
        let hover_t = motion::ease_bool(ui.ctx(), id.with("mi_hover"), hovered, motion::FAST);

        // Themed widget visuals — the SAME source a native `ui.button()` menu
        // item draws from: hovered = toolbar-border fill + accent-tinted border
        // + rounded corners; inactive text = dim, hovered text = bright.
        let wv_hover = ui.visuals().widgets.hovered;
        let wv_active = ui.visuals().widgets.active;
        let wv_inactive = ui.visuals().widgets.inactive;

        let painter = ui.painter().clone();

        // ── Hover / press band: themed fill + accent border, eased in ────────
        if hover_t > 0.0 {
            let wv = if pressed { wv_active } else { wv_hover };
            painter.rect(
                band.shrink(1.0),
                wv.corner_radius,
                wv.bg_fill.gamma_multiply(hover_t),
                Stroke::new(wv.bg_stroke.width, wv.bg_stroke.color.gamma_multiply(hover_t)),
                egui::StrokeKind::Inside,
            );
        }

        // ── Foreground colour ────────────────────────────────────────────────
        // Tinted items keep their tint at all times (matches the v0.9.7
        // RichText colouring). Untinted items go dim → bright on hover, exactly
        // as a native menu button does via inactive/hovered fg_stroke.
        let mut fg = match item.tint {
            Some(tint) => tint,
            None => lerp_color(wv_inactive.fg_stroke.color, wv_hover.fg_stroke.color, hover_t),
        };
        if !item.enabled {
            fg = fg.gamma_multiply(DISABLED_ALPHA);
        }
        let dim_fg = st::color_alpha(theme.dim(), if item.enabled { 160 } else { 80 });
        let cy = band.center().y;

        // ── Left: check-mark slot, icon, label ───────────────────────────────
        let mut x = band.left() + PAD_X;
        if item.selected {
            painter.text(
                Pos2::new(x, cy),
                egui::Align2::LEFT_CENTER,
                Icon::CHECK,
                FontId::proportional(CHECK_FONT),
                fg,
            );
        }
        x += CHECK_FONT + ICON_GAP;

        if let Some(glyph) = item.icon {
            painter.text(
                Pos2::new(x, cy),
                egui::Align2::LEFT_CENTER,
                glyph,
                FontId::proportional(ICON_FONT),
                fg,
            );
            x += ICON_FONT + ICON_GAP;
        }

        if !item.label.is_empty() {
            painter.text(
                Pos2::new(x, cy),
                egui::Align2::LEFT_CENTER,
                &item.label,
                FontId::proportional(LABEL_FONT),
                fg,
            );
        }

        // ── Right: submenu arrow / shortcut hint ──────────────────────────────
        let mut rx = band.right() - PAD_X;
        if item.submenu {
            painter.text(
                Pos2::new(rx, cy),
                egui::Align2::RIGHT_CENTER,
                Icon::CARET_RIGHT,
                FontId::proportional(HINT_FONT),
                dim_fg,
            );
            rx -= HINT_FONT + ICON_GAP;
        }
        if let Some(ref hint) = item.shortcut {
            painter.text(
                Pos2::new(rx, cy),
                egui::Align2::RIGHT_CENTER,
                hint,
                FontId::monospace(HINT_FONT),
                dim_fg,
            );
        }

        // ── Pointer cursor ────────────────────────────────────────────────────
        if hovered && item.enabled {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
    }

    response
}

/// Linear blend between two colours through linear (`Rgba`) space.
fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let a = egui::Rgba::from(a);
    let b = egui::Rgba::from(b);
    Color32::from(a * (1.0 - t) + b * t)
}
