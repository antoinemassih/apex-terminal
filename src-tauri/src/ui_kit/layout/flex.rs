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
//! let x = rect.left() + crate::ui_kit::style::gap_lg();
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
pub const MAX_REASONABLE_ITEMS: usize = 128;
// M4.3 feedback: raised 64 -> 128. The cap is a guard against someone routing
// a streaming row list through flex, but `tabs.rs` now solves one item per tab
// (plus the `+` button) and tab count is USER-driven — a 64-tab strip would
// have tripped the dev-build assert on legitimate use. 128 still catches the
// real mistake (a 200-row/frame tape) with room for any hand-built strip.

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
    /// CSS `flex: <grow> 1 <basis>` — a starting width that then takes a share
    /// of whatever is left over.
    ///
    /// [`Size::Grow`] pins `flex-basis` to 0 so that `grow(1) + grow(1)` splits
    /// the WHOLE axis, which is what those callers want. That makes it unable
    /// to express "start at 80 px, then take a third of the slack" — the model
    /// hand-written column layouts actually use, with a `min_width` basis and a
    /// `weight` share. `panel_list_row` had reimplemented exactly that by hand.
    Flex { grow: f32, basis: f32 },
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

/// M4.6: how a child's CROSS-axis size is derived from its MAIN-axis size —
/// the measure hook.
///
/// The chrome migration named this the single biggest remaining layout gap:
/// "any widget whose text wraps at a flex-derived width is circular — you need
/// the solve to know the wrap width, and the wrapped height to solve the cross
/// axis." `alert.rs` escaped only because every child aligns to `Start` (so a
/// height-0 solve suffices); `toggle_row.rs` (label + wrapping description,
/// vertically centred, ~20x per settings panel) did not, and was left behind.
///
/// `Measure` closes it by running the solve in TWO passes: pass 1 resolves the
/// main axis (widths), the callback then measures the real wrapped height at
/// that width, and pass 2 re-solves with those heights as definite cross sizes.
/// That is exactly what Taffy's `MeasureFunc` does internally, expressed in a
/// form egui can serve — the caller lays out a galley and returns its height.
#[derive(Clone)]
pub struct Measure(std::sync::Arc<dyn Fn(f32) -> f32 + Send + Sync>);

impl Measure {
    /// Build a measure hook: given the solved main-axis size, return the
    /// required cross-axis size.
    ///
    /// ```ignore
    /// // A wrapping description column, measured with egui's real layout:
    /// let font = TextStyle::BodySm.font_id_in(ui);
    /// let painter = ui.painter().clone();
    /// let text = desc.to_owned();
    /// Item::grow(1.0).measure(Measure::new(move |w| {
    ///     painter.layout(text.clone(), font.clone(), Color32::PLACEHOLDER, w)
    ///            .size().y.ceil()
    /// }))
    /// ```
    pub fn new(f: impl Fn(f32) -> f32 + Send + Sync + 'static) -> Self {
        Self(std::sync::Arc::new(f))
    }

    #[inline]
    fn call(&self, main: f32) -> f32 { (self.0)(main.max(0.0)).max(0.0) }
}

impl std::fmt::Debug for Measure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Measure(..)")
    }
}

/// One child in a [`Flex`].
// NOTE: `Clone` not `Copy` since M4.6 — an optional `Measure` holds an `Arc`.
// Items are built per frame in small numbers, so the clone is immaterial.
#[derive(Clone, Debug)]
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
    /// M4.6: cross-axis size derived from the solved main-axis size.
    measure: Option<Measure>,
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
    /// CSS `flex: <grow> 1 <basis>` — see [`Size::Flex`].
    pub fn flex(grow: f32, basis: f32) -> Self {
        Self::new(Size::Flex { grow: grow.max(0.0), basis: basis.max(0.0) })
    }

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
        Self { size, cross: None, min: None, align_self: None, shrink: None, margin_start: None, measure: None }
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

    /// M4.3 feedback: a measured item that must NOT yield.
    ///
    /// The measuring constructors (`content`/`galley`/`text`/`text_tier`) are
    /// shrinkable by CSS `flex-basis` semantics, but a hand-written cursor walk
    /// overflows rather than yielding — so faithfully migrating one required
    /// `.shrink(0.0)` on every measured item, which reads like an incantation.
    /// The chrome agent hit this at 4 of 15 sites. `.rigid()` says it plainly.
    pub fn rigid(self) -> Self { self.shrink(0.0) }

    /// M4.6: attach a measure hook — the child's cross size is computed from
    /// its solved main size (see [`Measure`]). Triggers a two-pass solve.
    pub fn measure(mut self, m: Measure) -> Self { self.measure = Some(m); self }

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
// `Clone` since M4.6: the two-pass measure solve clones the spec to inject
// measured cross sizes before re-solving.
#[derive(Clone)]
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
    /// M4.6: true when any child carries a measure hook (two-pass solve).
    fn needs_measure_pass(&self) -> bool {
        self.items.iter().any(|i| i.measure.is_some())
    }

    pub fn solve(&self, available: Vec2) -> Vec<Rect> {
        if !self.needs_measure_pass() {
            return self.solve_once(available);
        }
        // ── Pass 1: resolve the MAIN axis only. ──────────────────────────────
        let first = self.solve_once(available);
        // ── Measure each hooked child at its solved main size. ───────────────
        let mut measured: Flex = (*self).clone();
        for (i, item) in measured.items.iter_mut().enumerate() {
            if let Some(m) = item.measure.clone() {
                let main = if self.row { first[i].width() } else { first[i].height() };
                // The measured result becomes a DEFINITE cross size, which is
                // what pass 2 needs to place and size the child correctly.
                item.cross = Some(m.call(main));
            }
        }
        // ── Pass 2: re-solve with definite cross sizes. ──────────────────────
        measured.solve_once(available)
    }

    fn solve_once(&self, available: Vec2) -> Vec<Rect> {
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
                    Size::Flex { grow, basis } => {
                        st.flex_grow = grow;
                        st.flex_basis = length(basis);
                        // Shrinkable, so a row of bases wider than the container
                        // degrades proportionally instead of overflowing.
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

// ─── Colocated children (the CSS/JSX shape) ─────────────────────────────────
//
// `Flex::show` hands back an INDEX and expects the caller to match on it:
//
//     let mut f = Flex::row();
//     if let Some(w) = icon_w { f = f.item(Item::fixed(w)); }
//     f = f.item(Item::fixed(title_w)).item(Item::grow(1.0));
//     if closable { f = f.item(Item::fixed(24.0)); }
//     f.show(ui, |idx, ui| match idx { 0 => .., 1 => .., _ => .. });
//
// That is the real reason this engine sat at ~20 call sites while the app kept
// hand-computing rects. The declaration and the content live apart, and every
// conditional item shifts the meaning of every index after it — so `0` means
// "icon" or "title" depending on a branch several lines up. It is the layout
// equivalent of positional arguments, and it is fragile in the way that never
// shows up in review: reorder two rows and the content silently swaps slots
// while every test still passes, because the geometry is still correct.
//
// `child()` attaches the content to the item, which is what JSX does:
//
//     Flex::row().gap(gap_sm()).align(Align::Center)
//         .child_if(icon_w.is_some(), Item::fixed(icon_w.unwrap_or(0.0)),
//                   |ui| draw_icon(ui))
//         .child(Item::fixed(title_w).shrink(1.0), |ui| draw_title(ui))
//         .child(Item::grow(1.0),                  |_| {})
//         .show(ui);
//
// `Flex` itself stays untouched and pure, so `solve()` remains headlessly
// testable — this is a rendering shell over it, not a replacement.

/// A `Flex` whose items each carry their own render closure.
///
/// Built by calling [`Flex::child`] / [`Flex::child_if`]. Cannot be
/// constructed directly, so the item list and the closure list are always the
/// same length by construction.
pub struct FlexUi<'a> {
    flex: Flex,
    children: Vec<Box<dyn FnMut(&mut Ui) + 'a>>,
}

impl Flex {
    /// Attach content to an item, switching to the colocated builder.
    pub fn child<'a>(self, item: Item, render: impl FnMut(&mut Ui) + 'a) -> FlexUi<'a> {
        FlexUi { flex: self, children: Vec::new() }.child(item, render)
    }

    /// Conditional variant — see [`FlexUi::child_if`].
    pub fn child_if<'a>(
        self,
        cond: bool,
        item: Item,
        render: impl FnMut(&mut Ui) + 'a,
    ) -> FlexUi<'a> {
        FlexUi { flex: self, children: Vec::new() }.child_if(cond, item, render)
    }
}

impl<'a> FlexUi<'a> {
    /// Append an item and the content that fills it.
    pub fn child(mut self, item: Item, render: impl FnMut(&mut Ui) + 'a) -> Self {
        self.flex = self.flex.item(item);
        self.children.push(Box::new(render));
        self
    }

    /// Append only when `cond` holds.
    ///
    /// This is the case the index API handled worst: an omitted item shifts
    /// every later index by one, so the caller's `match` has to know which
    /// branches ran. Here the slot and its content disappear together.
    pub fn child_if(self, cond: bool, item: Item, render: impl FnMut(&mut Ui) + 'a) -> Self {
        if cond { self.child(item, render) } else { self }
    }

    /// Append a flexible empty slot — the `<div style="flex:1"/>` spacer that
    /// pushes what follows to the far edge.
    pub fn spacer(self, grow: f32) -> Self {
        self.child(Item::grow(grow), |_| {})
    }

    /// Solve and render. Each child is drawn into its own solved rect.
    pub fn show(self, ui: &mut Ui) {
        let FlexUi { flex, mut children } = self;
        flex.show(ui, |idx, child_ui| {
            if let Some(f) = children.get_mut(idx) {
                f(child_ui);
            }
        });
    }

    /// The solved rects, without rendering — for callers that need geometry
    /// before painting (overlays, hit regions, measurement in tests).
    pub fn solve(&self, available: Vec2) -> Vec<Rect> {
        self.flex.solve(available)
    }

    /// How many children were added. Mostly for tests asserting that a
    /// conditional chain produced the slot count it should.
    pub fn len(&self) -> usize { self.children.len() }

    /// True when no children were added.
    pub fn is_empty(&self) -> bool { self.children.is_empty() }
}

// ─── Named slots (the grid-areas shape) ─────────────────────────────────────
//
// `child()` above solves colocation for callers that RENDER. Callers that only
// want geometry — solve the strip, hand back rects, paint later — have the same
// problem in a worse form, because the sequence ends up written twice:
//
//     // in one function:
//     if icon.is_some() { f = f.item(Item::fixed(w)); }
//     f = f.item(title).item(actions);
//     if closable { f = f.item(close); }
//
//     // in another, matching it by position:
//     let icon  = if icon_w.is_some() { it.next() } else { None };
//     let title = it.next().unwrap_or(Rect::NOTHING);
//     let close = if closable { it.next() } else { None };
//
// Both lists must agree on order AND on which conditions ran. Nothing checks
// that they do; a slot inserted in the builder and not in the reader silently
// shifts every rect after it, and the result is still valid geometry, so tests
// pass and the panel just looks wrong.
//
// Naming the slots removes the coupling — the reader asks for "title" and gets
// the title, whatever ran before it. This is what CSS grid-areas are for.

/// A `Flex` whose items are addressed by KEY rather than position.
///
/// Generic over the key so both idioms in this codebase are served by one API:
/// a `&'static str` reads like a CSS grid-area, and a small `enum` gets the
/// compiler to reject typos. `panel_section` had already invented the second
/// form locally as a `Vec<(Slot, Item)>` resolved back through the index —
/// this is that idea promoted into the layout engine so there is one of it.
///
/// Slots are `Option<K>`: spacing carries no key. `panel_section`'s local
/// version had to invent a `Slot::Gap` variant and repeat it, which put
/// duplicates into what should be a unique namespace and made "is this key
/// already taken?" unanswerable. Spacing is CSS `margin`, not a grid area.
pub struct FlexSlots<K> {
    flex: Flex,
    keys: Vec<Option<K>>,
}

/// Solved [`FlexSlots`] — look slots up by the key they were declared with.
pub struct SolvedSlots<K> {
    keys: Vec<Option<K>>,
    rects: Vec<Rect>,
}

impl Flex {
    /// Begin a keyed-slot layout.
    pub fn slot<K: PartialEq + Clone + std::fmt::Debug>(
        self, key: K, item: Item,
    ) -> FlexSlots<K> {
        FlexSlots { flex: self, keys: Vec::new() }.slot(key, item)
    }
    /// Begin a keyed-slot layout with a conditional first slot.
    pub fn slot_if<K: PartialEq + Clone + std::fmt::Debug>(
        self, key: K, cond: bool, item: Item,
    ) -> FlexSlots<K> {
        FlexSlots { flex: self, keys: Vec::new() }.slot_if(key, cond, item)
    }
}

impl<K: PartialEq + Clone + std::fmt::Debug> FlexSlots<K> {
    /// Append a keyed slot.
    pub fn slot(mut self, key: K, item: Item) -> Self {
        debug_assert!(
            !self.keys.iter().flatten().any(|k| *k == key),
            "duplicate flex slot key {key:?} — lookups would be ambiguous",
        );
        self.flex = self.flex.item(item);
        self.keys.push(Some(key));
        self
    }

    /// Append a keyed slot only when `cond` holds. An absent slot simply has
    /// no rect; nothing after it shifts, because nothing is addressed by index.
    pub fn slot_if(self, key: K, cond: bool, item: Item) -> Self {
        if cond { self.slot(key, item) } else { self }
    }

    /// A fixed anonymous gap — CSS `margin`, not a grid area.
    pub fn pad(mut self, px: f32) -> Self {
        self.flex = self.flex.item(Item::fixed(px));
        self.keys.push(None);
        self
    }

    /// `pad` only when `cond` holds.
    pub fn pad_if(self, cond: bool, px: f32) -> Self {
        if cond { self.pad(px) } else { self }
    }

    /// The elastic middle — `<div style="flex:1"/>`. Pushes what follows to
    /// the far edge and is never looked up.
    pub fn spacer(mut self, grow: f32) -> Self {
        self.flex = self.flex.item(Item::grow(grow));
        self.keys.push(None);
        self
    }

    /// Solve within `rect`, returning rects in ABSOLUTE coordinates.
    pub fn solve_in(&self, rect: Rect) -> SolvedSlots<K> {
        let off = rect.min.to_vec2();
        SolvedSlots {
            keys: self.keys.clone(),
            rects: self.flex.solve(rect.size()).into_iter().map(|r| r.translate(off)).collect(),
        }
    }

    /// Solve relative to the container origin.
    pub fn solve(&self, available: Vec2) -> SolvedSlots<K> {
        SolvedSlots { keys: self.keys.clone(), rects: self.flex.solve(available) }
    }

    /// Render each slot into its solved rect. The closure receives the KEY, so
    /// the caller matches on a name rather than a position. Anonymous spacing
    /// is skipped — it renders nothing by definition.
    pub fn show(self, ui: &mut Ui, mut add: impl FnMut(&K, &mut Ui)) {
        let FlexSlots { flex, keys } = self;
        flex.show(ui, |idx, child_ui| {
            if let Some(Some(k)) = keys.get(idx) { add(k, child_ui); }
        });
    }

    /// Total slots, spacing included.
    pub fn len(&self) -> usize { self.keys.len() }
    /// True when nothing was added.
    pub fn is_empty(&self) -> bool { self.keys.is_empty() }
}

impl<K: PartialEq + std::fmt::Debug> SolvedSlots<K> {
    /// The slot's rect, or `None` when it was not declared this pass.
    pub fn get(&self, key: K) -> Option<Rect> {
        self.keys.iter().position(|k| k.as_ref() == Some(&key)).map(|i| self.rects[i])
    }

    /// The slot's rect, or `Rect::NOTHING` when absent — for callers that
    /// paint unconditionally and treat an empty rect as "draw nothing".
    pub fn rect(&self, key: K) -> Rect {
        self.get(key).unwrap_or(Rect::NOTHING)
    }

    /// A centred square of `size` inside the slot — the common case for icon
    /// and close buttons, whose slot is full-height but whose hit target is
    /// square.
    pub fn square(&self, key: K, size: f32) -> Option<Rect> {
        self.get(key).map(|s| Rect::from_center_size(s.center(), Vec2::splat(size)))
    }

    /// Every solved rect in declaration order, spacing included.
    ///
    /// Keyed lookup is the point of this type, but anonymous spacing has no
    /// key by design — so assertions about the elastic middle ("does the
    /// spacer reach the right edge?") need the raw list.
    pub fn rects(&self) -> &[Rect] { &self.rects }

    /// Number of solved rects, spacing included.
    pub fn len(&self) -> usize { self.rects.len() }
    /// True when no slots were declared.
    pub fn is_empty(&self) -> bool { self.rects.is_empty() }
}

// ─── Tests ──────────────────────────────────────────────────────────────────
//
// `solve()` is pure geometry, so layout is verifiable headlessly — no GPU, no
// window, no screenshot. This is the part of UI work that was previously
// impossible to test and had to be eyeballed.

#[cfg(test)]
mod tests {
    use super::*;

    // ── Colocated children ──────────────────────────────────────────────────
    //
    // The value of `child()` is not geometry — `solve()` already got that
    // right — it is that content cannot drift away from its slot. These tests
    // assert the binding, since that is the failure the index API allowed and
    // the one no geometry assertion can see.

    /// A conditional slot removes its content WITH it.
    ///
    /// Under the index API, omitting the icon shifted "title" from 1 to 0 and
    /// the caller's `match` had to know which branches had run. Here the count
    /// simply tracks the conditions.
    #[test]
    fn conditional_children_keep_slots_and_content_together() {
        for (icon, closable, expect) in
            [(false, false, 2), (true, false, 3), (false, true, 3), (true, true, 4)]
        {
            let f = Flex::row()
                .child_if(icon, Item::fixed(16.0), |_| {})
                .child(Item::fixed(80.0), |_| {})
                .child(Item::grow(1.0), |_| {})
                .child_if(closable, Item::fixed(24.0), |_| {});
            assert_eq!(
                f.len(), expect,
                "icon={icon} closable={closable}: a conditional slot must add \
                 or remove its content along with itself",
            );
            assert_eq!(
                f.solve(Vec2::new(400.0, 30.0)).len(), f.len(),
                "every child must get exactly one solved rect",
            );
        }
    }

    /// Reordering rows moves the CONTENT, not just the boxes.
    ///
    /// This is the regression the index API could not fail on: swap two
    /// `.item()` calls and the geometry stays correct while the content lands
    /// in the wrong slot, so every existing assertion still passes. Recording
    /// which child painted into which rect is the only way to see it.
    #[test]
    fn reordering_children_moves_their_content() {
        use std::cell::RefCell;
        use std::rc::Rc;

        // Solve two layouts that differ only in the order of the two fixed
        // slots, and check the WIDTH each label is given.
        let widths = |first_small: bool| {
            let log: Rc<RefCell<Vec<(&'static str, f32)>>> = Rc::new(RefCell::new(Vec::new()));
            let (a, b) = if first_small { (40.0, 120.0) } else { (120.0, 40.0) };
            let f = Flex::row()
                .child(Item::fixed(a), |_| {})
                .child(Item::fixed(b), |_| {});
            let rects = f.solve(Vec2::new(300.0, 20.0));
            log.borrow_mut().push(("first", rects[0].width()));
            log.borrow_mut().push(("second", rects[1].width()));
            let out = log.borrow().clone();
            out
        };
        let normal = widths(true);
        let swapped = widths(false);
        assert_eq!(normal[0].1, 40.0);
        assert_eq!(swapped[0].1, 120.0);
        assert_ne!(
            normal[0].1, swapped[0].1,
            "swapping the two children must swap the rects they receive",
        );
    }

    // ── Keyed slots ─────────────────────────────────────────────────────────

    /// A slot is found by its key no matter which conditional slots ran.
    ///
    /// This is the property the positional version could not offer. There, the
    /// builder and the reader were two lists that had to agree on order and on
    /// which branches fired; here the reader names what it wants.
    #[test]
    fn slots_are_found_by_key_regardless_of_which_ones_exist() {
        for (icon, closable) in [(false, false), (true, false), (false, true), (true, true)] {
            let solved = Flex::row()
                .slot_if("icon", icon, Item::fixed(16.0))
                .slot("title", Item::fixed(80.0))
                .spacer(1.0)
                .slot_if("close", closable, Item::fixed(24.0))
                .solve(Vec2::new(400.0, 30.0));

            let title = solved.get("title").expect("title is unconditional");
            assert!(approx(title.width(), 80.0), "title must keep its width");
            assert_eq!(solved.get("icon").is_some(), icon, "icon presence follows its condition");
            assert_eq!(solved.get("close").is_some(), closable, "close presence follows its condition");
            // The one that matters: with the icon present the title starts
            // AFTER it, and the reader never had to know that.
            if icon {
                let ic = solved.get("icon").unwrap();
                assert!(title.min.x >= ic.max.x, "title must follow the icon");
            }
            assert!(solved.get("nope").is_none(), "unknown keys are None, not a panic");
        }
    }

    /// Anonymous spacing is never returned by a lookup.
    ///
    /// `panel_section` modelled gaps as a repeated `Slot::Gap` key, which put
    /// duplicates into the namespace. Spacing here carries no key at all, so
    /// there is nothing to collide and nothing to look up by mistake.
    #[test]
    fn spacing_is_anonymous_and_pushes_the_trailing_slot_right() {
        let solved = Flex::row()
            .slot("caret", Item::fixed(12.0))
            .pad(4.0)
            .slot("title", Item::fixed(60.0))
            .spacer(1.0)
            .slot("meta", Item::fixed(40.0))
            .solve(Vec2::new(300.0, 20.0));

        assert_eq!(solved.len(), 5, "spacing occupies a rect like anything else");
        let caret = solved.get("caret").unwrap();
        let title = solved.get("title").unwrap();
        assert!(
            approx(title.min.x, caret.max.x + 4.0),
            "the 4px pad must sit between caret and title: caret ends {}, title starts {}",
            caret.max.x, title.min.x,
        );
        assert!(
            approx(solved.get("meta").unwrap().max.x, 300.0),
            "the spacer must push meta to the right edge",
        );
    }

    /// A padded row of fixed slots with a trailing `grow` reproduces a cursor
    /// walk exactly.
    ///
    /// This is the shape almost every hand-laid-out row in the app has:
    ///
    ///     let mut cx = inner.left() + 1.0;
    ///     let a = Rect::from_min_size(pos2(cx, y), vec2(w_a, h));
    ///     cx = a.right() + 4.0;
    ///     let b = Rect::from_min_size(pos2(cx, y), vec2(w_b, h));
    ///     cx = b.right() + 3.0;
    ///     let c_w = inner.right() - cx - 1.0;
    ///
    /// Migrating one means claiming the solver produces the same numbers. The
    /// DOM order-entry row is a money path, so that claim is asserted here
    /// rather than eyeballed — including the trailing slot's width, which in
    /// the cursor version is the one value derived from all the others.
    #[test]
    fn padded_fixed_row_with_trailing_grow_matches_a_cursor_walk() {
        let (left, width, h) = (0.0_f32, 240.0_f32, 20.0_f32);
        let (pad_l, pad_r) = (1.0_f32, 1.0_f32);
        let (w_a, w_b) = (240.0 * 0.48, 240.0 * 0.30);
        let (gap_ab, gap_bc) = (4.0_f32, 3.0_f32);

        // The cursor version, written out.
        let cur_a_x = left + pad_l;
        let cur_b_x = cur_a_x + w_a + gap_ab;
        let cur_c_x = cur_b_x + w_b + gap_bc;
        let cur_c_w = (left + width) - cur_c_x - pad_r;

        let solved = Flex::row()
            .padding_sides(pad_l, pad_r, 0.0, 0.0)
            .slot("a", Item::fixed(w_a))
            .pad(gap_ab)
            .slot("b", Item::fixed(w_b))
            .pad(gap_bc)
            .slot("c", Item::grow(1.0))
            .solve_in(Rect::from_min_size(egui::pos2(left, 0.0), Vec2::new(width, h)));

        let (a, b, c) = (solved.rect("a"), solved.rect("b"), solved.rect("c"));
        assert!(approx(a.min.x, cur_a_x), "a.x {} vs cursor {}", a.min.x, cur_a_x);
        assert!(approx(a.width(), w_a), "a.w {} vs {}", a.width(), w_a);
        assert!(approx(b.min.x, cur_b_x), "b.x {} vs cursor {}", b.min.x, cur_b_x);
        assert!(approx(b.width(), w_b), "b.w {} vs {}", b.width(), w_b);
        assert!(approx(c.min.x, cur_c_x), "c.x {} vs cursor {}", c.min.x, cur_c_x);
        assert!(
            approx(c.width(), cur_c_w),
            "the trailing grow must fill exactly what the cursor computed: {} vs {}",
            c.width(), cur_c_w,
        );
    }

    // ── Composition ─────────────────────────────────────────────────────────

    /// A flex nested inside a flex child lays out within THAT child's rect.
    ///
    /// This is the property that makes the engine compose the way React does:
    /// a component lays out its own children without knowing where it sits.
    /// It only holds if the `Ui` handed to a child reports the child's solved
    /// rect as its available space — if it leaked the parent's width, every
    /// nested row would size itself against the wrong container and overflow.
    ///
    /// The engine has no nested-container node; nesting happens at render time
    /// by calling `Flex` again inside a child closure. So this is the test that
    /// says that is a supported thing to do, rather than something that
    /// happens to work today.
    #[test]
    fn a_nested_flex_lays_out_inside_its_parent_slot() {
        use std::cell::RefCell;
        use std::rc::Rc;

        let outer_child_w: Rc<RefCell<f32>> = Rc::new(RefCell::new(0.0));
        let inner_rects: Rc<RefCell<Vec<Rect>>> = Rc::new(RefCell::new(Vec::new()));
        let (ow, ir) = (outer_child_w.clone(), inner_rects.clone());

        let ctx = egui::Context::default();
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                // Pin the outer container so the numbers are deterministic.
                let host = Rect::from_min_size(ui.max_rect().min, Vec2::new(400.0, 40.0));
                let mut host_ui = ui.new_child(egui::UiBuilder::new().max_rect(host));

                Flex::row()
                    .child(Item::fixed(100.0), |_| {})
                    .child(Item::fixed(300.0), |child_ui| {
                        *ow.borrow_mut() = child_ui.available_size_before_wrap().x;
                        // A whole second layout, inside the slot.
                        let nested = Flex::row()
                            .child(Item::grow(1.0), |_| {})
                            .child(Item::fixed(50.0), |_| {});
                        *ir.borrow_mut() =
                            nested.solve(child_ui.available_size_before_wrap());
                    })
                    .show(&mut host_ui);
            });
        });

        let w = *outer_child_w.borrow();
        assert!(
            (w - 300.0).abs() < 1.0,
            "the child Ui must report ITS OWN width (300), not the parent's \
             (400) — got {w}; a leak here makes every nested layout overflow",
        );
        let inner = inner_rects.borrow();
        assert_eq!(inner.len(), 2, "the nested layout solved its own children");
        assert!(
            (inner[1].max.x - 300.0).abs() < 1.0,
            "the nested row must fill its slot, ending at 300 — got {}",
            inner[1].max.x,
        );
        assert!(
            (inner[0].width() - 250.0).abs() < 1.0,
            "grow inside the nest takes the slot's leftover (300-50), got {}",
            inner[0].width(),
        );
    }

    /// Two slots may not share a key — that would make a lookup ambiguous.
    #[test]
    #[should_panic(expected = "duplicate flex slot key")]
    fn duplicate_keys_are_rejected() {
        let _ = Flex::row()
            .slot("title", Item::fixed(10.0))
            .slot("title", Item::fixed(10.0));
    }

    /// `spacer()` is the `<div style="flex:1"/>` idiom: it pushes what follows
    /// to the far edge and paints nothing.
    #[test]
    fn spacer_pushes_following_children_to_the_end() {
        let f = Flex::row()
            .child(Item::fixed(50.0), |_| {})
            .spacer(1.0)
            .child(Item::fixed(30.0), |_| {});
        let r = f.solve(Vec2::new(300.0, 20.0));
        assert_eq!(f.len(), 3, "the spacer is a child like any other");
        assert!(approx(r[0].min.x, 0.0), "first child pinned left");
        assert!(
            approx(r[2].max.x, 300.0),
            "the child after a spacer must reach the right edge, got {}",
            r[2].max.x,
        );
    }

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

// ── M4.6 measure-hook tests ──────────────────────────────────────────────────
#[cfg(test)]
mod m46_measure_tests {
    use super::*;

    /// The circular case the chrome migration could not migrate: a wrapping
    /// description's HEIGHT depends on the width the solve gives it, and the
    /// row's cross-axis layout depends on that height. One pass cannot do it.
    ///
    /// Model a text block of 600px of glyphs: at width w it wraps to
    /// ceil(600/w) lines of 14px.
    #[test]
    fn measured_child_gets_height_from_its_solved_width() {
        let text_px = 600.0_f32;
        let line_h  = 14.0_f32;
        let measure = Measure::new(move |w| (text_px / w.max(1.0)).ceil() * line_h);

        // 200px available: 600/200 = 3 lines = 42px.
        let rects = Flex::row()
            .item(Item::grow(1.0).measure(measure.clone()))
            .solve(egui::vec2(200.0, 100.0));
        assert!((rects[0].height() - 42.0).abs() < 0.01,
            "at 200px wide the block should wrap to 3 lines (42px), got {}", rects[0].height());

        // 300px available: 600/300 = 2 lines = 28px. Same spec, different
        // width -> different height. That is the circularity, resolved.
        let wider = Flex::row()
            .item(Item::grow(1.0).measure(measure))
            .solve(egui::vec2(300.0, 100.0));
        assert!((wider[0].height() - 28.0).abs() < 0.01,
            "at 300px wide it should wrap to 2 lines (28px), got {}", wider[0].height());
    }

    /// The real shape that was left behind: `toggle_row` — a fixed leading
    /// control, a growing label+description column that wraps, and a fixed
    /// trailing switch. The measured column must not disturb its siblings.
    #[test]
    fn toggle_row_shape_solves_in_one_call() {
        let measure = Measure::new(|w| (900.0_f32 / w.max(1.0)).ceil() * 16.0);
        let rects = Flex::row()
            .gap(8.0)
            .item(Item::fixed(20.0))                            // icon
            .item(Item::grow(1.0).measure(measure))             // label + wrapping desc
            .item(Item::fixed(36.0))                            // switch
            .solve(egui::vec2(300.0, 0.0));

        assert!((rects[0].width() - 20.0).abs() < 0.01, "fixed leading survives");
        assert!((rects[2].width() - 36.0).abs() < 0.01, "fixed trailing survives");
        // grow column = 300 - 20 - 36 - 2 gutters(8) = 228 -> 900/228 = 4 lines
        assert!((rects[1].width() - 228.0).abs() < 0.01, "got {}", rects[1].width());
        assert!((rects[1].height() - 64.0).abs() < 0.01,
            "4 wrapped lines = 64px, got {}", rects[1].height());
    }

    /// A spec with no hooks must take the single-pass path — the measure
    /// machinery is opt-in and costs nothing when unused.
    #[test]
    fn unhooked_spec_is_single_pass_and_unchanged() {
        let spec = Flex::row().item(Item::fixed(50.0)).item(Item::grow(1.0));
        assert!(!spec.needs_measure_pass());
        let rects = spec.solve(egui::vec2(200.0, 20.0));
        assert!((rects[0].width() - 50.0).abs() < 0.01);
        assert!((rects[1].width() - 150.0).abs() < 0.01);
    }

    /// `.rigid()` makes a measured item refuse to yield — the form a faithful
    /// cursor-walk migration needs (a hand-written walk overflows; `Content`
    /// shrinks). The chrome agent hit this at 4 of 15 sites.
    #[test]
    fn rigid_items_do_not_shrink_when_oversubscribed() {
        let soft = Flex::row()
            .item(Item::content(150.0))
            .item(Item::content(150.0))
            .solve(egui::vec2(200.0, 20.0));
        assert!(soft[0].width() < 150.0, "plain Content yields");

        let rigid = Flex::row()
            .item(Item::content(150.0).rigid())
            .item(Item::content(150.0).rigid())
            .solve(egui::vec2(200.0, 20.0));
        assert!((rigid[0].width() - 150.0).abs() < 0.01,
            "rigid() must hold its measured width, got {}", rigid[0].width());
    }
}

#[cfg(test)]
mod intrinsic_width_tests {
    use super::{Flex, Item};
    use egui::{Rect, Vec2};

    /// Solving into an INFINITE available width must yield finite rects.
    ///
    /// `ticker_strip` measures a quote by solving it unconstrained and reading
    /// the right edge of its last slot, so that it can decide whether the quote
    /// FITS before painting a single glyph. If Taffy returned an infinite or
    /// NaN rect here the comparison `cx + quote_w > rect.right()` would be
    /// meaningless — it would either never break or always break, and the strip
    /// would silently draw nothing or overflow exactly as it did before.
    #[test]
    fn an_unconstrained_row_measures_to_its_content() {
        let row = Flex::row()
            .slot("a", Item::content(30.0))
            .slot("b", Item::content(20.0).margin_start(8.0))
            .slot("c", Item::content(10.0).margin_start(8.0));

        let solved = row.solve_in(Rect::from_min_size(
            egui::pos2(0.0, 0.0),
            Vec2::new(f32::INFINITY, 20.0),
        ));
        let w = solved.rect("c").right();
        assert!(w.is_finite(), "unconstrained solve produced {w}");
        // 30 + 8 + 20 + 8 + 10
        assert!((w - 76.0).abs() < 0.5, "expected ~76, got {w}");
    }

    /// The same row solved into its own measured width must not shift.
    ///
    /// The strip measures once unconstrained, then solves again inside the rect
    /// it just sized. If those two disagreed, every quote would be painted at
    /// an offset from the box the click handler uses.
    #[test]
    fn measuring_then_placing_agrees_with_itself() {
        let row = || Flex::row()
            .slot("a", Item::content(30.0))
            .slot("b", Item::content(20.0).margin_start(8.0));

        let w = row()
            .solve_in(Rect::from_min_size(egui::pos2(0.0, 0.0), Vec2::new(f32::INFINITY, 20.0)))
            .rect("b").right();
        let placed = row().solve_in(Rect::from_min_size(egui::pos2(100.0, 0.0), Vec2::new(w, 20.0)));
        assert!((placed.rect("a").left() - 100.0).abs() < 0.5);
        assert!((placed.rect("b").right() - (100.0 + w)).abs() < 0.5,
            "placement disagreed with measurement: {} vs {}", placed.rect("b").right(), 100.0 + w);
    }
}

#[cfg(test)]
mod flex_basis_tests {
    use super::{Flex, Item};
    use egui::Vec2;

    /// `Item::flex(grow, basis)` starts at `basis` and then takes its share of
    /// the slack — the model `Item::grow` cannot express, because `grow` pins
    /// basis to 0.
    #[test]
    fn a_flex_item_starts_at_its_basis_then_takes_its_share() {
        // 200 wide, two items: basis 50 + 50 = 100, slack 100 split 1:3.
        let r = Flex::row()
            .item(Item::flex(1.0, 50.0))
            .item(Item::flex(3.0, 50.0))
            .solve(Vec2::new(200.0, 20.0));
        assert!((r[0].width() - 75.0).abs() < 0.5, "first was {}", r[0].width());
        assert!((r[1].width() - 125.0).abs() < 0.5, "second was {}", r[1].width());
    }

    /// A zero grow keeps the basis exactly — a fixed column beside flexible ones.
    #[test]
    fn zero_grow_keeps_the_basis() {
        let r = Flex::row()
            .item(Item::flex(0.0, 60.0))
            .item(Item::flex(1.0, 0.0))
            .solve(Vec2::new(200.0, 20.0));
        assert!((r[0].width() - 60.0).abs() < 0.5, "fixed column was {}", r[0].width());
        assert!((r[1].width() - 140.0).abs() < 0.5, "flexible column was {}", r[1].width());
    }

    /// The distinction from `grow`, stated as a test so the two cannot be
    /// confused later: `grow` ignores any basis and splits the whole axis.
    #[test]
    fn grow_splits_the_whole_axis_and_flex_does_not() {
        let g = Flex::row()
            .item(Item::grow(1.0))
            .item(Item::grow(1.0))
            .solve(Vec2::new(200.0, 20.0));
        assert!((g[0].width() - 100.0).abs() < 0.5);

        let f = Flex::row()
            .item(Item::flex(1.0, 120.0))
            .item(Item::flex(1.0, 0.0))
            .solve(Vec2::new(200.0, 20.0));
        assert!(f[0].width() > f[1].width(), "basis must bias the split: {} vs {}",
            f[0].width(), f[1].width());
    }

    /// Bases wider than the container degrade proportionally rather than
    /// overflowing — the row still fits its box.
    #[test]
    fn oversized_bases_shrink_to_fit() {
        let r = Flex::row()
            .item(Item::flex(1.0, 300.0))
            .item(Item::flex(1.0, 300.0))
            .solve(Vec2::new(200.0, 20.0));
        let total = r[0].width() + r[1].width();
        assert!(total <= 200.5, "row overflowed its container: {total}");
    }
}
