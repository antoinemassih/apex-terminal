//! Builder + impl Widget primitives — headers family.
//! See ui/widgets/mod.rs for the rationale.

#![allow(dead_code, unused_imports)]

use egui::{Color32, Response, RichText, Sense, Stroke, Ui, Vec2, Widget};
use super::super::style::*;
use super::super::components::{section_label_widget, pane_header_bar};
use super::semantic_label::{SemanticLabel, LabelVariant};

// ─── PanelHeader ──────────────────────────────────────────────────────────────

/// Builder for a panel title-only header (no close button).
/// Wraps the title portion of `style::panel_header_sub`.
///
/// ```ignore
/// ui.add(PanelHeader::new("Positions").theme(t));
/// ```
#[must_use = "PanelHeader must be added with `ui.add(...)` to render"]
pub struct PanelHeader<'a> {
    title: &'a str,
    accent: Color32,
    dim: Color32,
}

impl<'a> PanelHeader<'a> {
    pub fn new(title: &'a str) -> Self {
        Self {
            title,
            accent: Color32::from_rgb(120, 140, 220),
            dim:    Color32::from_rgb(120, 120, 130),
        }
    }
    pub fn accent(mut self, c: Color32) -> Self { self.accent = c; self }
    pub fn dim(mut self, c: Color32) -> Self { self.dim = c; self }
    pub fn theme(self, t: &super::super::super::gpu::Theme) -> Self {
        self.accent(t.accent).dim(t.dim)
    }
}

impl<'a> Widget for PanelHeader<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        let accent = self.accent;
        let title_text = style_label_case(self.title);
        ui.horizontal(|ui| {
            ui.label(SemanticLabel::new(title_text, LabelVariant::Header).color(accent).into_rich_text());
        }).response
    }
}

// ─── PanelHeaderWithClose ────────────────────────────────────────────────────

/// Builder for a panel header with close button. Returns `true` if close clicked.
/// Mirrors `style::panel_header_sub(ui, title, None, accent, dim)`.
///
/// ```ignore
/// if PanelHeaderWithClose::new("Positions").theme(t).show(ui) { open = false; }
/// ```
#[must_use]
pub struct PanelHeaderWithClose<'a> {
    title:    &'a str,
    subtitle: Option<&'a str>,
    accent:   Color32,
    dim:      Color32,
}

impl<'a> PanelHeaderWithClose<'a> {
    pub fn new(title: &'a str) -> Self {
        Self {
            title,
            subtitle: None,
            accent: Color32::from_rgb(120, 140, 220),
            dim:    Color32::from_rgb(120, 120, 130),
        }
    }
    pub fn accent(mut self, c: Color32) -> Self { self.accent = c; self }
    pub fn dim(mut self, c: Color32) -> Self { self.dim = c; self }
    pub fn subtitle(mut self, s: &'a str) -> Self { self.subtitle = Some(s); self }
    pub fn theme(self, t: &super::super::super::gpu::Theme) -> Self {
        self.accent(t.accent).dim(t.dim)
    }

    /// Render the header. Returns `true` if the close button was clicked.
    pub fn show(self, ui: &mut Ui) -> bool {
        super::super::style::panel_header_sub(ui, self.title, self.subtitle, self.accent, self.dim)
    }
}

// ─── DialogHeader ─────────────────────────────────────────────────────────────

/// Builder for a dialog header bar (no close button).
/// Mirrors the frame/title portion of `style::dialog_header_colored` without the X button.
///
/// ```ignore
/// ui.add(DialogHeader::new("Settings").theme(t));
/// ```
#[must_use = "DialogHeader must be added with `ui.add(...)` to render"]
pub struct DialogHeader<'a> {
    title: &'a str,
    dim:   Color32,
}

impl<'a> DialogHeader<'a> {
    pub fn new(title: &'a str) -> Self {
        Self {
            title,
            dim: Color32::from_rgb(120, 120, 130),
        }
    }
    pub fn dim(mut self, c: Color32) -> Self { self.dim = c; self }
    pub fn theme(self, t: &super::super::super::gpu::Theme) -> Self {
        self.dim(t.dim)
    }
}

impl<'a> Widget for DialogHeader<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        let darken = crate::dt_u8!(dialog.header_darken, 8);
        let bg = ui.visuals().window_fill();
        let fill = Color32::from_rgb(
            bg.r().saturating_sub(darken),
            bg.g().saturating_sub(darken),
            bg.b().saturating_sub(darken),
        );
        let s = current();
        let rlg = s.r_lg as u8;
        let title = self.title;
        let frame_resp = egui::Frame::NONE
            .fill(fill)
            .inner_margin(egui::Margin { left: 12, right: 10, top: 10, bottom: 10 })
            .corner_radius(egui::CornerRadius { nw: rlg, ne: rlg, sw: 0, se: 0 })
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let text_col = ui.style().visuals.override_text_color.unwrap_or(TEXT_PRIMARY);
                    let title_text = style_label_case(title);
                    ui.label(SemanticLabel::new(title_text, LabelVariant::HeaderLg).color(text_col).into_rich_text());
                });
            });
        if s.hairline_borders {
            let r = frame_resp.response.rect;
            let border = ui.style().visuals.override_text_color.unwrap_or(TEXT_PRIMARY);
            ui.painter().line_segment(
                [egui::pos2(r.left(), r.bottom()), egui::pos2(r.right(), r.bottom())],
                Stroke::new(s.stroke_std, color_alpha(border, alpha_muted())),
            );
        }
        frame_resp.response
    }
}

// ─── DialogHeaderWithClose ───────────────────────────────────────────────────

/// Builder for a dialog header bar with close button. Returns `true` if close clicked.
/// Mirrors `style::dialog_header(ui, title, dim)`.
///
/// ```ignore
/// if DialogHeaderWithClose::new("Settings").theme(t).show(ui) { open = false; }
/// ```
#[must_use]
pub struct DialogHeaderWithClose<'a> {
    title: &'a str,
    dim:   Color32,
}

impl<'a> DialogHeaderWithClose<'a> {
    pub fn new(title: &'a str) -> Self {
        Self {
            title,
            dim: Color32::from_rgb(120, 120, 130),
        }
    }
    pub fn dim(mut self, c: Color32) -> Self { self.dim = c; self }
    pub fn theme(self, t: &super::super::super::gpu::Theme) -> Self {
        self.dim(t.dim)
    }

    /// Render the header. Returns `true` if the close button was clicked.
    pub fn show(self, ui: &mut Ui) -> bool {
        super::super::style::dialog_header(ui, self.title, self.dim)
    }
}

// ─── PaneHeader ───────────────────────────────────────────────────────────────

/// Builder for a pane header bar (background + bottom rule, no close button).
/// Wraps `components::pane_header_bar` with a title label as the only content.
///
/// ```ignore
/// ui.add(PaneHeader::new("Chart").theme(t));
/// ```
#[must_use = "PaneHeader must be added with `ui.add(...)` to render"]
pub struct PaneHeader<'a> {
    title:        &'a str,
    title_color:  Color32,
    bg:           Color32,
    border:       Color32,
    height:       f32,
}

impl<'a> PaneHeader<'a> {
    pub fn new(title: &'a str) -> Self {
        Self {
            title,
            title_color: Color32::from_rgb(120, 140, 220),
            bg:          Color32::from_rgb(20, 20, 28),
            border:      Color32::from_rgb(50, 50, 60),
            height:      28.0,
        }
    }
    pub fn title_color(mut self, c: Color32) -> Self { self.title_color = c; self }
    pub fn bg(mut self, c: Color32) -> Self { self.bg = c; self }
    pub fn border(mut self, c: Color32) -> Self { self.border = c; self }
    pub fn height(mut self, h: f32) -> Self { self.height = h; self }
    pub fn theme(self, t: &super::super::super::gpu::Theme) -> Self {
        self.title_color(t.accent).bg(t.toolbar_bg).border(t.toolbar_border)
    }
}

impl<'a> Widget for PaneHeader<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        let title = self.title;
        let title_color = self.title_color;
        pane_header_bar(ui, self.height, self.bg, self.border, |ui| {
            section_label_widget(ui, title, title_color);
        });
        // Return an invisible response covering the allocated area.
        ui.allocate_response(Vec2::ZERO, Sense::hover())
    }
}

// ─── PaneHeaderWithClose ─────────────────────────────────────────────────────

/// Builder for a pane header bar with close button. Returns `true` if close clicked.
/// Mirrors `components::panel_header(ui, title, title_color, open)`.
///
/// ```ignore
/// if PaneHeaderWithClose::new("Chart").theme(t).show(ui, &mut open) { ... }
/// ```
#[must_use]
pub struct PaneHeaderWithClose<'a> {
    title:       &'a str,
    title_color: Color32,
}

impl<'a> PaneHeaderWithClose<'a> {
    pub fn new(title: &'a str) -> Self {
        Self {
            title,
            title_color: Color32::from_rgb(120, 140, 220),
        }
    }
    pub fn title_color(mut self, c: Color32) -> Self { self.title_color = c; self }
    pub fn theme(self, t: &super::super::super::gpu::Theme) -> Self {
        self.title_color(t.accent)
    }

    /// Render the header. Returns `true` if the close button was clicked.
    pub fn show(self, ui: &mut Ui, open: &mut bool) -> bool {
        super::super::components::panel_header(ui, self.title, self.title_color, open)
    }
}
