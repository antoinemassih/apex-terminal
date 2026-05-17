//! panel_divider — the single canonical hairline divider for panel bodies.
//!
//! Inline helper, not a builder. Paints one full-width hairline at
//! `stroke_thin()` and `color_alpha(t.toolbar_border, 36)`. **One weight, one
//! alpha, one method.** This is the ONLY divider primitive panels should use
//! in their body content.
//!
//! ```ignore
//! panel_divider(ui, t);
//! ```
//!
//! When to use:
//! - Between two `PanelSection`s when both have `.rule(false)`.
//! - Inside a long `PanelCard` to split logical groups.
//! - Anywhere you'd otherwise hand-roll a `ui.painter().line_segment(...)`.
//!
//! When NOT to use:
//! - Vertical dividers inside a header action cluster — use
//!   `kit::panel_action_divider`.
//! - The bottom rule of a `PanelSection` — that's drawn by the section itself
//!   when `.rule(true)` (the default).
//! - Form/field separation — use `Separator`.
//!
//! Sister widgets: `Separator`, `PanelSection` (auto-rule).

use egui::{Pos2, Stroke, Ui};

use crate::chart::renderer::ui::style::{color_alpha, gap_xs, stroke_thin};
use crate::chart_renderer::gpu::Theme;

/// Alpha of the panel divider line — locked at 36/255 per design spec.
const DIVIDER_ALPHA: u8 = 36;

/// Paint one full-width hairline divider at the current cursor.
pub fn panel_divider(ui: &mut Ui, t: &Theme) {
    ui.add_space(gap_xs());
    let rect = ui.available_rect_before_wrap();
    let y = ui.cursor().min.y;
    ui.painter().line_segment(
        [
            Pos2::new(rect.left(), y),
            Pos2::new(rect.right(), y),
        ],
        Stroke::new(stroke_thin(), color_alpha(t.toolbar_border, DIVIDER_ALPHA)),
    );
    ui.add_space(gap_xs());
}
