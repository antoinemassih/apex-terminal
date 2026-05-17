//! SidePanelShell — the canonical side-panel outer chrome.
//!
//! ## What it does
//!
//! Wraps `egui::SidePanel` + the chart-pane-parity `kit::PanelHeader` (or
//! `kit::PanelHeaderTabs`) + body padding into one builder call. Replaces the
//! 5 header types and 4 frame variants that today's panels reinvent — every
//! new side panel should reach for this widget instead of hand-rolling the
//! `SidePanel + PanelFrame + PanelHeaderWithClose` triple.
//!
//! ## Variants
//!
//! - [`SidePanelShell::new`] — static title shell (icon + title + optional
//!   trailing actions + close-X).
//! - [`SidePanelShell::tabs`] — tab-driven shell where the tabs *are* the
//!   title (orders, watchlist, dom).
//!
//! ## When to use
//!
//! - Any new side-docked panel (left or right rail).
//! - When migrating an existing panel that currently hand-builds the
//!   `SidePanel` + frame + header chrome.
//!
//! ## When NOT to use
//!
//! - Floating modals / settings dialogs — use `Header::dialog` + `Modal`.
//! - Chart pane chrome (the GPU-aligned in-pane header) — that lives in
//!   `chart/renderer/ui/chrome/painter_pane.rs` and has paint-pipeline
//!   constraints this widget does not share.
//! - Multi-section feed/signals/analysis panels — those have their own
//!   shell [`SplitSectionPanel`](super::split_section_panel::SplitSectionPanel).
//!
//! ## Sister widgets
//!
//! - [`SplitSectionPanel`](super::split_section_panel::SplitSectionPanel) —
//!   multi-pane shell for feed/signals/analysis.
//! - [`PanelFooter`](super::panel_footer::PanelFooter) — sticky bottom
//!   action band (primary/secondary + meta).

#![allow(dead_code)]

use std::ops::RangeInclusive;

use egui::{Context, Ui};

use super::placement::Side;
use crate::chart::renderer::ui::panels::kit::{PanelHeader, PanelHeaderTabs};
use crate::chart::renderer::ui::components::frames_widget::PanelFrame;
use crate::chart::renderer::ui::style::{gap_lg, gap_md, gap_sm};
use crate::chart_renderer::gpu::{Theme, Watchlist};

/// Response from rendering a [`SidePanelShell`] / [`SidePanelShellTabs`] /
/// [`SplitSectionPanel`]. Caller writes its own open-flag back when
/// `close_clicked` is `true`. Returned by value so the shell never holds a
/// mutable borrow of the caller's flag during body execution.
#[derive(Copy, Clone, Debug, Default)]
pub struct SidePanelShellResponse {
    /// `true` if the close-X was clicked this frame.
    pub close_clicked: bool,
}

/// Width preset for side panels. Default presets cover ~95% of cases; if a
/// panel needs an off-preset width it can still call `.resizable(min..=max)`
/// to override the bounds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Width {
    /// 240px — tight rails (alerts, signals).
    Narrow,
    /// 300px — default for general content (watchlist, news).
    Medium,
    /// 400px — feed-style multi-section content.
    Wide,
}

impl Width {
    /// Default starting width for the preset.
    pub fn px(self) -> f32 {
        match self {
            Width::Narrow => 240.0,
            Width::Medium => 300.0,
            Width::Wide => 400.0,
        }
    }
    /// Default resize bounds — generous enough that the preset comfortably
    /// covers shrinking + growing without callers needing to override.
    pub fn bounds(self) -> RangeInclusive<f32> {
        match self {
            Width::Narrow => 180.0..=360.0,
            Width::Medium => 220.0..=480.0,
            Width::Wide => 300.0..=620.0,
        }
    }
}

impl Default for Width {
    fn default() -> Self { Width::Medium }
}

// ─── Static-title shell ──────────────────────────────────────────────────────

/// Static-title side panel shell. Use `.show(...)` to render.
#[must_use = "SidePanelShell must be rendered with `.show(...)`"]
pub struct SidePanelShell<'a> {
    id: &'a str,
    title: &'a str,
    icon: Option<&'a str>,
    width: Width,
    width_bounds: Option<RangeInclusive<f32>>,
    side: Side,
    pane_metrics: Option<(f32, f32)>,
    header_actions: Option<Box<dyn FnOnce(&mut Ui, &Theme) + 'a>>,
    footer: Option<Box<dyn FnOnce(&mut Ui, &Theme) + 'a>>,
}

impl<'a> SidePanelShell<'a> {
    /// Construct a static-title shell.
    pub fn new(id: &'a str, title: &'a str) -> Self {
        Self {
            id,
            title,
            icon: None,
            width: Width::default(),
            width_bounds: None,
            side: Side::Right,
            pane_metrics: None,
            header_actions: None,
            footer: None,
        }
    }

    /// Optional leading icon (glyph string from `ui_kit::icons::Icon`).
    pub fn icon(mut self, icon: &'a str) -> Self { self.icon = Some(icon); self }

    /// Set the width preset. Default is [`Width::Medium`].
    pub fn width(mut self, w: Width) -> Self { self.width = w; self }

    /// Override the resize bounds. If not called, the preset's wide-enough
    /// defaults are used.
    pub fn resizable(mut self, bounds: RangeInclusive<f32>) -> Self {
        self.width_bounds = Some(bounds);
        self
    }

    /// Dock side (Right by default). Use [`Side::Left`] for left-rail panels.
    pub fn side(mut self, side: Side) -> Self { self.side = side; self }

    /// Pin the header height + title font to the chart-pane metrics so an
    /// open panel lines up pixel-y with the pane header above it. Convenience
    /// over [`Self::pane_metrics`] for callers that don't have a borrow
    /// conflict; internally just resolves the two values.
    pub fn pane_aligned(mut self, wl: &Watchlist) -> Self {
        self.pane_metrics = Some((
            crate::chart_renderer::gpu::pane_tabs_header_h(wl),
            wl.pane_header_size.title_font(),
        ));
        self
    }

    /// Pin the header height + title font to caller-resolved values. Use this
    /// when `.pane_aligned(&watchlist)` conflicts with `&mut watchlist` in the
    /// body — resolve the metrics before borrowing.
    pub fn pane_metrics(mut self, height: f32, title_font: f32) -> Self {
        self.pane_metrics = Some((height, title_font));
        self
    }

    /// Trailing header actions, painted to the LEFT of the close-X.
    pub fn header_actions(mut self, f: impl FnOnce(&mut Ui, &Theme) + 'a) -> Self {
        self.header_actions = Some(Box::new(f));
        self
    }

    /// Optional sticky footer band painted below the body.
    pub fn footer(mut self, f: impl FnOnce(&mut Ui, &Theme) + 'a) -> Self {
        self.footer = Some(Box::new(f));
        self
    }

    /// Render the shell. Returns a [`SidePanelShellResponse`] — caller writes
    /// its open-flag back to `false` if `close_clicked` is `true`. The body
    /// closure is called inside the standard panel padding (LR `gap_md`, top
    /// `gap_sm` under header, bottom `gap_lg`).
    ///
    /// The caller is responsible for the open/closed early-return — only call
    /// `.show()` when the panel is open.
    pub fn show(
        self,
        ctx: &Context,
        t: &Theme,
        body: impl FnOnce(&mut Ui, &Theme),
    ) -> SidePanelShellResponse {
        let panel = build_side_panel(self.id, self.side, self.width, self.width_bounds.as_ref());
        let frame = PanelFrame::new(t.toolbar_bg, t.toolbar_border).theme(t).build();
        let panel = panel.frame(frame);

        let SidePanelShell { title, icon, pane_metrics, header_actions, footer, .. } = self;

        let mut close_clicked = false;
        panel.show(ctx, |ui| {
            let closed = render_header(ui, t, title, icon, pane_metrics, header_actions);
            if closed { close_clicked = true; }
            render_body_and_footer(ui, t, body, footer);
        });
        SidePanelShellResponse { close_clicked }
    }

    // ── Sibling constructor for tab-driven shells ─────────────────────────

    /// Tab-driven sibling constructor — the tab strip *is* the title. Returns
    /// a [`SidePanelShellTabs`] builder.
    pub fn tabs<T: PartialEq + Copy + 'a>(
        id: &'a str,
        current: &'a mut T,
        tabs: &'a [(T, &'a str, Option<&'a str>)],
    ) -> SidePanelShellTabs<'a, T> {
        SidePanelShellTabs {
            id,
            current,
            tabs,
            width: Width::default(),
            width_bounds: None,
            side: Side::Right,
            pane_metrics: None,
            header_actions: None,
            footer: None,
        }
    }
}

// ─── Tab-driven shell ────────────────────────────────────────────────────────

/// Tab-driven side panel shell. See [`SidePanelShell::tabs`].
#[must_use = "SidePanelShellTabs must be rendered with `.show(...)`"]
pub struct SidePanelShellTabs<'a, T: PartialEq + Copy> {
    id: &'a str,
    current: &'a mut T,
    // (value, label, optional leading glyph — kept in API for future use)
    tabs: &'a [(T, &'a str, Option<&'a str>)],
    width: Width,
    width_bounds: Option<RangeInclusive<f32>>,
    side: Side,
    pane_metrics: Option<(f32, f32)>,
    header_actions: Option<Box<dyn FnOnce(&mut Ui, &Theme) + 'a>>,
    footer: Option<Box<dyn FnOnce(&mut Ui, &Theme) + 'a>>,
}

impl<'a, T: PartialEq + Copy + 'a> SidePanelShellTabs<'a, T> {
    pub fn width(mut self, w: Width) -> Self { self.width = w; self }
    pub fn resizable(mut self, bounds: RangeInclusive<f32>) -> Self {
        self.width_bounds = Some(bounds); self
    }
    pub fn side(mut self, side: Side) -> Self { self.side = side; self }
    /// See [`SidePanelShell::pane_aligned`].
    pub fn pane_aligned(mut self, wl: &Watchlist) -> Self {
        self.pane_metrics = Some((
            crate::chart_renderer::gpu::pane_tabs_header_h(wl),
            wl.pane_header_size.title_font(),
        ));
        self
    }
    /// See [`SidePanelShell::pane_metrics`].
    pub fn pane_metrics(mut self, height: f32, title_font: f32) -> Self {
        self.pane_metrics = Some((height, title_font));
        self
    }
    pub fn header_actions(mut self, f: impl FnOnce(&mut Ui, &Theme) + 'a) -> Self {
        self.header_actions = Some(Box::new(f)); self
    }
    pub fn footer(mut self, f: impl FnOnce(&mut Ui, &Theme) + 'a) -> Self {
        self.footer = Some(Box::new(f)); self
    }

    /// Render. Caller is responsible for the open/closed early-return — only
    /// call `.show()` when the panel is open. Returns
    /// [`SidePanelShellResponse`] for the close-X click.
    pub fn show(
        self,
        ctx: &Context,
        t: &Theme,
        body: impl FnOnce(&mut Ui, &Theme, T),
    ) -> SidePanelShellResponse {
        let panel = build_side_panel(self.id, self.side, self.width, self.width_bounds.as_ref());
        let frame = PanelFrame::new(t.toolbar_bg, t.toolbar_border).theme(t).build();
        let panel = panel.frame(frame);

        let SidePanelShellTabs {
            id, current, tabs, pane_metrics, header_actions, footer, ..
        } = self;

        let mut close_clicked = false;
        panel.show(ctx, |ui| {
            // Strip the glyph for the underlying PanelHeaderTabs widget which
            // takes `&[(T, &str)]`. Glyphs are reserved for a future enhancement.
            let stripped: Vec<(T, &str)> = tabs.iter().map(|(v, l, _)| (*v, *l)).collect();
            let mut header = PanelHeaderTabs::new(current, &stripped).id_salt(id);
            if let Some((h, f)) = pane_metrics {
                header = header.height(h).font_size(f);
            }

            let mut actions = header_actions;
            let closed = header.show_with(ui, t, |ui| {
                if let Some(a) = actions.take() { a(ui, t); }
            });
            if closed { close_clicked = true; }

            let active = *current;
            render_body_and_footer(ui, t, move |ui, t| body(ui, t, active), footer);
        });
        SidePanelShellResponse { close_clicked }
    }
}

// ─── Internal helpers ────────────────────────────────────────────────────────

fn build_side_panel(
    id: &str,
    side: Side,
    width: Width,
    override_bounds: Option<&RangeInclusive<f32>>,
) -> egui::SidePanel {
    let bounds = override_bounds.cloned().unwrap_or_else(|| width.bounds());
    // SidePanel::{left,right} require an `Into<Id>` — egui::Id::new accepts any
    // hashable value, sidestepping the &'static str constraint on string-id
    // overload.
    let egui_id = egui::Id::new(("ui_kit_side_panel_shell", id));
    let panel = match side {
        Side::Left => egui::SidePanel::left(egui_id),
        // Right is the default; Top/Bottom fall back to right (shell is side-only).
        _ => egui::SidePanel::right(egui_id),
    };
    panel
        .default_width(width.px())
        .min_width(*bounds.start())
        .max_width(*bounds.end())
        .resizable(true)
}

fn render_header<'a>(
    ui: &mut Ui,
    t: &Theme,
    title: &'a str,
    icon: Option<&'a str>,
    pane_metrics: Option<(f32, f32)>,
    actions: Option<Box<dyn FnOnce(&mut Ui, &Theme) + 'a>>,
) -> bool {
    let mut header = PanelHeader::new(title);
    if let Some(g) = icon { header = header.icon(g); }
    if let Some((h, f)) = pane_metrics {
        header = header.height(h).font_size(f);
    }

    let mut taken = actions;
    header.show_with(ui, t, |ui| {
        if let Some(a) = taken.take() { a(ui, t); }
    })
}

fn render_body_and_footer<'a>(
    ui: &mut Ui,
    t: &Theme,
    body: impl FnOnce(&mut Ui, &Theme),
    footer: Option<Box<dyn FnOnce(&mut Ui, &Theme) + 'a>>,
) {
    // Body padding (spec): LR gap_md, top gap_sm under header, bottom gap_lg.
    let body_frame = egui::Frame::NONE.inner_margin(egui::Margin {
        left:   gap_md() as i8,
        right:  gap_md() as i8,
        top:    gap_sm() as i8,
        bottom: gap_lg() as i8,
    });

    if let Some(f) = footer {
        // Reserve the footer at the bottom of the available area first.
        egui::TopBottomPanel::bottom(ui.id().with("side_panel_shell_footer"))
            .frame(egui::Frame::NONE)
            .show_inside(ui, |ui| { f(ui, t); });
        body_frame.show(ui, |ui| { body(ui, t); });
    } else {
        body_frame.show(ui, |ui| { body(ui, t); });
    }
}
