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

use crate::chart_renderer::gpu::{Chart, Theme, Watchlist, IndicatorType, Indicator, PaneType, get_theme, indicator_default_color};
use crate::chart_renderer::trading::{OrderStatus, PriceAlert, cancel_order_with_pair};
#[cfg(debug_assertions)]
use crate::chart_renderer::trading::{OrderLevel, OrderSide, OrderState};

// ─── ChartFlag ────────────────────────────────────────────────────────────────
// Enum covering every user-facing per-pane display boolean. Only the ~10 most
// clearly user-visible toggles are enumerated here in Phase 4; the remaining
// ~40+ booleans on Chart still live as direct field writes. This list grows
// incrementally — see STATE_ROADMAP.md § Phase 4.
//
// NOT included (intentionally): internal/transient flags (drag_zoom_active,
// picker_searching, history_loading, drawings_requested, etc.), order-UI flags
// (order_is_buy, order_market, order_bracket, etc.), and any field that is
// written inside core.rs's paint path. Those stay as direct writes for now.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChartFlag {
    // ── Core display ─────────────────────────────────────────────────────
    /// Show/hide the volume-bars sub-row at the bottom of the chart.
    ShowVolume,
    /// Logarithmic price axis.
    LogScale,
    /// Snap drawings and crosshair to nearest OHLC level.
    Magnet,
    /// Show OHLC values at crosshair position.
    OhlcTooltip,
    /// Show distance-only measurement tooltip.
    MeasureTooltip,
    // ── Overlay / sub-panel toggles ───────────────────────────────────────
    /// Toggle the oscillator sub-panel (RSI / MACD / etc.).
    ShowOscillators,
    /// Show previous session close + open lines.
    ShowPrevClose,
    /// Annotate detected chart patterns with labels.
    ShowPatternLabels,
    /// Show volume footprint on bar hover.
    ShowFootprint,
    // ── Bulk visibility ───────────────────────────────────────────────────
    /// Hide every indicator on this pane (global mute).
    HideAllIndicators,
    /// Hide all user drawings.
    HideAllDrawings,
    // ── Overlays (gamma + options strikes) ───────────────────────────────
    /// Show/hide the GEX gamma-levels overlay.
    ShowGamma,
    /// Show/hide the options strikes overlay on the chart.
    ShowStrikesOverlay,
}

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
    /// from the active theme palette via `indicator_default_color`; period defaults from `IndicatorType`.
    /// Also opens the editor for the freshly-added indicator.
    AddIndicator { pane: usize, kind: IndicatorType },
    /// W3-02: add a Rhai script indicator with the given source (evaluated in
    /// recompute_indicators). The script panel (slice 2) dispatches this.
    AddScriptIndicator { pane: usize, src: String },
    /// Remove an indicator by id from a pane.
    RemoveIndicator { pane: usize, id: u32 },
    /// Remove ALL indicators from a pane (clean slate).
    ClearIndicators { pane: usize },
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

    // ── Per-pane display flag toggles ────────────────────────────────────
    /// Set a named per-pane boolean display flag to `value`.
    ///
    /// Phase-4 command: replaces scattered `panes[ap].<field> = !panes[ap].<field>`
    /// inline writes in UI code. Reducer bounds-checks `pane` and applies the
    /// field write. Behaviour-equivalent to the old inline write — the command
    /// queue drains in the same frame.
    SetChartFlag {
        pane: usize,
        flag: ChartFlag,
        value: bool,
    },

    // ── Settings (domain preference toggles) ─────────────────────────────
    /// Switch the active theme by index into `THEMES`. Applies to every pane
    /// (theme is conceptually app-wide; per-pane storage is an implementation
    /// detail).
    SetThemeIdx { pane: usize, idx: usize },
    /// Switch the active style preset by index. Stored on the watchlist as
    /// `style_idx` and consumed by the renderer via `style::set_active_style`.
    SetStyleIdx { idx: usize },

    // ── Layout ───────────────────────────────────────────────────────────
    /// Switch the pane-layout TEMPLATE on the live app (grows/shrinks the
    /// pane array to the template's pane count).
    ///
    /// `layout` is a permissive label — every `Layout::label()` string plus
    /// the descriptive aliases the dev harness uses ("Single", "TwoColumns",
    /// "two_rows", "quad", "2x2", …); see `Layout::from_label`.
    ///
    /// DEFERRED: `dispatch()` receives panes as a `&mut [Chart]` SLICE and so
    /// cannot push or remove panes. The handler therefore only parks the
    /// parsed `Layout` in `gpu::PENDING_LAYOUT`; the render loop drains it
    /// where the real `Vec<Chart>` is in scope. Same pattern as
    /// `gpu::PENDING_PANE_CLOSE`.
    SetLayoutLive { layout: String },

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

    // ── Dev Inspector ────────────────────────────────────────────────────
    /// Close every open dialog/popup across all panes and the watchlist.
    /// Injected by `POST /reset` so scenarios always start from a clean UI state.
    #[cfg(debug_assertions)]
    CloseAllDialogs,

    // ── Dev Inspector — subsystem drivers (harness-only, debug builds) ─────
    // These exist so the scenario harness can OPEN and OBSERVE UI-only
    // subsystems (DOM, scanner, RRG, order-entry panel, gamma) that have no
    // production command. They are pure state mutations that NEVER touch the
    // broker/OrderManager or spawn network IO. See dev/SUBSYSTEM_DRIVABILITY.md.
    /// Populate + show the GEX/gamma overlay via the shared feed-or-synth path.
    #[cfg(debug_assertions)]
    SynthGamma { pane: usize },
    /// Toggle the per-pane DOM ladder sidebar (auto-populates a mock ladder).
    #[cfg(debug_assertions)]
    SetDomSidebar { pane: usize, open: bool },
    /// Seed a broker-safe DRAFT order onto the pane's *visual* list only
    /// (`Chart.orders`, System A). Never calls OrderManager/broker.
    #[cfg(debug_assertions)]
    SeedDraftOrder { pane: usize, side: String, price: f32, qty: u32 },
    /// Collapse/expand the per-pane order-entry panel.
    #[cfg(debug_assertions)]
    SetOrderPanel { pane: usize, collapsed: bool },
    /// Open/close the scanner side panel.
    #[cfg(debug_assertions)]
    SetScannerOpen { open: bool },
    /// Seed a deterministic pool of scanner results (no live fetch).
    #[cfg(debug_assertions)]
    SeedScannerResults { count: usize },
    /// Open/close the RRG side panel (demo sectors render when opened).
    #[cfg(debug_assertions)]
    SetRrgOpen { open: bool },
    /// Set the RRG tail length (clamped to a sane range).
    #[cfg(debug_assertions)]
    SetRrgTail { len: usize },
    /// Seed a deterministic set of heatmap cells (no live fetch).
    #[cfg(debug_assertions)]
    SeedHeatmapCells { count: usize },
    /// Open/close the Auto-Charting side panel (front-end panel test).
    #[cfg(debug_assertions)]
    SetAutoChartPanel { open: bool },
    /// Open/close the OBJECTS (object tree) left panel. Added for dev-inspector
    /// view control (compose a deterministic layout for screenshot verification).
    SetObjectTree { open: bool },
    /// Toggle egui's built-in widget-debug overlay (Ctrl+Shift+D equivalent),
    /// so the UI inspector can be driven headlessly from the dev harness.
    SetUiDebug { on: bool },
    /// Resize the app's LOGICAL viewport, so the harness can assert layout at a
    /// given width. Goes through `egui::ViewportCommand` — an OS-level window
    /// resize does not make the app re-lay-out, which left every responsive
    /// behaviour (overflow menus, collapsing toolbars) untestable.
    SetViewportSize { w: f32, h: f32 },
    /// Set a spreadsheet cell's raw text (grows the grid as needed).
    #[cfg(debug_assertions)]
    SetCell { pane: usize, row: usize, col: usize, text: String },
    /// Open/close the Playbook side panel.
    #[cfg(debug_assertions)]
    SetPlaybookPanel { open: bool },
    /// Seed a directional Play (playbook) onto the watchlist — local only.
    #[cfg(debug_assertions)]
    SeedPlay { symbol: String, long: bool, entry: f32, target: f32, stop: f32 },
    /// Clear all plays (playbook reset).
    #[cfg(debug_assertions)]
    ClearPlays,
    /// Save the current in-memory plays to the debug store (persistence test).
    #[cfg(debug_assertions)]
    PersistPlays,
    /// Reload plays from the debug store into memory (persistence test).
    #[cfg(debug_assertions)]
    ReloadPlays,
    /// Auto-grade all plays for `symbol` against a synthetic `price` (lifecycle test).
    #[cfg(debug_assertions)]
    GradePlaysAtPrice { symbol: String, price: f32 },
    /// Set play[idx].expiry (unix seconds; <=0 clears) — for expiry grading tests.
    #[cfg(debug_assertions)]
    SetPlayExpiry { idx: usize, expiry: i64 },
    /// Set the local author handle stamped on new plays.
    #[cfg(debug_assertions)]
    SetAuthor { handle: String },
    /// Export play[idx] to the debug export file (share round-trip test).
    #[cfg(debug_assertions)]
    ExportPlay { idx: usize },
    /// Import a play from the debug export file and add it to the book.
    #[cfg(debug_assertions)]
    ImportPlay,
    /// Fork play[idx] into a new copy owned by the local author (attribution kept).
    #[cfg(debug_assertions)]
    ForkPlay { idx: usize },
    /// Publish play[idx]: status Draft/Active → Published, and add it to the feed.
    #[cfg(debug_assertions)]
    PublishPlay { idx: usize },
    /// Seed the feed with `count` deterministic published plays (feed/filter test).
    #[cfg(debug_assertions)]
    SeedFeed { count: usize },
    /// Set the feed symbol filter ("" = all).
    #[cfg(debug_assertions)]
    SetFeedFilter { symbol: String },
    /// Format play[idx] as a Discord/social embed and stash it (never posts in tests).
    #[cfg(debug_assertions)]
    SharePlayToDiscord { idx: usize },
    /// Test the snap engine: snap `price` to the nearest of `targets` within
    /// `tolerance`; stashes the result for assertion (deterministic, no chart).
    #[cfg(debug_assertions)]
    SnapTest { price: f32, targets: Vec<f32>, tolerance: f32 },
    /// Set play[idx] entry ZONE (low/high); entry_price becomes the mid, R:R recomputes.
    #[cfg(debug_assertions)]
    SetPlayZone { idx: usize, low: f32, high: f32 },
    /// Set play[idx] invalidation level (<=0 clears).
    #[cfg(debug_assertions)]
    SetPlayInvalidation { idx: usize, price: f32 },
    /// Add a scale-IN entry (price + weight) to play[idx].
    #[cfg(debug_assertions)]
    AddScaleIn { idx: usize, price: f32, pct: f32 },
    /// Clear play[idx] scale-in ladder.
    #[cfg(debug_assertions)]
    ClearScaleIns { idx: usize },
    /// Set play[idx] entry/stop/target from an inline expression (=3R / ATR / callwall / …).
    #[cfg(debug_assertions)]
    SetPlayLevelExpr { idx: usize, which: String, expr: String },
    /// Set a per-level note ("entry" | "stop" | "thesis") on play[idx].
    #[cfg(debug_assertions)]
    SetPlayNote { idx: usize, which: String, note: String },
    /// Set play[idx] entry trigger style ("stop" | "limit" | "market").
    #[cfg(debug_assertions)]
    SetPlayTrigger { idx: usize, style: String },
    /// Add an if-then branch to play[idx] (arms when price crosses `level`).
    #[cfg(debug_assertions)]
    AddBranch { idx: usize, above: bool, level: f32, long: bool, entry: f32, target: f32, stop: f32 },
    /// Clear play[idx] branches.
    #[cfg(debug_assertions)]
    ClearBranches { idx: usize },
    /// Add an instrument leg (hedge/pair/basket) to play[idx].
    #[cfg(debug_assertions)]
    AddLeg { idx: usize, symbol: String, role: String, long: bool, entry: f32, target: f32, stop: f32, weight: f32 },
    /// Clear play[idx] legs.
    #[cfg(debug_assertions)]
    ClearLegs { idx: usize },
    /// Open a multi-instrument play across panes: pane i shows leg i's symbol +
    /// levels (authoring/restore). This is the multi-pane restore flow.
    #[cfg(debug_assertions)]
    OpenPlayMultiPane { idx: usize },
    /// Add an options leg (call/put, strike, buy/sell, premium, qty) to play[idx].
    #[cfg(debug_assertions)]
    AddSpreadLeg { idx: usize, is_call: bool, strike: f32, buy: bool, price: f32, qty: u32, expiry: String },
    /// Clear play[idx] options legs.
    #[cfg(debug_assertions)]
    ClearSpreadLegs { idx: usize },
    /// Compute the options-spread payoff at underlying `price` and stash it.
    #[cfg(debug_assertions)]
    PayoffAt { idx: usize, price: f32 },
    /// Set account equity + risk-per-trade fraction (drives sizing + portfolio risk).
    #[cfg(debug_assertions)]
    SetAccountRisk { account: f32, risk_pct: f32 },

    /// Set the density appearance override (None = inherit the ambient default).
    /// Command-bus migration REFERENCE (WS-E E1): the reducer arm does BOTH the
    /// field write and the global style-cache side-effect that settings_panel
    /// used to do inline. See docs/COMMAND_BUS_MIGRATION.md.
    SetDensityOverride(Option<crate::ui_kit::style::DensityMode>),

    // ── Screener (Wave S2) ────────────────────────────────────────────────
    //
    // All screener state lives on `ScreenPanelState` in `state/aggregates.rs`.
    // Do NOT add fields to `Watchlist` or `Chart` for screener work.
    // These variants are the ONLY way to mutate screener UI/session state.

    /// Open (or close) the screener side-panel. Dispatched by Ctrl+Shift+S
    /// in `top_nav.rs` and by the toolbar toggle button.
    OpenScreenerPanel { open: bool },

    /// Run a saved screen, identified by hotkey slot OR by its server-side UUID.
    ///
    /// Exactly one of `slot` or `id` must be `Some`. `slot` is 1-indexed (1..=9).
    /// The reducer validates the slot range and resolves the bound screen ID.
    /// If both are `None`, this is a no-op with a debug warning.
    /// If both are `Some`, `id` takes precedence.
    RunScreen { slot: Option<u8>, id: Option<String> },

    /// Switch the screener panel's active tab OR the Results sub-view.
    ///
    /// `Library`, `Build`, `Results` map to the three shell tabs (0/1/2).
    /// `Race` and `Heat` are sub-modes inside the Results tab — dispatching
    /// either of these also switches the tab to Results.
    SetScreenerView(crate::state::aggregates::ScreenViewMode),

    /// Load a saved screen into the builder tab for editing.
    /// Switches the active tab to `Build`.
    LoadScreenToBuilder { id: String },

    /// Assign or clear a saved screen's hotkey slot in the Library.
    /// `screen_id = None` clears the slot. `slot` is 1-indexed (1..=9).
    SetScreenHotkeySlot { slot: u8, screen_id: Option<String> },

    /// Toggle the pinned flag of a saved screen in the Library.
    PinScreen { id: String },

    /// Rename a saved screen in the Library (local metadata only;
    /// does NOT call the server — server rename goes via REST in the build tab).
    RenameScreen { id: String, name: String },

    /// Remove a saved screen from the local library cache.
    /// Does NOT delete the screen from ApexData — use the REST API for that.
    DeleteScreen { id: String },

    /// Export the screen definition (DSL text) to clipboard / file.
    /// Reducer stages the export; the actual clipboard write happens in
    /// `screener_panel.rs` which reads `watchlist.screener.pending_export`.
    ExportScreen { id: String },

    /// Mute / un-mute a symbol in the live Results view (session-only).
    /// Muted symbols are hidden in grid + race + heatmap for this session.
    MuteScreenSymbol { symbol: String },

    /// Load the symbol from a screener result row into the active chart pane.
    /// Maps directly to `SwapPaneSymbol { pane: active_pane, symbol }` —
    /// the reducer resolves `active_pane` from `watchlist.active_pane`.
    ScreenRowToChart { symbol: String },

    /// Cache the latest fetched screen list from ApexData into the local
    /// `ScreenPanelState::saved_screens` field. Called from the background
    /// fetch thread after a successful GET /api/data/scans.
    UpdateSavedScreenCache { screens: Vec<crate::state::aggregates::SavedScreenEntry> },

    // ─── Screener Builder commands (T-BUILD) ───────────────────────────────────
    // Emitted by `screener_build.rs`. Reducers mutate `ScreenPanelState::screen_builder`.

    /// Toggle between visual and DSL text mode in the Build tab.
    BuilderSetDslMode { dsl: bool },

    /// Update the DSL text buffer (one char per frame from TextEdit).
    BuilderSetDslText { text: String },

    /// Update the screen name field.
    BuilderSetName { name: String },

    /// Update the timeframe selection.
    BuilderSetTf { tf: String },

    /// Update the asset class ("stocks" | "options").
    BuilderSetClass { class: String },

    /// Update the universe filter (e.g. "SP500", "QQQ100").
    BuilderSetUniverse { universe: String },

    /// Update the rank-by expression and direction.
    BuilderSetRankBy { expr: String, asc: bool },

    /// Update the result limit (max rows returned).
    BuilderSetLimit { n: usize },

    /// Toggle the "alert on new entry" flag.
    BuilderSetAlertEntry { on: bool },

    /// Freeze the current condition_json into the active screen and switch to
    /// the RESULTS tab to show the scan output.
    BuilderCommit,

    /// POST the current screen definition to `/api/data/scans` and persist it
    /// to the local saved_screens cache.
    BuilderSave,

    /// Reset all builder state (conditions, metadata, DSL buffer, errors).
    BuilderReset,

    // ─── Screener Heatmap commands (T-HEAT) ───────────────────────────────────

    /// Set or clear the active sector drill-down filter on the heatmap view.
    /// `None` = clear filter (show all sectors).
    ScreenerSetSectorFilter { sector: Option<String> },

    // ─── Screener S3-TERM commands ─────────────────────────────────────────────

    /// Toggle the provenance popup for a result row (S3-TERM / S3-PROV).
    ///
    /// The actual popup open/close state is managed by the module-level
    /// `PROV_OPEN` set inside `screener_results.rs`. This command is emitted
    /// for command-bus observability (logging, replay) and any future external
    /// listener (e.g. dev-inspector assertions on provenance popup state).
    /// The reducer is a no-op; the UI reacts via the static flag directly.
    ShowRowProvenance { symbol: String },

    /// Kick off background efficacy fetches for all currently-saved screens.
    ///
    /// Emitted once after `UpdateSavedScreenCache` succeeds. The reducer reads
    /// the current saved-screen ID list and calls
    /// `screener_panel::fetch_efficacy_batch` for each ID not yet in the cache.
    /// Gate: `SCREEN_EFFICACY_ENABLED` env (default on; no-op pre-resume-day
    /// when the endpoint returns 404 — absorbed silently). S3-SCORE wires the
    /// `/api/data/screens/efficacy/{id}` endpoint on ApexData.
    FetchScreenEfficacyBatch,
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
    grade_open_plays_live(panes, watchlist);
}

/// Auto-grade open plays against current prices each frame (P0/D1-D3). Price
/// source per play: the watchlist row for its symbol, else the active pane's
/// last bar close if the symbol matches. Persists only when a status changes,
/// so there's no disk churn on quiet frames. Cheap: only open plays are scanned.
fn grade_open_plays_live(panes: &[Chart], watchlist: &mut Watchlist) {
    if watchlist.plays.iter().all(|p| !p.is_open()) { return; }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    // A6 (audit): this runs per frame in RELEASE — no per-frame HashSet/HashMap/
    // to_uppercase String churn. Resolve each open play's price directly
    // (get_price + pane fallback are already case-insensitive), collecting
    // (index, price) into one small scratch Vec, then mutate in a second pass.
    let mut jobs: Vec<(usize, f32)> = Vec::with_capacity(8);
    for (i, p) in watchlist.plays.iter().enumerate() {
        if !p.is_open() { continue; }
        let px = watchlist.get_price(&p.symbol).or_else(|| {
            panes.iter()
                .find(|c| c.symbol.eq_ignore_ascii_case(&p.symbol))
                .and_then(|c| c.bars.last())
                .map(|b| b.close)
                .filter(|&c| c > 0.0)
        });
        if let Some(px) = px { jobs.push((i, px)); }
    }
    let mut changed = 0usize;
    for (i, px) in jobs {
        if let Some(play) = watchlist.plays.get_mut(i) {
            let armed = crate::chart_renderer::arm_branches(play, px);
            let graded = crate::chart_renderer::grade_play(play, px, now);
            if armed || graded { changed += 1; }
        }
    }
    if changed > 0 { watchlist.persist_plays(); }
}

// ─── Transition-log helper ────────────────────────────────────────────────────
// Behind debug_assertions only — zero overhead in release.  One line per
// dispatched command to the stderr audit trail. This is the foundation for
// replay / time-travel debugging in later phases.
#[cfg(debug_assertions)]
#[inline(always)]
fn log_cmd(cmd: &AppCommand) {
    eprintln!("[cmd] {:?}", cmd);
}

/// Reducer — every state change lives here. Purely a state mutation; no
/// side effects (no logging, no IO, no spawning) unless commented otherwise.
fn dispatch(panes: &mut [Chart], watchlist: &mut Watchlist, cmd: AppCommand) {
    // ── Transition log (debug builds only) ──────────────────────────────
    #[cfg(debug_assertions)]
    log_cmd(&cmd);

    match cmd {
        AppCommand::AddPriceAlert { pane, price, above } => {
            if pane >= panes.len() {
                // NOT a debug_assert. The handling below is already safe
                // (`get_mut` + `if let`/`let else`), so the assert was
                // STRICTER THAN THE CODE IT GUARDED — and in a debug build it
                // aborted the process for a case the code handles fine.
                // That mattered: the dev harness legitimately sends commands
                // for pane indices the *current* workspace may not have (a
                // saved 1-pane layout replaying a 2-pane scenario), and the
                // panic took down the dev-inspector thread with it — after
                // which every later request got connection-refused. A whole
                // 1067-scenario corpus run reported as a mass failure, and was
                // misread for two days as "winsock is broken on this machine".
                // A dev-harness command must never be able to kill the app.
                tracing::warn!(target: "cmd",
                    "AddPriceAlert: pane index {} out of range (len={}) — ignored",
                    pane, panes.len());
            }
            let Some(p) = panes.get_mut(pane) else { return; };
            let sym = p.symbol.clone();
            // Add to pane only — the alerts panel already shows pane_active; pushing
            // to watchlist.alerts as well created duplicate rows in the ACTIVE section.
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
            if pane >= panes.len() {
                // NOT a debug_assert. The handling below is already safe
                // (`get_mut` + `if let`/`let else`), so the assert was
                // STRICTER THAN THE CODE IT GUARDED — and in a debug build it
                // aborted the process for a case the code handles fine.
                // That mattered: the dev harness legitimately sends commands
                // for pane indices the *current* workspace may not have (a
                // saved 1-pane layout replaying a 2-pane scenario), and the
                // panic took down the dev-inspector thread with it — after
                // which every later request got connection-refused. A whole
                // 1067-scenario corpus run reported as a mass failure, and was
                // misread for two days as "winsock is broken on this machine".
                // A dev-harness command must never be able to kill the app.
                tracing::warn!(target: "cmd",
                    "PlaceDraftAlert: pane index {} out of range (len={}) — ignored",
                    pane, panes.len());
            }
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
            if pane >= panes.len() {
                // NOT a debug_assert. The handling below is already safe
                // (`get_mut` + `if let`/`let else`), so the assert was
                // STRICTER THAN THE CODE IT GUARDED — and in a debug build it
                // aborted the process for a case the code handles fine.
                // That mattered: the dev harness legitimately sends commands
                // for pane indices the *current* workspace may not have (a
                // saved 1-pane layout replaying a 2-pane scenario), and the
                // panic took down the dev-inspector thread with it — after
                // which every later request got connection-refused. A whole
                // 1067-scenario corpus run reported as a mass failure, and was
                // misread for two days as "winsock is broken on this machine".
                // A dev-harness command must never be able to kill the app.
                tracing::warn!(target: "cmd",
                    "CancelPaneAlert: pane index {} out of range (len={}) — ignored",
                    pane, panes.len());
            }
            if let Some(p) = panes.get_mut(pane) {
                p.price_alerts.retain(|a| a.id != id);
            }
        }

        AppCommand::CancelWatchlistAlert { id } => {
            watchlist.update_alerts_state(|s| s.alerts.retain(|a| a.id != id));
        }

        AppCommand::SnoozeAlert { pane, id } => {
            if let Some(p) = panes.get_mut(pane) {
                if let Some(a) = p.price_alerts.iter_mut().find(|a| a.id == id) {
                    a.triggered = false;
                }
            }
        }

        AppCommand::CancelOrder { pane, id } => {
            if pane >= panes.len() {
                // NOT a debug_assert. The handling below is already safe
                // (`get_mut` + `if let`/`let else`), so the assert was
                // STRICTER THAN THE CODE IT GUARDED — and in a debug build it
                // aborted the process for a case the code handles fine.
                // That mattered: the dev harness legitimately sends commands
                // for pane indices the *current* workspace may not have (a
                // saved 1-pane layout replaying a 2-pane scenario), and the
                // panic took down the dev-inspector thread with it — after
                // which every later request got connection-refused. A whole
                // 1067-scenario corpus run reported as a mass failure, and was
                // misread for two days as "winsock is broken on this machine".
                // A dev-harness command must never be able to kill the app.
                tracing::warn!(target: "cmd",
                    "CancelOrder: pane index {} out of range (len={}) — ignored",
                    pane, panes.len());
            }
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
            for s in &sel {
                if let Some(pane) = panes.get_mut(s.pane_idx) {
                    // resolve pair_id while we have access to the order
                    let pair_id = pane.orders.iter().find(|o| o.id == s.order_id).and_then(|o| o.pair_id);
                    if let Some(o) = pane.orders.iter_mut().find(|o| o.id == s.order_id) {
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
            for s in &sel {
                if let Some(pane) = panes.get_mut(s.pane_idx) {
                    cancel_order_with_pair(&mut pane.orders, s.order_id);
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
            let t = get_theme(p.theme_idx);
            let color_owned = indicator_default_color(p.indicators.len(), &t);
            let id = p.next_indicator_id;
            p.next_indicator_id += 1;
            p.indicators.push(Indicator::new(id, kind, kind.default_period(), &color_owned));
            p.editing_indicator = Some(id);
            p.indicator_bar_count = 0;
        }

        // W3-02: add a Rhai script indicator. This is the seam the script panel
        // (slice 2) calls; the source is evaluated in recompute_indicators.
        // Forcing indicator_bar_count = 0 makes the next update recompute it.
        AppCommand::AddScriptIndicator { pane, src } => {
            let Some(p) = panes.get_mut(pane) else { return; };
            let t = get_theme(p.theme_idx);
            let color_owned = indicator_default_color(p.indicators.len(), &t);
            let id = p.next_indicator_id;
            p.next_indicator_id += 1;
            let mut ind = Indicator::new(id, IndicatorType::Script, 1, &color_owned);
            ind.script_src = src;
            p.indicators.push(ind);
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

        AppCommand::ClearIndicators { pane } => {
            if let Some(p) = panes.get_mut(pane) {
                p.indicators.clear();
                p.editing_indicator = None;
                p.indicator_bar_count = 0;
                // A cleared pane behaves like a fresh one: restart id numbering so
                // subsequently-added indicators get predictable ids 0,1,2,…
                p.next_indicator_id = 0;
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

        // ── Per-pane display flag ───────────────────────────────────────
        AppCommand::SetChartFlag { pane, flag, value } => {
            if pane >= panes.len() {
                // NOT a debug_assert. The handling below is already safe
                // (`get_mut` + `if let`/`let else`), so the assert was
                // STRICTER THAN THE CODE IT GUARDED — and in a debug build it
                // aborted the process for a case the code handles fine.
                // That mattered: the dev harness legitimately sends commands
                // for pane indices the *current* workspace may not have (a
                // saved 1-pane layout replaying a 2-pane scenario), and the
                // panic took down the dev-inspector thread with it — after
                // which every later request got connection-refused. A whole
                // 1067-scenario corpus run reported as a mass failure, and was
                // misread for two days as "winsock is broken on this machine".
                // A dev-harness command must never be able to kill the app.
                tracing::warn!(target: "cmd",
                    "SetChartFlag: pane index {} out of range (len={}) — ignored",
                    pane, panes.len());
            }
            let Some(p) = panes.get_mut(pane) else { return; };
            match flag {
                ChartFlag::ShowVolume        => p.show_volume = value,
                ChartFlag::LogScale          => p.log_scale = value,
                ChartFlag::Magnet            => p.magnet = value,
                ChartFlag::OhlcTooltip       => p.ohlc_tooltip = value,
                ChartFlag::MeasureTooltip    => p.measure_tooltip = value,
                ChartFlag::ShowOscillators   => p.show_oscillators = value,
                ChartFlag::ShowPrevClose     => p.show_prev_close = value,
                ChartFlag::ShowPatternLabels => p.show_pattern_labels = value,
                ChartFlag::ShowFootprint     => p.show_footprint = value,
                ChartFlag::HideAllIndicators  => p.hide_all_indicators = value,
                ChartFlag::HideAllDrawings    => p.hide_all_drawings = value,
                ChartFlag::ShowGamma          => p.show_gamma = value,
                ChartFlag::ShowStrikesOverlay => p.show_strikes_overlay = value,
            }
        }

        // ── Pane / layout ───────────────────────────────────────────────
        AppCommand::ChangePaneType { pane, kind } => {
            if pane >= panes.len() {
                // NOT a debug_assert. The handling below is already safe
                // (`get_mut` + `if let`/`let else`), so the assert was
                // STRICTER THAN THE CODE IT GUARDED — and in a debug build it
                // aborted the process for a case the code handles fine.
                // That mattered: the dev harness legitimately sends commands
                // for pane indices the *current* workspace may not have (a
                // saved 1-pane layout replaying a 2-pane scenario), and the
                // panic took down the dev-inspector thread with it — after
                // which every later request got connection-refused. A whole
                // 1067-scenario corpus run reported as a mass failure, and was
                // misread for two days as "winsock is broken on this machine".
                // A dev-harness command must never be able to kill the app.
                tracing::warn!(target: "cmd",
                    "ChangePaneType: pane index {} out of range (len={}) — ignored",
                    pane, panes.len());
            }
            if let Some(p) = panes.get_mut(pane) {
                p.pane_type = kind;
            }
        }

        AppCommand::SwapPaneSymbol { pane, symbol } => {
            if let Some(p) = panes.get_mut(pane) {
                p.request_gen = p.request_gen.wrapping_add(1);
                p.symbol = symbol.clone();
                p.symbol_meta = crate::foundation::types::symbol_or_guess(&symbol);
                p.pending_symbol_change = Some(symbol);
            }
        }

        AppCommand::ChangeTimeframe { pane, tf } => {
            if let Some(p) = panes.get_mut(pane) {
                p.request_gen = p.request_gen.wrapping_add(1);
                p.timeframe = tf.clone();
                p.pending_timeframe_change = Some(tf);
            }
        }

        AppCommand::SetThemeIdx { pane: _, idx } => {
            // Theme is conceptually app-wide — apply to every pane so all
            // chrome (panel headers, chart bg, watchlist rows) stays in sync.
            for p in panes.iter_mut() {
                p.theme_idx = idx;
                // Keep the drawing-tool default pen tracking the palette so a
                // new trendline starts in a theme-coherent colour (accent).
                // Still user-overridable per drawing afterwards.
                p.draw_color = indicator_default_color(0, &get_theme(idx));
            }
        }

        AppCommand::SetStyleIdx { idx } => {
            watchlist.style_idx = idx;
        }

        AppCommand::SetLayoutLive { layout } => {
            // Park the request — panes is a slice here, so the actual
            // grow/shrink happens in the render loop's PENDING_LAYOUT drain
            // (see gpu::PENDING_LAYOUT / pane_ops::apply_layout_template).
            match crate::chart_renderer::gpu::Layout::from_label(&layout) {
                Some(ly) => {
                    crate::chart_renderer::gpu::PENDING_LAYOUT
                        .with(|c| *c.borrow_mut() = Some(ly));
                }
                None => {
                    tracing::warn!(target: "cmd",
                        "SetLayoutLive: unrecognised layout label {:?} — ignored", layout);
                }
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

        #[cfg(debug_assertions)]
        AppCommand::CloseAllDialogs => {
            for p in panes.iter_mut() {
                p.pane_picker_open = false;
                p.editing_indicator = None;
            }
            watchlist.settings_open = false;
            watchlist.hotkey_editor_open = false;
            watchlist.order_entry_open = false;
            watchlist.orders_panel_open = false;
            watchlist.chain.select_mode = false;
        }

        // ── Dev Inspector — subsystem drivers ─────────────────────────────
        #[cfg(debug_assertions)]
        AppCommand::SynthGamma { pane } => {
            if let Some(p) = panes.get_mut(pane) {
                p.show_gamma = true;
                p.populate_gamma(true); // force synth — deterministic without :8412
            }
        }

        #[cfg(debug_assertions)]
        AppCommand::SetDomSidebar { pane, open } => {
            if let Some(p) = panes.get_mut(pane) {
                p.dom.sidebar_open = open;
                // Populate the mock ladder on command (not just in the render
                // path) so DOM is observable for ANY pane — pane 1's sidebar
                // isn't reliably rendered in the harness's default layout, so
                // the render-path fallback never fires there. Mirrors core.rs.
                if open && p.dom.levels.is_empty() {
                    let center = p.bars.last().map(|b| b.close).filter(|&c| c > 0.0).unwrap_or(500.0);
                    if p.dom.center_price <= 0.0 { p.dom.center_price = center; }
                    if p.dom.tick_size <= 0.0 { p.dom.tick_size = 0.01; }
                    p.dom.levels = crate::chart_renderer::ui::panels::dom_panel::generate_mock_levels(
                        p.dom.center_price, p.dom.tick_size, 30);
                }
            }
        }

        #[cfg(debug_assertions)]
        AppCommand::SeedDraftOrder { pane, side, price, qty } => {
            // Visual-only (System A). Never touches OrderManager/broker.
            if let Some(p) = panes.get_mut(pane) {
                let os = match side.to_ascii_lowercase().as_str() {
                    "sell" | "s" => OrderSide::Sell,
                    "stop"       => OrderSide::Stop,
                    _             => OrderSide::Buy,
                };
                let id = p.next_order_id;
                p.next_order_id = p.next_order_id.wrapping_add(1);
                p.orders.push(OrderLevel {
                    id,
                    side: os,
                    price,
                    qty,
                    status: OrderStatus::Draft,
                    state: OrderState::Draft,
                    pair_id: None,
                    option_symbol: None,
                    option_con_id: None,
                    trail_amount: None,
                    trail_percent: None,
                    filled_ratio: 0.0,
                });
            }
        }

        #[cfg(debug_assertions)]
        AppCommand::SetOrderPanel { pane, collapsed } => {
            if let Some(p) = panes.get_mut(pane) {
                p.order_panel.collapsed = collapsed;
            }
        }

        #[cfg(debug_assertions)]
        AppCommand::SetScannerOpen { open } => {
            // Route through the sidebar-state store, else the store→flat sync
            // overwrites the flat bool back to its persisted value every frame.
            watchlist.update_sidebar_state(|s| s.scanner_open = open);
        }

        #[cfg(debug_assertions)]
        AppCommand::SeedScannerResults { count } => {
            // Deterministic synthetic pool so `apply_scanner` filtering is
            // testable without a live /api/stocks/movers response.
            let n = count.min(500);
            let mut rows = Vec::with_capacity(n);
            for i in 0..n {
                let f = i as f32;
                // Spread change_pct across [-9.5, +9.5] deterministically.
                let change_pct = ((f * 1.9) % 19.0) - 9.5;
                rows.push(crate::chart_renderer::gpu::ScanResult {
                    symbol: format!("SYN{i:03}"),
                    price: 10.0 + (f * 3.17) % 490.0,
                    change_pct,
                    volume: 1_000_000 + (i as u64) * 250_000,
                });
            }
            watchlist.scanner.results = rows;
        }

        #[cfg(debug_assertions)]
        AppCommand::SetRrgOpen { open } => {
            watchlist.update_sidebar_state(|s| s.rrg_open = open);
        }

        #[cfg(debug_assertions)]
        AppCommand::SetRrgTail { len } => {
            watchlist.rrg.tail_length = len.clamp(1, 20);
        }

        #[cfg(debug_assertions)]
        AppCommand::SeedHeatmapCells { count } => {
            let n = count.min(200);
            let mut cells = Vec::with_capacity(n);
            for i in 0..n {
                let f = i as f32;
                let change_pct = ((f * 1.3) % 12.0) - 6.0;
                cells.push((format!("HM{i:03}"), change_pct, 1.0e6 + (i as f64) * 5.0e5));
            }
            watchlist.heatmap.cells = cells;
        }

        AppCommand::SetUiDebug { on } => {
            crate::chart_renderer::bug_anchor::set_ui_debug(on);
        }

        AppCommand::SetViewportSize { w, h } => {
            crate::chart_renderer::bug_anchor::request_viewport_size(w, h);
        }

        AppCommand::SetObjectTree { open } => {
            // Route through the sidebar-state store (the store→flat sync sets the
            // watchlist flag each frame). Dev-inspector view control.
            watchlist.update_sidebar_state(|s| s.object_tree_open = open);
        }
        #[cfg(debug_assertions)]
        AppCommand::SetAutoChartPanel { open } => {
            // Route through the sidebar-state store (else the store→flat sync
            // overwrites the flat bool every frame), same as scanner/RRG.
            watchlist.update_sidebar_state(|s| s.auto_chart_open = open);
        }

        #[cfg(debug_assertions)]
        AppCommand::SetPlaybookPanel { open } => {
            watchlist.update_sidebar_state(|s| s.playbook_panel_open = open);
        }

        #[cfg(debug_assertions)]
        AppCommand::SeedPlay { symbol, long, entry, target, stop } => {
            use crate::chart_renderer::{Play, PlayDirection, PlayType};
            let dir = if long { PlayDirection::Long } else { PlayDirection::Short };
            let mut play = Play::new(&symbol, dir, PlayType::Directional, entry, target, stop);
            play.author = crate::chart_renderer::gpu::author_handle();
            watchlist.plays.push(play);
        }

        #[cfg(debug_assertions)]
        AppCommand::SetAuthor { handle } => {
            // In-memory only — never write the user's real author.txt from a test.
            crate::chart_renderer::gpu::set_author_handle_mem(&handle);
        }

        #[cfg(debug_assertions)]
        AppCommand::ExportPlay { idx } => {
            if let Some(p) = watchlist.plays.get(idx) {
                let _ = crate::chart_renderer::gpu::export_play_to_file(
                    p, &crate::chart_renderer::gpu::play_export_debug_path());
            }
        }

        #[cfg(debug_assertions)]
        AppCommand::ImportPlay => {
            if let Some(p) = crate::chart_renderer::gpu::import_play_from_file(
                &crate::chart_renderer::gpu::play_export_debug_path()) {
                watchlist.plays.push(p);
            }
        }

        #[cfg(debug_assertions)]
        AppCommand::ForkPlay { idx } => {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
            let author = crate::chart_renderer::gpu::author_handle();
            if let Some(src) = watchlist.plays.get(idx) {
                let fork = src.fork(&author, now);
                watchlist.plays.push(fork);
            }
        }

        #[cfg(debug_assertions)]
        AppCommand::PublishPlay { idx } => {
            use crate::chart_renderer::PlayStatus;
            if let Some(p) = watchlist.plays.get_mut(idx) {
                p.status = PlayStatus::Published;
                let published = p.clone();
                // Add to the feed if not already present (idempotent by id).
                if !watchlist.feed.iter().any(|f| f.id == published.id) {
                    watchlist.feed.push(published);
                }
                watchlist.persist_plays();
            }
        }

        #[cfg(debug_assertions)]
        AppCommand::SeedFeed { count } => {
            use crate::chart_renderer::{Play, PlayDirection, PlayType, PlayStatus};
            let n = count.min(200);
            let syms = ["SPY","QQQ","NVDA","TSLA","AAPL","AMD","META","MSFT"];
            watchlist.feed = (0..n).map(|i| {
                let long = i % 2 == 0;
                let e = 100.0 + (i as f32 % 50.0);
                let (t, s) = if long { (e + 10.0, e - 5.0) } else { (e - 10.0, e + 5.0) };
                let mut p = Play::new(syms[i % syms.len()], if long { PlayDirection::Long } else { PlayDirection::Short },
                                      PlayType::Directional, e, t, s);
                p.author = format!("author{}", i % 4);
                p.status = PlayStatus::Published;
                p
            }).collect();
        }

        #[cfg(debug_assertions)]
        AppCommand::SetFeedFilter { symbol } => {
            watchlist.feed_filter_symbol = symbol.to_uppercase();
        }

        #[cfg(debug_assertions)]
        AppCommand::SharePlayToDiscord { idx } => {
            // Format + stash only — NEVER posts to Discord from a test.
            if let Some(p) = watchlist.plays.get(idx) {
                crate::chart_renderer::gpu::set_last_share(
                    crate::chart_renderer::gpu::format_play_embed(p));
            }
        }

        #[cfg(debug_assertions)]
        AppCommand::SetPlayZone { idx, low, high } => {
            if let Some(p) = watchlist.plays.get_mut(idx) {
                let (lo, hi) = if low <= high { (low, high) } else { (high, low) };
                p.entry_low = lo; p.entry_high = hi;
                p.entry_price = (lo + hi) / 2.0; // zone mid drives grading + R:R
                if (p.entry_price - p.stop_price).abs() > 0.001 {
                    p.risk_reward = (p.target_price - p.entry_price).abs()
                        / (p.entry_price - p.stop_price).abs();
                }
            }
        }

        #[cfg(debug_assertions)]
        AppCommand::SetPlayInvalidation { idx, price } => {
            if let Some(p) = watchlist.plays.get_mut(idx) {
                p.invalidation = if price > 0.0 { Some(price) } else { None };
            }
        }

        #[cfg(debug_assertions)]
        AppCommand::SetPlayTrigger { idx, style } => {
            use crate::chart_renderer::EntryTrigger;
            if let Some(p) = watchlist.plays.get_mut(idx) {
                p.trigger = match style.as_str() {
                    "limit"  => EntryTrigger::Limit,
                    "market" => EntryTrigger::Market,
                    _        => EntryTrigger::Stop,
                };
            }
        }

        #[cfg(debug_assertions)]
        AppCommand::AddBranch { idx, above, level, long, entry, target, stop } => {
            use crate::chart_renderer::{PlayBranch, PlayDirection};
            if let Some(p) = watchlist.plays.get_mut(idx) {
                p.branches.push(PlayBranch {
                    above, level,
                    direction: if long { PlayDirection::Long } else { PlayDirection::Short },
                    entry, target, stop, armed: false,
                });
            }
        }

        #[cfg(debug_assertions)]
        AppCommand::ClearBranches { idx } => {
            if let Some(p) = watchlist.plays.get_mut(idx) { p.branches.clear(); }
        }

        #[cfg(debug_assertions)]
        AppCommand::AddLeg { idx, symbol, role, long, entry, target, stop, weight } => {
            use crate::chart_renderer::{PlayLeg, PlayDirection, LegRole};
            if let Some(p) = watchlist.plays.get_mut(idx) {
                let role = match role.as_str() {
                    "hedge" => LegRole::Hedge, "pair_short" | "pairshort" => LegRole::PairShort,
                    "basket" => LegRole::BasketMember, _ => LegRole::Primary,
                };
                p.legs.push(PlayLeg {
                    symbol: symbol.to_uppercase(), role,
                    direction: if long { PlayDirection::Long } else { PlayDirection::Short },
                    entry, target, stop, weight: if weight > 0.0 { weight } else { 1.0 },
                });
            }
        }

        #[cfg(debug_assertions)]
        AppCommand::ClearLegs { idx } => {
            if let Some(p) = watchlist.plays.get_mut(idx) { p.legs.clear(); }
        }

        #[cfg(debug_assertions)]
        AppCommand::AddSpreadLeg { idx, is_call, strike, buy, price, qty, expiry } => {
            use crate::chart_renderer::{SpreadLeg, PlayDirection};
            if let Some(p) = watchlist.plays.get_mut(idx) {
                let cp = if is_call { 'C' } else { 'P' };
                p.spread_legs.push(SpreadLeg {
                    contract: format!("{strike:.0}{cp} {expiry}"),
                    side: if buy { PlayDirection::Long } else { PlayDirection::Short },
                    price, quantity: qty.max(1), strike, is_call, expiry,
                });
            }
        }

        #[cfg(debug_assertions)]
        AppCommand::ClearSpreadLegs { idx } => {
            if let Some(p) = watchlist.plays.get_mut(idx) { p.spread_legs.clear(); }
        }

        #[cfg(debug_assertions)]
        AppCommand::PayoffAt { idx, price } => {
            if let Some(p) = watchlist.plays.get(idx) {
                let v = crate::chart_renderer::option_payoff_at(&p.spread_legs, price);
                crate::chart_renderer::gpu::set_last_payoff(v);
            }
        }

        #[cfg(debug_assertions)]
        AppCommand::SetAccountRisk { account, risk_pct } => {
            watchlist.account_size = account.max(0.0);
            watchlist.risk_pct = risk_pct.clamp(0.0, 1.0);
        }

        AppCommand::SetDensityOverride(mode) => {
            watchlist.density_override = mode;
            // Side-effect that used to live inline in settings_panel — now
            // atomic with the field write inside the reducer.
            crate::chart_renderer::ui::style::set_density_override(mode);
        }

        // ── Screener reducers (Wave S2) ────────────────────────────────────
        //
        // `ScreenPanelState` is NOT on `Watchlist` (Watchlist is frozen).
        // All screener aggregate state lives in the module-level static
        // `screener_panel::SCREENER_STATE` (an `Arc<RwLock<ScreenPanelState>>`
        // initialized lazily on first panel open, loaded from disk by the
        // Store supervisor). The `panel_open` flag is the only screener field
        // that mirrors into `SidebarState` (so it persists via the existing
        // sidebar_state_store path). All other mutations go through the global.

        AppCommand::OpenScreenerPanel { open } => {
            // Route through sidebar_state_store so the flat bool in `Watchlist`
            // stays in sync with the persisted aggregate (same pattern as
            // scanner_open, rrg_open, playbook_panel_open, etc.).
            watchlist.update_sidebar_state(|s| s.screener_panel_open = open);
        }

        AppCommand::RunScreen { slot, id } => {
            use crate::chart_renderer::ui::panels::screener_panel as sp;
            // Resolve screen ID: explicit id wins over slot lookup.
            let screen_id = id.or_else(|| {
                slot.filter(|&s| s >= 1 && s <= 9).and_then(|s| {
                    sp::with_screener_state(|g| g.hotkey_slots[(s - 1) as usize].clone())
                        .flatten()
                })
            });
            if let Some(sid) = screen_id {
                sp::with_screener_state_mut(|g| {
                    g.active_screen_id = Some(sid);
                    g.active_tab = 2; // Results tab
                });
                watchlist.update_sidebar_state(|s| s.screener_panel_open = true);
            } else {
                #[cfg(debug_assertions)]
                eprintln!("[cmd] RunScreen: no screen bound to slot {:?}", slot);
            }
        }

        AppCommand::SetScreenerView(mode) => {
            use crate::state::aggregates::ScreenViewMode;
            use crate::chart_renderer::ui::panels::screener_panel as sp;
            sp::with_screener_state_mut(|g| {
                g.view_mode = mode;
                if matches!(mode, ScreenViewMode::Race | ScreenViewMode::Heat) {
                    g.active_tab = 2; // Results tab
                }
            });
            watchlist.update_sidebar_state(|s| s.screener_panel_open = true);
        }

        AppCommand::LoadScreenToBuilder { id } => {
            use crate::chart_renderer::ui::panels::screener_panel as sp;
            sp::with_screener_state_mut(|g| {
                g.active_screen_id = Some(id);
                g.active_tab = 1; // Build tab
            });
            watchlist.update_sidebar_state(|s| s.screener_panel_open = true);
        }

        AppCommand::SetScreenHotkeySlot { slot, screen_id } => {
            use crate::chart_renderer::ui::panels::screener_panel as sp;
            if !(1..=9).contains(&slot) {
                #[cfg(debug_assertions)]
                eprintln!("[cmd] SetScreenHotkeySlot: slot {} out of range 1..=9", slot);
                return;
            }
            sp::with_screener_state_mut(|g| {
                // Unassign any existing binding for this slot from saved_screens.
                for screen in g.saved_screens.iter_mut() {
                    if screen.hotkey_slot == Some(slot) {
                        if screen_id.as_deref().map_or(true, |sid| sid != screen.id) {
                            screen.hotkey_slot = None;
                        }
                    }
                }
                g.hotkey_slots[(slot - 1) as usize] = screen_id.clone();
                // Assign the slot on the matching saved_screen entry.
                if let Some(sid) = &screen_id {
                    if let Some(s) = g.saved_screens.iter_mut().find(|s| &s.id == sid) {
                        s.hotkey_slot = Some(slot);
                    }
                }
            });
        }

        AppCommand::PinScreen { id } => {
            use crate::chart_renderer::ui::panels::screener_panel as sp;
            sp::with_screener_state_mut(|g| {
                if let Some(s) = g.saved_screens.iter_mut().find(|s| s.id == id) {
                    s.pinned = !s.pinned;
                }
            });
        }

        AppCommand::RenameScreen { id, name } => {
            use crate::chart_renderer::ui::panels::screener_panel as sp;
            let name = name.trim().to_string();
            if name.is_empty() { return; }
            sp::with_screener_state_mut(|g| {
                if let Some(s) = g.saved_screens.iter_mut().find(|s| s.id == id) {
                    s.name = name;
                }
            });
        }

        AppCommand::DeleteScreen { id } => {
            use crate::chart_renderer::ui::panels::screener_panel as sp;
            sp::with_screener_state_mut(|g| {
                g.saved_screens.retain(|s| s.id != id);
                for slot in g.hotkey_slots.iter_mut() {
                    if slot.as_deref() == Some(&id) { *slot = None; }
                }
                if g.active_screen_id.as_deref() == Some(&id) {
                    g.active_screen_id = None;
                }
            });
        }

        AppCommand::ExportScreen { id } => {
            // Stage the export request; screener_panel.rs reads `pending_export_id`
            // each frame and performs the clipboard write before clearing it.
            use crate::chart_renderer::ui::panels::screener_panel as sp;
            sp::with_screener_state_mut(|g| {
                if g.saved_screens.iter().any(|s| s.id == id) {
                    g.pending_export_id = Some(id);
                }
            });
        }

        AppCommand::MuteScreenSymbol { symbol } => {
            // Session-only. Lives in the runtime muted-set inside screener_panel.
            use crate::chart_renderer::ui::panels::screener_panel as sp;
            let sym = symbol.to_uppercase();
            sp::toggle_muted_symbol(&sym);
        }

        AppCommand::ScreenRowToChart { symbol } => {
            // Resolve the active pane index via `watchlist.active_pane_idx`,
            // clamped to a valid index. Defaults to pane 0 if there are no panes.
            let ap = watchlist.active_pane_idx.min(panes.len().saturating_sub(1));
            // Dispatch as SwapPaneSymbol — same reducer path, same semantics.
            dispatch(panes, watchlist, AppCommand::SwapPaneSymbol { pane: ap, symbol });
        }

        AppCommand::UpdateSavedScreenCache { screens } => {
            use crate::chart_renderer::ui::panels::screener_panel as sp;
            sp::with_screener_state_mut(|g| {
                // Merge server list with local hotkey/pin metadata to avoid
                // losing user customizations that have not yet been synced to
                // the server (e.g., local-only pin + slot bindings).
                let old = std::mem::take(&mut g.saved_screens);
                g.saved_screens = screens.into_iter().map(|mut s| {
                    if let Some(existing) = old.iter().find(|e| e.id == s.id) {
                        s.pinned = existing.pinned;
                        s.hotkey_slot = existing.hotkey_slot;
                    }
                    s
                }).collect();
            });
        }

        // ─── Builder reducers (T-BUILD) ────────────────────────────────────────────

        AppCommand::BuilderSetDslMode { dsl } => {
            use crate::chart_renderer::ui::panels::screener_panel as sp;
            sp::with_screener_state_mut(|g| { g.screen_builder.dsl_mode = dsl; });
        }

        AppCommand::BuilderSetDslText { text } => {
            use crate::chart_renderer::ui::panels::screener_panel as sp;
            sp::with_screener_state_mut(|g| { g.screen_builder.dsl_text = text; });
        }

        AppCommand::BuilderSetName { name } => {
            use crate::chart_renderer::ui::panels::screener_panel as sp;
            sp::with_screener_state_mut(|g| { g.screen_builder.name = name; });
        }

        AppCommand::BuilderSetTf { tf } => {
            use crate::chart_renderer::ui::panels::screener_panel as sp;
            sp::with_screener_state_mut(|g| { g.screen_builder.tf = tf; });
        }

        AppCommand::BuilderSetClass { class } => {
            use crate::chart_renderer::ui::panels::screener_panel as sp;
            sp::with_screener_state_mut(|g| { g.screen_builder.class = class; });
        }

        AppCommand::BuilderSetUniverse { universe } => {
            use crate::chart_renderer::ui::panels::screener_panel as sp;
            sp::with_screener_state_mut(|g| { g.screen_builder.universe = universe; });
        }

        AppCommand::BuilderSetRankBy { expr, asc } => {
            use crate::chart_renderer::ui::panels::screener_panel as sp;
            sp::with_screener_state_mut(|g| {
                g.screen_builder.rank_by = expr;
                g.screen_builder.rank_asc = asc;
            });
        }

        AppCommand::BuilderSetLimit { n } => {
            use crate::chart_renderer::ui::panels::screener_panel as sp;
            sp::with_screener_state_mut(|g| { g.screen_builder.limit = n; });
        }

        AppCommand::BuilderSetAlertEntry { on } => {
            use crate::chart_renderer::ui::panels::screener_panel as sp;
            sp::with_screener_state_mut(|g| { g.screen_builder.alert_on_entry = on; });
        }

        AppCommand::BuilderCommit => {
            use crate::chart_renderer::ui::panels::screener_panel as sp;
            sp::with_screener_state_mut(|g| {
                // Freeze condition_json from builder, switch to Results tab.
                g.active_tab = 2;
            });
        }

        AppCommand::BuilderSave => {
            // POST to /api/data/scans is fire-and-forget from a background thread.
            // Stub: T-BUILD already has its own local statics; the reducer just logs intent.
            #[cfg(debug_assertions)]
            eprintln!("[cmd] BuilderSave — wire REST POST when ApexData endpoint is ready");
        }

        AppCommand::BuilderReset => {
            use crate::chart_renderer::ui::panels::screener_panel as sp;
            sp::with_screener_state_mut(|g| {
                g.screen_builder = crate::state::aggregates::ScreenBuilderState::default();
            });
        }

        // ─── Heatmap sector filter (T-HEAT) ───────────────────────────────────────

        AppCommand::ScreenerSetSectorFilter { sector } => {
            use crate::chart_renderer::ui::panels::screener_panel as sp;
            sp::with_screener_state_mut(|g| { g.screen_heatmap.sector_filter = sector; });
        }

        // ─── S3-TERM: provenance popup + efficacy fetch ────────────────────────────

        AppCommand::ShowRowProvenance { symbol: _ } => {
            // No-op in the reducer: provenance popup toggle state lives in the
            // module-level `PROV_OPEN` set inside `screener_results.rs` and is
            // managed entirely by the UI layer (toggled on trailing-button click).
            // This arm exists only for command-bus observability (debug logging
            // and future replay / dev-inspector assertions).
        }

        AppCommand::FetchScreenEfficacyBatch => {
            // Read the current saved-screen IDs from ScreenPanelState and kick
            // off background efficacy fetches for each ID. Gate is checked
            // inside `fetch_efficacy_batch` (SCREEN_EFFICACY_ENABLED env).
            use crate::chart_renderer::ui::panels::screener_panel as sp;
            let ids = sp::with_screener_state(|g| {
                g.saved_screens.iter().map(|s| s.id.clone()).collect::<Vec<_>>()
            }).unwrap_or_default();
            if !ids.is_empty() {
                sp::fetch_efficacy_batch(ids);
            }
        }

        #[cfg(debug_assertions)]
        AppCommand::OpenPlayMultiPane { idx } => {
            // Restore the whole multi-pane view: pane i ← leg i's symbol + levels.
            let legs = watchlist.plays.get(idx).map(|p| p.legs.clone()).unwrap_or_default();
            for (i, leg) in legs.iter().enumerate() {
                if let Some(pane) = panes.get_mut(i) {
                    pane.request_gen = pane.request_gen.wrapping_add(1);
                    pane.symbol = leg.symbol.clone();
                    pane.symbol_meta = crate::foundation::types::symbol_or_guess(&leg.symbol);
                    pane.pending_symbol_change = Some(leg.symbol.clone());
                    crate::chart_renderer::gpu::set_pane_play_lines(pane, leg.entry, leg.target, leg.stop);
                }
            }
        }

        #[cfg(debug_assertions)]
        AppCommand::SetPlayNote { idx, which, note } => {
            if let Some(p) = watchlist.plays.get_mut(idx) {
                match which.as_str() {
                    "entry"  => p.entry_note = note,
                    "stop"   => p.stop_note = note,
                    "thesis" => p.notes = note,
                    _ => {}
                }
            }
        }

        #[cfg(debug_assertions)]
        AppCommand::AddScaleIn { idx, price, pct } => {
            use crate::chart_renderer::PlayTarget;
            if let Some(p) = watchlist.plays.get_mut(idx) {
                let n = p.scale_ins.len() + 1;
                p.scale_ins.push(PlayTarget { price, pct, label: format!("S{n}") });
            }
        }

        #[cfg(debug_assertions)]
        AppCommand::ClearScaleIns { idx } => {
            if let Some(p) = watchlist.plays.get_mut(idx) { p.scale_ins.clear(); }
        }

        #[cfg(debug_assertions)]
        AppCommand::SetPlayLevelExpr { idx, which, expr } => {
            use crate::chart_renderer::PlayDirection;
            // Read the play's levels/symbol/direction, build ctx from the pane showing it.
            let Some((entry, stop, target, sym, long)) = watchlist.plays.get(idx).map(|p|
                (p.entry_price, p.stop_price, p.target_price, p.symbol.clone(),
                 matches!(p.direction, PlayDirection::Long))) else { return; };
            let chart = panes.iter().find(|c| c.symbol.eq_ignore_ascii_case(&sym)).or_else(|| panes.first());
            // Bare "NR" shorthand → place at N risk-units from entry (direction-aware);
            // otherwise evaluate the full expression.
            let resolved = if let Some(n) = crate::chart_renderer::parse_risk_multiple(&expr) {
                let r = (entry - stop).abs();
                let dir = if long { 1.0 } else { -1.0 };
                match which.as_str() {
                    "target" => Some(entry + n * r * dir),
                    "stop"   => Some(entry - n * r * dir),
                    _ => None,
                }
            } else {
                let ctx = crate::chart_renderer::gpu::play_expr_ctx(entry, stop, target, chart);
                crate::chart_renderer::resolve_level_expr(&expr, &ctx)
            };
            if let Some(v) = resolved {
                if let Some(p) = watchlist.plays.get_mut(idx) {
                    match which.as_str() {
                        "entry"  => { p.entry_price = v; p.entry_low = v; p.entry_high = v; }
                        "stop"   => p.stop_price = v,
                        "target" => p.target_price = v,
                        _ => {}
                    }
                    if (p.entry_price - p.stop_price).abs() > 0.001 {
                        p.risk_reward = (p.target_price - p.entry_price).abs()
                            / (p.entry_price - p.stop_price).abs();
                    }
                }
            }
        }

        #[cfg(debug_assertions)]
        AppCommand::SnapTest { price, targets, tolerance } => {
            use crate::chart_renderer::SnapLevel;
            let cands: Vec<SnapLevel> = targets.iter().enumerate()
                .map(|(i, &p)| SnapLevel { price: p, label: format!("t{i}") }).collect();
            let (snapped, label) = crate::chart_renderer::snap_price(price, &cands, tolerance);
            crate::chart_renderer::gpu::set_last_snap(snapped, label.as_deref().unwrap_or(""));
        }

        #[cfg(debug_assertions)]
        AppCommand::ClearPlays => {
            watchlist.plays.clear();
        }

        #[cfg(debug_assertions)]
        AppCommand::PersistPlays => {
            crate::chart_renderer::gpu::save_plays_to(
                &crate::chart_renderer::gpu::plays_debug_path(), &watchlist.plays);
        }

        #[cfg(debug_assertions)]
        AppCommand::ReloadPlays => {
            watchlist.plays = crate::chart_renderer::gpu::load_plays_from(
                &crate::chart_renderer::gpu::plays_debug_path());
        }

        #[cfg(debug_assertions)]
        AppCommand::GradePlaysAtPrice { symbol, price } => {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
            crate::chart_renderer::grade_plays(&mut watchlist.plays, &symbol, price, now);
        }

        #[cfg(debug_assertions)]
        AppCommand::SetPlayExpiry { idx, expiry } => {
            if let Some(p) = watchlist.plays.get_mut(idx) {
                p.expiry = if expiry > 0 { Some(expiry) } else { None };
            }
        }

        #[cfg(debug_assertions)]
        AppCommand::SetCell { pane, row, col, text } => {
            if let Some(p) = panes.get_mut(pane) {
                // Grow the grid so (row,col) is addressable.
                if col + 1 > p.spreadsheet_cols { p.spreadsheet_cols = col + 1; }
                while p.spreadsheet_cells.len() <= row {
                    p.spreadsheet_cells.push(vec![String::new(); p.spreadsheet_cols]);
                }
                for r in p.spreadsheet_cells.iter_mut() {
                    while r.len() < p.spreadsheet_cols { r.push(String::new()); }
                }
                p.spreadsheet_rows = p.spreadsheet_cells.len();
                p.spreadsheet_cells[row][col] = text;
            }
        }
    }
}
