//! Builder + impl Widget primitives — pane chrome family.
//!
//! These are the bars/headers/footers AROUND a chart pane's paint area, not
//! the chart paint pipeline itself. Each builder composes lower-level
//! primitives from `widgets::{text, headers, pills, buttons}` and the legacy
//! helpers in `components` / `style` so visuals are 1:1 with the existing
//! pane chrome.
//!
//! See ui/widgets/mod.rs for the rationale.

#![allow(dead_code, unused_imports)]

use egui::{Color32, Response, RichText, Sense, Stroke, StrokeKind, Ui, Vec2, Widget};
use super::super::style::*;
use super::super::components::{pane_header_bar, pane_title, section_label_widget};
use super::headers::PaneHeader;
use super::pills::{PillButton, RemovableChip, DisplayChip, StatusBadge};

// ─── PaneSymbolBadge ─────────────────────────────────────────────────────────

/// Symbol + exchange + asset-class chip combo for a pane header.
///
/// Composes `pane_title` for the symbol (large mono-strong) followed by an
/// optional muted exchange suffix and an optional asset-class `DisplayChip`.
///
/// ```ignore
/// ui.add(PaneSymbolBadge::new("SPY").exchange("ARCA").asset_class("ETF").theme(t));
/// ```
#[must_use = "PaneSymbolBadge must be added with `ui.add(...)` to render"]
pub struct PaneSymbolBadge<'a> {
    symbol:      &'a str,
    exchange:    Option<&'a str>,
    asset_class: Option<&'a str>,
    accent:      Color32,
    dim:         Color32,
}

impl<'a> PaneSymbolBadge<'a> {
    pub fn new(symbol: &'a str) -> Self {
        Self {
            symbol,
            exchange: None,
            asset_class: None,
            accent: Color32::from_rgb(120, 140, 220),
            dim:    Color32::from_rgb(120, 120, 130),
        }
    }
    pub fn exchange(mut self, e: &'a str) -> Self { self.exchange = Some(e); self }
    pub fn asset_class(mut self, a: &'a str) -> Self { self.asset_class = Some(a); self }
    pub fn accent(mut self, c: Color32) -> Self { self.accent = c; self }
    pub fn dim(mut self, c: Color32) -> Self { self.dim = c; self }
    pub fn theme(self, t: &super::super::super::gpu::Theme) -> Self {
        self.accent(t.accent).dim(t.dim)
    }
}

impl<'a> Widget for PaneSymbolBadge<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = gap_xs();
            pane_title(ui, self.symbol, self.accent);
            if let Some(ex) = self.exchange {
                ui.label(
                    RichText::new(ex)
                        .monospace()
                        .size(font_xs())
                        .color(color_alpha(self.dim, alpha_muted())),
                );
            }
            if let Some(ac) = self.asset_class {
                ui.add(DisplayChip::new(ac).color(self.accent));
            }
        }).response
    }
}

// ─── PaneTimeframeBadge ──────────────────────────────────────────────────────

/// Timeframe pill with a small dropdown caret affordance. Returns a Response
/// — `clicked()` opens the timeframe menu.
///
/// ```ignore
/// if ui.add(PaneTimeframeBadge::new("5m").active(true).theme(t)).clicked() {
///     open_tf_menu = true;
/// }
/// ```
#[must_use = "PaneTimeframeBadge must be added with `ui.add(...)` to render"]
pub struct PaneTimeframeBadge<'a> {
    label:  &'a str,
    active: bool,
    accent: Color32,
    dim:    Color32,
}

impl<'a> PaneTimeframeBadge<'a> {
    pub fn new(label: &'a str) -> Self {
        Self {
            label,
            active: false,
            accent: Color32::from_rgb(120, 140, 220),
            dim:    Color32::from_rgb(120, 120, 130),
        }
    }
    pub fn active(mut self, v: bool) -> Self { self.active = v; self }
    pub fn theme(mut self, t: &super::super::super::gpu::Theme) -> Self {
        self.accent = t.accent;
        self.dim = t.dim;
        self
    }
}

impl<'a> Widget for PaneTimeframeBadge<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        // Render `<label> ▾` as a single pill — caret hints dropdown.
        let with_caret = format!("{}  \u{25BE}", self.label);
        // PillButton owns a `&str`, so stash in a local with the same lifetime as the call.
        let pill = PillButton::new(&with_caret)
            .active(self.active)
            .palette(self.accent, self.dim);
        ui.add(pill)
    }
}

// ─── PaneIndicatorChip ───────────────────────────────────────────────────────

/// Small chip showing an active indicator (e.g. "EMA 20") with a paired ✕
/// remove affordance. Returns `(label_resp, x_clicked)` — when `x_clicked`
/// is true the caller should remove that indicator.
///
/// ```ignore
/// let (_, removed) = PaneIndicatorChip::new("EMA 20").theme(t).show(ui);
/// if removed { indicators.remove(idx); }
/// ```
#[must_use = "PaneIndicatorChip must be shown with `.show(ui)` to render"]
pub struct PaneIndicatorChip<'a> {
    label:  &'a str,
    accent: Color32,
    dim:    Color32,
}

impl<'a> PaneIndicatorChip<'a> {
    pub fn new(label: &'a str) -> Self {
        Self {
            label,
            accent: Color32::from_rgb(120, 140, 220),
            dim:    Color32::from_rgb(120, 120, 130),
        }
    }
    pub fn palette(mut self, accent: Color32, dim: Color32) -> Self {
        self.accent = accent; self.dim = dim; self
    }
    pub fn theme(self, t: &super::super::super::gpu::Theme) -> Self {
        self.palette(t.accent, t.dim)
    }
    /// Render. Returns `(label_response, x_was_clicked)`.
    pub fn show(self, ui: &mut Ui) -> (Response, bool) {
        RemovableChip::new(self.label).palette(self.accent, self.dim).show(ui)
    }
}

// ─── PaneStatusStrip ─────────────────────────────────────────────────────────

/// Pane-level loading / connection / data-quality indicators. Renders a tiny
/// row of colored dots + optional labels. Composes into the right side of a
/// `PaneHeaderBar`.
///
/// ```ignore
/// ui.add(PaneStatusStrip::new()
///     .connected(true)
///     .loading(false)
///     .data_quality(DataQuality::Good)
///     .theme(t));
/// ```
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum DataQuality { Good, Degraded, Stale }

#[must_use = "PaneStatusStrip must be added with `ui.add(...)` to render"]
pub struct PaneStatusStrip {
    connected:    Option<bool>,
    loading:      bool,
    data_quality: Option<DataQuality>,
    bull:         Color32,
    warn:         Color32,
    bear:         Color32,
    dim:          Color32,
}

impl PaneStatusStrip {
    pub fn new() -> Self {
        Self {
            connected: None,
            loading: false,
            data_quality: None,
            bull: Color32::from_rgb(110, 180, 130),
            warn: Color32::from_rgb(220, 180, 80),
            bear: Color32::from_rgb(210, 90, 90),
            dim:  Color32::from_rgb(120, 120, 130),
        }
    }
    pub fn connected(mut self, v: bool) -> Self { self.connected = Some(v); self }
    pub fn loading(mut self, v: bool) -> Self { self.loading = v; self }
    pub fn data_quality(mut self, q: DataQuality) -> Self { self.data_quality = Some(q); self }
    pub fn theme(mut self, t: &super::super::super::gpu::Theme) -> Self {
        self.bull = t.bull; self.bear = t.bear; self.dim = t.dim; self
    }
}

impl Default for PaneStatusStrip {
    fn default() -> Self { Self::new() }
}

fn paint_status_dot(ui: &mut Ui, color: Color32) -> Response {
    let size = Vec2::new(8.0, 8.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::hover());
    let center = rect.center();
    ui.painter().circle_filled(center, 3.5, color);
    ui.painter().circle_stroke(
        center,
        3.5,
        Stroke::new(stroke_thin(), color_alpha(color, alpha_strong())),
    );
    resp
}

impl Widget for PaneStatusStrip {
    fn ui(self, ui: &mut Ui) -> Response {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = gap_xs();
            if self.loading {
                ui.label(
                    RichText::new("\u{2026}") // …
                        .monospace()
                        .size(font_xs())
                        .color(color_alpha(self.dim, alpha_strong())),
                );
            }
            if let Some(q) = self.data_quality {
                let (col, label) = match q {
                    DataQuality::Good     => (self.bull, "OK"),
                    DataQuality::Degraded => (self.warn, "DEGR"),
                    DataQuality::Stale    => (self.bear, "STALE"),
                };
                ui.add(DisplayChip::new(label).color(col));
            }
            if let Some(c) = self.connected {
                paint_status_dot(ui, if c { self.bull } else { self.bear });
            }
        }).response
    }
}

// ─── PaneHeaderBar ───────────────────────────────────────────────────────────

/// Full pane top strip — symbol badge + timeframe + indicator chips + actions.
///
/// Renders the header background (via `pane_header_bar`) and exposes a closure
/// for the actions slot on the right. Use in place of hand-rolled
/// `pane_header_bar(...)` calls in pane code.
///
/// ```ignore
/// PaneHeaderBar::new("SPY", "5m")
///     .exchange("ARCA")
///     .asset_class("ETF")
///     .indicators(&["EMA 20", "VWAP"])
///     .theme(t)
///     .show(ui, |ui| {
///         if ui.add(IconButton::new("\u{2699}")).clicked() { open_settings = true; }
///     });
/// ```
#[must_use = "PaneHeaderBar must be shown with `.show(ui, actions)` to render"]
pub struct PaneHeaderBar<'a> {
    symbol:      &'a str,
    timeframe:   &'a str,
    exchange:    Option<&'a str>,
    asset_class: Option<&'a str>,
    indicators:  &'a [&'a str],
    height:      f32,
    accent:      Color32,
    dim:         Color32,
    bg:          Color32,
    border:      Color32,
}

impl<'a> PaneHeaderBar<'a> {
    pub fn new(symbol: &'a str, timeframe: &'a str) -> Self {
        Self {
            symbol,
            timeframe,
            exchange: None,
            asset_class: None,
            indicators: &[],
            height: 28.0,
            accent: Color32::from_rgb(120, 140, 220),
            dim:    Color32::from_rgb(120, 120, 130),
            bg:     Color32::from_rgb(20, 20, 28),
            border: Color32::from_rgb(50, 50, 60),
        }
    }
    pub fn exchange(mut self, e: &'a str) -> Self { self.exchange = Some(e); self }
    pub fn asset_class(mut self, a: &'a str) -> Self { self.asset_class = Some(a); self }
    pub fn indicators(mut self, idx: &'a [&'a str]) -> Self { self.indicators = idx; self }
    pub fn height(mut self, h: f32) -> Self { self.height = h; self }
    pub fn theme(mut self, t: &super::super::super::gpu::Theme) -> Self {
        self.accent = t.accent;
        self.dim    = t.dim;
        self.bg     = t.toolbar_bg;
        self.border = t.toolbar_border;
        self
    }

    /// Render. `actions` paints into the right-aligned slot (settings cog,
    /// fullscreen, close, etc.). Returns indices of any indicator chip that
    /// had its ✕ clicked, so the caller can prune their list.
    pub fn show(
        self,
        ui: &mut Ui,
        actions: impl FnOnce(&mut Ui),
    ) -> Vec<usize> {
        let mut to_remove = Vec::new();
        let symbol = self.symbol;
        let tf = self.timeframe;
        let exchange = self.exchange;
        let asset_class = self.asset_class;
        let indicators = self.indicators;
        let accent = self.accent;
        let dim = self.dim;
        pane_header_bar(ui, self.height, self.bg, self.border, |ui| {
            ui.spacing_mut().item_spacing.x = gap_md();
            // Symbol
            let mut badge = PaneSymbolBadge::new(symbol).accent(accent).dim(dim);
            if let Some(e) = exchange { badge = badge.exchange(e); }
            if let Some(a) = asset_class { badge = badge.asset_class(a); }
            ui.add(badge);
            // Timeframe
            ui.add(PaneTimeframeBadge::new(tf).palette_inline(accent, dim));
            // Indicator chips
            for (i, ind) in indicators.iter().enumerate() {
                let (_, removed) = PaneIndicatorChip::new(ind).palette(accent, dim).show(ui);
                if removed { to_remove.push(i); }
            }
            // Right-aligned actions
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), actions);
        });
        ui.allocate_response(Vec2::ZERO, Sense::hover());
        to_remove
    }
}

// Helper for PaneTimeframeBadge — internal builder style call from PaneHeaderBar.
impl<'a> PaneTimeframeBadge<'a> {
    fn palette_inline(mut self, accent: Color32, dim: Color32) -> Self {
        self.accent = accent; self.dim = dim; self
    }
}

// ─── PaneToolbar ─────────────────────────────────────────────────────────────

/// Secondary toolbar row beneath the pane header — drawing tools, study buttons,
/// alert / replay controls. Same chrome as `PaneHeaderBar` (background fill +
/// hairline rule) but typically shorter and houses only icon-buttons.
///
/// ```ignore
/// ui.add(PaneToolbar::new().theme(t).show(ui, |ui| {
///     if ui.add(IconButton::new("\u{270E}")).clicked() { tool = Tool::Trendline; }
/// }));
/// ```
#[must_use = "PaneToolbar must be shown with `.show(ui, contents)` to render"]
pub struct PaneToolbar {
    height: f32,
    bg:     Color32,
    border: Color32,
}

impl PaneToolbar {
    pub fn new() -> Self {
        Self {
            height: 24.0,
            bg:     Color32::from_rgb(18, 18, 24),
            border: Color32::from_rgb(50, 50, 60),
        }
    }
    pub fn height(mut self, h: f32) -> Self { self.height = h; self }
    pub fn theme(mut self, t: &super::super::super::gpu::Theme) -> Self {
        self.bg = t.toolbar_bg; self.border = t.toolbar_border; self
    }
    /// Render. `contents` paints the toolbar's inline children left-to-right.
    pub fn show(self, ui: &mut Ui, contents: impl FnOnce(&mut Ui)) {
        pane_header_bar(ui, self.height, self.bg, self.border, |ui| {
            ui.spacing_mut().item_spacing.x = gap_xs();
            contents(ui);
        });
    }
}

impl Default for PaneToolbar {
    fn default() -> Self { Self::new() }
}

// ─── PaneFooter ──────────────────────────────────────────────────────────────

/// Bottom status strip: last price, change, time, connection. Mirrors
/// `pane_header_bar` chrome but inverted (top rule rather than bottom rule
/// is implicit — pane_header_bar paints a bottom rule, which sits between
/// chart and footer regardless of which side the footer is on).
///
/// ```ignore
/// ui.add(PaneFooter::new()
///     .last_price("$478.21", t.bull)
///     .change("+1.24 (+0.26%)", t.bull)
///     .time("15:42:01")
///     .connected(true)
///     .theme(t));
/// ```
#[must_use = "PaneFooter must be added with `ui.add(...)` to render"]
pub struct PaneFooter<'a> {
    last_price: Option<(&'a str, Color32)>,
    change:     Option<(&'a str, Color32)>,
    time:       Option<&'a str>,
    connected:  Option<bool>,
    height:     f32,
    bg:         Color32,
    border:     Color32,
    bull:       Color32,
    bear:       Color32,
    dim:        Color32,
}

impl<'a> PaneFooter<'a> {
    pub fn new() -> Self {
        Self {
            last_price: None,
            change:     None,
            time:       None,
            connected:  None,
            height:     22.0,
            bg:         Color32::from_rgb(18, 18, 24),
            border:     Color32::from_rgb(50, 50, 60),
            bull:       Color32::from_rgb(110, 180, 130),
            bear:       Color32::from_rgb(210, 90, 90),
            dim:        Color32::from_rgb(120, 120, 130),
        }
    }
    pub fn last_price(mut self, s: &'a str, c: Color32) -> Self { self.last_price = Some((s, c)); self }
    pub fn change(mut self, s: &'a str, c: Color32) -> Self { self.change = Some((s, c)); self }
    pub fn time(mut self, s: &'a str) -> Self { self.time = Some(s); self }
    pub fn connected(mut self, v: bool) -> Self { self.connected = Some(v); self }
    pub fn theme(mut self, t: &super::super::super::gpu::Theme) -> Self {
        self.bg = t.toolbar_bg;
        self.border = t.toolbar_border;
        self.bull = t.bull;
        self.bear = t.bear;
        self.dim  = t.dim;
        self
    }
}

impl<'a> Default for PaneFooter<'a> {
    fn default() -> Self { Self::new() }
}

impl<'a> Widget for PaneFooter<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        let last_price = self.last_price;
        let change = self.change;
        let time = self.time;
        let connected = self.connected;
        let bull = self.bull;
        let bear = self.bear;
        let dim = self.dim;
        pane_header_bar(ui, self.height, self.bg, self.border, |ui| {
            ui.spacing_mut().item_spacing.x = gap_md();
            if let Some((s, c)) = last_price {
                ui.label(RichText::new(s).monospace().size(font_sm()).strong().color(c));
            }
            if let Some((s, c)) = change {
                ui.label(RichText::new(s).monospace().size(font_xs()).color(c));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if let Some(c) = connected {
                    paint_status_dot(ui, if c { bull } else { bear });
                }
                if let Some(t) = time {
                    ui.label(
                        RichText::new(t)
                            .monospace()
                            .size(font_xs())
                            .color(color_alpha(dim, alpha_strong())),
                    );
                }
            });
        });
        ui.allocate_response(Vec2::ZERO, Sense::hover())
    }
}

// ─── PaneDivider ─────────────────────────────────────────────────────────────

/// Visual divider between stacked / side-by-side panes. Drag is handled
/// elsewhere — this is purely the rule-line + tiny grip handle.
///
/// ```ignore
/// ui.add(PaneDivider::horizontal().theme(t));   // between top/bottom panes
/// ui.add(PaneDivider::vertical().theme(t));     // between left/right panes
/// ```
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PaneDividerOrientation { Horizontal, Vertical }

#[must_use = "PaneDivider must be added with `ui.add(...)` to render"]
pub struct PaneDivider {
    orient: PaneDividerOrientation,
    border: Color32,
    thickness: f32,
}

impl PaneDivider {
    pub fn horizontal() -> Self {
        Self {
            orient: PaneDividerOrientation::Horizontal,
            border: Color32::from_rgb(50, 50, 60),
            thickness: 4.0,
        }
    }
    pub fn vertical() -> Self {
        Self {
            orient: PaneDividerOrientation::Vertical,
            border: Color32::from_rgb(50, 50, 60),
            thickness: 4.0,
        }
    }
    pub fn thickness(mut self, t: f32) -> Self { self.thickness = t; self }
    pub fn theme(mut self, t: &super::super::super::gpu::Theme) -> Self {
        self.border = t.toolbar_border; self
    }
}

impl Widget for PaneDivider {
    fn ui(self, ui: &mut Ui) -> Response {
        let st = current();
        let stroke_w = if st.hairline_borders { st.stroke_std } else { st.stroke_thin };
        let line_color = if st.hairline_borders {
            color_alpha(self.border, alpha_heavy())
        } else {
            color_alpha(self.border, alpha_muted())
        };
        match self.orient {
            PaneDividerOrientation::Horizontal => {
                let avail_w = ui.available_width();
                let (rect, resp) =
                    ui.allocate_exact_size(Vec2::new(avail_w, self.thickness), Sense::hover());
                let y = rect.center().y;
                ui.painter().line_segment(
                    [egui::pos2(rect.left(), y), egui::pos2(rect.right(), y)],
                    Stroke::new(stroke_w, line_color),
                );
                // Tiny grip (3 dots) center
                let cx = rect.center().x;
                let dot = color_alpha(self.border, alpha_strong());
                for dx in [-6.0_f32, 0.0, 6.0] {
                    ui.painter().circle_filled(egui::pos2(cx + dx, y), 0.9, dot);
                }
                resp
            }
            PaneDividerOrientation::Vertical => {
                let avail_h = ui.available_height();
                let (rect, resp) =
                    ui.allocate_exact_size(Vec2::new(self.thickness, avail_h), Sense::hover());
                let x = rect.center().x;
                ui.painter().line_segment(
                    [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
                    Stroke::new(stroke_w, line_color),
                );
                let cy = rect.center().y;
                let dot = color_alpha(self.border, alpha_strong());
                for dy in [-6.0_f32, 0.0, 6.0] {
                    ui.painter().circle_filled(egui::pos2(x, cy + dy), 0.9, dot);
                }
                resp
            }
        }
    }
}
