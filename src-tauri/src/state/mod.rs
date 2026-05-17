//! Wave 5: focused state aggregates that decompose the `Watchlist` god-object.
//!
//! Wave 5 introduces the *contracts* — `SubscriptionBus` for cross-pane
//! events, `InFlightRegistry` for centralized loading state, `Persistable`
//! for versioned save/load, and skeleton aggregate structs whose fields
//! migrate out of `Watchlist` in follow-up waves. The sacred GPU paint
//! pipeline (`chart::renderer::render::pane::core.rs`) is untouched, and
//! `Watchlist`'s public field list is preserved so every existing caller
//! still compiles.
//!
//! New consumers should reach for these aggregates first. Legacy
//! `*_loading` flags and ad-hoc pull-based pane-iteration logic are left
//! in place to keep the diff small and reviewable.

pub mod aggregates;
pub mod inflight;
pub mod persistence;
pub mod subscriptions;

pub use aggregates::{
    AlertsState, ChatState, LayoutState, SidebarState, TradingDefaults, UiSettings,
};
pub use inflight::{InFlight, InFlightKind, InFlightRegistry, RequestId};
pub use persistence::{load, save, Persistable};
pub use subscriptions::{PaneEvent, SubscriptionBus};
