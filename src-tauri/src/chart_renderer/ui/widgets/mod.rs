//! Builder + `impl Widget` design-system primitives.
//!
//! Each file in this module owns one component family. New code should call
//! `ui.add(<Widget>::new(...).<knob>().theme(t))` instead of the legacy
//! positional-arg helpers in `components.rs` / `components_extra.rs` /
//! `style.rs`. The legacy helpers still work — they delegate to the same
//! paint code — so migration is incremental.
//!
//! Pattern: `Foo::new("label").primary().small().theme(t)` parsed by builder
//! methods on the struct, then `impl Widget` runs the paint when consumed by
//! `ui.add(...)`.

pub mod buttons;
pub mod cards;
pub mod frames;
pub mod headers;
pub mod pills;
pub mod rows;
pub mod tabs;
pub mod toolbar;
pub mod menus;
pub mod inputs;
pub mod text;
pub mod modal;
pub mod context_menu;
pub mod select;
pub mod layout;
pub mod form;
pub mod pane;
pub mod painter_pane;
pub mod status;
pub mod foundation;
pub mod icons;
pub mod watchlist;
pub mod perf_hud;
