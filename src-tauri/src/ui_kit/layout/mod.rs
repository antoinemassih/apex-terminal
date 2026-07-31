//! Layout engines for ui_kit.
//!
//! `flex` is a Taffy-backed flexbox for panel chrome, forms, headers and
//! toolbars — the places where alignment was previously hand-computed
//! arithmetic. It computes GEOMETRY ONLY: all colour, type, radius and
//! per-style treatment continues to come from the design system.
pub mod flex;
pub use flex::{Align, Flex, Item, Justify, Pad, Size};
pub mod surface;
pub use surface::{Surface, SurfaceResponse};
