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

use egui::{Color32, CornerRadius, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Ui, Vec2};

use crate::ui_kit::layout::{Align as FlexAlign, Flex, Item};
use super::theme::ComponentTheme;
use super::tokens::Size;
use super::motion;
use crate::ui_kit::tokens as st;
use crate::ui_kit::sx::{palette_ct, Tone};
use crate::ui_kit::interaction::{apply_interaction, InteractionState, InteractionTokens};

/// Hover-fill alpha for an inactive segment. Slightly firmer than the global
/// `alpha_ghost` because the pill sits on the control's own surface fill.
const SEGMENT_HOVER_ALPHA: u8 = 18;

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
        // Build the ctx from the UI so it carries the AMBIENT RecipeSet.
        // `StyleCtx::from_theme` would hand this widget an empty set — see
        // `ctx.rs` for why that shim must never be used inside a `show`.
        let sctx = super::ctx::StyleCtx::from_ui(theme, ui);
        self.show_ctx(ui, &sctx)
    }

    /// [`StyleCtx`](super::ctx::StyleCtx) entry point.
    ///
    /// Callers that need per-call-site token overrides or an explicit
    /// `RecipeSet` construct a `StyleCtx` and call this directly; `show`
    /// delegates here with the ambient one.
    pub fn show_ctx(self, ui: &mut Ui, sctx: &super::ctx::StyleCtx<'_>) -> Response {
        let theme = sctx.theme();
        // ── Size tokens ───────────────────────────────────────────────────
        let font_size = if self.compact { st::font_xs() } else { self.size.font_size() };
        let height: f32 = match (self.compact, self.size) {
            (true, _)         => 16.0,
            (false, Size::Xs) => 18.0,
            (false, Size::Sm) => 22.0,
            (false, Size::Md) => 26.0,
            (false, Size::Lg) => 30.0,
            (false, Size::Xl) => 30.0,
        };
        let pad_x = if self.compact { st::gap_2xs() } else { st::gap_xs() };
        let radius = 4.0_f32; // same in connected + separated modes (was a dead conditional)
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
                    f.layout_no_wrap(lbl, crate::ui_kit::style::mono_at(font_size), egui::Color32::PLACEHOLDER)
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

        let pal = palette_ct(theme);
        let accent = pal.base(Tone::Accent);
        let border = pal.base(Tone::Border);
        let painter = ui.painter_at(total_rect);
        let cr = CornerRadius::same(radius as u8);

        // ── Connected mode: draw the outer container ──────────────────────
        if self.connected {
            let fill = palette_ct(theme).base(Tone::Surface);
            // `segmented` key — the trough. Segment fills stay with the widget
            // (they encode selection state).
            let (cr, t_fill, t_stroke) = super::theme::resolve_control_chrome(
                ui.ctx(), theme, "segmented", radius, fill, border, st::stroke_thin(),
            );
            painter.rect_filled(total_rect, cr, t_fill);
            painter.rect_stroke(total_rect, cr, t_stroke, StrokeKind::Inside);
        }

        // ── Segment strip geometry ────────────────────────────────────────
        //
        // M4.3: `let mut x = total_rect.left(); … x += w + gap;` was the whole
        // strip. Fixed-width children on a `gap` gutter is the canonical flex
        // row — and `Item::fixed` (not `content`) is deliberate: the widths
        // are already scaled/rounded above, and a cursor walk overflows rather
        // than shrinking when `full_width` rounding over-subscribes the row.
        let seg_off = total_rect.min.to_vec2();
        let seg_rects: Vec<Rect> = Flex::row()
            .gap(gap)
            .align(FlexAlign::Stretch)
            .items(seg_widths.iter().map(|w| Item::fixed(*w)))
            .solve(total_rect.size())
            .into_iter()
            .map(|r| r.translate(seg_off))
            .collect();

        // ── Per-segment pass ──────────────────────────────────────────────
        let cy = total_rect.center().y;
        let mut changed = false;

        for i in 0..count {
            let seg_rect = seg_rects[i];

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
            // M3.3: the hover and active fills come from the ONE interaction
            // table — `selected` for the active pill (accent @ alpha_tint) and
            // `hover` for the neutral text tint this control has always used.
            let idle_bg   = if self.connected { Color32::TRANSPARENT } else { pal.base(Tone::Surface) };
            let active_bg = apply_interaction(
                seg_rect,
                InteractionState::new().selected(true),
                accent,
                &InteractionTokens::borderless(),
            ).fill;
            let hover_bg  = apply_interaction(
                seg_rect,
                InteractionState::new().hovered(true),
                pal.base(Tone::Text),
                &InteractionTokens::borderless().hover_alpha(SEGMENT_HOVER_ALPHA),
            ).fill;

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
            let fg_idle   = st::color_alpha(pal.base(Tone::Dim), crate::ui_kit::style::alpha_solid());
            let fg_active = accent;
            let fg_t = if is_active { 1.0_f32 } else { hover_t * 0.4 };
            let mut fg = motion::lerp_color(fg_idle, fg_active, active_t);
            // Subtle brighten on hover when not active.
            if !is_active {
                fg = motion::lerp_color(fg, pal.base(Tone::Text), fg_t);
            }
            if self.disabled {
                fg = st::color_alpha(fg, (fg.a() as f32 * 0.5) as u8);
            }

            let font_id = if is_active {
                crate::ui_kit::style::mono_at(font_size)
            } else {
                crate::ui_kit::style::mono_at(font_size)
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

            st::cursor::focus_ring(ui, &seg_resp, pal.base(Tone::Accent));

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
