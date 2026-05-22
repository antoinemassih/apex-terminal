# Apex Terminal — State System Reference

This document describes how state is organised, mutated, and persisted in the
Apex Terminal native renderer. The codebase is in active migration across four
overlapping generations of architecture. Nothing here is aspirational; every
claim is keyed to a real file and line.

---

## 1. Overview — Four Generations

At runtime today the app carries four parallel state strategies. They are not
cleanly separated; they overlap, share data, and each covers different parts of
the mutation surface.

| Generation | Status | What it covers |
|---|---|---|
| **Gen 0** — `Watchlist` + `Chart` god-objects | Live, ~80% of mutation | Everything the GPU render path reads every frame |
| **Gen 1** — `AppCommand` queue | Partially adopted, ~5% | Alerts, orders, indicators, watchlist structure, theme |
| **Gen 1b** — `src/state/` module | Partially wired, mirrors only | Typed aggregates backed by `Store<T>` + background persist |
| **Gen 2** — `chart/state/ChartState` | Schema only, NOT wired | Future canonical chart model |

The authoritative runtime state today is **Gen 0**: the `Watchlist` struct and the
`Vec<Chart>` pane array, both threaded as `&mut` through the render tree every
frame. Every other generation either mirrors Gen 0 or is not yet connected.

---

## 2. Where State Lives

| Container | Type | File:line | Scope | Authoritative? |
|---|---|---|---|---|
| `Watchlist` | ~390-field struct | `gpu.rs:4440` | Global (one per app) | Yes — Gen 0 |
| `Vec<Chart>` pane array | `Vec<Chart>` (~360 fields each) | `gpu.rs:1588` | Per-pane | Yes — Gen 0 |
| `Store<UiSettings>` | `Arc<Store<UiSettings>>` | `gpu.rs:4791` (field on `Watchlist`) | Global | Mirror only |
| `Store<TradingDefaults>` | `Arc<Store<TradingDefaults>>` | `gpu.rs:4801` (field on `Watchlist`) | Global | Mirror only |
| `Store<AlertsState>` | `Arc<Store<AlertsState>>` | `gpu.rs:4809` (field on `Watchlist`) | Global | Mirror only |
| `Store<SidebarState>` | `Arc<Store<SidebarState>>` | `gpu.rs:4817` (field on `Watchlist`) | Global | Mirror only |
| `CmdPaletteState` | serialised ad-hoc at save | `gpu.rs:7039` | Global | Ad-hoc |
| `SubscriptionBus` | field on `Watchlist` | `gpu.rs:4770`, `state/subscriptions.rs:141` | Global | Cross-pane events only |
| `InFlightRegistry` | field on `Watchlist` | `gpu.rs:4776`, `state/inflight.rs` | Global | Loading flags |
| `AppCommand` queue | `thread_local! QUEUE` | `commands.rs:191-193` | Render thread | Transient (drained per frame) |
| `ChartState` (Gen 2) | `chart/state/mod.rs:113` | `chart/state/mod.rs:113` | Future per-pane | NOT wired |
| `ORDER_MANAGER` | `OnceLock<Mutex<OrderManager>>` | `trading/order_manager.rs:569` | Global singleton | Yes |
| `ORDERS_SNAPSHOT` | `OnceLock<Mutex<Arc<OrdersSnapshot>>>` | `trading/snapshot.rs:18` | Global singleton | Yes |
| `ACCOUNT_DATA` | `OnceLock<Mutex<Option<…>>>` | `trading/mod.rs:168` | Global singleton | Yes |
| `STATE` (ApexData) | `OnceLock<State>` (18 inner mutexes) | `data/feeds/apex_data/live_state.rs:75` | Global singleton | Yes |
| `NATIVE_CHART_TXS` | `OnceLock<Mutex<Vec<mpsc::Sender<ChartCommand>>>>` | `lib.rs:43` | Global | ChartCommand delivery |
| Design tokens / themes | `OnceLock<RwLock<Vec<Theme>>>` | `gpu.rs:432` | Global | Yes |
| `STYLE_STORE` | `OnceLock<RwLock<…>>` | `chart/renderer/ui/style.rs:2342` | Global | Yes |
| `thread_local` side-channels | 9 cells (window, toasts, crosshair, etc.) | `gpu.rs:89-101` | Render thread | Yes |
| Journal WAL lock | `static Mutex<()>` | `trading/journal/wal.rs:19` | Global | Yes |
| Per-panel async singletons | Various (`ReplayPaneState`, spike popup, provenance, etc.) | `panels/replay_pane.rs:241`, `panels/spike_popup.rs:137`, `panels/provenance_pane.rs:61` | Global | Yes |
| `egui ctx.data()` | egui memory map | N/A | Frame-scoped | Ephemeral UI |

### The two god-objects in detail

**`Watchlist`** (`gpu.rs:4440–4828`, ~390 named fields) is the global app
shell: watchlist items and sections, all sidebar open/close booleans, UI
preferences (font, scale, compact mode), trading defaults, alert list, options
chain data, workspace/layout split ratios, command-palette frecency, play
system, scanner, heatmap, Discord, news, and more. Held as a single `&mut
Watchlist` passed through every render call.

**`Chart`** (`gpu.rs:1588–1948`, ~360 named fields) is the per-pane model:
symbol, timeframe, bars, timestamps, drawings, indicators, orders, alerts, DOM,
replay state, signal overlays, tab data, volume analytics, and all per-pane UI
scratch state. Every visible pane has one; the array lives in the `ChartWindow`
struct in `native_main.rs`.

---

## 3. How State Is Mutated

### 3a. Direct `&mut` writes (Gen 0 — dominant path)

The vast majority of mutations happen inside the render tree itself, where every
render function receives `chart: &mut Chart` and `watchlist: &mut Watchlist`.
Code simply assigns fields directly:

```rust
watchlist.font_scale = 1.8;
chart.show_volume = true;
```

This is the only path that the sacred GPU paint pipeline in `core.rs` interacts
with. There are no hooks, no versions, no observers.

### 3b. `AppCommand` queue (Gen 1)

**What it is.** A `thread_local` `Vec<AppCommand>` (`commands.rs:191-193`).
Components call `commands::push(AppCommand::Foo{…})` instead of mutating state
inline. At the end of each frame, `draw_chart` in `core.rs` calls
`commands::drain_and_dispatch(panes, watchlist)` (`core.rs:11748`), which
dispatches to the `dispatch()` reducer in `commands.rs:212`.

**What it covers.** 42 variants (as of writing) organised into:
- Alerts: `AddPriceAlert`, `PlaceDraftAlert`, `PlaceAllDraftAlerts`,
  `CancelPaneAlert`, `CancelWatchlistAlert`, `SnoozeAlert`
- Orders: `CancelOrder`, `PlaceAllDraftOrders`, `CancelAllOrders`,
  `ClearOrderHistory`, `PlaceSelectedOrders`, `CancelSelectedOrders`
- Indicators: `AddIndicator`, `RemoveIndicator`, `ToggleIndicatorVisibility`,
  `MoveIndicator`, `OpenIndicatorEditor`, `CloseIndicatorEditor`,
  `RecomputeIndicators`
- Pane/layout: `ChangePaneType`, `SwapPaneSymbol`, `ChangeTimeframe`
- Settings: `SetThemeIdx`, `SetStyleIdx`
- Watchlist structure: 17 variants (`WatchlistAddSymbol` through
  `WatchlistRenameActive`)

**When to use it.** Use `AppCommand` for any action that falls into one of the
groups above, or any new mutation you want to be replayable / testable / fired
from multiple input surfaces (hotkey, palette, Stream Deck).

### 3c. `ChartCommand` ingestion (data-ingestion channel)

Background threads deliver market data to the render thread through a
`mpsc::Sender<ChartCommand>`. The global sender registry is
`NATIVE_CHART_TXS` (`lib.rs:43`). The render loop drains it once per
frame via `route_commands` (`gpu.rs:3444`), called from `core.rs:10961`.

`ChartCommand` variants include `LoadBars`, `AppendBar`, `UpdateLastBar`,
`WatchlistPrice`, `TapeEntry`, `ScannerPrice`, `LoadDrawings`, `ChainData`,
`TrendHealthUpdate`, `PatternLabels`, and ~30 more (`mod.rs:811–~1100`). These
are purely inbound from data threads and are not emitted by UI code.

### 3d. `SubscriptionBus` (cross-pane fan-out)

`Watchlist::subscriptions` (`gpu.rs:4770`) is a queue-backed bus
(`state/subscriptions.rs:141`). Publishers push `PaneEvent` variants
(`SymbolChanged`, `TimeframeChanged`, `ToggleChanged`, `IndicatorAdded`, etc.)
with `publish` or `publish_from`. The render loop drains the bus once per frame
at a point where `&mut Vec<Chart>` is available without borrow conflicts — this
sidesteps the fan-out deadlock that a synchronous listener model would hit.

### 3e. `Store<T>::update` (Gen 1b — mirror writes)

The four wired `Store<T>` fields on `Watchlist` expose a `store.update(|s| {
… })` API that bumps a version counter and starts a debounce clock. The
`persist_supervisor` thread (`state/persist_supervisor.rs`) ticks every 50 ms
and flushes any store whose 200 ms debounce window has elapsed.

**Important:** writes via `Store::update` do NOT propagate back to the matching
`Watchlist` flat fields. The flat fields remain the read source of truth. To
stay in sync, callers must also call the matching `update_*` / `sync_from_*`
helpers on `Watchlist` that mirror flat ↔ store in both directions. In
practice, most Gen 0 mutations bypass the stores entirely (the desync hazard
described in Section 6).

---

## 4. Persistence

### Files on disk

macOS path root: `~/Library/Application Support/apex-terminal/`

| File | Format | Version | What it holds |
|---|---|---|---|
| `native-chart-state.json` | Ad-hoc JSON | `"version": 3` | Panes (symbols, timeframes, indicators, toggles), layout, theme, settings blob, watchlists |
| `workspaces/<name>.json` | Ad-hoc JSON | `"version": 2` | Named workspace snapshots — lags the main file by one field generation |
| `hotkeys.json` | Ad-hoc JSON | — | User-defined hotkey bindings |
| `watchlists.json` | Ad-hoc JSON | — | Saved named watchlists |
| `ui_settings.json` | `Persistable` envelope | `VERSION: 1` | `UiSettings` aggregate |
| `trading_defaults.json` | `Persistable` envelope | `VERSION: 1` | `TradingDefaults` aggregate |
| `alerts_state.json` | `Persistable` envelope | `VERSION: 1` | `AlertsState` aggregate |
| `sidebar_state.json` | `Persistable` envelope | `VERSION: 1` | `SidebarState` aggregate |
| `cmd_palette_state.json` | `Persistable` envelope | `VERSION: 1` | `CmdPaletteState` frecency |
| `templates/*.json` | Ad-hoc JSON | — | Saved pane templates |
| PostgreSQL | SQL | — | Drawings (canonical source), journal entries |

The `Persistable` envelope format (`state/persistence.rs`) wraps any aggregate
as `{"key": "...", "version": N, "payload": {...}}`, with a `migrate()` hook
called when the on-disk version is older than the current `VERSION`.

### `Store<T>` / supervisor mechanism

1. A mutation calls `store.update(|s| { … })`, which bumps `version` (an
   `AtomicU64`) and stamps `last_mutated` (`Mutex<Option<Instant>>`).
2. The `persist_supervisor` thread (`state/persist_supervisor.rs:21`) loops
   every 50 ms, calls `store.needs_persist()` (true once 200 ms have elapsed),
   calls `store.flush()` to write the `Persistable` JSON envelope, then calls
   `store.mark_persisted()` to clear the clock.
3. Failures are routed to `errors_sink::report(Warn, …)` — never panicked.

The `StoreRegistry` holds `Arc<dyn PersistableStore>` handles so the supervisor
can walk all registered stores without knowing their concrete types.

### Init / load order

On startup (`gpu.rs:6236`), `load_state()` reads `native-chart-state.json`
(version 3 path, with legacy version-2 fallback) and reconstructs `Vec<Chart>`,
`Layout`, and flat `Watchlist` settings from the JSON blob. Separately,
`pull_from_ui_settings()` on `Watchlist` copies the `UiSettings` aggregate back
into the legacy flat fields after loading. The `persist_supervisor` is spawned
at startup holding an `Arc<StoreRegistry>` that contains the four wired
aggregate stores.

---

## 5. Per-Pane vs Global

**The rule:** anything that varies per chart pane lives on `Chart`; anything
that is shared across the whole app lives on `Watchlist`.

Examples of correctly scoped per-pane state: `symbol`, `timeframe`, `bars`,
`indicators`, `orders`, `draw_tool`, `theme_idx`, `replay_mode`.

Examples of correctly scoped global state: `sections` (watchlist items),
`font_scale`, `hotkeys`, `active_workspace`, `pane_split_h/v`, layout
favorites, `alerts` (cross-pane list), Discord state.

**The hazard.** The render loop calls `render_chart_pane` once per visible
pane. Inside that function, keyboard handling (`core.rs:10382-10384`) is gated
to the active pane via `if pane_idx == *active_pane`. However, `handle_keyboard_shortcuts`
(`render/pane/keyboard_shortcuts.rs:20`) receives both `&mut Chart` (the active
pane's data) and `&mut Watchlist` (global). Any keyboard handler that writes to
`watchlist` fields mutates global state from inside what is conceptually a
per-pane call. Without the `pane_idx == *active_pane` guard, global state (and
`watchlist` fields) would be written N times — once per pane. The guard exists
and is respected for keyboard handling, but the pattern of passing `&mut
Watchlist` into per-pane render functions creates ongoing temptation to write
global state from a per-pane context.

---

## 6. Known Hazards and Gotchas

### 6a. Keyboard input fan-out class

`render_chart_pane` is called once per visible pane. egui's `ui.input(|i|
i.key_pressed(…))` is not consumed per call — without the
`if pane_idx == *active_pane` guard, every key event would fire once per pane.
Several sites in `core.rs` (lines `10480`, `10485`, `10524`, `10548`) apply
this guard correctly. Any new keyboard handling inside the pane render loop
must replicate this guard or it will fire N times.

### 6b. Dual-write desync (Store mirrors vs flat fields)

The four aggregate `Store<T>` fields on `Watchlist` are mirrors of the
corresponding flat fields. The two directions are bridged by
`push_to_ui_settings` / `pull_from_ui_settings` (and equivalent helpers for
the other stores), which are called explicitly at save time and load time.

Between those two events, the flat fields and the store can diverge. Any
mutation that writes directly to a flat field (`watchlist.font_scale = x`) and
does not also call `store.update(|s| { s.font_scale = x; })` will be lost from
the store's perspective. The `persist_supervisor` only flushes store state;
the main-file save flushes flat-field state. If the app is closed between a
flat write and the next explicit `save_state`, only the flat write survives.
Conversely, a `store.update` without a matching flat write means the hot render
path (which reads flat fields) sees stale data.

### 6c. Workspace v2 lag

`workspaces/<name>.json` is written by `save_workspace` (`gpu.rs:7154`) which
serialises to the version-2 JSON schema (field `"version": 2`). The main state
file (`native-chart-state.json`) is written by `save_state` and targets
`"version": 3`. Version 3 adds per-pane option-contract state, session-shading
fields, indicator band styling, `bar_source`, and the `settings` blob.
Workspace files saved before those fields were added will load without them,
silently defaulting. There is no migration path for workspace files.

### 6d. `LayoutState` and `ChatState` aggregates not yet wired

`LayoutState` and `ChatState` are defined in `state/aggregates.rs` with
`Persistable` implementations, but their corresponding `Store<T>` fields are
not yet on `Watchlist` and no `persist_supervisor` registration exists for
them. They will not survive restarts until a future wave wires them.

### 6e. `ChartState` (Gen 2) exists only as a schema

`chart/state/mod.rs` defines `ChartState`, `Drawing`, `Annotation`,
`IndicatorRef`, `Viewport`, `StyleTable`, etc. using SlotMap-keyed collections.
The `#![allow(dead_code)]` at the top of that module (`mod.rs:12`) confirms it
is not connected to any runtime path. It is a design model for a future storage
architecture, not live code.

---

## 7. Guidance for Contributors

### Adding a new piece of state

1. **Decide scope.** If the value is specific to one chart pane, put it on
   `Chart`. If it is app-wide or shared across panes, put it on `Watchlist`.
   When in doubt, per-pane is safer — it is harder to promote to global than
   to demote.

2. **Should it survive restart?** If yes, you need persistence. There are two
   options:
   - **Small, focused slice:** define or extend a `Persistable` aggregate in
     `state/aggregates.rs`, create a `Store<T>` field on `Watchlist`, register
     it in the `StoreRegistry` at startup, and add a flat-field mirror with
     `push_to_*` / `pull_from_*` helpers for the legacy save/load path. The
     `persist_supervisor` will flush it automatically.
   - **Temporarily:** add it to the `save_state` JSON blob in `gpu.rs:7047+`
     and restore it in `load_state`. This is the Gen 0 path and does not
     require a new aggregate.

3. **Which mutation path?** Use `AppCommand` if the mutation is an action that
   could plausibly be triggered from a hotkey, command palette, or remote
   control. Use direct `&mut` writes for ephemeral UI scratch state (popup
   positions, hover flags, input buffers). Use `Store<T>::update` for
   persistent aggregate mutations after the store is wired. Do not call
   `Store::update` inside a per-pane render function without the
   `pane_idx == *active_pane` guard.

4. **Adding an `AppCommand` variant.** Add the variant to the `AppCommand`
   enum in `commands.rs:67`, add a match arm in `dispatch()` at `commands.rs:212`,
   and emit it from UI code with `commands::push(AppCommand::YourVariant{…})`.
   The drain/dispatch cycle happens at end-of-frame (`core.rs:11748`).

5. **Adding a `PaneEvent`.** Add a variant to `PaneEvent` in
   `state/subscriptions.rs:73`, add a handler arm in
   `gpu.rs::apply_pane_events` (which the render loop calls after draining the
   bus), and publish from the originating site with
   `watchlist.subscriptions.publish(PaneEvent::YourVariant{…})`.

---

## 8. Migration Status and Roadmap

### What is intended

The intended end-state (described in `chart/state/mod.rs` and the storage
architecture spec) is a `ChartState`-centred model where each pane has a
strongly-typed, SlotMap-backed state with a single persistence codec. All
mutations would flow through `AppCommand`, and the `Watchlist` god-object would
be decomposed into purpose-specific aggregates backed by `Store<T>`.

### What is complete

- `AppCommand` queue and reducer (`commands.rs`) — fully functional, ~42
  variants.
- `Store<T>` + `StoreRegistry` + `persist_supervisor` — functional and tested
  (`state/store.rs`, `state/persist_supervisor.rs`).
- Four wired aggregates with `Store<T>` fields on `Watchlist`: `UiSettings`,
  `TradingDefaults`, `AlertsState`, `SidebarState`.
- `SubscriptionBus` and `InFlightRegistry` — wired and functional.
- `ChartState` data model and codecs in `chart/state/` — schema complete.

### What is incomplete

- **`ChartState` is not wired.** The render loop still reads `Chart` directly.
  Connecting `ChartState` would require replacing `Vec<Chart>` as the runtime
  store, wiring the codec boundary, and routing mutations through the new model.
- **Two aggregates are unregistered:** `LayoutState` and `ChatState` have no
  `Store<T>` fields on `Watchlist` and are not flushed by the supervisor.
- **~290 direct flat-field mutations bypass the Store mirrors.** Every write
  to a `Watchlist` field that has a corresponding aggregate field is a potential
  desync. Completing the migration means adding `store.update(…)` alongside
  every such write, or moving the read source of truth from the flat field to
  the store.
- **Workspace files are v2.** They will need a migration path or a version bump
  before they can reliably round-trip all v3 fields.
- **`AppCommand` covers ~5% of mutations.** The other ~95% of `Watchlist`
  mutations are still direct writes. Completing the migration means adding a
  command variant per distinct action and replacing the inline write with a
  `commands::push(…)` call — feasible incrementally, but a large body of work.

### What completing it would entail

1. Wire `ChartState` as the runtime model for chart panes (replaces `Vec<Chart>`
   in `ChartWindow`). This is the largest single change and touches `core.rs`.
2. Register `LayoutState` and `ChatState` stores and add `push_to_*` /
   `pull_from_*` helpers.
3. Sweep the remaining ~290 direct `Watchlist` flat-field mutations: add
   `store.update(…)` alongside each write for fields that have an aggregate
   counterpart.
4. Migrate workspace files to v3 schema.
5. Progressively add `AppCommand` variants for the remaining inline mutations,
   starting with the ones most likely to be triggered from multiple surfaces
   (symbol search, drawing operations, layout changes).
