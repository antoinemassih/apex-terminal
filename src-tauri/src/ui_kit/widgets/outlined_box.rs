//! `OutlinedBox` — themed bordered container frame.
//!
//! The audit identified ~6+ chrome sites hand-rolling
//! `egui::Frame::NONE.fill(t.toolbar_bg).stroke(Stroke::new(stroke_std(), border)).corner_radius(r)`
//! sequences for bordered cards (welcome wizard broker cards,
//! FloatingPaneChrome's outer border, painter_chrome paint sequences).
//! This component consolidates that pattern with token defaults.
//!
//! ### Usage
//!
//! ```ignore
//! // Simple bordered card.
//! OutlinedBox::new()
//!     .show(ui, theme, |ui| {
//!         ui.label("Hello");
//!     });
//!
//! // With explicit fill, radius, and padding.
//! OutlinedBox::new()
//!     .fill(theme.surface())
//!     .radius_md()
//!     .padding(gap_md())
//!     .show(ui, theme, |ui| { /* … */ });
//!
//! // Hairline (faint) border.
//! OutlinedBox::new().hairline().show(ui, theme, body);
//! ```
//!
//! ### Defaults
//! - Fill: `theme.bg()` (deepest background — use `.fill(theme.surface())` for raised cards)
//! - Border: `theme.border()` at `stroke_std()` (or `stroke_thin()` when `.hairline()`)
//! - Corner radius: `radius_sm()`
//! - Inner padding: `gap_sm()`

use egui::{Color32, CornerRadius, Margin, Response, Stroke, Ui};

use super::theme::ComponentTheme;
use crate::ui_kit::tokens as st;

/// Optional override for the border thickness tier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BorderTier {
    None,     // no border (fill-only band)
    Hairline, // stroke_thin
    Standard, // stroke_std (default)
    Bold,     // stroke_bold
}

#[must_use = "OutlinedBox does nothing until `.show(ui, theme, body)` is called"]
pub struct OutlinedBox {
    fill: Option<Color32>,
    border: Option<Color32>,
    border_tier: BorderTier,
    radius: Option<CornerRadius>,
    padding: Option<f32>,
    padding_margin: Option<Margin>,
    outer_margin: Option<f32>,
}

impl OutlinedBox {
    pub fn new() -> Self {
        Self {
            fill: None,
            border: None,
            border_tier: BorderTier::Standard,
            radius: None,
            padding: None,
            padding_margin: None,
            outer_margin: None,
        }
    }

    /// Override the fill color (defaults to `theme.bg()`).
    pub fn fill(mut self, c: Color32) -> Self { self.fill = Some(c); self }

    /// Override the border color (defaults to `theme.border()`).
    pub fn border(mut self, c: Color32) -> Self { self.border = Some(c); self }

    /// Use the hairline-tier border (`stroke_thin`).
    pub fn hairline(mut self) -> Self { self.border_tier = BorderTier::Hairline; self }

    /// Use the bold-tier border (`stroke_bold`).
    pub fn bold_border(mut self) -> Self { self.border_tier = BorderTier::Bold; self }

    /// Drop the border entirely. For fill-only bands (e.g. side-panel header
    /// surface band) where you want OutlinedBox's tokens for fill/radius/padding
    /// but no stroke at all. Pairs naturally with `.fill(theme.header_surface())`.
    pub fn borderless(mut self) -> Self { self.border_tier = BorderTier::None; self }

    /// Set the corner radius via raw `CornerRadius`. Prefer the `.radius_*()` shortcuts.
    pub fn radius(mut self, r: CornerRadius) -> Self { self.radius = Some(r); self }

    /// XS rounding (defaults to `radius_xs()`).
    pub fn radius_xs(mut self) -> Self { self.radius = Some(st::r_xs_cr()); self }
    pub fn radius_sm(mut self) -> Self { self.radius = Some(st::r_sm_cr()); self }
    pub fn radius_md(mut self) -> Self { self.radius = Some(st::r_md_cr()); self }
    pub fn radius_lg(mut self) -> Self { self.radius = Some(st::r_lg_cr()); self }
    /// Square (no rounding). For sites like FloatingPane outer chrome / numeric steppers
    /// where the design calls for hard 90° corners.
    pub fn square(mut self) -> Self { self.radius = Some(CornerRadius::ZERO); self }

    /// Set inner padding (defaults to `gap_sm()`).
    pub fn padding(mut self, px: f32) -> Self { self.padding = Some(px); self }

    /// Asymmetric inner padding via Margin (overrides .padding()).
    /// For rows where horizontal/vertical pad differ (e.g. RowShell).
    pub fn padding_margin(mut self, m: Margin) -> Self { self.padding_margin = Some(m); self }

    /// Add an outer margin around the entire box.
    pub fn outer_margin(mut self, px: f32) -> Self { self.outer_margin = Some(px); self }

    pub fn show<R>(
        self,
        ui: &mut Ui,
        theme: &dyn ComponentTheme,
        body: impl FnOnce(&mut Ui) -> R,
    ) -> egui::InnerResponse<R> {
        let fill = self.fill.unwrap_or_else(|| theme.bg());
        let stroke = match self.border_tier {
            BorderTier::None     => Stroke::NONE,
            BorderTier::Hairline => Stroke::new(st::stroke_thin(), self.border.unwrap_or_else(|| theme.border())),
            BorderTier::Standard => Stroke::new(st::stroke_std(),  self.border.unwrap_or_else(|| theme.border())),
            BorderTier::Bold     => Stroke::new(st::stroke_bold(), self.border.unwrap_or_else(|| theme.border())),
        };
        let radius = self.radius.unwrap_or_else(st::r_sm_cr);
        let inner_margin = self.padding_margin.unwrap_or_else(|| {
            let inner = self.padding.unwrap_or_else(st::gap_sm);
            Margin::same(inner as i8)
        });
        let outer = self.outer_margin.unwrap_or(0.0);

        egui::Frame::NONE
            .fill(fill)
            .stroke(stroke)
            .corner_radius(radius)
            .inner_margin(inner_margin)
            .outer_margin(Margin::same(outer as i8))
            .show(ui, body)
    }
}

impl Default for OutlinedBox {
    fn default() -> Self { Self::new() }
}

/// Convenience: get the underlying Response of the box (e.g. for hover detection).
pub trait OutlinedBoxResponseExt {
    fn response(&self) -> &Response;
}

impl<R> OutlinedBoxResponseExt for egui::InnerResponse<R> {
    fn response(&self) -> &Response { &self.response }
}
