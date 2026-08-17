//! Chart pane rendering.
//!
//! ## `core.rs` is SACRED
//!
//! `core.rs` contains the GPU-optimized chart paint pipeline. It is the
//! hottest code path in the app — sub-millisecond per-frame budgets,
//! tight inlining, deeply shared local state. **Do not refactor it
//! without per-frame benchmark evidence of zero regression.**
//!
//! Specifically:
//! - No mechanical token-compliance sweeps inside `core.rs`. Literals
//!   stay until a performance-conscious owner replaces them with
//!   benchmark cover.
//! - No "for cleanliness" function extractions. Function call overhead,
//!   lost inlining, and parameter passing can manifest as frame drops.
//! - No multi-agent fanout inside this file. One owner at a time, with
//!   the ability to profile a frame before merging.
//!
//! Multi-agent design-system work runs on everything else in the tree
//! (panels, tools, widgets, drawing tool modal UI). The chart paint
//! pipeline stays single-owner.
//!
//! See `docs/PANE_RS_SPLIT_PLAN.md` for the original split proposal and
//! the decision to defer it indefinitely.

mod core;
// The options-chain grid (watchlist_panel) publishes the rows it draws here so
// the quote-seat request asks for exactly those contracts. Re-exported rather
// than making the whole module public.
pub(crate) use core::{chain_visible_add, chain_visible_begin};
mod deferred;
mod pane_context_menu;
mod keyboard_shortcuts;
mod tool_previews;
mod signal_gauges;
pub(crate) mod notice;

// Re-export the public entry points so existing call sites (`use
// crate::chart::renderer::render::pane::{render_toolbar, draw_chart}`)
// keep compiling. As sub-systems get extracted in later waves, their
// re-exports get added here too.
// `render_toolbar` is re-exported for the LEGACY egui render path
// (`--no-default-features`), where it is the entry point. On the default build
// nothing calls it, so an "unused import" sweep will offer to delete it — and
// one did, which broke that build in CI while the default build stayed green.
//
// A `pub use` is API surface, not an import. Unused-ness of a re-export is a
// statement about THIS build's features, never about whether it is needed.
pub(crate) use core::{render_toolbar, draw_chart};
