//! Layout engines for ui_kit.
//!
//! `flex` is a Taffy-backed flexbox for panel chrome, forms, headers and
//! toolbars — the places where alignment was previously hand-computed
//! arithmetic. It computes GEOMETRY ONLY: all colour, type, radius and
//! per-style treatment continues to come from the design system.
pub mod flex;
pub use flex::{Align, Flex, FlexSlots, FlexUi, Item, Justify, Pad, Size, SolvedSlots};
pub mod surface;
pub use surface::{Surface, SurfaceResponse};
/// M4.4: CSS-grid layout. Taffy's `grid` feature was already compiled in and
/// unused; this wrapper unblocks Aperture's 12-col × 92px mosaic and the
/// editorial `300px / 1fr / 360px` dashboard, neither of which the binary-split
/// `PaneGrid` or the uniform `dashboard_pane` tiler can express (no spans).
pub mod grid;
pub use grid::{Grid, GridItem, Track};
