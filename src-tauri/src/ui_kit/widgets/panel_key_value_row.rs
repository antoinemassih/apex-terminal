//! PanelKeyValueRow — two-column label/value row for metric displays.
//!
//! ```ignore
//! PanelKeyValueRow::new("Buying Power", "$12,500.00")
//!     .tone(Tone::Default)
//!     .meta("USD")
//!     .show(ui, t);
//! ```
//!
//! Visual spec (locked):
//! - Two columns: label LEFT, value (and optional meta) RIGHT.
//! - Label: `mono_xs` in `color_muted(palette_ct(t).base(SxTone::Dim))`.
//! - Value: `mono_sm` in `t.text` by default, or `tone.color(t)` when set.
//! - Meta: optional very-muted `mono_xs` after the value (units, qualifier).
//! - Row height: `t.row_height()` — the per-style, density-aware row-height
//!   token (the same one watchlist/option-chain rows use). Previously this was
//!   `gap_lg()` (a SPACING token) which scrunched the row to 16px.
//!
//! When to use:
//! - Inside a `PanelSection` body for label/value metric stacks ("Buying Power",
//!   "Realized P&L", "Win Rate", etc.).
//! - Inside a `PanelCard` for the body of a summary card.
//!
//! When NOT to use:
//! - Clickable list items — use `PanelListRow`.
//! - Rows with bar/progress visualization — use `MetricRow`.
//! - Form fields (label + input control) — use `FormRow`.
//!
//! Sister widgets: `MetricRow`, `PanelListRow`, `PanelSection`.

use egui::{Pos2, Rect, Sense, Ui, Vec2};

use super::panel_section::Tone;
use crate::ui_kit::layout::{Align as FlexAlign, Flex, Item};
use crate::ui_kit::tokens::{
    color_alpha, color_muted, gap_xs,
};
use crate::ui_kit::widgets::theme::ComponentTheme;
use crate::ui_kit::text_style::TextStyle;
use crate::ui_kit::sx::{palette_ct, Tone as SxTone};

// ─── Row geometry (flexbox) ──────────────────────────────────────────────────
//
// The row used to right-align its value by walking a cursor leftwards from
// `rect.right()`, subtracting the meta galley and a `gap_xs`. That is the
// textbook "push the trailing content right" case, and it is now one flex row:
//
//   ┌ label (grows) ┬ gap_xs ┬ value (grows) ┬ gap_xs ┬ meta (measured) ┐
//
// Both text columns are anchored to the OUTER edges of their slots — label
// `LEFT_CENTER` at `label.left()`, value `RIGHT_CENTER` at `value.right()` —
// so neither the label nor the value needs to be measured, exactly as before.
// Geometry only: fonts, colours and tones are untouched.

/// Solved slots of a [`PanelKeyValueRow`]. Absolute rects.
#[derive(Clone, Copy, Debug)]
pub struct KeyValueRowSlots {
    /// Label column. Paint `LEFT_CENTER` at `label.left()`.
    pub label: Rect,
    /// Value column. Paint `RIGHT_CENTER` at `value.right()`.
    pub value: Rect,
    /// Optional trailing meta column, flush right.
    pub meta: Option<Rect>,
}

/// The flex spec for a key/value row. `meta_w` is the MEASURED meta galley
/// width (`Item::auto()` has no intrinsic size under `Flex::solve`).
pub fn key_value_row_flex(meta_w: Option<f32>) -> Flex {
    let mut f = Flex::row()
        .gap(gap_xs())
        .align(FlexAlign::Stretch)
        .item(Item::grow(1.0))
        .item(Item::grow(1.0));
    if let Some(w) = meta_w {
        // Shrinks rather than overflowing on a pathologically narrow row, so
        // the meta stays flush right the way the old cursor arithmetic did.
        f = f.item(Item::fixed(w).shrink(1.0));
    }
    f
}

/// Solve [`key_value_row_flex`] inside `rect`.
pub fn solve_key_value_row(rect: Rect, meta_w: Option<f32>) -> KeyValueRowSlots {
    let solved = key_value_row_flex(meta_w).solve(rect.size());
    let off = rect.min.to_vec2();
    let mut it = solved.into_iter().map(|r| r.translate(off));
    let label = it.next().unwrap_or(Rect::NOTHING);
    let value = it.next().unwrap_or(Rect::NOTHING);
    let meta = if meta_w.is_some() { it.next() } else { None };
    KeyValueRowSlots { label, value, meta }
}

#[must_use = "PanelKeyValueRow must be rendered with `.show(...)`"]
pub struct PanelKeyValueRow<'a> {
    label: &'a str,
    value: String,
    tone: Tone,
    meta: Option<String>,
}

impl<'a> PanelKeyValueRow<'a> {
    pub fn new(label: &'a str, value: impl Into<String>) -> Self {
        Self {
            label,
            value: value.into(),
            tone: Tone::Default,
            meta: None,
        }
    }

    pub fn tone(mut self, tone: Tone) -> Self {
        self.tone = tone;
        self
    }

    pub fn meta(mut self, m: impl Into<String>) -> Self {
        self.meta = Some(m.into());
        self
    }

    pub fn show<T: ComponentTheme>(self, ui: &mut Ui, t: &T) {
        let h = t.row_height();
        let avail_w = ui.available_width();
        let (rect, _resp) = ui.allocate_exact_size(Vec2::new(avail_w, h), Sense::hover());

        let painter = ui.painter_at(rect);

        // Measure the meta galley (the only intrinsically-sized column) so the
        // flex engine can reserve it; label and value stay unmeasured.
        let meta_color = color_alpha(palette_ct(t).base(SxTone::Dim), crate::ui_kit::style::alpha_scrim());
        let meta_font = TextStyle::MonoXs.font_id_in(ui);
        let meta_w = self.meta.as_ref().map(|m| {
            ui.fonts(|f| f.layout_no_wrap(m.clone(), meta_font.clone(), meta_color))
                .size().x
        });

        let slots = solve_key_value_row(rect, meta_w);

        // Label — left. PROPORTIONAL (labels are text, not data); the value on
        // the right stays monospace so numbers align. Consistent with the
        // app-wide label=proportional / value=mono rule.
        let label_color = color_muted(palette_ct(t).base(SxTone::Dim));
        let label_font = crate::ui_kit::style::prop_at(crate::ui_kit::style::font_xs());
        painter.text(
            Pos2::new(slots.label.left(), slots.label.center().y),
            egui::Align2::LEFT_CENTER,
            self.label,
            label_font,
            label_color,
        );

        // Value (+ meta) — right.
        let value_color = match self.tone {
            Tone::Default => palette_ct(t).base(SxTone::Text),
            other => other.color(t),
        };
        let value_font = TextStyle::MonoSm.font_id_in(ui);

        if let (Some(m), Some(mslot)) = (&self.meta, slots.meta) {
            painter.text(
                Pos2::new(mslot.right(), mslot.center().y),
                egui::Align2::RIGHT_CENTER,
                m,
                meta_font,
                meta_color,
            );
        }

        painter.text(
            Pos2::new(slots.value.right(), slots.value.center().y),
            egui::Align2::RIGHT_CENTER,
            &self.value,
            value_font,
            value_color,
        );
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────
//
// `solve_key_value_row` is pure geometry, so the two-column alignment that used
// to require eyeballing a running screenshot is now asserted headlessly.

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f32, b: f32) -> bool { (a - b).abs() < 0.01 }

    fn row() -> Rect {
        Rect::from_min_size(egui::pos2(12.0, 40.0), Vec2::new(280.0, 22.0))
    }

    /// The core shape: key flush left, value flush right, no meta.
    #[test]
    fn key_left_value_right() {
        let r = row();
        let s = solve_key_value_row(r, None);
        assert!(approx(s.label.left(), r.left()), "got {}", s.label.left());
        assert!(approx(s.value.right(), r.right()), "got {}", s.value.right());
        assert!(s.meta.is_none());
    }

    /// With a meta column the value sits one `gap_xs` to its left — the exact
    /// gutter the old `x_right -= g.size().x + gap_xs()` produced.
    #[test]
    fn meta_is_flush_right_and_the_value_clears_it_by_one_gap() {
        let r = row();
        let s = solve_key_value_row(r, Some(30.0));
        let m = s.meta.expect("meta slot");
        assert!(approx(m.right(), r.right()), "got {}", m.right());
        assert!(approx(m.width(), 30.0), "got {}", m.width());
        assert!(
            approx(s.value.right(), r.right() - 30.0 - gap_xs()),
            "got {}", s.value.right()
        );
        assert!(approx(s.label.left(), r.left()));
    }

    /// The two text columns split the row, so neither has to be measured and
    /// the gutter between them is the `gap_xs` token.
    #[test]
    fn label_and_value_share_the_row_with_a_token_gutter() {
        let r = row();
        let s = solve_key_value_row(r, None);
        assert!(approx(s.value.left() - s.label.right(), gap_xs()));
        assert!(approx(s.label.width(), s.value.width()));
        assert!(approx(s.label.width() + s.value.width() + gap_xs(), r.width()));
    }

    /// Every slot spans the full row height, so painting at `slot.center().y`
    /// is the same vertical centring the widget had before.
    #[test]
    fn slots_span_the_full_row_height() {
        let r = row();
        let s = solve_key_value_row(r, Some(30.0));
        assert!(approx(s.label.height(), r.height()));
        assert!(approx(s.value.center().y, r.center().y));
        assert!(approx(s.meta.unwrap().center().y, r.center().y));
    }

    /// A row narrower than its own meta column must not push the meta past the
    /// right edge — panels get laid out at degenerate widths while dragging.
    #[test]
    fn degenerate_width_keeps_the_meta_inside_the_row() {
        let r = Rect::from_min_size(egui::pos2(0.0, 0.0), Vec2::new(10.0, 22.0));
        let s = solve_key_value_row(r, Some(60.0));
        let m = s.meta.expect("meta slot");
        assert!(m.right() <= r.right() + 0.01, "got {}", m.right());
    }
}
