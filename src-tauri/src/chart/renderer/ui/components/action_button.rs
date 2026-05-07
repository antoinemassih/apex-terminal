//! Big action button (canonical builder + legacy helper), side-pane action,
//! brand CTA. Defines `ActionTier`, `ActionSize`, `ActionButton`.

use super::super::style::*;
use egui::{self, Color32, Response, RichText, Stroke, Ui};

// ─── Helper: luminance-aware contrast color ──────────────────────────────────

#[inline]
fn ds_contrast_fg(bg: Color32) -> Color32 {
    let lum = 0.299 * bg.r() as f32 + 0.587 * bg.g() as f32 + 0.114 * bg.b() as f32;
    if lum > 140.0 { Color32::from_rgb(20, 20, 24) } else { Color32::from_rgb(240, 240, 244) }
}

// ─── BigActionButton ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionTier {
    Primary,
    Destructive,
    Secondary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionSize { Small, Medium, Large }

/// **Canonical action button — builder + `impl Widget` API.**
///
/// This is the modern entry point. Use `ui.add(ActionButton::new("BUY").primary().large().theme(t))`
/// instead of the older `big_action_btn(...)` positional-arg helper. Adding new
/// knobs (e.g. `.icon(...)`, `.loading(true)`) becomes non-breaking, and the
/// fluent API documents the intent at the call site.
///
/// Defaults: `Primary` tier, `Medium` size, not disabled. You must call
/// `.theme(t)` (or `.palette(...)`) so the button has palette colors.
#[must_use = "ActionButton must be added with `ui.add(...)` to render"]
#[deprecated(note = "use ui_kit::widgets::Button")]
pub struct ActionButton<'a> {
    label: &'a str,
    tier: ActionTier,
    size: ActionSize,
    accent: Color32,
    bear: Color32,
    dim: Color32,
    disabled: bool,
    palette_set: bool,
}

impl<'a> ActionButton<'a> {
    /// New action button with the given label. Defaults to Primary/Medium/enabled.
    pub fn new(label: &'a str) -> Self {
        Self {
            label,
            tier: ActionTier::Primary,
            size: ActionSize::Medium,
            accent: Color32::from_rgb(120, 140, 220),
            bear: Color32::from_rgb(220, 80, 90),
            dim: Color32::from_rgb(120, 120, 130),
            disabled: false,
            palette_set: false,
        }
    }
    pub fn tier(mut self, t: ActionTier) -> Self { self.tier = t; self }
    pub fn size(mut self, s: ActionSize) -> Self { self.size = s; self }
    pub fn primary(mut self) -> Self { self.tier = ActionTier::Primary; self }
    pub fn destructive(mut self) -> Self { self.tier = ActionTier::Destructive; self }
    pub fn secondary(mut self) -> Self { self.tier = ActionTier::Secondary; self }
    pub fn small(mut self) -> Self { self.size = ActionSize::Small; self }
    pub fn medium(mut self) -> Self { self.size = ActionSize::Medium; self }
    pub fn large(mut self) -> Self { self.size = ActionSize::Large; self }
    pub fn disabled(mut self, d: bool) -> Self { self.disabled = d; self }
    /// Supply explicit palette colors. Prefer `.theme(t)` when you have a Theme handy.
    pub fn palette(mut self, accent: Color32, bear: Color32, dim: Color32) -> Self {
        self.accent = accent; self.bear = bear; self.dim = dim;
        self.palette_set = true;
        self
    }
    /// Pull palette colors from a Theme — the common path.
    pub fn theme(self, t: &super::super::super::gpu::Theme) -> Self {
        self.palette(t.accent, t.bear, t.dim)
    }
}

impl<'a> egui::Widget for ActionButton<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        // Body is identical to `big_action_btn` — they share visual primitives.
        // Once all call sites migrate, big_action_btn will delegate here too.
        let height: f32 = match self.size {
            ActionSize::Small => 24.0, ActionSize::Medium => 32.0, ActionSize::Large => 40.0,
        };
        let font_size: f32 = match self.size {
            ActionSize::Small => font_sm(), ActionSize::Medium => font_md(), ActionSize::Large => font_lg(),
        };
        let (bg, fg, border) = if self.disabled {
            (color_alpha(self.dim, alpha_subtle()),
             color_alpha(self.dim, alpha_dim()),
             color_alpha(self.dim, alpha_line()))
        } else {
            match self.tier {
                ActionTier::Primary =>
                    (self.accent, ds_contrast_fg(self.accent), color_alpha(self.accent, alpha_active())),
                ActionTier::Destructive =>
                    (self.bear, ds_contrast_fg(self.bear), color_alpha(self.bear, alpha_active())),
                ActionTier::Secondary =>
                    (color_alpha(self.accent, alpha_faint()), self.accent, color_alpha(self.accent, alpha_muted())),
            }
        };
        let prev_pad = ui.spacing().button_padding;
        ui.spacing_mut().button_padding = egui::vec2(gap_xl(), gap_xs());
        let resp = ui.add_enabled(
            !self.disabled,
            egui::Button::new(RichText::new(self.label).size(font_size).strong().color(fg))
                .fill(bg)
                .stroke(Stroke::new(stroke_thin(), border))
                .corner_radius(radius_md())
                .min_size(egui::vec2(0.0, height)),
        );
        ui.spacing_mut().button_padding = prev_pad;
        let inspect = crate::design_tokens::is_inspect_mode();
        let interactive = !self.disabled && !inspect;
        if resp.hovered() && interactive {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        use super::motion;
        let hover_id = resp.id.with("action_button_hover");
        let hover_t = motion::ease_bool(ui.ctx(), hover_id, resp.hovered() && interactive, motion::FAST);
        if hover_t > 0.001 {
            ui.painter().rect_filled(resp.rect, radius_md(),
                motion::fade_in(color_alpha(Color32::WHITE, 12), hover_t));
        }
        let _ = self.palette_set;
        resp
    }
}

/// Legacy positional-arg helper. Prefer [`ActionButton`] for new code:
/// `ui.add(ActionButton::new("BUY").primary().large().theme(t))`.
pub fn big_action_btn(
    ui: &mut Ui,
    label: &str,
    tier: ActionTier,
    size: ActionSize,
    accent: Color32,
    bear: Color32,
    dim: Color32,
    disabled: bool,
) -> Response {
    let height: f32 = match size { ActionSize::Small => 24.0, ActionSize::Medium => 32.0, ActionSize::Large => 40.0 };
    let font_size: f32 = match size { ActionSize::Small => font_sm(), ActionSize::Medium => font_md(), ActionSize::Large => font_lg() };
    let (bg, fg, border) = if disabled {
        (color_alpha(dim, alpha_subtle()), color_alpha(dim, alpha_dim()), color_alpha(dim, alpha_line()))
    } else {
        match tier {
            ActionTier::Primary => (accent, ds_contrast_fg(accent), color_alpha(accent, alpha_active())),
            ActionTier::Destructive => (bear, ds_contrast_fg(bear), color_alpha(bear, alpha_active())),
            ActionTier::Secondary => (color_alpha(accent, alpha_faint()), accent, color_alpha(accent, alpha_muted())),
        }
    };
    let prev_pad = ui.spacing().button_padding;
    ui.spacing_mut().button_padding = egui::vec2(gap_xl(), gap_xs());
    let resp = ui.add_enabled(
        !disabled,
        egui::Button::new(RichText::new(label).size(font_size).strong().color(fg))
            .fill(bg)
            .stroke(Stroke::new(stroke_thin(), border))
            .corner_radius(radius_md())
            .min_size(egui::vec2(0.0, height)),
    );
    ui.spacing_mut().button_padding = prev_pad;
    let inspect = crate::design_tokens::is_inspect_mode();
    let interactive = !disabled && !inspect;
    if resp.hovered() && interactive {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    use super::motion;
    let hover_id = resp.id.with("big_action_btn_hover");
    let hover_t = motion::ease_bool(ui.ctx(), hover_id, resp.hovered() && interactive, motion::FAST);
    if hover_t > 0.001 {
        ui.painter().rect_filled(resp.rect, radius_md(),
            motion::fade_in(color_alpha(Color32::WHITE, 12), hover_t));
    }
    resp
}

// ─── SidePaneActionButton ────────────────────────────────────────────────────

#[allow(unused_variables)]
pub fn side_pane_action_btn(
    ui: &mut Ui,
    icon: Option<&str>,
    label: &str,
    accent: Color32,
    dim: Color32,
) -> Response {
    let fg = accent;
    let bg = color_alpha(accent, alpha_soft());
    let border = color_alpha(accent, alpha_dim());
    let display = match icon {
        Some(ic) => format!("{} {}", ic, label),
        None => label.to_owned(),
    };
    let prev_pad = ui.spacing().button_padding;
    ui.spacing_mut().button_padding = egui::vec2(gap_lg(), gap_xs());
    let resp = ui.add(
        egui::Button::new(RichText::new(display).size(font_sm()).strong().color(fg))
            .fill(bg)
            .stroke(Stroke::new(stroke_thin(), border))
            .corner_radius(radius_sm())
            .min_size(egui::vec2(0.0, 22.0)),
    );
    ui.spacing_mut().button_padding = prev_pad;
    let inspect = crate::design_tokens::is_inspect_mode();
    if resp.hovered() && !inspect {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    use super::motion;
    let hover_id = resp.id.with("side_pane_action_btn_hover");
    let hover_t = motion::ease_bool(ui.ctx(), hover_id, resp.hovered() && !inspect, motion::FAST);
    if hover_t > 0.001 {
        ui.painter().rect_filled(resp.rect, radius_sm(),
            motion::fade_in(color_alpha(accent, alpha_faint()), hover_t));
    }
    resp
}

// ─── Brand CTA ────────────────────────────────────────────────────────────────

/// Brand-color CTA — like `big_action_btn` but with an explicit brand color
/// (e.g. Discord blurple from `palette.discord`). Uses the same height,
/// padding, font, radius, and border as `big_action_btn` so brand CTAs feel
/// like first-class action buttons in the same family.
pub fn brand_cta_button(
    ui: &mut Ui,
    label: &str,
    brand_color: Color32,
    fg_color: Color32,
    size: ActionSize,
    disabled: bool,
) -> Response {
    let height: f32 = match size { ActionSize::Small => 24.0, ActionSize::Medium => 32.0, ActionSize::Large => 40.0 };
    let font_size: f32 = match size { ActionSize::Small => font_sm(), ActionSize::Medium => font_md(), ActionSize::Large => font_lg() };
    let prev_pad = ui.spacing().button_padding;
    ui.spacing_mut().button_padding = egui::vec2(gap_xl(), gap_xs());
    let resp = ui.add_enabled(
        !disabled,
        egui::Button::new(RichText::new(label).size(font_size).strong().color(fg_color))
            .fill(brand_color)
            .stroke(Stroke::new(stroke_thin(), color_alpha(brand_color, alpha_active())))
            .corner_radius(radius_md())
            .min_size(egui::vec2(0.0, height)),
    );
    ui.spacing_mut().button_padding = prev_pad;
    let inspect = crate::design_tokens::is_inspect_mode();
    let interactive = !disabled && !inspect;
    if resp.hovered() && interactive {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    use super::motion;
    let hover_id = resp.id.with("brand_cta_btn_hover");
    let hover_t = motion::ease_bool(ui.ctx(), hover_id, resp.hovered() && interactive, motion::FAST);
    if hover_t > 0.001 {
        ui.painter().rect_filled(resp.rect, radius_md(),
            motion::fade_in(color_alpha(Color32::WHITE, 12), hover_t));
    }
    resp
}
