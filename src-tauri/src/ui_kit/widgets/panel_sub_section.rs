//! PanelSubSection — collapsible category grouping nested inside a panel.
//!
//! One level down from `PanelSection`: same visual vocabulary (uppercase
//! `mono_xs` strong title in `t.dim`, optional count chip), but with a
//! click-to-toggle caret on the left and a `&mut bool` for persistent
//! expanded state. A hairline rule is painted below the header **always**
//! (whether expanded or collapsed) so the category boundary is visible
//! even when the body is hidden.
//!
//! Replaces the hand-rolled "caret + UPPERCASE label + count chip +
//! divider" pattern in `chart/renderer/ui/panels/indicators_panel.rs`
//! (the LIBRARY category headers) and the equivalent shape in
//! `object_tree.rs` folder rows.
//!
//! ```ignore
//! PanelSubSection::new("trend_indicators", "TREND")
//!     .count(8)
//!     .expanded(&mut group.expanded)
//!     .show(ui, t, |ui, t| {
//!         // body — only invoked when expanded
//!     });
//! ```
//!
//! Visual spec:
//! - Header row: `gap_lg()` (22px-ish) tall, full available width, clickable.
//! - Caret: `Icon::CARET_RIGHT` (collapsed) / `Icon::CARET_DOWN` (expanded),
//!   proportional 12px, painted in `t.dim`.
//! - Title: `font_xs()` monospace, strong, uppercase, `t.dim`.
//! - Count chip: monospace `font_xs()` strong in `color_alpha(t.dim, 200)`,
//!   same treatment as `PanelSection::count`.
//! - Click anywhere on the header row toggles `*expanded`.
//! - Hover: very subtle `color_alpha(t.text, 8)` background, `radius_sm()`.
//! - Bottom rule: `stroke_thin()` at `color_alpha(t.toolbar_border, 36)` —
//!   matches `panel_divider` / `PanelSection`. Painted in both states.
//! - Body: indented from the left by `gap_md()` when expanded; nothing
//!   rendered when collapsed (early return).
//!
//! Sister widgets:
//! - `PanelSection` — top-level section header inside a panel body.
//! - `Disclosure` — generic collapsible (proportional font, `t.text` title);
//!   use when the row is *not* a panel category header.
//! - `panel_divider` — when you just need the hairline without a header.
//!
//! When NOT to use:
//! - As the outermost section in a panel body — use `PanelSection`.
//! - For freeform expand/collapse with a normal-case title — use
//!   `Disclosure`.

use egui::{CornerRadius, FontId, Pos2, Sense, Stroke, Ui, Vec2};

use super::super::icons::Icon;
use crate::chart::renderer::ui::style::{
    color_alpha, font_xs, gap_2xs, gap_md, gap_xs, radius_sm, stroke_thin,
};
use crate::chart_renderer::gpu::Theme;

/// Alpha (out of 255) of the bottom hairline rule. Matches `PanelSection`
/// and `panel_divider` so nested grouping stays visually consistent.
const RULE_ALPHA: u8 = 36;

/// Hover background alpha (out of 255), applied to `t.text`. Matches
/// `PanelListRow::HOVER_BG_ALPHA` so categories feel like the rows they
/// contain.
const HOVER_BG_ALPHA: u8 = 8;

/// Caret glyph point size — proportional, matches Disclosure default.
const CARET_FONT: f32 = 12.0;

/// Header row height. `gap_lg()` (16px) is too tight for a clickable
/// header with a count chip; 22 matches PanelListRow's dense row height.
const HEADER_H: f32 = 22.0;

#[must_use = "PanelSubSection must be rendered with `.show(...)`"]
pub struct PanelSubSection<'a> {
    id_salt: &'a str,
    title: &'a str,
    count: Option<usize>,
    expanded: Option<&'a mut bool>,
}

impl<'a> PanelSubSection<'a> {
    pub fn new(id_salt: &'a str, title: &'a str) -> Self {
        Self {
            id_salt,
            title,
            count: None,
            expanded: None,
        }
    }

    /// Add a count chip after the title.
    pub fn count(mut self, n: usize) -> Self {
        self.count = Some(n);
        self
    }

    /// Bind the expanded/collapsed state. **Required** for the header to
    /// toggle on click. If omitted, the body is always rendered (and the
    /// caret is shown in the down position).
    pub fn expanded(mut self, state: &'a mut bool) -> Self {
        self.expanded = Some(state);
        self
    }

    pub fn show<R>(
        self,
        ui: &mut Ui,
        t: &Theme,
        body: impl FnOnce(&mut Ui, &Theme) -> R,
    ) -> Option<R> {
        let Self { id_salt, title, count, expanded } = self;

        // Resolve current open state. If no state was bound, treat as
        // always-open so the widget still renders something useful.
        let is_open = expanded.as_ref().map(|b| **b).unwrap_or(true);

        let avail_w = ui.available_width();
        let (rect, resp) = ui.allocate_exact_size(
            Vec2::new(avail_w, HEADER_H),
            Sense::click(),
        );
        let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);

        // Toggle on click if state was bound.
        let mut is_open = is_open;
        if let Some(state) = expanded {
            if resp.clicked() {
                *state = !*state;
            }
            is_open = *state;
        }

        if ui.is_rect_visible(rect) {
            let painter = ui.painter_at(rect);

            // Hover background — very subtle, matches PanelListRow hover.
            if resp.hovered() {
                painter.rect_filled(
                    rect,
                    CornerRadius::same(radius_sm() as u8),
                    color_alpha(t.text, HOVER_BG_ALPHA),
                );
            }

            // Caret — proportional, in t.dim, vertically centered.
            let caret_glyph = if is_open { Icon::CARET_DOWN } else { Icon::CARET_RIGHT };
            let caret_font = FontId::proportional(CARET_FONT);
            let caret_galley = ui.fonts(|f| {
                f.layout_no_wrap(caret_glyph.to_string(), caret_font, t.dim)
            });
            let cy = rect.center().y;
            let mut x = rect.left() + gap_xs();
            painter.galley(
                Pos2::new(x, cy - caret_galley.rect.height() * 0.5),
                caret_galley.clone(),
                t.dim,
            );
            x += caret_galley.rect.width() + gap_2xs();

            // Title — uppercase mono_xs strong in t.dim. (We paint via the
            // painter for precise vertical centering alongside the caret.)
            let title_text = title.to_uppercase();
            let title_font = FontId::monospace(font_xs());
            let title_galley = ui.fonts(|f| {
                f.layout_no_wrap(title_text, title_font, t.dim)
            });
            painter.galley(
                Pos2::new(x, cy - title_galley.rect.height() * 0.5),
                title_galley.clone(),
                t.dim,
            );
            x += title_galley.rect.width();

            // Count chip — same treatment as PanelSection.count.
            if let Some(n) = count {
                x += gap_xs();
                let count_text = format!("{}", n);
                let count_color = color_alpha(t.dim, 200);
                let count_font = FontId::monospace(font_xs());
                let count_galley = ui.fonts(|f| {
                    f.layout_no_wrap(count_text, count_font, count_color)
                });
                painter.galley(
                    Pos2::new(x, cy - count_galley.rect.height() * 0.5),
                    count_galley,
                    count_color,
                );
            }

            // Bottom hairline rule — always, expanded or collapsed.
            let rule_y = rect.bottom() - 0.5;
            painter.line_segment(
                [
                    Pos2::new(rect.left(), rule_y),
                    Pos2::new(rect.right(), rule_y),
                ],
                Stroke::new(stroke_thin(), color_alpha(t.toolbar_border, RULE_ALPHA)),
            );
        }

        // Ensure the interaction has a stable id even when the same title
        // appears twice in a panel.
        let _ = ui.interact(
            rect,
            ui.id().with(("panel_sub_section", id_salt)),
            Sense::click(),
        );

        // Body — only when expanded.
        if !is_open {
            return None;
        }

        ui.add_space(gap_2xs());
        let mut out: Option<R> = None;
        ui.horizontal(|ui| {
            ui.add_space(gap_md());
            ui.vertical(|ui| {
                out = Some(body(ui, t));
            });
        });
        ui.add_space(gap_xs());
        out
    }
}
