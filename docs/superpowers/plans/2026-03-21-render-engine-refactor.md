# Render Engine & Architecture Refactor — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Refactor the WebGPU multi-chart trading terminal from per-pane async rendering to a centralized RenderEngine with single rAF loop, incremental indicators, and clean module boundaries.

**Architecture:** Singleton RenderEngine owns GPU device and FrameScheduler. IndicatorEngine provides O(1) per-tick computation. DataStore bridges feed→indicators→renderer. ChartPane becomes a thin React wrapper. All GPU work batched into single queue.submit() per frame.

**Tech Stack:** TypeScript, React, WebGPU, Zustand, Tauri IPC, Vite

**Spec:** `docs/superpowers/specs/2026-03-21-render-engine-refactor-design.md`

---

## Task 1: Shared Types & CoordSystem Relocation

**Files:**
- Create: `src/engine/types.ts`
- Create: `src/data/types.ts`
- Create: `src/data/timeframes.ts`
- Modify: `src/types.ts`

- [ ] **Step 1: Create `src/engine/types.ts`**

Copy CoordSystem from `src/chart/CoordSystem.ts` and re-export it. Add GPUContext type and engine state type.

```typescript
// src/engine/types.ts
export { CoordSystem, type CoordConfig } from '../chart/CoordSystem'

export interface GPUContext {
  device: GPUDevice
  format: GPUTextureFormat
}

export type EngineState = 'uninitialized' | 'ready' | 'recovering' | 'failed'
```

Note: CoordSystem.ts stays in place for now (avoids breaking imports during migration). We re-export from engine/types.ts. We'll delete the old file and update imports at the end.

- [ ] **Step 2: Create `src/data/types.ts`**

```typescript
// src/data/types.ts
export interface TickData {
  price: number
  volume: number
  time: number  // unix seconds
}
```

- [ ] **Step 3: Create `src/data/timeframes.ts`**

Extract from `src/chart/useChartData.ts` lines 8-16:

```typescript
// src/data/timeframes.ts
import type { Timeframe } from '../types'

export const TF_TO_INTERVAL: Record<Timeframe, { interval: string; period: string; seconds: number }> = {
  '1m':  { interval: '1m',  period: '1d',  seconds: 60 },
  '5m':  { interval: '5m',  period: '5d',  seconds: 300 },
  '15m': { interval: '15m', period: '5d',  seconds: 900 },
  '1h':  { interval: '1h',  period: '1mo', seconds: 3600 },
  '4h':  { interval: '1h',  period: '3mo', seconds: 14400 },
  '1d':  { interval: '1d',  period: '1y',  seconds: 86400 },
  '1wk': { interval: '1wk', period: '5y',  seconds: 604800 },
}
```

- [ ] **Step 4: Commit**

```bash
git add src/engine/types.ts src/data/types.ts src/data/timeframes.ts
git commit -m "feat: add shared types, CoordSystem re-export, timeframe mapping"
```

---

## Task 2: Incremental Indicators

**Files:**
- Create: `src/indicators/incremental/sma.ts`
- Create: `src/indicators/incremental/ema.ts`
- Create: `src/indicators/incremental/bollinger.ts`
- Create: `src/indicators/types.ts`
- Create: `src/indicators/IndicatorEngine.ts`
- Create: `src/indicators/index.ts`

- [ ] **Step 1: Create `src/indicators/types.ts`**

```typescript
export interface IndicatorSnapshot {
  sma20: Float64Array
  ema50: Float64Array
  bbUpper: Float64Array
  bbLower: Float64Array
}
```

- [ ] **Step 2: Create `src/indicators/incremental/sma.ts`**

Circular buffer + running sum. O(1) per push.

```typescript
export class IncrementalSMA {
  private buffer: Float64Array  // circular buffer of raw values
  private sum: number = 0
  private pos: number = 0       // next write position in circular buffer
  private count: number = 0     // how many values have been pushed total
  private output: Float64Array  // full output array
  private outputLen: number = 0

  constructor(private period: number, capacity: number) {
    this.buffer = new Float64Array(period)
    this.output = new Float64Array(capacity)
  }

  /** Bootstrap from historical data */
  bootstrap(closes: Float64Array, length: number): void {
    this.sum = 0
    this.pos = 0
    this.count = 0
    this.ensureCapacity(length)
    for (let i = 0; i < length; i++) {
      this.pushInternal(closes[i], i)
    }
    this.outputLen = length
  }

  /** Push a new value (new bar) */
  push(value: number): void {
    this.ensureCapacity(this.outputLen + 1)
    this.pushInternal(value, this.outputLen)
    this.outputLen++
  }

  /** Update the last value (existing bar updated) */
  updateLast(value: number): void {
    if (this.outputLen === 0) return
    const idx = this.outputLen - 1
    // Undo last push: revert circular buffer position
    const prevPos = (this.pos - 1 + this.period) % this.period
    const oldValue = this.buffer[prevPos]
    this.sum -= oldValue
    this.sum += value
    this.buffer[prevPos] = value
    this.output[idx] = this.count >= this.period ? this.sum / this.period : NaN
  }

  getOutput(): Float64Array { return this.output }
  getLength(): number { return this.outputLen }

  private pushInternal(value: number, outputIdx: number): void {
    if (this.count >= this.period) {
      this.sum -= this.buffer[this.pos]
    }
    this.buffer[this.pos] = value
    this.sum += value
    this.pos = (this.pos + 1) % this.period
    this.count++
    this.output[outputIdx] = this.count >= this.period ? this.sum / this.period : NaN
  }

  private ensureCapacity(needed: number): void {
    if (needed <= this.output.length) return
    const newCap = Math.max(needed, this.output.length * 2)
    const newOut = new Float64Array(newCap)
    newOut.set(this.output.subarray(0, this.outputLen))
    this.output = newOut
  }
}
```

- [ ] **Step 3: Create `src/indicators/incremental/ema.ts`**

```typescript
export class IncrementalEMA {
  private k: number
  private prevEma: number = 0
  private output: Float64Array
  private outputLen: number = 0
  private count: number = 0

  constructor(private period: number, capacity: number) {
    this.k = 2 / (period + 1)
    this.output = new Float64Array(capacity)
  }

  bootstrap(closes: Float64Array, length: number): void {
    this.count = 0
    this.ensureCapacity(length)
    this.prevEma = closes[0]
    this.output[0] = NaN
    this.count = 1
    for (let i = 1; i < length; i++) {
      this.prevEma = closes[i] * this.k + this.prevEma * (1 - this.k)
      this.output[i] = i >= this.period - 1 ? this.prevEma : NaN
      this.count++
    }
    this.outputLen = length
  }

  push(value: number): void {
    this.ensureCapacity(this.outputLen + 1)
    this.prevEma = value * this.k + this.prevEma * (1 - this.k)
    this.output[this.outputLen] = this.count >= this.period - 1 ? this.prevEma : NaN
    this.outputLen++
    this.count++
  }

  updateLast(value: number): void {
    if (this.outputLen < 2) return
    // Recompute from previous EMA
    const prevIdx = this.outputLen - 2
    // We need the EMA value before the last push
    // For update, we recalculate: the prev EMA is based on output[prevIdx] if it was valid
    // Simpler: recalculate from the stored previous non-NaN value
    const prevEmaValue = this.outputLen >= this.period
      ? this.output[prevIdx]
      : this.prevEma // fallback
    if (!isNaN(prevEmaValue)) {
      this.prevEma = value * this.k + prevEmaValue * (1 - this.k)
    } else {
      this.prevEma = value * this.k + this.prevEma * (1 - this.k)
    }
    this.output[this.outputLen - 1] = this.count >= this.period ? this.prevEma : NaN
  }

  getOutput(): Float64Array { return this.output }
  getLength(): number { return this.outputLen }

  private ensureCapacity(needed: number): void {
    if (needed <= this.output.length) return
    const newCap = Math.max(needed, this.output.length * 2)
    const newOut = new Float64Array(newCap)
    newOut.set(this.output.subarray(0, this.outputLen))
    this.output = newOut
  }
}
```

- [ ] **Step 4: Create `src/indicators/incremental/bollinger.ts`**

Welford's online algorithm for numerical stability.

```typescript
export class IncrementalBollinger {
  private buffer: Float64Array  // circular buffer
  private pos: number = 0
  private count: number = 0
  private mean: number = 0
  private m2: number = 0        // sum of squared differences from mean (Welford's)
  private sum: number = 0       // running sum for SMA (middle band)
  private upperOut: Float64Array
  private lowerOut: Float64Array
  private outputLen: number = 0

  constructor(private period: number, private stdDevs: number, capacity: number) {
    this.buffer = new Float64Array(period)
    this.upperOut = new Float64Array(capacity)
    this.lowerOut = new Float64Array(capacity)
  }

  bootstrap(closes: Float64Array, length: number): void {
    this.pos = 0
    this.count = 0
    this.mean = 0
    this.m2 = 0
    this.sum = 0
    this.ensureCapacity(length)

    for (let i = 0; i < length; i++) {
      this.pushInternal(closes[i], i)
    }
    this.outputLen = length
  }

  push(value: number): void {
    this.ensureCapacity(this.outputLen + 1)
    this.pushInternal(value, this.outputLen)
    this.outputLen++
  }

  updateLast(value: number): void {
    if (this.outputLen === 0) return
    const idx = this.outputLen - 1
    const prevPos = (this.pos - 1 + this.period) % this.period
    const oldValue = this.buffer[prevPos]

    // Reverse the last push using Welford's update
    if (this.count > this.period) {
      // We had removed an old value and added oldValue
      // Now we need to undo that and redo with new value
      // Simplest: recompute from buffer
      this.buffer[prevPos] = value
      this.recomputeFromBuffer()
    } else {
      this.buffer[prevPos] = value
      this.recomputeFromBuffer()
    }

    const sma = this.sum / Math.min(this.count, this.period)
    const variance = this.count >= this.period ? this.m2 / this.period : 0
    const std = Math.sqrt(Math.max(0, variance))
    this.upperOut[idx] = this.count >= this.period ? sma + this.stdDevs * std : NaN
    this.lowerOut[idx] = this.count >= this.period ? sma - this.stdDevs * std : NaN
  }

  getUpper(): Float64Array { return this.upperOut }
  getLower(): Float64Array { return this.lowerOut }
  getLength(): number { return this.outputLen }

  private pushInternal(value: number, outputIdx: number): void {
    if (this.count >= this.period) {
      const oldValue = this.buffer[this.pos]
      // Welford's remove-old, add-new
      const oldMean = this.mean
      this.sum -= oldValue
      this.sum += value
      this.mean = this.sum / this.period
      // Update M2: remove contribution of old, add contribution of new
      this.m2 += (value - oldMean) * (value - this.mean) - (oldValue - oldMean) * (oldValue - this.mean)
      if (this.m2 < 0) this.m2 = 0  // clamp numerical drift
    } else {
      this.sum += value
      const n = this.count + 1
      const oldMean = this.mean
      this.mean = this.sum / n
      this.m2 += (value - oldMean) * (value - this.mean)
    }

    this.buffer[this.pos] = value
    this.pos = (this.pos + 1) % this.period
    this.count++

    const n = Math.min(this.count, this.period)
    const sma = this.sum / n
    const variance = n >= this.period ? this.m2 / this.period : 0
    const std = Math.sqrt(Math.max(0, variance))
    this.upperOut[outputIdx] = this.count >= this.period ? sma + this.stdDevs * std : NaN
    this.lowerOut[outputIdx] = this.count >= this.period ? sma - this.stdDevs * std : NaN
  }

  private recomputeFromBuffer(): void {
    // Recompute mean, sum, m2 from current buffer contents
    const n = Math.min(this.count, this.period)
    this.sum = 0
    for (let i = 0; i < n; i++) {
      this.sum += this.buffer[i]
    }
    this.mean = this.sum / n
    this.m2 = 0
    for (let i = 0; i < n; i++) {
      const d = this.buffer[i] - this.mean
      this.m2 += d * d
    }
  }

  private ensureCapacity(needed: number): void {
    if (needed <= this.upperOut.length) return
    const newCap = Math.max(needed, this.upperOut.length * 2)
    const newUpper = new Float64Array(newCap)
    const newLower = new Float64Array(newCap)
    newUpper.set(this.upperOut.subarray(0, this.outputLen))
    newLower.set(this.lowerOut.subarray(0, this.outputLen))
    this.upperOut = newUpper
    this.lowerOut = newLower
  }
}
```

- [ ] **Step 5: Create `src/indicators/IndicatorEngine.ts`**

```typescript
import { IncrementalSMA } from './incremental/sma'
import { IncrementalEMA } from './incremental/ema'
import { IncrementalBollinger } from './incremental/bollinger'
import type { IndicatorSnapshot } from './types'
import type { ColumnStore } from '../data/columns'

interface SymbolState {
  sma20: IncrementalSMA
  ema50: IncrementalEMA
  bollinger: IncrementalBollinger
}

const INITIAL_CAPACITY = 2048

export class IndicatorEngine {
  private state = new Map<string, SymbolState>()
  private subscribers = new Map<string, Set<(snapshot: IndicatorSnapshot) => void>>()

  private key(symbol: string, tf: string): string { return `${symbol}:${tf}` }

  bootstrap(symbol: string, timeframe: string, data: ColumnStore): IndicatorSnapshot {
    const k = this.key(symbol, timeframe)
    const cap = Math.max(data.length + 512, INITIAL_CAPACITY)
    const state: SymbolState = {
      sma20: new IncrementalSMA(20, cap),
      ema50: new IncrementalEMA(50, cap),
      bollinger: new IncrementalBollinger(20, 2, cap),
    }
    state.sma20.bootstrap(data.closes, data.length)
    state.ema50.bootstrap(data.closes, data.length)
    state.bollinger.bootstrap(data.closes, data.length)
    this.state.set(k, state)
    return this.buildSnapshot(state)
  }

  onTick(symbol: string, timeframe: string, price: number, action: 'updated' | 'created'): IndicatorSnapshot {
    const k = this.key(symbol, timeframe)
    const state = this.state.get(k)
    if (!state) throw new Error(`No indicator state for ${k} — call bootstrap() first`)

    if (action === 'created') {
      state.sma20.push(price)
      state.ema50.push(price)
      state.bollinger.push(price)
    } else {
      state.sma20.updateLast(price)
      state.ema50.updateLast(price)
      state.bollinger.updateLast(price)
    }

    const snapshot = this.buildSnapshot(state)
    this.subscribers.get(k)?.forEach(cb => cb(snapshot))
    return snapshot
  }

  getSnapshot(symbol: string, timeframe: string): IndicatorSnapshot | null {
    const state = this.state.get(this.key(symbol, timeframe))
    return state ? this.buildSnapshot(state) : null
  }

  subscribe(symbol: string, timeframe: string, cb: (snapshot: IndicatorSnapshot) => void): () => void {
    const k = this.key(symbol, timeframe)
    if (!this.subscribers.has(k)) this.subscribers.set(k, new Set())
    this.subscribers.get(k)!.add(cb)
    return () => { this.subscribers.get(k)?.delete(cb) }
  }

  remove(symbol: string, timeframe: string): void {
    const k = this.key(symbol, timeframe)
    this.state.delete(k)
    this.subscribers.delete(k)
  }

  private buildSnapshot(s: SymbolState): IndicatorSnapshot {
    return {
      sma20: s.sma20.getOutput(),
      ema50: s.ema50.getOutput(),
      bbUpper: s.bollinger.getUpper(),
      bbLower: s.bollinger.getLower(),
    }
  }
}
```

- [ ] **Step 6: Create `src/indicators/index.ts`**

```typescript
export { IndicatorEngine } from './IndicatorEngine'
export type { IndicatorSnapshot } from './types'
```

- [ ] **Step 7: Commit**

```bash
git add src/indicators/
git commit -m "feat: add incremental indicator engine with Welford's Bollinger"
```

---

## Task 3: ColumnStore Hardening

**Files:**
- Modify: `src/data/columns.ts`

- [ ] **Step 1: Add auto-grow, max capacity eviction, division-by-zero guard, and return type change**

Modify `src/data/columns.ts`:

1. `applyTick()` returns `'updated' | 'created'` instead of boolean
2. When at capacity, auto-grow arrays (double capacity, copy)
3. Max capacity of 50,000 — when exceeded, evict oldest 25%
4. `priceRange()` guards against min === max

```typescript
// Key changes to applyTick:
applyTick(price: number, volume: number, time: number, intervalSecs: number): 'updated' | 'created' {
  // ... same logic but:
  // - When at capacity: call this.grow() instead of returning false
  // - Return 'updated' or 'created' instead of boolean
}

// New methods:
private grow(): void {
  const MAX_CAPACITY = 50_000
  if (this.length >= MAX_CAPACITY) {
    this.evict()
    return
  }
  const newCap = Math.min(this.times.length * 2, MAX_CAPACITY)
  // ... allocate new arrays, copy, reassign
}

private evict(): void {
  const keep = Math.floor(this.length * 0.75)
  const start = this.length - keep
  // ... shift arrays left by start positions
}

// Fix priceRange:
priceRange(start: number, end: number): { min: number; max: number } {
  // ... existing logic ...
  if (min === max) { min -= 0.005; max += 0.005 }
  return { min, max }
}
```

- [ ] **Step 2: Commit**

```bash
git add src/data/columns.ts
git commit -m "feat: harden ColumnStore with auto-grow, eviction, div-by-zero guard"
```

---

## Task 4: Data Layer — Feed, SimulatedFeed, DataStore

**Files:**
- Create: `src/data/Feed.ts`
- Create: `src/data/SimulatedFeed.ts`
- Create: `src/data/DataStore.ts`
- Create: `src/data/index.ts`

- [ ] **Step 1: Create `src/data/Feed.ts`**

```typescript
import type { TickData } from './types'

export interface Feed {
  connect(): Promise<void>
  disconnect(): void
  subscribe(symbol: string, timeframe: string): void
  unsubscribe(symbol: string, timeframe: string): void
  onTick(cb: (symbol: string, timeframe: string, tick: TickData) => void): () => void
  onDisconnect(cb: () => void): () => void
}
```

- [ ] **Step 2: Create `src/data/SimulatedFeed.ts`**

Extract tick simulation from `src/chart/useChartData.ts` lines 72-99 into a class implementing Feed interface.

```typescript
import type { Feed } from './Feed'
import type { TickData } from './types'
import { TF_TO_INTERVAL } from './timeframes'

export class SimulatedFeed implements Feed {
  private intervalId: number | null = null
  private subscriptions = new Map<string, { symbol: string; timeframe: string; simTime: number; tickCount: number }>()
  private tickCb: ((symbol: string, tf: string, tick: TickData) => void) | null = null
  private disconnectCb: (() => void) | null = null
  private lastPrices = new Map<string, number>()

  async connect(): Promise<void> {
    this.intervalId = window.setInterval(() => this.tick(), 250)
  }

  disconnect(): void {
    if (this.intervalId !== null) { clearInterval(this.intervalId); this.intervalId = null }
    this.disconnectCb?.()
  }

  subscribe(symbol: string, timeframe: string): void {
    const key = `${symbol}:${timeframe}`
    if (!this.subscriptions.has(key)) {
      this.subscriptions.set(key, { symbol, timeframe, simTime: Date.now() / 1000, tickCount: 0 })
    }
  }

  unsubscribe(symbol: string, timeframe: string): void {
    this.subscriptions.delete(`${symbol}:${timeframe}`)
    this.lastPrices.delete(`${symbol}:${timeframe}`)
  }

  onTick(cb: (symbol: string, tf: string, tick: TickData) => void): () => void {
    this.tickCb = cb
    return () => { this.tickCb = null }
  }

  onDisconnect(cb: () => void): () => void {
    this.disconnectCb = cb
    return () => { this.disconnectCb = null }
  }

  /** Called by DataStore after loading history so simulation continues from last price */
  setLastPrice(symbol: string, timeframe: string, price: number, time: number): void {
    const key = `${symbol}:${timeframe}`
    this.lastPrices.set(key, price)
    const sub = this.subscriptions.get(key)
    if (sub) sub.simTime = time
  }

  private tick(): void {
    for (const [key, sub] of this.subscriptions) {
      const lastPrice = this.lastPrices.get(key) ?? 100
      const tf = TF_TO_INTERVAL[sub.timeframe as keyof typeof TF_TO_INTERVAL]
      if (!tf) continue

      const change = lastPrice * (Math.random() - 0.495) * 0.003
      const price = Math.max(0.01, lastPrice + change)
      const volume = Math.random() * 500

      sub.tickCount++
      if (sub.tickCount % 20 === 0) {
        sub.simTime += tf.seconds
      } else {
        sub.simTime += tf.seconds / 20
      }

      this.lastPrices.set(key, price)
      this.tickCb?.(sub.symbol, sub.timeframe, { price, volume, time: sub.simTime })
    }
  }
}
```

- [ ] **Step 3: Create `src/data/DataStore.ts`**

```typescript
import { invoke } from '@tauri-apps/api/core'
import { ColumnStore } from './columns'
import { TF_TO_INTERVAL } from './timeframes'
import type { TickData } from './types'
import type { IndicatorEngine } from '../indicators'
import type { IndicatorSnapshot } from '../indicators'
import type { Bar, Timeframe } from '../types'

export class DataStore {
  private stores = new Map<string, ColumnStore>()
  private snapshots = new Map<string, IndicatorSnapshot>()
  private subscribers = new Map<string, Set<() => void>>()
  private loading = new Set<string>()

  constructor(private indicatorEngine: IndicatorEngine) {}

  private key(symbol: string, tf: string): string { return `${symbol}:${tf}` }

  async load(symbol: string, timeframe: string): Promise<{ data: ColumnStore; indicators: IndicatorSnapshot }> {
    const k = this.key(symbol, timeframe)
    if (this.stores.has(k)) {
      return { data: this.stores.get(k)!, indicators: this.snapshots.get(k)! }
    }
    if (this.loading.has(k)) {
      // Already loading, wait for it
      return new Promise(resolve => {
        const unsub = this.subscribe(symbol, timeframe, () => {
          unsub()
          resolve({ data: this.stores.get(k)!, indicators: this.snapshots.get(k)! })
        })
      })
    }
    this.loading.add(k)

    const tf = TF_TO_INTERVAL[timeframe as Timeframe] ?? TF_TO_INTERVAL['5m']
    try {
      const bars: Bar[] = await invoke('get_bars', { symbol, interval: tf.interval, period: tf.period })
      const store = ColumnStore.fromBars(bars)
      this.stores.set(k, store)
      const indicators = this.indicatorEngine.bootstrap(symbol, timeframe, store)
      this.snapshots.set(k, indicators)
      this.loading.delete(k)
      this.notify(k)
      return { data: store, indicators }
    } catch (err) {
      this.loading.delete(k)
      throw err
    }
  }

  applyTick(symbol: string, timeframe: string, tick: TickData): void {
    const k = this.key(symbol, timeframe)
    const store = this.stores.get(k)
    if (!store) return // data not loaded yet, skip

    const tf = TF_TO_INTERVAL[timeframe as Timeframe]
    if (!tf) return

    const action = store.applyTick(tick.price, tick.volume, tick.time, tf.seconds)
    const snapshot = this.indicatorEngine.onTick(symbol, timeframe, tick.price, action)
    this.snapshots.set(k, snapshot)
    this.notify(k)
  }

  getData(symbol: string, timeframe: string): ColumnStore | null {
    return this.stores.get(this.key(symbol, timeframe)) ?? null
  }

  getIndicators(symbol: string, timeframe: string): IndicatorSnapshot | null {
    return this.snapshots.get(this.key(symbol, timeframe)) ?? null
  }

  subscribe(symbol: string, timeframe: string, cb: () => void): () => void {
    const k = this.key(symbol, timeframe)
    if (!this.subscribers.has(k)) this.subscribers.set(k, new Set())
    this.subscribers.get(k)!.add(cb)
    return () => { this.subscribers.get(k)?.delete(cb) }
  }

  unload(symbol: string, timeframe: string): void {
    const k = this.key(symbol, timeframe)
    this.stores.delete(k)
    this.snapshots.delete(k)
    this.subscribers.delete(k)
    this.indicatorEngine.remove(symbol, timeframe)
  }

  private notify(k: string): void {
    this.subscribers.get(k)?.forEach(cb => cb())
  }
}
```

- [ ] **Step 4: Create `src/data/index.ts`**

```typescript
export { DataStore } from './DataStore'
export { SimulatedFeed } from './SimulatedFeed'
export { TF_TO_INTERVAL } from './timeframes'
export type { Feed } from './Feed'
export type { TickData } from './types'
```

- [ ] **Step 5: Commit**

```bash
git add src/data/Feed.ts src/data/SimulatedFeed.ts src/data/DataStore.ts src/data/index.ts src/data/types.ts src/data/timeframes.ts
git commit -m "feat: add DataStore, Feed interface, SimulatedFeed"
```

---

## Task 5: Renderer Import Path Updates

**Files:**
- Modify: `src/renderer/CandleRenderer.ts`
- Modify: `src/renderer/GridRenderer.ts`
- Modify: `src/renderer/LineRenderer.ts`
- Create: `src/renderer/index.ts`

- [ ] **Step 1: Update imports in all three renderers**

In each renderer, change:
- `import { GPUContext } from './gpu'` → `import type { GPUContext } from '../engine/types'`
- `import { CoordSystem } from '../chart/CoordSystem'` → `import { CoordSystem } from '../engine/types'`

- [ ] **Step 2: Create `src/renderer/index.ts`**

```typescript
export { CandleRenderer } from './CandleRenderer'
export { GridRenderer } from './GridRenderer'
export { LineRenderer } from './LineRenderer'
```

- [ ] **Step 3: Verify build compiles**

Run: `npm run build`

- [ ] **Step 4: Commit**

```bash
git add src/renderer/
git commit -m "refactor: update renderer imports to engine/types"
```

---

## Task 6: RenderEngine, FrameScheduler, PaneContext

**Files:**
- Create: `src/engine/RenderEngine.ts`
- Create: `src/engine/FrameScheduler.ts`
- Create: `src/engine/PaneContext.ts`
- Create: `src/engine/index.ts`

- [ ] **Step 1: Create `src/engine/PaneContext.ts`**

```typescript
import { CandleRenderer, GridRenderer, LineRenderer } from '../renderer'
import type { GPUContext, EngineState } from './types'
import { CoordSystem } from './types'
import type { ColumnStore } from '../data/columns'
import type { IndicatorSnapshot } from '../indicators'

// Line config for each indicator
const LINE_CONFIGS = [
  { key: 'sma20' as const, color: [0.3, 0.6, 1.0, 0.8] as [number,number,number,number], width: 2.0 },
  { key: 'ema50' as const, color: [1.0, 0.6, 0.2, 0.8] as [number,number,number,number], width: 2.0 },
  { key: 'bbUpper' as const, color: [0.5, 0.5, 0.5, 0.4] as [number,number,number,number], width: 1.0 },
  { key: 'bbLower' as const, color: [0.5, 0.5, 0.5, 0.4] as [number,number,number,number], width: 1.0 },
]

export class PaneContext {
  dirty = true
  data: ColumnStore | null = null
  indicators: IndicatorSnapshot | null = null
  viewport: { viewStart: number; viewCount: number; cs: CoordSystem } | null = null

  private device: GPUDevice
  private format: GPUTextureFormat
  gpuContext: GPUCanvasContext
  private renderers: { candle: CandleRenderer; grid: GridRenderer; lines: LineRenderer[] }
  private resizeTimer: number | null = null
  private markDirtyFn: () => void

  constructor(
    readonly id: string,
    readonly canvas: HTMLCanvasElement,
    ctx: GPUContext,
    markDirty: () => void,
  ) {
    this.device = ctx.device
    this.format = ctx.format
    this.markDirtyFn = markDirty

    this.gpuContext = canvas.getContext('webgpu') as GPUCanvasContext
    this.gpuContext.configure({ device: ctx.device, format: ctx.format, alphaMode: 'premultiplied' })

    this.renderers = {
      candle: new CandleRenderer(ctx),
      grid: new GridRenderer(ctx),
      lines: LINE_CONFIGS.map(() => new LineRenderer(ctx)),
    }
  }

  setViewport(v: { viewStart: number; viewCount: number; cs: CoordSystem }): void {
    this.viewport = v
    this.dirty = true
    this.markDirtyFn()
  }

  setData(d: ColumnStore, indicators: IndicatorSnapshot): void {
    this.data = d
    this.indicators = indicators
    this.dirty = true
    this.markDirtyFn()
  }

  resize(width: number, height: number): void {
    if (this.resizeTimer) clearTimeout(this.resizeTimer)
    this.resizeTimer = window.setTimeout(() => {
      const dpr = window.devicePixelRatio || 1
      this.canvas.width = Math.round(width * dpr)
      this.canvas.height = Math.round(height * dpr)
      this.canvas.style.width = width + 'px'
      this.canvas.style.height = height + 'px'
      this.gpuContext.configure({ device: this.device, format: this.format, alphaMode: 'premultiplied' })
      this.dirty = true
      this.markDirtyFn()
    }, 16)
  }

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

    if (this.viewport?.cs && this.data) {
      const { cs } = this.viewport
      const { viewStart, viewCount } = this.viewport

      this.renderers.grid.upload(cs)
      this.renderers.candle.upload(this.data, cs, viewStart, viewCount)

      if (this.indicators) {
        LINE_CONFIGS.forEach((cfg, i) => {
          const values = this.indicators![cfg.key]
          if (values) {
            this.renderers.lines[i].upload(values, cs, viewStart, viewCount, cfg.color, cfg.width)
          }
        })
      }

      this.renderers.grid.render(pass)
      this.renderers.candle.render(pass)
      for (const line of this.renderers.lines) line.render(pass)
    }

    pass.end()
    return encoder.finish()
  }

  reconfigure(ctx: GPUContext): void {
    if (!this.canvas.isConnected) return
    this.device = ctx.device
    this.format = ctx.format
    this.gpuContext.configure({ device: ctx.device, format: ctx.format, alphaMode: 'premultiplied' })
    this.renderers.candle.destroy()
    this.renderers.grid.destroy()
    for (const l of this.renderers.lines) l.destroy()
    this.renderers = {
      candle: new CandleRenderer(ctx),
      grid: new GridRenderer(ctx),
      lines: LINE_CONFIGS.map(() => new LineRenderer(ctx)),
    }
    this.dirty = true
  }

  destroy(): void {
    if (this.resizeTimer) clearTimeout(this.resizeTimer)
    this.renderers.candle.destroy()
    this.renderers.grid.destroy()
    for (const l of this.renderers.lines) l.destroy()
  }
}
```

- [ ] **Step 2: Create `src/engine/FrameScheduler.ts`**

```typescript
import type { PaneContext } from './PaneContext'

export class FrameScheduler {
  private panes = new Map<string, PaneContext>()
  private rafId: number | null = null
  private running = false
  private paused = false
  private device: GPUDevice
  activePaneId: string | null = null

  constructor(device: GPUDevice) {
    this.device = device
  }

  addPane(pane: PaneContext): void { this.panes.set(pane.id, pane) }
  removePane(id: string): void { this.panes.delete(id) }

  start(): void {
    this.running = true
    if (!this.rafId) this.rafId = requestAnimationFrame(() => this.tick())
  }

  stop(): void {
    this.running = false
    if (this.rafId) { cancelAnimationFrame(this.rafId); this.rafId = null }
  }

  pause(): void { this.paused = true }
  resume(): void {
    this.paused = false
    // Kick the loop if stopped
    if (this.running && !this.rafId) {
      this.rafId = requestAnimationFrame(() => this.tick())
    }
  }

  markDirty(paneId: string): void {
    const pane = this.panes.get(paneId)
    if (pane) pane.dirty = true
    if (!this.rafId && this.running && !this.paused) {
      this.rafId = requestAnimationFrame(() => this.tick())
    }
  }

  updateDevice(device: GPUDevice): void {
    this.device = device
  }

  private tick(): void {
    this.rafId = null

    if (this.paused) {
      // Keep loop alive during recovery to detect resume
      this.rafId = requestAnimationFrame(() => this.tick())
      return
    }

    const commandBuffers: GPUCommandBuffer[] = []

    // Active pane first
    if (this.activePaneId) {
      const active = this.panes.get(this.activePaneId)
      if (active?.dirty) this.renderPane(this.activePaneId, commandBuffers)
    }

    for (const [id, pane] of this.panes) {
      if (id === this.activePaneId) continue
      if (!pane.dirty) continue
      this.renderPane(id, commandBuffers)
    }

    if (commandBuffers.length > 0) {
      try {
        this.device.queue.submit(commandBuffers)
      } catch (e) {
        console.error('GPU submit failed:', e)
      }
    }

    // Schedule next frame only if there's still dirty work
    let hasDirty = false
    for (const pane of this.panes.values()) {
      if (pane.dirty) { hasDirty = true; break }
    }
    if (hasDirty && this.running) {
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
      pane.dirty = false
    }
  }
}
```

- [ ] **Step 3: Create `src/engine/RenderEngine.ts`**

```typescript
import { FrameScheduler } from './FrameScheduler'
import { PaneContext } from './PaneContext'
import type { GPUContext, EngineState } from './types'

export class RenderEngine {
  private ctx: GPUContext
  private panes = new Map<string, PaneContext>()
  readonly scheduler: FrameScheduler
  private _state: EngineState = 'ready'
  private recoveryAttempts = 0
  private stateListeners = new Set<(state: EngineState) => void>()
  private deviceReplacedListeners = new Set<(device: GPUDevice) => void>()

  private constructor(ctx: GPUContext) {
    this.ctx = ctx
    this.scheduler = new FrameScheduler(ctx.device)
    ctx.device.lost.then(info => this.onDeviceLost(info))
  }

  static async create(): Promise<RenderEngine> {
    const ctx = await RenderEngine.initGPU()
    return new RenderEngine(ctx)
  }

  private static async initGPU(): Promise<GPUContext> {
    if (!navigator.gpu) throw new Error('WebGPU not supported')
    const adapter = await navigator.gpu.requestAdapter({ powerPreference: 'high-performance' })
    if (!adapter) throw new Error('No GPU adapter found')
    const device = await adapter.requestDevice()
    return { device, format: navigator.gpu.getPreferredCanvasFormat() }
  }

  get state(): EngineState { return this._state }
  get gpuDevice(): GPUDevice { return this.ctx.device }

  registerPane(id: string, canvas: HTMLCanvasElement): PaneContext {
    const pane = new PaneContext(id, canvas, this.ctx, () => this.scheduler.markDirty(id))
    this.panes.set(id, pane)
    this.scheduler.addPane(pane)
    return pane
  }

  unregisterPane(id: string): void {
    const pane = this.panes.get(id)
    if (pane) {
      pane.destroy()
      this.panes.delete(id)
      this.scheduler.removePane(id)
    }
  }

  retry(): void {
    if (this._state === 'failed') {
      this.recoveryAttempts = 0
      this.recover()
    }
  }

  onStateChange(cb: (state: EngineState) => void): () => void {
    this.stateListeners.add(cb)
    return () => { this.stateListeners.delete(cb) }
  }

  onDeviceReplaced(cb: (device: GPUDevice) => void): () => void {
    this.deviceReplacedListeners.add(cb)
    return () => { this.deviceReplacedListeners.delete(cb) }
  }

  /** Expose panes for tab-visibility coalescing */
  getAllPanes(): IterableIterator<PaneContext> { return this.panes.values() }

  destroy(): void {
    this.scheduler.stop()
    for (const pane of this.panes.values()) pane.destroy()
    this.panes.clear()
  }

  private onDeviceLost(info: GPUDeviceLostInfo): void {
    console.error('GPU device lost:', info.message)
    this.setState('recovering')
    this.scheduler.pause()
    this.recover()
  }

  private async recover(): Promise<void> {
    const MAX_ATTEMPTS = 3
    while (this.recoveryAttempts < MAX_ATTEMPTS) {
      this.recoveryAttempts++
      const delay = 1000 * Math.pow(2, this.recoveryAttempts - 1) // 1s, 2s, 4s
      await new Promise(r => setTimeout(r, delay))

      try {
        this.ctx = await RenderEngine.initGPU()
        this.ctx.device.lost.then(info => this.onDeviceLost(info))
        this.scheduler.updateDevice(this.ctx.device)

        // Reconfigure all panes
        const deadPanes: string[] = []
        for (const [id, pane] of this.panes) {
          if (!pane.canvas.isConnected) {
            deadPanes.push(id)
            continue
          }
          pane.reconfigure(this.ctx)
        }
        for (const id of deadPanes) this.unregisterPane(id)

        // Notify device replaced listeners
        for (const cb of this.deviceReplacedListeners) cb(this.ctx.device)

        this.recoveryAttempts = 0
        this.scheduler.resume()
        this.setState('ready')
        return
      } catch (e) {
        console.error(`GPU recovery attempt ${this.recoveryAttempts} failed:`, e)
      }
    }

    this.setState('failed')
  }

  private setState(state: EngineState): void {
    this._state = state
    for (const cb of this.stateListeners) cb(state)
  }
}
```

- [ ] **Step 4: Create `src/engine/index.ts`**

```typescript
export { RenderEngine } from './RenderEngine'
export { FrameScheduler } from './FrameScheduler'
export { PaneContext } from './PaneContext'
export { CoordSystem } from './types'
export type { GPUContext, EngineState, CoordConfig } from './types'
```

- [ ] **Step 5: Verify build compiles**

Run: `npm run build`

- [ ] **Step 6: Commit**

```bash
git add src/engine/
git commit -m "feat: add RenderEngine, FrameScheduler, PaneContext"
```

---

## Task 7: Singleton Accessors

**Files:**
- Create: `src/globals.ts`

- [ ] **Step 1: Create `src/globals.ts`**

Module-level singletons for RenderEngine, DataStore, IndicatorEngine.

```typescript
import type { RenderEngine } from './engine'
import type { DataStore } from './data'
import type { IndicatorEngine } from './indicators'
import type { SimulatedFeed } from './data/SimulatedFeed'

let _engine: RenderEngine | null = null
let _dataStore: DataStore | null = null
let _indicatorEngine: IndicatorEngine | null = null
let _feed: SimulatedFeed | null = null

export function getRenderEngine(): RenderEngine {
  if (!_engine) throw new Error('RenderEngine not initialized')
  return _engine
}
export function setRenderEngine(e: RenderEngine) { _engine = e }

export function getDataStore(): DataStore {
  if (!_dataStore) throw new Error('DataStore not initialized')
  return _dataStore
}
export function setDataStore(d: DataStore) { _dataStore = d }

export function getIndicatorEngine(): IndicatorEngine {
  if (!_indicatorEngine) throw new Error('IndicatorEngine not initialized')
  return _indicatorEngine
}
export function setIndicatorEngine(i: IndicatorEngine) { _indicatorEngine = i }

export function getFeed(): SimulatedFeed {
  if (!_feed) throw new Error('Feed not initialized')
  return _feed
}
export function setFeed(f: SimulatedFeed) { _feed = f }
```

- [ ] **Step 2: Commit**

```bash
git add src/globals.ts
git commit -m "feat: add singleton accessor functions for global instances"
```

---

## Task 8: useChartViewport Hook

**Files:**
- Create: `src/chart/useChartViewport.ts`

- [ ] **Step 1: Create `src/chart/useChartViewport.ts`**

Extract viewport logic from `src/chart/useChartData.ts`. This hook owns viewStart, viewCount, priceOverride, auto-scroll, CoordSystem computation, and all pan/zoom callbacks.

Key differences from current useChartData:
- No data fetching (DataStore handles that)
- No tick simulation (SimulatedFeed handles that)
- No indicator computation (IndicatorEngine handles that)
- Subscribes to DataStore for data changes to recompute CoordSystem
- Calls scheduler.markDirty on viewport changes

```typescript
import { useState, useEffect, useCallback, useRef, useMemo } from 'react'
import { CoordSystem } from '../engine'
import { useChartStore } from '../store/chartStore'
import { getDataStore, getRenderEngine } from '../globals'
import type { Timeframe } from '../types'

const RIGHT_MARGIN_BARS = 8
const AUTO_SCROLL_TIMEOUT = 10_000

export interface Viewport {
  viewStart: number
  viewCount: number
  cs: CoordSystem | null
}

export function useChartViewport(symbol: string, timeframe: Timeframe, width: number, height: number, paneId: string) {
  const [viewStart, setViewStart] = useState(0)
  const [viewCount, setViewCount] = useState(200)
  const [priceOverride, setPriceOverride] = useState<{ min: number; max: number } | null>(null)
  const [cs, setCs] = useState<CoordSystem | null>(null)
  const [autoScrolling, setAutoScrolling] = useState(true)
  const [dataVersion, setDataVersion] = useState(0)
  const idleTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const autoScrollVersion = useChartStore(s => s.autoScrollVersion)

  // Global reset → force auto-scroll
  useEffect(() => {
    setAutoScrolling(true)
    setPriceOverride(null)
  }, [autoScrollVersion])

  // Pause auto-scroll on interaction
  const pauseAutoScroll = useCallback(() => {
    setAutoScrolling(false)
    if (idleTimerRef.current) clearTimeout(idleTimerRef.current)
    idleTimerRef.current = setTimeout(() => {
      setAutoScrolling(true)
      setPriceOverride(null)
    }, AUTO_SCROLL_TIMEOUT)
  }, [])

  useEffect(() => () => {
    if (idleTimerRef.current) clearTimeout(idleTimerRef.current)
  }, [])

  // Subscribe to data changes to trigger CS recompute
  useEffect(() => {
    const ds = getDataStore()
    return ds.subscribe(symbol, timeframe, () => setDataVersion(v => v + 1))
  }, [symbol, timeframe])

  // Auto-scroll
  useEffect(() => {
    if (!autoScrolling) return
    const data = getDataStore().getData(symbol, timeframe)
    if (!data) return
    const maxStart = Math.max(0, data.length - viewCount + RIGHT_MARGIN_BARS)
    setViewStart(maxStart)
  }, [autoScrolling, dataVersion, viewCount, symbol, timeframe])

  // Recompute CoordSystem
  useEffect(() => {
    const data = getDataStore().getData(symbol, timeframe)
    if (!data || width === 0 || height === 0) return

    const end = Math.min(viewStart + viewCount, data.length)
    const dataBars = end - viewStart
    if (dataBars <= 0) return
    const totalBars = dataBars + RIGHT_MARGIN_BARS

    let minP: number, maxP: number
    if (priceOverride) {
      minP = priceOverride.min
      maxP = priceOverride.max
    } else {
      const range = data.priceRange(viewStart, end)
      const pad = (range.max - range.min) * 0.05
      minP = range.min - pad
      maxP = range.max + pad
    }

    setCs(new CoordSystem({ width, height, barCount: totalBars, minPrice: minP, maxPrice: maxP }))
  }, [viewStart, viewCount, width, height, priceOverride, dataVersion, symbol, timeframe])

  // Reset on symbol/timeframe change
  useEffect(() => {
    const data = getDataStore().getData(symbol, timeframe)
    if (data) {
      setViewStart(Math.max(0, data.length - 200))
      setViewCount(Math.min(200, data.length))
    }
    setPriceOverride(null)
    setAutoScrolling(true)
  }, [symbol, timeframe])

  const pan = useCallback((deltaPixels: number) => {
    const data = getDataStore().getData(symbol, timeframe)
    if (!data || !cs) return
    const barDelta = Math.round(deltaPixels / cs.barStep)
    if (barDelta === 0) return
    pauseAutoScroll()
    setViewStart(v => Math.max(0, Math.min(data.length - viewCount + RIGHT_MARGIN_BARS, v - barDelta)))
  }, [cs, viewCount, pauseAutoScroll, symbol, timeframe])

  const zoomX = useCallback((factor: number) => {
    const data = getDataStore().getData(symbol, timeframe)
    if (!data) return
    pauseAutoScroll()
    setViewCount(v => {
      const newCount = Math.max(20, Math.min(data.length, Math.round(v * factor)))
      setViewStart(s => Math.max(0, Math.min(data.length - newCount + RIGHT_MARGIN_BARS, s + Math.round((v - newCount) / 2))))
      return newCount
    })
  }, [pauseAutoScroll, symbol, timeframe])

  const zoomY = useCallback((factor: number, anchorPrice?: number) => {
    if (!cs) return
    pauseAutoScroll()
    const center = anchorPrice ?? (cs.minPrice + cs.maxPrice) / 2
    const halfRange = ((cs.maxPrice - cs.minPrice) / 2) * factor
    setPriceOverride({ min: center - halfRange, max: center + halfRange })
  }, [cs, pauseAutoScroll])

  const panY = useCallback((deltaPixels: number) => {
    if (!cs) return
    pauseAutoScroll()
    const pricePerPixel = (cs.maxPrice - cs.minPrice) / cs.chartHeight
    const priceDelta = deltaPixels * pricePerPixel
    setPriceOverride({ min: cs.minPrice + priceDelta, max: cs.maxPrice + priceDelta })
  }, [cs, pauseAutoScroll])

  const resetYZoom = useCallback(() => setPriceOverride(null), [])

  const viewport: Viewport = useMemo(() => ({
    viewStart, viewCount, cs
  }), [viewStart, viewCount, cs])

  return { viewport, pan, zoomX, zoomY, panY, resetYZoom, autoScrolling, pauseAutoScroll }
}
```

- [ ] **Step 2: Commit**

```bash
git add src/chart/useChartViewport.ts
git commit -m "feat: add useChartViewport hook (extracted from useChartData)"
```

---

## Task 9: AxisCanvas Component

**Files:**
- Create: `src/chart/AxisCanvas.tsx`

- [ ] **Step 1: Create `src/chart/AxisCanvas.tsx`**

Extract axis rendering from ChartPane lines 114-144:

```typescript
import { useRef, useEffect } from 'react'
import type { CoordSystem } from '../engine'
import type { ColumnStore } from '../data/columns'

interface Props {
  cs: CoordSystem | null
  data: ColumnStore | null
  viewStart: number
  width: number
  height: number
}

export function AxisCanvas({ cs, data, viewStart, width, height }: Props) {
  const ref = useRef<HTMLCanvasElement>(null)

  useEffect(() => {
    if (!ref.current || !cs || !data) return
    const canvas = ref.current
    const dpr = window.devicePixelRatio || 1
    canvas.width = width * dpr
    canvas.height = height * dpr
    canvas.style.width = width + 'px'
    canvas.style.height = height + 'px'
    const ctx = canvas.getContext('2d')!
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0)
    ctx.clearRect(0, 0, width, height)
    ctx.fillStyle = '#444'
    ctx.font = '10px monospace'
    ctx.textAlign = 'left'

    const priceStep = (cs.maxPrice - cs.minPrice) / 8
    for (let i = 0; i <= 8; i++) {
      const price = cs.minPrice + i * priceStep
      ctx.fillText(price.toFixed(2), width - cs.pr + 4, cs.priceToY(price) + 4)
    }

    const barStep = Math.max(1, Math.floor(100 / cs.barStep))
    for (let i = 0; i < cs.barCount; i += barStep) {
      const dataIdx = viewStart + i
      if (dataIdx < data.length) {
        const d = new Date(data.times[dataIdx] * 1000)
        const label = `${d.getHours().toString().padStart(2, '0')}:${d.getMinutes().toString().padStart(2, '0')}`
        ctx.fillText(label, cs.barToX(i) - 16, height - cs.pb + 14)
      }
    }
  }, [cs, data, viewStart, width, height])

  return <canvas ref={ref} width={width} height={height}
    style={{ position: 'absolute', top: 0, left: 0, pointerEvents: 'none' }} />
}
```

- [ ] **Step 2: Commit**

```bash
git add src/chart/AxisCanvas.tsx
git commit -m "feat: extract AxisCanvas component from ChartPane"
```

---

## Task 10: Rewrite ChartPane

**Files:**
- Modify: `src/chart/ChartPane.tsx`

- [ ] **Step 1: Rewrite ChartPane as thin wrapper**

Replace the entire file. The new ChartPane:
- Registers/unregisters with RenderEngine
- Subscribes to DataStore for data → pushes to PaneContext
- Uses useChartViewport for all viewport logic
- Handles mouse/wheel events (same zone logic)
- Renders overlays (CrosshairOverlay, DrawingOverlay, AxisCanvas)
- Shows GPU state overlays

Key: NO GPU init, NO render effects, NO indicator computation.

The mouse handling code (getZone, onMouseDown, onMouseMove, onMouseUp, onMouseLeave, onWheel, onDoubleClick, onAuxClick, cursor logic) stays the same — it's purely input handling.

Full implementation in the actual code step — this is a complete rewrite of the file (~120 lines).

- [ ] **Step 2: Verify build compiles**

Run: `npm run build`

- [ ] **Step 3: Commit**

```bash
git add src/chart/ChartPane.tsx
git commit -m "refactor: rewrite ChartPane as thin React wrapper over RenderEngine"
```

---

## Task 11: Bootstrap & main.tsx

**Files:**
- Modify: `src/main.tsx`

- [ ] **Step 1: Rewrite `src/main.tsx` with bootstrap()**

```typescript
import React from 'react'
import ReactDOM from 'react-dom/client'
import App from './App'
import { RenderEngine } from './engine'
import { IndicatorEngine } from './indicators'
import { DataStore, SimulatedFeed } from './data'
import { setRenderEngine, setDataStore, setIndicatorEngine, setFeed } from './globals'
import { useChartStore } from './store/chartStore'

async function bootstrap() {
  const engine = await RenderEngine.create()
  const indicatorEngine = new IndicatorEngine()
  const dataStore = new DataStore(indicatorEngine)
  const feed = new SimulatedFeed()

  feed.onTick((symbol, tf, tick) => dataStore.applyTick(symbol, tf, tick))

  // Subscribe all default panes to the feed
  const panes = useChartStore.getState().panes
  for (const pane of panes) {
    feed.subscribe(pane.symbol, pane.timeframe)
  }

  await feed.connect()

  setRenderEngine(engine)
  setDataStore(dataStore)
  setIndicatorEngine(indicatorEngine)
  setFeed(feed)

  engine.scheduler.start()

  // Tab visibility handling
  document.addEventListener('visibilitychange', () => {
    if (document.hidden) {
      engine.scheduler.stop()
    } else {
      for (const pane of engine.getAllPanes()) pane.dirty = true
      engine.scheduler.start()
    }
  })

  ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
    <React.StrictMode>
      <App />
    </React.StrictMode>,
  )
}

bootstrap().catch(err => {
  document.body.innerHTML = `<div style="color:#e74c3c;padding:40px;font-family:monospace">
    GPU initialization failed: ${err.message}<br><br>
    <button onclick="location.reload()">Retry</button>
  </div>`
})
```

- [ ] **Step 2: Commit**

```bash
git add src/main.tsx
git commit -m "refactor: bootstrap GPU before React mount"
```

---

## Task 12: Cleanup Old Files & Final Integration

**Files:**
- Remove: `src/renderer/gpu.ts`
- Remove: `src/data/indicators.ts`
- Remove: `src/chart/useChartData.ts`
- Update: `src/chart/index.ts` (if exists)

- [ ] **Step 1: Delete old files**

```bash
rm src/renderer/gpu.ts
rm src/data/indicators.ts
rm src/chart/useChartData.ts
```

- [ ] **Step 2: Update any remaining imports**

Search for any file still importing from deleted modules and update them.

- [ ] **Step 3: Full build and type check**

Run: `npm run build`

Fix any type errors.

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "cleanup: remove old gpu.ts, indicators.ts, useChartData.ts"
```

---

## Task 13: Build & Launch

- [ ] **Step 1: Full clean build**

```bash
npm run build
```

- [ ] **Step 2: Launch Tauri app**

```bash
cargo tauri dev
```

- [ ] **Step 3: Verify all 7 charts render**

All panes should show candlestick charts with indicators.

- [ ] **Step 4: Verify interactions work**

- Pan (drag chart body)
- Zoom X (scroll on chart or drag X axis)
- Zoom Y (scroll on Y axis)
- Pan Y (drag Y axis)
- Double-click Y axis to reset
- Crosshair follows mouse
- Drawing tools (middle-click toggle, trendline, hline)

- [ ] **Step 5: Final commit**

```bash
git add -A
git commit -m "feat: complete render engine refactor — all charts rendering"
```
