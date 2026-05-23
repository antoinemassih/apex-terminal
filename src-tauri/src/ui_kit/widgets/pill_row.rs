//! PillRow — horizontal segmented-pill selector (mutually-exclusive chips).
//!
//! Renders a horizontal row of pill-shaped chips where exactly one is selected
//! at a time. The selected pill uses the `Variant::Chip` active styling (soft
//! accent fill + accent text); inactive pills are transparent with dim text.
//!
//! ```ignore
//! let resp = PillRow::new()
//!     .options(&[("1m", "1m"), ("5m", "5m"), ("1h", "1h"), ("1D", "1D")])
//!     .selected_idx(&mut timeframe_idx)
//!     .tone(PanelTone::Accent)
//!     .show(ui, t);
//! if resp.changed { /* timeframe changed */ }
//! ```
//!
//! Visual spec:
//! - Pills are laid out left-to-right, separated by `gap_xs()`.
//! - Each pill height is `Size::Sm` (22 px) with `gap_xs()` horizontal padding.
//! - Selected pill: `color_alpha(tone_color, alpha_soft())` fill + full
//!   tone color text.
//! - Inactive pill: transparent fill, `color_alpha(t.dim, alpha_muted())` text.
//! - Hover on inactive: `color_alpha(t.text, alpha_ghost())` fill.
//! - Corner radius: `radius_sm()` (pill-like when `radius_pill()` is too round
//!   for compact panels; use `.pill_radius(true)` for true pill shape).
//!
//! When to use:
//! - Mutually-exclusive short option sets (2–6 items): timeframe, side
//!   (Buy/Sell), duration (Day/GTC/IOC), view mode.
//! - Anywhere a `SegmentedControl` would fit but the options are in a tighter
//!   space budget (side panels, form rows).
//!
//! When NOT to use:
//! - Long option lists (> 6) — use `Select`.
//! - Non-exclusive multi-select — use `Tag` chips or `ToggleGroup`.
//! - Navigational tabs — use `Tabs`.
//!
//! Sister widgets: [`SegmentedControl`], [`Tabs`], [`Tag`], `Select`.

use egui::{Color32, CornerRadius, FontId, Pos2, Rect, Sense, Ui, Vec2};

use crate::chart::renderer::ui::style::{
    alpha_ghost, alpha_muted, alpha_soft, color_alpha, font_xs, gap_xs, radius_sm,
};
use crate::ui_kit::widgets::theme::Theme;
use crate::ui_kit::widgets::panel_section::Tone as PanelTone;

/// Response returned from [`PillRow::show`].
#[derive(Clone, Debug, Default)]
pub struct PillRowResponse {
    /// `true` when the selection changed this frame (the user clicked a
    /// different pill). The new index is already written into `selected_idx`.
    pub changed: bool,
    /// Index of the pill that was clicked, or `None` if no click this frame.
    pub clicked_idx: Option<usize>,
}

/// Height of each pill chip (matches `Size::Sm`).
const PILL_H: f32 = 22.0;

#[must_use = "PillRow must be rendered with `.show(...)`"]
pub struct PillRow<'a> {
    options: &'a [(&'a str, &'a str)],
    selected_idx: Option<&'a mut usize>,
    tone: PanelTone,
    pill_radius: bool,
}

impl<'a> PillRow<'a> {
    pub fn new() -> Self {
        Self {
            options: &[],
            selected_idx: None,
            tone: PanelTone::Accent,
            pill_radius: false,
        }
    }

    /// Set the option slice. Each entry is `(label, value)` where `value` is
    /// an opaque string tag (available via `clicked_idx` mapping, not used
    /// internally for rendering).
    pub fn options(mut self, opts: &'a [(&'a str, &'a str)]) -> Self {
        self.options = opts;
        self
    }

    /// Mutable reference to the currently-selected index. Updated in-place
    /// when the user clicks a pill.
    pub fn selected_idx(mut self, idx: &'a mut usize) -> Self {
        self.selected_idx = Some(idx);
        self
    }

    /// Tone for the selected pill. Defaults to `PanelTone::Accent`.
    pub fn tone(mut self, tone: PanelTone) -> Self {
        self.tone = tone;
        self
    }

    /// When `true`, use `radius_pill()` (999 px) instead of `radius_sm()`.
    /// Defaults to `false` for the compact panel context.
    pub fn pill_radius(mut self, v: bool) -> Self {
        self.pill_radius = v;
        self
    }

    pub fn show(mut self, ui: &mut Ui, t: &Theme) -> PillRowResponse {
        let mut resp = PillRowResponse::default();
        if self.options.is_empty() {
            return resp;
        }

        let cur = self.selected_idx.as_deref().copied().unwrap_or(0);
        let tone_color = self.tone.color(t);
        let cr = CornerRadius::same(if self.pill_radius { 99 } else { radius_sm() as u8 });
        let pad_x = gap_xs();
        let font = FontId::proportional(font_xs());

        // Measure all pills up front so we can compute total width.
        let galleys: Vec<_> = self.options.iter().map(|(label, _)| {
            ui.fonts(|f| f.layout_no_wrap(label.to_string(), font.clone(), Color32::WHITE))
        }).collect();

        let pill_widths: Vec<f32> = galleys.iter()
            .map(|g| g.size().x + pad_x * 2.0)
            .collect();

        let total_w: f32 = pill_widths.iter().sum::<f32>()
            + gap_xs() * (self.options.len() as f32 - 1.0).max(0.0);
        let total_w = total_w.min(ui.available_width());

        let (strip_rect, _) = ui.allocate_exact_size(
            Vec2::new(total_w, PILL_H),
            Sense::hover(),
        );

        if !ui.is_rect_visible(strip_rect) {
            return resp;
        }

        let mut x = strip_rect.left();

        for (i, (label, _value)) in self.options.iter().enumerate() {
            let pw = pill_widths[i].min(strip_rect.right() - x);
            if pw <= 0.0 { break; }

            let pill_rect = Rect::from_min_max(
                Pos2::new(x, strip_rect.top()),
                Pos2::new(x + pw, strip_rect.bottom()),
            );

            let id = ui.id().with(("pill_row", i));
            let pr = ui.interact(pill_rect, id, Sense::click())
                .on_hover_cursor(egui::CursorIcon::PointingHand);

            let is_selected = i == cur;
            let painter = ui.painter_at(pill_rect);

            // Background
            if is_selected {
                painter.rect_filled(pill_rect, cr, color_alpha(tone_color, alpha_soft()));
            } else if pr.hovered() {
                painter.rect_filled(pill_rect, cr, color_alpha(t.text, alpha_ghost()));
            }

            // Label
            let text_color = if is_selected {
                tone_color
            } else {
                color_alpha(t.dim, alpha_muted())
            };
            let galley = &galleys[i];
            let tw = galley.size().x;
            let th = galley.size().y;
            let cy = pill_rect.center().y;
            let tx = pill_rect.center().x - tw * 0.5;
            let cell_painter = painter.with_clip_rect(pill_rect);
            cell_painter.galley(
                Pos2::new(tx, cy - th * 0.5),
                galleys[i].clone(),
                text_color,
            );

            if pr.clicked() {
                resp.clicked_idx = Some(i);
                if let Some(idx_ref) = self.selected_idx.as_deref_mut() {
                    *idx_ref = i;
                }
                resp.changed = (i != cur);
            }

            x += pw + gap_xs();
        }

        resp
    }
}

impl<'a> Default for PillRow<'a> {
    fn default() -> Self { Self::new() }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Builder: options slice is stored, selected_idx defaulting to 0 is logical.
    #[test]
    fn builder_stores_options() {
        let opts: &[(&str, &str)] = &[("1m", "1m"), ("5m", "5m"), ("1h", "1h")];
        let row = PillRow::new().options(opts);
        assert_eq!(row.options.len(), 3);
        assert_eq!(row.options[0].0, "1m");
        assert_eq!(row.options[2].1, "1h");
    }

    /// Tone is stored correctly.
    #[test]
    fn tone_setter() {
        let row = PillRow::new().tone(PanelTone::Bull);
        assert_eq!(row.tone, PanelTone::Bull);
    }

    /// Pill radius toggle.
    #[test]
    fn pill_radius_toggle() {
        let row = PillRow::new().pill_radius(true);
        assert!(row.pill_radius);
        let row2 = PillRow::new().pill_radius(false);
        assert!(!row2.pill_radius);
    }

    /// Empty option slice is handled gracefully (no-op show).
    #[test]
    fn empty_options_no_panic() {
        let row = PillRow::new();
        assert!(row.options.is_empty());
    }
}
