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
pub mod overlays;
pub mod toolbar;
pub mod tps_overlay;
pub mod pane;
pub mod welcome;
pub mod theme_studio;

// Backward-compat: `widgets` is a re-export shim kept alive solely because the
// sacred `render/pane/core.rs` imports through it (see widgets/mod.rs).
pub mod widgets;

// Re-export key items at ui level for ergonomics
pub use style::*;
pub use components::*;
pub use foundation::*;
