//! Panel — the contract every side/dock/modal panel in the app implements.
//!
//! ## Why this exists
//!
//! Today the ~35 panels under `chart/renderer/ui/panels/` are each a free
//! function with a slightly different signature (`pub(crate) fn draw(ctx,
//! watchlist, panes, ap, t)`). Each panel mixes layout, data mutation,
//! event handling, and paint. Without a contract:
//!
//! - Multi-agent work on different panels is hard — every agent has to
//!   re-derive the panel lifecycle.
//! - Cross-cutting concerns (hotkeys, focus, persistence) can't be
//!   threaded through a unified surface.
//! - New panels reinvent the structure of old ones.
//!
//! This trait is a **migration target**, not a forced rewrite. Existing
//! panels keep working. New panels should `impl Panel`. As old panels
//! get touched, migrate them.
//!
//! ## The shape
//!
//! ```ignore
//! impl Panel for MyPanel<'_> {
//!     fn id(&self) -> &'static str { "my_panel" }
//!     fn render(&mut self, ctx: PanelCtx) -> PanelResponse {
//!         // layout, paint, interaction
//!         PanelResponse::default()
//!     }
//! }
//! ```
//!
//! ## What PanelCtx gives you
//!
//! - The egui context + theme (always needed).
//! - The active panel rect, if the host computed one. Lets the panel
//!   participate in the host's layout rather than allocating its own.
//! - A `dyn PanelHost` handle for cross-panel signals (open another
//!   panel, close self, request focus). Host-defined.
//!
//! ## What PanelResponse signals back
//!
//! - `request_close` — panel wants to close itself.
//! - `request_focus` — panel wants the next interaction routed to it.
//! - `dirty` — panel touched persistent state; the host should save.
//! - `consumed_hotkey` — panel handled a keypress; don't route further.

use egui::Context;

use crate::ui_kit::widgets::theme::ComponentTheme;

/// Context handed to a panel each render. Held by value because every
/// field is cheap to copy — `Context` is `Arc`-backed inside; the
/// theme is held as `&dyn ComponentTheme` so panels work with any
/// `ComponentTheme` implementor (chart-app `Theme` or `PortableTheme`).
pub struct PanelCtx<'a> {
    pub egui: &'a Context,
    pub theme: &'a dyn ComponentTheme,
}

impl<'a> PanelCtx<'a> {
    pub fn new(egui: &'a Context, theme: &'a dyn ComponentTheme) -> Self {
        Self { egui, theme }
    }
}

/// Signals a panel sends back to the host after one render pass.
#[derive(Default, Debug, Clone, Copy)]
pub struct PanelResponse {
    /// Panel wants to be closed (user clicked the X, hit Escape, etc.).
    pub request_close: bool,
    /// Panel wants focus on the next frame (e.g., text input was just
    /// activated and the host should ensure it gets keyboard events).
    pub request_focus: bool,
    /// Panel touched persistent state. Host should call its save path.
    pub dirty: bool,
    /// Panel handled a keypress; host should not route it further.
    pub consumed_hotkey: bool,
}

impl PanelResponse {
    pub fn close() -> Self {
        Self { request_close: true, ..Default::default() }
    }
    pub fn dirty() -> Self {
        Self { dirty: true, ..Default::default() }
    }
    /// Merge another response into this one (logical OR on every flag).
    /// For panels that delegate to sub-panels and want to combine signals.
    pub fn merge(&mut self, other: PanelResponse) {
        self.request_close   |= other.request_close;
        self.request_focus   |= other.request_focus;
        self.dirty           |= other.dirty;
        self.consumed_hotkey |= other.consumed_hotkey;
    }
}

/// The contract. Every dockable / floating / side panel in the app
/// should implement this. Free-function panels (the legacy `pub fn
/// draw(...)` style) are the migration source — wrap them in a struct
/// that implements `Panel` and the wiring is done.
pub trait Panel {
    /// Stable identifier — used for persistence, focus tracking, hotkey
    /// routing, debug overlays. Must be unique per panel kind. Convention:
    /// lowercase snake_case matching the panel's file name.
    fn id(&self) -> &'static str;

    /// Render one frame. Return a `PanelResponse` describing any cross-
    /// cutting signals the host should act on (close, save, focus).
    /// Return `PanelResponse::default()` for "no signal."
    fn render(&mut self, ctx: PanelCtx<'_>) -> PanelResponse;

    /// Optional human-readable title. Hosts that auto-build a window
    /// chrome (the floating panel host, etc.) use this. Returns `None`
    /// for panels that paint their own header.
    fn title(&self) -> Option<&str> { None }
}
