/**
 * IBKRProvider — Interactive Brokers data provider for Apex Terminal.
 *
 * ┌─ History ──────────────────────────────────────────────────────────┐
 * │  Stocks/ETFs/Indices:  yfinance (localhost:8777) → OCOCO cache     │
 * │  Options:              no history until Polygon is added           │
 * └────────────────────────────────────────────────────────────────────┘
 * ┌─ Real-time ─────────────────────────────────────────────────────────┐
 * │  In Tauri:   ibserver → Rust ib_ws → msgpack → app.emit("ib-tick") │
 * │  In browser: ibserver WebSocket → JSON  (dev fallback)             │
 * └────────────────────────────────────────────────────────────────────┘
 *
 * ibserver:  localhost:5000 — real-time ticks, options chains, contract resolution
 * yfinance:  localhost:8777 — historical bars for stocks/indices
 */

import type { Bar } from '../types'
import type { TickData } from './types'
import type { DataProvider, HistoryRequest, HistoryResponse } from './DataProvider'

const IBSERVER = 'http://localhost:5000'
const OCOCO = 'http://192.168.1.60:30300'
const YFINANCE = 'http://localhost:8777'

/** True when running inside the Tauri desktop shell */
const IS_TAURI = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window

const TF_SECONDS: Record<string, number> = {
  '1m': 60, '2m': 120, '5m': 300, '15m': 900, '30m': 1800,
  '1H': 3600, '4H': 14400, '1D': 86400, '1W': 604800,
}

/** Map Apex timeframe strings to yfinance interval + period */
const TF_TO_YFINANCE: Record<string, { interval: string; period: string }> = {
  '1m': { interval: '1m', period: '5d' },
  '2m': { interval: '2m', period: '5d' },
  '5m': { interval: '5m', period: '5d' },
  '15m': { interval: '15m', period: '60d' },
  '30m': { interval: '30m', period: '60d' },
  '1H': { interval: '60m', period: '60d' },
  '4H': { interval: '1h', period: '730d' },
  '1D': { interval: '1d', period: '5y' },
  '1W': { interval: '1wk', period: '10y' },
}

interface IBContractInfo {
  conId: number
  symbol: string
  secType: string
  exchange: string
  currency: string
  localSymbol: string
  description: string
}

export class IBKRProvider implements DataProvider {
  readonly name = 'ibkr'

  // ── WebSocket fallback (browser / dev mode) ───────────────────────────────
  private ws: WebSocket | null = null
  private wsRetryTimer: ReturnType<typeof setTimeout> | null = null

  // ── Tauri path ────────────────────────────────────────────────────────────
  /** Cleanup function returned by Tauri listen() calls */
  private tauriUnlisten: (() => void) | null = null

  // ── Shared state ─────────────────────────────────────────────────────────
  private wsReady = false
  private connected = false

  private tickCb: ((symbol: string, tf: string, tick: TickData) => void) | null = null

  /** symbol → set of subscribed timeframes */
  private subscriptions = new Map<string, Set<string>>()
  /** bidirectional conId ↔ symbol for WS message routing */
  private symbolToConId = new Map<string, number>()
  private conIdToSymbol = new Map<number, string>()

  private disconnectCbs = new Set<() => void>()
  private reconnectCbs = new Set<() => void>()

  // ── Simulation fallback (used when ibserver is not connected OR idle) ───────
  private simIntervalId: number | null = null
  private simPrices     = new Map<string, number>()  // key → last price
  private simTimes      = new Map<string, number>()  // key → sim wall-clock seconds
  private simTickCounts = new Map<string, number>()
  /** Timestamp of last real IB quote — simulation yields to real data when recent */
  private lastRealTickMs = 0

  /** Seed the simulation from the last loaded bar — called by ChartPane after load */
  setLastPrice(symbol: string, timeframe: string, price: number, time: number): void {
    const key = `${symbol}:${timeframe}`
    this.simPrices.set(key, price)
    this.simTimes.set(key, time)
    this.simTickCounts.set(key, 0)
    // Always start simulation as a fallback — simTick() yields when real IB data flows
    if (this.simIntervalId === null) this.startSimulation()
  }

  private startSimulation(): void {
    if (this.simIntervalId !== null) return
    this.simIntervalId = window.setInterval(() => this.simTick(), 50)
  }

  private stopSimulation(): void {
    if (this.simIntervalId !== null) { clearInterval(this.simIntervalId); this.simIntervalId = null }
  }

  private _simLogCount = 0
  private simTick(): void {
    if (!this.tickCb) return
    // Yield to real IB data: suppress simulation when live quotes arrived in last 3 seconds
    if (this.wsReady && this.lastRealTickMs > 0 && Date.now() - this.lastRealTickMs < 3000) {
      if (this._simLogCount++ < 3) console.info('[IBKRProvider] simTick yielding to live IB data')
      return
    }
    for (const [symbol, tfs] of this.subscriptions) {
      for (const tf of tfs) {
        const key = `${symbol}:${tf}`
        const lastPrice = this.simPrices.get(key)
        if (lastPrice === undefined) continue
        const tfDef = TF_TO_YFINANCE[tf]
        if (!tfDef) continue
        const tfSeconds = TF_SECONDS[tf] ?? 300

        const change = lastPrice * (Math.random() - 0.495) * 0.003
        const price  = Math.max(0.01, lastPrice + change)
        const volume = Math.random() * 500

        const count   = (this.simTickCounts.get(key) ?? 0) + 1
        this.simTickCounts.set(key, count)
        const prevTime = this.simTimes.get(key) ?? (Date.now() / 1000)
        const time = count % 20 === 0 ? prevTime + tfSeconds : prevTime + tfSeconds / 20
        this.simTimes.set(key, time)
        this.simPrices.set(key, price)
        this.tickCb(symbol, tf, { price, volume, time })
      }
    }
  }

  // ── History ─────────────────────────────────────────────────────────────────

  async getHistory(req: HistoryRequest): Promise<HistoryResponse> {
    // Options (numeric conId) — no historical data source until Polygon is added
    if (/^\d+$/.test(req.symbol)) return { bars: [], hasMore: false }

    // 1. OCOCO (InfluxDB cache) — sub-ms for already-seen symbols
    try {
      const p = new URLSearchParams({
        symbol: req.symbol,
        interval: req.timeframe,
        limit: String(req.limit ?? 500),
      })
      if (req.before) p.set('before', String(req.before))

      const ctrl = new AbortController()
      const t = setTimeout(() => ctrl.abort(), 1000)
      const res = await fetch(`${OCOCO}/api/bars?${p}`, { signal: ctrl.signal })
      clearTimeout(t)

      if (res.ok) {
        const bars: Bar[] = await res.json()
        if (bars.length > 0) return { bars, hasMore: bars.length >= (req.limit ?? 500) }
      }
    } catch { /* OCOCO unreachable — fall through */ }

    // 2. yfinance — stocks/ETFs/indices, internet-sourced, no IB required
    // (pagination not supported — yfinance only serves recent data per period)
    if (!req.before) {
      try {
        const yf = TF_TO_YFINANCE[req.timeframe]
        if (yf) {
          const p = new URLSearchParams({ symbol: req.symbol, interval: yf.interval, period: yf.period })
          const ctrl = new AbortController()
          const t = setTimeout(() => ctrl.abort(), 5000)
          const res = await fetch(`${YFINANCE}/bars?${p}`, { signal: ctrl.signal })
          clearTimeout(t)
          if (res.ok) {
            const bars: Bar[] = await res.json()
            if (bars.length > 0) return { bars, hasMore: false }
          }
        }
      } catch { /* yfinance not running */ }
    }

    // TODO: Polygon.io — options history + deeper stock history when added

    return { bars: [], hasMore: false }
  }

  // ── Subscription ─────────────────────────────────────────────────────────────

  subscribe(symbol: string, timeframe: string): void {
    if (!this.subscriptions.has(symbol)) this.subscriptions.set(symbol, new Set())
    this.subscriptions.get(symbol)!.add(timeframe)
    // Resolve and subscribe to IB stream only once per symbol
    if (!this.symbolToConId.has(symbol)) void this.resolveAndSubscribe(symbol)
  }

  unsubscribe(symbol: string, timeframe: string): void {
    const tfs = this.subscriptions.get(symbol)
    if (!tfs) return
    tfs.delete(timeframe)

    // Clean up per-timeframe simulation state immediately
    const key = `${symbol}:${timeframe}`
    this.simPrices.delete(key)
    this.simTimes.delete(key)
    this.simTickCounts.delete(key)

    if (tfs.size > 0) return

    // Last timeframe unsubscribed — cancel the IB stream
    this.subscriptions.delete(symbol)
    const conId = this.symbolToConId.get(symbol)
    if (conId !== undefined) {
      this.wsSend({ action: 'unsubscribe', conIds: [conId] })
      this.symbolToConId.delete(symbol)
      this.conIdToSymbol.delete(conId)
    }
  }

  onTick(cb: (symbol: string, tf: string, tick: TickData) => void): () => void {
    this.tickCb = cb
    return () => { this.tickCb = null }
  }

  // ── Lifecycle ────────────────────────────────────────────────────────────────

  async connect(): Promise<void> {
    this.connected = true
    if (IS_TAURI) {
      this.connectTauri()
    } else {
      this.openWs()
    }
  }

  disconnect(): void {
    this.connected = false
    // Tauri path cleanup
    this.tauriUnlisten?.()
    this.tauriUnlisten = null
    // WebSocket path cleanup
    if (this.wsRetryTimer) { clearTimeout(this.wsRetryTimer); this.wsRetryTimer = null }
    this.ws?.close()
    this.ws = null
    this.wsReady = false
    this.stopSimulation()
  }

  onDisconnect(cb: () => void): () => void {
    this.disconnectCbs.add(cb)
    return () => this.disconnectCbs.delete(cb)
  }

  onReconnect(cb: () => void): () => void {
    this.reconnectCbs.add(cb)
    return () => this.reconnectCbs.delete(cb)
  }

  // ── Tauri IPC path ────────────────────────────────────────────────────────

  private connectTauri(): void {
    import('@tauri-apps/api/event').then(({ listen }) => {
      Promise.all([
        // Hot path: tick data decoded by Rust, emitted as Tauri event
        listen<Record<string, unknown>>('ib-tick', ({ payload }) => {
          try { this.onWsMsg(payload) } catch { /* */ }
        }),
        listen<void>('ib-connected', () => {
          this.wsReady = true
          const conIds = [...this.symbolToConId.values()]
          if (conIds.length) this.wsSend({ action: 'subscribe', conIds })
          this.reconnectCbs.forEach(cb => { try { cb() } catch { /* */ } })
        }),
        listen<void>('ib-disconnected', () => {
          this.wsReady = false
          if (this.connected && this.simPrices.size > 0) this.startSimulation()
          this.disconnectCbs.forEach(cb => { try { cb() } catch { /* */ } })
        }),
      ]).then(([u1, u2, u3]) => {
        this.tauriUnlisten = () => { u1(); u2(); u3() }
        // Rust WS task starts at app launch — treat as ready until proven otherwise
        this.wsReady = true
      }).catch(e => {
        console.warn('[IBKRProvider] Tauri listen failed, falling back to WebSocket', e)
        this.openWs()
      })
    }).catch(() => this.openWs())
  }

  // ── WebSocket fallback (browser dev mode) ────────────────────────────────

  private openWs(): void {
    const url = IBSERVER.replace(/^http/, 'ws') + '/ws'
    try {
      this.ws = new WebSocket(url)
    } catch {
      this.scheduleWsRetry()
      return
    }

    this.ws.onopen = () => {
      console.info('[IBKRProvider] WebSocket connected to ibserver — simulation yields to live data when it flows')
      this.wsReady = true
      const conIds = [...this.symbolToConId.values()]
      if (conIds.length) this.wsSend({ action: 'subscribe', conIds })
      this.reconnectCbs.forEach(cb => { try { cb() } catch { /* */ } })
    }

    this.ws.onmessage = async ev => {
      try {
        const { decode } = await import('@msgpack/msgpack')
        const raw = ev.data instanceof Blob ? await ev.data.arrayBuffer() : ev.data as ArrayBuffer
        this.onWsMsg(decode(new Uint8Array(raw)) as Record<string, unknown>)
      } catch { /* */ }
    }

    this.ws.onclose = () => {
      console.info(`[IBKRProvider] WebSocket closed — wsReady=false simPrices=${this.simPrices.size}`)
      this.wsReady = false
      if (this.connected && this.simPrices.size > 0) this.startSimulation()
      this.disconnectCbs.forEach(cb => { try { cb() } catch { /* */ } })
      if (this.connected) this.scheduleWsRetry()
    }

    this.ws.onerror = () => this.ws?.close()
  }

  private scheduleWsRetry(): void {
    this.wsRetryTimer = setTimeout(() => {
      this.wsRetryTimer = null
      this.openWs()
    }, 3000)
  }

  // ── Shared message handling ───────────────────────────────────────────────

  private onWsMsg(msg: Record<string, unknown>): void {
    if (!this.tickCb) return
    if (msg.type === 'quote_batch') {
      const now = msg.t ? Math.floor(Number(msg.t) / 1000) : Math.floor(Date.now() / 1000)
      for (const q of (msg.quotes as Record<string, unknown>[])) this.dispatchQuote(q, now)
    } else if (msg.type === 'quote') {
      this.dispatchQuote(msg, Math.floor(Date.now() / 1000))
    }
  }

  private dispatchQuote(q: Record<string, unknown>, batchTime: number): void {
    this.lastRealTickMs = Date.now()  // real IB data flowing — simulation will yield
    const symbol = this.conIdToSymbol.get(q.conId as number)
    if (!symbol) return

    const tfs = this.subscriptions.get(symbol)
    if (!tfs?.size) return

    const last = Number(q.last)
    const bid  = Number(q.bid)
    const ask  = Number(q.ask)
    const price = last > 0 ? last : (bid > 0 && ask > 0 ? (bid + ask) / 2 : 0)
    if (!(price > 0)) return

    const tick: TickData = {
      price,
      volume: Number(q.volume) || 0,
      time: batchTime,
    }
    for (const tf of tfs) this.tickCb!(symbol, tf, tick)
  }

  private async resolveAndSubscribe(symbol: string): Promise<void> {
    try {
      const res = await fetch(`${IBSERVER}/contract/${encodeURIComponent(symbol)}`)
      if (!res.ok) return
      const info = await res.json() as IBContractInfo
      this.symbolToConId.set(symbol, info.conId)
      this.conIdToSymbol.set(info.conId, symbol)
      if (this.wsReady) this.wsSend({ action: 'subscribe', conIds: [info.conId] })
    } catch (e) {
      console.warn('[IBKRProvider] resolve failed:', symbol, e)
    }
  }

  /** Route WS control messages — Tauri invoke or WebSocket send */
  private wsSend(msg: object): void {
    if (IS_TAURI) {
      import('@tauri-apps/api/core').then(({ invoke }) =>
        invoke('ib_ws_send', { msg }).catch(() => { /* ibserver down */ })
      )
    } else if (this.ws?.readyState === WebSocket.OPEN) {
      this.ws.send(JSON.stringify(msg))
    }
  }
}
