import { IncrementalSMA } from './incremental/sma'
import { IncrementalEMA } from './incremental/ema'
import { IncrementalBollinger } from './incremental/bollinger'
import type { IndicatorSnapshot } from './types'

// Import ColumnStore type only
interface ColumnStoreLike {
  closes: Float64Array
  length: number
}

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

  bootstrap(symbol: string, timeframe: string, data: ColumnStoreLike): IndicatorSnapshot {
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
    if (!state) throw new Error(`No indicator state for ${k}`)

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
