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
  private loading = new Set<string>()

  constructor(private indicatorEngine: IndicatorEngine) {}

  private key(symbol: string, tf: string): string { return `${symbol}:${tf}` }

  async load(symbol: string, timeframe: string): Promise<{ data: ColumnStore; indicators: IndicatorSnapshot }> {
    const k = this.key(symbol, timeframe)
    if (this.stores.has(k)) {
      return { data: this.stores.get(k)!, indicators: this.snapshots.get(k)! }
    }
    if (this.loading.has(k)) {
      return new Promise(resolve => {
        const unsub = this.subscribe(symbol, timeframe, () => {
          if (this.stores.has(k)) {
            unsub()
            resolve({ data: this.stores.get(k)!, indicators: this.snapshots.get(k)! })
          }
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
    if (!store) return

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
