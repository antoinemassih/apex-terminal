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
pub mod top_nav;
pub mod action_button;
pub mod dom_action;

pub use inputs::*;
pub use chips::*;
pub use sortable_headers::*;
pub use toasts::*;
pub use header_buttons::*;
pub use panels::*;
pub use top_nav::*;
pub use action_button::*;
pub use dom_action::*;
