//! Backward-compat shim. Content has moved to ui/components/, ui/chrome/, ui/inputs/,
//! ui/lists/, ui/foundation/, ui/tools/.
//!
//! Pure re-export paths (text, frames, headers, inputs) have been removed — all
//! consumers now import from the canonical locations directly.
//!
//! The following shim entries remain because `render/pane/core.rs` uses them
//! via absolute `crate::chart_renderer::ui::widgets::*` paths and that file
//! is sacred (cannot be edited).

pub use super::components::*;
pub use super::chrome::*;

// cards — kept in-place in widgets/cards/mod.rs (contains Card struct + redirects domain cards)
pub mod cards;
pub mod rows; // kept in-place; still contains ListRow. Domain rows moved to lists::rows.
pub mod foundation {
    pub use crate::chart::renderer::ui::foundation::*;
    pub mod text_style {
        pub use crate::chart::renderer::ui::foundation::text_style::*;
    }
    pub mod shell {
        pub use crate::chart::renderer::ui::foundation::shell::*;
    }
    pub mod tokens {
        pub use crate::chart::renderer::ui::foundation::tokens::*;
    }
    pub mod variants {
        pub use crate::chart::renderer::ui::foundation::variants::*;
    }
    pub mod interaction {
        pub use crate::chart::renderer::ui::foundation::interaction::*;
    }
}
pub mod drawing {
    pub use crate::chart::renderer::ui::tools::drawing::*;
}
pub mod trading {
    pub use crate::chart::renderer::ui::tools::*;
    pub mod order_edit_dialog {
        pub use crate::chart::renderer::ui::tools::order_edit_dialog::*;
    }
    pub mod order_entry_panel {
        pub use crate::chart::renderer::ui::tools::order_entry_panel::*;
    }
    pub mod pending_order_toasts {
        pub use crate::chart::renderer::ui::tools::pending_order_toasts::*;
    }
}
// watchlist shim kept in widgets/watchlist/mod.rs
pub mod watchlist;
pub mod toolbar {
    pub use crate::chart::renderer::ui::components::toolbar::*;
}
