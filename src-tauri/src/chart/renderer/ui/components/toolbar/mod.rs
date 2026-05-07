//! Toolbar primitives.
//!
//! Legacy structs (ToolbarBtn/TopNavBtn/TopNavToggle/PaneTabBtn) and their
//! enums (TopNavTreatment/TopNavToggleSize/PaneTabStyle) were removed in the
//! ui_kit::widgets::Button migration. The remaining items here are
//! non-deprecated helpers used by the top-nav rendering code:
//!
//!  - `toolbar_btn` — thin wrapper over `style::tb_btn` that also flags the
//!    `gpu::TB_BTN_CLICKED` thread-local on click (so the window-drag handler
//!    ignores the same-frame click). Replaces `ToolbarBtn`.
//!  - `TimeframeSelector` — pill-row timeframe selector.
//!  - `PaneHeaderAction` — painter-positioned header action label.
//!
//! `top_nav` — the top navigation toolbar panel, extracted from `gpu.rs`.

#![allow(dead_code, unused_imports)]

pub mod top_nav;

use egui::{Color32, Response, RichText, Stroke, Ui, Widget};
use super::super::style::*;

fn ft() -> &'static super::super::super::gpu::Theme {
    &super::super::super::gpu::THEMES[0]
}

// ─── toolbar_btn (free function) ──────────────────────────────────────────────

/// Top-application-toolbar button. Delegates to `style::tb_btn` for pixel-exact
/// parity with the legacy renderer, and flags `gpu::TB_BTN_CLICKED` on click so
/// the window-drag handler ignores the click on the same frame.
pub fn toolbar_btn(
    ui: &mut Ui,
    label: &str,
    active: bool,
    t: &super::super::super::gpu::Theme,
) -> Response {
    let resp = super::super::style::tb_btn(
        ui, label, active,
        t.accent, t.dim, t.toolbar_bg, t.toolbar_border,
    );
    if resp.clicked() {
        super::super::super::gpu::TB_BTN_CLICKED.with(|f| f.set(true));
    }
    resp
}

// ─── TimeframeSelector ────────────────────────────────────────────────────────

/// Builder for the horizontal pill-row timeframe selector.
/// Returns `Option<usize>` — `Some(i)` when the user clicks a different tab.
///
/// ```ignore
/// if let Some(idx) = TimeframeSelector::new(&["1m","5m","15m","1h","1D"], active).theme(t).show(ui) {
///     active = idx;
/// }
/// ```
#[must_use = "TimeframeSelector must be shown with `.show(ui)` to render"]
pub struct TimeframeSelector<'a> {
    options: &'a [&'a str],
    active_idx: usize,
    accent: Color32,
    dim: Color32,
}

impl<'a> TimeframeSelector<'a> {
    pub fn new(options: &'a [&'a str], active_idx: usize) -> Self {
        let f = ft();
        Self {
            options,
            active_idx,
            accent: f.accent,
            dim: f.dim,
        }
    }
    pub fn theme(mut self, t: &super::super::super::gpu::Theme) -> Self {
        self.accent = t.accent;
        self.dim = t.dim;
        self
    }
    pub fn show(self, ui: &mut Ui) -> Option<usize> {
        let mut clicked = None;
        let pill_r = egui::CornerRadius::same(99);
        let prev_item_spacing = ui.spacing().item_spacing.x;
        ui.spacing_mut().item_spacing.x = gap_xs();
        let prev_pad = ui.spacing().button_padding;
        ui.spacing_mut().button_padding = egui::vec2(gap_md(), gap_xs());
        for (i, &label) in self.options.iter().enumerate() {
            let active = i == self.active_idx;
            let fg = if active { self.accent } else { self.dim };
            let (bg, border) = if active {
                (color_alpha(self.accent, alpha_tint()), color_alpha(self.accent, alpha_dim()))
            } else {
                (Color32::TRANSPARENT, Color32::TRANSPARENT)
            };
            let resp = ui.add(
                egui::Button::new(RichText::new(label).monospace().size(font_sm()).strong().color(fg))
                    .fill(bg)
                    .stroke(Stroke::new(stroke_thin(), border))
                    .corner_radius(pill_r)
                    .min_size(egui::vec2(0.0, 20.0)),
            );
            if resp.hovered() && !crate::design_tokens::is_inspect_mode() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            if resp.clicked() && i != self.active_idx {
                clicked = Some(i);
            }
        }
        ui.spacing_mut().button_padding = prev_pad;
        ui.spacing_mut().item_spacing.x = prev_item_spacing;
        clicked
    }
}

// ─── PaneHeaderAction ─────────────────────────────────────────────────────────

/// Builder for painter-positioned pane header action labels.
/// Uses `.show(ui, painter, rect)` because `impl Widget` cannot accept a
/// pre-existing `Painter` + `Rect` from the caller's layout pass.
///
/// ```ignore
/// let resp = PaneHeaderAction::new("Settings").active(true).theme(t)
///     .show(ui, &header_painter, action_rect);
/// ```
#[must_use = "PaneHeaderAction must be shown with `.show(ui, painter, rect)` to render"]
pub struct PaneHeaderAction<'a> {
    label: &'a str,
    active: bool,
    text_color: Color32,
    dim_color: Color32,
}

impl<'a> PaneHeaderAction<'a> {
    pub fn new(label: &'a str) -> Self {
        let f = ft();
        Self {
            label,
            active: false,
            text_color: f.text,
            dim_color: f.dim,
        }
    }
    pub fn active(mut self, v: bool) -> Self { self.active = v; self }
    pub fn text_color(mut self, c: Color32) -> Self { self.text_color = c; self }
    pub fn dim_color(mut self, c: Color32) -> Self { self.dim_color = c; self }
    pub fn theme(mut self, t: &super::super::super::gpu::Theme) -> Self {
        self.text_color = t.text;
        self.dim_color = t.dim;
        self
    }
    pub fn show(self, ui: &mut Ui, painter: &egui::Painter, rect: egui::Rect) -> Response {
        let resp = ui.allocate_rect(rect, egui::Sense::click());
        let fg = if self.active {
            self.text_color
        } else if resp.hovered() {
            self.text_color
        } else {
            self.dim_color.gamma_multiply(0.85)
        };
        painter.text(
            egui::pos2(rect.left(), rect.center().y),
            egui::Align2::LEFT_CENTER,
            self.label,
            egui::FontId::monospace(font_md()),
            fg,
        );
        if resp.hovered() && !crate::design_tokens::is_inspect_mode() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        resp
    }
}
