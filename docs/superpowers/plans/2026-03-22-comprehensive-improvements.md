# Comprehensive Improvements — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add tests, volume bars, toggleable indicators, dynamic indicator registry, partial tick uploads, frame telemetry, historical pagination, feed reconnection, data persistence, and CoordSystem caching to the apex-terminal trading app.

**Architecture:** Dynamic indicator registry replaces hard-coded LINE_CONFIGS. ChartStore gains per-pane visibility toggles. VolumeRenderer added as new GPU renderer. PaneContext uses tick action for surgical buffer updates. IndexedDB caches historical bars. Feed interface gains reconnection protocol.

**Tech Stack:** TypeScript, Vitest, React, WebGPU, Zustand, Tauri IPC, IndexedDB (idb-keyval), Vite

**Spec:** `docs/superpowers/specs/2026-03-21-render-engine-refactor-design.md`

---

## File Map

### New Files
```
src/tests/indicators.test.ts       # Incremental indicator correctness tests
src/tests/columnStore.test.ts       # ColumnStore hardened operations tests
src/tests/dataStore.test.ts         # DataStore load/tick/subscribe tests
src/tests/frameScheduler.test.ts    # FrameScheduler dirty/tick logic tests
src/tests/indicatorRegistry.test.ts # Registry add/remove/toggle tests
src/indicators/registry.ts          # Dynamic indicator registry
src/indicators/incremental/rsi.ts   # RSI indicator (demonstrates extensibility)
src/indicators/incremental/vwap.ts  # VWAP indicator
src/renderer/VolumeRenderer.ts      # GPU volume bar renderer
src/renderer/shaders/volume.wgsl    # Volume bar shader
src/data/BarCache.ts                # IndexedDB bar persistence
src/chart/FrameStats.tsx            # FPS/frame-time debug overlay
```

### Modified Files
```
src/indicators/types.ts             # IndicatorSnapshot → dynamic, IndicatorConfig
src/indicators/IndicatorEngine.ts   # Use registry, dynamic indicators
src/indicators/index.ts             # Export registry
src/engine/PaneContext.ts           # VolumeRenderer, dynamic indicators, partial upload
src/engine/FrameScheduler.ts        # Frame timing ring buffer
src/engine/types.ts                 # FrameStats type
src/store/chartStore.ts             # Per-pane indicator/volume visibility toggles
src/data/DataStore.ts               # BarCache integration, pagination
src/data/Feed.ts                    # Reconnection protocol
src/data/SimulatedFeed.ts           # Implement reconnection
src/data/columns.ts                 # prepend() for historical pagination
src/chart/ChartPane.tsx             # Volume toggle, indicator toggles, frame stats
src/chart/useChartViewport.ts       # Scroll-left pagination trigger
src/chart/CoordSystem.ts            # Flyweight cache
src/toolbar/Toolbar.tsx             # Indicator/volume toggle buttons
src/renderer/index.ts               # Export VolumeRenderer
```

---

## Phase 1: Tests

### Task 1: Incremental Indicator Correctness Tests

**Files:**
- Create: `src/tests/indicators.test.ts`

These tests verify that incremental indicators produce identical output to naive O(n) implementations.

- [ ] **Step 1: Write SMA correctness test**

```typescript
// src/tests/indicators.test.ts
import { describe, it, expect } from 'vitest'
import { IncrementalSMA } from '../indicators/incremental/sma'

function naiveSMA(values: number[], period: number): number[] {
  const result: number[] = []
  for (let i = 0; i < values.length; i++) {
    if (i < period - 1) { result.push(NaN); continue }
    let sum = 0
    for (let j = i - period + 1; j <= i; j++) sum += values[j]
    result.push(sum / period)
  }
  return result
}

describe('IncrementalSMA', () => {
  it('matches naive SMA on bootstrap', () => {
    const prices = Array.from({ length: 200 }, () => 100 + Math.random() * 50)
    const closes = new Float64Array(prices)
    const sma = new IncrementalSMA(20, 300)
    sma.bootstrap(closes, closes.length)

    const naive = naiveSMA(prices, 20)
    const output = sma.getOutput()
    for (let i = 0; i < prices.length; i++) {
      if (isNaN(naive[i])) {
        expect(isNaN(output[i])).toBe(true)
      } else {
        expect(output[i]).toBeCloseTo(naive[i], 10)
      }
    }
  })

  it('matches naive SMA after incremental pushes', () => {
    const prices = Array.from({ length: 100 }, () => 100 + Math.random() * 50)
    const closes = new Float64Array(prices.slice(0, 50))
    const sma = new IncrementalSMA(20, 200)
    sma.bootstrap(closes, 50)

    // Push remaining 50 one by one
    for (let i = 50; i < 100; i++) sma.push(prices[i])

    const naive = naiveSMA(prices, 20)
    const output = sma.getOutput()
    for (let i = 0; i < 100; i++) {
      if (isNaN(naive[i])) {
        expect(isNaN(output[i])).toBe(true)
      } else {
        expect(output[i]).toBeCloseTo(naive[i], 10)
      }
    }
  })

  it('updateLast produces correct SMA', () => {
    const prices = [10, 20, 30, 40, 50]
    const closes = new Float64Array(prices)
    const sma = new IncrementalSMA(3, 20)
    sma.bootstrap(closes, 5)

    // Update last value from 50 to 60
    sma.updateLast(60)
    const output = sma.getOutput()
    // Last SMA should be (30+40+60)/3 = 43.333...
    expect(output[4]).toBeCloseTo((30 + 40 + 60) / 3, 10)
  })
})
```

- [ ] **Step 2: Run test to verify it passes**

Run: `npx vitest run src/tests/indicators.test.ts`

- [ ] **Step 3: Write EMA correctness test**

Add to same file:

```typescript
import { IncrementalEMA } from '../indicators/incremental/ema'

function naiveEMA(values: number[], period: number): number[] {
  const k = 2 / (period + 1)
  const result: number[] = [NaN]
  let prev = values[0]
  for (let i = 1; i < values.length; i++) {
    prev = values[i] * k + prev * (1 - k)
    result.push(i >= period - 1 ? prev : NaN)
  }
  return result
}

describe('IncrementalEMA', () => {
  it('matches naive EMA on bootstrap', () => {
    const prices = Array.from({ length: 200 }, () => 100 + Math.random() * 50)
    const closes = new Float64Array(prices)
    const ema = new IncrementalEMA(50, 300)
    ema.bootstrap(closes, closes.length)

    const naive = naiveEMA(prices, 50)
    const output = ema.getOutput()
    for (let i = 0; i < prices.length; i++) {
      if (isNaN(naive[i])) {
        expect(isNaN(output[i])).toBe(true)
      } else {
        expect(output[i]).toBeCloseTo(naive[i], 8)
      }
    }
  })

  it('matches naive EMA after incremental pushes', () => {
    const prices = Array.from({ length: 100 }, () => 100 + Math.random() * 50)
    const closes = new Float64Array(prices.slice(0, 50))
    const ema = new IncrementalEMA(20, 200)
    ema.bootstrap(closes, 50)
    for (let i = 50; i < 100; i++) ema.push(prices[i])

    const naive = naiveEMA(prices, 20)
    const output = ema.getOutput()
    for (let i = 0; i < 100; i++) {
      if (isNaN(naive[i])) {
        expect(isNaN(output[i])).toBe(true)
      } else {
        expect(output[i]).toBeCloseTo(naive[i], 8)
      }
    }
  })
})
```

- [ ] **Step 4: Write Bollinger correctness test**

```typescript
import { IncrementalBollinger } from '../indicators/incremental/bollinger'

function naiveBollinger(values: number[], period: number, stdDevs: number) {
  const upper: number[] = [], lower: number[] = []
  for (let i = 0; i < values.length; i++) {
    if (i < period - 1) { upper.push(NaN); lower.push(NaN); continue }
    let sum = 0
    for (let j = i - period + 1; j <= i; j++) sum += values[j]
    const mean = sum / period
    let sqSum = 0
    for (let j = i - period + 1; j <= i; j++) sqSum += (values[j] - mean) ** 2
    const std = Math.sqrt(sqSum / period)
    upper.push(mean + stdDevs * std)
    lower.push(mean - stdDevs * std)
  }
  return { upper, lower }
}

describe('IncrementalBollinger', () => {
  it('matches naive Bollinger on bootstrap', () => {
    const prices = Array.from({ length: 200 }, () => 100 + Math.random() * 50)
    const closes = new Float64Array(prices)
    const bb = new IncrementalBollinger(20, 2, 300)
    bb.bootstrap(closes, closes.length)

    const naive = naiveBollinger(prices, 20, 2)
    const upper = bb.getUpper(), lower = bb.getLower()
    for (let i = 0; i < prices.length; i++) {
      if (isNaN(naive.upper[i])) {
        expect(isNaN(upper[i])).toBe(true)
      } else {
        expect(upper[i]).toBeCloseTo(naive.upper[i], 6)
        expect(lower[i]).toBeCloseTo(naive.lower[i], 6)
      }
    }
  })

  it('matches after incremental pushes', () => {
    const prices = Array.from({ length: 100 }, () => 100 + Math.random() * 50)
    const closes = new Float64Array(prices.slice(0, 50))
    const bb = new IncrementalBollinger(20, 2, 200)
    bb.bootstrap(closes, 50)
    for (let i = 50; i < 100; i++) bb.push(prices[i])

    const naive = naiveBollinger(prices, 20, 2)
    const upper = bb.getUpper(), lower = bb.getLower()
    for (let i = 0; i < 100; i++) {
      if (isNaN(naive.upper[i])) continue
      expect(upper[i]).toBeCloseTo(naive.upper[i], 5)
      expect(lower[i]).toBeCloseTo(naive.lower[i], 5)
    }
  })

  it('updateLast is correct', () => {
    const prices = Array.from({ length: 30 }, (_, i) => 100 + i)
    const closes = new Float64Array(prices)
    const bb = new IncrementalBollinger(20, 2, 50)
    bb.bootstrap(closes, 30)

    // Update last to a different value and verify against naive
    const modifiedPrices = [...prices]
    modifiedPrices[29] = 150
    bb.updateLast(150)

    const naive = naiveBollinger(modifiedPrices, 20, 2)
    expect(bb.getUpper()[29]).toBeCloseTo(naive.upper[29], 5)
    expect(bb.getLower()[29]).toBeCloseTo(naive.lower[29], 5)
  })
})
```

- [ ] **Step 5: Run all indicator tests**

Run: `npx vitest run src/tests/indicators.test.ts`
Expected: All pass

- [ ] **Step 6: Commit**

```bash
git add src/tests/indicators.test.ts
git commit -m "test: add incremental indicator correctness tests with naive reference implementations"
```

---

### Task 2: ColumnStore Hardened Operations Tests

**Files:**
- Modify: `src/tests/columns.test.ts` (add new tests to existing file)

- [ ] **Step 1: Write applyTick tests**

Add to existing `src/tests/columns.test.ts`:

```typescript
describe('applyTick', () => {
  it('updates existing candle when within interval', () => {
    const bars = [{ time: 1000, open: 100, high: 110, low: 90, close: 105, volume: 500 }]
    const store = ColumnStore.fromBars(bars)
    const action = store.applyTick(115, 100, 1030, 60) // within 60s interval
    expect(action).toBe('updated')
    expect(store.length).toBe(1)
    expect(store.closes[0]).toBe(115)
    expect(store.highs[0]).toBe(115) // new high
    expect(store.lows[0]).toBe(90) // unchanged
    expect(store.volumes[0]).toBe(600) // accumulated
  })

  it('creates new candle when past interval', () => {
    const bars = [{ time: 1000, open: 100, high: 110, low: 90, close: 105, volume: 500 }]
    const store = ColumnStore.fromBars(bars)
    const action = store.applyTick(120, 200, 1060, 60) // at 60s boundary
    expect(action).toBe('created')
    expect(store.length).toBe(2)
    expect(store.opens[1]).toBe(120)
    expect(store.closes[1]).toBe(120)
    expect(store.times[1]).toBe(1060) // nextCandleTime
  })
})

describe('grow and evict', () => {
  it('grows capacity when full', () => {
    const bars = Array.from({ length: 512 }, (_, i) => ({
      time: i * 60, open: 100, high: 110, low: 90, close: 105, volume: 100,
    }))
    const store = ColumnStore.fromBars(bars) // capacity = 512 + 512 = 1024
    // Fill to capacity
    for (let i = 0; i < 513; i++) {
      store.applyTick(100, 100, (512 + i) * 60 + 60, 60)
    }
    expect(store.length).toBe(1025)
    // Should have grown without data loss
    expect(store.times[0]).toBe(0)
    expect(store.times[1024]).toBe(1025 * 60)
  })

  it('evicts oldest 25% when at max capacity', () => {
    // Create store near max capacity by using fromBars with large array
    const bars = Array.from({ length: 49999 }, (_, i) => ({
      time: i * 60, open: 100, high: 110, low: 90, close: 105, volume: 100,
    }))
    const store = ColumnStore.fromBars(bars)
    // Push one more to trigger eviction
    store.applyTick(100, 100, 49999 * 60 + 60, 60)
    // After eviction: kept 75% of 50000 = 37500
    expect(store.length).toBeLessThan(50000)
    expect(store.length).toBeGreaterThan(37000)
    // Newest data preserved
    expect(store.times[store.length - 1]).toBe(50000 * 60)
  })
})

describe('priceRange edge cases', () => {
  it('returns epsilon range when min equals max', () => {
    const bars = [
      { time: 0, open: 100, high: 100, low: 100, close: 100, volume: 0 },
      { time: 60, open: 100, high: 100, low: 100, close: 100, volume: 0 },
    ]
    const store = ColumnStore.fromBars(bars)
    const range = store.priceRange(0, 2)
    expect(range.max).toBeGreaterThan(range.min)
  })

  it('handles empty range gracefully', () => {
    const bars = [{ time: 0, open: 100, high: 110, low: 90, close: 105, volume: 100 }]
    const store = ColumnStore.fromBars(bars)
    const range = store.priceRange(5, 10) // out of bounds
    expect(range.min).toBeDefined()
    expect(range.max).toBeGreaterThan(range.min)
  })
})
```

- [ ] **Step 2: Run tests**

Run: `npx vitest run src/tests/columns.test.ts`

- [ ] **Step 3: Commit**

```bash
git add src/tests/columns.test.ts
git commit -m "test: add ColumnStore applyTick, grow/evict, and priceRange edge case tests"
```

---

### Task 3: IndicatorEngine Tests

**Files:**
- Create: `src/tests/indicatorEngine.test.ts`

- [ ] **Step 1: Write IndicatorEngine integration tests**

```typescript
// src/tests/indicatorEngine.test.ts
import { describe, it, expect, vi } from 'vitest'
import { IndicatorEngine } from '../indicators/IndicatorEngine'

function makeStore(length: number) {
  const closes = new Float64Array(length)
  for (let i = 0; i < length; i++) closes[i] = 100 + Math.sin(i * 0.1) * 20
  return { closes, length }
}

describe('IndicatorEngine', () => {
  it('bootstrap produces valid snapshot', () => {
    const engine = new IndicatorEngine()
    const store = makeStore(200)
    const snapshot = engine.bootstrap('AAPL', '5m', store)

    expect(snapshot.sma20).toBeInstanceOf(Float64Array)
    expect(snapshot.ema50).toBeInstanceOf(Float64Array)
    expect(snapshot.bbUpper).toBeInstanceOf(Float64Array)
    expect(snapshot.bbLower).toBeInstanceOf(Float64Array)
    // First 19 SMA values should be NaN (period=20)
    expect(isNaN(snapshot.sma20[18])).toBe(true)
    expect(isNaN(snapshot.sma20[19])).toBe(false)
  })

  it('onTick creates new indicator values', () => {
    const engine = new IndicatorEngine()
    const store = makeStore(100)
    engine.bootstrap('AAPL', '5m', store)

    const snapshot = engine.onTick('AAPL', '5m', 125, 'created')
    // Should have one more data point
    expect(snapshot.sma20.length).toBeGreaterThanOrEqual(101)
  })

  it('onTick updates last indicator value', () => {
    const engine = new IndicatorEngine()
    const store = makeStore(100)
    engine.bootstrap('AAPL', '5m', store)

    const snap1 = engine.onTick('AAPL', '5m', 130, 'updated')
    const snap2 = engine.onTick('AAPL', '5m', 140, 'updated')
    // Last SMA should differ between updates
    const idx = 99
    expect(snap2.sma20[idx]).not.toBe(snap1.sma20[idx])
  })

  it('subscribe notifies on tick', () => {
    const engine = new IndicatorEngine()
    const store = makeStore(100)
    engine.bootstrap('AAPL', '5m', store)

    const cb = vi.fn()
    engine.subscribe('AAPL', '5m', cb)
    engine.onTick('AAPL', '5m', 130, 'created')
    expect(cb).toHaveBeenCalledTimes(1)
  })

  it('remove cleans up state', () => {
    const engine = new IndicatorEngine()
    const store = makeStore(100)
    engine.bootstrap('AAPL', '5m', store)
    engine.remove('AAPL', '5m')
    expect(engine.getSnapshot('AAPL', '5m')).toBeNull()
  })

  it('throws on onTick without bootstrap', () => {
    const engine = new IndicatorEngine()
    expect(() => engine.onTick('AAPL', '5m', 100, 'created')).toThrow()
  })
})
```

- [ ] **Step 2: Run tests**

Run: `npx vitest run src/tests/indicatorEngine.test.ts`

- [ ] **Step 3: Commit**

```bash
git add src/tests/indicatorEngine.test.ts
git commit -m "test: add IndicatorEngine integration tests"
```

---

## Phase 2: Dynamic Indicator Registry + Toggles

### Task 4: Indicator Registry

**Files:**
- Create: `src/indicators/registry.ts`
- Modify: `src/indicators/types.ts`
- Modify: `src/indicators/index.ts`

- [ ] **Step 1: Update types**

```typescript
// src/indicators/types.ts
export interface IndicatorOutput {
  /** Display name shown in UI */
  name: string
  /** Parent indicator ID from the registry (e.g., 'sma20', 'bollinger') */
  indicatorId: string
  /** Unique key for this output (e.g., 'sma20', 'bbUpper') */
  key: string
  /** Line color [r, g, b, a] in 0-1 range */
  color: [number, number, number, number]
  /** Line width in pixels */
  width: number
  /** The computed values */
  values: Float64Array
}

export interface IncrementalIndicator {
  readonly id: string
  readonly name: string
  bootstrap(closes: Float64Array, length: number): void
  push(value: number): void
  updateLast(value: number): void
  getOutputs(): IndicatorOutput[]
  getLength(): number
}

// Legacy — kept for backward compat during migration, will be removed
export interface IndicatorSnapshot {
  sma20: Float64Array
  ema50: Float64Array
  bbUpper: Float64Array
  bbLower: Float64Array
  [key: string]: Float64Array  // dynamic indicators
}
```

- [ ] **Step 2: Create registry**

```typescript
// src/indicators/registry.ts
import type { IncrementalIndicator, IndicatorOutput } from './types'
import { IncrementalSMA } from './incremental/sma'
import { IncrementalEMA } from './incremental/ema'
import { IncrementalBollinger } from './incremental/bollinger'

/** Wraps our existing incremental classes into the IncrementalIndicator interface */
class SMAIndicator implements IncrementalIndicator {
  readonly id: string
  readonly name: string
  private sma: IncrementalSMA
  constructor(private period: number, private color: [number, number, number, number], private lineWidth: number) {
    this.id = `sma${period}`
    this.name = `SMA ${period}`
    this.sma = new IncrementalSMA(period, 2048)
  }
  bootstrap(closes: Float64Array, length: number) { this.sma = new IncrementalSMA(this.period, Math.max(length + 512, 2048)); this.sma.bootstrap(closes, length) }
  push(value: number) { this.sma.push(value) }
  updateLast(value: number) { this.sma.updateLast(value) }
  getLength() { return this.sma.getLength() }
  getOutputs(): IndicatorOutput[] {
    return [{ name: this.name, indicatorId: this.id, key: this.id, color: this.color, width: this.lineWidth, values: this.sma.getOutput() }]
  }
}

class EMAIndicator implements IncrementalIndicator {
  readonly id: string
  readonly name: string
  private ema: IncrementalEMA
  constructor(private period: number, private color: [number, number, number, number], private lineWidth: number) {
    this.id = `ema${period}`
    this.name = `EMA ${period}`
    this.ema = new IncrementalEMA(period, 2048)
  }
  bootstrap(closes: Float64Array, length: number) { this.ema = new IncrementalEMA(this.period, Math.max(length + 512, 2048)); this.ema.bootstrap(closes, length) }
  push(value: number) { this.ema.push(value) }
  updateLast(value: number) { this.ema.updateLast(value) }
  getLength() { return this.ema.getLength() }
  getOutputs(): IndicatorOutput[] {
    return [{ name: this.name, indicatorId: this.id, key: this.id, color: this.color, width: this.lineWidth, values: this.ema.getOutput() }]
  }
}

class BollingerIndicator implements IncrementalIndicator {
  readonly id = 'bollinger'
  readonly name = 'Bollinger Bands'
  private bb: IncrementalBollinger
  constructor(private period: number, private stdDevs: number, private color: [number, number, number, number], private lineWidth: number) {
    this.bb = new IncrementalBollinger(period, stdDevs, 2048)
  }
  bootstrap(closes: Float64Array, length: number) { this.bb = new IncrementalBollinger(this.period, this.stdDevs, Math.max(length + 512, 2048)); this.bb.bootstrap(closes, length) }
  push(value: number) { this.bb.push(value) }
  updateLast(value: number) { this.bb.updateLast(value) }
  getLength() { return this.bb.getLength() }
  getOutputs(): IndicatorOutput[] {
    return [
      { name: 'BB Upper', indicatorId: this.id, key: 'bbUpper', color: this.color, width: this.lineWidth, values: this.bb.getUpper() },
      { name: 'BB Lower', indicatorId: this.id, key: 'bbLower', color: this.color, width: this.lineWidth, values: this.bb.getLower() },
    ]
  }
}

export type IndicatorFactory = () => IncrementalIndicator

/** Registry of available indicators. Use getDefaults() for the standard set. */
export const INDICATOR_CATALOG: Record<string, { name: string; factory: IndicatorFactory }> = {
  sma20:     { name: 'SMA 20',    factory: () => new SMAIndicator(20, [0.3, 0.6, 1.0, 0.8], 2.0) },
  ema50:     { name: 'EMA 50',    factory: () => new EMAIndicator(50, [1.0, 0.6, 0.2, 0.8], 2.0) },
  bollinger: { name: 'Bollinger', factory: () => new BollingerIndicator(20, 2, [0.5, 0.5, 0.5, 0.4], 1.0) },
}

export function getDefaultIndicatorIds(): string[] {
  return ['sma20', 'ema50', 'bollinger']
}
```

- [ ] **Step 3: Update index exports**

```typescript
// src/indicators/index.ts
export { IndicatorEngine } from './IndicatorEngine'
export { INDICATOR_CATALOG, getDefaultIndicatorIds } from './registry'
export type { IndicatorSnapshot, IndicatorOutput, IncrementalIndicator } from './types'
```

- [ ] **Step 4: Commit**

```bash
git add src/indicators/registry.ts src/indicators/types.ts src/indicators/index.ts
git commit -m "feat: add dynamic indicator registry with catalog pattern"
```

---

### Task 5: Refactor IndicatorEngine to Use Registry

**Files:**
- Modify: `src/indicators/IndicatorEngine.ts`

- [ ] **Step 1: Rewrite IndicatorEngine**

Replace the hard-coded SMA/EMA/Bollinger with dynamic indicators from registry:

```typescript
// src/indicators/IndicatorEngine.ts
import { INDICATOR_CATALOG } from './registry'
import type { IncrementalIndicator, IndicatorSnapshot, IndicatorOutput } from './types'

interface SymbolState {
  indicators: Map<string, IncrementalIndicator>  // id → indicator
}

interface ColumnStoreLike {
  closes: Float64Array
  length: number
}

export class IndicatorEngine {
  private state = new Map<string, SymbolState>()
  private subscribers = new Map<string, Set<(snapshot: IndicatorSnapshot) => void>>()

  private key(symbol: string, tf: string): string { return `${symbol}:${tf}` }

  bootstrap(symbol: string, timeframe: string, data: ColumnStoreLike, indicatorIds?: string[]): IndicatorSnapshot {
    const k = this.key(symbol, timeframe)
    const ids = indicatorIds ?? Object.keys(INDICATOR_CATALOG)
    const indicators = new Map<string, IncrementalIndicator>()

    for (const id of ids) {
      const entry = INDICATOR_CATALOG[id]
      if (!entry) { console.warn(`Unknown indicator: ${id}`); continue }
      const ind = entry.factory()
      ind.bootstrap(data.closes, data.length)
      indicators.set(id, ind)
    }

    this.state.set(k, { indicators })
    return this.buildSnapshot(indicators)
  }

  onTick(symbol: string, timeframe: string, price: number, action: 'updated' | 'created'): IndicatorSnapshot {
    const k = this.key(symbol, timeframe)
    const state = this.state.get(k)
    if (!state) throw new Error(`No indicator state for ${k}`)

    for (const ind of state.indicators.values()) {
      if (action === 'created') ind.push(price)
      else ind.updateLast(price)
    }

    const snapshot = this.buildSnapshot(state.indicators)
    this.subscribers.get(k)?.forEach(cb => { try { cb(snapshot) } catch (e) { /* */ } })
    return snapshot
  }

  /** Get all outputs for rendering (flat list of IndicatorOutput) */
  getOutputs(symbol: string, timeframe: string): IndicatorOutput[] {
    const state = this.state.get(this.key(symbol, timeframe))
    if (!state) return []
    const outputs: IndicatorOutput[] = []
    for (const ind of state.indicators.values()) {
      outputs.push(...ind.getOutputs())
    }
    return outputs
  }

  getSnapshot(symbol: string, timeframe: string): IndicatorSnapshot | null {
    const state = this.state.get(this.key(symbol, timeframe))
    return state ? this.buildSnapshot(state.indicators) : null
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

  private buildSnapshot(indicators: Map<string, IncrementalIndicator>): IndicatorSnapshot {
    const snapshot: IndicatorSnapshot = {} as IndicatorSnapshot
    for (const ind of indicators.values()) {
      for (const out of ind.getOutputs()) {
        snapshot[out.key] = out.values
      }
    }
    return snapshot
  }
}
```

- [ ] **Step 2: Run existing indicator tests to verify backward compat**

Run: `npx vitest run src/tests/indicatorEngine.test.ts`

- [ ] **Step 3: Commit**

```bash
git add src/indicators/IndicatorEngine.ts
git commit -m "refactor: IndicatorEngine uses dynamic registry instead of hard-coded indicators"
```

---

### Task 6: Volume Renderer

**Files:**
- Create: `src/renderer/shaders/volume.wgsl`
- Create: `src/renderer/VolumeRenderer.ts`
- Modify: `src/renderer/index.ts`

- [ ] **Step 1: Create volume shader**

```wgsl
// src/renderer/shaders/volume.wgsl
struct VolumeInstance {
  @location(0) x_clip:    f32,
  @location(1) height:    f32, // 0-1 normalized
  @location(2) body_w:    f32,
  @location(3) color:     vec4<f32>,
}

struct VertOut {
  @builtin(position) pos: vec4<f32>,
  @location(0) color: vec4<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vi: u32, inst: VolumeInstance) -> VertOut {
  // Volume bars sit at the bottom of the chart
  // baseY is the bottom edge in clip space (-1 typically, but we use a uniform)
  let corners = array<vec2<f32>, 6>(
    vec2(0.0, 0.0), vec2(1.0, 0.0), vec2(0.0, 1.0),
    vec2(0.0, 1.0), vec2(1.0, 0.0), vec2(1.0, 1.0),
  );

  let c = corners[vi];
  let left = inst.x_clip - inst.body_w;
  let right = inst.x_clip + inst.body_w;
  let bottom = -1.0; // bottom of clip space
  let top = bottom + inst.height * 0.3; // volume takes bottom 30% of chart

  let x = left + c.x * (right - left);
  let y = bottom + c.y * (top - bottom);

  var out: VertOut;
  out.pos = vec4(x, y, 0.0, 1.0);
  out.color = inst.color;
  return out;
}

@fragment
fn fs_main(in: VertOut) -> @location(0) vec4<f32> {
  return in.color;
}
```

- [ ] **Step 2: Create VolumeRenderer**

```typescript
// src/renderer/VolumeRenderer.ts
import type { GPUContext } from '../engine/types'
import { CoordSystem } from '../engine/types'
import type { ColumnStore } from '../data/columns'
import shaderSrc from './shaders/volume.wgsl?raw'

const FLOATS_PER_INSTANCE = 8 // x, height, bodyW, color(rgba)
const VERTS_PER_BAR = 6
const BULL_COLOR = [0.18, 0.78, 0.45, 0.25]
const BEAR_COLOR = [0.93, 0.27, 0.27, 0.25]

export class VolumeRenderer {
  private pipeline: GPURenderPipeline
  private instanceBuffer: GPUBuffer | null = null
  private instanceBufferSize = 0
  private instanceCount = 0
  private readonly device: GPUDevice
  private cpuBuffer: Float32Array | null = null

  constructor(ctx: GPUContext) {
    this.device = ctx.device
    const module = ctx.device.createShaderModule({ code: shaderSrc })

    this.pipeline = ctx.device.createRenderPipeline({
      layout: 'auto',
      vertex: {
        module, entryPoint: 'vs_main',
        buffers: [{
          arrayStride: FLOATS_PER_INSTANCE * 4,
          stepMode: 'instance',
          attributes: [
            { shaderLocation: 0, offset: 0,  format: 'float32' },   // x_clip
            { shaderLocation: 1, offset: 4,  format: 'float32' },   // height
            { shaderLocation: 2, offset: 8,  format: 'float32' },   // body_w
            { shaderLocation: 3, offset: 12, format: 'float32x4' }, // color
          ],
        }],
      },
      fragment: {
        module, entryPoint: 'fs_main',
        targets: [{
          format: ctx.format,
          blend: {
            color: { srcFactor: 'src-alpha', dstFactor: 'one-minus-src-alpha', operation: 'add' },
            alpha: { srcFactor: 'one', dstFactor: 'one-minus-src-alpha', operation: 'add' },
          },
        }],
      },
      primitive: { topology: 'triangle-list' },
    })
  }

  upload(data: ColumnStore, cs: CoordSystem, viewStart: number, viewCount: number) {
    const end = Math.min(viewStart + viewCount, data.length)
    const count = end - viewStart
    if (count <= 0) return

    // Find max volume in view for normalization
    let maxVol = 0
    for (let i = viewStart; i < end; i++) {
      if (data.volumes[i] > maxVol) maxVol = data.volumes[i]
    }
    if (maxVol === 0) maxVol = 1

    const floatsNeeded = count * FLOATS_PER_INSTANCE
    if (!this.cpuBuffer || this.cpuBuffer.length < floatsNeeded) {
      this.cpuBuffer = new Float32Array(Math.ceil(floatsNeeded * 1.5))
    }
    const arr = this.cpuBuffer
    const bodyW = cs.clipBarWidth() * 0.4

    for (let i = 0; i < count; i++) {
      const di = viewStart + i
      const base = i * FLOATS_PER_INSTANCE
      const isBull = data.closes[di] >= data.opens[di]
      const color = isBull ? BULL_COLOR : BEAR_COLOR

      arr[base + 0] = cs.barToClipX(i)
      arr[base + 1] = data.volumes[di] / maxVol // normalized 0-1
      arr[base + 2] = bodyW
      arr[base + 3] = color[0]
      arr[base + 4] = color[1]
      arr[base + 5] = color[2]
      arr[base + 6] = color[3]
      arr[base + 7] = 0 // padding
    }

    const byteLength = floatsNeeded * 4
    if (this.instanceBuffer && this.instanceBufferSize >= byteLength) {
      this.device.queue.writeBuffer(this.instanceBuffer, 0, arr, 0, floatsNeeded)
    } else {
      if (this.instanceBuffer) this.instanceBuffer.destroy()
      const allocSize = Math.ceil(byteLength * 1.5)
      this.instanceBuffer = this.device.createBuffer({
        size: allocSize, usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST,
      })
      this.instanceBufferSize = allocSize
      this.device.queue.writeBuffer(this.instanceBuffer, 0, arr, 0, floatsNeeded)
    }
    this.instanceCount = count
  }

  render(pass: GPURenderPassEncoder) {
    if (!this.instanceBuffer || this.instanceCount === 0) return
    pass.setPipeline(this.pipeline)
    pass.setVertexBuffer(0, this.instanceBuffer)
    pass.draw(VERTS_PER_BAR, this.instanceCount)
  }

  destroy() {
    this.instanceBuffer?.destroy()
    this.instanceBuffer = null
    this.instanceBufferSize = 0
    this.cpuBuffer = null
  }
}
```

- [ ] **Step 3: Update renderer index**

```typescript
// src/renderer/index.ts
export { CandleRenderer } from './CandleRenderer'
export { GridRenderer } from './GridRenderer'
export { LineRenderer } from './LineRenderer'
export { VolumeRenderer } from './VolumeRenderer'
```

- [ ] **Step 4: Commit**

```bash
git add src/renderer/shaders/volume.wgsl src/renderer/VolumeRenderer.ts src/renderer/index.ts
git commit -m "feat: add VolumeRenderer with instanced GPU rendering"
```

---

### Task 7: Per-Pane Visibility Toggles in ChartStore

**Files:**
- Modify: `src/store/chartStore.ts`

- [ ] **Step 1: Add visibility state to chartStore**

Add `showVolume` and `visibleIndicators` to PaneConfig:

```typescript
// In chartStore.ts, update PaneConfig interface:
interface PaneConfig {
  symbol: string
  timeframe: Timeframe
  showVolume: boolean
  visibleIndicators: string[]  // indicator IDs from registry
}

// Update defaultPanes to include new fields:
const defaultPanes: PaneConfig[] = [
  { symbol: 'AAPL', timeframe: '5m', showVolume: true, visibleIndicators: ['sma20', 'ema50', 'bollinger'] },
  // ... same for all 7
]

// Add new actions:
interface ChartState {
  // ... existing
  toggleVolume: (paneId: string) => void
  toggleIndicator: (paneId: string, indicatorId: string) => void
}

// Implementations:
toggleVolume: (paneId) => set(s => {
  const panes = s.panes.map(p =>
    p.id === paneId ? { ...p, showVolume: !p.showVolume } : p
  )
  return { panes }
}),
toggleIndicator: (paneId, indicatorId) => set(s => {
  const panes = s.panes.map(p => {
    if (p.id !== paneId) return p
    const vis = p.visibleIndicators.includes(indicatorId)
      ? p.visibleIndicators.filter(id => id !== indicatorId)
      : [...p.visibleIndicators, indicatorId]
    return { ...p, visibleIndicators: vis }
  })
  return { panes }
}),
```

- [ ] **Step 2: Commit**

```bash
git add src/store/chartStore.ts
git commit -m "feat: add per-pane volume and indicator visibility toggles to chartStore"
```

---

### Task 8: Wire Toggles into PaneContext and Rendering

**Files:**
- Modify: `src/engine/PaneContext.ts`
- Modify: `src/chart/ChartPane.tsx`

- [ ] **Step 1: Update PaneContext to support dynamic indicators and volume**

Key changes to PaneContext:
- Add `VolumeRenderer`
- Replace hard-coded `LINE_CONFIGS` with dynamic line renderers that grow/shrink as needed
- Add `showVolume` and `visibleIndicatorOutputs` to the render call
- Render volume bars BEFORE candles (so candles draw on top)

PaneContext needs:
```typescript
// Add to constructor:
private volumeRenderer: VolumeRenderer

// Add to render():
// 1. Render volume bars first (behind candles)
if (this.showVolume && this.data) {
  this.volumeRenderer.upload(this.data, cs, viewStart, viewCount)
  this.volumeRenderer.render(pass)
}
// 2. Grid
// 3. Candles
// 4. Dynamic indicator lines
for (let i = 0; i < this.indicatorOutputs.length; i++) {
  const out = this.indicatorOutputs[i]
  if (!this.lineRenderers[i]) this.lineRenderers[i] = new LineRenderer({ device: this.device, format: this.format })
  this.lineRenderers[i].upload(out.values, cs, viewStart, viewCount, out.color, out.width)
  this.lineRenderers[i].render(pass)
}
```

Add methods:
```typescript
setVisibility(showVolume: boolean, indicatorOutputs: IndicatorOutput[]): void
```

- [ ] **Step 2: Update ChartPane to pass visibility from store**

ChartPane reads `showVolume` and `visibleIndicators` from chartStore, gets filtered `IndicatorOutput[]` from IndicatorEngine, passes to PaneContext.

```typescript
// In ChartPane, in the data subscription effect:
const paneConfig = useChartStore(s => s.panes[paneIndex])
const { showVolume, visibleIndicators } = paneConfig

// When pushing data to PaneContext:
const outputs = getIndicatorEngine().getOutputs(symbol, timeframe)
  .filter(out => visibleIndicators.includes(out.indicatorId))
paneRef.current?.setVisibility(showVolume, outputs)
```

- [ ] **Step 3: Commit**

```bash
git add src/engine/PaneContext.ts src/chart/ChartPane.tsx
git commit -m "feat: wire volume and indicator toggles into rendering pipeline"
```

---

### Task 9: Toggle UI in Toolbar

**Files:**
- Modify: `src/toolbar/Toolbar.tsx`

- [ ] **Step 1: Add toggle buttons**

Add to Toolbar after the timeframe buttons:

```tsx
{/* Indicator toggles */}
{Object.entries(INDICATOR_CATALOG).map(([id, { name }]) => {
  const active = pane?.visibleIndicators.includes(id)
  return (
    <button key={id}
      onClick={() => toggleIndicator(activePane, id)}
      style={{ background: active ? '#1a3a5c' : '#1a1a1a', color: active ? '#4a9eff' : '#555',
        border: '1px solid ' + (active ? '#2a5a8c' : '#333'), borderRadius: 3,
        padding: '2px 8px', fontSize: 11, fontFamily: 'monospace', cursor: 'pointer' }}>
      {name}
    </button>
  )
})}
{/* Volume toggle */}
<button onClick={() => toggleVolume(activePane)}
  style={{ background: pane?.showVolume ? '#1a3a5c' : '#1a1a1a', color: pane?.showVolume ? '#4a9eff' : '#555',
    border: '1px solid ' + (pane?.showVolume ? '#2a5a8c' : '#333'), borderRadius: 3,
    padding: '2px 8px', fontSize: 11, fontFamily: 'monospace', cursor: 'pointer' }}>
  VOL
</button>
```

- [ ] **Step 2: Verify build**

Run: `npx tsc --noEmit && echo "OK"`

- [ ] **Step 3: Commit**

```bash
git add src/toolbar/Toolbar.tsx
git commit -m "feat: add indicator and volume toggle buttons to toolbar"
```

---

## Phase 3: Performance

### Task 10: Partial Tick Uploads

**Files:**
- Modify: `src/engine/PaneContext.ts`

- [ ] **Step 1: Track dirty reason**

Instead of a simple `dirty` boolean, track what changed:

```typescript
// In PaneContext:
private dirtyFlags = { viewport: true, data: true, lastTick: false }

setViewport(v) {
  this.viewport = v
  this.dirtyFlags.viewport = true
  this.dirty = true
  this.markDirtyFn()
}

setData(d, indicators, action?: 'updated' | 'created') {
  this.data = d
  this.indicators = indicators
  if (action === 'updated') {
    this.dirtyFlags.lastTick = true
  } else {
    this.dirtyFlags.data = true
  }
  this.dirty = true
  this.markDirtyFn()
}
```

**To propagate the tick action to PaneContext**, add `lastAction` tracking to DataStore:

```typescript
// In DataStore:
private lastActions = new Map<string, 'updated' | 'created'>()

applyTick(...) {
  // ... existing logic ...
  const action = store.applyTick(tick.price, tick.volume, tick.time, tf.seconds)
  this.lastActions.set(k, action)
  // ...
}

getLastAction(symbol: string, timeframe: string): 'updated' | 'created' | null {
  return this.lastActions.get(this.key(symbol, timeframe)) ?? null
}
```

Then in ChartPane's data subscription:
```typescript
return ds.subscribe(symbol, timeframe, () => {
  const d = ds.getData(symbol, timeframe)
  const indicators = ds.getIndicators(symbol, timeframe)
  const action = ds.getLastAction(symbol, timeframe)
  if (d && indicators) paneRef.current?.setData(d, indicators, action ?? undefined)
})
```

In `render()`, only re-upload candle data if `dirtyFlags.data || dirtyFlags.viewport`. If only `dirtyFlags.lastTick`, do a surgical write of just the last candle to the GPU buffer.

CandleRenderer needs a new method: `updateLastCandle(data, cs, viewStart, viewCount)` that writes only the last instance to the existing buffer via `writeBuffer` at the correct offset.

- [ ] **Step 2: Add updateLastCandle to CandleRenderer**

```typescript
// In CandleRenderer:
updateLastCandle(data: ColumnStore, cs: CoordSystem, viewStart: number, viewCount: number) {
  if (!this.instanceBuffer) return
  const end = Math.min(viewStart + viewCount, data.length)
  const lastIdx = end - 1 - viewStart
  if (lastIdx < 0 || lastIdx >= this.instanceCount) return

  const di = viewStart + lastIdx
  const bodyW = cs.clipBarWidth() * 0.5
  const isBull = data.closes[di] >= data.opens[di]
  const color = isBull ? BULL_COLOR : BEAR_COLOR

  // Write into CPU buffer at last candle offset
  const base = lastIdx * FLOATS_PER_INSTANCE
  if (!this.cpuBuffer) return
  this.cpuBuffer[base + 0] = cs.barToClipX(lastIdx)
  this.cpuBuffer[base + 1] = cs.priceToClipY(data.opens[di])
  this.cpuBuffer[base + 2] = cs.priceToClipY(data.closes[di])
  this.cpuBuffer[base + 3] = cs.priceToClipY(data.lows[di])
  this.cpuBuffer[base + 4] = cs.priceToClipY(data.highs[di])
  this.cpuBuffer[base + 5] = bodyW
  this.cpuBuffer[base + 6] = color[0]
  this.cpuBuffer[base + 7] = color[1]
  this.cpuBuffer[base + 8] = color[2]
  this.cpuBuffer[base + 9] = color[3]

  // Surgical GPU write — just 40 bytes for one candle
  this.device.queue.writeBuffer(
    this.instanceBuffer, base * 4,
    this.cpuBuffer, base, FLOATS_PER_INSTANCE
  )
}
```

- [ ] **Step 3: Commit**

```bash
git add src/engine/PaneContext.ts src/renderer/CandleRenderer.ts
git commit -m "perf: partial tick uploads — surgical GPU write for last candle on tick update"
```

---

### Task 11: Frame Timing Telemetry

**Files:**
- Modify: `src/engine/FrameScheduler.ts`
- Modify: `src/engine/types.ts`
- Create: `src/chart/FrameStats.tsx`

- [ ] **Step 1: Add FrameStats type**

```typescript
// Add to src/engine/types.ts
export interface FrameStats {
  fps: number
  frameTimeMs: number
  frameTimePeak: number
  dirtyPanes: number
}
```

- [ ] **Step 2: Add timing to FrameScheduler**

```typescript
// In FrameScheduler, add:
private frameTimes = new Float64Array(120) // ring buffer, 2 seconds at 60fps
private frameIdx = 0
private lastFrameTime = 0

// In tick(), at the start:
const now = performance.now()
if (this.lastFrameTime > 0) {
  this.frameTimes[this.frameIdx % 120] = now - this.lastFrameTime
  this.frameIdx++
}
this.lastFrameTime = now

// New public method:
getStats(): FrameStats {
  const count = Math.min(this.frameIdx, 120)
  if (count === 0) return { fps: 0, frameTimeMs: 0, frameTimePeak: 0, dirtyPanes: 0 }
  let sum = 0, peak = 0
  for (let i = 0; i < count; i++) {
    const t = this.frameTimes[i]
    sum += t
    if (t > peak) peak = t
  }
  const avg = sum / count
  let dirtyCount = 0
  for (const pane of this.panes.values()) if (pane.dirty) dirtyCount++
  return { fps: Math.round(1000 / avg), frameTimeMs: Math.round(avg * 100) / 100, frameTimePeak: Math.round(peak * 100) / 100, dirtyPanes: dirtyCount }
}
```

- [ ] **Step 3: Create FrameStats overlay**

```tsx
// src/chart/FrameStats.tsx
import { useState, useEffect } from 'react'
import { getRenderEngine } from '../globals'

export function FrameStats() {
  const [stats, setStats] = useState({ fps: 0, frameTimeMs: 0, frameTimePeak: 0, dirtyPanes: 0 })

  useEffect(() => {
    const id = setInterval(() => {
      setStats(getRenderEngine().scheduler.getStats())
    }, 500)
    return () => clearInterval(id)
  }, [])

  return (
    <div style={{
      position: 'fixed', top: 40, right: 8, zIndex: 999,
      background: '#111', border: '1px solid #333', borderRadius: 3,
      padding: '4px 8px', fontFamily: 'monospace', fontSize: 10, color: '#666',
    }}>
      {stats.fps} fps · {stats.frameTimeMs}ms · peak {stats.frameTimePeak}ms · {stats.dirtyPanes} dirty
    </div>
  )
}
```

- [ ] **Step 4: Wire into App.tsx**

Add `<FrameStats />` to App layout (conditionally, e.g., always-on for now, can add a keyboard toggle later).

- [ ] **Step 5: Commit**

```bash
git add src/engine/FrameScheduler.ts src/engine/types.ts src/chart/FrameStats.tsx src/App.tsx
git commit -m "feat: add frame timing telemetry with FPS overlay"
```

---

### Task 12: CoordSystem Flyweight Cache

**Files:**
- Modify: `src/chart/CoordSystem.ts`

- [ ] **Step 1: Add cache to CoordSystem constructor**

```typescript
// At module level in CoordSystem.ts:
const cache = new Map<string, CoordSystem>()
const MAX_CACHE = 64

function cacheKey(cfg: CoordConfig): string {
  return `${cfg.width}|${cfg.height}|${cfg.barCount}|${cfg.minPrice.toFixed(6)}|${cfg.maxPrice.toFixed(6)}|${cfg.paddingRight ?? 80}|${cfg.paddingTop ?? 20}|${cfg.paddingBottom ?? 40}`
}

// In CoordSystem class, add static factory:
static create(config: CoordConfig): CoordSystem {
  const key = cacheKey(config)
  const cached = cache.get(key)
  if (cached) return cached
  const cs = new CoordSystem(config)
  if (cache.size >= MAX_CACHE) {
    // Evict oldest (first inserted)
    const firstKey = cache.keys().next().value
    if (firstKey) cache.delete(firstKey)
  }
  cache.set(key, cs)
  return cs
}
```

Update `useChartViewport.ts` to use `CoordSystem.create()` instead of `new CoordSystem()`.

- [ ] **Step 2: Commit**

```bash
git add src/chart/CoordSystem.ts src/chart/useChartViewport.ts
git commit -m "perf: CoordSystem flyweight cache eliminates redundant object creation"
```

---

## Phase 4: Data & Production

### Task 13: ColumnStore prepend() for Historical Pagination

**Files:**
- Modify: `src/data/columns.ts`

- [ ] **Step 1: Add prepend method**

```typescript
// In ColumnStore:
/** Prepend older bars. Truncates input if it would exceed MAX_CAPACITY. */
prepend(bars: Bar[]): number {
  if (bars.length === 0) return 0
  // Limit: don't exceed MAX_CAPACITY. Truncate prepend if needed.
  const maxPrepend = Math.max(0, 50_000 - this.length)
  const actualBars = bars.slice(-maxPrepend) // keep newest of the prepend batch
  if (actualBars.length === 0) return 0

  const newLen = this.length + actualBars.length
  const names = ['times', 'opens', 'highs', 'lows', 'closes', 'volumes'] as const
  const barKeys = ['time', 'open', 'high', 'low', 'close', 'volume'] as const

  // Grow arrays if needed
  if (newLen > this.capacity) {
    const newCap = Math.min(Math.max(newLen, this.capacity * 2), 50_000)
    for (const name of names) {
      const old = this[name]
      const arr = new Float64Array(newCap)
      arr.set(old.subarray(0, this.length), actualBars.length)
      this[name] = arr
    }
    this.capacity = newCap
  } else {
    for (const name of names) {
      this[name].copyWithin(actualBars.length, 0, this.length)
    }
  }

  for (let i = 0; i < actualBars.length; i++) {
    for (let j = 0; j < names.length; j++) {
      this[names[j]][i] = actualBars[i][barKeys[j] as keyof Bar] as number
    }
  }
  this.length = newLen
  return actualBars.length // return how many were actually prepended
}
```

- [ ] **Step 2: Commit**

```bash
git add src/data/columns.ts
git commit -m "feat: add ColumnStore.prepend() for historical data pagination"
```

---

### Task 14: Historical Data Pagination

**Files:**
- Modify: `src/data/DataStore.ts`
- Modify: `src/chart/useChartViewport.ts`

- [ ] **Step 1: Add loadMore to DataStore**

```typescript
// In DataStore:
private oldestLoaded = new Map<string, number>() // key → oldest timestamp loaded

async loadMore(symbol: string, timeframe: string): Promise<number> {
  const k = this.key(symbol, timeframe)
  const store = this.stores.get(k)
  if (!store || store.length === 0) return 0

  const tf = TF_TO_INTERVAL[timeframe as Timeframe] ?? TF_TO_INTERVAL['5m']
  const oldestTime = store.times[0]
  // Request bars ending before our oldest
  const endDate = new Date(oldestTime * 1000).toISOString().split('T')[0]

  try {
    const bars: Bar[] = await invoke('get_bars', {
      symbol, interval: tf.interval, period: tf.period, end: endDate
    })
    // Filter out bars we already have
    const newBars = bars.filter(b => b.time < oldestTime)
    if (newBars.length === 0) return 0
    store.prepend(newBars)
    // Re-bootstrap indicators with full data
    const indicators = this.indicatorEngine.bootstrap(symbol, timeframe, store)
    this.snapshots.set(k, indicators)
    this.notify(k)
    return newBars.length
  } catch (e) {
    console.warn(`Failed to load more ${k}:`, e)
    return 0
  }
}
```

- [ ] **Step 2: Trigger pagination on scroll-left**

In `useChartViewport.ts`, detect when `viewStart === 0`:

```typescript
// In useChartViewport, add effect:
const loadingMore = useRef(false)

useEffect(() => {
  if (viewStart > 0 || loadingMore.current) return
  loadingMore.current = true
  const ds = getDataStore()
  ds.loadMore(symbol, timeframe).then(added => {
    if (added > 0) setViewStart(v => v + added) // keep same visual position
    loadingMore.current = false
  })
}, [viewStart, symbol, timeframe])
```

- [ ] **Step 3: Commit**

```bash
git add src/data/DataStore.ts src/chart/useChartViewport.ts
git commit -m "feat: historical data pagination — auto-loads more bars on scroll-left"
```

---

### Task 15: Bar Cache (IndexedDB)

**Files:**
- Create: `src/data/BarCache.ts`
- Modify: `src/data/DataStore.ts`

- [ ] **Step 1: Create BarCache**

```typescript
// src/data/BarCache.ts
import type { Bar } from '../types'

const DB_NAME = 'apex-bars'
const DB_VERSION = 1

function openDB(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const req = indexedDB.open(DB_NAME, DB_VERSION)
    req.onupgradeneeded = () => {
      const db = req.result
      if (!db.objectStoreNames.contains('bars')) {
        db.createObjectStore('bars')
      }
    }
    req.onsuccess = () => resolve(req.result)
    req.onerror = () => reject(req.error)
  })
}

export class BarCache {
  private db: IDBDatabase | null = null

  async init(): Promise<void> {
    this.db = await openDB()
  }

  async get(symbol: string, timeframe: string): Promise<Bar[] | null> {
    if (!this.db) return null
    const key = `${symbol}:${timeframe}`
    return new Promise((resolve) => {
      const tx = this.db!.transaction('bars', 'readonly')
      const req = tx.objectStore('bars').get(key)
      req.onsuccess = () => resolve(req.result ?? null)
      req.onerror = () => resolve(null)
    })
  }

  async set(symbol: string, timeframe: string, bars: Bar[]): Promise<void> {
    if (!this.db) return
    const key = `${symbol}:${timeframe}`
    return new Promise((resolve) => {
      const tx = this.db!.transaction('bars', 'readwrite')
      tx.objectStore('bars').put(bars, key)
      tx.oncomplete = () => resolve()
      tx.onerror = () => resolve() // don't crash on cache failure
    })
  }
}
```

- [ ] **Step 2: Wire into DataStore**

In `DataStore.doLoad()`:
1. Try cache first: `const cached = await this.barCache.get(symbol, timeframe)`
2. If cached, use it but also fire a background refresh
3. If not cached, load from IPC and cache the result
4. On any successful IPC load, update cache

```typescript
// In DataStore constructor:
constructor(private indicatorEngine: IndicatorEngine, private barCache?: BarCache) {}

// In doLoad():
private async doLoad(symbol: string, timeframe: string, k: string) {
  const tf = TF_TO_INTERVAL[timeframe as Timeframe] ?? TF_TO_INTERVAL['5m']

  // Try cache first for instant render
  const cached = this.barCache ? await this.barCache.get(symbol, timeframe) : null
  if (cached && cached.length > 0) {
    const store = ColumnStore.fromBars(cached)
    this.stores.set(k, store)
    const indicators = this.indicatorEngine.bootstrap(symbol, timeframe, store)
    this.snapshots.set(k, indicators)
    this.notify(k)
    // Background refresh from API
    this.refreshFromAPI(symbol, timeframe, k, tf).catch(() => {})
    return { data: store, indicators }
  }

  // No cache — load from API
  const bars: Bar[] = await invoke('get_bars', { symbol, interval: tf.interval, period: tf.period })
  const store = ColumnStore.fromBars(bars)
  this.stores.set(k, store)
  const indicators = this.indicatorEngine.bootstrap(symbol, timeframe, store)
  this.snapshots.set(k, indicators)
  this.notify(k)
  // Cache for next time
  this.barCache?.set(symbol, timeframe, bars).catch(() => {})
  return { data: store, indicators }
}
```

- [ ] **Step 3: Init BarCache in bootstrap**

```typescript
// In main.tsx bootstrap():
const barCache = new BarCache()
await barCache.init()
const dataStore = new DataStore(indicatorEngine, barCache)
```

- [ ] **Step 4: Commit**

```bash
git add src/data/BarCache.ts src/data/DataStore.ts src/main.tsx
git commit -m "feat: IndexedDB bar cache for instant chart load on restart"
```

---

### Task 16: Feed Reconnection Protocol

**Files:**
- Modify: `src/data/Feed.ts`
- Modify: `src/data/SimulatedFeed.ts`

- [ ] **Step 1: Add reconnection to Feed interface**

```typescript
// In Feed interface, add:
onReconnect(cb: () => void): () => void
```

- [ ] **Step 2: Add reconnection to SimulatedFeed**

For the simulated feed, this is trivial (just restart the interval). The important thing is the interface is ready for a real WebSocket feed:

```typescript
// In SimulatedFeed:
private reconnectListeners = new Set<() => void>()
private connected = false

onReconnect(cb: () => void): () => void {
  this.reconnectListeners.add(cb)
  return () => { this.reconnectListeners.delete(cb) }
}

async connect(): Promise<void> {
  if (this.connected) return
  this.connected = true
  this.startTicking()
}

disconnect(): void {
  this.connected = false
  if (this.interval) { clearInterval(this.interval); this.interval = null }
  for (const cb of this.disconnectListeners) { try { cb() } catch (e) {} }
}

// Simulate a reconnection (useful for testing)
async reconnect(): Promise<void> {
  this.connected = true
  this.startTicking()
  for (const cb of this.reconnectListeners) { try { cb() } catch (e) {} }
}

private startTicking(): void {
  if (this.interval) clearInterval(this.interval)
  this.interval = window.setInterval(() => { /* existing tick logic */ }, 250)
}
```

- [ ] **Step 3: Wire disconnect/reconnect into DataStore**

DataStore should subscribe to feed events and show status:

```typescript
// In main.tsx bootstrap(), after feed.connect():
feed.onDisconnect(() => console.warn('Feed disconnected'))
feed.onReconnect(() => {
  console.info('Feed reconnected')
  // Mark all panes dirty to refresh with latest data
  for (const pane of engine.getAllPanes()) pane.dirty = true
})
```

- [ ] **Step 4: Commit**

```bash
git add src/data/Feed.ts src/data/SimulatedFeed.ts src/main.tsx
git commit -m "feat: feed reconnection protocol with disconnect/reconnect events"
```

---

## Phase 5: Build & Verify

### Task 17: Full Build & Integration Test

- [ ] **Step 1: TypeScript check**

Run: `npx tsc --noEmit`
Expected: No errors

- [ ] **Step 2: Run all tests**

Run: `npx vitest run`
Expected: All pass

- [ ] **Step 3: Production build**

Run: `npm run build`
Expected: Clean build

- [ ] **Step 4: Launch and verify**

Run: `cargo tauri dev`
Verify:
1. All 7 chart panes render candlesticks
2. Volume bars visible below candles
3. Toolbar shows toggle buttons: SMA 20, EMA 50, Bollinger, VOL
4. Clicking toggle hides/shows the indicator
5. FPS overlay shows in top-right corner
6. Pan left past loaded data triggers pagination (loads more bars)
7. App restart loads data from cache instantly

- [ ] **Step 5: Final commit**

```bash
git add -A
git commit -m "feat: comprehensive improvements — tests, volume bars, toggles, pagination, caching, telemetry"
```

---

## Deferred (Not in This Plan)

These are valuable but require larger architectural changes. Tracked for future work:

- **WebWorker for data + indicators** — Move DataStore + IndicatorEngine to SharedWorker. Requires ArrayBuffer transfer protocol and proxy API. Defer until pattern recognition workloads actually need it.
- **Canvas2D → GPU unification** — Unify CrosshairOverlay, DrawingOverlay, and AxisCanvas into the WebGPU render pass using SDF text atlas. Major effort (~500 lines of new shader code). Defer until the extra canvases cause measurable performance issues.
