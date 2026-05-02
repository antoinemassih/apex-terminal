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

use egui::{Color32, Pos2, Rect, Response, RichText, Sense, Stroke, StrokeKind, Ui, Vec2, Widget};
use super::super::style::*;
use super::super::components::{pane_header_bar, pane_title, section_label_widget};
use super::headers::PaneHeader;
use super::pills::{PillButton, RemovableChip, DisplayChip, StatusBadge};

fn ft() -> &'static crate::chart_renderer::gpu::Theme {
    &crate::chart_renderer::gpu::THEMES[0]
}

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
            accent: ft().accent,
            dim:    ft().dim,
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
            accent: ft().accent,
            dim:    ft().dim,
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
            accent: ft().accent,
            dim:    ft().dim,
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
            bull: ft().bull,
            warn: Color32::from_rgb(220, 180, 80),
            bear: ft().bear,
            dim:  ft().dim,
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
            accent: ft().accent,
            dim:    ft().dim,
            bg:     ft().toolbar_bg,
            border: ft().toolbar_border,
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
            bg:     ft().toolbar_bg,
            border: ft().toolbar_border,
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
            bg:         ft().toolbar_bg,
            border:     ft().toolbar_border,
            bull:       ft().bull,
            bear:       ft().bear,
            dim:        ft().dim,
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

// ─── PaneHeaderActions (#10) ─────────────────────────────────────────────────

/// Right-aligned action cluster for a pane header bar.
///
/// Accepts a slice of `(label, active)` pairs and renders them as flat
/// frameless label buttons separated by full-height hairline dividers (when
/// `current().hairline_borders` is true). Returns the index of the button
/// that was clicked, if any.
///
/// Layout is right-to-left (rightmost button first). The caller is responsible
/// for positioning — typically used inside a `with_layout(right_to_left)` block
/// or a horizontal strip.
///
/// ```ignore
/// let header_h = 28.0;
/// if let Some(idx) = PaneHeaderActions::new(&[("+ Compare", false), ("Order", oe_open),
///         ("DOM", dom_open), ("Options", opt_open)])
///     .header_height(header_h)
///     .theme(t)
///     .show(ui)
/// {
///     match idx {
///         0 => add_compare(),
///         1 => oe_open = !oe_open,
///         2 => dom_open = !dom_open,
///         _ => opt_open = !opt_open,
///     }
/// }
/// ```
#[must_use = "PaneHeaderActions must be shown with `.show(ui)` to render"]
pub struct PaneHeaderActions<'a> {
    actions: &'a [(&'a str, bool)],
    header_height: f32,
    active_color: Color32,
    inactive_color: Color32,
    border_color: Color32,
}

impl<'a> PaneHeaderActions<'a> {
    pub fn new(actions: &'a [(&'a str, bool)]) -> Self {
        Self {
            actions,
            header_height: 28.0,
            active_color: ft().text,
            inactive_color: ft().dim,
            border_color: ft().toolbar_border,
        }
    }
    pub fn header_height(mut self, h: f32) -> Self { self.header_height = h; self }
    pub fn theme(mut self, t: &super::super::super::gpu::Theme) -> Self {
        self.active_color   = t.text;
        self.inactive_color = t.dim;
        self.border_color   = t.toolbar_border;
        self
    }

    /// Render. Returns `Some(index)` of the clicked action, `None` if none clicked.
    pub fn show(self, ui: &mut Ui) -> Option<usize> {
        let hairline = current().hairline_borders;
        let label_gap  = 14.0_f32;
        let divider_sp =  7.0_f32;
        let font = egui::FontId::monospace(font_md());
        let mut clicked = None;

        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            for (i, &(label, active)) in self.actions.iter().enumerate() {
                // Hairline divider to the left of each action (except the first)
                if i > 0 && hairline {
                    ui.add_space(divider_sp);
                    let x = ui.cursor().left();
                    let top = ui.cursor().top();
                    let bot = top + self.header_height;
                    let rule_col = rule_color_for(self.border_color);
                    ui.painter().line_segment(
                        [Pos2::new(x, top), Pos2::new(x, bot)],
                        Stroke::new(1.0, rule_col),
                    );
                    ui.add_space(divider_sp);
                } else if i > 0 {
                    ui.add_space(label_gap);
                }

                let color = if active { self.active_color } else { self.inactive_color };
                let (text_size, _) = ui.fonts(|f| {
                    let g = f.layout_no_wrap(label.to_string(), font.clone(), color);
                    (g.size(), ())
                });
                let (rect, resp) = ui.allocate_exact_size(
                    Vec2::new(text_size.x + 4.0, self.header_height), Sense::click());
                if resp.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    ui.painter().rect_filled(rect, egui::CornerRadius::ZERO,
                        color_alpha(self.border_color, alpha_soft()));
                }
                ui.painter().text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    label,
                    font.clone(),
                    color,
                );
                if resp.clicked() { clicked = Some(i); }
            }
        });
        clicked
    }
}

/// Returns a hairline rule color: slightly higher alpha on dark backgrounds.
fn rule_color_for(border: Color32) -> Color32 {
    // Approximate: if border luminance < 100 → dark bg → use 85% alpha, else 60%.
    let lum = 0.299 * border.r() as f32 + 0.587 * border.g() as f32 + 0.114 * border.b() as f32;
    let a = if lum < 100.0 { alpha_strong() } else { alpha_dim() };
    color_alpha(border, a)
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
            border: ft().toolbar_border,
            thickness: 4.0,
        }
    }
    pub fn vertical() -> Self {
        Self {
            orient: PaneDividerOrientation::Vertical,
            border: ft().toolbar_border,
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

// ─── AccountStrip ─────────────────────────────────────────────────────────────

/// Top-panel account summary strip.
///
/// Renders connection state, NAV (hero), Daily P&L (hero, colored), Buying
/// Power, Unrealized P&L, Margin, Excess Liquidity, Realized P&L, and two
/// emergency action buttons (CANCEL ALL / FLATTEN).
///
/// The `TopBottomPanel` frame is owned by the caller (gpu.rs) so that the
/// exact height token from `style::current().account_strip_height` is applied
/// at the panel boundary. This widget is called from inside the panel closure.
///
/// ```ignore
/// AccountStrip::new()
///     .account_data(account_data_cached.as_ref().map(|(a,_,_)| a))
///     .broker_url(APEXIB_URL)
///     .theme(&t)
///     .show(ui, || { /* cancel_all */ }, || { /* flatten */ });
/// ```
pub struct AccountStrip<'a> {
    account_data: Option<&'a crate::chart_renderer::trading::AccountSummary>,
    broker_url:   &'a str,
    theme:        Option<&'a super::super::super::gpu::Theme>,
}

impl<'a> AccountStrip<'a> {
    pub fn new() -> Self {
        Self { account_data: None, broker_url: "", theme: None }
    }

    pub fn account_data(mut self, d: Option<&'a crate::chart_renderer::trading::AccountSummary>) -> Self {
        self.account_data = d; self
    }

    pub fn broker_url(mut self, u: &'a str) -> Self {
        self.broker_url = u; self
    }

    pub fn theme(mut self, t: &'a super::super::super::gpu::Theme) -> Self {
        self.theme = Some(t); self
    }

    /// Render the strip inside a pre-allocated `ui` (typically the inner ui of
    /// a `TopBottomPanel`). `on_cancel_all` and `on_flatten` fire if the
    /// respective button is clicked.
    pub fn show(
        self,
        ui: &mut Ui,
        on_cancel_all: impl FnOnce(),
        on_flatten:    impl FnOnce(),
    ) -> Response {
        let t = match self.theme {
            Some(t) => t,
            None => return ui.label(""),
        };

        ui.with_layout(egui::Layout::centered_and_justified(egui::Direction::TopDown), |ui| {
            ui.horizontal(|ui| {
                let avail = ui.available_width();
                ui.spacing_mut().item_spacing.x = 16.0;

                if let Some(acct) = self.account_data {
                    if acct.connected {
                        let content_w = 680.0_f32;
                        let pad = ((avail - content_w) / 2.0).max(0.0);
                        ui.add_space(pad);

                        // NAV — hero number
                        ui.label(RichText::new("NAV").monospace().size(11.0).color(color_alpha(t.dim, 128)));
                        ui.label(hero_text(&format!("${:.0}", acct.nav), t.text).strong());

                        ui.add(egui::Separator::default().spacing(8.0));

                        // Buying Power
                        ui.label(RichText::new("BP").monospace().size(11.0).color(color_alpha(t.dim, 128)));
                        ui.label(RichText::new(format!("${:.0}", acct.buying_power)).monospace().size(11.0).color(t.dim));

                        ui.add(egui::Separator::default().spacing(8.0));

                        // Daily P&L — hero, colored
                        let daily_color = if acct.daily_pnl >= 0.0 { t.bull } else { t.bear };
                        ui.label(RichText::new("Day P&L").monospace().size(11.0).color(color_alpha(t.dim, 128)));
                        ui.label(hero_text(&format!("{:+.0}", acct.daily_pnl), daily_color).strong());

                        ui.add(egui::Separator::default().spacing(8.0));

                        // Unrealized P&L
                        let unr_color = if acct.unrealized_pnl >= 0.0 { t.bull } else { t.bear };
                        ui.label(RichText::new("Unr P&L").monospace().size(11.0).color(color_alpha(t.dim, 128)));
                        ui.label(RichText::new(format!("{:+.0}", acct.unrealized_pnl)).monospace().size(11.0).color(unr_color));

                        ui.add(egui::Separator::default().spacing(8.0));

                        // Margin
                        ui.label(RichText::new("Margin").monospace().size(11.0).color(color_alpha(t.dim, 128)));
                        ui.label(RichText::new(format!("${:.0}", acct.initial_margin)).monospace().size(11.0).color(t.dim));

                        ui.add(egui::Separator::default().spacing(8.0));

                        // Excess Liquidity
                        ui.label(RichText::new("Excess").monospace().size(11.0).color(color_alpha(t.dim, 128)));
                        ui.label(RichText::new(format!("${:.0}", acct.excess_liquidity)).monospace().size(11.0).color(t.dim));

                        ui.add(egui::Separator::default().spacing(8.0));

                        // Realized P&L
                        let rpnl_color = if acct.realized_pnl >= 0.0 { t.bull } else { t.bear };
                        ui.label(RichText::new("Real P&L").monospace().size(11.0).color(color_alpha(t.dim, 128)));
                        ui.label(RichText::new(format!("{:+.0}", acct.realized_pnl)).monospace().size(11.0).strong().color(rpnl_color));

                        ui.add(egui::Separator::default().spacing(8.0));

                        // Emergency action buttons
                        if ui.add(
                            egui::Button::new(RichText::new("CANCEL ALL").monospace().size(9.0).strong().color(Color32::WHITE))
                                .fill(color_alpha(t.bear, 120))
                                .corner_radius(3.0)
                                .min_size(egui::vec2(0.0, 22.0))
                                .stroke(Stroke::new(1.0, t.bear)),
                        ).clicked() {
                            on_cancel_all();
                        }

                        if ui.add(
                            egui::Button::new(RichText::new("FLATTEN").monospace().size(9.0).strong().color(Color32::WHITE))
                                .fill(color_alpha(t.bear, 180))
                                .corner_radius(3.0)
                                .min_size(egui::vec2(0.0, 22.0))
                                .stroke(Stroke::new(1.0, t.bear)),
                        ).clicked() {
                            on_flatten();
                        }
                    } else {
                        // Disconnected
                        ui.label(RichText::new("IB Disconnected").monospace().size(9.0).color(color_alpha(t.dim, 128)));
                        ui.label(RichText::new(format!("connecting to {}...", self.broker_url)).monospace().size(9.0).color(color_alpha(t.dim, 76)));
                    }
                } else {
                    // Loading
                    ui.label(RichText::new("Loading account...").monospace().size(11.0).color(color_alpha(t.dim, 102)));
                }
            });
        }).response
    }
}

impl Default for AccountStrip<'_> {
    fn default() -> Self { Self::new() }
}

// ─── FloatingOrderPaneChrome ──────────────────────────────────────────────────

/// Chrome wrapper for a floating order-entry window.
///
/// Renders the title bar (background fill, armed toggle, title label, optional
/// position indicator, expand/collapse toggle, close button) and a drag handle
/// occupying the middle of that header strip. The body closure receives the
/// inner `Ui` and should call `render_order_entry_body` (or equivalent).
///
/// ```ignore
/// FloatingOrderPaneChrome::new(pane.id)
///     .title(&pane.title)
///     .width(fp_panel_w)
///     .armed(chart.armed)
///     .advanced(chart.order_advanced)
///     .position_text(pos_text, pos_color) // optional
///     .theme(t)
///     .show(ui, |ui| {
///         render_order_entry_body(ui, chart, t, 1000 + pane.id as u64, fp_panel_w);
///     })
/// ```
///
/// Returns a `FloatingOrderPaneChromeResponse` with flags for what happened.
#[must_use = "FloatingOrderPaneChrome must be shown with `.show(ui, body)` to render"]
pub struct FloatingOrderPaneChrome<'a> {
    id:           u32,
    title:        &'a str,
    width:        f32,
    armed:        bool,
    advanced:     bool,
    pos_text:     Option<(&'a str, Color32)>,
    accent:       Color32,
    dim:          Color32,
    toolbar_bg:   Color32,
    toolbar_border: Color32,
}

pub struct FloatingOrderPaneChromeResponse {
    /// Close button was clicked — caller should remove this pane.
    pub close_clicked: bool,
    /// Armed toggle was clicked — caller should flip `chart.armed`.
    pub armed_toggled: bool,
    /// Expand/collapse toggle was clicked — caller should flip `chart.order_advanced`.
    pub advanced_toggled: bool,
    /// Drag delta this frame (zero if not dragging).
    pub drag_delta: egui::Vec2,
}

impl<'a> FloatingOrderPaneChrome<'a> {
    pub fn new(id: u32) -> Self {
        Self {
            id,
            title:          "",
            width:          210.0,
            armed:          false,
            advanced:       false,
            pos_text:       None,
            accent:         ft().accent,
            dim:            ft().dim,
            toolbar_bg:     ft().toolbar_bg,
            toolbar_border: ft().toolbar_border,
        }
    }

    pub fn title(mut self, t: &'a str) -> Self { self.title = t; self }
    pub fn width(mut self, w: f32) -> Self { self.width = w; self }
    pub fn armed(mut self, v: bool) -> Self { self.armed = v; self }
    pub fn advanced(mut self, v: bool) -> Self { self.advanced = v; self }
    /// Optional: current position label for this symbol, e.g. "+2" in bull color.
    pub fn position_text(mut self, text: &'a str, color: Color32) -> Self {
        self.pos_text = Some((text, color)); self
    }
    pub fn theme(mut self, t: &super::super::super::gpu::Theme) -> Self {
        self.accent         = t.accent;
        self.dim            = t.dim;
        self.toolbar_bg     = t.toolbar_bg;
        self.toolbar_border = t.toolbar_border;
        self
    }

    /// Render the header chrome and call `body` for the pane contents.
    pub fn show(
        self,
        ui: &mut Ui,
        body: impl FnOnce(&mut Ui),
    ) -> FloatingOrderPaneChromeResponse {
        use crate::ui_kit::icons::Icon;

        let header_h = 22.0_f32;
        let w        = self.width;
        let armed    = self.armed;
        let advanced = self.advanced;
        let dim      = self.dim;
        let accent   = self.accent;
        let border   = self.toolbar_border;

        let mut close_clicked    = false;
        let mut armed_toggled    = false;
        let mut advanced_toggled = false;
        let mut drag_delta       = egui::Vec2::ZERO;

        // ── Header row ────────────────────────────────────────────────────
        let header_resp = ui.horizontal(|ui| {
            ui.set_min_width(w);
            let header_rect = ui.max_rect();
            // Background fill
            ui.painter().rect_filled(
                egui::Rect::from_min_size(header_rect.min, egui::vec2(w, header_h)),
                egui::CornerRadius { nw: radius_md() as u8, ne: radius_md() as u8, sw: 0, se: 0 },
                color_alpha(border, alpha_soft()),
            );
            ui.add_space(gap_sm());

            // Armed toggle
            let armed_icon  = if armed { Icon::SHIELD_WARNING } else { Icon::PLAY };
            let armed_color = if armed { accent } else { dim.gamma_multiply(0.4) };
            let armed_resp  = ui.add(
                egui::Button::new(egui::RichText::new(armed_icon).size(font_xs() + 3.0).color(armed_color))
                    .fill(if armed { color_alpha(accent, alpha_soft()) } else { Color32::TRANSPARENT })
                    .stroke(egui::Stroke::NONE)
                    .min_size(egui::vec2(18.0, 18.0))
                    .corner_radius(radius_sm()),
            );
            if armed_resp.clicked()  { armed_toggled = true; }
            if armed_resp.hovered()  { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }

            // Title
            ui.label(
                egui::RichText::new(self.title)
                    .monospace()
                    .size(font_xs() + 1.0)
                    .strong()
                    .color(color_alpha(dim, alpha_strong())),
            );

            // Optional position indicator
            if let Some((text, color)) = self.pos_text {
                ui.label(
                    egui::RichText::new(text)
                        .monospace()
                        .size(font_xs() + 1.0)
                        .strong()
                        .color(color),
                );
            }

            // Right-side controls
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(gap_sm());

                // Close button
                let close_resp = ui.add(
                    egui::Button::new(
                        egui::RichText::new(Icon::X).size(font_xs() + 1.0).color(dim.gamma_multiply(0.5)),
                    )
                    .fill(Color32::TRANSPARENT)
                    .min_size(egui::vec2(20.0, 18.0))
                    .corner_radius(radius_sm()),
                );
                if close_resp.clicked() { close_clicked = true; }

                ui.add(egui::Separator::default().spacing(2.0));

                // Expand/collapse toggle
                let exp_icon = if advanced { Icon::MINUS } else { Icon::PLUS };
                let exp_resp = ui.add(
                    egui::Button::new(
                        egui::RichText::new(exp_icon).size(font_xs() + 1.0).color(dim.gamma_multiply(0.5)),
                    )
                    .fill(Color32::TRANSPARENT)
                    .min_size(egui::vec2(20.0, 18.0))
                    .corner_radius(radius_sm()),
                );
                if exp_resp.clicked() { advanced_toggled = true; }
                if exp_resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
            });
        });

        // ── Drag handle (middle strip of header) ─────────────────────────
        let hdr_min  = header_resp.response.rect.min;
        let mid_rect = egui::Rect::from_min_size(
            egui::pos2(hdr_min.x + 26.0, hdr_min.y),
            egui::vec2(w - 80.0, header_h),
        );
        let drag_resp = ui.interact(
            mid_rect,
            egui::Id::new(("float_order_drag", self.id)),
            egui::Sense::click_and_drag(),
        );
        if drag_resp.dragged()  { drag_delta = drag_resp.drag_delta(); }
        if drag_resp.hovered()  { ui.ctx().set_cursor_icon(egui::CursorIcon::Grab); }

        // ── Body ──────────────────────────────────────────────────────────
        body(ui);

        FloatingOrderPaneChromeResponse { close_clicked, armed_toggled, advanced_toggled, drag_delta }
    }
}

impl Default for FloatingOrderPaneChrome<'_> {
    fn default() -> Self { Self::new(0) }
}
