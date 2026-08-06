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
pub mod chart_controls;
pub mod dropdowns;
pub mod window_controls;
pub mod ticker_strip;
pub mod toolnav;
pub mod alert_feed;
pub mod workspace_rail;

use egui::{Color32, Response, RichText, Stroke, Ui, Widget};
use super::super::style::*;
use crate::ui_kit::widgets::Button as KitButton;
use crate::ui_kit::widgets::tokens::{Variant as KitVariant, Size as KitSize};

#[inline(always)]
fn ambient(ctx: &egui::Context) -> super::super::super::gpu::Theme {
    crate::chart_renderer::theme_impl::active_theme(ctx)
}

// ─── toolbar_btn (free function) ──────────────────────────────────────────────

/// Top-application-toolbar button. Motion-driven hover + active fades (FAST /
/// MED) with an instant press-snap darken; flags `gpu::TB_BTN_CLICKED` on
/// click so the window-drag handler ignores the click on the same frame.
pub fn toolbar_btn(
    ui: &mut Ui,
    label: &str,
    active: bool,
    t: &super::super::super::gpu::Theme,
) -> Response {
    use crate::ui_kit::widgets::Button;

    // Phosphor PUA glyphs (icon-only labels) render via Button::icon so the
    // glyph paints at font_size * 1.25, matching the larger visual weight of
    // the legacy `font_md` icon path. Plain text labels use Button::new.
    let is_icon_only = !label.is_empty() && label.chars().all(|c| {
        let cp = c as u32;
        (0xE000..=0xF8FF).contains(&cp)
            || (0xF0000..=0x10FFFF).contains(&cp)
            || c.is_ascii_whitespace()
            || c.is_ascii_digit()
    });

    // Nav/toolbar buttons are GHOST (transparent) so there is exactly ONE
    // highlight system: the Ghost variant's own hover/active fill. Previously
    // this stacked status-mode bg + an accent-pill fill + a bespoke nav column
    // tint = the "multiple/weird" highlights.
    // Active = accent FOREGROUND only (no competing bg fill).
    let mut btn = if is_icon_only {
        Button::icon(label).glyph_size(16.0).placement(crate::ui_kit::widgets::icon_placement::IconPlacement::Toolbar)
    } else {
        Button::new(label)
    }
    .variant(crate::ui_kit::widgets::tokens::Variant::Ghost)
    .active(active);

    // ONE height for every control in the toolbar row. Icon buttons resolved
    // from `Size::Md` while label chips (IBKR / $ / CLOSED / TPS) picked up
    // egui's button padding, so the filled broker pill sat visibly taller than
    // the outlined chips next to it. `toolbar_control_h()` is the themed,
    // density-aware source they now share.
    btn = btn.min_size(egui::vec2(
        0.0,
        crate::chart_renderer::ui::style::toolbar_control_h(),
    ));

    if active {
        btn = btn.fg(t.accent);
    }

    let resp = btn.show(ui, t);

    if resp.hovered() && !crate::design_tokens::is_inspect_mode() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

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
    accent: Option<Color32>,
    dim: Option<Color32>,
}

impl<'a> TimeframeSelector<'a> {
    pub fn new(options: &'a [&'a str], active_idx: usize) -> Self {
        Self {
            options,
            active_idx,
            accent: None,
            dim: None,
        }
    }
    pub fn theme(mut self, t: &super::super::super::gpu::Theme) -> Self {
        self.accent = Some(t.accent);
        self.dim = Some(t.dim);
        self
    }
    pub fn show(self, ui: &mut Ui) -> Option<usize> {
        let amb = ambient(ui.ctx());
        let accent_c = self.accent.unwrap_or(amb.accent);
        let dim_c = self.dim.unwrap_or(amb.dim);
        let mut clicked = None;
        let pill_r = egui::CornerRadius::same(radius_pill() as u8);
        let prev_item_spacing = ui.spacing().item_spacing.x;
        ui.spacing_mut().item_spacing.x = gap_xs();
        let prev_pad = ui.spacing().button_padding;
        ui.spacing_mut().button_padding = egui::vec2(gap_md(), gap_xs());
        let theme = ambient(ui.ctx());
        for (i, &label) in self.options.iter().enumerate() {
            let active = i == self.active_idx;
            let resp = KitButton::toggle(label, active).size(KitSize::Sm)
                .min_size(egui::vec2(0.0, row_height_default()))
                .show(ui, &theme);
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
    text_color: Option<Color32>,
    dim_color: Option<Color32>,
}

impl<'a> PaneHeaderAction<'a> {
    pub fn new(label: &'a str) -> Self {
        Self {
            label,
            active: false,
            text_color: None,
            dim_color: None,
        }
    }
    pub fn active(mut self, v: bool) -> Self { self.active = v; self }
    pub fn text_color(mut self, c: Color32) -> Self { self.text_color = Some(c); self }
    pub fn dim_color(mut self, c: Color32) -> Self { self.dim_color = Some(c); self }
    pub fn theme(mut self, t: &super::super::super::gpu::Theme) -> Self {
        self.text_color = Some(t.text);
        self.dim_color = Some(t.dim);
        self
    }
    pub fn show(self, ui: &mut Ui, painter: &egui::Painter, rect: egui::Rect) -> Response {
        let amb = ambient(ui.ctx());
        let text_c = self.text_color.unwrap_or(amb.text);
        let dim_c = self.dim_color.unwrap_or(amb.dim);
        let resp = ui.allocate_rect(rect, egui::Sense::click());
        let fg = if self.active {
            text_c
        } else if resp.hovered() {
            text_c
        } else {
            color_subtle(dim_c)
        };
        painter.text(
            egui::pos2(rect.left(), rect.center().y),
            egui::Align2::LEFT_CENTER,
            self.label,
            crate::ui_kit::style::mono_md(),
            fg,
        );
        if resp.hovered() && !crate::design_tokens::is_inspect_mode() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        resp
    }
}
