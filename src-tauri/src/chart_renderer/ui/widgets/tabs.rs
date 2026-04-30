//! Builder + `.show(ui)` design-system primitives — tabs family.
//! See ui/widgets/mod.rs for the rationale.
//!
//! Three builders wrap the three legacy tab helpers:
//!  - `TabBar`          → wraps `style::tab_bar`            (generic, mutates `&mut T`)
//!  - `TabStrip`        → wraps `components::tab_strip`     (index-based, returns `Option<usize>`)
//!  - `TabBarWithClose` → wraps `components_extra::tab_bar_with_close` (returns `TabAction`)
//!
//! All use `.show(ui)` instead of `impl Widget` because they hold `&mut` references
//! and/or need non-`Response` return values.

#![allow(dead_code, unused_imports)]

use egui::{Color32, Pos2, RichText, Stroke, Ui, Vec2};
use super::super::style::*;
use super::super::components_extra::TabAction;

// ─── TabBar ───────────────────────────────────────────────────────────────────

/// Builder for a horizontal tab bar with 2 px underline on the active tab.
/// Wraps `style::tab_bar`.
///
/// ```ignore
/// TabBar::new(&mut current_tab, &[(Tab::Overview, "Overview"), (Tab::Detail, "Detail")])
///     .accent(theme.bull)
///     .dim(theme.dim)
///     .show(ui);
/// ```
pub struct TabBar<'a, 'b, T: PartialEq + Copy> {
    current: &'b mut T,
    tabs: &'a [(T, &'a str)],
    accent: Color32,
    dim: Color32,
    font_size: Option<f32>,
    underline: bool,
    min_height: Option<f32>,
}

impl<'a, 'b, T: PartialEq + Copy> TabBar<'a, 'b, T> {
    pub fn new(current: &'b mut T, tabs: &'a [(T, &'a str)]) -> Self {
        Self {
            current,
            tabs,
            accent: Color32::from_rgb(100, 160, 255),
            dim:    Color32::from_rgb(120, 120, 130),
            font_size: None,
            underline: true,
            min_height: None,
        }
    }

    pub fn accent(mut self, c: Color32) -> Self { self.accent = c; self }
    pub fn dim(mut self, c: Color32) -> Self { self.dim = c; self }
    /// Override the label font size (default: `font_lg()`).
    pub fn font_size(mut self, px: f32) -> Self { self.font_size = Some(px); self }
    /// Show the 2px active-tab underline (default: true). Set false for flat tabs.
    pub fn underline(mut self, on: bool) -> Self { self.underline = on; self }
    /// Force a minimum button height (used when the tab strip needs a fixed row size).
    pub fn min_height(mut self, h: f32) -> Self { self.min_height = Some(h); self }

    /// Pull accent + dim from a theme.
    pub fn theme(self, t: &super::super::super::gpu::Theme) -> Self {
        self.accent(t.bull).dim(t.dim)
    }

    pub fn show(self, ui: &mut Ui) {
        // Default behaviour preserved: when no knobs were touched (font_size=None,
        // underline=true, min_height=None), this delegates byte-for-byte to
        // `style::tab_bar` to keep all existing call sites visually identical.
        if self.font_size.is_none() && self.underline && self.min_height.is_none() {
            tab_bar(ui, self.current, self.tabs, self.accent, self.dim);
            return;
        }
        let fs = self.font_size.unwrap_or_else(font_lg);
        let tab_ul = crate::dt_f32!(tab.underline_thickness, 2.0);
        for (tab, label) in self.tabs {
            let active = *self.current == *tab;
            let color = if active { self.accent } else { self.dim };
            let mut btn = egui::Button::new(
                egui::RichText::new(*label).monospace().size(fs).strong().color(color)
            ).frame(false).fill(Color32::TRANSPARENT).stroke(Stroke::NONE);
            if let Some(h) = self.min_height {
                btn = btn.min_size(Vec2::new(0.0, h));
            }
            let resp = ui.add(btn);
            if resp.clicked() { *self.current = *tab; }
            if active && self.underline {
                let r = resp.rect;
                ui.painter().rect_filled(
                    egui::Rect::from_min_max(
                        egui::pos2(r.left(), r.max.y - tab_ul),
                        egui::pos2(r.right(), r.max.y)),
                    0.0, self.accent);
            }
        }
    }
}

// ─── TabStrip ─────────────────────────────────────────────────────────────────

/// Builder for a horizontal tab strip (index-based).
/// Wraps `components::tab_strip`. Returns `Option<usize>` — the index clicked.
///
/// ```ignore
/// if let Some(i) = TabStrip::new(&["Orders", "Fills"], active).show(ui) {
///     active = i;
/// }
/// ```
pub struct TabStrip<'a> {
    tabs: &'a [&'a str],
    active: usize,
    accent: Color32,
    dim: Color32,
}

impl<'a> TabStrip<'a> {
    pub fn new(tabs: &'a [&'a str], active: usize) -> Self {
        Self {
            tabs,
            active,
            accent: Color32::from_rgb(100, 160, 255),
            dim:    Color32::from_rgb(120, 120, 130),
        }
    }

    pub fn accent(mut self, c: Color32) -> Self { self.accent = c; self }
    pub fn dim(mut self, c: Color32) -> Self { self.dim = c; self }

    /// Pull accent + dim from a theme.
    pub fn theme(self, t: &super::super::super::gpu::Theme) -> Self {
        self.accent(t.bull).dim(t.dim)
    }

    pub fn show(self, ui: &mut Ui) -> Option<usize> {
        let st = current();
        let mut clicked = None;

        ui.horizontal(|ui| {
            let prev = ui.spacing().item_spacing.x;
            ui.spacing_mut().item_spacing.x = gap_md();

            for (i, label) in self.tabs.iter().enumerate() {
                let is_active = i == self.active;
                let text = style_label_case(label);
                let fg = if is_active { self.accent } else { self.dim };

                if is_active && !st.hairline_borders {
                    // Relay: pill background behind active tab.
                    let resp = ui.add(
                        egui::Button::new(
                            RichText::new(text).monospace().size(font_md()).strong().color(fg),
                        )
                        .fill(color_alpha(self.accent, alpha_tint()))
                        .stroke(Stroke::NONE)
                        .corner_radius(r_pill())
                        .min_size(Vec2::new(0.0, 20.0)),
                    );
                    if resp.clicked() {
                        clicked = Some(i);
                    }
                    if resp.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                } else {
                    let resp = ui.add(
                        egui::Button::new(
                            RichText::new(text).monospace().size(font_md()).strong().color(fg),
                        )
                        .frame(false)
                        .min_size(Vec2::new(0.0, 20.0)),
                    );
                    if resp.clicked() {
                        clicked = Some(i);
                    }
                    if resp.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    if is_active && st.hairline_borders {
                        let r = resp.rect;
                        ui.painter().line_segment(
                            [
                                Pos2::new(r.left(), r.bottom() + 0.5),
                                Pos2::new(r.right(), r.bottom() + 0.5),
                            ],
                            Stroke::new(st.stroke_std, self.accent),
                        );
                    }
                }
            }

            ui.spacing_mut().item_spacing.x = prev;
        });

        clicked
    }
}

// ─── TabBarWithClose ──────────────────────────────────────────────────────────

/// Builder for a tab strip with per-tab close buttons.
/// Wraps `components_extra::tab_bar_with_close`. Returns `TabAction`.
///
/// ```ignore
/// match TabBarWithClose::new(&["Chart", "DOM"], active).show(ui) {
///     TabAction::Selected(i) => active = i,
///     TabAction::Closed(i)   => close_tab(i),
///     TabAction::None        => {}
/// }
/// ```
pub struct TabBarWithClose<'a> {
    tabs: &'a [&'a str],
    active: usize,
    accent: Color32,
    dim: Color32,
}

impl<'a> TabBarWithClose<'a> {
    pub fn new(tabs: &'a [&'a str], active: usize) -> Self {
        Self {
            tabs,
            active,
            accent: Color32::from_rgb(100, 160, 255),
            dim:    Color32::from_rgb(120, 120, 130),
        }
    }

    pub fn accent(mut self, c: Color32) -> Self { self.accent = c; self }
    pub fn dim(mut self, c: Color32) -> Self { self.dim = c; self }

    /// Pull accent + dim from a theme.
    pub fn theme(self, t: &super::super::super::gpu::Theme) -> Self {
        self.accent(t.bull).dim(t.dim)
    }

    pub fn show(self, ui: &mut Ui) -> TabAction {
        let st = current();
        let mut action = TabAction::None;

        ui.horizontal(|ui| {
            let prev_x = ui.spacing().item_spacing.x;
            ui.spacing_mut().item_spacing.x = gap_xs();

            for (i, label) in self.tabs.iter().enumerate() {
                let is_active = i == self.active;
                let fg = if is_active { self.accent } else { self.dim };
                let s = style_label_case(label);

                // Per-tab cluster (label + ×)
                ui.horizontal(|ui| {
                    let prev_inner = ui.spacing().item_spacing.x;
                    ui.spacing_mut().item_spacing.x = 1.0;

                    if is_active && !st.hairline_borders {
                        let resp = ui.add(
                            egui::Button::new(
                                RichText::new(s).monospace().size(font_sm()).strong().color(fg),
                            )
                            .fill(color_alpha(self.accent, alpha_tint()))
                            .stroke(Stroke::NONE)
                            .corner_radius(r_pill())
                            .min_size(Vec2::new(0.0, 18.0)),
                        );
                        if resp.clicked() { action = TabAction::Selected(i); }
                        if resp.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                    } else {
                        let resp = ui.add(
                            egui::Button::new(
                                RichText::new(s).monospace().size(font_sm()).strong().color(fg),
                            )
                            .frame(false)
                            .min_size(Vec2::new(0.0, 18.0)),
                        );
                        if resp.clicked() { action = TabAction::Selected(i); }
                        if resp.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                        if is_active && st.hairline_borders {
                            let r = resp.rect;
                            ui.painter().line_segment(
                                [
                                    egui::pos2(r.left(), r.bottom() + 0.5),
                                    egui::pos2(r.right(), r.bottom() + 0.5),
                                ],
                                Stroke::new(st.stroke_std, self.accent),
                            );
                        }
                    }

                    if super::super::components_extra::secondary_close_btn(ui, self.dim) {
                        action = TabAction::Closed(i);
                    }

                    ui.spacing_mut().item_spacing.x = prev_inner;
                });
            }
            ui.spacing_mut().item_spacing.x = prev_x;
        });

        action
    }
}
