import { invoke } from '@tauri-apps/api/core'
import { ColumnStore } from './columns'
import { TF_TO_INTERVAL } from './timeframes'
import type { TickData } from './types'
import type { IndicatorEngine, IndicatorSnapshot } from '../indicators'
import type { Bar, Timeframe } from '../types'
import type { BarCache } from './BarCache'

export class DataStore {
  private stores = new Map<string, ColumnStore>()
  private snapshots = new Map<string, IndicatorSnapshot>()
  private subscribers = new Map<string, Set<() => void>>()
  private loadPromises = new Map<string, Promise<{ data: ColumnStore; indicators: IndicatorSnapshot }>>()
  private lastActions = new Map<string, 'updated' | 'created'>()

  constructor(
    private indicatorEngine: IndicatorEngine,
    private barCache?: BarCache,
  ) {}

  private key(symbol: string, tf: string): string { return `${symbol}:${tf}` }

  async load(symbol: string, timeframe: string): Promise<{ data: ColumnStore; indicators: IndicatorSnapshot }> {
    const k = this.key(symbol, timeframe)

    if (this.stores.has(k)) {
      return { data: this.stores.get(k)!, indicators: this.snapshots.get(k)! }
    }

    const existing = this.loadPromises.get(k)
    if (existing) return existing

    const promise = this.doLoad(symbol, timeframe, k)
    this.loadPromises.set(k, promise)

    try {
      return await promise
    } finally {
      this.loadPromises.delete(k)
    }
  }

  private async doLoad(symbol: string, timeframe: string, k: string): Promise<{ data: ColumnStore; indicators: IndicatorSnapshot }> {
    const tf = TF_TO_INTERVAL[timeframe as Timeframe] ?? TF_TO_INTERVAL['5m']

    // Try cache first for instant render
    if (this.barCache) {
      const cached = await this.barCache.get(symbol, timeframe)
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
    }

    // No cache — load from API
    const bars: Bar[] = await invoke('get_bars', { symbol, interval: tf.interval, period: tf.period })
    const store = ColumnStore.fromBars(bars)
    this.stores.set(k, store)
    const indicators = this.indicatorEngine.bootstrap(symbol, timeframe, store)
    this.snapshots.set(k, indicators)
    this.notify(k)
    this.barCache?.set(symbol, timeframe, bars).catch(() => {})
    return { data: store, indicators }
  }

  private async refreshFromAPI(symbol: string, timeframe: string, k: string, tf: { interval: string; period: string; seconds: number }): Promise<void> {
    try {
      const bars: Bar[] = await invoke('get_bars', { symbol, interval: tf.interval, period: tf.period })
      if (bars.length === 0) return
      const store = ColumnStore.fromBars(bars)
      this.stores.set(k, store)
      const indicators = this.indicatorEngine.bootstrap(symbol, timeframe, store)
      this.snapshots.set(k, indicators)
      this.notify(k)
      this.barCache?.set(symbol, timeframe, bars).catch(() => {})
    } catch (e) {
      // Refresh failed — we still have cached data, so this is non-fatal
      console.warn(`Background refresh failed for ${symbol}:${timeframe}:`, e)
    }
  }

  applyTick(symbol: string, timeframe: string, tick: TickData): void {
    const k = this.key(symbol, timeframe)
    const store = this.stores.get(k)
    if (!store) return

    const tf = TF_TO_INTERVAL[timeframe as Timeframe]
    if (!tf) return

    const action = store.applyTick(tick.price, tick.volume, tick.time, tf.seconds)
    this.lastActions.set(k, action)
    try {
      const snapshot = this.indicatorEngine.onTick(symbol, timeframe, tick.price, action)
      this.snapshots.set(k, snapshot)
    } catch (e) {
      console.warn(`Indicator update failed for ${k}:`, e)
    }
    this.notify(k)
  }

  /** Load more historical data by prepending older bars */
  async loadMore(symbol: string, timeframe: string): Promise<number> {
    const k = this.key(symbol, timeframe)
    const store = this.stores.get(k)
    if (!store || store.length === 0) return 0

    const tf = TF_TO_INTERVAL[timeframe as Timeframe] ?? TF_TO_INTERVAL['5m']
    const oldestTime = store.times[0]

    try {
      const bars: Bar[] = await invoke('get_bars', {
        symbol, interval: tf.interval, period: tf.period
      })
      const newBars = bars.filter(b => b.time < oldestTime)
      if (newBars.length === 0) return 0
      const added = store.prepend(newBars)
      if (added > 0) {
        const indicators = this.indicatorEngine.bootstrap(symbol, timeframe, store)
        this.snapshots.set(k, indicators)
        this.notify(k)
      }
      return added
    } catch (e) {
      console.warn(`Failed to load more ${k}:`, e)
      return 0
    }
  }

  getData(symbol: string, timeframe: string): ColumnStore | null {
    return this.stores.get(this.key(symbol, timeframe)) ?? null
  }

  getIndicators(symbol: string, timeframe: string): IndicatorSnapshot | null {
    return this.snapshots.get(this.key(symbol, timeframe)) ?? null
  }

  getLastAction(symbol: string, timeframe: string): 'updated' | 'created' | null {
    return this.lastActions.get(this.key(symbol, timeframe)) ?? null
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
    this.lastActions.delete(k)
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
