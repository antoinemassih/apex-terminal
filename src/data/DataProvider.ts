/**
 * Data Provider abstraction — decouples the app from any specific data source.
 *
 * A DataProvider has two responsibilities:
 * 1. Historical bars (request/response)
 * 2. Real-time ticks (streaming)
 *
 * Implementations:
 * - YFinanceProvider: current yfinance sidecar + SimulatedFeed (default)
 * - InhouseProvider: your own history server + Polygon WebSocket (future)
 * - ReplayProvider: replay historical data as if live (for backtesting)
 *
 * The active provider is set at bootstrap. Everything else (DataStore, ChartPane,
 * IndicatorEngine) is provider-agnostic — they only see bars and ticks.
 */

import type { Bar, Timeframe } from '../types'
import type { TickData } from './types'

// ---------------------------------------------------------------------------
// Interfaces
// ---------------------------------------------------------------------------

export interface HistoryRequest {
  symbol: string
  timeframe: Timeframe
  /** If set, fetch bars BEFORE this timestamp (for pagination) */
  before?: number
  /** Max bars to return */
  limit?: number
}

export interface HistoryResponse {
  bars: Bar[]
  /** True if the server has more history available before these bars */
  hasMore: boolean
}

export interface DataProvider {
  readonly name: string

  /** Fetch historical bars */
  getHistory(req: HistoryRequest): Promise<HistoryResponse>

  /** Subscribe to real-time ticks for a symbol+timeframe */
  subscribe(symbol: string, timeframe: string): void

  /** Unsubscribe */
  unsubscribe(symbol: string, timeframe: string): void

  /** Register tick callback */
  onTick(cb: (symbol: string, timeframe: string, tick: TickData) => void): () => void

  /** Connect to the real-time feed */
  connect(): Promise<void>

  /** Disconnect */
  disconnect(): void

  /** Lifecycle events */
  onDisconnect(cb: () => void): () => void
  onReconnect(cb: () => void): () => void
}

// ---------------------------------------------------------------------------
// YFinance Provider (current default)
// ---------------------------------------------------------------------------

import { invoke } from '@tauri-apps/api/core'
import { TF_TO_INTERVAL } from './timeframes'

const OCOCO_API = 'http://192.168.1.60:30300'

export class YFinanceProvider implements DataProvider {
  readonly name = 'yfinance + influx'

  private subscriptions = new Map<string, { symbol: string; timeframe: string; simTime: number; tickCount: number }>()
  private tickCb: ((symbol: string, tf: string, tick: TickData) => void) | null = null
  private disconnectListeners = new Set<() => void>()
  private reconnectListeners = new Set<() => void>()
  private lastPrices = new Map<string, number>()
  private intervalId: number | null = null
  private connected = false

  async getHistory(req: HistoryRequest): Promise<HistoryResponse> {
    const tf = TF_TO_INTERVAL[req.timeframe] ?? TF_TO_INTERVAL['5m']

    // Try InfluxDB (via OCOCO API) first — fast, server-side, deep history
    try {
      const start = req.before ? `-10y` : this.periodToFluxRange(tf.period)
      const url = `${OCOCO_API}/api/bars?symbol=${req.symbol}&interval=${req.timeframe}&start=${start}`
      const resp = await fetch(url)
      if (resp.ok) {
        let bars: Bar[] = await resp.json()
        if (bars.length > 10) {
          if (req.before) bars = bars.filter(b => b.time < req.before!)
          if (req.limit) bars = bars.slice(-req.limit)
          return { bars, hasMore: bars.length > 0 }
        }
      }
    } catch {
      // InfluxDB/OCOCO API not reachable — fall through to yfinance
    }

    // Fallback: yfinance sidecar (local) — also backfill InfluxDB
    let period = tf.period
    if (req.before) period = this.expandPeriod(tf.period)

    try {
      const bars: Bar[] = await invoke('get_bars', {
        symbol: req.symbol, interval: tf.interval, period
      })

      // Backfill InfluxDB so next request is served from there
      if (bars.length > 0) {
        this.backfillInflux(req.symbol, req.timeframe, bars)
      }

      let filtered = bars
      if (req.before) filtered = bars.filter(b => b.time < req.before!)
      if (req.limit) filtered = filtered.slice(-req.limit)

      return {
        bars: filtered,
        hasMore: filtered.length > 0 && filtered.length === (req.limit ?? filtered.length),
      }
    } catch (e) {
      console.error(`getHistory failed:`, e)
      return { bars: [], hasMore: false }
    }
  }

  /** Fire-and-forget: push bars to InfluxDB via OCOCO API ingestion endpoint */
  private backfillInflux(symbol: string, _timeframe: string, _bars: Bar[]): void {
    fetch(`${OCOCO_API}/api/ingest/symbol?symbol=${symbol}`, { method: 'POST' })
      .catch(() => { /* non-blocking, don't care if it fails */ })
  }

  private periodToFluxRange(period: string): string {
    const map: Record<string, string> = {
      '1d': '-1d', '5d': '-5d', '1mo': '-30d', '3mo': '-90d',
      '6mo': '-180d', '1y': '-365d', '2y': '-730d', '5y': '-1825d', '10y': '-3650d',
    }
    return map[period] ?? '-365d'
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

  async connect(): Promise<void> {
    if (this.connected) return
    this.connected = true
    this.intervalId = window.setInterval(() => this.tick(), 50)
  }

  disconnect(): void {
    this.connected = false
    if (this.intervalId !== null) { clearInterval(this.intervalId); this.intervalId = null }
    for (const cb of this.disconnectListeners) { try { cb() } catch { /* */ } }
  }

  onDisconnect(cb: () => void): () => void {
    this.disconnectListeners.add(cb)
    return () => { this.disconnectListeners.delete(cb) }
  }

  onReconnect(cb: () => void): () => void {
    this.reconnectListeners.add(cb)
    return () => { this.reconnectListeners.delete(cb) }
  }

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

  /** Expand period for deeper pagination */
  private expandPeriod(period: string): string {
    const map: Record<string, string> = {
      '1d': '5d', '5d': '1mo', '1mo': '3mo', '3mo': '6mo',
      '6mo': '1y', '1y': '2y', '2y': '5y', '5y': '10y', '10y': 'max',
    }
    return map[period] ?? 'max'
  }
}

// ---------------------------------------------------------------------------
// Stub for future Inhouse Provider
// ---------------------------------------------------------------------------

/**
 * Future: InhouseProvider
 * - History from your own server (REST API to your bar storage)
 * - Realtime from Polygon.io WebSocket
 *
 * Implement DataProvider interface:
 *   getHistory() → fetch from your history server with pagination
 *   subscribe() → add to Polygon WS subscription list
 *   onTick() → forward Polygon WS messages as TickData
 */
