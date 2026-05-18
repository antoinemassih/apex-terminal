//! PanelToolbar — standardised panel-internal header strip.
//!
//! Renders a consistent `height_strip_px`-tall bar with three slots:
//! - **Left** — a plain text label (`left_label`) in `mono_xs` / `t.dim`.
//! - **Center** — an optional free-form closure (filter chips, segmented
//!   control, etc.).
//! - **Right** — an optional free-form closure painted **right-to-left**
//!   (icon buttons for filter / refresh / add). Actions are painted RTL so
//!   callers can add items in logical order (most-important leftmost in the
//!   code, rightmost on screen) without manual width math.
//!
//! ```ignore
//! PanelToolbar::new()
//!     .left_label("POSITIONS")
//!     .right_actions(|ui, t| {
//!         if Button::icon(Icon::ARROW_COUNTER_CLOCKWISE)
//!             .variant(Variant::Ghost).size(Size::Sm).show(ui, t).clicked() {
//!             state.refresh();
//!         }
//!         if Button::icon(Icon::FUNNEL)
//!             .variant(Variant::Ghost).size(Size::Sm).show(ui, t).clicked() {
//!             state.toggle_filter();
//!         }
//!     })
//!     .show(ui, t);
//! ```
//!
//! Visual spec:
//! - Height: 22 px (same as a dense `PanelListRow`).
//! - Background: `color_alpha(t.toolbar_border, alpha_ghost())` — the same
//!   recessed tint used by `PanelSubSection` and `TableHeader`.
//! - Bottom hairline: `stroke_thin()` at `color_alpha(t.toolbar_border, 60)`.
//! - Left label: `mono_xs`, `t.dim` color, left-padded `gap_md()`.
//! - Center slot: centered in the remaining space after left label.
//! - Right slot: `Layout::right_to_left(Center)`, right-padded `gap_xs()`.
//! - No interactive sense on the strip itself — clicks land on child widgets.
//!
//! When to use:
//! - Panel-internal sub-toolbars (filter row, view controls).
//! - Above a `PanelListRow` list that needs filter/refresh/add buttons.
//! - Anywhere you'd otherwise hand-roll `ui.horizontal(|ui| { label; ui.with_layout(RTL ...) })`.
//!
//! When NOT to use:
//! - Top-of-panel headers — use `Header::panel(title)` (from `ui_kit::Header`).
//! - Section headers — use `PanelSection::new(title)`.
//! - Global toolbars at the app chrome level.
//!
//! Sister widgets: [`Header`], [`PanelSection`], [`TableHeader`], [`PanelSubSection`].

use egui::{Align, FontId, Layout, Pos2, Sense, Ui, Vec2};

use crate::chart::renderer::ui::style::{
    alpha_ghost, color_alpha, font_xs, gap_md, gap_xs, stroke_thin,
};
use crate::chart_renderer::gpu::Theme;

/// Height of the toolbar strip in pixels.
const TOOLBAR_H: f32 = 22.0;

#[must_use = "PanelToolbar must be rendered with `.show(...)`"]
pub struct PanelToolbar<'a> {
    left_label: Option<&'a str>,
    center: Option<Box<dyn FnOnce(&mut Ui, &Theme) + 'a>>,
    right_actions: Option<Box<dyn FnOnce(&mut Ui, &Theme) + 'a>>,
    height: f32,
}

impl<'a> PanelToolbar<'a> {
    pub fn new() -> Self {
        Self {
            left_label: None,
            center: None,
            right_actions: None,
            height: TOOLBAR_H,
        }
    }

    /// Text label on the left. Uses `mono_xs` in `t.dim`. Optional — if not
    /// set, the left slot is empty.
    pub fn left_label(mut self, s: &'a str) -> Self {
        self.left_label = Some(s);
        self
    }

    /// Center slot — arbitrary content (filter chips, segmented control).
    /// Receives `(&mut Ui, &Theme)`. The center slot is omitted when not set.
    pub fn center(mut self, f: impl FnOnce(&mut Ui, &Theme) + 'a) -> Self {
        self.center = Some(Box::new(f));
        self
    }

    /// Right slot — icon buttons or other compact actions. Painted
    /// **right-to-left** so the first closure call paints the rightmost item.
    /// Receives `(&mut Ui, &Theme)`.
    pub fn right_actions(mut self, f: impl FnOnce(&mut Ui, &Theme) + 'a) -> Self {
        self.right_actions = Some(Box::new(f));
        self
    }

    /// Override the toolbar height. Default is 22 px. Most callers should
    /// leave this at the default.
    pub fn height(mut self, px: f32) -> Self {
        self.height = px;
        self
    }

    pub fn show(self, ui: &mut Ui, t: &Theme) {
        let avail_w = ui.available_width();
        let (rect, _) = ui.allocate_exact_size(
            Vec2::new(avail_w, self.height),
            Sense::hover(),
        );

        if !ui.is_rect_visible(rect) {
            return;
        }

        let painter = ui.painter_at(rect);

        // Strip background.
        painter.rect_filled(rect, 0.0, color_alpha(t.toolbar_border, alpha_ghost()));

        // Bottom hairline.
        let y = rect.bottom() - 0.5;
        painter.line_segment(
            [Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
            egui::Stroke::new(stroke_thin(), color_alpha(t.toolbar_border, 60)),
        );

        // Use a new child UI covering the full rect for the layout.
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(rect)
                .layout(Layout::left_to_right(Align::Center)),
        );
        child.spacing_mut().item_spacing.x = 0.0;

        // ── Left label ──────────────────────────────────────────────────────
        child.add_space(gap_md());
        if let Some(label) = self.left_label {
            child.label(
                egui::RichText::new(label)
                    .monospace()
                    .size(font_xs())
                    .color(t.dim),
            );
        }

        // ── Right actions (RTL) ─────────────────────────────────────────────
        // We paint the right slot inside a nested RTL layout. This must
        // happen BEFORE the center slot so egui allocates from the right edge
        // first; the center gets whatever horizontal space is left.
        if let Some(right) = self.right_actions {
            child.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add_space(gap_xs());
                right(ui, t);
            });
        }

        // ── Center slot ─────────────────────────────────────────────────────
        if let Some(center) = self.center {
            child.with_layout(
                Layout::left_to_right(Align::Center).with_main_align(Align::Center),
                |ui| {
                    center(ui, t);
                },
            );
        }
    }
}

impl<'a> Default for PanelToolbar<'a> {
    fn default() -> Self { Self::new() }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Builder stores left_label.
    #[test]
    fn left_label_stored() {
        let tb = PanelToolbar::new().left_label("POSITIONS");
        assert_eq!(tb.left_label, Some("POSITIONS"));
    }

    /// Builder: no label by default.
    #[test]
    fn no_label_by_default() {
        let tb = PanelToolbar::new();
        assert!(tb.left_label.is_none());
    }

    /// Height override is stored.
    #[test]
    fn height_override() {
        let tb = PanelToolbar::new().height(28.0);
        assert!((tb.height - 28.0).abs() < f32::EPSILON);
    }

    /// Default height is TOOLBAR_H (22).
    #[test]
    fn default_height() {
        let tb = PanelToolbar::new();
        assert!((tb.height - TOOLBAR_H).abs() < f32::EPSILON);
    }

    /// Right_actions closure is stored.
    #[test]
    fn right_actions_stored() {
        let tb = PanelToolbar::new().right_actions(|_ui, _t| {});
        assert!(tb.right_actions.is_some());
    }

    /// Center closure is stored.
    #[test]
    fn center_stored() {
        let tb = PanelToolbar::new().center(|_ui, _t| {});
        assert!(tb.center.is_some());
    }
}
