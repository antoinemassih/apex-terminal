//! Command / event flow — the centralized dispatch layer.
//!
//! UI components emit `AppCommand`s instead of mutating state inline.
//! The top-level draw loop drains the queue at end-of-frame and reduces
//! every command into the global state via `dispatch()`. Benefits:
//!
//! 1. Single place to log / debug / replay every state change
//! 2. Same command can be triggered from button click, hotkey, Stream Deck,
//!    voice, MCP, etc. — wire once, dispatched everywhere
//! 3. Components become pure-ish — no business logic interleaved with paint
//! 4. State invariants live next to the reducer, not scattered across UI
//!
//! Pattern:
//! ```ignore
//! // In component:
//! if ui.add(ActionButton::new("Cancel").destructive().theme(t)).clicked() {
//!     commands::push(AppCommand::CancelAlert { pane: ap, id: alert.id });
//! }
//!
//! // In draw_chart end-of-frame:
//! commands::drain_and_dispatch(panes, watchlist);
//! ```
//!
//! This file deliberately starts SMALL. New commands get added as panels
//! migrate. Inline mutations and command emissions coexist during the
//! transition — both work, no big-bang refactor required.

use crate::chart_renderer::gpu::{Chart, Theme, Watchlist, IndicatorType, Indicator, PaneType};
use crate::chart_renderer::trading::{Alert, OrderStatus, PriceAlert, cancel_order_with_pair};

// ─── UiCtx ─────────────────────────────────────────────────────────────────
// A single bundle of UI context that flows through every component instead of
// passing `t: &Theme` (and eventually `&UiState`, `&dispatch_fn`, etc.) as
// separate args. Components call `cx.dispatch(AppCommand::Foo)` to emit
// commands and access theme colors via `cx.accent` (auto-deref).
//
// Phase 3 of the design-system roadmap. New panels/components should accept
// `cx: &UiCtx<'_>` instead of `t: &Theme`. Old call sites continue to work —
// UiCtx is additive, not a breaking change.

pub(crate) struct UiCtx<'a> {
    pub(crate) theme: &'a Theme,
}

impl<'a> UiCtx<'a> {
    /// Construct from the active theme. Cheap — just borrows.
    #[inline]
    pub(crate) fn new(theme: &'a Theme) -> Self { Self { theme } }

    /// Emit an AppCommand. Same as `commands::push(cmd)`.
    #[inline]
    pub(crate) fn dispatch(&self, cmd: AppCommand) { push(cmd); }
}

impl<'a> std::ops::Deref for UiCtx<'a> {
    type Target = Theme;
    /// `cx.accent` works through deref — no need for `cx.theme.accent`.
    #[inline]
    fn deref(&self) -> &Theme { self.theme }
}

// ─── AppCommand enum ───────────────────────────────────────────────────────
// Every action a UI surface can request. Append-only — adding variants is a
// non-breaking change. Variants name their *intent*, not their *side effect*.

#[derive(Debug, Clone)]
pub enum AppCommand {
    // ── Alerts ───────────────────────────────────────────────────────────
    /// Create a price alert above/below a price for the given pane's symbol.
    AddPriceAlert {
        pane: usize,
        price: f32,
        above: bool,
    },
    /// Promote a draft alert to active (place it).
    PlaceDraftAlert {
        pane: usize,
        id: u32,
    },
    /// Promote every draft across every pane to active.
    PlaceAllDraftAlerts,
    /// Cancel / dismiss a per-pane price alert.
    CancelPaneAlert {
        pane: usize,
        id: u32,
    },
    /// Cancel a watchlist-level (cross-pane) alert by id.
    CancelWatchlistAlert {
        id: u32,
    },
    /// Snooze a triggered alert (un-trigger so it fires again).
    SnoozeAlert {
        pane: usize,
        id: u32,
    },

    // ── Orders ───────────────────────────────────────────────────────────
    /// Cancel a single order on a pane (also cancels its paired bracket leg).
    CancelOrder {
        pane: usize,
        id: u32,
    },
    /// Promote every draft order across every pane to placed.
    PlaceAllDraftOrders,
    /// Cancel every active (draft or placed) order across every pane.
    CancelAllOrders,
    /// Remove executed/cancelled order rows from history across every pane.
    ClearOrderHistory,
    /// Promote the selected (pane, id) order set from draft to placed (incl. bracket legs).
    PlaceSelectedOrders,
    /// Cancel the selected (pane, id) order set (incl. bracket legs).
    CancelSelectedOrders,

    // ── Indicators ───────────────────────────────────────────────────────
    /// Append a new indicator of `kind` to a pane. Color is auto-assigned
    /// from `INDICATOR_COLORS`; period defaults from `IndicatorType`.
    /// Also opens the editor for the freshly-added indicator.
    AddIndicator { pane: usize, kind: IndicatorType },
    /// Remove an indicator by id from a pane.
    RemoveIndicator { pane: usize, id: u32 },
    /// Toggle the `visible` flag for an indicator on a pane.
    ToggleIndicatorVisibility { pane: usize, id: u32 },
    /// Reorder an indicator within a pane (move from index → to index).
    MoveIndicator { pane: usize, from: usize, to: usize },
    /// Open the indicator editor popup for an indicator id.
    OpenIndicatorEditor { pane: usize, id: u32 },
    /// Close the indicator editor popup on a pane.
    CloseIndicatorEditor { pane: usize },
    /// Mark indicators on a pane as needing recompute (clears cached counter).
    RecomputeIndicators { pane: usize },

    // ── Pane / layout ────────────────────────────────────────────────────
    /// Switch a pane's `pane_type` (Chart / Portfolio / Heatmap / Dashboard).
    ChangePaneType { pane: usize, kind: PaneType },
    /// Swap the symbol shown by a pane. Reducer also flags
    /// `pending_symbol_change` so the bar fetch can be triggered downstream.
    SwapPaneSymbol { pane: usize, symbol: String },
    /// Change a pane's timeframe.
    ChangeTimeframe { pane: usize, tf: String },

    // ── Watchlist (domain mutations) ────────────────────────────────────
    /// Add a symbol to the active watchlist (de-dup'd, lands in last stock section).
    WatchlistAddSymbol { symbol: String },
    /// Remove a symbol from every section of the active watchlist.
    WatchlistRemoveSymbol { symbol: String },
    /// Move an item between (or within) sections by index.
    WatchlistMoveItem { src_sec: usize, src_idx: usize, dst_sec: usize, dst_idx: usize },
    /// Add a new (empty) stock section.
    WatchlistAddSection { title: String },
    /// Add a new (empty) options section.
    WatchlistAddOptionSection { title: String },
    /// Remove a section by index — only if empty.
    WatchlistRemoveSection { idx: usize },
    /// Toggle the collapse state of a section.
    WatchlistToggleSectionCollapse { idx: usize },
    /// Set (or clear with None) the color hex of a section by id.
    WatchlistSetSectionColor { sec_id: u32, hex: Option<String> },
    /// Rename a section by id.
    WatchlistRenameSection { sec_id: u32, title: String },
    /// Toggle the pinned flag of an item.
    WatchlistTogglePinned { sec_idx: usize, item_idx: usize },
    /// Force-unpin an item (used by the pinned strip's left-edge click).
    WatchlistUnpinItem { sec_idx: usize, item_idx: usize },
    /// Add an option contract to the active watchlist.
    WatchlistAddOption { sym: String, strike: f32, is_call: bool, expiry: String, bid: f32, ask: f32 },
    /// Create a new watchlist with the given name and switch to it.
    WatchlistCreate { name: String },
    /// Delete a watchlist by index (no-op if it would empty the list).
    WatchlistDelete { idx: usize },
    /// Duplicate a watchlist by index and switch to the copy.
    WatchlistDuplicate { idx: usize },
    /// Switch the active watchlist by index.
    WatchlistSwitchActive { idx: usize },
    /// Rename the currently-active watchlist.
    WatchlistRenameActive { name: String },
}

// ─── CommandQueue (thread-local, drained per frame) ────────────────────────
// Thread-local so components don't have to thread `&mut CommandQueue` through
// every function signature. Frame-scoped: drain happens at end of draw_chart.

std::thread_local! {
    static QUEUE: std::cell::RefCell<Vec<AppCommand>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Emit a command from anywhere in the UI tree. Cheap; just pushes onto a
/// per-thread Vec.
pub fn push(cmd: AppCommand) {
    QUEUE.with(|q| q.borrow_mut().push(cmd));
}

/// Drain the queue and dispatch every command. Call once per frame at the
/// END of draw_chart (after all UI has had a chance to push).
pub fn drain_and_dispatch(panes: &mut [Chart], watchlist: &mut Watchlist) {
    let cmds: Vec<AppCommand> = QUEUE.with(|q| std::mem::take(&mut *q.borrow_mut()));
    for cmd in cmds {
        dispatch(panes, watchlist, cmd);
    }
}

/// Reducer — every state change lives here. Purely a state mutation; no
/// side effects (no logging, no IO, no spawning) unless commented otherwise.
fn dispatch(panes: &mut [Chart], watchlist: &mut Watchlist, cmd: AppCommand) {
    match cmd {
        AppCommand::AddPriceAlert { pane, price, above } => {
            let Some(p) = panes.get_mut(pane) else { return; };
            let sym = p.symbol.clone();
            let wl_id = watchlist.next_alert_id;
            watchlist.next_alert_id += 1;
            watchlist.alerts.push(Alert {
                id: wl_id,
                symbol: sym.clone(),
                price,
                above,
                triggered: false,
                message: String::new(),
            });
            let pid = p.next_alert_id;
            p.next_alert_id += 1;
            p.price_alerts.push(PriceAlert {
                id: pid,
                price,
                above,
                triggered: false,
                draft: false,
                symbol: sym,
            });
            p.alert_input_price.clear();
        }

        AppCommand::PlaceDraftAlert { pane, id } => {
            if let Some(p) = panes.get_mut(pane) {
                if let Some(a) = p.price_alerts.iter_mut().find(|a| a.id == id) {
                    a.draft = false;
                }
            }
        }

        AppCommand::PlaceAllDraftAlerts => {
            for p in panes.iter_mut() {
                for a in p.price_alerts.iter_mut() {
                    if a.draft { a.draft = false; }
                }
            }
        }

        AppCommand::CancelPaneAlert { pane, id } => {
            if let Some(p) = panes.get_mut(pane) {
                p.price_alerts.retain(|a| a.id != id);
            }
        }

        AppCommand::CancelWatchlistAlert { id } => {
            watchlist.alerts.retain(|a| a.id != id);
        }

        AppCommand::SnoozeAlert { pane, id } => {
            if let Some(p) = panes.get_mut(pane) {
                if let Some(a) = p.price_alerts.iter_mut().find(|a| a.id == id) {
                    a.triggered = false;
                }
            }
        }

        AppCommand::CancelOrder { pane, id } => {
            if let Some(p) = panes.get_mut(pane) {
                cancel_order_with_pair(&mut p.orders, id);
            }
        }

        AppCommand::PlaceAllDraftOrders => {
            for p in panes.iter_mut() {
                for o in &mut p.orders {
                    if o.status == OrderStatus::Draft { o.status = OrderStatus::Placed; }
                }
            }
        }

        AppCommand::CancelAllOrders => {
            for p in panes.iter_mut() {
                for o in &mut p.orders {
                    if o.status == OrderStatus::Draft || o.status == OrderStatus::Placed {
                        o.status = OrderStatus::Cancelled;
                    }
                }
            }
        }

        AppCommand::ClearOrderHistory => {
            for p in panes.iter_mut() {
                p.orders.retain(|o| o.status == OrderStatus::Draft || o.status == OrderStatus::Placed);
            }
        }

        AppCommand::PlaceSelectedOrders => {
            // snapshot selection before mutating panes (selection is on watchlist).
            let sel = watchlist.selected_order_ids.clone();
            for (pi, oid) in &sel {
                if let Some(pane) = panes.get_mut(*pi) {
                    // resolve pair_id while we have access to the order
                    let pair_id = pane.orders.iter().find(|o| o.id == *oid).and_then(|o| o.pair_id);
                    if let Some(o) = pane.orders.iter_mut().find(|o| o.id == *oid) {
                        if o.status == OrderStatus::Draft { o.status = OrderStatus::Placed; }
                    }
                    if let Some(pid) = pair_id {
                        if let Some(p) = pane.orders.iter_mut().find(|o| o.id == pid) {
                            if p.status == OrderStatus::Draft { p.status = OrderStatus::Placed; }
                        }
                    }
                }
            }
            watchlist.selected_order_ids.clear();
        }

        AppCommand::CancelSelectedOrders => {
            let sel = watchlist.selected_order_ids.clone();
            for (pi, oid) in &sel {
                if let Some(pane) = panes.get_mut(*pi) {
                    cancel_order_with_pair(&mut pane.orders, *oid);
                }
            }
            watchlist.selected_order_ids.clear();
        }

        // ── Watchlist domain ────────────────────────────────────────────
        AppCommand::WatchlistAddSymbol { symbol } => {
            watchlist.add_symbol(&symbol);
            crate::chart_renderer::gpu::fetch_watchlist_prices(vec![symbol.to_uppercase()]);
            watchlist.persist();
        }

        AppCommand::WatchlistRemoveSymbol { symbol } => {
            watchlist.remove_symbol(&symbol);
            watchlist.persist();
        }

        AppCommand::WatchlistMoveItem { src_sec, src_idx, dst_sec, dst_idx } => {
            watchlist.move_item(src_sec, src_idx, dst_sec, dst_idx);
            watchlist.persist();
        }

        AppCommand::WatchlistAddSection { title } => {
            watchlist.add_section(&title);
            watchlist.persist();
        }

        AppCommand::WatchlistAddOptionSection { title } => {
            watchlist.add_option_section(&title);
            watchlist.persist();
        }

        AppCommand::WatchlistRemoveSection { idx } => {
            if idx < watchlist.sections.len() && watchlist.sections[idx].items.is_empty() {
                watchlist.sections.remove(idx);
                watchlist.persist();
            }
        }

        AppCommand::WatchlistToggleSectionCollapse { idx } => {
            if let Some(sec) = watchlist.sections.get_mut(idx) {
                sec.collapsed = !sec.collapsed;
                watchlist.persist();
            }
        }

        AppCommand::WatchlistSetSectionColor { sec_id, hex } => {
            if let Some(sec) = watchlist.sections.iter_mut().find(|s| s.id == sec_id) {
                sec.color = hex;
                watchlist.persist();
            }
        }

        AppCommand::WatchlistRenameSection { sec_id, title } => {
            if let Some(sec) = watchlist.sections.iter_mut().find(|s| s.id == sec_id) {
                sec.title = title;
                watchlist.persist();
            }
        }

        AppCommand::WatchlistTogglePinned { sec_idx, item_idx } => {
            if let Some(sec) = watchlist.sections.get_mut(sec_idx) {
                if let Some(item) = sec.items.get_mut(item_idx) {
                    item.pinned = !item.pinned;
                    // (no persist — matches existing inline behavior)
                }
            }
        }

        AppCommand::WatchlistUnpinItem { sec_idx, item_idx } => {
            if let Some(sec) = watchlist.sections.get_mut(sec_idx) {
                if let Some(item) = sec.items.get_mut(item_idx) {
                    item.pinned = false;
                }
            }
        }

        AppCommand::WatchlistAddOption { sym, strike, is_call, expiry, bid, ask } => {
            watchlist.add_option_to_watchlist(&sym, strike, is_call, &expiry, bid, ask);
            watchlist.persist();
        }

        AppCommand::WatchlistCreate { name } => {
            let syms = watchlist.create_watchlist(&name);
            if !syms.is_empty() {
                crate::chart_renderer::gpu::fetch_watchlist_prices(syms);
            }
        }

        AppCommand::WatchlistDelete { idx } => {
            let syms = watchlist.delete_watchlist(idx);
            if !syms.is_empty() {
                crate::chart_renderer::gpu::fetch_watchlist_prices(syms);
            }
        }

        AppCommand::WatchlistDuplicate { idx } => {
            let syms = watchlist.duplicate_watchlist(idx);
            if !syms.is_empty() {
                crate::chart_renderer::gpu::fetch_watchlist_prices(syms);
            }
        }

        AppCommand::WatchlistSwitchActive { idx } => {
            let syms = watchlist.switch_to(idx);
            if !syms.is_empty() {
                crate::chart_renderer::gpu::fetch_watchlist_prices(syms);
            }
        }

        // ── Indicators ──────────────────────────────────────────────────
        AppCommand::AddIndicator { pane, kind } => {
            let Some(p) = panes.get_mut(pane) else { return; };
            let color = crate::chart_renderer::gpu::INDICATOR_COLORS[
                p.indicators.len() % crate::chart_renderer::gpu::INDICATOR_COLORS.len()];
            let id = p.next_indicator_id;
            p.next_indicator_id += 1;
            p.indicators.push(Indicator::new(id, kind, kind.default_period(), color));
            p.editing_indicator = Some(id);
            p.indicator_bar_count = 0;
        }

        AppCommand::RemoveIndicator { pane, id } => {
            if let Some(p) = panes.get_mut(pane) {
                p.indicators.retain(|i| i.id != id);
                if p.editing_indicator == Some(id) { p.editing_indicator = None; }
                p.indicator_bar_count = 0;
            }
        }

        AppCommand::ToggleIndicatorVisibility { pane, id } => {
            if let Some(p) = panes.get_mut(pane) {
                if let Some(ind) = p.indicators.iter_mut().find(|i| i.id == id) {
                    ind.visible = !ind.visible;
                }
            }
        }

        AppCommand::MoveIndicator { pane, from, to } => {
            if let Some(p) = panes.get_mut(pane) {
                if from < p.indicators.len() && to < p.indicators.len() && from != to {
                    let item = p.indicators.remove(from);
                    p.indicators.insert(to, item);
                }
            }
        }

        AppCommand::OpenIndicatorEditor { pane, id } => {
            if let Some(p) = panes.get_mut(pane) {
                p.editing_indicator = Some(id);
            }
        }

        AppCommand::CloseIndicatorEditor { pane } => {
            if let Some(p) = panes.get_mut(pane) {
                p.editing_indicator = None;
            }
        }

        AppCommand::RecomputeIndicators { pane } => {
            if let Some(p) = panes.get_mut(pane) {
                p.indicator_bar_count = 0;
            }
        }

        // ── Pane / layout ───────────────────────────────────────────────
        AppCommand::ChangePaneType { pane, kind } => {
            if let Some(p) = panes.get_mut(pane) {
                p.pane_type = kind;
            }
        }

        AppCommand::SwapPaneSymbol { pane, symbol } => {
            if let Some(p) = panes.get_mut(pane) {
                p.symbol = symbol.clone();
                p.pending_symbol_change = Some(symbol);
            }
        }

        AppCommand::ChangeTimeframe { pane, tf } => {
            if let Some(p) = panes.get_mut(pane) {
                p.timeframe = tf;
            }
        }

        AppCommand::WatchlistRenameActive { name } => {
            let trimmed = name.trim();
            if !trimmed.is_empty() {
                let active = watchlist.active_watchlist_idx;
                if let Some(wl) = watchlist.saved_watchlists.get_mut(active) {
                    wl.name = trimmed.to_string();
                }
                watchlist.persist();
            }
        }
    }
}
