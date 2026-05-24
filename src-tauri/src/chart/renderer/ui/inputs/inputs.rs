//! Builder + impl Widget primitives — inputs family.
//! See ui/widgets/mod.rs for the rationale.

#![allow(dead_code, unused_imports, deprecated)]

use egui::{Color32, Response, RichText, Sense, Stroke, Ui, Vec2, Widget};
use super::super::style::*;
use super::super::super::gpu::Theme;

#[inline(always)]
fn ambient(ctx: &egui::Context) -> Theme {
    crate::ui_kit::widgets::theme::active_theme(ctx)
}

// ─── ColorSwatchPicker ────────────────────────────────────────────────────────

/// A horizontal row of color-dot swatches drawn from a `&[&str]` hex palette,
/// with an optional "auto" (empty-string) button at the right end.
///
/// `value` is a `String` holding the currently-selected hex code, or `""` for auto.
///
/// ```ignore
/// if ColorSwatchPicker::new(&mut ind.color).palette(INDICATOR_COLORS)
///     .theme(t).show(ui)
/// { /* changed */ }
/// ```
#[must_use = "ColorSwatchPicker must be rendered via `.show(ui)`"]
pub struct ColorSwatchPicker<'a> {
    value: &'a mut String,
    palette: &'a [&'a str],
    swatch_size: f32,
    dot_radius: f32,
    auto_button: bool,
    /// Alpha applied to the dot when rendering the fill swatch (0 = full opacity).
    fill_alpha: u8,
    accent: Option<Color32>,
    dim: Option<Color32>,
    theme: Option<&'a Theme>,
}

impl<'a> ColorSwatchPicker<'a> {
    pub fn new(value: &'a mut String) -> Self {
        Self {
            value,
            palette: &[],
            swatch_size: 12.0,
            dot_radius: 3.0,
            auto_button: false,
            fill_alpha: 255,
            accent: None,
            dim: None,
            theme: None,
        }
    }
    pub fn palette(mut self, p: &'a [&'a str]) -> Self { self.palette = p; self }
    /// Size of the hit-rect allocated per swatch (default 12×12).
    pub fn swatch_size(mut self, s: f32) -> Self { self.swatch_size = s; self }
    /// Dot radius when idle (selected dot is 1px larger, default 3.0).
    pub fn dot_radius(mut self, r: f32) -> Self { self.dot_radius = r; self }
    /// Show an "auto" button that clears the selection to `""` (default false).
    pub fn auto_button(mut self, v: bool) -> Self { self.auto_button = v; self }
    /// Alpha channel applied to every dot fill (255 = opaque).
    pub fn fill_alpha(mut self, a: u8) -> Self { self.fill_alpha = a; self }
    pub fn theme(mut self, t: &'a Theme) -> Self {
        self.theme = Some(t);
        self.accent = Some(t.accent);
        self.dim = Some(t.dim);
        self
    }

    /// Returns `true` if the value was changed.
    pub fn show(self, ui: &mut Ui) -> bool {
        use super::super::style::*;
        let accent = self.accent.unwrap_or(ambient(ui.ctx()).accent);
        let dim = self.dim.unwrap_or(ambient(ui.ctx()).dim);
        let dot_r = self.dot_radius;
        let sel_dot_r = dot_r + 1.0;
        let sz = self.swatch_size;
        let mut changed = false;
        let value = self.value;
        let fill_alpha = self.fill_alpha;

        let prev_spacing = ui.spacing().item_spacing.x;
        ui.spacing_mut().item_spacing.x = 3.0;

        for &hex in self.palette {
            let col_full = super::super::style::hex_to_color(hex, 1.0);
            let col_draw = if fill_alpha < 255 {
                color_alpha(col_full, fill_alpha)
            } else {
                col_full
            };
            let is_cur = value.as_str() == hex;
            let (r, resp) = ui.allocate_exact_size(egui::vec2(sz, sz), Sense::click());
            if is_cur {
                ui.painter().rect_stroke(r, 2.0,
                    egui::Stroke::new(stroke_std(), col_full), egui::StrokeKind::Outside);
            }
            ui.painter().circle_filled(r.center(), if is_cur { sel_dot_r } else { dot_r }, col_draw);
            if resp.clicked() && !is_cur {
                *value = hex.to_string();
                changed = true;
            }
        }

        if self.auto_button {
            let is_auto = value.is_empty();
            use crate::ui_kit::widgets::Button;
            use crate::ui_kit::widgets::tokens::{Variant, Size as KitSize};
            if ui.add(Button::new("auto").variant(Variant::Chip).active(is_auto).size(KitSize::Xs)
                .min_size(egui::vec2(24.0, sz))).clicked() && !is_auto {
                *value = String::new();
                changed = true;
            }
        }

        ui.spacing_mut().item_spacing.x = prev_spacing;
        changed
    }
}

// ─── ThicknessPicker ──────────────────────────────────────────────────────────

/// A connected-pill row for selecting a stroke thickness from a fixed list of
/// `f32` values. Renders as a segmented control (connected pills).
/// Domain-specific (chart drawing tool stroke width) — no ui_kit equivalent.
///
/// ```ignore
/// ThicknessPicker::new(&mut ind.thickness)
///     .values(&[0.5, 1.0, 1.5, 2.0, 3.0])
///     .height(18.0)
///     .theme(t)
///     .show(ui);
/// ```
#[must_use = "ThicknessPicker must be rendered via `.show(ui)`"]
pub struct ThicknessPicker<'a> {
    value: &'a mut f32,
    values: &'a [f32],
    height: f32,
    font_size: f32,
    min_btn_w: f32,
    accent: Option<Color32>,
    dim: Option<Color32>,
    border: Option<Color32>,
    theme: Option<&'a Theme>,
}

impl<'a> ThicknessPicker<'a> {
    pub fn new(value: &'a mut f32) -> Self {
        Self {
            value,
            values: &[0.5, 1.0, 1.5, 2.0, 3.0],
            height: 18.0,
            font_size: 8.0,
            min_btn_w: 26.0,
            accent: None,
            dim: None,
            border: None,
            theme: None,
        }
    }
    pub fn values(mut self, v: &'a [f32]) -> Self { self.values = v; self }
    pub fn height(mut self, h: f32) -> Self { self.height = h; self }
    pub fn font_size(mut self, s: f32) -> Self { self.font_size = s; self }
    pub fn min_btn_w(mut self, w: f32) -> Self { self.min_btn_w = w; self }
    pub fn theme(mut self, t: &'a Theme) -> Self {
        self.theme = Some(t);
        self.accent = Some(t.accent);
        self.dim = Some(t.dim);
        self.border = Some(t.toolbar_border);
        self
    }

    /// Returns `true` if the value was changed.
    pub fn show(self, ui: &mut Ui) -> bool {
        use super::super::style::*;
        let accent = self.accent.unwrap_or(ambient(ui.ctx()).accent);
        let dim = self.dim.unwrap_or(ambient(ui.ctx()).dim);
        let border = self.border.unwrap_or(ambient(ui.ctx()).toolbar_border);
        let n = self.values.len();
        let st = current();
        let r_sm = st.r_sm;
        let mut changed = false;
        let value = self.value;

        let prev_spacing = ui.spacing().item_spacing.x;
        ui.spacing_mut().item_spacing.x = 0.0;

        for (i, &th) in self.values.iter().enumerate() {
            let sel = (*value - th).abs() < 0.1;
            let fg = if sel { contrast_fg(accent) } else { color_subtle(dim) };
            let bg = if sel { color_alpha(accent, alpha_dim()) } else { color_alpha(border, alpha_subtle()) };
            let rounding: egui::CornerRadius = if i == 0 {
                egui::CornerRadius { nw: r_sm, sw: r_sm, ne: 0, se: 0 }
            } else if i == n - 1 {
                egui::CornerRadius { nw: 0, sw: 0, ne: r_sm, se: r_sm }
            } else {
                egui::CornerRadius::ZERO
            };
            let stroke_col = if sel { color_alpha(accent, alpha_heavy()) } else { color_alpha(border, alpha_line()) };
            // Intentional low-level egui::Button — threshold-row toggle
            // with per-position corner radii (first/middle/last cell of a
            // segmented strip) and selection-driven stroke colour. Same
            // shape problem as select.rs's pill segments — Variant doesn't
            // expose per-corner radii. Kept as legitimate low-level use.
            if ui.add(egui::Button::new(
                    egui::RichText::new(format!("{:.1}", th)).monospace().size(self.font_size).color(fg))
                .fill(bg)
                .corner_radius(rounding)
                .min_size(egui::vec2(self.min_btn_w, self.height))
                .stroke(egui::Stroke::new(stroke_thin(), stroke_col)))
                .clicked() && !sel
            {
                *value = th;
                changed = true;
            }
        }

        ui.spacing_mut().item_spacing.x = prev_spacing;
        changed
    }
}
