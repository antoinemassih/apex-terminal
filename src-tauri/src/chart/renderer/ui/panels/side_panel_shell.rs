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
use crate::ui_kit::sx::Tone;

use egui::{Context, Ui};

use crate::ui_kit::widgets::placement::Side;
use super::kit::{PanelHeader, PanelHeaderTabs};
use crate::ui_kit::widgets::frames::PanelFrame;
use crate::ui_kit::tokens::{
    gap_lg, gap_md, gap_sm, shadow_color_alpha, stroke_thin,
};
use crate::ui_kit::widgets::theme::ComponentTheme;
use crate::chart_renderer::gpu::Theme;
// Watchlist + pane_tabs_header_h come from the chart-app gpu state — this
// file lives in chart_renderer now, so import them directly.
use crate::chart_renderer::gpu::{Watchlist, pane_tabs_header_h};

/// Response from rendering a [`SidePanelShell`] / [`SidePanelShellTabs`] /
/// [`SplitSectionPanel`]. Caller writes its own open-flag back when
/// `close_clicked` is `true`. Returned by value so the shell never holds a
/// mutable borrow of the caller's flag during body execution.
#[derive(Copy, Clone, Debug, Default)]
pub struct SidePanelShellResponse {
    /// `true` if the close-X was clicked this frame.
    pub close_clicked: bool,
}

// ─── Rail-slot mode (the right_rail manager drives this) ──────────────────────
//
// When the right_rail manager is laying panels out, it sets a per-panel slot
// (a rect on the rail's egui layer + the panel's current height) just before
// calling that panel's existing `draw(...)`. SidePanelShell then renders its
// body INTO that slot — as a child Ui with a size-control header — instead of
// creating its own `egui::SidePanel`. The panel's own code is unchanged.

use crate::chart_renderer::ui::panels::rail_layout::RailHeight;

/// An assigned rail slot for the panel about to render.
#[derive(Clone, Copy)]
pub(crate) struct RailSlot {
    pub rect: egui::Rect,
    pub layer: egui::LayerId,
    pub height: RailHeight,
}

// Rail-slot plumbing: the manager passes a `RailSlot` into the shell builder via
// `.rail_slot(Some(slot))`. The split-toggle click is reported back through one
// shared egui frame-memory flag — NOT keyed by panel id. The rail renders panels
// one slot at a time (set slot → render → read), so a single flag is unambiguous:
// whatever panel just rendered is the one whose toggle was clicked. (Keying by id
// was fragile — the shell id and the registry id differ for most panels.)
fn cycle_mem_id() -> egui::Id { egui::Id::new("rail_size_cycle_pending") }

/// Rail manager: read-and-clear the split-toggle request for the panel that just
/// rendered (one-shot). Call immediately after each panel's render.
pub(crate) fn take_size_cycle(ctx: &Context) -> bool {
    let mid = cycle_mem_id();
    let v = ctx.data(|d| d.get_temp::<bool>(mid).unwrap_or(false));
    if v { ctx.data_mut(|d| d.insert_temp(mid, false)); }
    v
}

/// Create the child Ui a panel renders into when in rail-slot mode, painting
/// the slot's body surface first so the real header band composites over it
/// exactly as in standalone mode.
fn rail_slot_ui(ctx: &Context, id: &str, slot: RailSlot, t: &Theme) -> Ui {
    let mut ui = Ui::new(
        ctx.clone(),
        egui::Id::new(("rail_slot_ui", id)),
        egui::UiBuilder::new().max_rect(slot.rect).layer_id(slot.layer),
    );
    ui.set_clip_rect(slot.rect);
    // Slot body surface (matches the standalone panel frame fill).
    ui.painter().rect_filled(slot.rect, egui::CornerRadius::ZERO, t.panel_surface());
    ui
}

/// The size-cycle control (`1`/`½`/`⅓`) injected into a rail panel's header,
/// to the LEFT of the close button. Returns `true` if clicked (the right_rail
/// manager applies the cycle next frame). Rendered as a header action so it
/// sits in the same trailing slot as the real header's other actions.
fn render_rail_size_control(ui: &mut Ui, t: &Theme, height: RailHeight) {
    // Split toggle: Full ⇄ Half. Depicts two stacked cells (vertical rail split);
    // highlighted when already split (Half).
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(20.0, 16.0), egui::Sense::click());
    if resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
    let split = height == RailHeight::Half;
    let col = if resp.hovered() || split { t.accent } else { t.dim };
    let stroke = egui::Stroke::new(1.2, col);
    let r = rect.shrink2(egui::vec2(3.0, 2.0));
    ui.painter().rect_stroke(r, egui::CornerRadius::same(2), stroke, egui::StrokeKind::Inside);
    // Horizontal divider → two stacked halves (the rail splits vertically).
    let y = r.center().y;
    ui.painter().line_segment([egui::pos2(r.left(), y), egui::pos2(r.right(), y)], stroke);
    if resp.clicked() {
        ui.ctx().data_mut(|d| d.insert_temp(cycle_mem_id(), true));
    }
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
    rail_slot: Option<RailSlot>,
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
            rail_slot: None,
        }
    }

    /// Render into a rail slot instead of an `egui::SidePanel`. When `Some`, the
    /// real header + body paint into the manager-assigned rect (with the
    /// size-cycle control injected into the header actions), and
    /// [`SidePanelShellResponse::size_cycle_requested`] reports clicks. `None`
    /// (the default) renders the normal docked side panel.
    pub(crate) fn rail_slot(mut self, slot: Option<RailSlot>) -> Self {
        self.rail_slot = slot; self
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
            pane_tabs_header_h(wl),
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
        // Rail-slot mode: render the REAL header + body into the assigned slot
        // (not a crude strip) so the panel keeps its native header styling. The
        // only addition is the size-cycle control, injected left of the close.
        if let Some(slot) = self.rail_slot {
            let SidePanelShell { id, title, icon, pane_metrics, header_actions, footer, .. } = self;
            let mut ui = rail_slot_ui(ctx, id, slot, t);
            let mut close_clicked = false;
            let header_resp = crate::ui_kit::widgets::OutlinedBox::new()
                .fill(t.header_surface()).borderless().square().padding(0.0)
                .show(&mut ui, t, |ui| {
                    let actions = move |ui: &mut Ui, t: &Theme| {
                        render_rail_size_control(ui, t, slot.height);
                        if let Some(a) = header_actions { a(ui, t); }
                    };
                    let closed = render_header(ui, t, title, icon, pane_metrics, Some(Box::new(actions)));
                    if closed { close_clicked = true; }
                });
            paint_header_underline_and_shadow(&mut ui, t, header_resp.response.rect, id);
            render_body_and_footer(&mut ui, t, body, footer);
            return SidePanelShellResponse { close_clicked };
        }
        let panel = build_side_panel(self.id, self.side, self.width, self.width_bounds.as_ref());
        let mut frame = PanelFrame::new(t.panel_surface(), t.toolbar_border).theme(t).build();
        // Shell region: float the rail as a rounded card when the active style
        // is tiled (Aperture/Glass). Gap on all sides separates it from the
        // workspace (left) and window edge (right).
        let rgap = crate::chart_renderer::ui::style::region_gap();
        if rgap > 0.0 {
            let rr = crate::chart_renderer::ui::style::current().region_radius as u8;
            frame = frame
                .outer_margin(egui::Margin { left: rgap as i8, right: rgap as i8, top: 0, bottom: rgap as i8 })
                .corner_radius(egui::CornerRadius::same(rr));
        }
        let panel = panel.frame(frame);

        let SidePanelShell { id, title, icon, pane_metrics, header_actions, footer, .. } = self;

        let mut close_clicked = false;
        panel.show(ctx, |ui| {
            // Wrap the header in a Frame filled with `t.header_surface()`
            // so the side panel's header band matches the chart pane
            // header above it — same fill, same visual weight.
            let header_resp = crate::ui_kit::widgets::OutlinedBox::new()
                .fill(t.header_surface())
                .borderless()
                .square()
                .padding(0.0)
                .show(ui, t, |ui| {
                    let closed = render_header(ui, t, title, icon, pane_metrics, header_actions);
                    if closed { close_clicked = true; }
                });
            paint_header_underline_and_shadow(ui, t, header_resp.response.rect, id);
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
            rail_slot: None,
            on_tab_secondary: None,
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
    rail_slot: Option<RailSlot>,
    on_tab_secondary: Option<Box<dyn FnMut(&mut Ui, T) + 'a>>,
}

impl<'a, T: PartialEq + Copy + 'a> SidePanelShellTabs<'a, T> {
    /// See [`SidePanelShell::rail_slot`].
    pub(crate) fn rail_slot(mut self, slot: Option<RailSlot>) -> Self {
        self.rail_slot = slot; self
    }

    /// Per-tab right-click handler, forwarded to [`PanelHeaderTabs::on_tab_secondary`].
    /// Used to offer "open this tab as a new instance" in the rail.
    pub(crate) fn on_tab_secondary(mut self, f: impl FnMut(&mut Ui, T) + 'a) -> Self {
        self.on_tab_secondary = Some(Box::new(f)); self
    }

    pub fn width(mut self, w: Width) -> Self { self.width = w; self }
    pub fn resizable(mut self, bounds: RangeInclusive<f32>) -> Self {
        self.width_bounds = Some(bounds); self
    }
    pub fn side(mut self, side: Side) -> Self { self.side = side; self }
    /// See [`SidePanelShell::pane_aligned`].
    pub fn pane_aligned(mut self, wl: &Watchlist) -> Self {
        self.pane_metrics = Some((
            pane_tabs_header_h(wl),
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
        // Rail-slot mode: render the REAL tab header (with tab switching intact)
        // + size control + active body into the assigned slot.
        if let Some(slot) = self.rail_slot {
            let SidePanelShellTabs { id, current, tabs, pane_metrics, header_actions, footer, on_tab_secondary, .. } = self;
            let mut ui = rail_slot_ui(ctx, id, slot, t);
            let mut close_clicked = false;
            let stripped: Vec<(T, &str)> = tabs.iter().map(|(v, l, _)| (*v, *l)).collect();
            let mut header = PanelHeaderTabs::new(current, &stripped).id_salt(id);
            if let Some((h, f)) = pane_metrics { header = header.height(h).font_size(f); }
            if let Some(cb) = on_tab_secondary { header = header.on_tab_secondary(cb); }
            let mut actions = header_actions;
            let header_resp = crate::ui_kit::widgets::OutlinedBox::new()
                .fill(t.header_surface()).borderless().square().padding(0.0)
                .show(&mut ui, t, |ui| {
                    let closed = header.show_with(ui, t, |ui| {
                        render_rail_size_control(ui, t, slot.height);
                        if let Some(a) = actions.take() { a(ui, t); }
                    });
                    if closed { close_clicked = true; }
                });
            paint_header_underline_and_shadow(&mut ui, t, header_resp.response.rect, id);
            let active = *current;
            render_body_and_footer(&mut ui, t, move |ui, t| body(ui, t, active), footer);
            return SidePanelShellResponse { close_clicked };
        }
        let panel = build_side_panel(self.id, self.side, self.width, self.width_bounds.as_ref());
        let mut frame = PanelFrame::new(t.panel_surface(), t.toolbar_border).theme(t).build();
        // Shell region: float the rail as a rounded card when the active style
        // is tiled (Aperture/Glass). Gap on all sides separates it from the
        // workspace (left) and window edge (right).
        let rgap = crate::chart_renderer::ui::style::region_gap();
        if rgap > 0.0 {
            let rr = crate::chart_renderer::ui::style::current().region_radius as u8;
            frame = frame
                .outer_margin(egui::Margin { left: rgap as i8, right: rgap as i8, top: 0, bottom: rgap as i8 })
                .corner_radius(egui::CornerRadius::same(rr));
        }
        let panel = panel.frame(frame);

        let SidePanelShellTabs {
            id, current, tabs, pane_metrics, header_actions, footer, on_tab_secondary, ..
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
            if let Some(cb) = on_tab_secondary { header = header.on_tab_secondary(cb); }

            let mut actions = header_actions;
            // Same header_surface wrap as the static-title variant for
            // visual parity with the chart pane header.
            let header_resp = crate::ui_kit::widgets::OutlinedBox::new()
                .fill(t.header_surface())
                .borderless()
                .square()
                .padding(0.0)
                .show(ui, t, |ui| {
                    let closed = header.show_with(ui, t, |ui| {
                        if let Some(a) = actions.take() { a(ui, t); }
                    });
                    if closed { close_clicked = true; }
                });
            paint_header_underline_and_shadow(ui, t, header_resp.response.rect, id);

            let active = *current;
            render_body_and_footer(ui, t, move |ui, t| body(ui, t, active), footer);
        });
        SidePanelShellResponse { close_clicked }
    }
}

/// Paint a hairline underline + inset drop shadow below a header strip.
/// Mirrors the chart pane header treatment so side-panel headers and
/// chart pane headers read as one visual family. The shadow goes on the
/// Foreground layer so it can't shift widget layer-ids mid-frame.
fn paint_header_underline_and_shadow(ui: &mut Ui, t: &Theme, hr: egui::Rect, panel_id: &str) {
    // Hairline bottom rule — same color/alpha as the chart pane header
    // border so the whole header family reads as one bordered system.
    ui.painter().line_segment(
        [
            egui::Pos2::new(hr.left(), hr.bottom() - 0.5),
            egui::Pos2::new(hr.right(), hr.bottom() - 0.5),
        ],
        egui::Stroke::new(stroke_thin(), t.header_border()),
    );
    // Inset drop shadow falling into the body. 6px gradient, fading
    // alpha 38 → 0 over the height. Foreground layer keeps it from
    // shifting widget layer-ids during interaction.
    let shadow_h = 6.0_f32;
    let layer = egui::LayerId::new(
        egui::Order::Foreground,
        ui.id().with(("side_panel_shell_shadow", panel_id)),
    );
    let painter = ui.ctx().layer_painter(layer);
    for i in 0..shadow_h as i32 {
        let frac = 1.0 - (i as f32 / shadow_h);
        let a = (38.0 * frac) as u8;
        if a == 0 { continue; }
        let y = hr.bottom() + i as f32 + 0.5;
        painter.line_segment(
            [egui::Pos2::new(hr.left(), y), egui::Pos2::new(hr.right(), y)],
            egui::Stroke::new(stroke_thin(), shadow_color_alpha(t, a)),
        );
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
    // Body padding: ZERO LR + ZERO TOP — PanelSection paints
    // edge-to-edge header strips, and a section that's the first child
    // in the body should sit FLUSH against the SidePanelShell header
    // above it so the inset shadow from the panel header falls
    // directly onto the section header below. The LR inset that used
    // to live here moved into PanelSection's body Frame.
    let body_frame = egui::Frame::NONE.inner_margin(egui::Margin {
        left:   0,
        right:  0,
        top:    0,
        bottom: gap_lg() as i8,
    });

    if let Some(f) = footer {
        // Reserve the footer at the bottom of the available area first.
        egui::TopBottomPanel::bottom(ui.id().with("side_panel_shell_footer"))
            .frame(egui::Frame::NONE)
            .show_inside(ui, |ui| {
                // Pinned footer as an elevated rounded *card* (Aperture/Glass P&L
                // block) vs flat band — driven by the panel_footer_card token.
                let cst = crate::chart_renderer::ui::style::current();
                if cst.panel_footer_card {
                    let card = egui::Frame::NONE
                        .fill(t.toolbar_bg)
                        .corner_radius(egui::CornerRadius::same(cst.panel_footer_radius as u8))
                        .stroke(egui::Stroke::new(
                            stroke_thin(),
                            crate::chart_renderer::ui::style::tint(t, Tone::Border, 60),
                        ))
                        .inner_margin(egui::Margin::same(gap_sm() as i8))
                        .outer_margin(egui::Margin::same(gap_sm() as i8));
                    card.show(ui, |ui| { f(ui, t); });
                } else {
                    f(ui, t);
                }
            });
        body_frame.show(ui, |ui| { body(ui, t); });
    } else {
        body_frame.show(ui, |ui| { body(ui, t); });
    }
}
