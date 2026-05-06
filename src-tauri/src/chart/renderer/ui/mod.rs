//! UI components for the native GPU chart renderer.
//! Reusable helpers, widget factories, and drawing functions.

pub mod style;
pub mod foundation;
pub mod chrome;
pub mod inputs;
pub mod lists;
pub mod components;
pub mod tools;
pub mod panels;
pub mod watchlist;
pub mod command_palette;
pub mod chart_pane;
pub mod chart_widgets;
pub mod toolbar;
pub mod pane;

// Backward-compat: keep widgets and components_extra declared so old import paths
// that still exist in gpu.rs and others continue to resolve.
pub mod widgets;
pub mod components_extra;

// Re-export key items at ui level for ergonomics
pub use style::*;
pub use components::*;
pub use foundation::*;
