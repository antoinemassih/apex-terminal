import { invoke } from '@tauri-apps/api/core'
import { ColumnStore } from './columns'
import { TF_TO_INTERVAL } from './timeframes'
import type { TickData } from './types'
import type { IndicatorEngine, IndicatorSnapshot } from '../indicators'
import type { Bar, Timeframe } from '../types'

export class DataStore {
  private stores = new Map<string, ColumnStore>()
  private snapshots = new Map<string, IndicatorSnapshot>()
  private subscribers = new Map<string, Set<() => void>>()
  // Promise-based dedup: store the in-flight promise, not just a boolean flag
  private loadPromises = new Map<string, Promise<{ data: ColumnStore; indicators: IndicatorSnapshot }>>()

  constructor(private indicatorEngine: IndicatorEngine) {}

  private key(symbol: string, tf: string): string { return `${symbol}:${tf}` }

  async load(symbol: string, timeframe: string): Promise<{ data: ColumnStore; indicators: IndicatorSnapshot }> {
    const k = this.key(symbol, timeframe)

    // Already loaded — return cached
    if (this.stores.has(k)) {
      return { data: this.stores.get(k)!, indicators: this.snapshots.get(k)! }
    }

    // Already loading — return the same promise (no duplicate IPC calls)
    const existing = this.loadPromises.get(k)
    if (existing) return existing

    // Start loading — cache the promise immediately to prevent races
    const promise = this.doLoad(symbol, timeframe, k)
    this.loadPromises.set(k, promise)

    try {
      const result = await promise
      return result
    } finally {
      this.loadPromises.delete(k)
    }
  }

  private async doLoad(symbol: string, timeframe: string, k: string): Promise<{ data: ColumnStore; indicators: IndicatorSnapshot }> {
    const tf = TF_TO_INTERVAL[timeframe as Timeframe] ?? TF_TO_INTERVAL['5m']
    const bars: Bar[] = await invoke('get_bars', { symbol, interval: tf.interval, period: tf.period })
    const store = ColumnStore.fromBars(bars)
    this.stores.set(k, store)
    const indicators = this.indicatorEngine.bootstrap(symbol, timeframe, store)
    this.snapshots.set(k, indicators)
    this.notify(k)
    return { data: store, indicators }
  }

  applyTick(symbol: string, timeframe: string, tick: TickData): void {
    const k = this.key(symbol, timeframe)
    const store = this.stores.get(k)
    if (!store) return // tick before load — expected during startup, safe to drop

    const tf = TF_TO_INTERVAL[timeframe as Timeframe]
    if (!tf) return

    const action = store.applyTick(tick.price, tick.volume, tick.time, tf.seconds)
    try {
      const snapshot = this.indicatorEngine.onTick(symbol, timeframe, tick.price, action)
      this.snapshots.set(k, snapshot)
    } catch (e) {
      // Indicator state missing — shouldn't happen if load completed, but don't crash the feed
      console.warn(`Indicator update failed for ${k}:`, e)
    }
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
    this.loadPromises.delete(k)
    this.indicatorEngine.remove(symbol, timeframe)
  }

  private notify(k: string): void {
    const subs = this.subscribers.get(k)
    if (!subs) return
    for (const cb of subs) {
      try { cb() } catch (e) { console.error('DataStore subscriber error:', e) }
    }
  }
}
