//! Canonical chrome components — style-aware (Relay/Meridien) building blocks.
//!
//! These encapsulate paint patterns repeated across panels/dialogs. All radii,
//! strokes, and treatments route through `super::style` so a single style flip
//! propagates everywhere. Colors are passed in by callers (no `Theme` coupling).
//!
//! Split into submodules by concern (labels, pills, frames, headers, hairlines,
//! metrics). Everything is re-exported here so external callers continue to use
//! `components::foo` without source changes.

#![allow(dead_code)]

// Original components
pub mod labels;
pub mod pills;
pub mod frames;
pub mod headers;
pub mod hairlines;
pub mod metrics;

// From components_extra
pub mod action_button;
pub mod chips;
pub mod dom_action;
pub mod header_buttons;
pub mod inputs;
pub mod panels;
pub mod sortable_headers;
pub mod toasts;

// From widgets
pub mod context_menu;
pub mod menus;
pub mod layout;
pub mod tabs;
pub mod text;
pub mod status;
pub mod perf_hud;
pub mod design_mode_panel;

// Widget variants (legacy builder API)
pub mod frames_widget;
pub mod headers_widget;
pub mod pills_widget;

pub mod semantic_label;
pub mod toolbar;
pub mod motion;

pub use labels::*;
pub use pills::*;
pub use frames::*;
pub use headers::*;
pub use hairlines::*;
pub use metrics::*;
pub use action_button::*;
pub use chips::*;
pub use dom_action::*;
pub use header_buttons::*;
pub use sortable_headers::*;
pub use toasts::*;
pub use context_menu::*;
pub use menus::*;
pub use layout::*;
pub use frames_widget::*;
pub use headers_widget::*;
pub use pills_widget::*;
