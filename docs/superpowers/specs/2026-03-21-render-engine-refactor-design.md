# Render Engine & Architecture Refactor — Design Spec

**Date:** 2026-03-21
**Scope:** Full refactor of GPU rendering, data pipeline, indicator computation, and chart components

## Goals

- Eliminate all race conditions in GPU lifecycle and rendering
- Zero-cost idle charts (dirty-flag rendering)
- O(1) per-tick indicator computation (incremental algorithms)
- Proper resource management (no leaks, graceful recovery)
- Clean module boundaries with strict public APIs
- Architecture ready for real-time pattern recognition and external feed server

## Non-Goals

- Implementing a real market data feed (designed for, not built)
- Pattern recognition system (architecture supports it, not in scope)
- UI/UX changes beyond GPU error overlays

---

## Module Structure

```
src/
├── engine/              # GPU lifecycle, frame scheduling, resource management
│   ├── index.ts         # Public API: RenderEngine, FrameScheduler
│   ├── RenderEngine.ts  # Owns GPU device, handles init/recovery/destroy
│   ├── FrameScheduler.ts # Single global rAF loop, renders all panes
│   ├── PaneContext.ts   # Per-pane GPU state (canvas context, renderers)
│   └── types.ts         # CoordSystem, shared GPU types
│
├── renderer/            # GPU renderers (stateless draw commands)
│   ├── index.ts         # Public API: CandleRenderer, GridRenderer, LineRenderer
│   ├── CandleRenderer.ts
│   ├── GridRenderer.ts
│   ├── LineRenderer.ts
│   └── shaders/
│
├── indicators/          # Incremental indicator engine
│   ├── index.ts         # Public API: IndicatorEngine, subscribe/query
│   ├── IndicatorEngine.ts  # Manages per-symbol incremental state
│   ├── incremental/     # O(1) per-tick implementations
│   │   ├── sma.ts
│   │   ├── ema.ts
│   │   └── bollinger.ts # Uses Welford's algorithm for numerical stability
│   └── types.ts
│
├── data/                # Data storage and feed ingestion
│   ├── index.ts         # Public API: DataStore, Feed interface
│   ├── DataStore.ts     # Per-symbol ColumnStore management + Tauri history loading
│   ├── columns.ts       # ColumnStore (hardened)
│   ├── Feed.ts          # Feed interface
│   ├── SimulatedFeed.ts # Current tick simulation, implements Feed
│   ├── timeframes.ts    # TF_TO_INTERVAL mapping (interval, period, seconds)
│   └── types.ts         # TickData, Bar, ColumnStore types
│
├── chart/               # React layer — thin wrappers only
│   ├── index.ts
│   ├── ChartPane.tsx    # ~80 lines: register pane, render overlays, forward events
│   ├── useChartViewport.ts  # Viewport state (pan, zoom, scroll, priceOverride, autoScroll)
│   ├── CrosshairOverlay.tsx
│   ├── DrawingOverlay.tsx
│   ├── LineStylePopup.tsx
│   └── AxisCanvas.ts    # Extracted 2D axis rendering (NEW file)
│
├── store/               # Zustand stores (unchanged)
│   ├── chartStore.ts
│   └── drawingStore.ts
│
├── toolbar/             # Toolbar (unchanged)
│   └── Toolbar.tsx
│
├── workspace/
│   └── Workspace.tsx
│
├── App.tsx              # Unchanged (renders Toolbar + Workspace)
├── main.tsx             # Updated: bootstrap() before React mount
└── types.ts             # Shared types (Timeframe, Bar)
```

### Module Boundary Rules

- Each module exports only through `index.ts`
- No cross-module imports except through barrel exports
- `engine/` has zero React dependencies
- `indicators/` has zero React, zero renderer, zero GPU dependencies
- `data/` depends only on `indicators/` and Tauri `invoke`
- `chart/` depends on `engine/`, `data/`, `store/`
- `renderer/` is consumed only by `engine/`
- `CoordSystem` lives in `engine/types.ts` (shared by renderer and chart layers)

---

## Engine: RenderEngine

Singleton. Created once at app startup, before React mounts.

```typescript
class RenderEngine {
  private device: GPUDevice
  private format: GPUTextureFormat
  private panes: Map<string, PaneContext>
  readonly scheduler: FrameScheduler
  private state: 'uninitialized' | 'ready' | 'recovering' | 'failed'
  private recoveryAttempts: number
  private stateListeners: Set<(state) => void>

  static async create(): Promise<RenderEngine>
  destroy(): void

  get gpuDevice(): GPUDevice  // read-only access for ComputeDispatcher

  registerPane(id: string, canvas: HTMLCanvasElement): PaneContext
  unregisterPane(id: string): void

  private onDeviceLost(info: GPUDeviceLostInfo): void
  private async recover(): Promise<void>
  retry(): void  // public: triggers recovery from 'failed' state
  onStateChange(cb: (state) => void): () => void
}
```

### Device Recovery

1. `device.lost` fires — state → `'recovering'`, scheduler pauses (stops submitting), notify subscribers
2. Re-request adapter + device (up to 3 attempts, 1s exponential backoff)
3. Re-configure every registered pane's canvas context (skip panes with detached canvases — unregister them)
4. Re-create all renderers with new device
5. Notify `onDeviceReplaced` listeners (IndicatorEngine's ComputeDispatcher updates its device ref)
6. State → `'ready'`, scheduler resumes, notify subscribers, next frame renders
7. All attempts fail → state → `'failed'`, panes show retry UI with `onClick={() => engine.retry()}`

**Rule:** The GPU device is never passed to React components. Renderers receive it at construction. RenderEngine replaces them on recovery. No stale device references.

**Device replacement notification:**
```typescript
onDeviceReplaced(cb: (device: GPUDevice) => void): () => void
```
Used by ComputeDispatcher to update its device reference after recovery.

---

## Engine: FrameScheduler

Single global `requestAnimationFrame` loop rendering all panes.

```typescript
class FrameScheduler {
  private panes: Map<string, PaneContext>
  private rafId: number | null
  private running: boolean
  private paused: boolean  // true during recovery
  private activePaneId: string | null  // renders first for responsiveness

  start(): void
  stop(): void
  pause(): void   // called during recovery — keeps rAF alive but skips submit
  resume(): void  // called after recovery
  markDirty(paneId: string): void
  setActivePaneId(id: string): void  // active pane renders first

  private tick(): void {
    if (this.paused) {
      this.rafId = requestAnimationFrame(() => this.tick())
      return
    }

    const commandBuffers: GPUCommandBuffer[] = []

    // Active pane first for perceived responsiveness
    if (this.activePaneId && this.panes.get(this.activePaneId)?.dirty) {
      this.renderPane(this.activePaneId, commandBuffers)
    }

    for (const [id, pane] of this.panes) {
      if (id === this.activePaneId) continue  // already rendered
      if (!pane.dirty) continue
      this.renderPane(id, commandBuffers)
    }

    if (commandBuffers.length > 0) {
      this.device.queue.submit(commandBuffers)
    }

    // Only schedule next frame if there might be more work
    // markDirty() restarts the loop if stopped
    if (this.running) {
      this.rafId = requestAnimationFrame(() => this.tick())
    }
  }

  private renderPane(id: string, buffers: GPUCommandBuffer[]): void {
    const pane = this.panes.get(id)!
    try {
      buffers.push(pane.render())
      pane.dirty = false
    } catch (e) {
      console.warn(`Pane ${id} render failed:`, e)
      pane.dirty = false  // don't retry broken frame endlessly
    }
  }
}
```

### Properties

- One rAF loop for all 7+ charts
- Dirty flag: idle charts cost zero
- Single `queue.submit()` batches all GPU work
- No promises in the render path — synchronous command recording only
- `markDirty()` called by data layer (tick) or viewport changes (pan/zoom)
- Per-pane try/catch: one broken pane doesn't block the rest
- Active pane renders first for perceived responsiveness
- Pauses during GPU recovery (keeps loop alive but skips rendering)

### Idle optimization

When no panes are dirty, the rAF loop can stop. `markDirty()` restarts it:

```typescript
markDirty(paneId: string): void {
  this.panes.get(paneId)!.dirty = true
  if (!this.rafId && this.running && !this.paused) {
    this.rafId = requestAnimationFrame(() => this.tick())
  }
}
```

---

## Engine: PaneContext

Per-chart GPU state managed by the engine.

```typescript
class PaneContext {
  readonly id: string
  dirty: boolean
  canvas: HTMLCanvasElement
  gpuContext: GPUCanvasContext
  renderers: { candle: CandleRenderer; grid: GridRenderer; lines: LineRenderer[] }

  // Data + viewport set by chart component
  data: ColumnStore | null
  indicators: IndicatorSnapshot | null
  viewport: { viewStart: number; viewCount: number; cs: CoordSystem }

  setViewport(v): void  // updates viewport, marks dirty via scheduler
  setData(d: ColumnStore, indicators: IndicatorSnapshot): void  // marks dirty

  render(): GPUCommandBuffer   // pure command recording, no side effects
  reconfigure(device, format): void  // called on recovery, checks canvas.isConnected
  destroy(): void              // destroys all renderers, unconfigures canvas
}
```

### Render Method

```typescript
render(): GPUCommandBuffer {
  const encoder = this.device.createCommandEncoder()
  const view = this.gpuContext.getCurrentTexture().createView()
  const pass = encoder.beginRenderPass({
    colorAttachments: [{
      view, loadOp: 'clear',
      clearValue: { r: 0.05, g: 0.05, b: 0.05, a: 1 },
      storeOp: 'store',
    }],
  })

  this.renderers.grid.upload(this.viewport.cs)
  this.renderers.candle.upload(this.data, this.viewport.cs, ...)
  for (const line of this.renderers.lines) { line.upload(...) }

  this.renderers.grid.render(pass)
  this.renderers.candle.render(pass)
  for (const line of this.renderers.lines) { line.render(pass) }
  pass.end()

  return encoder.finish()  // returns buffer, does NOT submit
}
```

### Canvas Resize

Handled via a `resize(width, height)` method with debouncing:

```typescript
private resizeTimer: number | null = null

resize(width: number, height: number): void {
  if (this.resizeTimer) clearTimeout(this.resizeTimer)
  this.resizeTimer = setTimeout(() => {
    const dpr = window.devicePixelRatio || 1
    this.canvas.width = Math.round(width * dpr)
    this.canvas.height = Math.round(height * dpr)
    this.canvas.style.width = width + 'px'
    this.canvas.style.height = height + 'px'
    this.gpuContext.configure({ device: this.device, format: this.format, alphaMode: 'premultiplied' })
    this.dirty = true
  }, 16)  // debounce to ~1 frame
}
```

---

## Indicators: IndicatorEngine

Manages per-symbol incremental state. Serves renderer and future pattern recognition. **No GPU dependency** — pure CPU computation.

```typescript
class IndicatorEngine {
  private state: Map<string, SymbolState>  // keyed by "AAPL:1m"
  private subscribers: Map<string, Set<(snapshot: IndicatorSnapshot) => void>>

  // Called by data layer when a tick arrives — O(1) per tick
  onTick(symbol: string, timeframe: string, price: number): IndicatorSnapshot

  // Called when chart loads history — CPU bootstrap, populates incremental state
  bootstrap(symbol: string, timeframe: string, data: ColumnStore): IndicatorSnapshot

  // Subscribe to indicator updates (for future pattern recognition)
  subscribe(symbol: string, timeframe: string, cb: (snapshot) => void): () => void

  // Query current state without subscribing
  getSnapshot(symbol: string, timeframe: string): IndicatorSnapshot | null

  // Tear down state for a symbol (when chart removed)
  remove(symbol: string, timeframe: string): void
}
```

**Note:** Bootstrap is synchronous CPU — even 5000 bars of Bollinger Bands computes in <1ms on CPU. GPU compute is reserved for the future pattern recognition system where workloads actually warrant it. This keeps `indicators/` free of GPU dependencies.

### Incremental State

```typescript
interface SymbolState {
  sma20: IncrementalSMA
  ema50: IncrementalEMA
  bollinger: IncrementalBollinger
}

interface IndicatorSnapshot {
  sma20: Float64Array
  ema50: Float64Array
  bbUpper: Float64Array
  bbLower: Float64Array
}
```

### Incremental Implementations

**IncrementalSMA:** Circular buffer + running sum. `push()` subtracts oldest, adds newest. O(1).

**IncrementalEMA:** Stores previous EMA value + multiplier. `push()` applies `EMA = price * k + prev * (1 - k)`. O(1).

**IncrementalBollinger:** Uses **Welford's online algorithm** for numerical stability. Maintains running mean and M2 (sum of squared differences from mean) over a circular buffer. On each tick:

```
// Remove oldest value x_old, add new value x_new:
old_mean = mean
mean += (x_new - x_old) / n
M2 += (x_new - old_mean) * (x_new - mean) - (x_old - old_mean) * (x_old - mean)
variance = M2 / n
stddev = sqrt(variance)
upper = mean + k * stddev
lower = mean - k * stddev
```

O(1) per tick, numerically stable even with large price values (BTC at 60k+) and extended periods.

---

## Data: DataStore

Owns all per-symbol ColumnStore instances. Bridges feed → indicators → renderer.

```typescript
class DataStore {
  private stores: Map<string, ColumnStore>  // keyed by "AAPL:1m"
  private indicatorEngine: IndicatorEngine
  private subscribers: Map<string, Set<() => void>>

  // Called by feed adapter — updates store, runs incremental indicators, notifies panes
  applyTick(symbol: string, timeframe: string, tick: TickData): void

  // Loads historical data via Tauri IPC, bootstraps indicators
  async load(symbol: string, timeframe: string): Promise<{ data: ColumnStore, indicators: IndicatorSnapshot }>

  // Pane subscription — notified on every tick for this symbol+tf
  subscribe(symbol: string, timeframe: string, cb: () => void): () => void

  // Read current state
  getData(symbol: string, timeframe: string): ColumnStore | null

  // Cleanup
  unload(symbol: string, timeframe: string): void
}
```

### History Loading (Tauri)

```typescript
async load(symbol: string, timeframe: string): Promise<...> {
  const { interval, period, seconds } = TF_TO_INTERVAL[timeframe]
  const bars: Bar[] = await invoke('get_bars', { symbol, interval, period })
  const store = ColumnStore.fromBars(bars)
  this.stores.set(key, store)
  const indicators = this.indicatorEngine.bootstrap(symbol, timeframe, store)
  return { data: store, indicators }
}
```

### Tick Flow

```
Feed.onTick()
  → DataStore.applyTick()
    → ColumnStore.applyTick()        // update/create candle
    → IndicatorEngine.onTick()       // O(1) incremental update
    → notify subscribers             // → PaneContext.setData() → markDirty
```

### Feed Interface

```typescript
interface Feed {
  connect(): Promise<void>
  disconnect(): void
  subscribe(symbol: string, timeframe: string): void
  unsubscribe(symbol: string, timeframe: string): void
  onTick(cb: (symbol: string, timeframe: string, tick: TickData) => void): () => void
  onDisconnect(cb: () => void): () => void
}
```

`SimulatedFeed` implements this now — 250ms interval, random walk per subscribed symbol. Future external feed server implements the same interface (WebSocket, Tauri IPC, whatever protocol).

### Timeframe Mapping

```typescript
// data/timeframes.ts — extracted from current useChartData

const TF_TO_INTERVAL: Record<Timeframe, { interval: string; period: string; seconds: number }> = {
  '1m':  { interval: '1m',  period: '1d',  seconds: 60 },
  '5m':  { interval: '5m',  period: '5d',  seconds: 300 },
  '15m': { interval: '15m', period: '5d',  seconds: 900 },
  '1h':  { interval: '1h',  period: '1mo', seconds: 3600 },
  '1d':  { interval: '1d',  period: '1y',  seconds: 86400 },
  '1wk': { interval: '1wk', period: '5y',  seconds: 604800 },
}
```

### ColumnStore Hardening

- **Auto-grow:** When capacity reached, allocate 2x arrays and copy. Never drop ticks.
- **Max capacity:** 50,000 bars per symbol. When exceeded, evict oldest 25% (shift arrays). Keeps memory bounded (~2.4MB per symbol for 6 Float64 columns).
- **Division-by-zero guard:** `priceRange()` returns `[min, min + 0.01]` when `min === max`.
- **Return type:** `applyTick()` returns `'updated' | 'created'` so IndicatorEngine knows whether to update last value or push new.

---

## Chart: Slim React Layer

### ChartPane (~80 lines)

```typescript
function ChartPane({ symbol, timeframe, width, height }: Props) {
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const paneRef = useRef<PaneContext | null>(null)
  const drawingRef = useRef<DrawingOverlayHandle>(null)
  const { viewport, pan, zoomX, zoomY, panY, resetYZoom, autoScrolling, pauseAutoScroll }
    = useChartViewport(symbol, timeframe, width, height)
  const [engineState, setEngineState] = useState<'ready' | 'recovering' | 'failed'>('ready')

  // Register/unregister with engine
  useEffect(() => {
    const engine = getRenderEngine()
    const pane = engine.registerPane(paneId, canvasRef.current!)
    paneRef.current = pane
    const unsub = engine.onStateChange(setEngineState)
    return () => { engine.unregisterPane(paneId); unsub() }
  }, [])

  // Handle resize
  useEffect(() => {
    paneRef.current?.resize(width, height)
  }, [width, height])

  // Subscribe to data updates → push to pane context
  useEffect(() => {
    const ds = getDataStore()
    ds.load(symbol, timeframe)  // async, populates store
    return ds.subscribe(symbol, timeframe, () => {
      const data = ds.getData(symbol, timeframe)
      const indicators = getIndicatorEngine().getSnapshot(symbol, timeframe)
      if (data && indicators) paneRef.current?.setData(data, indicators)
      if (viewport.cs) paneRef.current?.setViewport(viewport)
    })
  }, [symbol, timeframe, viewport])

  // Input handlers (zone detection, drawing delegation, pan/zoom)
  // ... same logic as current, no rendering code

  return (
    <div style={{ position: 'relative', width, height, background: '#0d0d0d', cursor: cursorStyle }}
      onMouseDown={onMouseDown} onMouseMove={onMouseMove}
      onMouseUp={onMouseUp} onMouseLeave={onMouseLeave}
      onWheel={onWheel} onDoubleClick={onDoubleClick} onAuxClick={onAuxClick}>
      <canvas ref={canvasRef} style={{ display: 'block', pointerEvents: 'none' }} />
      <AxisCanvas cs={viewport.cs} data={...} viewStart={viewport.viewStart}
        width={width} height={height} />
      <CrosshairOverlay ref={crosshairRef} ... />
      <DrawingOverlay ref={drawingRef} ... />
      {engineState === 'recovering' && (
        <div className="gpu-overlay">Reconnecting GPU...</div>
      )}
      {engineState === 'failed' && (
        <div className="gpu-overlay" onClick={() => getRenderEngine().retry()}>
          GPU unavailable — click to retry
        </div>
      )}
    </div>
  )
}
```

### useChartViewport (extracted from useChartData)

Owns all viewport state. Extracted from the current 200-line `useChartData`:

- `viewStart`, `viewCount` — visible window position
- `priceOverride` — manual Y-axis zoom range, with reset
- Auto-scroll logic — 10-second pause timeout, `autoScrollVersion` subscription from chartStore
- `RIGHT_MARGIN_BARS` constant and viewStart calculation
- `CoordSystem` computation — memoized on viewport + dimensions + data price range
- `pan()`, `zoomX()`, `zoomY()`, `panY()`, `resetYZoom()` — all call `scheduler.markDirty()`

Does NOT own: data fetching, tick simulation, indicator computation.

### AxisCanvas (extracted, new file)

Pure function extracted from ChartPane lines 114-144:

```typescript
// chart/AxisCanvas.ts
export function AxisCanvas({ cs, data, viewStart, width, height }: Props) {
  const ref = useRef<HTMLCanvasElement>(null)
  useEffect(() => { drawAxes(ref.current!, cs, data, viewStart, width, height) },
    [cs, data, viewStart, width, height])
  return <canvas ref={ref} width={width} height={height}
    style={{ position: 'absolute', top: 0, left: 0, pointerEvents: 'none' }} />
}
```

---

## App Bootstrap

```typescript
// main.tsx

async function bootstrap() {
  const engine = await RenderEngine.create()
  const indicatorEngine = new IndicatorEngine()
  const dataStore = new DataStore(indicatorEngine)
  const feed = new SimulatedFeed()

  feed.onTick((symbol, tf, tick) => dataStore.applyTick(symbol, tf, tick))
  await feed.connect()

  setRenderEngine(engine)
  setDataStore(dataStore)
  setIndicatorEngine(indicatorEngine)

  engine.scheduler.start()

  // React mounts only after GPU is guaranteed ready
  ReactDOM.createRoot(document.getElementById('root')!).render(<App />)
}

bootstrap().catch(err => {
  document.body.innerHTML = `<div style="color:#e74c3c;padding:40px;font-family:monospace">
    GPU initialization failed: ${err.message}<br><br>
    <button onclick="location.reload()">Retry</button>
  </div>`
})
```

**Key:** React never mounts until GPU is ready. No race conditions. No `gpuReady` state in components.

### Tab Visibility Handling

```typescript
// In bootstrap, after feed.connect():
document.addEventListener('visibilitychange', () => {
  if (document.hidden) {
    engine.scheduler.stop()  // no rAF while hidden
  } else {
    // Coalesce: mark all panes dirty once, don't replay every missed tick
    for (const pane of engine.panes.values()) pane.dirty = true
    engine.scheduler.start()
  }
})
```

---

## Error Handling

### GPU State Machine

```
uninitialized → ready → recovering → ready (success)
                                   → failed (3 attempts, exponential backoff)
failed → recovering (user calls retry())
```

### Recovery Guards

- **In-flight render:** Scheduler `pause()` is called immediately on `device.lost`. Current frame's `tick()` checks `paused` flag before `queue.submit()`. Any already-recorded command buffers are discarded (they're invalid anyway with a lost device).
- **Detached canvases:** During recovery step 3, `reconfigure()` checks `canvas.isConnected`. Panes with detached canvases are unregistered.
- **Ticks during recovery:** DataStore keeps updating ColumnStores and running incremental indicators (pure CPU, no GPU). Panes get marked dirty but scheduler skips submit. When recovery completes, next frame renders latest state.

### Per-Pane Overlays

- `recovering` → "Reconnecting GPU..." semi-transparent overlay
- `failed` → "GPU unavailable — click to retry" overlay with `onClick={engine.retry()}`

### Data Layer Errors

- `DataStore.load()` failure → pane shows "Failed to load data" with retry
- Feed disconnection (via `onDisconnect`) → DataStore pauses ticks, "Feed disconnected" badge
- ColumnStore at max capacity → evict oldest 25%, continue

---

## Memory Management

| Resource | Limit | Strategy |
|----------|-------|----------|
| ColumnStore per symbol | 50,000 bars max | Evict oldest 25% when full |
| Float64Arrays per symbol | ~2.4MB (6 columns × 50k × 8 bytes) | Bounded by bar limit |
| Total for 7 symbols | ~17MB | Acceptable |
| GPU buffers per pane | 6 renderers × 2 buffers | Destroyed on `unregisterPane()` |
| Indicator state per symbol | ~40KB (circular buffers + output arrays) | Destroyed on `remove()` |

---

## Performance Comparison

| Metric | Current | After |
|--------|---------|-------|
| GPU init | 7 parallel device requests (race) | 1 device, pre-initialized |
| Render scheduling | 7 async useEffect chains with Promises | 1 rAF loop, dirty flags |
| GPU submissions per frame | Up to 7 `queue.submit()` | 1 batched submit |
| Bollinger Bands per tick | O(n × period) recompute from scratch | O(1) incremental (Welford's) |
| SMA per tick | O(n) recompute from scratch | O(1) incremental |
| EMA per tick | O(n) recompute from scratch | O(1) incremental |
| Idle chart cost | Full recompute + render | Zero (dirty flag + idle rAF stop) |
| Resource cleanup on unmount | None (GPU buffer leak) | `unregisterPane()` destroys all |
| Device lost handling | Silent crash | Auto-recovery with UI feedback |
| ChartPane responsibility | 330 lines, 6 concerns | ~80 lines, React wrapper |
| Tab hidden cost | Full rAF + ticks | rAF stopped, ticks coalesced |

---

## Migration Notes

### Files Created (New)
- `src/engine/index.ts` — barrel export
- `src/engine/RenderEngine.ts`
- `src/engine/FrameScheduler.ts`
- `src/engine/PaneContext.ts`
- `src/engine/types.ts` — includes CoordSystem (relocated)
- `src/indicators/index.ts` — barrel export
- `src/indicators/IndicatorEngine.ts`
- `src/indicators/incremental/sma.ts`
- `src/indicators/incremental/ema.ts`
- `src/indicators/incremental/bollinger.ts`
- `src/indicators/types.ts`
- `src/data/DataStore.ts`
- `src/data/Feed.ts` — interface only
- `src/data/SimulatedFeed.ts` — current tick sim extracted
- `src/data/timeframes.ts` — TF_TO_INTERVAL mapping
- `src/data/types.ts`
- `src/chart/useChartViewport.ts` — viewport logic extracted from useChartData
- `src/chart/AxisCanvas.ts` — 2D axis rendering extracted from ChartPane

### Files Removed
- `src/renderer/gpu.ts` — absorbed into `RenderEngine`
- `src/data/indicators.ts` — replaced by `indicators/` module
- `src/chart/useChartData.ts` — split into `useChartViewport` + `DataStore` + `Feed`

### Files Significantly Changed
- `src/main.tsx` — bootstrap() with GPU init before React mount
- `src/chart/ChartPane.tsx` — rewritten as thin ~80-line wrapper
- `src/data/columns.ts` — hardened (auto-grow, max capacity, eviction, guards)
- `src/chart/CoordSystem.ts` — relocated to `src/engine/types.ts`

### Files Unchanged
- `src/renderer/CandleRenderer.ts` — same API, receives device via PaneContext
- `src/renderer/GridRenderer.ts` — same
- `src/renderer/LineRenderer.ts` — same
- `src/renderer/shaders/*` — same
- `src/chart/CrosshairOverlay.tsx` — same
- `src/chart/DrawingOverlay.tsx` — same
- `src/chart/LineStylePopup.tsx` — same
- `src/store/chartStore.ts` — same
- `src/store/drawingStore.ts` — same
- `src/workspace/Workspace.tsx` — same
- `src/toolbar/Toolbar.tsx` — same
- `src/App.tsx` — same (still renders Toolbar + Workspace)
