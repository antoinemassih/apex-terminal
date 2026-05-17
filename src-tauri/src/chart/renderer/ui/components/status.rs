//! Builder + impl Widget primitives — status / feedback family.
//!
//! Adds builder wrappers for status dots, spinners, progress bars/rings,
//! skeleton placeholders, toasts, notification badges, connection
//! indicators, and trend arrows. **NEW additions only** — no migration of
//! existing call sites (Wave 5).
//!
//! Reuses `widgets::{text, frames, pills}` plus `style::*` primitives where
//! possible. Chart paint is sacred — `chart_widgets.rs` was read for visual
//! reference only.
//!
//! All builders implement `impl Widget` (or expose `.show(ui)` when they
//! return non-`Response` data) and follow the ambient design-token rules.

#![allow(dead_code, unused_imports)]

use egui::{Color32, Pos2, Rect, Response, RichText, Sense, Stroke, Ui, Vec2, Widget};
use super::super::style::*;

type Theme = crate::chart_renderer::gpu::Theme;

#[inline(always)]
fn ambient(ctx: &egui::Context) -> &'static Theme {
    crate::ui_kit::widgets::theme::active_theme(ctx)
}

// ─── Shared size enum ─────────────────────────────────────────────────────────

/// Tri-size knob shared by [`Spinner`] and other loading primitives.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum LoadSize { Sm, Md, Lg }

impl LoadSize {
    fn px(self) -> f32 { match self { LoadSize::Sm => 10.0, LoadSize::Md => 14.0, LoadSize::Lg => 20.0 } }
}

// ─── StatusDot ────────────────────────────────────────────────────────────────

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum DotVariant { Success, Danger, Warning, Neutral, Custom }

/// Small filled circle that conveys binary / categorical status. An optional
/// label is laid out to the right of the dot.
///
/// ```ignore
/// ui.add(StatusDot::new().success().label("Connected").theme(t));
/// ui.add(StatusDot::new().danger().pulsing().theme(t));
/// ```
#[must_use = "StatusDot must be added with `ui.add(...)` to render"]
pub struct StatusDot<'a> {
    label: Option<&'a str>,
    variant: DotVariant,
    color: Option<Color32>,
    label_color: Option<Color32>,
    pulsing: bool,
    radius: f32,
}

impl<'a> StatusDot<'a> {
    pub fn new() -> Self {
        Self {
            label: None,
            variant: DotVariant::Neutral,
            color: None,
            label_color: None,
            pulsing: false,
            radius: 3.5,
        }
    }
    pub fn label(mut self, s: &'a str) -> Self { self.label = Some(s); self }
    pub fn radius(mut self, r: f32) -> Self { self.radius = r; self }
    pub fn pulsing(mut self) -> Self { self.pulsing = true; self }
    pub fn color(mut self, c: Color32) -> Self { self.color = Some(c); self.variant = DotVariant::Custom; self }
    pub fn success(mut self) -> Self { self.variant = DotVariant::Success; self }
    pub fn danger(mut self)  -> Self { self.variant = DotVariant::Danger;  self }
    pub fn warning(mut self) -> Self { self.variant = DotVariant::Warning; self }
    pub fn neutral(mut self) -> Self { self.variant = DotVariant::Neutral; self }
    pub fn theme(mut self, t: &Theme) -> Self {
        self.label_color = Some(t.text);
        self.color = Some(match self.variant {
            DotVariant::Success => t.bull,
            DotVariant::Danger  => t.bear,
            DotVariant::Warning => t.warn,
            DotVariant::Neutral => t.dim,
            DotVariant::Custom  => self.color.unwrap_or(t.dim),
        });
        self
    }
}

impl<'a> Default for StatusDot<'a> {
    fn default() -> Self { Self::new() }
}

impl<'a> Widget for StatusDot<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        let amb = ambient(ui.ctx());
        let base_color = self.color.unwrap_or_else(|| match self.variant {
            DotVariant::Success => amb.bull,
            DotVariant::Danger  => amb.bear,
            DotVariant::Warning => amb.warn,
            DotVariant::Neutral => amb.dim,
            DotVariant::Custom  => amb.dim,
        });
        let label_color = self.label_color.unwrap_or(amb.text);
        let r = self.radius;
        let label_h = font_sm() + 2.0;
        let dot_box = Vec2::new(r * 2.0 + 2.0, label_h.max(r * 2.0 + 2.0));
        let label_w = self.label.map(|s| s.len() as f32 * font_sm() * 0.6).unwrap_or(0.0);
        let total = Vec2::new(dot_box.x + if self.label.is_some() { label_w + gap_sm() } else { 0.0 }, dot_box.y);
        let (rect, resp) = ui.allocate_exact_size(total, Sense::hover());
        let painter = ui.painter();

        // Pulsing alpha animation
        let mut color = base_color;
        if self.pulsing {
            let t = ui.ctx().input(|i| i.time);
            let phase = (t.sin() * 0.5 + 0.5) as f32;
            let a = (alpha_dim() as f32 + phase * (255.0 - alpha_dim() as f32)) as u8;
            color = color_alpha(base_color, a);
            ui.ctx().request_repaint();
            // Outer halo
            painter.circle_filled(
                Pos2::new(rect.left() + r + 1.0, rect.center().y),
                r + 2.0,
                color_alpha(base_color, (alpha_soft() as f32 * (1.0 - phase)) as u8),
            );
        }
        painter.circle_filled(Pos2::new(rect.left() + r + 1.0, rect.center().y), r, color);

        if let Some(s) = self.label {
            let text_pos = Pos2::new(rect.left() + dot_box.x + gap_sm(), rect.center().y);
            painter.text(
                text_pos,
                egui::Align2::LEFT_CENTER,
                s,
                egui::FontId::monospace(font_sm()),
                label_color,
            );
        }
        resp
    }
}

// ─── Spinner ──────────────────────────────────────────────────────────────────

/// Animated rotating arc loader.
///
/// ```ignore
/// ui.add(Spinner::new().md().theme(t));
/// ```
#[must_use = "Spinner must be added with `ui.add(...)` to render"]
pub struct Spinner {
    size: LoadSize,
    color: Option<Color32>,
    width: f32,
}

impl Spinner {
    pub fn new() -> Self {
        Self { size: LoadSize::Md, color: None, width: 1.5 }
    }
    pub fn size(mut self, s: LoadSize) -> Self { self.size = s; self }
    pub fn sm(mut self) -> Self { self.size = LoadSize::Sm; self }
    pub fn md(mut self) -> Self { self.size = LoadSize::Md; self }
    pub fn lg(mut self) -> Self { self.size = LoadSize::Lg; self }
    pub fn color(mut self, c: Color32) -> Self { self.color = Some(c); self }
    pub fn theme(mut self, t: &Theme) -> Self { self.color = Some(t.accent); self }
}

impl Default for Spinner {
    fn default() -> Self { Self::new() }
}

impl Widget for Spinner {
    fn ui(self, ui: &mut Ui) -> Response {
        let color = self.color.unwrap_or(ambient(ui.ctx()).accent);
        let d = self.size.px();
        let (rect, resp) = ui.allocate_exact_size(Vec2::splat(d + 2.0), Sense::hover());
        let center = rect.center();
        let radius = d * 0.5;
        let t = ui.ctx().input(|i| i.time) as f32;
        ui.ctx().request_repaint();

        // Arc: ~270° sweep rotating
        let segments = 24;
        let sweep = std::f32::consts::TAU * 0.75;
        let start = t * 4.0;
        let painter = ui.painter();
        for i in 0..segments {
            let a0 = start + sweep * (i as f32) / (segments as f32);
            let a1 = start + sweep * ((i + 1) as f32) / (segments as f32);
            let p0 = center + Vec2::new(a0.cos(), a0.sin()) * radius;
            let p1 = center + Vec2::new(a1.cos(), a1.sin()) * radius;
            // Fade older tail of the arc
            let frac = i as f32 / segments as f32;
            let alpha = (alpha_soft() as f32 + frac * (alpha_active() as f32 - alpha_soft() as f32)) as u8;
            painter.line_segment([p0, p1], Stroke::new(self.width, color_alpha(color, alpha)));
        }
        resp
    }
}

// ─── ProgressBar ──────────────────────────────────────────────────────────────

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum BarVariant { Thin, Thick, Striped }

/// Horizontal progress bar with optional label.
///
/// ```ignore
/// ui.add(ProgressBar::new(0.42).label("Loading…").thick().theme(t));
/// ```
#[must_use = "ProgressBar must be added with `ui.add(...)` to render"]
pub struct ProgressBar<'a> {
    progress: f32,
    label: Option<&'a str>,
    variant: BarVariant,
    fill: Option<Color32>,
    track: Option<Color32>,
    text_color: Option<Color32>,
    width: Option<f32>,
}

impl<'a> ProgressBar<'a> {
    pub fn new(progress: f32) -> Self {
        Self {
            progress: progress.clamp(0.0, 1.0),
            label: None,
            variant: BarVariant::Thin,
            fill: None,
            track: None,
            text_color: None,
            width: None,
        }
    }
    pub fn label(mut self, s: &'a str) -> Self { self.label = Some(s); self }
    pub fn width(mut self, w: f32) -> Self { self.width = Some(w); self }
    pub fn thin(mut self)    -> Self { self.variant = BarVariant::Thin;    self }
    pub fn thick(mut self)   -> Self { self.variant = BarVariant::Thick;   self }
    pub fn striped(mut self) -> Self { self.variant = BarVariant::Striped; self }
    pub fn fill(mut self, c: Color32) -> Self { self.fill = Some(c); self }
    pub fn theme(mut self, t: &Theme) -> Self {
        self.fill = Some(t.accent);
        self.track = Some(color_alpha(t.toolbar_border, alpha_strong()));
        self.text_color = Some(t.text);
        self
    }
}

impl<'a> Widget for ProgressBar<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        let amb = ambient(ui.ctx());
        let fill = self.fill.unwrap_or(amb.accent);
        let track = self.track.unwrap_or(color_alpha(amb.toolbar_border, alpha_strong()));
        let text_color = self.text_color.unwrap_or(amb.text);
        let h = match self.variant { BarVariant::Thin => 3.0, BarVariant::Thick => 8.0, BarVariant::Striped => 6.0 };
        let w = self.width.unwrap_or_else(|| ui.available_width().max(60.0));
        let label_h = if self.label.is_some() { font_xs() + 2.0 } else { 0.0 };
        let total = Vec2::new(w, h + label_h + if self.label.is_some() { 2.0 } else { 0.0 });
        let (rect, resp) = ui.allocate_exact_size(total, Sense::hover());
        let painter = ui.painter();

        if let Some(s) = self.label {
            painter.text(
                Pos2::new(rect.left(), rect.top()),
                egui::Align2::LEFT_TOP,
                s,
                egui::FontId::monospace(font_xs()),
                color_alpha(text_color, alpha_dim()),
            );
        }

        let bar_top = rect.top() + label_h + if self.label.is_some() { 2.0 } else { 0.0 };
        let track_rect = Rect::from_min_size(Pos2::new(rect.left(), bar_top), Vec2::new(w, h));
        let cr = egui::CornerRadius::same((h * 0.5) as u8);
        painter.rect_filled(track_rect, cr, track);

        let fill_w = (w * self.progress).max(0.0);
        if fill_w > 0.0 {
            let fill_rect = Rect::from_min_size(track_rect.min, Vec2::new(fill_w, h));
            painter.rect_filled(fill_rect, cr, fill);

            if matches!(self.variant, BarVariant::Striped) {
                let t_anim = ui.ctx().input(|i| i.time) as f32;
                ui.ctx().request_repaint();
                let stripe_w = 6.0;
                let offset = (t_anim * 20.0) % (stripe_w * 2.0);
                let mut x = fill_rect.left() - offset;
                while x < fill_rect.right() {
                    let x0 = x.max(fill_rect.left());
                    let x1 = (x + stripe_w).min(fill_rect.right());
                    if x1 > x0 {
                        painter.rect_filled(
                            Rect::from_min_max(Pos2::new(x0, fill_rect.top()), Pos2::new(x1, fill_rect.bottom())),
                            egui::CornerRadius::ZERO,
                            color_alpha(contrast_fg(fill), alpha_soft()),
                        );
                    }
                    x += stripe_w * 2.0;
                }
            }
        }
        resp
    }
}

// ─── ProgressRing ─────────────────────────────────────────────────────────────

/// Circular progress with percentage label in the center.
///
/// ```ignore
/// ui.add(ProgressRing::new(0.65).diameter(36.0).theme(t));
/// ```
#[must_use = "ProgressRing must be added with `ui.add(...)` to render"]
pub struct ProgressRing {
    progress: f32,
    diameter: f32,
    fill: Option<Color32>,
    track: Option<Color32>,
    text_color: Option<Color32>,
    show_label: bool,
}

impl ProgressRing {
    pub fn new(progress: f32) -> Self {
        Self {
            progress: progress.clamp(0.0, 1.0),
            diameter: 32.0,
            fill: None,
            track: None,
            text_color: None,
            show_label: true,
        }
    }
    pub fn diameter(mut self, d: f32) -> Self { self.diameter = d; self }
    pub fn show_label(mut self, v: bool) -> Self { self.show_label = v; self }
    pub fn fill(mut self, c: Color32) -> Self { self.fill = Some(c); self }
    pub fn theme(mut self, t: &Theme) -> Self {
        self.fill = Some(t.accent);
        self.track = Some(color_alpha(t.toolbar_border, alpha_strong()));
        self.text_color = Some(t.text);
        self
    }
}

impl Widget for ProgressRing {
    fn ui(self, ui: &mut Ui) -> Response {
        let amb = ambient(ui.ctx());
        let fill = self.fill.unwrap_or(amb.accent);
        let track = self.track.unwrap_or(color_alpha(amb.toolbar_border, alpha_strong()));
        let text_color = self.text_color.unwrap_or(amb.text);
        let d = self.diameter;
        let (rect, resp) = ui.allocate_exact_size(Vec2::splat(d + 2.0), Sense::hover());
        let center = rect.center();
        let radius = d * 0.5;
        let stroke_w = (d * 0.10).max(2.0);
        let painter = ui.painter();

        // Track ring
        painter.circle_stroke(center, radius, Stroke::new(stroke_w, track));

        // Progress arc
        let segments = 64;
        let total_angle = std::f32::consts::TAU * self.progress;
        let start = -std::f32::consts::FRAC_PI_2;
        for i in 0..segments {
            let f0 = i as f32 / segments as f32;
            let f1 = (i + 1) as f32 / segments as f32;
            if f0 >= self.progress { break; }
            let a0 = start + total_angle * f0 / self.progress.max(1e-6);
            let a1 = start + total_angle * f1.min(self.progress) / self.progress.max(1e-6);
            let p0 = center + Vec2::new(a0.cos(), a0.sin()) * radius;
            let p1 = center + Vec2::new(a1.cos(), a1.sin()) * radius;
            painter.line_segment([p0, p1], Stroke::new(stroke_w, fill));
        }

        if self.show_label {
            let pct = (self.progress * 100.0).round() as i32;
            painter.text(
                center,
                egui::Align2::CENTER_CENTER,
                format!("{}%", pct),
                egui::FontId::monospace((d * 0.30).max(font_xs())),
                text_color,
            );
        }
        resp
    }
}

// ─── Skeleton ─────────────────────────────────────────────────────────────────

/// Shimmer placeholder rectangle for loading states.
///
/// ```ignore
/// ui.add(Skeleton::new().size(120.0, 14.0).rounding(4.0).theme(t));
/// ```
#[must_use = "Skeleton must be added with `ui.add(...)` to render"]
pub struct Skeleton {
    size: Vec2,
    rounding: f32,
    base: Option<Color32>,
    highlight: Option<Color32>,
}

impl Skeleton {
    pub fn new() -> Self {
        Self {
            size: Vec2::new(80.0, 12.0),
            rounding: 3.0,
            base: None,
            highlight: None,
        }
    }
    pub fn size(mut self, w: f32, h: f32) -> Self { self.size = Vec2::new(w, h); self }
    pub fn width(mut self, w: f32) -> Self { self.size.x = w; self }
    pub fn height(mut self, h: f32) -> Self { self.size.y = h; self }
    pub fn rounding(mut self, r: f32) -> Self { self.rounding = r; self }
    pub fn theme(mut self, t: &Theme) -> Self {
        self.base = Some(color_alpha(t.toolbar_border, alpha_heavy()));
        self.highlight = Some(color_alpha(t.dim, alpha_muted()));
        self
    }
}

impl Default for Skeleton {
    fn default() -> Self { Self::new() }
}

impl Widget for Skeleton {
    fn ui(self, ui: &mut Ui) -> Response {
        let amb = ambient(ui.ctx());
        let base = self.base.unwrap_or(color_alpha(amb.toolbar_border, alpha_heavy()));
        let highlight = self.highlight.unwrap_or(color_alpha(amb.dim, alpha_muted()));
        let (rect, resp) = ui.allocate_exact_size(self.size, Sense::hover());
        let cr = egui::CornerRadius::same(self.rounding as u8);
        let painter = ui.painter();
        painter.rect_filled(rect, cr, base);

        // Animated shimmer band moving left → right
        let t = ui.ctx().input(|i| i.time) as f32;
        ui.ctx().request_repaint();
        let phase = (t * 0.6).fract();
        let band_w = self.size.x * 0.35;
        let x0 = rect.left() - band_w + phase * (self.size.x + band_w * 2.0);
        let band = Rect::from_min_size(Pos2::new(x0, rect.top()), Vec2::new(band_w, self.size.y))
            .intersect(rect);
        if band.width() > 0.0 {
            painter.rect_filled(band, cr, color_alpha(highlight, alpha_subtle()));
        }
        resp
    }
}

// ─── Toast ────────────────────────────────────────────────────────────────────
//
// Toast / ToastVariant / ToastResponse have moved to
// `crate::ui_kit::widgets::toast`. Re-exported here for back-compat.
#[deprecated(note = "use crate::ui_kit::widgets::Toast")]
pub use crate::ui_kit::widgets::toast::Toast;
#[deprecated(note = "use crate::ui_kit::widgets::toast::ToastVariant")]
pub use crate::ui_kit::widgets::toast::ToastVariant;
#[deprecated(note = "use crate::ui_kit::widgets::toast::ToastResponse")]
pub use crate::ui_kit::widgets::toast::ToastResponse;

// (Toast struct/impl moved to ui_kit::widgets::toast — see re-exports above.)

// ─── NotificationBadge ────────────────────────────────────────────────────────

/// Tiny numeric badge for tab/icon overlays — caps display at `max` (e.g.
/// `99+`).
///
/// ```ignore
/// ui.add(NotificationBadge::new(unread_count).theme(t));
/// ```
#[must_use = "NotificationBadge must be added with `ui.add(...)` to render"]
pub struct NotificationBadge {
    count: u32,
    max: u32,
    color: Option<Color32>,
    fg: Option<Color32>,
    show_zero: bool,
}

impl NotificationBadge {
    pub fn new(count: u32) -> Self {
        Self {
            count,
            max: 99,
            color: None,
            fg: None,
            show_zero: false,
        }
    }
    pub fn max(mut self, m: u32) -> Self { self.max = m; self }
    pub fn show_zero(mut self, v: bool) -> Self { self.show_zero = v; self }
    pub fn color(mut self, c: Color32) -> Self { self.color = Some(c); self }
    pub fn fg(mut self, c: Color32) -> Self { self.fg = Some(c); self }
    pub fn theme(mut self, t: &Theme) -> Self { self.color = Some(t.bear); self.fg = Some(contrast_fg(t.bear)); self }
}

impl Widget for NotificationBadge {
    fn ui(self, ui: &mut Ui) -> Response {
        if self.count == 0 && !self.show_zero {
            return ui.allocate_exact_size(Vec2::ZERO, Sense::hover()).1;
        }
        let color = self.color.unwrap_or(ambient(ui.ctx()).bear);
        let label = if self.count > self.max {
            format!("{}+", self.max)
        } else {
            self.count.to_string()
        };
        let h = 12.0;
        let pad_x = if label.len() > 1 { 4.0 } else { 0.0 };
        let w = h + pad_x;
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, h), Sense::hover());
        let painter = ui.painter();
        let cr = egui::CornerRadius::same((h * 0.5) as u8);
        painter.rect_filled(rect, cr, color);
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            &label,
            mono_sm(),
            self.fg.unwrap_or_else(|| contrast_fg(color)),
        );
        resp
    }
}

// ─── ConnectionIndicator ──────────────────────────────────────────────────────

/// Connection status enum used by [`ConnectionIndicator`].
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ConnectionStatus { Connected, Connecting, Disconnected, Error }

/// Dot + label combo expressing a service connection state. `Connecting`
/// renders a pulsing dot.
///
/// ```ignore
/// ui.add(ConnectionIndicator::new("Redis", ConnectionStatus::Connected).theme(t));
/// ```
#[must_use = "ConnectionIndicator must be added with `ui.add(...)` to render"]
pub struct ConnectionIndicator<'a> {
    label: &'a str,
    status: ConnectionStatus,
    bull: Option<Color32>,
    bear: Option<Color32>,
    warn: Option<Color32>,
    dim: Option<Color32>,
    text: Option<Color32>,
}

impl<'a> ConnectionIndicator<'a> {
    pub fn new(label: &'a str, status: ConnectionStatus) -> Self {
        Self {
            label,
            status,
            bull: None,
            bear: None,
            warn: None,
            dim: None,
            text: None,
        }
    }
    pub fn theme(mut self, t: &Theme) -> Self {
        self.bull = Some(t.bull);
        self.bear = Some(t.bear);
        self.warn = Some(t.warn);
        self.dim  = Some(t.dim);
        self.text = Some(t.text);
        self
    }
}

impl<'a> Widget for ConnectionIndicator<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        let amb = ambient(ui.ctx());
        let bull = self.bull.unwrap_or(amb.bull);
        let bear = self.bear.unwrap_or(amb.bear);
        let warn = self.warn.unwrap_or(amb.warn);
        let dim  = self.dim.unwrap_or(amb.dim);
        let text = self.text.unwrap_or(amb.text);
        let (color, text_color, pulsing, status_text) = match self.status {
            ConnectionStatus::Connected    => (bull, text, false, "OK"),
            ConnectionStatus::Connecting   => (warn, text, true,  "…"),
            ConnectionStatus::Disconnected => (dim,  color_alpha(text, alpha_dim()), false, "OFF"),
            ConnectionStatus::Error        => (bear, text, false, "ERR"),
        };
        let mut dot = StatusDot::new().color(color).label(self.label);
        if pulsing { dot = dot.pulsing(); }
        let resp = ui.horizontal(|ui| {
            let r = ui.add(dot);
            ui.add_space(gap_sm());
            ui.label(
                RichText::new(status_text)
                    .monospace()
                    .size(font_xs())
                    .color(color_alpha(text_color, alpha_dim())),
            );
            r
        }).inner;
        let _ = text_color;
        resp
    }
}

// ─── SearchPill (#6) ─────────────────────────────────────────────────────────

/// Toolbar search / command-palette trigger pill.
///
/// Renders as a flat pill reading "🔍 /CMD" in the right toolbar cluster.
/// Background tint scales with `current().hairline_borders`: 1.05× for Meridien
/// (slightly lighter than canvas), 0.92× otherwise.
///
/// ```ignore
/// if SearchPill::new().height(panel_rect.height() - 14.0).theme(t).show(ui).clicked() {
///     state.cmd_palette_open = !state.cmd_palette_open;
/// }
/// ```
#[must_use = "SearchPill must be shown with `.show(ui)` to render"]
pub struct SearchPill {
    width: f32,
    height: f32,
    bg: Option<egui::Color32>,
    border: Option<egui::Color32>,
    icon_color: Option<egui::Color32>,
    label_color: Option<egui::Color32>,
}

impl SearchPill {
    pub fn new() -> Self {
        Self {
            width: 78.0,
            height: 20.0,
            bg: None,
            border: None,
            icon_color: None,
            label_color: None,
        }
    }
    pub fn width(mut self, w: f32) -> Self { self.width = w; self }
    pub fn height(mut self, h: f32) -> Self { self.height = h; self }
    pub fn theme(mut self, t: &Theme) -> Self {
        let tint = if current().hairline_borders { 1.05 } else { 0.92 };
        self.bg = Some(t.bg.gamma_multiply(tint));
        self.border = Some(rule_stroke_for(t.bg, t.toolbar_border).color);
        self.icon_color = Some(color_alpha(t.dim, alpha_active()));
        self.label_color = Some(color_alpha(t.dim, alpha_strong()));
        self
    }

    /// Render and return the egui `Response` — check `.clicked()` to open palette.
    pub fn show(self, ui: &mut Ui) -> Response {
        let amb = ambient(ui.ctx());
        let tint = if current().hairline_borders { 1.05 } else { 0.92 };
        let bg = self.bg.unwrap_or_else(|| amb.bg.gamma_multiply(tint));
        let border = self.border.unwrap_or_else(|| rule_stroke_for(amb.bg, amb.toolbar_border).color);
        let icon_color = self.icon_color.unwrap_or_else(|| color_alpha(amb.dim, alpha_active()));
        let label_color = self.label_color.unwrap_or_else(|| color_alpha(amb.dim, alpha_strong()));
        let size = Vec2::new(self.width, self.height.max(20.0));
        let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
        if resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }

        let painter = ui.painter();
        let cr = egui::CornerRadius::same(current().r_pill.min(8));
        painter.rect_filled(rect, cr, bg);
        painter.rect_stroke(rect, cr,
            egui::Stroke::new(stroke_thin(), border), egui::StrokeKind::Outside);

        // Icon + label — laid out left-to-right with a small gap
        let text_x = rect.left() + 8.0;
        let cy = rect.center().y;
        painter.text(
            egui::pos2(text_x, cy),
            egui::Align2::LEFT_CENTER,
            "\u{1F50D}", // 🔍
            egui::FontId::proportional(font_sm()),
            icon_color,
        );
        painter.text(
            egui::pos2(text_x + font_sm() + 4.0, cy),
            egui::Align2::LEFT_CENTER,
            "/CMD",
            egui::FontId::monospace(font_xs()),
            label_color,
        );
        resp
    }
}

impl Default for SearchPill {
    fn default() -> Self { Self::new() }
}

// ─── TrendArrow ───────────────────────────────────────────────────────────────

/// Trend direction.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum TrendDir { Up, Down, Flat }

/// Up / down / flat arrow with optional value text. Bull / bear / dim
/// coloring follows the active theme.
///
/// ```ignore
/// ui.add(TrendArrow::up().value("+1.42%").theme(t));
/// ```
#[must_use = "TrendArrow must be added with `ui.add(...)` to render"]
pub struct TrendArrow<'a> {
    dir: TrendDir,
    value: Option<&'a str>,
    bull: Option<Color32>,
    bear: Option<Color32>,
    dim: Option<Color32>,
}

impl<'a> TrendArrow<'a> {
    pub fn new(dir: TrendDir) -> Self {
        Self {
            dir,
            value: None,
            bull: None,
            bear: None,
            dim:  None,
        }
    }
    pub fn up()   -> Self { Self::new(TrendDir::Up) }
    pub fn down() -> Self { Self::new(TrendDir::Down) }
    pub fn flat() -> Self { Self::new(TrendDir::Flat) }
    pub fn value(mut self, s: &'a str) -> Self { self.value = Some(s); self }
    pub fn theme(mut self, t: &Theme) -> Self {
        self.bull = Some(t.bull);
        self.bear = Some(t.bear);
        self.dim  = Some(t.dim);
        self
    }
}

impl<'a> Widget for TrendArrow<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        let amb = ambient(ui.ctx());
        let bull = self.bull.unwrap_or(amb.bull);
        let bear = self.bear.unwrap_or(amb.bear);
        let dim  = self.dim.unwrap_or(amb.dim);
        let (glyph, color) = match self.dir {
            TrendDir::Up   => ("\u{25B2}", bull), // ▲
            TrendDir::Down => ("\u{25BC}", bear), // ▼
            TrendDir::Flat => ("\u{2192}", dim),  // →
        };
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = gap_xs();
            let r = ui.label(
                RichText::new(glyph)
                    .monospace()
                    .size(font_sm())
                    .color(color),
            );
            if let Some(v) = self.value {
                ui.label(
                    RichText::new(v)
                        .monospace()
                        .size(font_sm())
                        .color(color),
                );
            }
            r
        }).inner
    }
}
