//! Design-system menu item — the canonical primitive for every dropdown row.
//!
//! `MenuItem` is a thin, themed wrapper over `egui::Button`, purpose-built for
//! `egui::context_menu` / `menu_button` closures. Because it renders through
//! egui's native menu-button path it is pixel-consistent with `ui.menu_button`
//! triggers and any remaining native menu items — the same hover treatment
//! (toolbar-border fill + accent-tinted border, from the themed
//! `widgets.hovered` visuals), the same left-aligned content-sized row, the
//! same metrics. It adds two things on top: a uniform `font_sm()` label size
//! and a pointing-hand cursor on hover.
//!
//! ## Builder methods
//! - `.icon(Icon::*)` — leading glyph, glued into the label
//! - `.tint(Color32)` — colours the label (bull / bear / accent / warn)
//! - `.enabled(bool)` — disabled rows: egui-dimmed, non-interactive, no pointer cursor
//! - `.shortcut(str)` — right-aligned dim hint (egui `Button::shortcut_text`)
//! - `.submenu(bool)` — trailing ▸ glyph; does NOT open a submenu (use `ui.menu_button`)
//! - `.selected(bool)` — leading ✓ check-mark
//!
//! ## Leaf vs. submenu rows
//! - `.show(ui, theme)` — a leaf row (a clickable action).
//! - `.show_menu(ui, theme, |ui| { ... })` — an expandable row that opens a
//!   nested flyout. Both are `MenuItem`, so a whole menu is written in one
//!   vocabulary.
//!
//! ## When to use a different widget
//! - `ui_kit::Button` everywhere outside a menu context.
//! - `ui.label(...)` + `ui.separator()` for section headers / dividers inside menus.

use egui::{Button, Color32, CursorIcon, Response, RichText, Ui};

use super::theme::ComponentTheme;
use crate::chart::renderer::ui::style as st;
use crate::ui_kit::icons::Icon;

/// Builder for a single menu-item row.
///
/// Construct via [`MenuItem::new`], chain builder methods, then call
/// [`MenuItem::show`]; the returned [`Response`] carries `.clicked()` for
/// action dispatch.
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

    /// Optional leading Phosphor Bold icon glyph (e.g. `Icon::TRASH`), glued
    /// into the label string.
    pub fn icon(mut self, glyph: &'a str) -> Self {
        self.icon = Some(glyph);
        self
    }

    /// Tint the label with `color`. Use theme semantics: `t.bull` for
    /// buy/positive, `t.bear` for destructive, `t.accent` for neutral CTAs.
    pub fn tint(mut self, color: Color32) -> Self {
        self.tint = Some(color);
        self
    }

    /// When `false`, the item is dimmed and non-interactive (no pointer cursor,
    /// no click) — via egui's native disabled treatment.
    pub fn enabled(mut self, v: bool) -> Self {
        self.enabled = v;
        self
    }

    /// Right-aligned shortcut hint text (e.g. `"Ctrl+Z"`), rendered dim.
    pub fn shortcut(mut self, hint: impl Into<String>) -> Self {
        self.shortcut = Some(hint.into());
        self
    }

    /// When `true`, a trailing ▸ glyph is appended to signal a nested submenu.
    /// Does NOT open the submenu itself — pair with `ui.menu_button(...)`.
    pub fn submenu(mut self, v: bool) -> Self {
        self.submenu = v;
        self
    }

    /// When `true`, a leading ✓ check-mark is rendered (active/selected state).
    pub fn selected(mut self, v: bool) -> Self {
        self.selected = v;
        self
    }

    /// Render the menu item into `ui`. Returns the interaction [`Response`];
    /// check `.clicked()` to dispatch the action.
    pub fn show(self, ui: &mut Ui, theme: &dyn ComponentTheme) -> Response {
        // Compose the label in one string — optional leading check / icon,
        // the text, then an optional trailing submenu caret — so it renders
        // exactly as a native `ui.button(...)` menu item (the v0.9.7 form).
        let mut text = String::new();
        if self.selected {
            text.push_str(Icon::CHECK);
            text.push(' ');
        }
        if let Some(glyph) = self.icon {
            text.push_str(glyph);
            text.push(' ');
        }
        text.push_str(&self.label);
        if self.submenu {
            text.push(' ');
            text.push_str(Icon::CARET_RIGHT);
        }

        let mut rt = RichText::new(text).size(st::font_sm());
        if let Some(tint) = self.tint {
            rt = rt.color(tint);
        }

        let mut btn = Button::new(rt);
        if let Some(ref hint) = self.shortcut {
            btn = btn.shortcut_text(
                RichText::new(hint.clone())
                    .size(st::font_sm())
                    .color(st::color_alpha(theme.dim(), 160)),
            );
        }

        // `ui.add_enabled` routes through egui's native menu-button rendering —
        // same hover band, border, font metrics and left alignment as every
        // other native menu item, so the whole menu stays consistent.
        let resp = ui.add_enabled(self.enabled, btn);
        if self.enabled && resp.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        resp
    }

    /// Render this row as a **submenu trigger** that opens a nested flyout.
    ///
    /// egui owns the popup, the hover-to-open behaviour, and the trailing `⏵`
    /// caret; this styles the trigger label to match a leaf [`MenuItem::show`]
    /// row (icon glued in, `font_sm()`, optional tint). Use it for every
    /// expandable row so a menu is written entirely in `MenuItem` terms —
    /// `.show()` for leaves, `.show_menu()` for submenus.
    ///
    /// `body` fills the nested submenu; its return value is surfaced via the
    /// `InnerResponse`, exactly like `ui.menu_button`.
    pub fn show_menu<R>(
        self,
        ui: &mut Ui,
        _theme: &dyn ComponentTheme,
        body: impl FnOnce(&mut Ui) -> R,
    ) -> egui::InnerResponse<Option<R>> {
        let mut text = String::new();
        if let Some(glyph) = self.icon {
            text.push_str(glyph);
            text.push(' ');
        }
        text.push_str(&self.label);
        let mut rt = RichText::new(text).size(st::font_sm());
        if let Some(tint) = self.tint {
            rt = rt.color(tint);
        }
        ui.menu_button(rt, body)
    }
}
