//! SegmentedControl — horizontal connected-pill selector for picking one of N options.
//!
//! Use for: order type (MKT/LMT/STP), time-in-force (DAY/GTC/IOC), bar style
//! (Candle/Heikin/Renko), or any small fixed-set toggle.
//!
//! ```ignore
//! const OPTS: &[(usize, &str)] = &[(0, "MKT"), (1, "LMT"), (2, "STP")];
//! if SegmentedControl::new(&mut order_type_idx, OPTS).show(ui, theme).changed() {
//!     // selection changed
//! }
//!
//! // With custom render:
//! SegmentedControl::new_with(&mut value, &items, |item| item.label())
//!     .compact(true).show(ui, theme);
//! ```

use egui::{Color32, CornerRadius, FontId, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Ui, Vec2};
use super::theme::ComponentTheme;
use super::tokens::Size;
use super::motion;
use crate::ui_kit::tokens as st;

/// Horizontal connected-pill selector for picking one of N fixed options.
///
/// **Connected mode** (default): all segments share one bordered outer
/// container; hairline verticals separate them. Active segment gets a
/// soft accent fill + accent text.
///
/// **Separated mode** (`.connected(false)`): each segment is its own
/// bordered pill with a small gap between.
#[must_use = "SegmentedControl does nothing until `.show(ui, theme)` is called"]
pub struct SegmentedControl<'a, T: Copy + PartialEq + 'a> {
    selected: &'a mut T,
    items: &'a [(T, &'a str)],
    label_fn: Option<Box<dyn Fn(&T) -> String + 'a>>,
    items_dyn: Option<&'a [T]>,
    size: Size,
    full_width: bool,
    compact: bool,
    connected: bool,
    disabled: bool,
}

impl<'a, T: Copy + PartialEq + 'a> SegmentedControl<'a, T> {
    /// Create from a static `(value, label)` slice.
    pub fn new(selected: &'a mut T, items: &'a [(T, &'a str)]) -> Self {
        Self {
            selected,
            items,
            label_fn: None,
            items_dyn: None,
            size: Size::Sm,
            full_width: false,
            compact: false,
            connected: true,
            disabled: false,
        }
    }

    /// Create from a slice of values + a label closure. Useful when the
    /// label comes from a method on `T`.
    pub fn new_with<F: Fn(&T) -> String + 'a>(
        selected: &'a mut T,
        items: &'a [T],
        label_fn: F,
    ) -> Self {
        Self {
            selected,
            items: &[],
            label_fn: Some(Box::new(label_fn)),
            items_dyn: Some(items),
            size: Size::Sm,
            full_width: false,
            compact: false,
            connected: true,
            disabled: false,
        }
    }

    /// Override the size tier (default: `Size::Sm`).
    pub fn size(mut self, s: Size) -> Self { self.size = s; self }
    /// Stretch to fill the available width.
    pub fn full_width(mut self) -> Self { self.full_width = true; self }
    /// Tighter font + reduced height for inline / dense contexts.
    pub fn compact(mut self, c: bool) -> Self { self.compact = c; self }
    /// `true` = one bordered container (default); `false` = individual pills.
    pub fn connected(mut self, c: bool) -> Self { self.connected = c; self }
    /// Disables interaction (alpha-dimmed, no hover).
    pub fn disabled(mut self, d: bool) -> Self { self.disabled = d; self }

    /// Render and return a `Response` whose `.changed()` is `true` when the
    /// user selected a different option.
    pub fn show(self, ui: &mut Ui, theme: &dyn ComponentTheme) -> Response {
        // ── Size tokens ───────────────────────────────────────────────────
        let font_size = if self.compact { st::font_xs() } else { self.size.font_size() };
        let height: f32 = match (self.compact, self.size) {
            (true, _)         => 16.0,
            (false, Size::Xs) => 18.0,
            (false, Size::Sm) => 22.0,
            (false, Size::Md) => 26.0,
            (false, Size::Lg) => 30.0,
        };
        let pad_x = if self.compact { st::gap_2xs() } else { st::gap_xs() };
        let radius = if self.connected { 4.0_f32 } else { 4.0_f32 };
        let gap = if self.connected { 0.0_f32 } else { 3.0_f32 };

        // Resolve item count + labels
        let count = if let Some(dyn_items) = self.items_dyn { dyn_items.len() } else { self.items.len() };
        if count == 0 {
            return ui.allocate_exact_size(Vec2::ZERO, Sense::hover()).1;
        }

        // ── Measure each segment's intrinsic width ────────────────────────
        let seg_widths: Vec<f32> = (0..count)
            .map(|i| {
                let lbl = self.label_for(i);
                // layout-only: only `.rect.width()` is read; color is discarded.
                let galley = ui.fonts(|f| {
                    f.layout_no_wrap(lbl, FontId::monospace(font_size), Color32::WHITE)
                });
                galley.rect.width() + 2.0 * pad_x
            })
            .collect();

        let natural_total = seg_widths.iter().sum::<f32>() + gap * (count.saturating_sub(1)) as f32;
        let avail = ui.available_width();
        let total_w = if self.full_width { avail.max(natural_total) } else { natural_total };

        // If full_width, expand each segment proportionally.
        let scale = if self.full_width && natural_total > 0.0 { total_w / natural_total } else { 1.0 };
        let seg_widths: Vec<f32> = seg_widths.iter().map(|w| (*w * scale).round()).collect();

        // ── Allocate the total rect ───────────────────────────────────────
        let desired = Vec2::new(total_w, height);
        let (total_rect, mut outer_resp) = ui.allocate_exact_size(desired, Sense::hover());

        if !ui.is_rect_visible(total_rect) {
            return outer_resp;
        }

        let accent = theme.accent();
        let border = theme.border();
        let painter = ui.painter_at(total_rect);
        let cr = CornerRadius::same(radius as u8);

        // ── Connected mode: draw the outer container ──────────────────────
        if self.connected {
            let fill = theme.surface();
            painter.rect_filled(total_rect, cr, fill);
            painter.rect_stroke(total_rect, cr, Stroke::new(st::stroke_thin(), border), StrokeKind::Inside);
        }

        // ── Per-segment pass ──────────────────────────────────────────────
        let mut x = total_rect.left();
        let cy = total_rect.center().y;
        let mut changed = false;

        for i in 0..count {
            let w = seg_widths[i];
            let seg_rect = Rect::from_min_size(Pos2::new(x, total_rect.top()), Vec2::new(w, height));

            let val = self.value_for(i);
            let is_active = *self.selected == val;
            let is_last = i == count - 1;

            // Interaction
            let seg_id = outer_resp.id.with(i);
            let seg_sense = if self.disabled { Sense::hover() } else { Sense::click() };
            let seg_resp = ui.interact(seg_rect, seg_id, seg_sense);
            let hovered = seg_resp.hovered() && !self.disabled;

            if seg_resp.clicked() && !is_active && !self.disabled {
                *self.selected = val;
                changed = true;
            }

            // Animation
            let hover_t  = motion::ease_bool(ui.ctx(), seg_id.with("h"), hovered,  motion::FAST);
            let active_t = motion::ease_bool(ui.ctx(), seg_id.with("a"), is_active, motion::MED);

            // Background
            let idle_bg   = if self.connected { Color32::TRANSPARENT } else { theme.surface() };
            let active_bg = st::color_alpha(accent, st::alpha_tint());
            let hover_bg  = st::color_alpha(theme.text(), 18);

            let mut bg = motion::lerp_color(idle_bg, hover_bg, hover_t);
            bg = motion::lerp_color(bg, active_bg, active_t);

            if self.disabled {
                bg = st::color_alpha(bg, (bg.a() as f32 * 0.5) as u8);
            }

            // In separated mode each pill gets its own border + radius.
            let seg_cr = if self.connected {
                // Leftmost: left corners rounded; rightmost: right corners rounded.
                match (i == 0, is_last) {
                    (true, true)  => cr,
                    (true, false) => CornerRadius { nw: radius as u8, sw: radius as u8, ne: 0, se: 0 },
                    (false, true) => CornerRadius { nw: 0, sw: 0, ne: radius as u8, se: radius as u8 },
                    (false, false) => CornerRadius::ZERO,
                }
            } else {
                cr
            };

            if bg.a() > 0 {
                painter.rect_filled(seg_rect, seg_cr, bg);
            }

            if !self.connected {
                let bcol = if is_active { st::color_alpha(accent, st::alpha_active()) } else { border };
                painter.rect_stroke(seg_rect, seg_cr, Stroke::new(st::stroke_thin(), bcol), StrokeKind::Inside);
            }

            // Hairline divider between segments (connected mode only, not after last).
            if self.connected && !is_last {
                let div_x = seg_rect.right();
                let top = total_rect.top() + 3.0;
                let bot = total_rect.bottom() - 3.0;
                painter.line_segment(
                    [Pos2::new(div_x, top), Pos2::new(div_x, bot)],
                    Stroke::new(st::stroke_thin(), border),
                );
            }

            // Label
            let fg_idle   = st::color_alpha(theme.dim(), 200);
            let fg_active = accent;
            let fg_t = if is_active { 1.0_f32 } else { hover_t * 0.4 };
            let mut fg = motion::lerp_color(fg_idle, fg_active, active_t);
            // Subtle brighten on hover when not active.
            if !is_active {
                fg = motion::lerp_color(fg, theme.text(), fg_t);
            }
            if self.disabled {
                fg = st::color_alpha(fg, (fg.a() as f32 * 0.5) as u8);
            }

            let font_id = if is_active {
                FontId::monospace(font_size)
            } else {
                FontId::monospace(font_size)
            };

            painter.text(
                Pos2::new(seg_rect.center().x, cy),
                egui::Align2::CENTER_CENTER,
                self.label_for(i),
                font_id,
                fg,
            );

            // Hover cursor
            if hovered {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }

            x += w + gap;

            // Merge response so the outer response reflects clicks.
            outer_resp = outer_resp.union(seg_resp);
        }

        // Mark as changed on the response.
        if changed {
            outer_resp.mark_changed();
        }
        outer_resp
    }

    // ── Helpers ───────────────────────────────────────────────────────────

    fn label_for(&self, i: usize) -> String {
        if let (Some(label_fn), Some(items_dyn)) = (self.label_fn.as_ref(), self.items_dyn) {
            label_fn(&items_dyn[i])
        } else {
            self.items[i].1.to_string()
        }
    }

    fn value_for(&self, i: usize) -> T {
        if let Some(items_dyn) = self.items_dyn {
            items_dyn[i]
        } else {
            self.items[i].0
        }
    }
}
