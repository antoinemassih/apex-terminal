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
pub(crate) use core::{render_toolbar, draw_chart};
