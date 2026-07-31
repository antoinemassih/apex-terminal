//! Frame + pane/panel-header widgets — re-exported from chart_renderer's
//! `ui::components` modules.
//!
//! Part of the UI extraction (see `docs/UI_EXTRACTION.md`). Centralises the
//! `chart_renderer` import in one ui_kit file so the rest of the widgets can
//! reach these primitives via a ui_kit path.
//!
//! ### KNOWN LAYER VIOLATION — do not add to it
//! This is the LAST `chart_renderer` import in `ui_kit/widgets/` and is
//! deliberately quarantined here. `ui_kit/layer_guard.rs` pins it (see its
//! `KNOWN_VIOLATIONS` ratchet): the guard test fails if a new
//! `chart_renderer::` reference appears in any other ui_kit file, or if the
//! count in this file changes in either direction.
//!
//! Why it cannot be inverted yet:
//!   * `PanelFrame` / `CardFrame` / `PopupFrame` / `CompactPanelFrame` read the
//!     chart `StyleSettings` via `chart_renderer::ui::style::current()` for
//!     `hairline_borders`, `shadows_enabled`, `shadow_blur`, `shadow_offset_y`
//!     and `card_padding_*`. None of those exist on `ui_kit`'s `TokenSnapshot`,
//!     so porting them means growing the snapshot AND the chart-side
//!     `begin_frame()` that populates it.
//!   * `PanelHeaderWithClose` delegates to `chart_renderer::ui::panels::kit::
//!     PanelHeader`, which is hard-coupled to `gpu::Watchlist` (pane-header
//!     metrics) and `gpu::Theme` — not expressible through `ComponentTheme`.
//!
//! `DialogHeaderWithClose` USED to be re-exported here too; it was inverted in
//! P6 and now lives at `ui_kit::widgets::DialogHeader`, with the legacy name a
//! delegating wrapper around it. That is the pattern the remaining entries
//! should follow when their chart coupling is unwound.
#![allow(unused_imports)]
pub use crate::chart_renderer::ui::components::frames_widget::*;
pub use crate::chart_renderer::ui::components::{
    PaneHeaderWithClose, PanelHeaderWithClose, PopupFrame,
};
pub use crate::chart_renderer::ui::components::text::SectionLabelSize;
// `kit::PanelHeader` / `PanelHeaderTabs` / `panel_action_btn` re-exports
// removed — the side_panel_shell + split_section_panel widgets that used
// them moved to `chart_renderer::ui::panels` so they now import directly.
