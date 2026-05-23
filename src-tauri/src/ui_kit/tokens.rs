//! Design tokens — re-exported from `chart_renderer::ui::style` for now.
//!
//! Part of the UI extraction (see `docs/UI_EXTRACTION.md`). This module
//! exists so `ui_kit::widgets/*` files can write
//!
//! ```ignore
//! use crate::ui_kit::tokens::{font_sm, gap_xs, color_alpha};
//! ```
//!
//! instead of reaching back into `chart_renderer::ui::style::*`. The
//! actual token helpers will physically move into this module in a later
//! phase; for now the re-export severs the source-level dependency
//! direction so `ui_kit` can be split into its own crate cleanly.

#[allow(unused_imports)]
pub use crate::chart_renderer::ui::style::*;
