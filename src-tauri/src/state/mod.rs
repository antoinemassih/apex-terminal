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
    AlertsState, ChatState, CmdPaletteState, DefaultOrderType, DefaultTimeInForce, LayoutState,
    PersistedAlert, PersistedLinkGroup, SidebarState, TradingDefaults, UiSettings,
};
pub use inflight::{InFlight, InFlightKind, InFlightRegistry, RequestId};
pub use persistence::{atomic_write, load, save, Persistable};
pub use subscriptions::{PaneEvent, PaneToggle, SubscriptionBus, BROADCAST_GROUP};

pub mod store;
pub mod persistable_store;
pub mod store_registry;
pub mod persist_supervisor;

pub use store::{Store, DEBOUNCE_MS};
pub use persistable_store::PersistableStore;
pub use store_registry::StoreRegistry;
pub use persist_supervisor::{spawn as spawn_persist_supervisor, TICK_MS as PERSIST_TICK_MS};
