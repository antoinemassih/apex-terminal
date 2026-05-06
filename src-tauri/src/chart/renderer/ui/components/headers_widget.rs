//! Builder + impl Widget primitives — headers family.
//! See ui/widgets/mod.rs for the rationale.

#![allow(dead_code, unused_imports)]

use egui::{Color32, Response, RichText, Sense, Ui, Vec2, Widget};
use super::super::style::*;
use super::super::components::{section_label_widget, pane_header_bar};
use super::text::{SectionLabel, SectionLabelSize};

// Fallback colors used when no Theme is supplied via `.theme(t)`. Every concrete
// panel passes a Theme, but these keep the builders usable in isolation (tests,
// previews, headless rendering) without crashing the design-token resolver.
const FALLBACK_ACCENT:      Color32 = Color32::from_rgb(120, 140, 220);
const FALLBACK_DIM:         Color32 = Color32::from_rgb(120, 120, 130);
const FALLBACK_PANE_BG:     Color32 = Color32::from_rgb( 20,  20,  28);
const FALLBACK_PANE_BORDER: Color32 = Color32::from_rgb( 50,  50,  60);

// ─── PanelHeaderWithClose ────────────────────────────────────────────────────

/// Builder for a panel header with close button. Returns `true` if close clicked.
/// Mirrors `style::panel_header_sub(ui, title, None, accent, dim)`.
///
/// ```ignore
/// if PanelHeaderWithClose::new("Positions").theme(t).show(ui) { open = false; }
/// ```
#[must_use]
pub struct PanelHeaderWithClose<'a> {
    title:           &'a str,
    subtitle:        Option<&'a str>,
    accent:          Color32,
    dim:             Color32,
    title_size:      SectionLabelSize,
    title_size_px:   Option<f32>,
    title_monospace: Option<bool>,
    leading_space:   f32,
    trailing_space:  f32,
}

impl<'a> PanelHeaderWithClose<'a> {
    pub fn new(title: &'a str) -> Self {
        Self {
            title,
            subtitle:        None,
            accent:          FALLBACK_ACCENT,
            dim:             FALLBACK_DIM,
            title_size:      SectionLabelSize::Sm,
            title_size_px:   None,
            title_monospace: None,
            leading_space:   0.0,
            trailing_space:  0.0,
        }
    }
    pub fn accent(mut self, c: Color32) -> Self { self.accent = c; self }
    pub fn dim(mut self, c: Color32) -> Self { self.dim = c; self }
    pub fn subtitle(mut self, s: &'a str) -> Self { self.subtitle = Some(s); self }
    /// Override the title's `SectionLabel` size variant (default: `Sm`).
    pub fn title_size(mut self, s: SectionLabelSize) -> Self { self.title_size = s; self }
    /// Escape-hatch pixel size that overrides the variant — use sparingly for
    /// bespoke title sizes that don't fit the size scale (e.g. 9px for tight
    /// sidebars). Prefer `title_size()` when a variant works.
    pub fn title_size_px(mut self, px: f32) -> Self { self.title_size_px = Some(px); self }
    /// Force monospace (or proportional) title rendering. Default: proportional
    /// (`TextStyle::Label`). Set to `true` for code-like titles ("OBJECTS",
    /// "DEBUG"), or `false` to force-disable when a variant defaults to mono.
    pub fn title_monospace(mut self, mono: bool) -> Self { self.title_monospace = Some(mono); self }
    /// Pixels of horizontal space inserted before the title. Used by panels with
    /// zero-margin frames (popups) where the header needs its own edge padding.
    pub fn leading_space(mut self, px: f32) -> Self { self.leading_space = px; self }
    /// Pixels of horizontal space inserted after the close button (right edge).
    pub fn trailing_space(mut self, px: f32) -> Self { self.trailing_space = px; self }
    pub fn theme(self, t: &super::super::super::gpu::Theme) -> Self {
        self.accent(t.accent).dim(t.dim)
    }

    /// Render the header. Returns `true` if the close button was clicked.
    ///
    /// Title is rendered via `SectionLabel` (proportional, `TextStyle::Label`),
    /// matching the design-system label style used everywhere else. Subtitle —
    /// when set — is rendered as monospace `font_sm` in `dim`.
    pub fn show(self, ui: &mut Ui) -> bool {
        self.show_full(ui, |_| {}, |_| {})
    }

    /// Render the header with extra trailing controls placed to the LEFT of the
    /// close button (inside the right-to-left layout). Returns `true` if the
    /// close button was clicked.
    ///
    /// ```ignore
    /// PanelHeaderWithClose::new("ANALYSIS").theme(t).show_with(ui, |ui| {
    ///     if ui.add(ChromeBtn::new("+")).clicked() { /* … */ }
    /// });
    /// ```
    pub fn show_with(self, ui: &mut Ui, actions: impl FnOnce(&mut Ui)) -> bool {
        self.show_full(ui, |_| {}, actions)
    }

    /// Render with controls placed immediately to the RIGHT of the title (LTR
    /// flow). Useful for filter chips, count badges, etc. that visually belong
    /// next to the title rather than next to the close button.
    pub fn show_with_title_actions(self, ui: &mut Ui, title_actions: impl FnOnce(&mut Ui)) -> bool {
        self.show_full(ui, title_actions, |_| {})
    }

    /// Render with both leading (next-to-title) and trailing (next-to-close)
    /// controls. Returns `true` if the close button was clicked.
    pub fn show_full(
        self,
        ui: &mut Ui,
        title_actions: impl FnOnce(&mut Ui),
        actions: impl FnOnce(&mut Ui),
    ) -> bool {
        let mut closed = false;
        let title = self.title;
        let subtitle = self.subtitle;
        let accent = self.accent;
        let dim = self.dim;
        let size = self.title_size;
        let size_px = self.title_size_px;
        let mono = self.title_monospace;
        let leading = self.leading_space;
        let trailing = self.trailing_space;
        ui.horizontal(|ui| {
            if leading > 0.0 { ui.add_space(leading); }
            let mut label = SectionLabel::new(title).size(size).color(accent);
            if let Some(px) = size_px { label = label.size_px(px); }
            if let Some(m) = mono   { label = label.monospace(m); }
            ui.add(label);
            if let Some(sub) = subtitle {
                ui.label(RichText::new(sub).monospace().size(font_sm()).color(dim));
            }
            title_actions(ui);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if trailing > 0.0 { ui.add_space(trailing); }
                if super::super::style::close_button(ui, dim) { closed = true; }
                actions(ui);
            });
        });
        closed
    }
}

// ─── PanelHeaderWithTabs ─────────────────────────────────────────────────────

/// Builder for a tab-driven panel header: `TabBar` on the left, close button on
/// the right, optional trailing controls (badges, icons) left of the close.
///
/// Used by panels where the *tabs* are the title — orders, watchlist — instead
/// of a static label.
///
/// ```ignore
/// let closed = PanelHeaderWithTabs::new(&mut tab, &[
///     (Tab::A, "ALPHA"),
///     (Tab::B, "BETA"),
/// ]).theme(t).show(ui);
/// if closed { open = false; }
/// ```
#[must_use]
pub struct PanelHeaderWithTabs<'a, T: PartialEq + Copy> {
    current:    &'a mut T,
    tabs:       &'a [(T, &'a str)],
    accent:     Color32,
    dim:        Color32,
    min_height: f32,
}

impl<'a, T: PartialEq + Copy> PanelHeaderWithTabs<'a, T> {
    pub fn new(current: &'a mut T, tabs: &'a [(T, &'a str)]) -> Self {
        Self {
            current,
            tabs,
            accent:     FALLBACK_ACCENT,
            dim:        FALLBACK_DIM,
            min_height: 24.0,
        }
    }
    pub fn accent(mut self, c: Color32) -> Self { self.accent = c; self }
    pub fn dim(mut self, c: Color32) -> Self { self.dim = c; self }
    pub fn min_height(mut self, h: f32) -> Self { self.min_height = h; self }
    pub fn theme(self, t: &super::super::super::gpu::Theme) -> Self {
        self.accent(t.accent).dim(t.dim)
    }

    /// Render the header. Returns `true` if the close button was clicked.
    pub fn show(self, ui: &mut Ui) -> bool {
        self.show_with(ui, |_| {})
    }

    /// Render with extra controls placed to the LEFT of the close button (RTL
    /// layout). Used for badges (e.g. watchlist's market-session indicator).
    pub fn show_with(self, ui: &mut Ui, actions: impl FnOnce(&mut Ui)) -> bool {
        use super::tabs::TabBar;
        let mut closed = false;
        let accent = self.accent;
        let dim = self.dim;
        let min_h = self.min_height;
        let tabs = self.tabs;
        let current = self.current;
        ui.horizontal(|ui| {
            ui.set_min_height(min_h);
            TabBar::new(current, tabs).accent(accent).dim(dim).show(ui);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if super::super::style::close_button(ui, dim) { closed = true; }
                actions(ui);
            });
        });
        closed
    }
}

// ─── DialogHeaderWithClose ───────────────────────────────────────────────────

/// Builder for a dialog header bar with close button. Returns `true` if close clicked.
/// Mirrors `style::dialog_header(ui, title, dim)`.
///
/// ```ignore
/// if DialogHeaderWithClose::new("Settings").theme(t).show(ui) { open = false; }
/// ```
#[must_use]
pub struct DialogHeaderWithClose<'a> {
    title: &'a str,
    dim:   Color32,
}

impl<'a> DialogHeaderWithClose<'a> {
    pub fn new(title: &'a str) -> Self {
        Self {
            title,
            dim: FALLBACK_DIM,
        }
    }
    pub fn dim(mut self, c: Color32) -> Self { self.dim = c; self }
    pub fn theme(self, t: &super::super::super::gpu::Theme) -> Self {
        self.dim(t.dim)
    }

    /// Render the header. Returns `true` if the close button was clicked.
    pub fn show(self, ui: &mut Ui) -> bool {
        super::super::style::dialog_header(ui, self.title, self.dim)
    }
}

// ─── PaneHeader ───────────────────────────────────────────────────────────────

/// Builder for a pane header bar (background + bottom rule, no close button).
/// Wraps `components::pane_header_bar` with a title label as the only content.
///
/// ```ignore
/// ui.add(PaneHeader::new("Chart").theme(t));
/// ```
#[must_use = "PaneHeader must be added with `ui.add(...)` to render"]
pub struct PaneHeader<'a> {
    title:        &'a str,
    title_color:  Color32,
    bg:           Color32,
    border:       Color32,
    height:       f32,
}

impl<'a> PaneHeader<'a> {
    pub fn new(title: &'a str) -> Self {
        Self {
            title,
            title_color: FALLBACK_ACCENT,
            bg:          FALLBACK_PANE_BG,
            border:      FALLBACK_PANE_BORDER,
            height:      28.0,
        }
    }
    pub fn title_color(mut self, c: Color32) -> Self { self.title_color = c; self }
    pub fn bg(mut self, c: Color32) -> Self { self.bg = c; self }
    pub fn border(mut self, c: Color32) -> Self { self.border = c; self }
    pub fn height(mut self, h: f32) -> Self { self.height = h; self }
    pub fn theme(self, t: &super::super::super::gpu::Theme) -> Self {
        self.title_color(t.accent).bg(t.toolbar_bg).border(t.toolbar_border)
    }
}

impl<'a> Widget for PaneHeader<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        let title = self.title;
        let title_color = self.title_color;
        pane_header_bar(ui, self.height, self.bg, self.border, |ui| {
            section_label_widget(ui, title, title_color);
        });
        // Return an invisible response covering the allocated area.
        ui.allocate_response(Vec2::ZERO, Sense::hover())
    }
}

// ─── PaneHeaderWithClose ─────────────────────────────────────────────────────

/// Builder for a pane header bar with close button. Returns `true` if close clicked.
/// Mirrors `components::panel_header(ui, title, title_color, open)`.
///
/// ```ignore
/// if PaneHeaderWithClose::new("Chart").theme(t).show(ui, &mut open) { ... }
/// ```
#[must_use]
pub struct PaneHeaderWithClose<'a> {
    title:       &'a str,
    title_color: Color32,
}

impl<'a> PaneHeaderWithClose<'a> {
    pub fn new(title: &'a str) -> Self {
        Self {
            title,
            title_color: FALLBACK_ACCENT,
        }
    }
    pub fn title_color(mut self, c: Color32) -> Self { self.title_color = c; self }
    pub fn theme(self, t: &super::super::super::gpu::Theme) -> Self {
        self.title_color(t.accent)
    }

    /// Render the header. Returns `true` if the close button was clicked.
    pub fn show(self, ui: &mut Ui, open: &mut bool) -> bool {
        super::super::components::panel_header(ui, self.title, self.title_color, open)
    }
}
