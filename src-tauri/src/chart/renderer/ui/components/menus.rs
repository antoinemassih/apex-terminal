//! Builder + impl Widget primitives — menus family.
//! See ui/widgets/mod.rs for the rationale.

#![allow(dead_code, unused_imports)]

use egui::{Color32, Response, RichText, Stroke, Ui, Widget};
use super::super::style::*;
use super::semantic_label::{SemanticLabel, LabelVariant};

#[inline(always)]
fn ft() -> &'static super::super::super::gpu::Theme { &crate::chart_renderer::gpu::THEMES[0] }

// ─── MenuTrigger ─────────────────────────────────────────────────────────────

/// Builder for a menu-bar trigger button. Replaces `components_extra::menu_trigger(...)`.
///
/// ```ignore
/// ui.add(MenuTrigger::new("File").open(true).theme(t));
/// ```
#[must_use = "MenuTrigger must be added with `ui.add(...)` to render"]
pub struct MenuTrigger<'a> {
    label: &'a str,
    open: bool,
    accent: Color32,
    dim: Color32,
}

impl<'a> MenuTrigger<'a> {
    pub fn new(label: &'a str) -> Self {
        Self {
            label,
            open: false,
            accent: ft().accent,
            dim: ft().dim,
        }
    }
    pub fn open(mut self, o: bool) -> Self { self.open = o; self }
    pub fn theme(mut self, t: &super::super::super::gpu::Theme) -> Self {
        self.accent = t.accent;
        self.dim = t.dim;
        self
    }
}

impl<'a> Widget for MenuTrigger<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        let fg = if self.open { self.accent } else { self.dim };
        let bg = if self.open { color_alpha(self.accent, alpha_soft()) } else { Color32::TRANSPARENT };
        let border = if self.open { color_alpha(self.accent, alpha_muted()) } else { Color32::TRANSPARENT };
        let display = format!("{} \u{25BE}", self.label);
        let prev_pad = ui.spacing().button_padding;
        ui.spacing_mut().button_padding = egui::vec2(gap_md(), gap_sm());
        let resp = ui.add(
            egui::Button::new(
                SemanticLabel::new(display, LabelVariant::MenuItem).monospace(false).color(fg).into_rich_text()
            )
                .fill(bg)
                .stroke(Stroke::new(stroke_thin(), border))
                .corner_radius(radius_sm())
                .min_size(egui::vec2(0.0, 20.0)),
        );
        ui.spacing_mut().button_padding = prev_pad;
        if resp.hovered() && !self.open && !crate::design_tokens::is_inspect_mode() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            ui.painter().rect_filled(resp.rect, radius_sm(), color_alpha(self.accent, alpha_ghost()));
        }
        resp
    }
}

// ─── MenuItem ────────────────────────────────────────────────────────────────

/// Variant for a [`MenuItem`] — controls prefix/suffix decoration and separator rendering.
#[derive(Debug, Clone, PartialEq)]
pub enum MenuItemVariant {
    Default,
    Submenu,
    Checkbox(bool),
    Separator,
}

/// Builder for a single menu row. Replaces `components_extra::menu_item(...)`.
///
/// ```ignore
/// ui.add(MenuItem::new("Copy").shortcut_str("⌘C").theme(t));
/// ui.add(MenuItem::new("").separator());
/// ```
#[must_use = "MenuItem must be added with `ui.add(...)` to render"]
pub struct MenuItem<'a> {
    label: &'a str,
    variant: MenuItemVariant,
    shortcut: Option<&'a str>,
    accent: Color32,
    dim: Color32,
}

impl<'a> MenuItem<'a> {
    pub fn new(label: &'a str) -> Self {
        Self {
            label,
            variant: MenuItemVariant::Default,
            shortcut: None,
            accent: ft().accent,
            dim: ft().dim,
        }
    }
    pub fn variant(mut self, v: MenuItemVariant) -> Self { self.variant = v; self }
    pub fn default(self) -> Self { self.variant(MenuItemVariant::Default) }
    pub fn submenu(self) -> Self { self.variant(MenuItemVariant::Submenu) }
    pub fn checkbox(self, checked: bool) -> Self { self.variant(MenuItemVariant::Checkbox(checked)) }
    pub fn separator(self) -> Self { self.variant(MenuItemVariant::Separator) }
    pub fn shortcut(mut self, sc: Option<&'a str>) -> Self { self.shortcut = sc; self }
    pub fn shortcut_str(mut self, sc: &'a str) -> Self { self.shortcut = Some(sc); self }
    pub fn theme(mut self, t: &'a super::super::super::gpu::Theme) -> Self {
        self.accent = t.accent;
        self.dim = t.dim;
        self
    }
}

impl<'a> Widget for MenuItem<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        if self.variant == MenuItemVariant::Separator {
            let (sep_rect, resp) = ui.allocate_exact_size(
                egui::vec2(ui.available_width(), 1.0),
                egui::Sense::hover(),
            );
            ui.painter().line_segment(
                [
                    egui::pos2(sep_rect.left() + gap_sm(), sep_rect.center().y),
                    egui::pos2(sep_rect.right() - gap_sm(), sep_rect.center().y),
                ],
                Stroke::new(stroke_hair(), color_alpha(self.dim, alpha_line())),
            );
            ui.add_space(gap_xs());
            return resp;
        }
        let prefix = match &self.variant {
            MenuItemVariant::Checkbox(true)  => "\u{2713} ",
            MenuItemVariant::Checkbox(false) => "  ",
            _ => "",
        };
        let suffix = match &self.variant {
            MenuItemVariant::Submenu => " \u{25B8}",
            _ => "",
        };
        let display = format!("{}{}{}", prefix, self.label, suffix);
        let fg = self.dim;
        let prev_pad = ui.spacing().button_padding;
        ui.spacing_mut().button_padding = egui::vec2(gap_lg(), gap_xs());
        let resp = ui.horizontal(|ui| {
            let r = ui.add(
                egui::Button::new(
                    SemanticLabel::new(&display, LabelVariant::MenuItem).monospace(false).color(fg).into_rich_text()
                )
                    .fill(Color32::TRANSPARENT)
                    .stroke(Stroke::NONE)
                    .min_size(egui::vec2(ui.available_width().max(80.0), 20.0)),
            );
            if let Some(sc) = self.shortcut {
                let sc_color = color_alpha(self.dim, alpha_muted());
                let max_x = r.rect.right() - gap_sm();
                let y = r.rect.center().y;
                ui.painter().text(
                    egui::pos2(max_x, y),
                    egui::Align2::RIGHT_CENTER,
                    sc,
                    egui::FontId::monospace(font_xs()),
                    sc_color,
                );
            }
            r
        }).inner;
        ui.spacing_mut().button_padding = prev_pad;
        if resp.hovered() && !crate::design_tokens::is_inspect_mode() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            ui.painter().rect_filled(resp.rect, radius_sm(), color_alpha(self.accent, alpha_ghost()));
        }
        resp
    }
}

// ─── SidePaneAction ──────────────────────────────────────────────────────────

/// Builder for a side-pane action button. Replaces `components_extra::side_pane_action_btn(...)`.
///
/// ```ignore
/// ui.add(SidePaneAction::new("Add Alert").icon_str("🔔").theme(t));
/// ```
#[must_use = "SidePaneAction must be added with `ui.add(...)` to render"]
pub struct SidePaneAction<'a> {
    label: &'a str,
    icon: Option<&'a str>,
    accent: Color32,
    dim: Color32,
}

impl<'a> SidePaneAction<'a> {
    pub fn new(label: &'a str) -> Self {
        Self {
            label,
            icon: None,
            accent: ft().accent,
            dim: ft().dim,
        }
    }
    pub fn icon(mut self, ic: Option<&'a str>) -> Self { self.icon = ic; self }
    pub fn icon_str(mut self, ic: &'a str) -> Self { self.icon = Some(ic); self }
    pub fn theme(mut self, t: &super::super::super::gpu::Theme) -> Self {
        self.accent = t.accent;
        self.dim = t.dim;
        self
    }
}

impl<'a> Widget for SidePaneAction<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        let fg = self.accent;
        let bg = color_alpha(self.accent, alpha_soft());
        let border = color_alpha(self.accent, alpha_dim());
        let display = match self.icon {
            Some(ic) => format!("{} {}", ic, self.label),
            None => self.label.to_owned(),
        };
        let prev_pad = ui.spacing().button_padding;
        ui.spacing_mut().button_padding = egui::vec2(gap_lg(), gap_xs());
        let resp = ui.add(
            egui::Button::new(
                SemanticLabel::new(display, LabelVariant::MenuItem).monospace(false).strong(true).color(fg).into_rich_text()
            )
                .fill(bg)
                .stroke(Stroke::new(stroke_thin(), border))
                .corner_radius(radius_sm())
                .min_size(egui::vec2(0.0, 22.0)),
        );
        ui.spacing_mut().button_padding = prev_pad;
        if resp.hovered() && !crate::design_tokens::is_inspect_mode() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            ui.painter().rect_filled(resp.rect, radius_sm(), color_alpha(self.accent, alpha_faint()));
        }
        resp
    }
}
