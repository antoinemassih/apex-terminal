//! Builder + impl Widget primitives — menus family.
//! See ui/widgets/mod.rs for the rationale.

#![allow(dead_code, unused_imports)]

use egui::{Color32, Response, RichText, Stroke, Ui, Widget};
use super::super::style::*;
use super::semantic_label::{SemanticLabel, LabelVariant};
use crate::ui_kit::widgets::Button as KitButton;
use crate::ui_kit::widgets::tokens::{Variant as KitVariant, Size as KitSize};

#[inline(always)]
fn ambient(ctx: &egui::Context) -> super::super::super::gpu::Theme {
    crate::chart_renderer::theme_impl::active_theme(ctx)
}

// ─── MenuTrigger ─────────────────────────────────────────────────────────────

/// Builder for a menu-bar trigger button. Replaces `components_extra::menu_trigger(...)`.
///
/// ```ignore
/// ui.add(MenuTrigger::new("File").open(true).theme(t));
/// ```

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
    accent: Option<Color32>,
    dim: Option<Color32>,
}

impl<'a> MenuItem<'a> {
    pub fn new(label: &'a str) -> Self {
        Self {
            label,
            variant: MenuItemVariant::Default,
            shortcut: None,
            accent: None,
            dim: None,
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
        self.accent = Some(t.accent);
        self.dim = Some(t.dim);
        self
    }
}

impl<'a> Widget for MenuItem<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        let amb = ambient(ui.ctx());
        let accent = self.accent.unwrap_or(amb.accent);
        let dim = self.dim.unwrap_or(amb.dim);
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
                Stroke::new(stroke_hair(), color_alpha(dim, alpha_line())),
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
        let resp = ui.horizontal(|ui| {
            let min_w = ui.available_width().max(80.0);
            let r = KitButton::new(display.as_str()).variant(KitVariant::Ghost).size(KitSize::Sm)
                .fg(dim).min_size(egui::vec2(min_w, row_height_compact()))
                .full_width(true).show(ui, &amb);
            if let Some(sc) = self.shortcut {
                let sc_color = color_alpha(dim, alpha_muted());
                let max_x = r.rect.right() - gap_sm();
                let y = r.rect.center().y;
                ui.painter().text(
                    egui::pos2(max_x, y),
                    egui::Align2::RIGHT_CENTER,
                    sc,
                    crate::ui_kit::style::mono_xs(),
                    sc_color,
                );
            }
            r
        }).inner;
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
    accent: Option<Color32>,
    dim: Option<Color32>,
}

impl<'a> SidePaneAction<'a> {
    pub fn new(label: &'a str) -> Self {
        Self {
            label,
            icon: None,
            accent: None,
            dim: None,
        }
    }
    pub fn icon(mut self, ic: Option<&'a str>) -> Self { self.icon = ic; self }
    pub fn icon_str(mut self, ic: &'a str) -> Self { self.icon = Some(ic); self }
    pub fn theme(mut self, t: &super::super::super::gpu::Theme) -> Self {
        self.accent = Some(t.accent);
        self.dim = Some(t.dim);
        self
    }
}

impl<'a> Widget for SidePaneAction<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        let amb = ambient(ui.ctx());
        let accent = self.accent.unwrap_or(amb.accent);
        let display = match self.icon {
            Some(ic) => format!("{} {}", ic, self.label),
            None => self.label.to_owned(),
        };
        KitButton::new(display.as_str()).variant(KitVariant::Secondary).size(KitSize::Sm)
            .tint(accent).min_size(egui::vec2(0.0, row_height_default()))
            .show(ui, &amb)
    }
}
