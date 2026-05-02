//! Watchlist-specific widget subcomponents.
//!
//! These components are only used inside `watchlist_panel.rs` and have been
//! extracted here to reduce duplication (each pattern appeared 2+ times in
//! the panel). They follow the same builder pattern as all other widgets.

pub mod filter_pill;
pub mod section_header;
pub mod nmf_toggle;

pub use filter_pill::FilterPill;
pub use section_header::SectionHeader;
pub use nmf_toggle::NmfToggle;
