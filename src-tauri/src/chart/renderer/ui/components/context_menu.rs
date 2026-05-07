//! Context menu has been re-homed to `ui_kit::widgets::context_menu`.
//! This file is a thin shim so existing call sites keep compiling. After the
//! follow-up sweep migrates callers to the new path, this file goes away.

#![allow(deprecated)]

#[deprecated(note = "use crate::ui_kit::widgets::context_menu::*")]
pub use crate::ui_kit::widgets::context_menu::*;
