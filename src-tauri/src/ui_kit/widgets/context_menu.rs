//! Context menu — re-homed from `chart::renderer::ui::components::context_menu`.
//!
//! Floating right-click / popup menu builder with sections, dividers,
//! checks, radios, submenus, and danger styling. The original module's
//! design notes apply unchanged. Migration into ui_kit:
//!   * `ContextMenu::new` accepts any `&T: ComponentTheme`.
//!   * Open animation: alpha fade + tiny scale-in over `motion::FAST`,
//!     keyed on the menu id so concurrent menus animate independently.
//!   * Public types (`MenuTheme`, `MenuBuilder`, `MenuItem`, etc.)
//!     are unchanged so callers compile via the back-compat re-export.

#![allow(dead_code, unused_imports)]

use egui::{Align2, Color32, FontId, Id, Pos2, Response, RichText, Sense, Stroke, Ui, Vec2, Widget};

use super::theme::ComponentTheme;
use super::motion;

use crate::chart_renderer::ui::components::frames_widget::{BorderAlpha, PopupFrame};
use crate::chart_renderer::ui::style::*;

type Theme = crate::chart_renderer::gpu::Theme;

// ─── Shared theme snapshot ───────────────────────────────────────────────────

/// Lightweight theme snapshot copied out of a `ComponentTheme` so
/// `MenuBuilder` can be passed to user closures without lifetime gymnastics.
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
    pub fn from_component<T: ComponentTheme + ?Sized>(t: &T) -> Self {
        Self {
            accent: t.accent(),
            dim: t.dim(),
            bg: t.bg(),
            fg: t.text(),
            danger: t.bear(),
        }
    }
}

// ─── ContextMenu builder ─────────────────────────────────────────────────────

#[derive(Copy, Clone)]
pub enum MenuAnchor {
    Pos(Pos2),
    BelowRect(egui::Rect),
}

#[must_use = "ContextMenu must be terminated with `.show(ui, |menu| { ... })`"]
pub struct ContextMenu {
    id: Id,
    anchor: Option<MenuAnchor>,
    theme: MenuTheme,
    min_width: f32,
}

impl ContextMenu {
    /// Construct from any `ComponentTheme`. Matches legacy `ContextMenu::new(&Theme)`.
    pub fn new<T: ComponentTheme + ?Sized>(t: &T) -> Self {
        Self {
            id: Id::new("apex_context_menu"),
            anchor: None,
            theme: MenuTheme::from_component(t),
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

    /// Paint the menu and run `body` to populate rows.
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

        // Open animation: alpha 0->1 over FAST. Origin = anchor pos.
        let appear_t = motion::ease_bool(ui.ctx(), id.with("apex_ctx_anim"), true, motion::FAST);

        let mut out: Option<R> = None;
        let size_id = id.with("apex_ctx_size");
        let prior_size: Vec2 = ui
            .ctx()
            .memory(|m| m.data.get_temp(size_id))
            .unwrap_or(Vec2::new(min_width, 32.0));
        let area_resp = egui::Area::new(id)
            .order(egui::Order::Foreground)
            .fixed_pos(pos)
            .show(ui.ctx(), |ui| {
                ui.set_opacity(appear_t);
                let shadow_rect = egui::Rect::from_min_size(pos, prior_size);
                super::paint_shadow_gpu(
                    ui.painter(),
                    shadow_rect,
                    super::ShadowSpec::md(),
                );
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
        let measured = area_resp.response.rect.size();
        if measured.x > 0.0 && measured.y > 0.0 {
            ui.ctx().memory_mut(|m| m.data.insert_temp(size_id, measured));
        }
        out
    }
}

// ─── MenuBuilder — passed into the body closure ─────────────────────────────

pub struct MenuBuilder<'a> {
    pub ui: &'a mut Ui,
    pub theme: MenuTheme,
}

impl<'a> MenuBuilder<'a> {
    pub fn add<R: MenuRow>(&mut self, row: R) -> Response {
        row.show(self.ui, &self.theme)
    }
    pub fn add_section(&mut self, label: &str) -> Response {
        self.add(MenuSection::new(label))
    }
    pub fn add_divider(&mut self) -> Response {
        self.add(MenuDivider)
    }
}

// ─── MenuRow trait ───────────────────────────────────────────────────────────

pub trait MenuRow {
    fn show(self, ui: &mut Ui, theme: &MenuTheme) -> Response;
}

// ─── MenuSection ─────────────────────────────────────────────────────────────

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
            let appear_t = motion::ease_bool(ui.ctx(), popup_id.with("anim"), true, motion::FAST);
            let anchor = egui::pos2(resp.rect.right() + gap_xs(), resp.rect.top());
            egui::Area::new(popup_id)
                .order(egui::Order::Foreground)
                .fixed_pos(anchor)
                .show(ui.ctx(), |ui| {
                    ui.set_opacity(appear_t);
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
