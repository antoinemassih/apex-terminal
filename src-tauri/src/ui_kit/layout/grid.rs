//! `Grid` — declarative CSS-grid layout over Taffy (M4.4).
//!
//! ## Why this exists
//!
//! The architecture audit found that the `grid` feature of Taffy was already
//! compiled into the binary (`Cargo.toml`) and **completely unused**, while the
//! three grid-shaped things in the tree could not express what the design
//! systems need:
//!
//! | in-tree | model | why it can't do the job |
//! |---|---|---|
//! | `ui_kit::PaneGrid` | recursive BINARY SPLIT tree, depth-capped at 8 | no spans, no tracks; a 12-column mosaic would exceed the depth cap and still only approximate the fractions |
//! | `panels::rail_layout` | greedy first-fit column packer, `Full`/`Half` only | binary vertical granularity, no horizontal spans |
//! | `panels::dashboard_pane` | uniform auto-tiler | equal cells only, no spans, magic literals |
//!
//! Aperture's signature **12-column × 92px mosaic** (`grid-auto-rows: 92px`,
//! spans of 1/2/3/4/6/12 columns and 1–4 rows) is therefore inexpressible
//! today. So is the editorial `300px / 1fr / 360px` dashboard. This module is
//! the ~200-line wrapper that closes that gap, mirroring `flex.rs`'s design:
//! a pure `solve()` for headless testing, exact f32 (rounding disabled), and
//! `show()` for the egui path.
//!
//! ## Scope
//!
//! Panel chrome, dashboards and tile mosaics — **not** chart panes or streaming
//! rows, which need painter-exact geometry and no per-frame solve. One
//! `TaffyTree` is built per `show()`/`solve()` call, same as `Flex`.
//!
//! ```ignore
//! // Aperture's mosaic: 12 equal columns, 92px rows, 12px gutters.
//! Grid::new()
//!     .cols(Track::fr_repeat(12, 1.0))
//!     .auto_rows(92.0)
//!     .gap(12.0)
//!     .item(GridItem::new().col_span(4).row_span(2))   // hero tile
//!     .item(GridItem::new().col_span(2))               // KPI
//!     .show(ui, |ui, i| render_tile(ui, i));
//!
//! // Editorial dashboard: fixed / flexible / fixed.
//! Grid::new()
//!     .cols(vec![Track::px(300.0), Track::fr(1.0), Track::px(360.0)])
//!     .rows(vec![Track::auto(), Track::fr(1.1), Track::fr(1.0), Track::fr(0.9)])
//!     .solve(avail);
//! ```

use egui::{Rect, Ui, Vec2};
use taffy::prelude::*;
use taffy::style::GridTemplateComponent;

/// Guard rail mirroring `Flex`: grids are for chrome and dashboards, not for
/// hot per-row rendering.
const MAX_REASONABLE_CELLS: usize = 256;

/// One track (column or row) in the template — CSS `grid-template-*`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Track {
    /// Exactly this many pixels (`300px`).
    Px(f32),
    /// A share of the leftover space (`1fr`, `1.1fr`).
    Fr(f32),
    /// Sized to content (`auto`).
    Auto,
}

impl Track {
    pub fn px(v: f32) -> Self { Track::Px(v) }
    pub fn fr(v: f32) -> Self { Track::Fr(v) }
    pub fn auto() -> Self { Track::Auto }

    /// `repeat(n, <track>)` — the 12-column idiom.
    pub fn repeat(n: usize, t: Track) -> Vec<Track> { vec![t; n] }
    /// `repeat(n, <f>fr)`, the common case.
    pub fn fr_repeat(n: usize, f: f32) -> Vec<Track> { vec![Track::Fr(f); n] }

    fn to_taffy(self) -> TrackSizingFunction {
        match self {
            Track::Px(v)  => length(v),
            Track::Fr(v)  => fr(v),
            Track::Auto   => taffy::style_helpers::auto(),
        }
    }
}

/// One cell placed in the grid. Spans default to 1×1 and auto-flow places it.
#[derive(Clone, Copy, Debug, Default)]
pub struct GridItem {
    col_span: u16,
    row_span: u16,
    /// Explicit 1-based column line, or `None` for auto-placement.
    col_start: Option<u16>,
    /// Explicit 1-based row line, or `None` for auto-placement.
    row_start: Option<u16>,
}

impl GridItem {
    pub fn new() -> Self { Self { col_span: 1, row_span: 1, col_start: None, row_start: None } }
    /// Span N columns (Aperture's tiles use 1–12).
    pub fn col_span(mut self, n: u16) -> Self { self.col_span = n.max(1); self }
    /// Span N rows (Aperture's tiles use 1–4).
    pub fn row_span(mut self, n: u16) -> Self { self.row_span = n.max(1); self }
    /// Pin to an explicit 1-based column line instead of auto-flow.
    pub fn col_start(mut self, line: u16) -> Self { self.col_start = Some(line.max(1)); self }
    /// Pin to an explicit 1-based row line instead of auto-flow.
    pub fn row_start(mut self, line: u16) -> Self { self.row_start = Some(line.max(1)); self }
}

/// A CSS-grid container.
#[derive(Clone, Debug, Default)]
pub struct Grid {
    cols: Vec<Track>,
    rows: Vec<Track>,
    /// Height of implicitly-created rows (`grid-auto-rows`), e.g. Aperture's 92.
    auto_row: Option<f32>,
    gap_x: f32,
    gap_y: f32,
    items: Vec<GridItem>,
}

impl Grid {
    pub fn new() -> Self { Self::default() }

    /// Explicit column template.
    pub fn cols(mut self, tracks: Vec<Track>) -> Self { self.cols = tracks; self }
    /// Explicit row template. Omit to rely on `auto_rows`.
    pub fn rows(mut self, tracks: Vec<Track>) -> Self { self.rows = tracks; self }
    /// `grid-auto-rows: <px>` — the height of rows the grid creates implicitly.
    pub fn auto_rows(mut self, px: f32) -> Self { self.auto_row = Some(px); self }

    /// Uniform gutter on both axes.
    pub fn gap(mut self, g: f32) -> Self { self.gap_x = g; self.gap_y = g; self }
    /// Separate column / row gutters (CSS `column-gap` / `row-gap`).
    pub fn gap_xy(mut self, x: f32, y: f32) -> Self { self.gap_x = x; self.gap_y = y; self }

    pub fn item(mut self, i: GridItem) -> Self { self.items.push(i); self }
    pub fn items(mut self, it: impl IntoIterator<Item = GridItem>) -> Self {
        self.items.extend(it); self
    }

    /// Solve the layout headlessly and return one `Rect` per item, in the order
    /// the items were added. Pure — no egui, no side effects — so grid geometry
    /// is unit-testable exactly like `Flex::solve`.
    pub fn solve(&self, available: Vec2) -> Vec<Rect> {
        debug_assert!(
            self.items.len() <= MAX_REASONABLE_CELLS,
            "Grid with {} cells — grids are for chrome/dashboards, not hot row \
             rendering; see the module docs",
            self.items.len()
        );
        if self.items.is_empty() || self.cols.is_empty() {
            return Vec::new();
        }

        let mut tree: TaffyTree<()> = TaffyTree::new();
        // Same rationale as `flex.rs`: egui lays out at fractional x, and
        // spacing tokens scale by 0.75/1.15/1.25 — rounding would quantise
        // every migrated tile and shift it against the design it reproduces.
        tree.disable_rounding();

        let children: Vec<NodeId> = self
            .items
            .iter()
            .map(|it| {
                let mut st = Style::default();
                st.grid_column = match it.col_start {
                    Some(l) => Line { start: line(l as i16), end: span(it.col_span) },
                    None       => Line { start: taffy::style::GridPlacement::Auto, end: span(it.col_span) },
                };
                st.grid_row = match it.row_start {
                    Some(l) => Line { start: line(l as i16), end: span(it.row_span) },
                    None       => Line { start: taffy::style::GridPlacement::Auto, end: span(it.row_span) },
                };
                tree.new_leaf(st).expect("taffy leaf")
            })
            .collect();

        let mut root_style = Style {
            display: Display::Grid,
            size: Size {
                width:  length(available.x),
                height: length(available.y),
            },
            gap: Size { width: length(self.gap_x), height: length(self.gap_y) },
            grid_template_columns: self.cols.iter().map(|t| GridTemplateComponent::Single(t.to_taffy())).collect(),
            ..Default::default()
        };
        if !self.rows.is_empty() {
            root_style.grid_template_rows = self.rows.iter().map(|t| GridTemplateComponent::Single(t.to_taffy())).collect();
        }
        if let Some(h) = self.auto_row {
            root_style.grid_auto_rows = vec![length(h)];
        }

        let root = tree.new_with_children(root_style, &children).expect("taffy root");
        tree.compute_layout(
            root,
            Size {
                width:  AvailableSpace::Definite(available.x),
                height: AvailableSpace::Definite(available.y),
            },
        )
        .expect("taffy grid solve");

        children
            .iter()
            .map(|&c| {
                let l = tree.layout(c).expect("taffy layout");
                Rect::from_min_size(
                    egui::pos2(l.location.x, l.location.y),
                    egui::vec2(l.size.width, l.size.height),
                )
            })
            .collect()
    }

    /// Solve against `ui`'s available space and render each cell through
    /// `render(ui, index)` inside its solved rect.
    pub fn show<R>(
        self,
        ui: &mut Ui,
        mut render: impl FnMut(&mut Ui, usize) -> R,
    ) -> Vec<R> {
        let avail = ui.available_size_before_wrap();
        let origin = ui.min_rect().min.to_vec2();
        let rects = self.solve(avail);
        let mut out = Vec::with_capacity(rects.len());
        for (i, r) in rects.iter().enumerate() {
            // Solved rects are container-relative; offset into screen space.
            let screen = Rect::from_min_size(
                egui::pos2(r.min.x + origin.x, r.min.y + origin.y),
                r.size(),
            );
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(screen));
            out.push(render(&mut child, i));
        }
        // Claim the space so siblings flow after the grid.
        let used = rects.iter().fold(0.0_f32, |acc, r| acc.max(r.max.y));
        ui.allocate_space(egui::vec2(avail.x, used));
        out
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Aperture's signature mosaic, headlessly: 12 equal columns, 92px rows.
    /// This is the layout the audit called inexpressible — `PaneGrid`'s binary
    /// splits have no spans and would blow the depth cap, and `dashboard_pane`
    /// only tiles uniformly.
    #[test]
    fn aperture_12_col_92px_mosaic() {
        // width chosen so 12 cols + 11 gutters divide exactly:
        // 12*100 + 11*12 = 1332
        let rects = Grid::new()
            .cols(Track::fr_repeat(12, 1.0))
            .auto_rows(92.0)
            .gap(12.0)
            .item(GridItem::new().col_span(4).row_span(2))  // hero
            .item(GridItem::new().col_span(2))              // KPI
            .item(GridItem::new().col_span(6))              // watchlist
            .solve(egui::vec2(1332.0, 400.0));

        assert_eq!(rects.len(), 3);
        // 4-col hero = 4 tracks + 3 inner gutters = 4*100 + 3*12 = 436
        assert!((rects[0].width() - 436.0).abs() < 0.5,
            "4-col span should be 436px, got {}", rects[0].width());
        // 2 rows tall = 2*92 + 1 row gutter = 196
        assert!((rects[0].height() - 196.0).abs() < 0.5,
            "2-row span should be 196px, got {}", rects[0].height());
        // 2-col KPI = 2*100 + 12 = 212
        assert!((rects[1].width() - 212.0).abs() < 0.5,
            "2-col span should be 212px, got {}", rects[1].width());
    }

    /// The editorial dashboard track list: `300px / 1fr / 360px`. Fixed tracks
    /// hold; the flexible middle absorbs the remainder.
    #[test]
    fn editorial_300_1fr_360_tracks() {
        let rects = Grid::new()
            .cols(vec![Track::px(300.0), Track::fr(1.0), Track::px(360.0)])
            .rows(vec![Track::fr(1.0)])
            .gap(0.0)
            .item(GridItem::new())
            .item(GridItem::new())
            .item(GridItem::new())
            .solve(egui::vec2(1440.0, 800.0));

        assert!((rects[0].width() - 300.0).abs() < 0.5, "left rail fixed at 300");
        assert!((rects[2].width() - 360.0).abs() < 0.5, "right rail fixed at 360");
        assert!((rects[1].width() - 780.0).abs() < 0.5,
            "centre takes the remainder (1440-300-360), got {}", rects[1].width());
    }

    /// Row weights: the editorial dashboard's `auto / 1.1fr / 1fr / 0.9fr`.
    #[test]
    fn fractional_row_weights_split_proportionally() {
        let rects = Grid::new()
            .cols(vec![Track::fr(1.0)])
            .rows(vec![Track::fr(1.1), Track::fr(1.0), Track::fr(0.9)])
            .gap(0.0)
            .item(GridItem::new())
            .item(GridItem::new())
            .item(GridItem::new())
            .solve(egui::vec2(100.0, 300.0));

        // 1.1 + 1.0 + 0.9 = 3.0 -> 110 / 100 / 90 of 300
        assert!((rects[0].height() - 110.0).abs() < 0.5, "got {}", rects[0].height());
        assert!((rects[1].height() - 100.0).abs() < 0.5, "got {}", rects[1].height());
        assert!((rects[2].height() -  90.0).abs() < 0.5, "got {}", rects[2].height());
    }

    /// Explicit placement pins a cell to a line instead of auto-flowing.
    #[test]
    fn explicit_placement_pins_a_cell() {
        let rects = Grid::new()
            .cols(Track::fr_repeat(4, 1.0))
            .auto_rows(50.0)
            .gap(0.0)
            .item(GridItem::new().col_start(3).col_span(2))
            .solve(egui::vec2(400.0, 100.0));
        // column line 3 => x offset of two 100px tracks
        assert!((rects[0].min.x - 200.0).abs() < 0.5,
            "pinned cell should start at x=200, got {}", rects[0].min.x);
        assert!((rects[0].width() - 200.0).abs() < 0.5, "and span two tracks");
    }

    /// Gutters are real space, not padding on the cells.
    #[test]
    fn gaps_separate_cells_without_overlap() {
        let rects = Grid::new()
            .cols(Track::fr_repeat(3, 1.0))
            .auto_rows(40.0)
            .gap(10.0)
            .items((0..3).map(|_| GridItem::new()))
            .solve(egui::vec2(320.0, 40.0));
        assert!(rects[0].right() + 9.9 <= rects[1].left(), "10px gutter between 1 and 2");
        assert!(rects[1].right() + 9.9 <= rects[2].left(), "10px gutter between 2 and 3");
    }

    /// An empty grid is a no-op rather than a panic (defensive: callers build
    /// these from data that may legitimately be empty).
    #[test]
    fn empty_grid_solves_to_nothing() {
        assert!(Grid::new().cols(Track::fr_repeat(12, 1.0)).solve(egui::vec2(100.0, 100.0)).is_empty());
        assert!(Grid::new().item(GridItem::new()).solve(egui::vec2(100.0, 100.0)).is_empty());
    }
}
