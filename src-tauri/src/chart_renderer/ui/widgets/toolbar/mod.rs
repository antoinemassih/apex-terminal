//! Builder + impl Widget primitives — toolbar family.
//! See ui/widgets/mod.rs for the rationale.
//!
//! `top_nav` — the top navigation toolbar panel, extracted from `gpu.rs`.

#![allow(dead_code, unused_imports)]

pub mod top_nav;

use egui::{Color32, Response, RichText, Stroke, Ui, Widget};
use super::super::style::*;

fn ft() -> &'static super::super::super::gpu::Theme {
    &super::super::super::gpu::THEMES[0]
}

// Re-export the enums from components_extra so call-sites can use either path.
pub use super::super::components_extra::{TopNavTreatment, TopNavToggleSize, PaneTabStyle};

// ─── ToolbarBtn ───────────────────────────────────────────────────────────────

/// Builder + `impl Widget` for the top-application-toolbar buttons. Delegates
/// to `style::tb_btn` for pixel-exact parity with the legacy renderer, and
/// flags `gpu::TB_BTN_CLICKED` on click so the window-drag handler ignores
/// the click on the same frame.
///
/// ```ignore
/// if ui.add(ToolbarBtn::new("Settings").active(open).theme(t))
///     .on_hover_text("Open settings").clicked() { open = !open; }
/// ```
#[must_use = "ToolbarBtn must be added with `ui.add(...)` to render"]
pub struct ToolbarBtn<'a> {
    label: &'a str,
    active: bool,
    accent: Color32,
    dim: Color32,
    toolbar_bg: Color32,
    toolbar_border: Color32,
}

impl<'a> ToolbarBtn<'a> {
    pub fn new(label: &'a str) -> Self {
        let f = ft();
        Self {
            label,
            active: false,
            accent: f.accent,
            dim: f.dim,
            toolbar_bg: f.toolbar_bg,
            toolbar_border: f.toolbar_border,
        }
    }
    pub fn active(mut self, v: bool) -> Self { self.active = v; self }
    pub fn theme(mut self, t: &super::super::super::gpu::Theme) -> Self {
        self.accent = t.accent;
        self.dim = t.dim;
        self.toolbar_bg = t.toolbar_bg;
        self.toolbar_border = t.toolbar_border;
        self
    }
}

impl<'a> Widget for ToolbarBtn<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        let resp = super::super::style::tb_btn(
            ui, self.label, self.active,
            self.accent, self.dim, self.toolbar_bg, self.toolbar_border,
        );
        if resp.clicked() {
            super::super::super::gpu::TB_BTN_CLICKED.with(|f| f.set(true));
        }
        resp
    }
}

// ─── TopNavBtn ────────────────────────────────────────────────────────────────

/// Builder + `impl Widget` for top-navigation tab buttons.
///
/// ```ignore
/// ui.add(TopNavBtn::new("Charts").active(true).underline().theme(t));
/// ```
#[must_use = "TopNavBtn must be added with `ui.add(...)` to render"]
pub struct TopNavBtn<'a> {
    label: &'a str,
    active: bool,
    treatment: TopNavTreatment,
    accent: Color32,
    dim: Color32,
}

impl<'a> TopNavBtn<'a> {
    pub fn new(label: &'a str) -> Self {
        let f = ft();
        Self {
            label,
            active: false,
            treatment: TopNavTreatment::Raised,
            accent: f.accent,
            dim: f.dim,
        }
    }
    pub fn active(mut self, v: bool) -> Self { self.active = v; self }
    pub fn treatment(mut self, t: TopNavTreatment) -> Self { self.treatment = t; self }
    pub fn raised(mut self) -> Self { self.treatment = TopNavTreatment::Raised; self }
    pub fn underline(mut self) -> Self { self.treatment = TopNavTreatment::Underline; self }
    pub fn soft_pill(mut self) -> Self { self.treatment = TopNavTreatment::SoftPill; self }
    pub fn theme(mut self, t: &super::super::super::gpu::Theme) -> Self {
        self.accent = t.accent;
        self.dim = t.dim;
        self
    }
}

impl<'a> Widget for TopNavBtn<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        let fg = if self.active { self.accent } else { self.dim };
        let (bg, border) = match self.treatment {
            TopNavTreatment::Raised => {
                let b = if self.active { color_alpha(self.accent, alpha_tint()) } else { Color32::TRANSPARENT };
                let s = if self.active { color_alpha(self.accent, alpha_line()) } else { Color32::TRANSPARENT };
                (b, s)
            }
            TopNavTreatment::Underline => (Color32::TRANSPARENT, Color32::TRANSPARENT),
            TopNavTreatment::SoftPill => {
                let b = if self.active { color_alpha(self.accent, alpha_soft()) } else { Color32::TRANSPARENT };
                (b, Color32::TRANSPARENT)
            }
        };
        let prev_pad = ui.spacing().button_padding;
        ui.spacing_mut().button_padding = egui::vec2(gap_lg(), gap_md());
        let resp = ui.add(
            egui::Button::new(RichText::new(self.label).size(font_md()).strong().color(fg))
                .fill(bg)
                .stroke(Stroke::new(stroke_thin(), border))
                .corner_radius(radius_sm())
                .min_size(egui::vec2(0.0, gap_3xl())),
        );
        ui.spacing_mut().button_padding = prev_pad;
        if self.active && self.treatment == TopNavTreatment::Underline {
            let r = resp.rect;
            ui.painter().line_segment(
                [egui::pos2(r.left() + gap_sm(), r.bottom()), egui::pos2(r.right() - gap_sm(), r.bottom())],
                Stroke::new(stroke_std(), self.accent),
            );
        }
        if resp.hovered() && !self.active && !crate::design_tokens::is_inspect_mode() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            ui.painter().rect_filled(resp.rect, radius_sm(), color_alpha(self.accent, alpha_faint()));
        }
        resp
    }
}

// ─── TopNavToggle ─────────────────────────────────────────────────────────────

/// Builder + `impl Widget` for top-navigation icon toggle buttons.
///
/// ```ignore
/// ui.add(TopNavToggle::new("⚙").active(settings_open).medium().theme(t));
/// ```
#[must_use = "TopNavToggle must be added with `ui.add(...)` to render"]
pub struct TopNavToggle<'a> {
    icon: &'a str,
    active: bool,
    size: TopNavToggleSize,
    accent: Color32,
    dim: Color32,
}

impl<'a> TopNavToggle<'a> {
    pub fn new(icon: &'a str) -> Self {
        let f = ft();
        Self {
            icon,
            active: false,
            size: TopNavToggleSize::Small,
            accent: f.accent,
            dim: f.dim,
        }
    }
    pub fn active(mut self, v: bool) -> Self { self.active = v; self }
    pub fn size(mut self, s: TopNavToggleSize) -> Self { self.size = s; self }
    pub fn small(mut self) -> Self { self.size = TopNavToggleSize::Small; self }
    pub fn medium(mut self) -> Self { self.size = TopNavToggleSize::Medium; self }
    pub fn theme(mut self, t: &super::super::super::gpu::Theme) -> Self {
        self.accent = t.accent;
        self.dim = t.dim;
        self
    }
}

impl<'a> Widget for TopNavToggle<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        let side = match self.size { TopNavToggleSize::Small => 22.0_f32, TopNavToggleSize::Medium => 28.0_f32 };
        let font = match self.size { TopNavToggleSize::Small => font_md(), TopNavToggleSize::Medium => font_lg() };
        let fg = if self.active { self.accent } else { self.dim };
        let bg = if self.active { color_alpha(self.accent, alpha_tint()) } else { Color32::TRANSPARENT };
        let border = if self.active { color_alpha(self.accent, alpha_muted()) } else { color_alpha(self.dim, alpha_subtle()) };
        let prev_pad = ui.spacing().button_padding;
        ui.spacing_mut().button_padding = egui::vec2(0.0, 0.0);
        let resp = ui.add(
            egui::Button::new(RichText::new(self.icon).size(font).color(fg))
                .fill(bg)
                .stroke(Stroke::new(stroke_thin(), border))
                .corner_radius(radius_sm())
                .min_size(egui::vec2(side, side)),
        );
        ui.spacing_mut().button_padding = prev_pad;
        if resp.hovered() && !crate::design_tokens::is_inspect_mode() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            if !self.active {
                ui.painter().rect_filled(resp.rect, radius_sm(), color_alpha(self.accent, alpha_ghost()));
            }
        }
        resp
    }
}

// ─── PaneTabBtn ───────────────────────────────────────────────────────────────

/// Builder + `impl Widget` for pane-level tab buttons.
///
/// ```ignore
/// ui.add(PaneTabBtn::new("Orders").icon(Some("📋")).active(true).filled().theme(t));
/// ```
#[must_use = "PaneTabBtn must be added with `ui.add(...)` to render"]
pub struct PaneTabBtn<'a> {
    label: &'a str,
    icon: Option<&'a str>,
    active: bool,
    style: PaneTabStyle,
    accent: Color32,
    dim: Color32,
}

impl<'a> PaneTabBtn<'a> {
    pub fn new(label: &'a str) -> Self {
        let f = ft();
        Self {
            label,
            icon: None,
            active: false,
            style: PaneTabStyle::Underline,
            accent: f.accent,
            dim: f.dim,
        }
    }
    pub fn icon(mut self, ic: Option<&'a str>) -> Self { self.icon = ic; self }
    pub fn active(mut self, v: bool) -> Self { self.active = v; self }
    pub fn style(mut self, s: PaneTabStyle) -> Self { self.style = s; self }
    pub fn underline(mut self) -> Self { self.style = PaneTabStyle::Underline; self }
    pub fn filled(mut self) -> Self { self.style = PaneTabStyle::Filled; self }
    pub fn border(mut self) -> Self { self.style = PaneTabStyle::Border; self }
    pub fn theme(mut self, t: &super::super::super::gpu::Theme) -> Self {
        self.accent = t.accent;
        self.dim = t.dim;
        self
    }
}

impl<'a> Widget for PaneTabBtn<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        let text = match self.icon {
            Some(ic) => format!("{} {}", ic, self.label),
            None => self.label.to_owned(),
        };
        let fg = if self.active { self.accent } else { self.dim };
        let (bg, border) = match (self.active, self.style) {
            (true, PaneTabStyle::Filled) => (color_alpha(self.accent, alpha_tint()), color_alpha(self.accent, alpha_active())),
            (true, PaneTabStyle::Border) => (Color32::TRANSPARENT, color_alpha(self.accent, alpha_active())),
            _ => (Color32::TRANSPARENT, Color32::TRANSPARENT),
        };
        let cr = egui::CornerRadius::same(radius_sm() as u8);
        let prev_pad = ui.spacing().button_padding;
        ui.spacing_mut().button_padding = egui::vec2(gap_md(), gap_sm());
        let resp = ui.add(
            egui::Button::new(RichText::new(&text).monospace().size(font_sm()).color(fg))
                .fill(bg)
                .stroke(Stroke::new(stroke_thin(), border))
                .corner_radius(cr)
                .min_size(egui::vec2(0.0, 22.0)),
        );
        ui.spacing_mut().button_padding = prev_pad;
        if resp.hovered() && !crate::design_tokens::is_inspect_mode() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        if self.active && self.style == PaneTabStyle::Underline {
            let r = resp.rect;
            ui.painter().line_segment(
                [egui::pos2(r.left() + 3.0, r.bottom() + 1.0), egui::pos2(r.right() - 3.0, r.bottom() + 1.0)],
                Stroke::new(stroke_thick(), color_alpha(self.accent, alpha_strong())),
            );
        }
        resp
    }
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
