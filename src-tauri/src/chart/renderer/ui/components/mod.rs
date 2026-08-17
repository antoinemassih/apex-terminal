//! Canonical chrome components — style-aware (Relay/Meridien) building blocks.
//!
//! These encapsulate paint patterns repeated across panels/dialogs. All radii,
//! strokes, and treatments route through `super::style` so a single style flip
//! propagates everywhere. Colors are passed in by callers (no `Theme` coupling).
//!
//! Split into submodules by concern (labels, pills, frames, headers, hairlines,
//! metrics). Everything is re-exported here so external callers continue to use
//! `components::foo` without source changes.


// Original components
pub mod headers;
pub mod hairlines;

// From components_extra
pub mod dom_action;
pub mod header_buttons;

// From widgets
pub mod menus;
pub mod layout;
pub mod text;
pub mod perf_hud;
pub mod design_mode_panel;

// Widget variants (legacy builder API)
pub mod frames_widget;
pub mod headers_widget;

pub mod semantic_label;
pub mod toolbar;
pub mod motion;

pub use headers::*;
pub use hairlines::*;
pub use dom_action::*;
pub use header_buttons::*;
pub use menus::*;
pub use layout::*;
pub use frames_widget::*;
pub use headers_widget::*;
