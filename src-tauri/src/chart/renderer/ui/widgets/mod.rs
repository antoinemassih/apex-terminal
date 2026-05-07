//! Backward-compat shim. Content has moved to ui/components/, ui/chrome/, ui/inputs/,
//! ui/lists/, ui/foundation/, ui/tools/.
//! Re-export from new locations so old `widgets::foo` paths keep compiling.

pub use super::components::*;
pub use super::chrome::*;
pub use super::inputs::*;
pub use super::foundation::*;

// Sub-module shims for `widgets::frames`, `widgets::inputs` etc.
pub mod frames {
    pub use crate::chart::renderer::ui::components::frames_widget::*;
    pub use crate::chart::renderer::ui::components::frames::*;
}
pub mod inputs {
    pub use crate::chart::renderer::ui::inputs::inputs::*;
}
pub mod text {
    pub use crate::chart::renderer::ui::components::text::*;
}
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
pub mod headers {
    pub use crate::chart::renderer::ui::components::headers::*;
    pub use crate::chart::renderer::ui::components::headers_widget::*;
}
pub mod pills {
    pub use crate::chart::renderer::ui::components::pills::*;
    pub use crate::chart::renderer::ui::components::pills_widget::*;
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
