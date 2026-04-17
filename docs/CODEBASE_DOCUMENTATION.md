# Apex Terminal Codebase Documentation

## 1. Executive Summary

Apex Terminal is a multi-runtime trading platform composed of:

1. A React + WebGPU frontend (`src/`) for charting, overlays, watchlists, orders, and UI orchestration.
2. A Rust/Tauri desktop backend (`src-tauri/`) that provides IPC commands, native chart windows, persistence bridges, sidecar lifecycle management, feed ingestion, and monitoring.
3. A Node/Fastify service (`ococo-api/`) that acts as a signal/data backend (annotations, alerts, symbol catalog, trendline detection, Redis pub/sub bridge, and ingestion jobs).

The project supports both:

1. Hybrid desktop mode (Tauri host + WebView frontend + optional native GPU windows).
2. Standalone native chart mode (`apex-native` binary) using Rust + wgpu + egui directly.

## 2. Repository Layout

- `src/`: React frontend and WebGPU chart runtime.
- `src-tauri/`: Rust crate for Tauri app and native chart renderer.
- `ococo-api/`: Fastify backend for annotations/signals/data ingestion.
- `scripts/`: build and sidecar helper scripts.
- `tasks/`: feature planning markdown.
- `docs/superpowers/`: architecture/refactor plans/specs.

Primary entrypoints:

- Frontend bootstrap: `src/main.tsx`
- React root layout: `src/App.tsx`
- Tauri app bootstrap: `src-tauri/src/lib.rs`
- Native standalone app: `src-tauri/src/native_main.rs`
- OCOCO API bootstrap: `ococo-api/src/index.ts`

## 3. High-Level Architecture

### 3.1 Process Topology

In desktop mode, the runtime graph is:

1. Tauri host process (Rust)
2. WebView process running Vite/React app
3. Optional sidecar process (`ococo-api` binary)
4. Optional native chart render thread/windows (wgpu + winit)
5. External infra: PostgreSQL, Redis, InfluxDB, yfinance sidecar, ibserver WS, ApexSignals WS, ApexCrypto HTTP/WS

### 3.2 Data Plane Layers

1. Frontend data acquisition via `DataProvider` abstraction (`src/data/DataProvider.ts`).
2. In-memory columnar store (`ColumnStore`) managed by `DataStore` with pagination, incremental ticks, LRU pair eviction, and metrics.
3. Multi-tier bar retrieval fallback chain in `IBKRProvider` / Rust `get_bars`:
   - Redis/OCOCO cache
   - yfinance sidecar
   - direct Yahoo fallback (Rust path)
4. Signal and annotation persistence through `DrawingRepository` abstraction:
   - OCOCO REST/WS (preferred)
   - Tauri IPC -> PostgreSQL
   - IndexedDB fallback

### 3.3 Rendering Plane Layers

Frontend:

1. `RenderEngine` manages WebGPU device lifecycle and recovery.
2. `FrameScheduler` batches dirty pane render submissions.
3. `PaneContext` owns per-pane GPU buffers/renderers and incremental updates.
4. `ChartPane` executes direct interaction loop (pan/zoom/wheel/crosshair/overlay/order UI).

Native Rust chart:

1. `chart_renderer/gpu.rs` is the monolithic immediate-mode renderer and app state host.
2. Receives `ChartCommand` messages from Tauri commands/feeds.
3. Supports extensive signal/overlay/order/watchlist/panel behaviors in native mode.

## 4. Frontend (`src/`) Detailed Documentation

## 4.1 Bootstrap and Global Singletons

`src/main.tsx` startup sequence:

1. Initializes `RenderEngine` and `IndicatorEngine`.
2. Initializes IndexedDB bar cache (`BarCache`).
3. Initializes drawing persistence using fallback chain:
   - `OcocoClient` (`http://192.168.1.60:30300`)
   - `TauriDrawingRepository` (IPC)
   - `LocalDrawingRepository` (IndexedDB)
4. Creates `IBKRProvider` and `DataStore`.
5. Wires provider tick callbacks into `DataStore.applyTick` and native chart tick forwarding (`native_chart_tick`).
6. Subscribes provider for configured panes.
7. Connects provider and registers reconnect/disconnect lifecycle hooks.
8. Stores globals via `setRenderEngine`, `setDataStore`, `setIndicatorEngine`, `setDataProvider` in `src/globals.ts`.
9. Starts frame scheduler and memory manager.
10. Registers Tauri event bridge (`native-chart-load`) for WebView->native chart bar hydration.
11. Mounts React root and adds visibility pause/resume behavior.

The bootstrap hardcodes OCOCO endpoint and is opinionated toward desktop deployment.

## 4.2 React Shell

`src/App.tsx` composes:

1. `Toolbar`
2. `Workspace`
3. `OrdersPanel`
4. `Watchlist`

Each area is wrapped in `ErrorBoundary` for compartmentalized fault isolation.

## 4.3 State Management (Zustand Stores)

`src/store/chartStore.ts`:

- Pane configuration (symbol/timeframe/indicator visibility/volume toggle).
- Multi-pane layout (`1`, `2`, `2h`, `3`, `4`, `6`, `6h`, `9`).
- Theme selection.
- Annotation filter toggles by timeframe/source/direction/method.

`src/store/drawingStore.ts`:

- Drawing CRUD, selection model (single + multiselect), hide/show controls.
- Group CRUD and style batch operations.
- Tool state machine (`cursor`, `trendline`, `hline`, `hzone`, `barmarker`).
- Background persistence via repository methods.

`src/store/orderStore.ts`:

- Per-pane order levels with OCO/trigger pair semantics.
- Status model (`draft`, `placed`, `executed`, `cancelled`).
- Toast stream and auto-TTL cleanup.
- Global order filtering and history maintenance.

`src/store/watchlistStore.ts`:

- Persisted watchlist symbols and options view state.
- Live price snapshots and previous close tracking.
- Modes: `stocks`, `chain`, `saved`.

## 4.4 Data Layer

`src/data/columns.ts` (`ColumnStore`):

- Columnar typed-array OHLCV representation.
- Tick application with candle rollover by timeframe interval.
- Capacity growth and eviction (max 20k bars in current constant, keep-ratio 75% on max eviction).
- Binary search time index and range min/max.
- Prepend for historical pagination.

`src/data/DataStore.ts`:

- Cache key: `symbol:timeframe`.
- Coalesced load promises to avoid duplicate network work.
- Cache-first load with optional background refresh.
- Incremental indicator updates based on tick action (`updated`/`created`).
- Subscriber fanout for chart panes.
- Pair-level LRU eviction (`MAX_CACHED_PAIRS = 10`) for inactive pairs.
- Explicit pressure API `evictAll()` used by memory manager.

`src/data/IBKRProvider.ts`:

- Hybrid real-time provider using:
  - Tauri event bridge (`ib-tick`) when running under Tauri
  - direct WebSocket fallback in browser/dev
  - simulation fallback when real feed unavailable or idle
- History retrieval chain:
  - OCOCO `/api/bars`
  - yfinance sidecar `/bars`
- Per-symbol contract resolution via `ibserver /contract/:symbol` and conId subscribe messages.

`src/data/DataProvider.ts` also includes an alternate `YFinanceProvider` implementation for historical and simulated ticking.

## 4.5 Rendering Engine

`src/engine/RenderEngine.ts`:

- Creates WebGPU adapter/device.
- Tracks engine state (`ready`, `recovering`, `failed`).
- Handles device loss with exponential backoff recovery and pane reconfigure.
- Notifies state listeners and device replacement listeners.

`src/engine/FrameScheduler.ts`:

- Dirty-pane scheduling, rAF-driven only when necessary.
- Separate compute and render command submissions for fault isolation.
- Maintains frame timing/update-rate stats.

`src/engine/PaneContext.ts`:

- Owns per-pane GPU resources:
  - candle renderer
  - volume renderer
  - indicator line renderers
  - price-range compute pass
  - overlay renderer
- Uses incremental GPU writes for last/append updates.
- Handles full reload on symbol switches or eviction events.
- Supports immediate `forceRender()` for low-latency interactions.

## 4.6 Chart UI and Interaction

`src/chart/ChartPane.tsx` is the hot interaction component:

- Registers pane with render engine.
- Subscribes provider + datastore updates for symbol/timeframe.
- Uses imperative rAF loop for viewport updates without React churn.
- Implements drag pan, axis zoom, drag-zoom rectangle, wheel zoom, crosshair, overlays.
- Context menu integration for chart actions/order entry.
- Integrates `DrawingOverlay`, `AxisCanvas`, `OrderEntry`, `OrderLevels`.
- Implements native-chart data bridge through Tauri events/invoke.

`src/workspace/Workspace.tsx`:

- Grid and custom 3-pane layout orchestration.
- Responsive pane sizing using `ResizeObserver`.
- Virtualized pane paging when pane count exceeds layout capacity.

## 4.7 Toolbar / Watchlist / Orders

`src/toolbar/Toolbar.tsx`:

- Timeframe/layout/theme/tool toggles.
- Indicator/volume toggles per active pane.
- Watchlist/book/order-entry toggles.
- Tauri-only controls:
  - open extra WebView window
  - open native GPU chart (`open_native_chart` command)
  - custom window controls (minimize/maximize/close)

`src/watchlist/Watchlist.tsx`:

- Symbol search and local symbol suggestion integration.
- Tick subscription for watchlist pricing.
- Mode switching across stocks/options-chain/saved-options.

`src/orders/OrdersPanel.tsx`:

- Grouped visual model for single/OCO/trigger order structures.
- Trigger auto-detection polling against latest chart close.
- Bulk actions (place all, cancel all, clear history).

## 4.8 Theming

`src/themes.ts` defines a typed theme registry and GPU-ready RGBA conversions.
Built-in themes include Midnight, Nord, Monokai, Solarized Dark, Dracula, Gruvbox, Catppuccin, Tokyo Night.

## 4.9 Frontend Testing

Vitest suites under `src/tests/` cover:

1. Coordinate mapping (`CoordSystem.test.ts`)
2. Columnar store behavior and tick/update/evict paths (`columns.test.ts`)
3. Incremental indicator correctness against naive implementations (`indicators.test.ts`)
4. IndicatorEngine lifecycle (`indicatorEngine.test.ts`)
5. Drawing store behavior (`drawingStore.test.ts`)

## 5. Rust/Tauri Backend (`src-tauri/`) Detailed Documentation

## 5.1 Crate Composition

Key modules exposed by `src-tauri/src/lib.rs`:

- `data`: bars/options retrieval commands.
- `drawings`: PostgreSQL drawing CRUD + groups via Tauri commands.
- `ib_ws`: Rust-native IB WebSocket path and control channel command.
- `chart_renderer`: native GPU chart system.
- `bar_cache`: Redis cache utility.
- `drawing_db`: direct DB worker for native chart path.
- `monitoring`: metrics, jank/leak telemetry, Prometheus endpoint.
- `discord`: OAuth2 + channel/message integration.
- `crypto_feed`: ApexCrypto real-time feed.
- `signals_feed`: ApexSignals real-time feed.

## 5.2 Tauri Setup and Lifecycle

`run()` in `lib.rs` does:

1. Plugin registration (`opener`, `shell`).
2. Optional PostgreSQL pool creation + schema migration (`drawings::ensure_schema`).
3. Redis bar cache init.
4. Monitoring startup.
5. Discord config load.
6. Crypto feed start.
7. Signals feed start.
8. IB websocket task spawn.
9. Sidecar spawn (`ococo-api`) via shell plugin, log stream drain.
10. Command handler registration.
11. Exit hook killing sidecar child process.

## 5.3 Exposed Tauri Commands

From `invoke_handler` and command definitions:

- `open_native_chart`
- `native_chart_data`
- `native_chart_tick`
- `get_bars`
- `get_options_chain`
- Drawings commands:
  - `drawings_load_all`
  - `drawings_load_symbol`
  - `drawings_save`
  - `drawings_update_points`
  - `drawings_update_style`
  - `drawings_remove`
  - `drawings_clear`
- Groups commands:
  - `groups_load_all`
  - `groups_save`
  - `groups_remove`
  - `groups_update_style`
  - `drawings_apply_group_style`
- `ib_ws_send`

Permission mapping is defined in `src-tauri/permissions/default.toml`.

## 5.4 Native Chart Bridge

`native_chart_data`:

- Receives bar arrays from WebView.
- Caches bars into Redis via `bar_cache::set`.
- Converts bars to GPU-native structs and broadcasts `ChartCommand::LoadBars` to all native chart windows.

`native_chart_tick`:

- Receives single tick and broadcasts `ChartCommand::UpdateLastBar`.

Global sender registry uses `NATIVE_CHART_TXS` (`OnceLock<Mutex<Vec<Sender<ChartCommand>>>>`).

## 5.5 Data Retrieval in Rust

`src-tauri/src/data.rs` (`get_bars`):

- Crypto symbol detection routes to ApexCrypto HTTP API.
- Stocks path fallback chain:
  1. Redis cache (`bar_cache`)
  2. OCOCO API
  3. local yfinance sidecar
  4. direct Yahoo Finance v8 API parsing

`get_options_chain` proxies to yfinance sidecar `/options`.

## 5.6 IB WebSocket Hot Path

`src-tauri/src/ib_ws.rs`:

- Connects to `ws://127.0.0.1:5000/ws`.
- Decodes MessagePack tick frames in Rust.
- Emits `ib-tick` events to Tauri frontend.
- Restores conId subscriptions after reconnect.
- Supports JSON control frames through `ib_ws_send` command.

## 5.7 Drawings Persistence

`src-tauri/src/drawings.rs`:

- Defines drawing/group row structs and schema migrator (`ensure_schema`).
- Implements full CRUD and style update commands.
- Supports group-level batch style update in one SQL operation.

`src-tauri/src/drawing_db.rs`:

- Separate native path DB worker thread with persistent tokio runtime.
- Message-based DB operations (`DbOp`) to avoid per-call runtime creation and connection churn.

## 5.8 Redis Cache Module

`src-tauri/src/bar_cache.rs`:

- Persistent Redis connection with reconnection fallback.
- Key pattern `apex:bars:{SYMBOL}:{timeframe}`.
- Timeframe-specific TTL policy.

## 5.9 Monitoring

`src-tauri/src/monitoring.rs`:

- Global allocator stats (`CountingAlloc`).
- System/GPU/process/frame metrics sampling.
- Leak/jank tracking.
- HTTP Prometheus endpoint on `0.0.0.0:9091/metrics`.

## 5.10 Signals and Crypto Feeds

`signals_feed.rs`:

- Connects to `ws://localhost:8200/ws`.
- Subscribes to `patterns`, `alerts`, `trendlines`, `significance`.
- Translates feed messages into `ChartCommand` variants.

`crypto_feed.rs` (started at bootstrap) integrates real-time crypto stream forwarding.

## 5.11 Native Renderer Domain Model

`src-tauri/src/chart_renderer/mod.rs` defines:

- Core drawing model with many drawing kinds (trendlines, fibs, channel, pitchfork, VWAP, notes, etc.).
- Significance metadata model.
- `PatternLabel`, `SignalZone`, `DivergenceMarker`.
- Large `ChartCommand` enum for all data/UI/signal events.

`chart_renderer/gpu.rs` is the dominant module (~16k LOC) implementing the render loop, UI panels, command handling, and native interaction model.

## 6. OCOCO API Service (`ococo-api/`) Detailed Documentation

## 6.1 Service Boot

`ococo-api/src/index.ts`:

1. Creates Fastify app.
2. Registers CORS + websocket plugins.
3. Connects Redis clients.
4. Initializes Redis->WebSocket signal bus.
5. Verifies PostgreSQL availability.
6. Registers REST routes and WS endpoint.
7. Starts TTL reaper for expired annotations.
8. Schedules periodic ingestion and trendline detection cycles.

## 6.2 Data Stores and Responsibilities

PostgreSQL (`db.ts`, `migrate.ts`):

- `annotations`
- `alert_rules`
- `symbols`
- `recent_symbols`
- migration from legacy `drawings` table into annotations

Redis (`redis.ts`, `cache.ts`, `barCache.ts`):

- annotation cache (`ococo:ann:{symbol}`)
- signal pub/sub channels (`signals:{symbol}`)
- sorted-set working set for bars (`bars:{symbol}:{interval}`)

InfluxDB (`influx.ts`):

- deep OHLCV storage in bucket `stocks`, measurement `bars`.
- write/read/count APIs via Flux HTTP.

## 6.3 REST API Surfaces (`routes.ts`)

Core endpoints:

- Health: `/api/health`
- Annotation CRUD: `/api/annotations`, `/api/annotations/:id`, points/style patches, filtered delete
- Alerts CRUD: `/api/alerts`, `/api/alerts/:id`
- Symbols/search/recents: `/api/symbols`, `/api/recents`
- Trendline config/detection: `/api/trendlines/config`, `/api/trendlines/detect`
- Ingestion triggers: `/api/ingest/all`, `/api/ingest/symbol`
- Bars query/count/range: `/api/bars`, `/api/bars/count`, `/api/bars/range`

Bar reads prefer Redis working set first, then Influx fallback.

## 6.4 WebSocket Protocol (`ws.ts`)

Client messages:

- `subscribe` with symbols
- `unsubscribe` with symbols
- `price` updates for alert checks

Server messages:

- `snapshot`
- `signal`
- `signal_remove`
- `alert`
- `error`

On subscribe, snapshots are sent immediately for each symbol.

## 6.5 Signal Bus (`signalBus.ts`)

Responsibilities:

1. Track websocket clients and symbol subscriptions.
2. Manage dynamic Redis channel subscriptions only while needed.
3. Broadcast Redis signal messages to subscribed clients.
4. Publish signal/signal-remove events back to Redis from internal workflows.

## 6.6 Trendline Engines

`trendlines-v2.ts` (primary advanced engine):

- Methods: pivot, regression, fractal, volume-weighted, touch-density.
- Backtests each candidate line on forward action.
- Composite strength scoring and dedup.
- Persists `source='auto-trend'` annotations and publishes each signal.

`trendlines.ts` (legacy engine) remains present, with channel detection and simpler scoring.

## 6.7 Ingestion Pipeline (`ingest.ts`)

1. Fetches bars from yfinance sidecar for configured intervals.
2. Writes to Redis working set and Influx deep store.
3. Derives 4h bars by aggregating 1h bars.
4. Runs advanced detection after ingest.
5. Supports full-cycle and single-symbol operations.

## 7. Build, Run, and Packaging

## 7.1 Frontend

- Dev: `npm run dev`
- Build: `npm run build`
- Tests: `npm test`

Vite dev server is fixed at `5174` (`vite.config.ts`).

## 7.2 Tauri Desktop

- Dev: `npm run tauri dev`
- Build: `npm run tauri build`

Tauri config (`src-tauri/tauri.conf.json`):

- `beforeDevCommand`: `npm run dev`
- `devUrl`: `http://localhost:5174`
- external sidecar binary: `binaries/ococo-api`

## 7.3 Native Binary

- Standalone native chart build script: `build-native.sh`
- Native entrypoint: `src-tauri/src/native_main.rs`

## 7.4 Sidecar Packaging

`/scripts/build-sidecar.sh`:

1. Builds TypeScript (`ococo-api` -> `dist`).
2. Bundles with `pkg` into platform-specific executable.
3. Places output into `src-tauri/binaries` for Tauri bundling.

## 7.5 yfinance Sidecar

`/scripts/yfinance_server.py`:

- Tiny HTTP wrapper over `yfinance`.
- Endpoints: `/bars`, `/options`, `/health`.
- Expected local bind: `127.0.0.1:8777`.

## 8. Configuration and Environment Dependencies

Hardcoded endpoints appear across layers. Key defaults include:

- OCOCO API: `http://192.168.1.60:30300`
- Redis: `192.168.1.89:6379`
- PostgreSQL: `192.168.1.143:5432`
- InfluxDB: `192.168.1.67:8086`
- ibserver WS/HTTP: `localhost:5000`
- yfinance sidecar: `127.0.0.1:8777`
- ApexSignals WS: `ws://localhost:8200/ws`
- ApexCrypto API/WS references in Rust modules

Operational implication: this repo is currently tuned to a specific local/LAN environment and requires configuration externalization for portable deployment.

## 9. Notable Engineering Characteristics

## 9.1 Performance-Oriented Patterns

- Typed-array column stores for OHLCV and indicator vectors.
- Incremental last-bar/append GPU buffer writes instead of full reuploads.
- Dirty-pane frame scheduling to avoid constant redraw.
- Separate compute and render GPU submissions for resilience.
- Native event bridges for hot-path tick flow (Rust msgpack decode).

## 9.2 Resilience Patterns

- WebGPU device loss recovery with bounded retries.
- Multi-tier data fallbacks across cache/network providers.
- Drawing repository fallback chain (server -> IPC -> IndexedDB).
- Background refresh and cache invalidation strategies.

## 9.3 Extensibility Hooks

- `DataProvider` interface allows feed/backend swapping.
- `DrawingRepository` abstraction allows persistence backend swaps.
- `ChartCommand` enum provides a wide native message bus.
- OCOCO route and ws protocol supports external signal producers via Redis channels.

## 10. Risks and Technical Debt Observed

1. Multiple hardcoded credentials/endpoints in committed source create security and portability risk.
2. Monolithic native renderer file (`gpu.rs`) carries high cognitive and change risk.
3. Configuration layering is inconsistent (some `.env` support, many direct literals).
4. Some legacy docs (`AGENT_CONTEXT.md`, `AGENT_BRIEF.md`) contain stale statements versus current implementation.
5. Sidecar and infra dependencies are numerous; local onboarding requires substantial external services.

## 11. Suggested Documentation Maintenance Model

To keep docs current as the code evolves:

1. Treat this file as source-of-truth architecture map.
2. Add a short changelog section per release with:
   - new endpoints/commands
   - changed fallback behavior
   - new infra requirements
3. Update module responsibility table whenever a major file is split/refactored.

## 12. Key File Index

Frontend core:

- `src/main.tsx`
- `src/App.tsx`
- `src/workspace/Workspace.tsx`
- `src/chart/ChartPane.tsx`
- `src/engine/RenderEngine.ts`
- `src/engine/FrameScheduler.ts`
- `src/engine/PaneContext.ts`
- `src/data/DataStore.ts`
- `src/data/IBKRProvider.ts`
- `src/store/chartStore.ts`
- `src/store/drawingStore.ts`
- `src/store/orderStore.ts`
- `src/store/watchlistStore.ts`

Rust/Tauri core:

- `src-tauri/src/lib.rs`
- `src-tauri/src/native_main.rs`
- `src-tauri/src/data.rs`
- `src-tauri/src/drawings.rs`
- `src-tauri/src/ib_ws.rs`
- `src-tauri/src/bar_cache.rs`
- `src-tauri/src/drawing_db.rs`
- `src-tauri/src/monitoring.rs`
- `src-tauri/src/chart_renderer/mod.rs`
- `src-tauri/src/chart_renderer/gpu.rs`

OCOCO API core:

- `ococo-api/src/index.ts`
- `ococo-api/src/routes.ts`
- `ococo-api/src/ws.ts`
- `ococo-api/src/signalBus.ts`
- `ococo-api/src/annotations.ts`
- `ococo-api/src/alerts.ts`
- `ococo-api/src/symbols.ts`
- `ococo-api/src/trendlines-v2.ts`
- `ococo-api/src/ingest.ts`
- `ococo-api/src/influx.ts`
- `ococo-api/src/barCache.ts`
- `ococo-api/src/migrate.ts`

---

Last updated: 2026-04-16
Generated from direct source inspection of the current repository state.
