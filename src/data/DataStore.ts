import { ColumnStore } from './columns'
import { TF_TO_INTERVAL } from './timeframes'
import type { TickData } from './types'
import type { IndicatorEngine, IndicatorSnapshot } from '../indicators'
import type { Timeframe } from '../types'
import type { BarCache } from './BarCache'
import type { DataProvider } from './DataProvider'

/** Max number of symbol:timeframe pairs to keep in memory. Oldest unused are evicted. */
const MAX_CACHED_PAIRS = 10

export class DataStore {
  private stores = new Map<string, ColumnStore>()
  private snapshots = new Map<string, IndicatorSnapshot>()
  private subscribers = new Map<string, Set<() => void>>()
  private loadPromises = new Map<string, Promise<{ data: ColumnStore; indicators: IndicatorSnapshot }>>()
  private lastActions = new Map<string, 'updated' | 'created'>()
  private paginationState = new Map<string, { loading: boolean; hasMore: boolean }>()
  /** Track last access time for LRU eviction */
  private lastAccess = new Map<string, number>()

  // Performance metrics
  private metrics = {
    loadCount: 0,
    loadTotalMs: 0,
    loadPeakMs: 0,
    paginationCount: 0,
    tickCount: 0,
    tickTotalMs: 0,
    cacheHits: 0,
    cacheMisses: 0,
  }

  constructor(
    private indicatorEngine: IndicatorEngine,
    private provider: DataProvider,
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
    const t0 = performance.now()

    // Try cache first
    if (this.barCache) {
      const cached = await this.barCache.get(symbol, timeframe)
      if (cached && cached.length > 0) {
        this.metrics.cacheHits++
        const store = ColumnStore.fromBars(cached)
        this.stores.set(k, store)
        const indicators = this.indicatorEngine.bootstrap(symbol, timeframe, store)
        this.snapshots.set(k, indicators)
        this.paginationState.set(k, { loading: false, hasMore: true })
        this.recordLoadMetric(t0)
        this.lastActions.delete(k)  // force full gpuBars.load()
        this.lastAccess.set(k, Date.now())
        this.notify(k)
        this.evictOldEntries()
        // Background refresh
        this.refreshFromProvider(symbol, timeframe as Timeframe, k).catch(() => {})
        return { data: store, indicators }
      }
      this.metrics.cacheMisses++
    }

    // Load from provider
    const resp = await this.provider.getHistory({ symbol, timeframe: timeframe as Timeframe })
    const store = ColumnStore.fromBars(resp.bars)
    this.stores.set(k, store)
    const indicators = this.indicatorEngine.bootstrap(symbol, timeframe, store)
    this.snapshots.set(k, indicators)
    this.paginationState.set(k, { loading: false, hasMore: resp.hasMore || resp.bars.length > 50 })
    this.recordLoadMetric(t0)
    this.lastActions.delete(k)  // force full gpuBars.load() — not incremental append
    this.lastAccess.set(k, Date.now())
    this.notify(k)
    this.evictOldEntries()
    this.barCache?.set(symbol, timeframe, resp.bars).catch(() => {})
    return { data: store, indicators }
  }

  private async refreshFromProvider(symbol: string, timeframe: Timeframe, k: string): Promise<void> {
    try {
      const resp = await this.provider.getHistory({ symbol, timeframe })
      if (resp.bars.length === 0) return
      const store = ColumnStore.fromBars(resp.bars)
      this.stores.set(k, store)
      const indicators = this.indicatorEngine.bootstrap(symbol, timeframe, store)
      this.snapshots.set(k, indicators)
      this.paginationState.set(k, { loading: false, hasMore: resp.hasMore || resp.bars.length > 50 })
      this.lastActions.delete(k)  // force full gpuBars.load()
      this.notify(k)
      this.barCache?.set(symbol, timeframe as string, resp.bars).catch(() => {})
    } catch (e) {
      console.warn(`Background refresh failed for ${symbol}:${timeframe}:`, e)
    }
  }

  applyTick(symbol: string, timeframe: string, tick: TickData): void {
    const t0 = performance.now()
    const k = this.key(symbol, timeframe)
    let store = this.stores.get(k)
    if (!store) {
      // Lazily create an empty store so early ticks (before load completes) are not dropped.
      // doLoad() will overwrite this store when history arrives.
      store = ColumnStore.fromBars([])
      this.stores.set(k, store)
      const indicators = this.indicatorEngine.bootstrap(symbol, timeframe, store)
      this.snapshots.set(k, indicators)
    }

    const tf = TF_TO_INTERVAL[timeframe as Timeframe]
    if (!tf) return

    const action = store.applyTick(tick.price, tick.volume, tick.time, tf.seconds)
    this.lastActions.set(k, action)

    // If ColumnStore evicted old bars, keep indicators in sync
    if (store.lastEvictCount > 0) {
      this.indicatorEngine.evict(symbol, timeframe, store.lastEvictCount)
    }

    try {
      const snapshot = this.indicatorEngine.onTick(symbol, timeframe, tick.price, action)
      this.snapshots.set(k, snapshot)
    } catch (e) {
      console.warn(`Indicator update failed for ${k}:`, e)
    }
    this.metrics.tickCount++
    this.metrics.tickTotalMs += performance.now() - t0
    this.notify(k)
  }

  /** Load older history when user scrolls left. Returns bars added. */
  async loadMore(symbol: string, timeframe: string): Promise<number> {
    const k = this.key(symbol, timeframe)
    const store = this.stores.get(k)
    if (!store || store.length === 0) return 0

    const state = this.paginationState.get(k)
    if (state?.loading) return 0 // already fetching
    if (state && !state.hasMore) return 0 // no more history

    this.paginationState.set(k, { loading: true, hasMore: state?.hasMore ?? true })

    const t0 = performance.now()
    const oldestTime = store.times[0]

    try {
      const resp = await this.provider.getHistory({
        symbol,
        timeframe: timeframe as Timeframe,
        before: oldestTime,
        limit: 500,
      })

      const newBars = resp.bars.filter(b => b.time < oldestTime)
      if (newBars.length === 0) {
        this.paginationState.set(k, { loading: false, hasMore: false })
        return 0
      }

      const added = store.prepend(newBars)
      if (added > 0) {
        const indicators = this.indicatorEngine.bootstrap(symbol, timeframe, store)
        this.snapshots.set(k, indicators)
        this.metrics.paginationCount++
        this.recordLoadMetric(t0)
        this.lastActions.delete(k)  // prepend shifts all bars — force full gpuBars.load()
        this.notify(k)
      }

      this.paginationState.set(k, { loading: false, hasMore: resp.hasMore && newBars.length > 0 })
      return added
    } catch (e) {
      console.warn(`Failed to load more ${k}:`, e)
      this.paginationState.set(k, { loading: false, hasMore: state?.hasMore ?? false })
      return 0
    }
  }

  /** Check if more history is available for a symbol+timeframe */
  canLoadMore(symbol: string, timeframe: string): boolean {
    const state = this.paginationState.get(this.key(symbol, timeframe))
    return state ? state.hasMore && !state.loading : false
  }

  /** Check if currently loading more history */
  isLoadingMore(symbol: string, timeframe: string): boolean {
    return this.paginationState.get(this.key(symbol, timeframe))?.loading ?? false
  }

  getData(symbol: string, timeframe: string): ColumnStore | null {
    const k = this.key(symbol, timeframe)
    if (this.stores.has(k)) this.lastAccess.set(k, Date.now())
    return this.stores.get(k) ?? null
  }

  getIndicators(symbol: string, timeframe: string): IndicatorSnapshot | null {
    return this.snapshots.get(this.key(symbol, timeframe)) ?? null
  }

  getLastAction(symbol: string, timeframe: string): 'updated' | 'created' | null {
    return this.lastActions.get(this.key(symbol, timeframe)) ?? null
  }

  getMetrics() {
    return {
      ...this.metrics,
      avgLoadMs: this.metrics.loadCount > 0 ? Math.round(this.metrics.loadTotalMs / this.metrics.loadCount * 10) / 10 : 0,
      avgTickMs: this.metrics.tickCount > 0 ? Math.round(this.metrics.tickTotalMs / this.metrics.tickCount * 1000) / 1000 : 0,
    }
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
    this.paginationState.delete(k)
    this.lastAccess.delete(k)
    this.indicatorEngine.remove(symbol, timeframe)
  }

  /** Evict least-recently-used entries when cache exceeds MAX_CACHED_PAIRS.
   *  Entries with active subscribers are never evicted. */
  private evictOldEntries(): void {
    if (this.stores.size <= MAX_CACHED_PAIRS) return
    // Sort keys by last access time, oldest first
    const entries = [...this.lastAccess.entries()]
      .filter(([k]) => {
        // Don't evict entries with active subscribers
        const subs = this.subscribers.get(k)
        return !subs || subs.size === 0
      })
      .sort((a, b) => a[1] - b[1])
    // Evict oldest until we're at the limit
    let toEvict = this.stores.size - MAX_CACHED_PAIRS
    for (const [k] of entries) {
      if (toEvict <= 0) break
      const [sym, tf] = k.split(':')
      this.unload(sym, tf)
      toEvict--
    }
  }

  /** Aggressively evict ALL entries without active subscribers. Called under memory pressure. */
  evictAll(): void {
    const keys = [...this.stores.keys()]
    let evicted = 0
    for (const k of keys) {
      const subs = this.subscribers.get(k)
      if (!subs || subs.size === 0) {
        const [sym, tf] = k.split(':')
        this.unload(sym, tf)
        evicted++
      }
    }
    if (evicted > 0) console.info(`[DataStore] Memory pressure: evicted ${evicted} inactive entries (${this.stores.size} remaining)`)
  }

  private notify(k: string): void {
    const subs = this.subscribers.get(k)
    if (!subs) return
    for (const cb of subs) {
      try { cb() } catch (e) { console.error('DataStore subscriber error:', e) }
    }
  }

  private recordLoadMetric(t0: number): void {
    const elapsed = performance.now() - t0
    this.metrics.loadCount++
    this.metrics.loadTotalMs += elapsed
    if (elapsed > this.metrics.loadPeakMs) this.metrics.loadPeakMs = elapsed
  }
}
