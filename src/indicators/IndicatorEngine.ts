import { INDICATOR_CATALOG } from './registry'
import type { IncrementalIndicator, IndicatorSnapshot, IndicatorOutput } from './types'

interface SymbolState {
  indicators: Map<string, IncrementalIndicator>  // id -> indicator
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
    this.subscribers.get(k)?.forEach(cb => { try { cb(snapshot) } catch (_e) { /* swallow subscriber errors */ } })
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

  evict(symbol: string, timeframe: string, count: number): void {
    const state = this.state.get(this.key(symbol, timeframe))
    if (!state) return
    for (const ind of state.indicators.values()) {
      ind.evict(count)
    }
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
