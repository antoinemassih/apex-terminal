//! Flexbox layout for egui, backed by [Taffy].
//!
//! ## Why this exists
//!
//! egui gives you `Layout` (left-to-right / top-down) and `columns()`. Anything
//! beyond that — "this child grows, that one is intrinsic, space-between the
//! rest, wrap when narrow, all with a consistent gutter" — has historically
//! been written as arithmetic at the call site:
//!
//! ```ignore
//! let x = rect.left() + 16.0;
//! let w = (rect.width() - 16.0 * 2.0 - 8.0) * 0.5;
//! painter.text(egui::pos2(x, cy - galley.height() * 0.5), ..);
//! ```
//!
//! Every one of those numbers is a place alignment can drift by a pixel, and
//! the 2026-07-31 UI audit found the app is full of them. Flexbox solves this
//! class of problem outright, and Taffy is the production Rust implementation
//! of it (the same engine behind Bevy, Dioxus and Zed).
//!
//! ## What this does NOT do
//!
//! **It does not style anything.** Taffy computes rectangles; that is all.
//! Colours, fonts, radii, strokes, elevation and per-style treatment continue
//! to come from the existing design system (`ui_kit::style` tokens, the
//! `Theme`, and the `StyleSystem` per-style knobs). The intended usage is:
//!
//! ```ignore
//! Flex::row()
//!     .gap(gap_sm())                       // <- OUR spacing token
//!     .padding(gap_md())                   // <- OUR spacing token
//!     .align(Align::Center)
//!     .item(Item::fixed(80.0))
//!     .item(Item::grow(1.0))
//!     .item(Item::auto())
//!     .show(ui, |idx, ui| {
//!         // ordinary egui, ordinary design-system widgets
//!     });
//! ```
//!
//! The gaps and padding you hand it are token values, so a style change still
//! flows through exactly as before — Taffy just stops you hand-computing where
//! the boxes land.
//!
//! ## Scope
//!
//! Intended for **panel chrome, forms, headers, toolbars, cards** — places with
//! a handful of children and real alignment requirements.
//!
//! Deliberately NOT intended for the chart panes or hot streaming rows
//! (watchlist, DOM, tape). Those paint at computed pixel positions for a
//! reason: they render hundreds of items per frame and want zero per-frame
//! solve. Running a layout tree there would trade a real performance budget for
//! tidiness. Use `RowShell`/painter geometry there, as today.
//!
//! ## Cost
//!
//! One `TaffyTree` is built and solved per `show()` call. That is fine for a
//! header with 5 children; it is not fine inside a 200-row loop. The
//! `debug_assert` in [`Flex::show`] will complain in dev builds if a single
//! layout exceeds [`MAX_REASONABLE_ITEMS`] children, which almost always means
//! it is being used somewhere it shouldn't be.

use egui::{Rect, Ui, Vec2};
use taffy::prelude::*;

/// Above this many children, a flex layout is probably the wrong tool (see the
/// module docs on hot paths). Dev-build assertion only.
pub const MAX_REASONABLE_ITEMS: usize = 64;

/// Cross-axis alignment (CSS `align-items`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Align {
    Start,
    #[default]
    Stretch,
    Center,
    End,
    /// Align children on their text baseline.
    Baseline,
}

/// Main-axis distribution (CSS `justify-content`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Justify {
    #[default]
    Start,
    Center,
    End,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
}

/// How one child is sized along the main axis.
#[derive(Clone, Copy, Debug)]
pub enum Size {
    /// Exactly this many pixels.
    Fixed(f32),
    /// A fraction of the container's main-axis size (0.0..=1.0).
    Percent(f32),
    /// CSS `flex-basis: auto` with no intrinsic size.
    ///
    /// ⚠ This does **not** size to content. A Taffy leaf has no measure
    /// function here, so an `Auto` item resolves to **zero** on the main axis
    /// and its child renders into an empty, clipped rect. (An earlier version
    /// of this doc claimed egui measures it; it does not — the migration of
    /// the panel headers proved otherwise, and `auto_item_resolves_to_zero`
    /// pins the real behaviour.)
    ///
    /// For content-sized pieces (a title, a value, a count chip) measure with
    /// egui and pass the result: `Item::fixed(galley.size().x.ceil())`. Ceil,
    /// because a fractionally-narrow rect clips the last glyph.
    Auto,
    /// Take a share of the leftover space (CSS `flex-grow`).
    Grow(f32),
    /// M4.1: **content-sized** — a real intrinsic size, measured by the caller
    /// through egui and carried as a definite basis.
    ///
    /// This is the fix for the layout audit's #1 adoption blocker. `Auto`
    /// resolves to ZERO (Taffy leaves have no measure function), so every
    /// content-sized child had to be hand-measured and passed as
    /// `Item::fixed(...)` — which made migrating a header cost MORE code than
    /// the arithmetic it replaced, and is why adoption stalled at 10 sites.
    ///
    /// Prefer the ergonomic constructors that do the measuring for you:
    /// [`Item::text`], [`Item::galley`], [`Item::content`].
    Content(f32),
}

/// One child in a [`Flex`].
#[derive(Clone, Copy, Debug)]
pub struct Item {
    size: Size,
    /// Optional cross-axis size (height in a row, width in a column).
    cross: Option<f32>,
    /// Minimum main-axis size — prevents a `Grow` child collapsing to nothing.
    min: Option<f32>,
    /// Per-item cross-axis alignment override (CSS `align-self`).
    align_self: Option<Align>,
    /// CSS `flex-shrink` override. `Fixed`/`Percent` children default to not
    /// shrinking; opt one in when it must yield rather than push its siblings
    /// out of the container (a long title vs. a right-anchored close button).
    shrink: Option<f32>,
    /// Extra leading gutter for this child only (CSS `margin-inline-start`),
    /// stacked on top of the container `gap`. Lets one seam in a strip use a
    /// different spacing token without abandoning the uniform gutter.
    margin_start: Option<f32>,
}

impl Item {
    pub fn fixed(px: f32) -> Self { Self::new(Size::Fixed(px)) }
    pub fn percent(f: f32) -> Self { Self::new(Size::Percent(f)) }
    pub fn auto() -> Self { Self::new(Size::Auto) }
    pub fn grow(factor: f32) -> Self { Self::new(Size::Grow(factor)) }

    // ── M4.1: content-sized constructors (the adoption unblock) ─────────────

    /// Content-sized from an explicit measurement (px). Use when you already
    /// have a size — e.g. an icon's fixed glyph box or a cached galley width.
    pub fn content(px: f32) -> Self { Self::new(Size::Content(px.max(0.0))) }

    /// Content-sized from an egui galley — the exact width egui will paint,
    /// ceiled so a fractionally-narrow rect cannot clip the last glyph.
    pub fn galley(g: &egui::Galley) -> Self {
        Self::content(g.size().x.ceil())
    }

    /// Content-sized by laying `text` out in `ui`'s current style — the
    /// one-liner that replaces the measure-then-`Item::fixed` dance:
    ///
    /// ```ignore
    /// // before (why adoption stalled):
    /// let galley = ui.painter().layout_no_wrap(title.into(), font.clone(), color);
    /// Item::fixed(galley.size().x.ceil())
    /// // after:
    /// Item::text(ui, title, font.clone())
    /// ```
    pub fn text(ui: &egui::Ui, text: impl Into<String>, font: egui::FontId) -> Self {
        let galley = ui.painter().layout_no_wrap(
            text.into(), font, egui::Color32::PLACEHOLDER,
        );
        Self::galley(&galley)
    }

    /// Content-sized from a semantic [`TextStyle`] tier — the cascade-aware
    /// form (per-style sizes, subtree overrides) of [`Item::text`].
    pub fn text_tier(
        ui: &egui::Ui,
        text: impl Into<String>,
        tier: crate::ui_kit::text_style::TextStyle,
    ) -> Self {
        Self::text(ui, text, tier.font_id_in(ui))
    }

    fn new(size: Size) -> Self {
        Self { size, cross: None, min: None, align_self: None, shrink: None, margin_start: None }
    }

    /// Fix the cross-axis extent (row: height, column: width).
    pub fn cross(mut self, px: f32) -> Self { self.cross = Some(px); self }
    /// Floor the main-axis size. Use on `grow` children that must stay legible.
    pub fn min(mut self, px: f32) -> Self { self.min = Some(px); self }
    /// Override the container's `align` for this child only.
    pub fn align_self(mut self, a: Align) -> Self { self.align_self = Some(a); self }
    /// CSS `flex-shrink`. Set `1.0` on an intrinsically-sized child that should
    /// give up width when the container is too narrow, instead of overflowing
    /// and shoving the children after it out of the box.
    pub fn shrink(mut self, factor: f32) -> Self { self.shrink = Some(factor); self }

    /// Extra leading gutter for this child only (CSS `margin-inline-start`):
    /// `margin-left` in a row, `margin-top` in a column. Stacks on top of the
    /// container's `gap`. Use it when one seam in a strip is a different
    /// spacing token from the rest — pass the TOKEN, not a literal.
    pub fn margin_start(mut self, px: f32) -> Self { self.margin_start = Some(px); self }
}

/// Inner padding in sub-pixel-accurate f32. `egui::Margin` is `i8`, which
/// silently truncates spacing tokens that aren't whole pixels — and panel
/// chrome is exactly where a lost half-pixel shows up as a drifting gutter.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Pad {
    pub left: f32,
    pub right: f32,
    pub top: f32,
    pub bottom: f32,
}

/// A flexbox container. Build it, add [`Item`]s, then [`Flex::show`].
#[must_use = "Flex does nothing until `.show(...)` is called"]
pub struct Flex {
    row: bool,
    gap: f32,
    pad: Pad,
    align: Align,
    justify: Justify,
    wrap: bool,
    items: Vec<Item>,
}

impl Flex {
    /// Horizontal container (CSS `flex-direction: row`).
    pub fn row() -> Self { Self::new(true) }
    /// Vertical container (CSS `flex-direction: column`).
    pub fn column() -> Self { Self::new(false) }

    fn new(row: bool) -> Self {
        Self {
            row,
            gap: 0.0,
            pad: Pad::default(),
            align: Align::default(),
            justify: Justify::default(),
            wrap: false,
            items: Vec::new(),
        }
    }

    /// Gutter between children. Pass a spacing TOKEN (`gap_xs()`, `gap_sm()`, …)
    /// so the rhythm still follows the design system.
    pub fn gap(mut self, px: f32) -> Self { self.gap = px; self }

    /// Uniform inner padding. Pass a spacing token.
    pub fn padding(mut self, px: f32) -> Self {
        self.pad = Pad { left: px, right: px, top: px, bottom: px };
        self
    }

    /// Non-uniform inner padding.
    pub fn padding_margin(mut self, m: egui::Margin) -> Self {
        self.pad = Pad {
            left: m.left as f32,
            right: m.right as f32,
            top: m.top as f32,
            bottom: m.bottom as f32,
        };
        self
    }

    /// Non-uniform inner padding in f32 — use this (not [`Self::padding_margin`])
    /// when the insets come straight from spacing tokens, so a fractional token
    /// isn't truncated to whole pixels.
    pub fn padding_sides(mut self, left: f32, right: f32, top: f32, bottom: f32) -> Self {
        self.pad = Pad { left, right, top, bottom };
        self
    }

    pub fn align(mut self, a: Align) -> Self { self.align = a; self }
    pub fn justify(mut self, j: Justify) -> Self { self.justify = j; self }
    /// Allow children to wrap onto additional lines when they don't fit.
    pub fn wrap(mut self, on: bool) -> Self { self.wrap = on; self }

    pub fn item(mut self, i: Item) -> Self { self.items.push(i); self }
    pub fn items(mut self, it: impl IntoIterator<Item = Item>) -> Self {
        self.items.extend(it);
        self
    }

    /// Solve the layout inside `ui`'s available width and render each child.
    ///
    /// The closure is called once per item with its index and a child `Ui`
    /// clipped to that item's solved rect. Style inheritance still works:
    /// the child `Ui` inherits the parent's `Style`, so the type cascade and
    /// theme reach it normally.
    pub fn show<R>(
        self,
        ui: &mut Ui,
        mut add_item: impl FnMut(usize, &mut Ui) -> R,
    ) -> Vec<R> {
        let avail = ui.available_size_before_wrap();
        let rects = self.solve(Vec2::new(avail.x, avail.y));

        let origin = ui.cursor().min;
        let mut out = Vec::with_capacity(rects.len());

        // Reserve the space the solved layout actually occupies so subsequent
        // egui widgets flow after it.
        let used = rects
            .iter()
            .fold(Rect::NOTHING, |acc, r| acc.union(*r))
            .translate(origin.to_vec2());

        for (idx, r) in rects.iter().enumerate() {
            let rect = r.translate(origin.to_vec2());
            let mut child = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(rect)
                    .layout(egui::Layout::top_down(egui::Align::Min)),
            );
            child.set_clip_rect(rect.intersect(ui.clip_rect()));
            out.push(add_item(idx, &mut child));
        }

        if used.is_positive() {
            ui.allocate_rect(used, egui::Sense::hover());
        }
        out
    }

    /// Solve the layout and return each child's rect **relative to the
    /// container origin**. Exposed (and pure) so layout can be unit-tested
    /// headlessly — no GPU, no egui context, no window.
    pub fn solve(&self, available: Vec2) -> Vec<Rect> {
        debug_assert!(
            self.items.len() <= MAX_REASONABLE_ITEMS,
            "Flex with {} children — flexbox is for panel chrome/forms, not hot \
             row rendering; see the module docs",
            self.items.len()
        );
        if self.items.is_empty() {
            return Vec::new();
        }

        let mut tree: TaffyTree<()> = TaffyTree::new();
        // Taffy rounds solved geometry to whole pixels by default (a browser
        // habit — snap boxes to the device grid). egui does not: the geometry
        // this engine replaces was exact f32, glyphs are laid out at fractional
        // x, and spacing tokens scale by 0.75/1.15/1.25. Rounding would quantise
        // every migrated header to the nearest pixel and shift it by up to 0.5px
        // against the design it is meant to reproduce, so solve in exact f32.
        tree.disable_rounding();

        let children: Vec<NodeId> = self
            .items
            .iter()
            .map(|it| {
                let mut st = Style::default();
                match it.size {
                    Size::Fixed(px) => {
                        if self.row { st.size.width = length(px); }
                        else { st.size.height = length(px); }
                        st.flex_grow = 0.0;
                        st.flex_shrink = 0.0;
                    }
                    Size::Percent(f) => {
                        if self.row { st.size.width = percent(f); }
                        else { st.size.height = percent(f); }
                        st.flex_grow = 0.0;
                    }
                    Size::Auto => {
                        st.flex_grow = 0.0;
                        st.flex_basis = auto();
                    }
                    Size::Content(px) => {
                        // A measured intrinsic size behaves like CSS
                        // `flex-basis: <px>` with `flex-grow: 0`: it holds its
                        // content width, but MAY shrink when the container is
                        // over-subscribed (unlike Fixed, which never does).
                        if self.row { st.flex_basis = length(px); }
                        else { st.flex_basis = length(px); }
                        st.flex_grow = 0.0;
                        st.flex_shrink = 1.0;
                    }
                    Size::Grow(g) => {
                        st.flex_grow = g;
                        // basis 0 so grow factors split the WHOLE main axis,
                        // which is what callers expect from `grow(1)+grow(1)`.
                        st.flex_basis = length(0.0);
                    }
                }
                if let Some(c) = it.cross {
                    if self.row { st.size.height = length(c); }
                    else { st.size.width = length(c); }
                }
                if let Some(m) = it.min {
                    if self.row { st.min_size.width = length(m); }
                    else { st.min_size.height = length(m); }
                }
                if let Some(a) = it.align_self {
                    st.align_self = Some(to_align_items(a));
                }
                if let Some(s) = it.shrink {
                    st.flex_shrink = s;
                }
                if let Some(m) = it.margin_start {
                    if self.row { st.margin.left = length(m); }
                    else { st.margin.top = length(m); }
                }
                tree.new_leaf(st).expect("taffy leaf")
            })
            .collect();

        let root_style = Style {
            display: Display::Flex,
            flex_direction: if self.row { FlexDirection::Row } else { FlexDirection::Column },
            flex_wrap: if self.wrap { FlexWrap::Wrap } else { FlexWrap::NoWrap },
            align_items: Some(to_align_items(self.align)),
            justify_content: Some(to_justify(self.justify)),
            gap: taffy::geometry::Size {
                width: length(self.gap),
                height: length(self.gap),
            },
            padding: taffy::geometry::Rect {
                left: length(self.pad.left),
                right: length(self.pad.right),
                top: length(self.pad.top),
                bottom: length(self.pad.bottom),
            },
            size: taffy::geometry::Size {
                width: length(available.x.max(0.0)),
                height: if available.y.is_finite() && available.y > 0.0 {
                    length(available.y)
                } else {
                    auto()
                },
            },
            ..Default::default()
        };

        let root = tree.new_with_children(root_style, &children).expect("taffy root");
        tree.compute_layout(
            root,
            taffy::geometry::Size {
                width: AvailableSpace::Definite(available.x.max(0.0)),
                height: if available.y.is_finite() && available.y > 0.0 {
                    AvailableSpace::Definite(available.y)
                } else {
                    AvailableSpace::MaxContent
                },
            },
        )
        .expect("taffy layout");

        children
            .iter()
            .map(|c| {
                let l = tree.layout(*c).expect("taffy child layout");
                Rect::from_min_size(
                    egui::pos2(l.location.x, l.location.y),
                    egui::vec2(l.size.width, l.size.height),
                )
            })
            .collect()
    }
}

// taffy 0.12 models alignment as structs with associated constants (keyword +
// safety), not enums, and `JustifyContent` is an alias of `AlignContent`.
fn to_align_items(a: Align) -> AlignItems {
    match a {
        Align::Start => AlignItems::FLEX_START,
        Align::Stretch => AlignItems::STRETCH,
        Align::Center => AlignItems::CENTER,
        Align::End => AlignItems::FLEX_END,
        Align::Baseline => AlignItems::BASELINE,
    }
}

fn to_justify(j: Justify) -> JustifyContent {
    match j {
        Justify::Start => JustifyContent::FLEX_START,
        Justify::Center => JustifyContent::CENTER,
        Justify::End => JustifyContent::FLEX_END,
        Justify::SpaceBetween => JustifyContent::SPACE_BETWEEN,
        Justify::SpaceAround => JustifyContent::SPACE_AROUND,
        Justify::SpaceEvenly => JustifyContent::SPACE_EVENLY,
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────
//
// `solve()` is pure geometry, so layout is verifiable headlessly — no GPU, no
// window, no screenshot. This is the part of UI work that was previously
// impossible to test and had to be eyeballed.

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f32, b: f32) -> bool { (a - b).abs() < 0.01 }

    #[test]
    fn row_two_equal_grow_children_split_the_width() {
        let rects = Flex::row()
            .item(Item::grow(1.0))
            .item(Item::grow(1.0))
            .solve(Vec2::new(200.0, 40.0));
        assert_eq!(rects.len(), 2);
        assert!(approx(rects[0].width(), 100.0), "got {}", rects[0].width());
        assert!(approx(rects[1].width(), 100.0), "got {}", rects[1].width());
        assert!(approx(rects[1].left(), 100.0));
    }

    #[test]
    fn gap_is_subtracted_from_the_growable_space() {
        let rects = Flex::row()
            .gap(20.0)
            .item(Item::grow(1.0))
            .item(Item::grow(1.0))
            .solve(Vec2::new(220.0, 40.0));
        assert!(approx(rects[0].width(), 100.0), "got {}", rects[0].width());
        assert!(approx(rects[1].left(), 120.0), "got {}", rects[1].left());
    }

    #[test]
    fn padding_insets_all_children() {
        let rects = Flex::row()
            .padding(10.0)
            .item(Item::grow(1.0))
            .solve(Vec2::new(200.0, 50.0));
        assert!(approx(rects[0].left(), 10.0), "got {}", rects[0].left());
        assert!(approx(rects[0].width(), 180.0), "got {}", rects[0].width());
    }

    #[test]
    fn fixed_child_keeps_its_size_and_grow_takes_the_rest() {
        let rects = Flex::row()
            .item(Item::fixed(60.0))
            .item(Item::grow(1.0))
            .solve(Vec2::new(200.0, 40.0));
        assert!(approx(rects[0].width(), 60.0), "got {}", rects[0].width());
        assert!(approx(rects[1].width(), 140.0), "got {}", rects[1].width());
    }

    #[test]
    fn grow_factors_split_proportionally() {
        let rects = Flex::row()
            .item(Item::grow(1.0))
            .item(Item::grow(3.0))
            .solve(Vec2::new(200.0, 40.0));
        assert!(approx(rects[0].width(), 50.0), "got {}", rects[0].width());
        assert!(approx(rects[1].width(), 150.0), "got {}", rects[1].width());
    }

    #[test]
    fn min_width_prevents_a_grow_child_collapsing() {
        let rects = Flex::row()
            .item(Item::fixed(180.0))
            .item(Item::grow(1.0).min(40.0))
            .solve(Vec2::new(200.0, 40.0));
        assert!(rects[1].width() >= 40.0 - 0.01, "got {}", rects[1].width());
    }

    #[test]
    fn space_between_pushes_children_to_the_edges() {
        let rects = Flex::row()
            .justify(Justify::SpaceBetween)
            .item(Item::fixed(40.0))
            .item(Item::fixed(40.0))
            .solve(Vec2::new(200.0, 40.0));
        assert!(approx(rects[0].left(), 0.0), "got {}", rects[0].left());
        assert!(approx(rects[1].right(), 200.0), "got {}", rects[1].right());
    }

    #[test]
    fn column_stacks_vertically_with_gap() {
        let rects = Flex::column()
            .gap(8.0)
            .item(Item::fixed(20.0))
            .item(Item::fixed(20.0))
            .solve(Vec2::new(100.0, 200.0));
        assert!(approx(rects[0].top(), 0.0));
        assert!(approx(rects[1].top(), 28.0), "got {}", rects[1].top());
    }

    #[test]
    fn stretch_makes_children_fill_the_cross_axis() {
        let rects = Flex::row()
            .align(Align::Stretch)
            .item(Item::fixed(50.0))
            .solve(Vec2::new(200.0, 40.0));
        assert!(approx(rects[0].height(), 40.0), "got {}", rects[0].height());
    }

    #[test]
    fn per_item_cross_size_overrides_stretch() {
        let rects = Flex::row()
            .align(Align::Stretch)
            .item(Item::fixed(50.0).cross(16.0))
            .solve(Vec2::new(200.0, 40.0));
        assert!(approx(rects[0].height(), 16.0), "got {}", rects[0].height());
    }

    #[test]
    fn empty_layout_is_not_an_error() {
        assert!(Flex::row().solve(Vec2::new(100.0, 20.0)).is_empty());
    }

    /// Zero/negative available width must not panic — panels get laid out at
    /// degenerate sizes during drags and first frames.
    #[test]
    fn degenerate_available_size_does_not_panic() {
        let r = Flex::row().item(Item::grow(1.0)).solve(Vec2::new(0.0, 0.0));
        assert_eq!(r.len(), 1);
        let r2 = Flex::row().item(Item::grow(1.0)).solve(Vec2::new(-5.0, -5.0));
        assert_eq!(r2.len(), 1);
    }

    /// The classic panel-header shape: icon | title (grows) | actions.
    #[test]
    fn realistic_panel_header_row() {
        let rects = Flex::row()
            .gap(8.0)
            .padding(12.0)
            .align(Align::Center)
            .item(Item::fixed(16.0))
            .item(Item::grow(1.0))
            .item(Item::fixed(24.0))
            .solve(Vec2::new(300.0, 32.0));
        assert!(approx(rects[0].left(), 12.0));
        assert!(approx(rects[1].left(), 36.0), "got {}", rects[1].left());
        // 300 - 24 padding - 16 - 24 icons - 16 (two gaps) = 220
        assert!(approx(rects[1].width(), 220.0), "got {}", rects[1].width());
        assert!(approx(rects[2].right(), 288.0), "got {}", rects[2].right());
    }

    /// f32 padding must not be truncated to whole pixels. `egui::Margin` is
    /// `i8`; spacing tokens are `f32` and a 0.5px loss is exactly the kind of
    /// gutter drift this engine exists to remove.
    #[test]
    fn fractional_padding_is_not_truncated() {
        let rects = Flex::row()
            .padding_sides(7.5, 7.5, 0.0, 0.0)
            .item(Item::grow(1.0))
            .solve(Vec2::new(200.0, 30.0));
        assert!(approx(rects[0].left(), 7.5), "got {}", rects[0].left());
        assert!(approx(rects[0].right(), 192.5), "got {}", rects[0].right());
    }

    /// A `fixed` child does not shrink by default, so an oversized one pushes
    /// its siblings out of the container…
    #[test]
    fn fixed_children_do_not_shrink_by_default() {
        let rects = Flex::row()
            .item(Item::fixed(300.0))
            .item(Item::fixed(20.0))
            .solve(Vec2::new(200.0, 30.0));
        assert!(rects[1].right() > 200.0, "got {}", rects[1].right());
    }

    /// …unless it opts into `flex-shrink`, which is how a long panel title
    /// yields instead of shoving the close button off the header.
    #[test]
    fn shrinkable_child_yields_instead_of_overflowing() {
        let rects = Flex::row()
            .item(Item::fixed(300.0).shrink(1.0))
            .item(Item::fixed(20.0))
            .solve(Vec2::new(200.0, 30.0));
        assert!(approx(rects[1].right(), 200.0), "got {}", rects[1].right());
        assert!(rects[0].width() <= 180.0 + 0.01, "got {}", rects[0].width());
    }

    /// `margin_start` widens ONE seam without changing the container gutter —
    /// the panel-header case where icon→title is `gap_sm` but title→actions is
    /// `gap_md`.
    #[test]
    fn margin_start_widens_a_single_seam() {
        let rects = Flex::row()
            .gap(8.0)
            .item(Item::fixed(20.0))
            .item(Item::fixed(20.0))
            .item(Item::fixed(20.0).margin_start(4.0))
            .solve(Vec2::new(300.0, 30.0));
        // seam 1: plain gap
        assert!(approx(rects[1].left(), 28.0), "got {}", rects[1].left());
        // seam 2: gap + margin_start
        assert!(approx(rects[2].left(), 60.0), "got {}", rects[2].left());
    }

    /// `Item::auto()` sizes to CONTENT — but `solve()` is headless and leaves
    /// have no measure function, so an auto child resolves to zero. Callers
    /// that need intrinsic width must measure the galley themselves and pass
    /// `Item::fixed(measured)`. This test pins that contract so nobody
    /// "migrates" a title to `auto()` and watches it vanish.
    #[test]
    fn auto_children_have_no_intrinsic_size_when_solved_headlessly() {
        let rects = Flex::row()
            .item(Item::auto())
            .item(Item::grow(1.0))
            .solve(Vec2::new(200.0, 30.0));
        assert!(approx(rects[0].width(), 0.0), "got {}", rects[0].width());
        assert!(approx(rects[1].width(), 200.0), "got {}", rects[1].width());
    }
}

// ── M4.1 intrinsic-sizing tests ──────────────────────────────────────────────
#[cfg(test)]
mod m41_content_sizing_tests {
    use super::*;

    /// The audit's #1 layout blocker, pinned: `Auto` resolves to ZERO, which is
    /// why every content-sized child had to be hand-measured and why adoption
    /// stalled at 10 call sites. (This mirrors the existing
    /// `auto_children_have_no_intrinsic_size_when_solved_headlessly` test —
    /// kept here so the contrast with `Content` is visible in one place.)
    #[test]
    fn auto_still_resolves_to_zero() {
        let rects = Flex::row()
            .item(Item::auto())
            .item(Item::grow(1.0))
            .solve(egui::vec2(200.0, 20.0));
        assert_eq!(rects[0].width(), 0.0, "Auto has no intrinsic size");
    }

    /// `Content` holds a measured intrinsic width — the fix.
    #[test]
    fn content_holds_its_measured_width() {
        let rects = Flex::row()
            .item(Item::content(80.0))
            .item(Item::grow(1.0))
            .solve(egui::vec2(200.0, 20.0));
        assert!((rects[0].width() - 80.0).abs() < 0.01,
            "Content must keep its measured width, got {}", rects[0].width());
        assert!((rects[1].width() - 120.0).abs() < 0.01,
            "the Grow sibling takes the remainder");
    }

    /// Content sits BETWEEN Fixed and Grow: it may shrink when the container is
    /// over-subscribed (CSS flex-basis semantics), where Fixed never does.
    #[test]
    fn content_shrinks_when_oversubscribed_but_fixed_does_not() {
        let content = Flex::row()
            .item(Item::content(150.0))
            .item(Item::content(150.0))
            .solve(egui::vec2(200.0, 20.0));
        let total: f32 = content.iter().map(|r| r.width()).sum();
        assert!(total <= 200.5, "Content items must shrink to fit, got {total}");

        let fixed = Flex::row()
            .item(Item::fixed(150.0))
            .item(Item::fixed(150.0))
            .solve(egui::vec2(200.0, 20.0));
        assert!((fixed[0].width() - 150.0).abs() < 0.01,
            "Fixed must NOT shrink — that is the distinction");
    }

    /// A row of content-sized items keeps them in order and non-overlapping —
    /// the header/label/value/chip shape these constructors exist for.
    #[test]
    fn content_row_lays_out_in_order() {
        let rects = Flex::row()
            .gap(4.0)
            .item(Item::content(30.0))
            .item(Item::content(50.0))
            .item(Item::grow(1.0))
            .solve(egui::vec2(300.0, 20.0));
        assert!(rects[0].right() <= rects[1].left() + 0.01, "no overlap");
        assert!(rects[1].right() <= rects[2].left() + 0.01, "no overlap");
        assert!(rects[2].width() > 200.0, "grow child takes the rest");
    }
}
