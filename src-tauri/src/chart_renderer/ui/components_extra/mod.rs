//! Additional canonical components — keyboard chips, search inputs, steppers,
//! toggles, filter chips, sortable headers, toasts, spinners, breadcrumbs,
//! notification badges, top-nav, action buttons, menus, DOM ladder buttons, etc.
//! Style-aware via `super::style::current()`.
//!
//! Split into submodules by concern. All previously-public items are
//! re-exported here so external callers continue to use
//! `components_extra::Foo` without source changes.

#![allow(dead_code)]

pub mod inputs;
pub mod chips;
pub mod sortable_headers;
pub mod toasts;
pub mod header_buttons;
pub mod panels;
pub mod action_button;
pub mod dom_action;

pub use inputs::*;
pub use chips::*;
pub use sortable_headers::*;
pub use toasts::*;
pub use header_buttons::*;
pub use panels::*;
pub use action_button::*;
pub use dom_action::*;

// TopNav enums are defined in widgets/toolbar/mod.rs; re-export here for
// backward compatibility so callers using `components_extra::TopNavTreatment`
// continue to compile without changes.
pub use super::widgets::toolbar::{TopNavTreatment, TopNavToggleSize, PaneTabStyle};
