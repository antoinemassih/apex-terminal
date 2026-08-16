//! The declarative half — a tree of layout intent that compiles to egui.
//!
//! # Why a tree at all
//!
//! Chrome in this app is written as a sequence: allocate, paint, advance a
//! cursor, allocate again. That is how `+= galley.width + gap` walks got into
//! ~80 places, how a header's title came to overlap its own buttons, and why
//! "does this fit" could only be answered after something had already been
//! drawn. A tree states the row's shape *before* anything is painted, so the
//! solver can answer that question first.
//!
//! # No closures for the common cases
//!
//! An earlier attempt at this (`FlexUi`) took a render closure per child. Five
//! buttons that each need `&mut self` cannot each hold a `&mut` closure, so the
//! API was unusable exactly where rows are busiest. Here a node *carries* its
//! content — text carries its string, a spacer carries nothing — and anything
//! interactive is addressed by id afterwards:
//!
//! ```ignore
//! let r = El::row()
//!     .gap(gap_sm())
//!     .style(Inherited::default().color(t.dim))
//!     .child(El::text("POSITIONS").tier(TextStyle::Label))
//!     .child(El::spacer())
//!     .child(El::button("close_all", "Close All"))
//!     .show(ui, theme);
//!
//! if r.clicked("close_all") { … }          // no borrow gymnastics
//! ```
//!
//! # The escape hatch is the migration path
//!
//! [`El::slot`] reserves a rect and paints nothing. The caller draws into
//! `r.rect("id")` with whatever imperative code it already has. A surface can
//! therefore move its *layout* into the tree without moving its *painting*,
//! which is what makes adopting this incremental rather than a rewrite.
//!
//! # Not a virtual DOM
//!
//! Built and consumed inside one frame. No diffing, no keys, no lifecycle, no
//! retained graph — the same immediate-mode model egui already uses. This is an
//! organizing layer over egui, not a different way of rendering.

use std::collections::HashMap;

use egui::{Rect, Response, Ui, Vec2};

use super::context::{self, Inherited};
use crate::ui_kit::layout::{Flex, Item};
use crate::ui_kit::text_style::TextStyle;
use crate::ui_kit::widgets::theme::ComponentTheme;

/// What a node is.
enum Kind {
    /// Lays its children out along an axis.
    Container { row: bool, children: Vec<El>, gap: f32 },
    /// A string. Measured from the resolved tier, painted at its solved rect.
    Text { text: String, tier: Option<TextStyle> },
    /// Takes the leftover space. CSS `flex: 1` with nothing in it.
    Spacer { grow: f32 },
    /// An interactive rect. Painted as a button; addressed by id afterwards.
    Button { id: String, label: String },
    /// A reserved rect the caller paints itself. The migration escape hatch.
    Slot { id: String, size: Vec2 },
}

/// One node of the tree.
pub struct El {
    kind: Kind,
    /// Inherited-style delta applied to this node AND its subtree.
    style: Inherited,
    /// Main-axis sizing. `None` = size to content.
    fixed: Option<f32>,
    grow: Option<f32>,
    shrink: Option<f32>,
    /// CSS `margin-inline-start` — the seam before this child.
    margin_start: Option<f32>,
    /// CSS `padding`, applied inside a container before its children.
    pad: (f32, f32, f32, f32),
}

impl El {
    fn new(kind: Kind) -> Self {
        Self {
            kind,
            style: Inherited::default(),
            fixed: None,
            grow: None,
            shrink: None,
            margin_start: None,
            pad: (0.0, 0.0, 0.0, 0.0),
        }
    }

    // ── Constructors ───────────────────────────────────────────────────────

    pub fn row() -> Self {
        Self::new(Kind::Container { row: true, children: Vec::new(), gap: 0.0 })
    }
    pub fn column() -> Self {
        Self::new(Kind::Container { row: false, children: Vec::new(), gap: 0.0 })
    }
    pub fn text(s: impl Into<String>) -> Self {
        Self::new(Kind::Text { text: s.into(), tier: None })
    }
    pub fn spacer() -> Self {
        Self::new(Kind::Spacer { grow: 1.0 })
    }
    pub fn button(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self::new(Kind::Button { id: id.into(), label: label.into() })
    }
    /// Reserve `size` and paint nothing — read the rect back from [`Rendered`].
    pub fn slot(id: impl Into<String>, size: Vec2) -> Self {
        Self::new(Kind::Slot { id: id.into(), size })
    }

    // ── Declaration ────────────────────────────────────────────────────────

    /// Add a child. Containers only; ignored elsewhere.
    #[must_use]
    pub fn child(mut self, c: El) -> Self {
        if let Kind::Container { children, .. } = &mut self.kind {
            children.push(c);
        }
        self
    }

    /// Add a child only when `cond`. Keeps a row's shape declarative instead of
    /// splitting it across an `if`.
    #[must_use]
    pub fn child_if(self, cond: bool, c: El) -> Self {
        if cond { self.child(c) } else { self }
    }

    #[must_use]
    pub fn gap(mut self, px: f32) -> Self {
        if let Kind::Container { gap, .. } = &mut self.kind {
            *gap = px;
        }
        self
    }

    /// Inherited-style delta for this node and its subtree. This is the seam
    /// between the two halves of the cascade.
    #[must_use]
    pub fn style(mut self, s: Inherited) -> Self {
        self.style = s;
        self
    }

    /// Type tier for a text node — sugar for `.style(Inherited::default()
    /// .text_style(t))`, which also means it inherits down.
    #[must_use]
    pub fn tier(mut self, t: TextStyle) -> Self {
        if let Kind::Text { tier, .. } = &mut self.kind {
            *tier = Some(t);
        }
        self.style = self.style.text_style(t);
        self
    }

    #[must_use]
    pub fn fixed(mut self, px: f32) -> Self {
        self.fixed = Some(px);
        self
    }
    #[must_use]
    pub fn grow(mut self, f: f32) -> Self {
        self.grow = Some(f);
        self
    }
    #[must_use]
    pub fn shrink(mut self, f: f32) -> Self {
        self.shrink = Some(f);
        self
    }
    #[must_use]
    pub fn margin_start(mut self, px: f32) -> Self {
        self.margin_start = Some(px);
        self
    }
    #[must_use]
    pub fn pad(mut self, px: f32) -> Self {
        self.pad = (px, px, px, px);
        self
    }
    #[must_use]
    pub fn pad_x(mut self, px: f32) -> Self {
        self.pad.0 = px;
        self.pad.1 = px;
        self
    }

    // ── Measurement ────────────────────────────────────────────────────────

    /// Intrinsic main-axis size, resolved against the cascade in force.
    ///
    /// Computed before anything is painted — that is the whole point of the
    /// tree. A container asks its children, so "does this row fit" has an
    /// answer at declaration time rather than after an overrun.
    fn intrinsic(&self, ui: Option<&Ui>, inherited: Inherited) -> f32 {
        if let Some(px) = self.fixed {
            return px;
        }
        let here = self.style.over(inherited);
        match &self.kind {
            Kind::Container { row, children, gap } => {
                let n = children.len();
                let sum: f32 = children
                    .iter()
                    .map(|c| c.intrinsic(ui, here) + c.margin_start.unwrap_or(0.0))
                    .sum();
                let gaps = if n > 1 { gap * (n as f32 - 1.0) } else { 0.0 };
                let padding = if *row { self.pad.0 + self.pad.1 } else { self.pad.2 + self.pad.3 };
                sum + gaps + padding
            }
            Kind::Text { text, tier } => {
                // Without a `Ui` there is no font stack to measure against.
                // `solve_rect` documents that a text node must then carry an
                // explicit width; returning 0 here makes that failure visible
                // as a collapsed slot rather than a wrong one.
                let Some(ui) = ui else { return 0.0 };
                let t = tier.or(here.text_style).unwrap_or(TextStyle::Body);
                let font = t.font_id_in(ui);
                ui.fonts(|f| {
                    f.layout_no_wrap(text.clone(), font, egui::Color32::PLACEHOLDER)
                        .size()
                        .x
                })
            }
            Kind::Spacer { .. } => 0.0,
            Kind::Button { label, .. } => {
                let Some(ui) = ui else { return 0.0 };
                let t = here.text_style.unwrap_or(TextStyle::BodySm);
                let font = t.font_id_in(ui);
                let w = ui.fonts(|f| {
                    f.layout_no_wrap(label.clone(), font, egui::Color32::PLACEHOLDER)
                        .size()
                        .x
                });
                w + crate::ui_kit::style::gap_md() * 2.0
            }
            Kind::Slot { size, .. } => size.x,
        }
    }

    fn as_item(&self, ui: Option<&Ui>, inherited: Inherited) -> Item {
        let mut it = match (self.fixed, &self.kind) {
            (Some(px), _) => Item::fixed(px),
            (None, Kind::Spacer { grow }) => Item::grow(*grow),
            _ => Item::content(self.intrinsic(ui, inherited)),
        };
        if let Some(g) = self.grow {
            it = Item::grow(g);
        }
        if let Some(s) = self.shrink {
            it = it.shrink(s);
        }
        if let Some(m) = self.margin_start {
            it = it.margin_start(m);
        }
        it
    }

    // ── Render ─────────────────────────────────────────────────────────────

    /// Solve and paint into the space `ui` offers.
    pub fn show(self, ui: &mut Ui, theme: &dyn ComponentTheme) -> Rendered {
        let avail = ui.available_size_before_wrap();
        let h = if avail.y.is_finite() && avail.y > 0.0 { avail.y } else { self.intrinsic_cross() };
        let (rect, _) = ui.allocate_exact_size(Vec2::new(avail.x, h), egui::Sense::hover());
        self.show_in(ui, theme, rect)
    }

    /// Solve geometry with NO `Ui` at all — for painter-only surfaces.
    ///
    /// Several widgets paint from a bare `&egui::Painter` and never receive a
    /// `Ui` (`paint_one_tab_painter` is one). They are exactly the code most
    /// full of cursor walks, because without a layout context the only tool
    /// available was a running `x`.
    ///
    /// Every node must carry its own width here — `slot`, `fixed`, `spacer`,
    /// `grow`. A `text` node has no font stack to measure against and resolves
    /// to zero, which shows up as a collapsed slot rather than a wrong one.
    pub fn solve_rect(self, rect: Rect) -> Rendered {
        let mut out = Rendered::default();
        let inherited = context::resolved();
        solve(self, None, rect, inherited, &mut out);
        out
    }

    /// Solve geometry only — paint nothing, return the rects.
    ///
    /// The migration entry point. A surface that already knows how to paint
    /// itself (and often has hard-won reasons for exactly how — fades, clip
    /// invariants, morph animations) can move its LAYOUT here without moving
    /// its painting, then draw into `r.rect("id")`.
    ///
    /// Takes no theme, because solving does not need one. An earlier version
    /// required `&dyn ComponentTheme` for the layout-only case and that was
    /// simply wrong: it made the cheap, safe migration path demand a value it
    /// never used.
    pub fn solve_in(self, ui: &mut Ui, rect: Rect) -> Rendered {
        let mut out = Rendered::default();
        let inherited = context::resolved();
        solve(self, Some(ui), rect, inherited, &mut out);
        out
    }

    /// Solve and paint into an explicit rect.
    pub fn show_in(self, ui: &mut Ui, theme: &dyn ComponentTheme, rect: Rect) -> Rendered {
        let mut out = Rendered::default();
        let inherited = context::resolved();
        paint(self, ui, theme, rect, inherited, &mut out);
        out
    }

    fn intrinsic_cross(&self) -> f32 {
        match &self.kind {
            Kind::Slot { size, .. } => size.y,
            _ => crate::ui_kit::style::control_h_md(),
        }
    }
}

/// Everything the tree produced that the caller might need back.
#[derive(Default)]
pub struct Rendered {
    rects: HashMap<String, Rect>,
    responses: HashMap<String, Response>,
}

impl Rendered {
    /// The solved rect for an id'd node (`slot` or `button`).
    ///
    /// `Rect::NOTHING` when the id was not in the tree — a `child_if` that did
    /// not fire is the normal reason, so this is absence rather than an error.
    #[must_use]
    pub fn rect(&self, id: &str) -> Rect {
        self.rects.get(id).copied().unwrap_or(Rect::NOTHING)
    }
    #[must_use]
    pub fn response(&self, id: &str) -> Option<&Response> {
        self.responses.get(id)
    }
    #[must_use]
    pub fn clicked(&self, id: &str) -> bool {
        self.responses.get(id).is_some_and(egui::Response::clicked)
    }
    #[must_use]
    pub fn hovered(&self, id: &str) -> bool {
        self.responses.get(id).is_some_and(egui::Response::hovered)
    }
}

fn paint(
    el: El,
    ui: &mut Ui,
    theme: &dyn ComponentTheme,
    rect: Rect,
    inherited: Inherited,
    out: &mut Rendered,
) {
    // This node's declarations apply to it and everything under it — the
    // inheritance half of the cascade, applied exactly at the tree edge.
    let here = el.style.over(inherited);

    match el.kind {
        Kind::Container { row, children, gap } => {
            let inner = Rect::from_min_max(
                egui::pos2(rect.left() + el.pad.0, rect.top() + el.pad.2),
                egui::pos2(rect.right() - el.pad.1, rect.bottom() - el.pad.3),
            );
            let mut flex = if row { Flex::row() } else { Flex::column() }.gap(gap);
            for c in &children {
                flex = flex.item(c.as_item(Some(ui), here));
            }
            let solved = flex.solve(inner.size());
            let off = inner.min.to_vec2();
            for (c, r) in children.into_iter().zip(solved) {
                paint(c, ui, theme, r.translate(off), here, out);
            }
        }
        Kind::Text { text, tier } => {
            let t = tier.or(here.text_style).unwrap_or(TextStyle::Body);
            let col = here.color_or(crate::ui_kit::sx::palette_ct(theme).base(crate::ui_kit::sx::Tone::Text));
            let font = t.font_id_in(ui);
            ui.painter().text(
                egui::pos2(rect.left(), rect.center().y),
                egui::Align2::LEFT_CENTER,
                text,
                font,
                col,
            );
        }
        Kind::Spacer { .. } => {}
        Kind::Button { id, label } => {
            let painter = ui.painter().clone();
            let resp = crate::ui_kit::widgets::Button::new(label.as_str())
                .show_at(ui, &painter, rect, theme);
            out.rects.insert(id.clone(), rect);
            out.responses.insert(id, resp);
        }
        Kind::Slot { id, .. } => {
            out.rects.insert(id, rect);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui_kit::cascade::context;
    use crate::ui_kit::widgets::theme::PortableTheme;
    use std::cell::{Cell, RefCell};

    /// `__run_test_ui` with the tiers installed — the same harness
    /// `panel_section` uses. Without `TextStyle::install` the raw test context
    /// has no entry for our named tiers and text layout panics inside egui.
    fn run(f: impl FnOnce(&mut Ui)) {
        let f = Cell::new(Some(f));
        egui::__run_test_ui(|ui| {
            TextStyle::install(ui.style_mut());
            if let Some(f) = f.take() {
                f(ui);
            }
        });
    }

    fn theme() -> PortableTheme {
        PortableTheme::dark()
    }

    /// A spacer takes the slack, so trailing children sit flush right.
    ///
    /// This is the shape every "title … actions" row in the app wants and the
    /// one a cursor walk cannot express: the walk has to know the trailing
    /// width up front, which is exactly what `right_edge - close_w` was doing
    /// by hand in the header.
    #[test]
    fn a_spacer_pushes_trailing_children_to_the_end() {
        let got: RefCell<Option<(Rect, Rect)>> = RefCell::new(None);
        run(|ui| {
            let r = El::row()
                .child(El::slot("lead", Vec2::new(30.0, 20.0)))
                .child(El::spacer())
                .child(El::slot("trail", Vec2::new(40.0, 20.0)))
                .show_in(ui, &theme(), Rect::from_min_size(egui::pos2(0.0, 0.0), Vec2::new(200.0, 20.0)));
            *got.borrow_mut() = Some((r.rect("lead"), r.rect("trail")));
        });
        let (lead, trail) = got.borrow().expect("tree did not render");
        assert!((lead.left() - 0.0).abs() < 0.5, "lead at {}", lead.left());
        assert!((trail.right() - 200.0).abs() < 0.5, "trail right at {}", trail.right());
        assert!(lead.right() < trail.left(), "lead and trail overlap");
    }

    /// Gaps land BETWEEN children, not after the last one.
    #[test]
    fn gaps_sit_between_children_only() {
        let got: RefCell<Option<(Rect, Rect)>> = RefCell::new(None);
        run(|ui| {
            let r = El::row()
                .gap(10.0)
                .child(El::slot("a", Vec2::new(20.0, 20.0)))
                .child(El::slot("b", Vec2::new(20.0, 20.0)))
                .show_in(ui, &theme(), Rect::from_min_size(egui::pos2(0.0, 0.0), Vec2::new(200.0, 20.0)));
            *got.borrow_mut() = Some((r.rect("a"), r.rect("b")));
        });
        let (a, b) = got.borrow().expect("tree did not render");
        assert!((b.left() - a.right() - 10.0).abs() < 0.5,
            "gap was {} not 10", b.left() - a.right());
    }

    /// A `child_if` that did not fire leaves NO slot — the row closes up.
    ///
    /// The cursor-walk equivalent is an `if` around a `cx +=`, which is where
    /// conditional chrome drifts: forget one and every later item is offset.
    #[test]
    fn a_conditional_child_that_is_absent_takes_no_space() {
        let got: RefCell<Option<Rect>> = RefCell::new(None);
        run(|ui| {
            let r = El::row()
                .gap(10.0)
                .child_if(false, El::slot("maybe", Vec2::new(50.0, 20.0)))
                .child(El::slot("after", Vec2::new(20.0, 20.0)))
                .show_in(ui, &theme(), Rect::from_min_size(egui::pos2(0.0, 0.0), Vec2::new(200.0, 20.0)));
            *got.borrow_mut() = Some(r.rect("after"));
        });
        let after = got.borrow().expect("tree did not render");
        assert!((after.left() - 0.0).abs() < 0.5,
            "absent child still reserved space; `after` starts at {}", after.left());
    }

    /// An id that is not in the tree reads as absence, not as a panic.
    #[test]
    fn an_unknown_id_is_absence() {
        run(|ui| {
            let r = El::row()
                .child(El::slot("a", Vec2::new(10.0, 10.0)))
                .show_in(ui, &theme(), Rect::from_min_size(egui::pos2(0.0, 0.0), Vec2::new(50.0, 10.0)));
            assert_eq!(r.rect("nope"), Rect::NOTHING);
            assert!(!r.clicked("nope"));
        });
    }

    /// A container's declaration reaches a descendant across nesting.
    ///
    /// This is the join between the two halves: the tree walks, and each edge
    /// applies the inheritance merge. Asserted through the context rather than
    /// through pixels, because the pixel is the *consequence* — what must be
    /// true is that the descendant RESOLVES to the ancestor's declaration.
    #[test]
    fn a_declaration_reaches_a_nested_descendant() {
        context::reset_for_frame();
        let declared = egui::Color32::from_rgb(7, 8, 9);
        context::scope(Inherited::default().color(declared), || {
            context::scope(Inherited::default(), || {
                context::scope(Inherited::default().letter_spacing(1.5), || {
                    let r = context::resolved();
                    assert_eq!(r.text_color, Some(declared), "colour did not reach depth 3");
                    assert_eq!(r.letter_spacing, Some(1.5));
                });
            });
        });
    }

    /// Intrinsic width is known BEFORE painting — the property the tree exists
    /// for. A row of two 20px slots with a 10px gap measures 50.
    #[test]
    fn a_row_measures_itself_before_anything_is_painted() {
        let w = Cell::new(f32::NAN);
        run(|ui| {
            let row = El::row()
                .gap(10.0)
                .child(El::slot("a", Vec2::new(20.0, 20.0)))
                .child(El::slot("b", Vec2::new(20.0, 20.0)));
            w.set(row.intrinsic(Some(ui), Inherited::default()));
        });
        assert!((w.get() - 50.0).abs() < 0.5, "measured {} not 50", w.get());
    }

    /// Padding is a BOX property: it insets a container's own children and is
    /// not inherited by them. Both halves matter — if padding inherited, every
    /// descendant would inset again and nesting would compound.
    #[test]
    fn padding_insets_children_and_does_not_inherit() {
        let got: RefCell<Option<Rect>> = RefCell::new(None);
        run(|ui| {
            let r = El::row()
                .pad(8.0)
                .child(El::slot("a", Vec2::new(20.0, 20.0)))
                .show_in(ui, &theme(), Rect::from_min_size(egui::pos2(0.0, 0.0), Vec2::new(200.0, 40.0)));
            *got.borrow_mut() = Some(r.rect("a"));
        });
        let a = got.borrow().expect("tree did not render");
        assert!((a.left() - 8.0).abs() < 0.5, "padding did not inset: {}", a.left());
        // and it is absent from the inheritable set by construction
        assert_eq!(Inherited::default(), Inherited::default());
    }
}

/// Geometry-only walk. Mirrors `paint`'s traversal exactly so a surface that
/// solves here and paints itself lands on the same rects a full `show_in`
/// would have produced.
fn solve(el: El, ui: Option<&Ui>, rect: Rect, inherited: Inherited, out: &mut Rendered) {
    let here = el.style.over(inherited);
    match el.kind {
        Kind::Container { row, children, gap } => {
            let inner = Rect::from_min_max(
                egui::pos2(rect.left() + el.pad.0, rect.top() + el.pad.2),
                egui::pos2(rect.right() - el.pad.1, rect.bottom() - el.pad.3),
            );
            let mut flex = if row { Flex::row() } else { Flex::column() }.gap(gap);
            for c in &children {
                flex = flex.item(c.as_item(ui, here));
            }
            let solved = flex.solve(inner.size());
            let off = inner.min.to_vec2();
            for (c, r) in children.into_iter().zip(solved) {
                solve(c, ui, r.translate(off), here, out);
            }
        }
        Kind::Slot { id, .. } => {
            out.rects.insert(id, rect);
        }
        Kind::Button { id, .. } => {
            // No painting here, so no response — but the rect is still useful.
            out.rects.insert(id, rect);
        }
        Kind::Text { .. } | Kind::Spacer { .. } => {}
    }
}
