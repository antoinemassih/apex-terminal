//! Watchlist-specific widget subcomponents — backward-compat shim.
//! `filter_pill` removed (dead code deleted in cleanup #10).

pub mod nmf_toggle {
    pub use crate::chart::renderer::ui::inputs::nmf_toggle::*;
}

pub use nmf_toggle::*;
