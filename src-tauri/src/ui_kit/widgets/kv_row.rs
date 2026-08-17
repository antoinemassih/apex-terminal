//! `KvRow` — a label/value row for painter-only surfaces.
//!
//! ```ignore
//! KvRow::new("GAP", format!("{:+.1}%", gap))
//!     .label_font(mono_2xs())
//!     .label_color(color_dim(t.dim))
//!     .value_font(mono_sm())
//!     .value_color(gap_col)
//!     .show(painter, t, rect);
//! ```
//!
//! # Why this exists
//!
//! Twelve surfaces in `chart_widgets` alone paint the same two calls:
//!
//! ```ignore
//! p.text(pos2(left,  y), Align2::LEFT_CENTER,  label, font_a, col_a);
//! p.text(pos2(right, y), Align2::RIGHT_CENTER, value, font_b, col_b);
//! ```
//!
//! Every copy states the row's right edge twice — once in `right` and once by
//! anchoring to it — and each is free to be computed differently. That is the
//! same class of defect as the tab strip's fit/paint disagreement and the
//! spreadsheet's three spellings of a column offset: not a bug today, a bug the
//! moment one of the two moves.
//!
//! Underneath it is an [`El`] row with a spacer, so "flush right" is a property
//! of the tree rather than an arithmetic coincidence, and the whole row honours
//! the cascade — an ancestor's declared colour reaches both halves unless they
//! state their own.
//!
//! # Painter-only on purpose
//!
//! This takes a `&Painter`, not a `&mut Ui`. The surfaces that need it — chart
//! overlay panels, pane chrome — never receive a `Ui`, which is exactly why the
//! element tree's component half sat unused for so long (AT-167). A `Ui`-based
//! sibling would be `PanelKeyValueRow`, which already exists and is deliberately
//! NOT built on this: it has exact-pixel geometry tests that a font-measured
//! tree would turn into font-metric-dependent ones.

use egui::{Color32, FontId, Painter, Rect};

use crate::ui_kit::cascade::El;
use crate::ui_kit::widgets::theme::ComponentTheme;

#[must_use = "KvRow does nothing until `.show(painter, theme, rect)` is called"]
pub struct KvRow {
    label: String,
    value: String,
    label_font: Option<FontId>,
    label_color: Option<Color32>,
    value_font: Option<FontId>,
    value_color: Option<Color32>,
    /// A fixed left column. `None` = the label sizes to its text and the
    /// spacer takes the rest, which is what a two-item row wants; `Some(w)`
    /// pins the label column so a STACK of rows aligns its values.
    label_w: Option<f32>,
}

impl KvRow {
    pub fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
            label_font: None,
            label_color: None,
            value_font: None,
            value_color: None,
            label_w: None,
        }
    }

    pub fn label_font(mut self, f: FontId) -> Self { self.label_font = Some(f); self }
    pub fn value_font(mut self, f: FontId) -> Self { self.value_font = Some(f); self }
    /// Absent means "inherit" — an ancestor's `cascade::scope` colour reaches
    /// this half. That is the point of not defaulting it to a palette tone.
    pub fn label_color(mut self, c: Color32) -> Self { self.label_color = Some(c); self }
    pub fn value_color(mut self, c: Color32) -> Self { self.value_color = Some(c); self }
    pub fn label_w(mut self, w: f32) -> Self { self.label_w = Some(w); self }

    /// The row as a tree — exposed so a caller can nest it, and so the tests
    /// can solve it without painting.
    #[must_use]
    pub fn el(self) -> El {
        let mut label = El::text_with_font(
            self.label,
            self.label_font.unwrap_or_else(|| crate::ui_kit::style::mono_xs()),
        );
        if let Some(c) = self.label_color {
            label = label.color(c);
        }
        if let Some(w) = self.label_w {
            label = label.fixed(w);
        }

        let mut value = El::text_with_font(
            self.value,
            self.value_font.unwrap_or_else(|| crate::ui_kit::style::mono_sm()),
        );
        if let Some(c) = self.value_color {
            value = value.color(c);
        }

        El::row().child(label).child(El::spacer()).child(value)
    }

    /// Solve and paint into `rect`. The row is vertically centred in it.
    pub fn show(self, painter: &Painter, theme: &dyn ComponentTheme, rect: Rect) {
        self.el().show_with(painter, theme, rect);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui_kit::cascade::{context, Inherited};
    use crate::ui_kit::text_style::TextStyle;
    use crate::ui_kit::widgets::theme::PortableTheme;
    use std::cell::{Cell, RefCell};

    /// Paint one row and return (x, colour) for every text shape, left to
    /// right. Two frames so the font atlas exists — with one, every string
    /// measures 0 px and a "flush right" assertion proves nothing.
    fn painted(f: impl FnOnce(&Painter, &PortableTheme)) -> Vec<(f32, Color32)> {
        let out = RefCell::new(Vec::new());
        let f = Cell::new(Some(f));
        let ctx = egui::Context::default();
        let _ = ctx.run(Default::default(), |_| {});
        let _ = ctx.run(Default::default(), |c| {
            egui::CentralPanel::default().show(c, |ui| {
                TextStyle::install(ui.style_mut());
                let p = ui.painter().clone();
                if let Some(f) = f.take() {
                    f(&p, &PortableTheme::dark());
                }
                let layer = ui.layer_id();
                let mut v: Vec<(f32, Color32)> = ui.ctx().graphics(|g| {
                    g.get(layer)
                        .map(|l| {
                            l.all_entries()
                                .filter_map(|cs| match &cs.shape {
                                    egui::Shape::Text(t) => Some((
                                        t.pos.x,
                                        t.galley
                                            .job
                                            .sections
                                            .first()
                                            .map_or(Color32::PLACEHOLDER, |s| s.format.color),
                                    )),
                                    _ => None,
                                })
                                .collect()
                        })
                        .unwrap_or_default()
                });
                v.sort_by(|a, b| a.0.total_cmp(&b.0));
                *out.borrow_mut() = v;
            });
        });
        out.into_inner()
    }

    fn rect() -> Rect {
        Rect::from_min_size(egui::pos2(20.0, 0.0), egui::vec2(200.0, 16.0))
    }

    /// The shape: label at the left edge, value pushed to the right by the
    /// spacer. This is the assertion the twelve hand-written copies could not
    /// make, because each one anchored its value to a `right` computed
    /// separately from the row it was painted into.
    #[test]
    fn the_label_is_left_and_the_value_is_pushed_right() {
        let v = painted(|p, t| {
            context::reset_for_frame();
            KvRow::new("GAP", "+1.2%").show(p, t, rect());
        });
        assert_eq!(v.len(), 2, "label + value expected, got {v:?}");
        assert!((v[0].0 - 20.0).abs() < 0.01, "label at the rect's left: {v:?}");
        assert!(v[1].0 > 100.0, "the spacer must push the value right: {v:?}");
        assert!(v[1].0 < 220.0, "and not past the rect: {v:?}");
    }

    /// A longer value starts FURTHER LEFT — flush right means the value's
    /// right edge is pinned, not its left. Hand-written `RIGHT_CENTER` got
    /// this right by construction; a tree has to earn it.
    #[test]
    fn a_longer_value_extends_leftwards() {
        let short = painted(|p, t| {
            context::reset_for_frame();
            KvRow::new("GAP", "1%").show(p, t, rect());
        });
        let long = painted(|p, t| {
            context::reset_for_frame();
            KvRow::new("GAP", "-1234.56%").show(p, t, rect());
        });
        assert!(
            long[1].0 < short[1].0,
            "a longer value must extend left, not right: long={long:?}, short={short:?}"
        );
    }

    /// Colours are per-half and independent.
    #[test]
    fn each_half_keeps_its_own_colour() {
        let lc = Color32::from_rgb(11, 22, 33);
        let vc = Color32::from_rgb(200, 100, 50);
        let v = painted(|p, t| {
            context::reset_for_frame();
            KvRow::new("GAP", "1%").label_color(lc).value_color(vc).show(p, t, rect());
        });
        assert_eq!(v[0].1, lc, "label colour");
        assert_eq!(v[1].1, vc, "value colour");
    }

    /// With no colour stated, an ancestor's declaration reaches BOTH halves.
    /// This is the cascade doing the job the twelve copies did by passing the
    /// same resolved colour to two calls.
    #[test]
    fn an_undeclared_row_inherits_from_an_ancestor() {
        let declared = Color32::from_rgb(9, 90, 190);
        let v = painted(|p, t| {
            context::reset_for_frame();
            context::scope(Inherited::default().color(declared), || {
                KvRow::new("GAP", "1%").show(p, t, rect());
            });
        });
        assert_eq!(v[0].1, declared, "label inherits");
        assert_eq!(v[1].1, declared, "value inherits");
    }

    /// A stated colour outranks the ancestor — the direction that must not
    /// regress, since every adopting call site states its own.
    #[test]
    fn a_stated_colour_outranks_the_ancestor() {
        let ancestor = Color32::from_rgb(9, 90, 190);
        let mine = Color32::from_rgb(1, 2, 3);
        let v = painted(|p, t| {
            context::reset_for_frame();
            context::scope(Inherited::default().color(ancestor), || {
                KvRow::new("GAP", "1%").value_color(mine).show(p, t, rect());
            });
        });
        assert_eq!(v[0].1, ancestor, "label still inherits");
        assert_eq!(v[1].1, mine, "value states its own");
    }

    /// `label_w` pins the left column so a STACK of rows aligns.
    #[test]
    fn a_fixed_label_column_does_not_move_with_its_text() {
        let a = painted(|p, t| {
            context::reset_for_frame();
            KvRow::new("A", "1").label_w(64.0).show(p, t, rect());
        });
        let b = painted(|p, t| {
            context::reset_for_frame();
            KvRow::new("A MUCH LONGER LABEL", "1").label_w(64.0).show(p, t, rect());
        });
        assert!(
            (a[1].0 - b[1].0).abs() < 0.01,
            "a pinned label column must not move the value: {a:?} vs {b:?}"
        );
    }
}
