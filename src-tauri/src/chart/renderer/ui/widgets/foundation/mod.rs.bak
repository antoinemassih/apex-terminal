//! Foundation layer for the apex-terminal design system.
//!
//! This module provides the base building blocks that every concrete widget
//! family in `widgets/*` will eventually compose:
//!
//! - `tokens`      — `Size`, `Density`, `Radius` (resolve through `style::*`)
//! - `text_style`  — typography scale (`TextStyle::as_rich`)
//! - `interaction` — hover / focus / pressed / disabled treatment
//! - `variants`    — variant enums + per-variant theme color resolution
//! - `shell`       — `ButtonShell`, `RowShell`, `CardShell`, `InputShell`, `ChipShell`
//!
//! Wave 4.5b will migrate the existing widget files (`buttons.rs`, `pills.rs`,
//! `rows/`, `cards/`, etc.) onto these shells. This module is foundation only;
//! it does not modify any existing widget.

#![allow(dead_code, unused_imports)]

pub mod tokens;
pub mod text_style;
pub mod interaction;
pub mod variants;
pub mod shell;

pub use tokens::*;
pub use text_style::*;
pub use interaction::*;
pub use variants::*;
pub use shell::*;
