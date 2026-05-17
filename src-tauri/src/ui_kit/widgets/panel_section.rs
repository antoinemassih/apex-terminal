//! PanelSection — labeled section with optional count, meta, and trailing action.
//!
//! The canonical "section header + body" primitive for side-panel content. Every
//! panel that lists things ("ACTIVE", "PENDING", "CLOSED", "FILTERS", "SETUPS")
//! reaches for this. Replaces the ad-hoc `ui.horizontal { SectionLabel.tiny() ...
//! RTL { small_action_btn ... } }` boilerplate that each panel currently
//! reinvents.
//!
//! ```ignore
//! PanelSection::new("ACTIVE")
//!     .count(3)
//!     .meta("12 total")
//!     .action("Clear", Tone::Danger)
//!     .show(ui, t, |ui, t| {
//!         for row in &active { /* ... */ }
//!     });
//! ```
//!
//! Visual spec (locked by design):
//! - Title: `mono_xs` UPPERCASE strong, in `t.dim` by default; pass
//!   `.title_color(t.accent)` when this section is the "current" one.
//! - Count: numeric badge after title, mono_xs strong, tinted with title color.
//! - Meta: muted right-aligned mono_xs (e.g. "12 total").
//! - Action: trailing ghost button (`panel_action_btn` style), right-aligned
//!   before meta. The click is surfaced via `SectionResponse.action_clicked`.
//! - Bottom rule: hairline at `color_alpha(t.toolbar_border, 36)`, **on by
//!   default** per user spec (matches chart-pane header rule).
//!
//! Sister widgets:
//! - `PanelEmpty` — body content when the section is empty.
//! - `PanelListRow` — body content for repeating list items.
//! - `PanelKeyValueRow` — body content for label/value pairs.
//! - `PanelDivider` — between sections (when rule is off).
//!
//! When NOT to use:
//! - Top-level panel chrome (header bar + close button) — use `Header` /
//!   `kit::PanelHeader`.
//! - Form-field grouping with input gutters — use `FormSection`.

use egui::{Color32, CursorIcon, FontId, Pos2, Rect, RichText, Sense, Stroke, Ui, Vec2};

use crate::chart::renderer::ui::style::{
    color_alpha, font_xs, gap_lg, gap_md, gap_sm, gap_xs, header_border, section_header_surface, shadow_color_alpha, stroke_thin,
};
use crate::chart_renderer::gpu::Theme;

/// Shared semantic tone for the panel body primitives. Resolves to a theme
/// color via [`Tone::color`]. Defined here so the seven panel-body widgets
/// (Section, Empty, Loading, Divider, ListRow, Card, KV) share one vocabulary.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Default)]
pub enum Tone {
    /// Muted / dim — non-emphatic default.
    #[default]
    Default,
    /// Accent — focus / current selection.
    Accent,
    /// Bull / positive / success — long, gain, confirm.
    Bull,
    /// Bear / negative / destructive — short, loss, delete.
    Bear,
    /// Warn — amber-ish caution.
    Warn,
    /// Alias for `Bear` so callers reading the spec literally can write
    /// `Tone::Danger`. Same color, semantic intent: destructive action.
    Danger,
    /// Alias for `Bull` so callers can write `Tone::Success` for non-trading
    /// confirmations.
    Success,
    /// Full-strength text.
    Text,
}

impl Tone {
    pub fn color(self, t: &Theme) -> Color32 {
        match self {
            Tone::Default => t.dim,
            Tone::Accent => t.accent,
            Tone::Bull | Tone::Success => t.bull,
            Tone::Bear | Tone::Danger => t.bear,
            Tone::Warn => t.warn,
            Tone::Text => t.text,
        }
    }
}

/// Alpha (out of 255) of the section hairline rule — locked per user spec.
/// Section rule alpha. Bumped from 36 → 100 so section boundaries
/// actually read against the panel surface. The "calm machine" goal
/// is fewer separators, not invisible ones — this rule fires only
/// when `.rule(true)` so callers still control whether it appears.
const RULE_ALPHA: u8 = 100;

#[must_use = "PanelSection must be rendered with `.show(...)`"]
pub struct PanelSection<'a> {
    title: &'a str,
    title_color: Option<Color32>,
    count: Option<usize>,
    meta: Option<String>,
    action: Option<(String, Tone)>,
    rule: bool,
}

pub struct SectionResponse<R> {
    pub action_clicked: bool,
    pub body: R,
}

impl<'a> PanelSection<'a> {
    pub fn new(title: &'a str) -> Self {
        Self {
            title,
            title_color: None,
            count: None,
            meta: None,
            action: None,
            rule: true,
        }
    }

    pub fn count(mut self, n: usize) -> Self {
        self.count = Some(n);
        self
    }

    pub fn meta(mut self, m: impl Into<String>) -> Self {
        self.meta = Some(m.into());
        self
    }

    /// Override the title color (default: `t.dim`). Use `t.accent` to mark
    /// this as the "current" section. Per spec, this is the ONLY place
    /// accent should appear on a section title.
    pub fn title_color(mut self, c: Color32) -> Self {
        self.title_color = Some(c);
        self
    }

    /// Add a trailing action button. The click is exposed via
    /// [`SectionResponse::action_clicked`] returned from [`Self::show`].
    pub fn action(mut self, label: impl Into<String>, tone: Tone) -> Self {
        self.action = Some((label.into(), tone));
        self
    }

    /// Toggle the bottom hairline rule. Default is `true` per user spec.
    pub fn rule(mut self, on: bool) -> Self {
        self.rule = on;
        self
    }

    pub fn show<R>(
        self,
        ui: &mut Ui,
        t: &Theme,
        body: impl FnOnce(&mut Ui, &Theme) -> R,
    ) -> SectionResponse<R> {
        let title_color = self.title_color.unwrap_or(t.dim);
        let mut action_clicked = false;

        // Header row: DARKER (recessed) L0 background + edge-to-edge
        // top AND bottom rules. The header strip uses `color_layer_down`
        // so it reads as recessed inset chrome — the panel's labeled
        // band. Body content sits below on the regular panel surface.
        //
        // Edge-to-edge: SidePanelShell now drops the LR body inset, so
        // the section rect spans the full panel chrome width. The
        // inner_margin only applies to the title text and trailing
        // actions; the FILL + rules cover the full strip width.
        let prev_pad = ui.spacing().item_spacing;
        let header_resp = egui::Frame::NONE
            .inner_margin(egui::Margin {
                left: gap_lg() as i8,
                right: gap_lg() as i8,
                top: gap_xs() as i8,
                bottom: gap_xs() as i8,
            })
            .fill(section_header_surface(t))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    // Title — uppercase mono_xs strong. One tier smaller
                    // than the SidePanelShell header so the section
                    // reads as nested chrome inside the panel.
                    ui.label(
                        RichText::new(self.title.to_uppercase())
                            .monospace()
                            .size(font_xs())
                            .strong()
                            .color(title_color),
                    );
                    if let Some(n) = self.count {
                        ui.add_space(gap_xs());
                        ui.label(
                            RichText::new(format!("{}", n))
                                .monospace()
                                .size(font_xs())
                                .strong()
                                .color(color_alpha(title_color, 200)),
                        );
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if let Some((label, tone)) = &self.action {
                            if section_action_button(ui, label, tone.color(t)) {
                                action_clicked = true;
                            }
                            if self.meta.is_some() {
                                ui.add_space(gap_sm());
                            }
                        }
                        if let Some(m) = &self.meta {
                            ui.label(
                                RichText::new(m)
                                    .monospace()
                                    .size(font_xs())
                                    .color(color_alpha(t.dim, 160)),
                            );
                        }
                    });
                });
            });
        ui.spacing_mut().item_spacing = prev_pad;
        // Edge-to-edge top + bottom rules bracketing the recessed strip.
        // Border color matches the chart pane header — t.text @ 38 alpha.
        let hr = header_resp.response.rect;
        let rule_col = header_border(t);
        ui.painter().line_segment(
            [Pos2::new(hr.left(), hr.top() + 0.5), Pos2::new(hr.right(), hr.top() + 0.5)],
            Stroke::new(stroke_thin(), rule_col),
        );
        if self.rule {
            ui.painter().line_segment(
                [Pos2::new(hr.left(), hr.bottom() - 0.5), Pos2::new(hr.right(), hr.bottom() - 0.5)],
                Stroke::new(stroke_thin(), rule_col),
            );
        }
        // Inset drop-shadow falling down from the bottom rule — 6px
        // tall gradient painted into a stable layer above the body,
        // fading from alpha 38 at the top to 0 at the bottom.
        // Painted on the FOREGROUND layer (not ui.painter) so it
        // doesn't shift widget layer-ids mid-frame.
        let shadow_h = 6.0_f32;
        {
            let layer = egui::LayerId::new(
                egui::Order::Foreground,
                ui.id().with(("panel_section_shadow", hr.left().to_bits(), hr.top().to_bits())),
            );
            let painter = ui.ctx().layer_painter(layer);
            for i in 0..shadow_h as i32 {
                let frac = 1.0 - (i as f32 / shadow_h);
                let a = (38.0 * frac) as u8;
                if a == 0 { continue; }
                let y = hr.bottom() + i as f32 + 0.5;
                painter.line_segment(
                    [Pos2::new(hr.left(), y), Pos2::new(hr.right(), y)],
                    Stroke::new(stroke_thin(), shadow_color_alpha(t, a)),
                );
            }
        }
        ui.add_space(shadow_h + 2.0);

        // Body — natural flow on the panel surface. Inset by gap_lg so
        // body content's left edge aligns with the header's title text
        // (header uses gap_lg L/R via the Frame inner_margin above).
        let r = egui::Frame::NONE
            .inner_margin(egui::Margin {
                left: gap_lg() as i8,
                right: gap_lg() as i8,
                top: 0,
                bottom: 0,
            })
            .show(ui, |ui| body(ui, t))
            .inner;
        ui.add_space(gap_sm());

        SectionResponse {
            action_clicked,
            body: r,
        }
    }
}

fn paint_rule(ui: &mut Ui, t: &Theme) {
    let rect = ui.available_rect_before_wrap();
    let y = ui.cursor().min.y;
    ui.painter().line_segment(
        [
            Pos2::new(rect.left(), y),
            Pos2::new(rect.right(), y),
        ],
        Stroke::new(stroke_thin(), header_border(t)),
    );
    ui.add_space(1.0);
}

/// Local section-action button — small ghost button matching `kit::panel_action_btn`
/// for visual parity. Reproduced here so this widget has no dependency on the
/// legacy `kit` module.
fn section_action_button(ui: &mut Ui, label: &str, color: Color32) -> bool {
    let text = RichText::new(label)
        .monospace()
        .size(font_xs())
        .strong()
        .color(color);
    let font = FontId::monospace(font_xs());
    let galley = ui.fonts(|f| f.layout_no_wrap(label.to_string(), font, color));
    let pad_x = gap_xs() + 2.0;
    let size = Vec2::new(galley.size().x + pad_x * 2.0, 16.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    if resp.hovered() {
        ui.painter()
            .rect_filled(rect, 2.0, color_alpha(color, 24));
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        FontId::monospace(font_xs()),
        color,
    );
    let _ = text;
    resp.clicked()
}

// ─── PanelSectionGroup ──────────────────────────────────────────────────────
//
// Stacks N PanelSections vertically with user-draggable grippy dividers
// between adjacent sections. The caller owns the persistent fraction state
// as a `&mut [f32; N]` slice; the group normalizes the sum to 1.0 and
// enforces a configurable minimum section height.
//
// Why this exists: PanelSection is natural-flow — it takes whatever
// vertical space it needs. Several panels (indicators, alerts) want
// "TOOLS / ACTIVE / LIBRARY"-style 3-way splits where the user can drag
// the divider between sections to give one more room than another. Each
// pane has a header (the PanelSection title) and a scrollable body. The
// dividers BETWEEN sections are the affordance — hence a group widget
// rather than a `.resizable()` builder on a single section.
//
// Divider visual (matches PanelDivider hairline aesthetic):
//   - 5px tall hit/paint band, hairline rule centered
//   - Center dot triplet (3 dots, 2px diameter, gap 3px) as grab affordance
//   - Idle  : `color_alpha(t.toolbar_border, 36)`
//   - Hover : `color_alpha(t.toolbar_border, 96)` (and dots brightened)
//   - Cursor: `CursorIcon::ResizeVertical` on hover/drag

/// Divider hit-band height (px). Drawn between adjacent sections.
const DIVIDER_BAND_H: f32 = 6.0;
/// Idle alpha of the divider hairline + dots (matches PanelSection rule).
const DIVIDER_IDLE_ALPHA: u8 = 36;
/// Hover / drag alpha of the divider hairline + dots.
const DIVIDER_HOVER_ALPHA: u8 = 96;
/// Default minimum section height when the caller doesn't override it.
const DEFAULT_MIN_SECTION_H: f32 = 32.0;

/// Vertical stack of N resizable [`PanelSection`]s with grippy dividers
/// between them.
///
/// State (`&mut [f32; N]`) lives on the caller (e.g.
/// `Watchlist::indicators_section_fracs`). Fractions sum to 1.0 — the
/// group renormalizes if the caller's storage drifts. A minimum section
/// height is enforced when dragging.
///
/// ```ignore
/// PanelSectionGroup::new(&mut watchlist.indicators_section_fracs)
///     .min_section_height(40.0)
///     .show(ui, t, |grp| {
///         grp.section(|ui, t| {
///             PanelSection::new("TOOLS").show(ui, t, |ui, t| { /* ... */ });
///         });
///         grp.section(|ui, t| {
///             PanelSection::new("ACTIVE").count(n).show(ui, t, |ui, t| { /* ... */ });
///         });
///         grp.section(|ui, t| {
///             PanelSection::new("LIBRARY").show(ui, t, |ui, t| { /* ... */ });
///         });
///     });
/// ```
///
/// The caller must invoke `grp.section(...)` exactly `N` times (one per
/// fraction in the storage slice). Extra calls are silently dropped;
/// missing calls leave those rects unrendered.
#[must_use = "PanelSectionGroup must be rendered with `.show(...)`"]
pub struct PanelSectionGroup<'a> {
    fracs: &'a mut [f32],
    min_section_height: f32,
}

impl<'a> PanelSectionGroup<'a> {
    pub fn new(fracs: &'a mut [f32]) -> Self {
        Self {
            fracs,
            min_section_height: DEFAULT_MIN_SECTION_H,
        }
    }

    /// Minimum height (px) any one section is allowed to shrink to while
    /// the user drags a divider. Defaults to 32px.
    pub fn min_section_height(mut self, px: f32) -> Self {
        self.min_section_height = px.max(8.0);
        self
    }

    pub fn show<F>(self, ui: &mut Ui, t: &Theme, mut body: F)
    where
        F: FnMut(&mut PanelSectionGroupBuilder<'_>),
    {
        let n = self.fracs.len();
        if n == 0 {
            return;
        }

        // Available rect — full remaining width × height inside the parent
        // UI. We allocate it up front so divider drags stay anchored to
        // the same band regardless of scrollbar reflow inside a section.
        let avail = ui.available_size_before_wrap();
        let total_w = if avail.x.is_finite() && avail.x > 0.0 { avail.x } else { 200.0 };
        let total_h = if avail.y.is_finite() && avail.y > 0.0 { avail.y } else { 200.0 };
        if total_h <= 1.0 {
            return;
        }

        let (rect, _resp) = ui.allocate_exact_size(
            Vec2::new(total_w, total_h),
            Sense::hover(),
        );

        // Normalize fractions (defensive — caller might init to zeros).
        let sum: f32 = self.fracs.iter().copied().filter(|f| f.is_finite()).sum();
        if sum <= 0.0001 {
            let even = 1.0 / n as f32;
            for f in self.fracs.iter_mut() {
                *f = even;
            }
        } else if (sum - 1.0).abs() > 0.001 {
            for f in self.fracs.iter_mut() {
                *f /= sum;
            }
        }

        // ── Drag handling ──────────────────────────────────────────────
        //
        // For each of the N-1 dividers we allocate a thin hit rect and,
        // if dragged, transfer height between the two adjacent sections.
        // Compute initial pixel heights from fractions.
        let divider_total = DIVIDER_BAND_H * (n - 1) as f32;
        let body_total = (total_h - divider_total).max(0.0);
        let min_h = self.min_section_height;

        let mut heights: Vec<f32> = self.fracs.iter().map(|f| f * body_total).collect();

        // Process each divider — must do it in a separate pass so we can
        // mutate heights[i] and heights[i+1] together.
        let base_id = ui.id().with("ui_kit_panel_section_group");
        let mut hovered_divider: Option<usize> = None;
        for i in 0..n.saturating_sub(1) {
            // Compute divider band y-position from current heights.
            let mut y = rect.top();
            for j in 0..=i {
                y += heights[j];
                if j < i {
                    y += DIVIDER_BAND_H;
                }
            }
            let div_rect = Rect::from_min_size(
                Pos2::new(rect.left(), y),
                Vec2::new(rect.width(), DIVIDER_BAND_H),
            );
            // Expand hit rect a couple px for ease of grabbing.
            let hit_rect = div_rect.expand2(Vec2::new(0.0, 2.0));
            let resp = ui.interact(
                hit_rect,
                base_id.with(("div", i)),
                Sense::click_and_drag(),
            );
            if resp.hovered() || resp.dragged() {
                hovered_divider = Some(i);
                ui.ctx().set_cursor_icon(CursorIcon::ResizeVertical);
            }
            if resp.dragged() {
                let dy = resp.drag_delta().y;
                if dy.abs() > 0.0 {
                    let a = heights[i];
                    let b = heights[i + 1];
                    // Clamp transfer by min-height on the shrinking side.
                    let new_a = (a + dy).max(min_h).min(a + b - min_h);
                    let delta = new_a - a;
                    heights[i] += delta;
                    heights[i + 1] -= delta;
                }
            }
        }

        // Write heights back as fractions of body_total.
        if body_total > 0.0 {
            for (h, f) in heights.iter().zip(self.fracs.iter_mut()) {
                *f = h / body_total;
            }
            // Re-normalize for floating point drift.
            let s: f32 = self.fracs.iter().sum();
            if s > 0.0 {
                for f in self.fracs.iter_mut() {
                    *f /= s;
                }
            }
        }

        // ── Paint sections + dividers ──────────────────────────────────
        let mut builder = PanelSectionGroupBuilder {
            ui,
            t,
            outer_rect: rect,
            heights: &heights,
            divider_h: DIVIDER_BAND_H,
            index: 0,
            cursor_y: rect.top(),
            hovered_divider,
        };
        body(&mut builder);

        // Paint dividers AFTER section bodies so the grippy sits on top
        // of any section bottom-rule.
        for i in 0..n.saturating_sub(1) {
            let mut y = rect.top();
            for j in 0..=i {
                y += heights[j];
                if j < i {
                    y += DIVIDER_BAND_H;
                }
            }
            let div_rect = Rect::from_min_size(
                Pos2::new(rect.left(), y),
                Vec2::new(rect.width(), DIVIDER_BAND_H),
            );
            paint_grippy_divider(builder.ui, t, div_rect, hovered_divider == Some(i));
        }
    }
}

/// Builder handed to the closure passed to [`PanelSectionGroup::show`].
/// Each call to [`Self::section`] consumes one fraction slot.
pub struct PanelSectionGroupBuilder<'u> {
    ui: &'u mut Ui,
    t: &'u Theme,
    outer_rect: Rect,
    heights: &'u [f32],
    divider_h: f32,
    index: usize,
    cursor_y: f32,
    hovered_divider: Option<usize>,
}

impl<'u> PanelSectionGroupBuilder<'u> {
    /// Render one section into the next slot. The closure runs inside a
    /// child UI clipped to the section's allocated rect — typical use is
    /// to call `PanelSection::new(...).show(ui, t, |ui, t| { ... })`
    /// inside it.
    pub fn section<F>(&mut self, add_contents: F)
    where
        F: FnOnce(&mut Ui, &Theme),
    {
        let i = self.index;
        if i >= self.heights.len() {
            return;
        }
        let h = self.heights[i].max(0.0);
        let section_rect = Rect::from_min_size(
            Pos2::new(self.outer_rect.left(), self.cursor_y),
            Vec2::new(self.outer_rect.width(), h),
        );

        if section_rect.height() > 0.5 {
            let mut child = self.ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(section_rect)
                    .layout(egui::Layout::top_down(egui::Align::Min)),
            );
            child.set_clip_rect(section_rect);
            add_contents(&mut child, self.t);
        }

        self.cursor_y += h;
        if i + 1 < self.heights.len() {
            self.cursor_y += self.divider_h;
        }
        self.index += 1;
        let _ = self.hovered_divider; // silence unused on this path
    }
}

/// Paint a hairline + 3-dot grippy in the middle of `rect`. The divider
/// runs left→right; dots are centered horizontally.
fn paint_grippy_divider(ui: &mut Ui, t: &Theme, rect: Rect, hovered: bool) {
    let alpha = if hovered { DIVIDER_HOVER_ALPHA } else { DIVIDER_IDLE_ALPHA };
    let line_color = color_alpha(t.toolbar_border, alpha);
    let cy = rect.center().y;
    let painter = ui.painter();
    // Hairline rule across full width.
    painter.line_segment(
        [Pos2::new(rect.left(), cy), Pos2::new(rect.right(), cy)],
        Stroke::new(stroke_thin(), line_color),
    );
    // Center dot triplet — three filled circles, ~1.4px radius, gap 3px,
    // tinted with the same border color but a touch brighter.
    let dot_color = color_alpha(t.toolbar_border, alpha.saturating_add(48));
    let dot_r = 1.4;
    let gap = 3.0;
    let cx = rect.center().x;
    let xs = [cx - (dot_r * 2.0 + gap), cx, cx + (dot_r * 2.0 + gap)];
    for x in xs {
        painter.circle_filled(Pos2::new(x, cy), dot_r, dot_color);
    }
}
