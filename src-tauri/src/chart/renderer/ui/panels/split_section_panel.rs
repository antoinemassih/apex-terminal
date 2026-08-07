//! SplitSectionPanel — multi-pane side panel shell.
//!
//! ## What it does
//!
//! Collapses the feed / signals / analysis triplication (~250 lines of
//! copy-pasted divider + tab-strip + close-X code across three sibling
//! panel modules) into a single widget. The caller owns:
//!
//! 1. The list of [`SplitSection<T>`] (each section is a `(tab, frac)`
//!    pair) — usually persisted on the watchlist struct.
//! 2. A static list of available tab kinds (`&[(T, label)]`).
//! 3. A `body` closure that paints content for whichever tab is active in
//!    the current section.
//!
//! The widget owns the outer `egui::SidePanel`, the chart-pane-aligned
//! header (`kit::PanelHeader`) with optional "+" action, the per-section
//! tab bars, the draggable dividers between sections, and the per-section
//! close-X (hidden when only one section remains).
//!
//! ## When to use
//!
//! - Feed, signals, analysis panels — anything where the panel body is a
//!   stack of vertically-divided sections each picking from a shared set
//!   of tabs.
//!
//! ## When NOT to use
//!
//! - Single-section panels — use
//!   [`SidePanelShell`](super::side_panel_shell::SidePanelShell).
//! - Tab-driven panels where the tabs *are* the title (orders,
//!   watchlist) — use [`SidePanelShell::tabs`].
//!
//! ## Sister widgets
//!
//! - [`SidePanelShell`](super::side_panel_shell::SidePanelShell) — single
//!   section variant.
//! - [`PanelFooter`](super::panel_footer::PanelFooter) — sticky bottom
//!   actions.

#![allow(dead_code)]

use std::ops::RangeInclusive;
use crate::chart_renderer::ui::style::tint;
use crate::ui_kit::sx::Tone;

use egui::{Context, Pos2, Sense, Stroke, Ui, Vec2};

use crate::ui_kit::widgets::placement::Side;
use super::side_panel_shell::{SidePanelShellResponse, Width};
use crate::ui_kit::widgets::frames::PanelFrame;
use super::kit::PanelHeader;
use crate::ui_kit::widgets::Tooltip;
use crate::ui_kit::widgets::icon_placement::IconPlacement;
use crate::ui_kit::tokens::{
    alpha_faint, split_divider, stroke_thin,
};
use crate::ui_kit::widgets::theme::ComponentTheme;
use crate::chart_renderer::gpu::Theme;
// SplitSection / Watchlist / pane_tabs_header_h / panel_action_btn come from
// chart-app code — this file lives in chart_renderer now, so import directly.
use crate::chart_renderer::gpu::{SplitSection, Watchlist, pane_tabs_header_h};
use super::kit::panel_action_btn;

const TAB_BAR_H: f32 = 28.0;
const DIVIDER_H: f32 = 6.0;

/// Multi-section side panel shell. See module docs.
#[must_use = "SplitSectionPanel must be rendered with `.show(...)`"]
pub struct SplitSectionPanel<'a, T: PartialEq + Copy + Clone + 'a> {
    id: &'a str,
    title: &'a str,
    icon: Option<&'a str>,
    splits: &'a mut Vec<SplitSection<T>>,
    available_tabs: &'a [(T, &'a str)],
    default_tab: Option<T>,
    width: Width,
    width_bounds: Option<RangeInclusive<f32>>,
    side: Side,
    pane_metrics: Option<(f32, f32)>,
    min_section_height: f32,
    allow_add: bool,
}

impl<'a, T: PartialEq + Copy + Clone + 'a> SplitSectionPanel<'a, T> {
    pub fn new(id: &'a str, splits: &'a mut Vec<SplitSection<T>>) -> Self {
        Self {
            id,
            title: "",
            icon: None,
            splits,
            available_tabs: &[],
            default_tab: None,
            width: Width::Wide,
            width_bounds: None,
            side: Side::Right,
            pane_metrics: None,
            min_section_height: 30.0,
            allow_add: true,
        }
    }

    /// Title text for the header bar.
    pub fn title(mut self, title: &'a str) -> Self { self.title = title; self }
    /// Optional leading icon glyph.
    pub fn icon(mut self, icon: &'a str) -> Self { self.icon = Some(icon); self }

    /// The list of tab kinds the user can pick from inside any section.
    /// Required — the per-section tab strip iterates this slice.
    pub fn tabs(mut self, tabs: &'a [(T, &'a str)]) -> Self {
        self.available_tabs = tabs;
        self
    }

    /// Tab to assign to a freshly-added section when the "+" button can't
    /// find an unused tab in [`Self::tabs`]. Defaults to the first entry.
    pub fn default_tab(mut self, tab: T) -> Self { self.default_tab = Some(tab); self }

    pub fn width(mut self, w: Width) -> Self { self.width = w; self }
    pub fn resizable(mut self, bounds: RangeInclusive<f32>) -> Self {
        self.width_bounds = Some(bounds); self
    }
    pub fn side(mut self, side: Side) -> Self { self.side = side; self }
    /// See [`super::side_panel_shell::SidePanelShell::pane_aligned`].
    pub fn pane_aligned(mut self, wl: &Watchlist) -> Self {
        self.pane_metrics = Some((
            pane_tabs_header_h(wl),
            wl.pane_header_size.title_font(),
        ));
        self
    }
    /// See [`super::side_panel_shell::SidePanelShell::pane_metrics`].
    pub fn pane_metrics(mut self, height: f32, title_font: f32) -> Self {
        self.pane_metrics = Some((height, title_font));
        self
    }
    /// Minimum pixel height for any section. Default 30px.
    pub fn min_section_height(mut self, h: f32) -> Self {
        self.min_section_height = h; self
    }
    /// Disable the header "+" add-section button.
    pub fn allow_add(mut self, on: bool) -> Self { self.allow_add = on; self }

    /// Render. Caller is responsible for the open/closed early-return — only
    /// call `.show()` when the panel is open. Returns
    /// [`SidePanelShellResponse`] for the close-X click. The `body` closure is
    /// called once per section with `(section_idx, frac)`.
    pub fn show(
        self,
        ctx: &Context,
        t: &Theme,
        mut body: impl FnMut(&mut Ui, &Theme, usize, f32),
    ) -> SidePanelShellResponse {
        let bounds = self.width_bounds.clone().unwrap_or_else(|| self.width.bounds());
        let egui_id = egui::Id::new(("ui_kit_split_section_panel", self.id));
        let panel = match self.side {
            Side::Left => egui::SidePanel::left(egui_id),
            _ => egui::SidePanel::right(egui_id),
        }
        .default_width(self.width.px())
        .min_width(*bounds.start())
        .max_width(*bounds.end())
        .resizable(true)
        .frame(PanelFrame::new(t.toolbar_bg, t.toolbar_border).theme(t).build());

        let SplitSectionPanel {
            title, icon, splits, available_tabs, default_tab, pane_metrics,
            min_section_height, allow_add, ..
        } = self;

        let mut close_clicked = false;
        panel.show(ctx, |ui| {
            // ── Header ────────────────────────────────────────────────────
            let mut header = PanelHeader::new(title).numbered();
            if let Some(g) = icon { header = header.icon(g); }
            if let Some((h, f)) = pane_metrics {
                header = header.height(h).font_size(f);
            }

            let mut add_clicked = false;
            let closed = header.show_with(ui, t, |ui| {
                if allow_add {
                    // Reuse the kit's panel_action_btn for a consistent ghost-+.
                    if panel_action_btn(
                        ui, "+", t.dim,
                    ) {
                        add_clicked = true;
                    }
                }
            });
            if closed { close_clicked = true; }

            if add_clicked && !available_tabs.is_empty() {
                let used: Vec<T> = splits.iter().map(|s| s.tab).collect();
                let next = available_tabs.iter()
                    .find(|(tv, _)| !used.contains(tv))
                    .map(|(tv, _)| *tv)
                    .or(default_tab)
                    .unwrap_or(available_tabs[0].0);
                if let Some(last) = splits.last_mut() { last.frac *= 0.5; }
                let frac = splits.last().map(|s| s.frac).unwrap_or(1.0);
                splits.push(SplitSection::new(next, frac));
            }

            // ── Ensure at least one section ──────────────────────────────
            if splits.is_empty() {
                if let Some(d) = default_tab.or_else(|| available_tabs.first().map(|(v, _)| *v)) {
                    splits.push(SplitSection::new(d, 1.0));
                } else {
                    return;
                }
            }

            // ── Compute per-section heights ──────────────────────────────
            let available_h = ui.available_height();
            let n = splits.len();
            let divider_total = n.saturating_sub(1) as f32 * DIVIDER_H;
            let tab_bar_total = n as f32 * TAB_BAR_H;
            let content_h = (available_h - divider_total - tab_bar_total).max(40.0);
            let total_frac: f32 = splits.iter().map(|s| s.frac).sum();
            let norm = if total_frac > 0.001 { 1.0 / total_frac } else { 1.0 };
            let heights: Vec<f32> = splits.iter()
                .map(|s| (s.frac * norm * content_h).max(min_section_height))
                .collect();

            let mut remove_idx: Option<usize> = None;
            let mut divider_drags: Vec<(usize, f32)> = Vec::new();

            for i in 0..n {
                let active = splits[i].tab;
                let h_body = heights[i];
                let can_close = n > 1;

                // ── Per-section tab strip + close-X ───────────────────────
                let mut new_tab: Option<T> = None;
                ui.horizontal(|ui| {
                    ui.set_min_height(TAB_BAR_H - 2.0);
                    for (tv, label) in available_tabs {
                        let sel = active == *tv;
                        let resp = crate::ui_kit::widgets::Button::new(*label)
                            .variant(crate::ui_kit::widgets::tokens::Variant::Tab)
                            .active(sel)
                            .min_size(Vec2::new(0.0, crate::ui_kit::widgets::tokens::Size::Sm.height()))
                            .show(ui, t);
                        if resp.clicked() { new_tab = Some(*tv); }
                    }
                    if can_close {
                        ui.with_layout(
                            egui::Layout::right_to_left(egui::Align::Center),
                            |ui| {
                                let r = crate::ui_kit::widgets::Button::close().placement(IconPlacement::PanelHeader).show(ui, t);
                                Tooltip::new("Close section").show(ui, &r, t);
                                if r.clicked() { remove_idx = Some(i); }
                            },
                        );
                    }
                });
                if let Some(tv) = new_tab { splits[i].tab = tv; }

                // Hairline under the tab strip.
                let row_rect = ui.min_rect();
                ui.painter().line_segment(
                    [Pos2::new(row_rect.left(), row_rect.bottom()),
                     Pos2::new(row_rect.right(), row_rect.bottom())],
                    Stroke::new(stroke_thin(),
                        tint(t, Tone::Border, alpha_faint())),
                );

                // ── Body for this section ─────────────────────────────────
                let frac = splits[i].frac * norm;
                egui::ScrollArea::vertical()
                    .id_salt(format!("split_section_{}_{}", "panel", i))
                    .max_height(h_body)
                    .show(ui, |ui| {
                        body(ui, t, i, frac);
                    });

                // ── Divider between sections ──────────────────────────────
                if i + 1 < n {
                    let d = split_divider(ui, &format!("div_{}", i), t.dim);
                    if d != 0.0 { divider_drags.push((i, d)); }
                }
            }

            // ── Apply mutations gathered during the loop ─────────────────
            if let Some(idx) = remove_idx {
                let removed = splits[idx].frac;
                splits.remove(idx);
                if !splits.is_empty() {
                    let share = removed / splits.len() as f32;
                    for s in splits.iter_mut() { s.frac += share; }
                }
            }
            for (idx, delta) in divider_drags {
                if idx + 1 < splits.len() {
                    let fd = delta / available_h.max(1.0);
                    splits[idx].frac = (splits[idx].frac + fd).clamp(0.05, 0.90);
                    splits[idx + 1].frac = (splits[idx + 1].frac - fd).clamp(0.05, 0.90);
                }
            }

            // Touch unused symbol to silence -Wunused on borderline configs.
            let _ = Sense::hover();
        });
        SidePanelShellResponse { close_clicked }
    }
}
