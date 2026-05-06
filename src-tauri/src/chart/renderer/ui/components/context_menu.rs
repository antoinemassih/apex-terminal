//! Unified context-menu / popup-menu builder system.
//!
//! Coexists with `widgets/menus.rs` (which is for menu-bar triggers and
//! flat menu rows). This module focuses on right-click / floating popup
//! menus with sections, dividers, checks, radios, submenus, and danger
//! styling — visually matching the existing `response.context_menu(...)`
//! call sites used throughout the codebase.
//!
//! Wave 5 will migrate call sites; for now this file is additive only.
//!
//! Example:
//! ```ignore
//! use crate::chart_renderer::ui::widgets::context_menu::*;
//! let mut pinned = false;
//! ContextMenu::new(t)
//!     .pos(click_pos)
//!     .show(ui, |menu| {
//!         menu.add_section("Watchlist");
//!         if menu.add(MenuItem::new("Add to chart")).clicked() {
//!             // ...
//!         }
//!         menu.add_divider();
//!         if menu.add(CheckMenuItem::new("Pin", &mut pinned)).clicked() {}
//!         if menu.add(DangerMenuItem::new("Delete")).clicked() {}
//!     });
//! ```

#![allow(dead_code, unused_imports)]

use egui::{Align2, Color32, FontId, Id, Pos2, Response, RichText, Sense, Stroke, Ui, Vec2, Widget};

use super::super::style::*;
use super::frames_widget::{PopupFrame, BorderAlpha};
use super::super::super::gpu::Theme;

// ─── Shared theme snapshot ───────────────────────────────────────────────────

/// Lightweight theme snapshot copied out of `&Theme` so `MenuBuilder` can be
/// passed to user closures without lifetime gymnastics.
#[derive(Copy, Clone)]
pub struct MenuTheme {
    pub accent: Color32,
    pub dim: Color32,
    pub bg: Color32,
    pub fg: Color32,
    pub danger: Color32,
}

impl MenuTheme {
    pub fn from_theme(t: &Theme) -> Self {
        Self {
            accent: t.accent,
            dim: t.dim,
            bg: t.bg,
            fg: t.text,
            danger: t.bear,
        }
    }
}

// ─── ContextMenu builder ─────────────────────────────────────────────────────

/// Anchor for a `ContextMenu`. Either an absolute screen position (typical
/// for right-click handlers that already know where the pointer was) or
/// "below a Response" (for click-to-open dropdowns).
#[derive(Copy, Clone)]
pub enum MenuAnchor {
    Pos(Pos2),
    BelowRect(egui::Rect),
}

/// Builder for a floating popup/context menu.
///
/// Drop-in replacement target for ad-hoc `response.context_menu(...)` blocks.
/// Today this opens an `egui::Area` painted with the chart theme so the
/// visuals match the rest of the trading UI.
#[must_use = "ContextMenu must be terminated with `.show(ui, |menu| { ... })`"]
pub struct ContextMenu {
    id: Id,
    anchor: Option<MenuAnchor>,
    theme: MenuTheme,
    min_width: f32,
}

impl ContextMenu {
    pub fn new(t: &Theme) -> Self {
        Self {
            id: Id::new("apex_context_menu"),
            anchor: None,
            theme: MenuTheme::from_theme(t),
            min_width: 160.0,
        }
    }

    pub fn id(mut self, id: impl std::hash::Hash) -> Self {
        self.id = Id::new(id);
        self
    }

    pub fn pos(mut self, p: Pos2) -> Self {
        self.anchor = Some(MenuAnchor::Pos(p));
        self
    }

    pub fn below(mut self, r: &Response) -> Self {
        self.anchor = Some(MenuAnchor::BelowRect(r.rect));
        self
    }

    pub fn min_width(mut self, w: f32) -> Self {
        self.min_width = w;
        self
    }

    /// Paint the menu and run `body` to populate rows. Returns whatever the
    /// body returns (typically an `Option<Action>` for the caller to act on).
    pub fn show<R, F>(self, ui: &mut Ui, body: F) -> Option<R>
    where
        F: FnOnce(&mut MenuBuilder<'_>) -> R,
    {
        let pos = match self.anchor {
            Some(MenuAnchor::Pos(p)) => p,
            Some(MenuAnchor::BelowRect(r)) => egui::pos2(r.left(), r.bottom() + gap_xs()),
            None => ui.cursor().min,
        };
        let theme = self.theme;
        let min_width = self.min_width;
        let id = self.id;

        let mut out: Option<R> = None;
        egui::Area::new(id)
            .order(egui::Order::Foreground)
            .fixed_pos(pos)
            .show(ui.ctx(), |ui| {
                let frame = PopupFrame::new()
                    .colors(theme.bg, theme.dim)
                    .ctx(ui.ctx())
                    .border_alpha(BorderAlpha::Line)
                    .corner_radius(radius_sm())
                    .inner_margin(egui::Margin::symmetric(gap_xs() as i8, gap_xs() as i8))
                    .build();
                frame.show(ui, |ui| {
                    ui.set_min_width(min_width);
                    let mut mb = MenuBuilder { ui, theme };
                    out = Some(body(&mut mb));
                });
            });
        out
    }
}

// ─── MenuBuilder — passed into the body closure ─────────────────────────────

/// Wraps the popup `Ui` and exposes a uniform `add(...)` pipeline so all menu
/// rows pick up consistent padding, hover, and theme colours.
pub struct MenuBuilder<'a> {
    pub ui: &'a mut Ui,
    pub theme: MenuTheme,
}

impl<'a> MenuBuilder<'a> {
    /// Add any [`MenuRow`] (the trait implemented by all builders below).
    pub fn add<R: MenuRow>(&mut self, row: R) -> Response {
        row.show(self.ui, &self.theme)
    }

    /// Convenience: add a section header.
    pub fn add_section(&mut self, label: &str) -> Response {
        self.add(MenuSection::new(label))
    }

    /// Convenience: add a divider between sections.
    pub fn add_divider(&mut self) -> Response {
        self.add(MenuDivider)
    }
}

// ─── MenuRow trait ───────────────────────────────────────────────────────────

/// All row builders implement this. Lets `MenuBuilder::add` accept any of
/// them uniformly while each builder controls its own painting.
pub trait MenuRow {
    fn show(self, ui: &mut Ui, theme: &MenuTheme) -> Response;
}

// ─── MenuSection ─────────────────────────────────────────────────────────────

/// Small dim header label used to group items inside a single popup.
pub struct MenuSection<'a> {
    label: &'a str,
}

impl<'a> MenuSection<'a> {
    pub fn new(label: &'a str) -> Self { Self { label } }
}

impl<'a> MenuRow for MenuSection<'a> {
    fn show(self, ui: &mut Ui, theme: &MenuTheme) -> Response {
        let resp = ui.horizontal(|ui| {
            ui.add_space(gap_sm());
            ui.label(
                RichText::new(self.label.to_uppercase())
                    .size(font_xs())
                    .color(color_alpha(theme.dim, alpha_strong())),
            )
        }).response;
        ui.add_space(gap_xs());
        resp
    }
}

// ─── MenuDivider ─────────────────────────────────────────────────────────────

/// Horizontal rule between sections of a popup.
pub struct MenuDivider;

impl MenuRow for MenuDivider {
    fn show(self, ui: &mut Ui, theme: &MenuTheme) -> Response {
        let (rect, resp) = ui.allocate_exact_size(
            egui::vec2(ui.available_width(), 1.0),
            Sense::hover(),
        );
        ui.painter().line_segment(
            [
                egui::pos2(rect.left() + gap_sm(), rect.center().y),
                egui::pos2(rect.right() - gap_sm(), rect.center().y),
            ],
            Stroke::new(stroke_hair(), color_alpha(theme.dim, alpha_line())),
        );
        ui.add_space(gap_xs());
        resp
    }
}

// ─── Internal row painter ───────────────────────────────────────────────────

fn paint_row(
    ui: &mut Ui,
    theme: &MenuTheme,
    label: &str,
    fg: Color32,
    icon: Option<&str>,
    shortcut: Option<&str>,
    suffix: Option<&str>,
    leading_check: Option<bool>,
) -> Response {
    let prev_pad = ui.spacing().button_padding;
    ui.spacing_mut().button_padding = egui::vec2(gap_lg(), gap_xs());

    let mut display = String::new();
    if let Some(checked) = leading_check {
        display.push_str(if checked { "\u{2713} " } else { "  " });
    }
    if let Some(ic) = icon {
        display.push_str(ic);
        display.push(' ');
    }
    display.push_str(label);
    if let Some(sx) = suffix {
        display.push(' ');
        display.push_str(sx);
    }

    let resp = ui
        .horizontal(|ui| {
            let r = ui.add(
                egui::Button::new(RichText::new(&display).size(font_sm()).color(fg))
                    .fill(Color32::TRANSPARENT)
                    .stroke(Stroke::NONE)
                    .min_size(egui::vec2(ui.available_width().max(80.0), 20.0)),
            );
            if let Some(sc) = shortcut {
                let sc_color = color_alpha(theme.dim, alpha_muted());
                let max_x = r.rect.right() - gap_sm();
                let y = r.rect.center().y;
                ui.painter().text(
                    egui::pos2(max_x, y),
                    Align2::RIGHT_CENTER,
                    sc,
                    FontId::monospace(font_xs()),
                    sc_color,
                );
            }
            r
        })
        .inner;

    ui.spacing_mut().button_padding = prev_pad;

    if resp.hovered() && !crate::design_tokens::is_inspect_mode() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        ui.painter()
            .rect_filled(resp.rect, radius_sm(), color_alpha(theme.accent, alpha_ghost()));
    }
    resp
}

// ─── MenuItem ────────────────────────────────────────────────────────────────

/// Plain clickable menu row.
pub struct MenuItem<'a> {
    label: &'a str,
}

impl<'a> MenuItem<'a> {
    pub fn new(label: &'a str) -> Self { Self { label } }
}

impl<'a> MenuRow for MenuItem<'a> {
    fn show(self, ui: &mut Ui, theme: &MenuTheme) -> Response {
        paint_row(ui, theme, self.label, theme.dim, None, None, None, None)
    }
}

// ─── MenuItemWithShortcut ───────────────────────────────────────────────────

/// Menu row with a right-aligned monospace keybind chip.
pub struct MenuItemWithShortcut<'a> {
    label: &'a str,
    shortcut: &'a str,
}

impl<'a> MenuItemWithShortcut<'a> {
    pub fn new(label: &'a str, shortcut: &'a str) -> Self { Self { label, shortcut } }
}

impl<'a> MenuRow for MenuItemWithShortcut<'a> {
    fn show(self, ui: &mut Ui, theme: &MenuTheme) -> Response {
        paint_row(ui, theme, self.label, theme.dim, None, Some(self.shortcut), None, None)
    }
}

// ─── MenuItemWithIcon ───────────────────────────────────────────────────────

/// Menu row with a leading text/glyph icon before the label.
pub struct MenuItemWithIcon<'a> {
    label: &'a str,
    icon: &'a str,
}

impl<'a> MenuItemWithIcon<'a> {
    pub fn new(label: &'a str, icon: &'a str) -> Self { Self { label, icon } }
}

impl<'a> MenuRow for MenuItemWithIcon<'a> {
    fn show(self, ui: &mut Ui, theme: &MenuTheme) -> Response {
        paint_row(ui, theme, self.label, theme.dim, Some(self.icon), None, None, None)
    }
}

// ─── CheckMenuItem ───────────────────────────────────────────────────────────

/// Toggleable menu row that flips a `&mut bool` when clicked. Renders a leading
/// check glyph on the active state.
pub struct CheckMenuItem<'a> {
    label: &'a str,
    checked: &'a mut bool,
}

impl<'a> CheckMenuItem<'a> {
    pub fn new(label: &'a str, checked: &'a mut bool) -> Self { Self { label, checked } }
}

impl<'a> MenuRow for CheckMenuItem<'a> {
    fn show(self, ui: &mut Ui, theme: &MenuTheme) -> Response {
        let resp = paint_row(
            ui,
            theme,
            self.label,
            theme.dim,
            None,
            None,
            None,
            Some(*self.checked),
        );
        if resp.clicked() {
            *self.checked = !*self.checked;
        }
        resp
    }
}

// ─── RadioMenuItem<T> ───────────────────────────────────────────────────────

/// Radio-style row — when clicked, sets `*current = value`.
pub struct RadioMenuItem<'a, T: PartialEq + Clone> {
    label: &'a str,
    value: T,
    current: &'a mut T,
}

impl<'a, T: PartialEq + Clone> RadioMenuItem<'a, T> {
    pub fn new(label: &'a str, value: T, current: &'a mut T) -> Self {
        Self { label, value, current }
    }
}

impl<'a, T: PartialEq + Clone> MenuRow for RadioMenuItem<'a, T> {
    fn show(self, ui: &mut Ui, theme: &MenuTheme) -> Response {
        let selected = *self.current == self.value;
        let resp = paint_row(
            ui,
            theme,
            self.label,
            theme.dim,
            None,
            None,
            None,
            Some(selected),
        );
        if resp.clicked() {
            *self.current = self.value.clone();
        }
        resp
    }
}

// ─── Submenu ─────────────────────────────────────────────────────────────────

/// Nested menu — renders a row with a trailing chevron, and on hover/click
/// opens a cascading popup populated by `body`.
pub struct Submenu<'a, F>
where
    F: FnOnce(&mut MenuBuilder<'_>),
{
    label: &'a str,
    body: F,
}

impl<'a, F> Submenu<'a, F>
where
    F: FnOnce(&mut MenuBuilder<'_>),
{
    pub fn new(label: &'a str, body: F) -> Self { Self { label, body } }
}

impl<'a, F> MenuRow for Submenu<'a, F>
where
    F: FnOnce(&mut MenuBuilder<'_>),
{
    fn show(self, ui: &mut Ui, theme: &MenuTheme) -> Response {
        let resp = paint_row(
            ui,
            theme,
            self.label,
            theme.dim,
            None,
            None,
            Some("\u{25B8}"),
            None,
        );

        let popup_id = ui.id().with(("submenu", self.label));
        let open_mem = ui.memory(|m| m.data.get_temp::<bool>(popup_id).unwrap_or(false));
        let want_open = open_mem || resp.hovered() || resp.clicked();
        ui.memory_mut(|m| m.data.insert_temp(popup_id, want_open));

        if want_open {
            let anchor = egui::pos2(resp.rect.right() + gap_xs(), resp.rect.top());
            egui::Area::new(popup_id)
                .order(egui::Order::Foreground)
                .fixed_pos(anchor)
                .show(ui.ctx(), |ui| {
                    let frame = PopupFrame::new()
                        .colors(theme.bg, theme.dim)
                        .ctx(ui.ctx())
                        .border_alpha(BorderAlpha::Line)
                        .corner_radius(radius_sm())
                        .inner_margin(egui::Margin::symmetric(gap_xs() as i8, gap_xs() as i8))
                        .build();
                    frame.show(ui, |ui| {
                        ui.set_min_width(140.0);
                        let mut mb = MenuBuilder { ui, theme: *theme };
                        (self.body)(&mut mb);
                    });
                });
        }

        resp
    }
}

// ─── DangerMenuItem ─────────────────────────────────────────────────────────

/// Destructive action — rendered with a red foreground (delete, remove, etc.).
pub struct DangerMenuItem<'a> {
    label: &'a str,
    icon: Option<&'a str>,
}

impl<'a> DangerMenuItem<'a> {
    pub fn new(label: &'a str) -> Self { Self { label, icon: None } }
    pub fn icon(mut self, icon: &'a str) -> Self { self.icon = Some(icon); self }
}

impl<'a> MenuRow for DangerMenuItem<'a> {
    fn show(self, ui: &mut Ui, theme: &MenuTheme) -> Response {
        paint_row(ui, theme, self.label, theme.danger, self.icon, None, None, None)
    }
}
