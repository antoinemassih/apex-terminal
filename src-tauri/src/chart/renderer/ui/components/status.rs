//! Builder + impl Widget primitives — status / feedback family.
//!
//! `StatusDot` has been removed — use `ui_kit::Indicator::dot().custom_color(c).label(text)`
//! instead (supports arbitrary colors and inline text labels).
//! `Spinner` and `Skeleton` have been migrated to `ui_kit::widgets`.

#![allow(dead_code, unused_imports)]

use egui::{Color32, Response, Ui};

// ─── Toast ────────────────────────────────────────────────────────────────────
//
// Toast / ToastVariant / ToastResponse have moved to
// `crate::ui_kit::widgets::toast`. Re-exported here for back-compat.
#[deprecated(note = "use crate::ui_kit::widgets::Toast")]
pub use crate::ui_kit::widgets::toast::Toast;
#[deprecated(note = "use crate::ui_kit::widgets::toast::ToastVariant")]
pub use crate::ui_kit::widgets::toast::ToastVariant;
#[deprecated(note = "use crate::ui_kit::widgets::toast::ToastResponse")]
pub use crate::ui_kit::widgets::toast::ToastResponse;

// (Toast struct/impl moved to ui_kit::widgets::toast — see re-exports above.)


