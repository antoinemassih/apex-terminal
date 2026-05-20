//! Builder primitives — frames family.
//!
//! Canonical builder API for all frame types. Call sites use the chained-setter
//! pattern:
//!
//! ```ignore
//! let frame = PanelFrame::new(bg, border).build();
//! let frame = PanelFrame::new(bg, border).theme(t).build();
//! ```

#![allow(dead_code, unused_imports)]

use egui::{Color32, Stroke};
use super::super::style::*;

// Shorthand for the Theme type used across the codebase.
type Theme = crate::chart_renderer::gpu::Theme;

// ─── PanelFrame ───────────────────────────────────────────────────────────────

/// Builder for `panel_frame(toolbar_bg, toolbar_border) -> egui::Frame`.
///
/// ```ignore
/// let f = PanelFrame::new(theme.toolbar_bg, theme.toolbar_border).build();
/// let f = PanelFrame::new(Color32::TRANSPARENT, Color32::TRANSPARENT)
///             .theme(t).build();
/// ```
pub struct PanelFrame {
    bg: Color32,
    border: Color32,
    margin: Option<egui::Margin>,
}

impl PanelFrame {
    pub fn new(toolbar_bg: Color32, toolbar_border: Color32) -> Self {
        Self { bg: toolbar_bg, border: toolbar_border, margin: None }
    }

    /// Pull bg + border from a theme.
    pub fn theme(mut self, t: &Theme) -> Self {
        self.bg = t.toolbar_bg;
        self.border = t.toolbar_border;
        self
    }

    /// Override with explicit colors (alternative to `.theme()`).
    pub fn colors(mut self, bg: Color32, border: Color32) -> Self {
        self.bg = bg;
        self.border = border;
        self
    }

    /// Drop the default inner padding. Use when the panel renders its own
    /// chart-pane-aligned header that needs to sit flush against the panel
    /// edge.
    pub fn zero_margin(mut self) -> Self {
        self.margin = Some(egui::Margin::ZERO);
        self
    }

    /// Custom inner margin override.
    pub fn inner_margin(mut self, m: egui::Margin) -> Self {
        self.margin = Some(m);
        self
    }

    /// Build the `egui::Frame`. Edge-only border discipline (Zed): the
    /// SidePanel divider is provided by egui's resize handle on the touching
    /// edge — we omit the 4-edge perimeter stroke so we don't draw a doubled
    /// hairline against neighboring panels.
    ///
    /// Default margin is **zero** so the new chart-pane-parity `PanelHeader`
    /// renders flush at the panel's top edge and full width. Panels add their
    /// own body padding (typically via `kit::panel_body`) inside.
    pub fn build(self) -> egui::Frame {
        let _ = self.border; // kept for API symmetry; perimeter stroke dropped
        let margin = self.margin.unwrap_or(egui::Margin::ZERO);
        egui::Frame::NONE
            .fill(self.bg)
            .inner_margin(margin)
            .corner_radius(r_md_cr())
    }
}

// ─── CardFrame ────────────────────────────────────────────────────────────────

/// Builder for card frames.
///
/// Extracts frame construction so callers can obtain the `egui::Frame` value
/// directly and call `.show(ui, |ui| { ... })` themselves.
///
/// ```ignore
/// CardFrame::new().theme(t).build().show(ui, |ui| { ... });
/// ```
pub struct CardFrame {
    bg: Color32,
    border: Color32,
    /// Themed shadow base color; `None` falls back to the legacy black tint.
    shadow_color: Option<Color32>,
}

impl CardFrame {
    pub fn new() -> Self {
        Self { bg: Color32::TRANSPARENT, border: Color32::TRANSPARENT, shadow_color: None }
    }

    pub fn theme(self, t: &Theme) -> Self {
        Self { bg: t.toolbar_bg, border: t.toolbar_border, shadow_color: Some(t.shadow_color) }
    }

    pub fn colors(self, bg: Color32, border: Color32) -> Self {
        Self { bg, border, ..self }
    }

    /// Build the `egui::Frame`.
    pub fn build(self) -> egui::Frame {
        let st = current();
        // card_padding_y / card_padding_x knobs let the user tune card insets per style.
        let pad_y = st.card_padding_y as i8;
        let pad_x = st.card_padding_x as i8;
        let mut frame = egui::Frame::NONE
            .fill(self.bg)
            .corner_radius(r_md_cr())
            .inner_margin(egui::Margin { left: pad_x, right: pad_x, top: pad_y, bottom: pad_y });

        if st.hairline_borders {
            frame = frame.stroke(Stroke::new(
                st.stroke_std,
                color_alpha(self.border, alpha_strong()),
            ));
        } else {
            frame = frame.stroke(Stroke::new(
                st.stroke_thin,
                color_alpha(self.border, alpha_muted()),
            ));
        }

        if st.shadows_enabled {
            // shadow_blur / shadow_offset_y / shadow_alpha knobs override global tokens.
            let shadow_col = if let Some(sc) = self.shadow_color {
                color_alpha(sc, st.shadow_alpha)
            } else {
                Color32::from_black_alpha(st.shadow_alpha)
            };
            frame = frame.shadow(egui::epaint::Shadow {
                offset: [0, st.shadow_offset_y as i8],
                blur:   st.shadow_blur as u8,
                spread: 0,
                color:  shadow_col,
            });
        }

        frame
    }
}

impl Default for CardFrame {
    fn default() -> Self { Self::new() }
}

// ─── PopupFrame ───────────────────────────────────────────────────────────────

/// Builder for popup frames.
///
/// ```ignore
/// PopupFrame::new().theme(t).ctx(ctx).build()
/// ```
/// Controls which alpha tier is used for the popup border stroke.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BorderAlpha {
    /// `alpha_line()` — 50 — used by context menus / submenus.
    Line,
    /// `alpha_strong()` — 80 — the default for most popups.
    Strong,
}

pub struct PopupFrame<'a> {
    bg: Color32,
    border: Color32,
    ctx: Option<&'a egui::Context>,
    inner_margin: Option<egui::Margin>,
    corner_radius_override: Option<f32>,
    border_alpha: BorderAlpha,
    /// Themed shadow base color; `None` falls back to the legacy black tint.
    shadow_color: Option<Color32>,
}

impl<'a> PopupFrame<'a> {
    pub fn new() -> Self {
        Self {
            bg: Color32::TRANSPARENT,
            border: Color32::TRANSPARENT,
            ctx: None,
            inner_margin: None,
            corner_radius_override: None,
            border_alpha: BorderAlpha::Strong,
            shadow_color: None,
        }
    }

    pub fn theme(self, t: &Theme) -> Self {
        Self { bg: t.toolbar_bg, border: t.toolbar_border, shadow_color: Some(t.shadow_color), ..self }
    }

    pub fn colors(self, bg: Color32, border: Color32) -> Self {
        Self { bg, border, ..self }
    }

    pub fn ctx(self, ctx: &'a egui::Context) -> Self {
        Self { ctx: Some(ctx), ..self }
    }

    /// Override the inner margin (e.g. `egui::Margin::ZERO` for zero-margin popups).
    pub fn inner_margin(self, m: egui::Margin) -> Self {
        Self { inner_margin: Some(m), ..self }
    }

    /// Convenience: set inner margin to zero on all sides.
    pub fn no_inner_margin(self) -> Self {
        self.inner_margin(egui::Margin::ZERO)
    }

    /// Override the corner radius (e.g. `12.0` for pill-shaped popups).
    pub fn corner_radius(self, r: f32) -> Self {
        Self { corner_radius_override: Some(r), ..self }
    }

    /// Choose which alpha tier to use for the border stroke.
    pub fn border_alpha(self, a: BorderAlpha) -> Self {
        Self { border_alpha: a, ..self }
    }

    /// Build the `egui::Frame`.
    pub fn build(self) -> egui::Frame {
        let st = current();
        let ctx = self.ctx.expect("PopupFrame::build requires a Context — call .ctx(ctx) first");

        let pop_bg = if st.hairline_borders {
            self.bg.gamma_multiply(1.10)
        } else {
            self.bg
        };

        let cr = if let Some(r) = self.corner_radius_override {
            egui::CornerRadius::same(r as u8)
        } else {
            r_lg_cr()
        };

        let margin = self.inner_margin.unwrap_or_else(|| egui::Margin::same(gap_lg() as i8));

        let mut frame = egui::Frame::popup(&ctx.style())
            .fill(pop_bg)
            .corner_radius(cr)
            .inner_margin(margin);

        let border_alpha_val = match self.border_alpha {
            BorderAlpha::Line   => alpha_line(),
            BorderAlpha::Strong => alpha_strong(),
        };

        if st.hairline_borders {
            frame = frame.stroke(Stroke::new(st.stroke_std, self.border));
        } else {
            frame = frame.stroke(Stroke::new(
                st.stroke_thin,
                color_alpha(self.border, border_alpha_val),
            ));
        }

        if st.shadows_enabled {
            let shadow_col = if let Some(sc) = self.shadow_color {
                color_alpha(sc, st.shadow_alpha)
            } else {
                Color32::from_black_alpha(st.shadow_alpha)
            };
            frame = frame.shadow(egui::epaint::Shadow {
                offset: [0, st.shadow_offset_y as i8],
                blur:   st.shadow_blur as u8,
                spread: 1,
                color:  shadow_col,
            });
        } else {
            frame = frame.shadow(egui::epaint::Shadow::NONE);
        }

        frame
    }
}

impl<'a> Default for PopupFrame<'a> {
    fn default() -> Self { Self::new() }
}

// ─── CompactPanelFrame ────────────────────────────────────────────────────────

/// Builder for `style::panel_frame_compact(toolbar_bg, toolbar_border) -> egui::Frame`.
///
/// Tighter margins than `PanelFrame` for narrow info-dense panels (scanner, tape).
///
/// ```ignore
/// let f = CompactPanelFrame::new(theme.toolbar_bg, theme.toolbar_border).build();
/// let f = CompactPanelFrame::new(Color32::TRANSPARENT, Color32::TRANSPARENT)
///             .theme(t).build();
/// ```
pub struct CompactPanelFrame {
    bg: Color32,
    border: Color32,
}

impl CompactPanelFrame {
    pub fn new(toolbar_bg: Color32, toolbar_border: Color32) -> Self {
        Self { bg: toolbar_bg, border: toolbar_border }
    }

    pub fn theme(self, t: &Theme) -> Self {
        Self { bg: t.toolbar_bg, border: t.toolbar_border }
    }

    pub fn colors(self, bg: Color32, border: Color32) -> Self {
        Self { bg, border }
    }

    /// Build the `egui::Frame`. Edge-only border discipline (Zed): SidePanel
    /// divider provided by egui's resize handle; we drop the 4-edge perimeter
    /// stroke to avoid double hairlines.
    pub fn build(self) -> egui::Frame {
        let _ = self.border; // kept for API symmetry; perimeter stroke dropped
        egui::Frame::NONE
            .fill(self.bg)
            .inner_margin(egui::Margin {
                left:   gap_lg() as i8,
                right:  gap_lg() as i8,
                top:    gap_lg() as i8,
                bottom: gap_md() as i8,
            })
            .corner_radius(r_sm_cr())
    }
}

