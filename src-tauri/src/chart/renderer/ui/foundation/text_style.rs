//! M2.1: `TextStyle` moved DOWN into `ui_kit` (`crate::ui_kit::text_style`)
//! so the design-system widgets can finally participate in the 16-tier text
//! cascade — the dependency direction (`chart` -> `ui_kit`) had fenced all 75
//! widget files out of it. This shim keeps every existing chart-side path
//! (`foundation::text_style::TextStyle`, `foundation::TextStyle`) compiling
//! unchanged.

pub use crate::ui_kit::text_style::*;
