//! Design-system menu item — the canonical primitive for every dropdown row.
//!
//! `MenuItem` replaces raw `ui.button(...)` / `egui::RichText::new(...).color(...)` calls
//! inside `egui::context_menu` and `menu_button` closures. It allocates the full
//! available width, renders a themed hover band with the same eased-fade animation as
//! `ui_kit::Button`, applies a pointer-hand cursor when enabled, and surfaces a plain
//! `egui::Response` so callers do `if MenuItem::new(...).show(ui, t).clicked() { ... }`.
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

use egui::{Color32, FontId, Pos2, Rect, Response, Sense, Ui, Vec2};

use super::motion;
use super::theme::ComponentTheme;
use crate::chart::renderer::ui::style as st;
use crate::ui_kit::icons::Icon;

/// Row height for menu items — matches egui's native menu-button height as configured
/// by `setup_theme`. 26 px is the standard interact_size.y across all active themes.
const MENU_ROW_H: f32 = 26.0;

/// Horizontal padding inside the row (left edge to icon/label, label to right edge).
/// Mirrors the button_padding set in setup_theme (4px each side).
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
    // Allocate: full available width × MENU_ROW_H. Sense::hover() when disabled
    // so the item still blocks the hover band from flickering through but does
    // not respond to clicks.
    let desired = Vec2::new(ui.available_width(), MENU_ROW_H);
    let sense = if item.enabled { Sense::click() } else { Sense::hover() };
    let (rect, response) = ui.allocate_exact_size(desired, sense);

    if ui.is_rect_visible(rect) {
        let id = response.id;
        let hovered = response.hovered() && item.enabled;
        let pressed = response.is_pointer_button_down_on() && item.enabled;

        // ── Eased hover band — matches Button's Ghost hover treatment ──────────
        // Button uses `motion::ease_bool` with `motion::FAST` for hover (not status
        // snap), so we do the same. Ghost variant hover_bg = text @ alpha 18.
        let hover_t = motion::ease_bool(ui.ctx(), id.with("mi_hover"), hovered, motion::FAST);
        let press_t = motion::ease_bool(ui.ctx(), id.with("mi_press"), pressed, 0.06_f32);

        // Hover band color: text @ alpha 18 (exact Ghost idle→hover from button.rs).
        let hover_bg = st::color_alpha(theme.text(), 18);
        let mut bg = st::color_alpha(theme.text(), (18.0 * hover_t) as u8);
        // On press, darken by 12% (same as Button's press_t branch).
        if press_t > 0.0 {
            let darken_amt = 0.12 * press_t;
            let f = (1.0 - darken_amt).clamp(0.0, 1.0);
            bg = Color32::from_rgba_premultiplied(
                ((bg.r() as f32) * f) as u8,
                ((bg.g() as f32) * f) as u8,
                ((bg.b() as f32) * f) as u8,
                bg.a().saturating_add(((hover_bg.a() as f32) * 0.12 * press_t) as u8),
            );
        }

        let painter = ui.painter_at(rect);

        // Background hover band (no corner radius — menus use flat rows).
        if bg.a() > 0 {
            painter.rect_filled(rect, egui::CornerRadius::ZERO, bg);
        }

        // ── Resolve foreground color ──────────────────────────────────────────
        let mut fg = item.tint.unwrap_or_else(|| theme.text());
        if !item.enabled {
            // Dim to DISABLED_ALPHA.
            fg = Color32::from_rgba_premultiplied(
                fg.r(), fg.g(), fg.b(),
                ((fg.a() as f32) * DISABLED_ALPHA) as u8,
            );
        }

        let dim_fg = st::color_alpha(theme.dim(), if item.enabled { 160 } else { 80 });
        let cy = rect.center().y;

        // ── Left side: check-mark or icon, then label ──────────────────────────
        let mut x = rect.left() + PAD_X;

        // Leading check mark (selected state). Reserves a glyph slot even when
        // not selected so label text is always at the same x position.
        let check_slot_w = CHECK_FONT + ICON_GAP;
        if item.selected {
            painter.text(
                Pos2::new(x, cy),
                egui::Align2::LEFT_CENTER,
                Icon::CHECK,
                FontId::proportional(CHECK_FONT),
                fg,
            );
        }
        x += check_slot_w;

        // Leading icon (optional).
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

        // Label.
        if !item.label.is_empty() {
            painter.text(
                Pos2::new(x, cy),
                egui::Align2::LEFT_CENTER,
                &item.label,
                FontId::proportional(LABEL_FONT),
                fg,
            );
        }

        // ── Right side: shortcut hint and/or submenu arrow ────────────────────
        let mut rx = rect.right() - PAD_X;

        // Submenu arrow ▸.
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

        // Shortcut hint.
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

    // Suppress hover bg animation noise when disabled.
    if !item.enabled {
        let _ = Rect::NOTHING;
    }

    response
}
