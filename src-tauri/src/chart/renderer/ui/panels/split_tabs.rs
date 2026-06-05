//! `SplitTabs` — a reusable "one surface, N panes, each showing one tab".
//!
//! A surface (the footer today; a side panel later) divides into N panes along
//! an axis. Each pane has its own tab strip and independently selects which tab
//! it shows. A `+` control (beside the close `×`) splits — adds another pane;
//! `×` removes that pane. Closing the *last* pane reports `closed_last` so the
//! caller can decide what that means (the footer collapses to its strip).
//!
//! Panes divide the surface equally along `axis`; the caller renders each pane's
//! body via a closure `(ui, pane_index, tab)`. Each pane is its own egui `Ui`
//! (own id + clip rect), so widget ids and scroll state never collide between
//! panes — the same property the rail relies on.
//!
//! ## Reuse
//! * Footer → [`SplitAxis::Horizontal`] (columns side by side).
//! * Side panels → [`SplitAxis::Vertical`] (tab views stacked top-to-bottom).
//!
//! The widget is generic over the tab type `T` (any `Copy + PartialEq` enum) and
//! owns no state — the caller passes `&mut Vec<T>` (one selected tab per pane),
//! which the widget grows/shrinks as panes are added/removed.

use egui::Ui;
use crate::ui_kit::sx::Tone;
use super::kit::PanelHeaderTabs;
use crate::ui_kit::widgets::{OutlinedBox, MenuItem};
use crate::ui_kit::widgets::theme::ComponentTheme;
use super::super::style::*;

type Theme = crate::chart_renderer::gpu::Theme;

/// Axis a [`SplitTabs`] surface divides along.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum SplitAxis {
    /// Panes side by side (footer columns).
    Horizontal,
    /// Panes stacked top-to-bottom (side-panel tab views).
    Vertical,
}

pub(crate) struct SplitTabsResponse {
    /// The user closed the only remaining pane — caller decides the meaning
    /// (e.g. collapse the surface).
    pub closed_last: bool,
}

#[must_use = "SplitTabs must be rendered with `.show(...)`"]
pub(crate) struct SplitTabs<'a, T: Copy + PartialEq> {
    id_salt: &'a str,
    axis: SplitAxis,
    tabs: &'a [(T, &'a str)],
    slots: &'a mut Vec<T>,
    strip_height: f32,
    max_panes: usize,
}

impl<'a, T: Copy + PartialEq> SplitTabs<'a, T> {
    pub fn new(id_salt: &'a str, axis: SplitAxis, tabs: &'a [(T, &'a str)], slots: &'a mut Vec<T>) -> Self {
        // Binary tiling: a region is one full instance or two equal halves.
        Self { id_salt, axis, tabs, slots, strip_height: 30.0, max_panes: 2 }
    }
    pub fn strip_height(mut self, h: f32) -> Self { self.strip_height = h; self }
    /// Cap on simultaneous panes. Clamped to 2 (binary, no thirds).
    pub fn max_panes(mut self, n: usize) -> Self { self.max_panes = n.clamp(1, 2); self }

    pub fn show(self, ui: &mut Ui, t: &Theme, mut body: impl FnMut(&mut Ui, usize, T)) -> SplitTabsResponse {
        let SplitTabs { id_salt, axis, tabs, slots, strip_height, max_panes } = self;
        if tabs.is_empty() { return SplitTabsResponse { closed_last: false }; }
        if slots.is_empty() { slots.push(tabs[0].0); }
        slots.truncate(max_panes); // enforce the binary cap defensively

        let n = slots.len();
        let rect = ui.available_rect_before_wrap();
        let layer = ui.layer_id();
        let ctx = ui.ctx().clone();
        let gap = 1.0;

        // Deferred edits (can't mutate `slots` mid-iteration). At most one fires.
        let mut remove_idx: Option<usize> = None;
        let mut split_now = false;       // split-toggle clicked → add a 2nd half
        let mut spawn: Option<T> = None;  // right-clicked tab → "open as new instance"
        let mut closed_last = false;
        let can_grow = n < max_panes;

        for i in 0..n {
            let sub = pane_rect(rect, axis, i, n, gap);
            if sub.width() < 2.0 || sub.height() < 2.0 { continue; }

            let mut pane = Ui::new(
                ctx.clone(),
                egui::Id::new((id_salt, "pane", i)),
                egui::UiBuilder::new().max_rect(sub).layer_id(layer),
            );
            pane.set_clip_rect(sub);

            // ── Header: tabs + split-toggle + close. Right-click a tab to spawn. ──
            let salt = format!("{}_pane_{}", id_salt, i);
            let header = OutlinedBox::new()
                .fill(t.header_surface()).borderless().square().padding(0.0)
                .show(&mut pane, t, |ui| {
                    let closed = PanelHeaderTabs::new(&mut slots[i], tabs)
                        .id_salt(&salt)
                        .height(strip_height)
                        .closable(true)
                        .on_tab_secondary(|ui, tab| {
                            // Right-click a tab → open it as a second instance.
                            if can_grow {
                                if MenuItem::new("Open as new instance").show(ui, t).clicked() {
                                    spawn = Some(tab);
                                    ui.close_menu();
                                }
                            } else {
                                MenuItem::new("Split is full (max 2)").show(ui, t);
                            }
                        })
                        .show_with(ui, t, |ui| {
                            // Split toggle — only when there's room to split.
                            if can_grow {
                                if split_icon_button(ui, t, axis) { split_now = true; }
                                ui.add_space(gap_xs());
                            }
                        });
                    if closed {
                        if n > 1 { remove_idx = Some(i); } else { closed_last = true; }
                    }
                });

            // Hairline underline (header family).
            let hr = header.response.rect;
            pane.painter().line_segment(
                [egui::pos2(hr.left(), hr.bottom() - 0.5), egui::pos2(hr.right(), hr.bottom() - 0.5)],
                egui::Stroke::new(1.0, t.header_border()),
            );
            pane.add_space(gap_xs());

            // ── Body (caller-rendered) ───────────────────────────────────────
            let tab = slots[i];
            body(&mut pane, i, tab);

            // Divider between the two panes.
            if i + 1 < n {
                let col = tint(t, Tone::Border, 150);
                match axis {
                    SplitAxis::Horizontal => {
                        let x = sub.right() + gap * 0.5;
                        ui.painter().line_segment(
                            [egui::pos2(x, sub.top()), egui::pos2(x, sub.bottom())],
                            egui::Stroke::new(1.0, col),
                        );
                    }
                    SplitAxis::Vertical => {
                        let y = sub.bottom() + gap * 0.5;
                        ui.painter().line_segment(
                            [egui::pos2(sub.left(), y), egui::pos2(sub.right(), y)],
                            egui::Stroke::new(1.0, col),
                        );
                    }
                }
            }
        }

        // Apply the one deferred edit (close wins; then explicit spawn; then split).
        if let Some(i) = remove_idx {
            if slots.len() > 1 && i < slots.len() { slots.remove(i); }
        } else if let Some(tab) = spawn {
            if slots.len() < max_panes { slots.push(tab); }
        } else if split_now && slots.len() < max_panes {
            let nt = next_tab(tabs, slots);
            slots.push(nt);
        }

        SplitTabsResponse { closed_last }
    }
}

/// A small split-toggle button that depicts splitting along `axis` (two cells
/// divided by a line). Returns `true` on click.
fn split_icon_button(ui: &mut Ui, t: &Theme, axis: SplitAxis) -> bool {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(18.0, 14.0), egui::Sense::click());
    if resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
    let col = if resp.hovered() { t.accent } else { t.dim };
    let stroke = egui::Stroke::new(1.2, col);
    let r = rect.shrink2(egui::vec2(3.0, 2.0));
    ui.painter().rect_stroke(r, egui::CornerRadius::same(2), stroke, egui::StrokeKind::Inside);
    match axis {
        SplitAxis::Horizontal => {
            let x = r.center().x;
            ui.painter().line_segment([egui::pos2(x, r.top()), egui::pos2(x, r.bottom())], stroke);
        }
        SplitAxis::Vertical => {
            let y = r.center().y;
            ui.painter().line_segment([egui::pos2(r.left(), y), egui::pos2(r.right(), y)], stroke);
        }
    }
    resp.clicked()
}

/// The i-th of `n` equal panes within `rect`, separated by `gap`.
fn pane_rect(rect: egui::Rect, axis: SplitAxis, i: usize, n: usize, gap: f32) -> egui::Rect {
    let n = n.max(1) as f32;
    let i = i as f32;
    match axis {
        SplitAxis::Horizontal => {
            let w = ((rect.width() - gap * (n - 1.0)) / n).max(0.0);
            let x = rect.left() + i * (w + gap);
            egui::Rect::from_min_size(egui::pos2(x, rect.top()), egui::vec2(w, rect.height()))
        }
        SplitAxis::Vertical => {
            let h = ((rect.height() - gap * (n - 1.0)) / n).max(0.0);
            let y = rect.top() + i * (h + gap);
            egui::Rect::from_min_size(egui::pos2(rect.left(), y), egui::vec2(rect.width(), h))
        }
    }
}

/// First tab not already shown in a pane, else the first tab.
fn next_tab<T: Copy + PartialEq>(tabs: &[(T, &str)], slots: &[T]) -> T {
    tabs.iter().map(|(v, _)| *v).find(|v| !slots.contains(v)).unwrap_or(tabs[0].0)
}
