//! Unified icon glyph re-exports for widgets.
//!
//! Provides both the `Icon` type (with all associated consts) and bare
//! `&'static str` glyph constants for ergonomic `use widgets::icons::*;`.
//!
//! Source of truth remains `crate::ui_kit::icons::Icon`.

pub use crate::ui_kit::icons::Icon;

/// Bare glyph constants — convenience for widgets that prefer
/// `icons::DOTS_SIX_VERTICAL` over `Icon::DOTS_SIX_VERTICAL`.
pub const DOTS_SIX_VERTICAL: &str = Icon::DOTS_SIX_VERTICAL;
pub const SPARKLE: &str = Icon::SPARKLE;
pub const STAR: &str = Icon::STAR;
pub const X: &str = Icon::X;
pub const LIGHTNING: &str = Icon::LIGHTNING;
pub const EYE: &str = Icon::EYE;
pub const EYE_SLASH: &str = Icon::EYE_SLASH;
