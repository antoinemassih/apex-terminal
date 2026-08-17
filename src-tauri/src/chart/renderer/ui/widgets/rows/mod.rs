//! Row re-export shim.
//!
//! Domain-specific rows (`WatchlistRow`, `OrderRow`, `NewsRow`, `DomRow`,
//! …) live in `ui::lists::rows` and are re-exported
//! here for the call sites that still import via this path.
//!
//! The generic `ListRow` vehicle that used to live in this file was removed:
//! it had been `#[deprecated]` since Wave 11b with its own note recording
//! "zero remaining callers", and a repo-wide search confirmed the only
//! `ListRow::new(` occurrence was its own doc example. 234 lines of dead code.
//! Use `ui_kit::PanelListRow` (generic rows) or a `RowShell`-based domain row.

#![allow(unused_imports)]

// Domain rows moved to lists::rows — re-export for backward compat
pub mod dom_row {
    pub use crate::chart::renderer::ui::lists::rows::dom_row::*;
}
pub mod news_row {
    pub use crate::chart::renderer::ui::lists::rows::news_row::*;
}
pub mod order_row {
    pub use crate::chart::renderer::ui::lists::rows::order_row::*;
}
pub mod watchlist_row {
    pub use crate::chart::renderer::ui::lists::rows::watchlist_row::*;
}
// Re-exports for direct items
pub use crate::chart::renderer::ui::lists::rows::*;
