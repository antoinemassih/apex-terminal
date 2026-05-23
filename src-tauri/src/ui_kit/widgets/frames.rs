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
// `kit::*` panel composites — chart-app primitives. Used by side_panel_shell /
// split_section_panel until those move out of ui_kit (audit item 4).
pub(crate) use crate::chart_renderer::ui::panels::kit::{
    PanelHeader, PanelHeaderTabs, panel_action_btn,
};
