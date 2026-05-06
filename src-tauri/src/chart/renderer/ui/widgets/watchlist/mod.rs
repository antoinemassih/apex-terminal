//! Watchlist-specific widget subcomponents — backward-compat shim.

pub mod filter_pill {
    pub use crate::chart::renderer::ui::inputs::filter_pill::*;
}
pub mod nmf_toggle {
    pub use crate::chart::renderer::ui::inputs::nmf_toggle::*;
}
pub mod section_header {
    pub use crate::chart::renderer::ui::watchlist::section_header::*;
}

pub use filter_pill::*;
pub use nmf_toggle::*;
pub use section_header::*;
