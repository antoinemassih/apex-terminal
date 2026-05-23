//! Frame + dialog-header widgets — re-exported from chart_renderer's
//! `ui::components` modules.
//!
//! Part of the UI extraction (see `docs/UI_EXTRACTION.md`). Centralises the
//! `chart_renderer` import in one ui_kit file so the rest of the widgets can
//! reach these primitives via a ui_kit path.
#![allow(unused_imports)]
pub use crate::chart_renderer::ui::components::frames_widget::*;
pub use crate::chart_renderer::ui::components::{
    DialogHeaderWithClose, PaneHeaderWithClose, PanelHeaderWithClose, PopupFrame,
};
pub use crate::chart_renderer::ui::components::text::SectionLabelSize;
// `kit::PanelHeader` / `PanelHeaderTabs` / `panel_action_btn` re-exports
// removed — the side_panel_shell + split_section_panel widgets that used
// them moved to `chart_renderer::ui::panels` so they now import directly.
