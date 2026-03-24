/**
 * IBKRProvider — Interactive Brokers data provider for Apex Terminal.
 *
 * History flow:  OCOCO (InfluxDB cache, ~1 ms) → ibserver /bars (IB + InfluxDB backfill)
 * Realtime flow: ibserver WebSocket → conId-keyed quote events → TickData
 *
 * ibserver must be running on localhost:5000 (same machine as TWS/IB Gateway).
 */

import type { Bar } from '../types'
import type { TickData } from './types'
import type { DataProvider, HistoryRequest, HistoryResponse } from './DataProvider'

const IBSERVER = 'http://localhost:5000'
const OCOCO = 'http://192.168.1.60:30300'

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

  private ws: WebSocket | null = null
  private wsReady = false
  private wsRetryTimer: ReturnType<typeof setTimeout> | null = null
  private connected = false

  private tickCb: ((symbol: string, tf: string, tick: TickData) => void) | null = null

  /** symbol → set of subscribed timeframes */
  private subscriptions = new Map<string, Set<string>>()
  /** bidirectional conId ↔ symbol for WS message routing */
  private symbolToConId = new Map<string, number>()
  private conIdToSymbol = new Map<number, string>()

  private disconnectCbs = new Set<() => void>()
  private reconnectCbs = new Set<() => void>()

  // ── History ─────────────────────────────────────────────────────────────────

  async getHistory(req: HistoryRequest): Promise<HistoryResponse> {
    // 1. OCOCO (InfluxDB) — fast, deep history
    try {
      const p = new URLSearchParams({
        symbol: req.symbol,
        interval: req.timeframe,
        limit: String(req.limit ?? 500),
      })
      if (req.before) p.set('before', String(req.before))

      const ctrl = new AbortController()
      const t = setTimeout(() => ctrl.abort(), 3000)
      const res = await fetch(`${OCOCO}/api/bars?${p}`, { signal: ctrl.signal })
      clearTimeout(t)

      if (res.ok) {
        const bars: Bar[] = await res.json()
        if (bars.length > 0) return { bars, hasMore: bars.length >= (req.limit ?? 500) }
      }
    } catch { /* OCOCO unreachable or no data — fall through */ }

    // 2. ibserver /bars — fetches from IB and backfills InfluxDB
    const p = new URLSearchParams({
      timeframe: req.timeframe,
      limit: String(req.limit ?? 500),
    })
    if (req.before) p.set('before', String(req.before))

    const res = await fetch(`${IBSERVER}/bars/${encodeURIComponent(req.symbol)}?${p}`)
    if (!res.ok) throw new Error(`ibserver /bars returned ${res.status}`)
    const data = await res.json()
    return { bars: data.bars ?? [], hasMore: data.hasMore ?? false }
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
    this.openWs()
  }

  disconnect(): void {
    this.connected = false
    if (this.wsRetryTimer) { clearTimeout(this.wsRetryTimer); this.wsRetryTimer = null }
    this.ws?.close()
    this.ws = null
    this.wsReady = false
  }

  onDisconnect(cb: () => void): () => void {
    this.disconnectCbs.add(cb)
    return () => this.disconnectCbs.delete(cb)
  }

  onReconnect(cb: () => void): () => void {
    this.reconnectCbs.add(cb)
    return () => this.reconnectCbs.delete(cb)
  }

  // ── Internal ─────────────────────────────────────────────────────────────────

  private openWs(): void {
    const url = IBSERVER.replace(/^http/, 'ws') + '/ws'
    try {
      this.ws = new WebSocket(url)
    } catch {
      this.scheduleWsRetry()
      return
    }

    this.ws.onopen = () => {
      this.wsReady = true
      // Re-subscribe all active symbols after reconnect/restart
      const conIds = [...this.symbolToConId.values()]
      if (conIds.length) this.wsSend({ action: 'subscribe', conIds })
      this.reconnectCbs.forEach(cb => { try { cb() } catch { /* */ } })
    }

    this.ws.onmessage = ev => {
      try { this.onWsMsg(JSON.parse(ev.data as string)) } catch { /* */ }
    }

    this.ws.onclose = () => {
      this.wsReady = false
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

  private onWsMsg(msg: Record<string, unknown>): void {
    if (msg.type !== 'quote' || !this.tickCb) return

    const symbol = this.conIdToSymbol.get(msg.conId as number)
    if (!symbol) return

    const tfs = this.subscriptions.get(symbol)
    if (!tfs?.size) return

    const last = Number(msg.last)
    const bid = Number(msg.bid)
    const ask = Number(msg.ask)
    const price = last > 0 ? last : (bid > 0 && ask > 0 ? (bid + ask) / 2 : 0)
    if (!(price > 0)) return

    const tick: TickData = {
      price,
      volume: Number(msg.volume) || 0,
      time: msg.timestamp
        ? Math.floor(Number(msg.timestamp) / 1000)
        : Math.floor(Date.now() / 1000),
    }

    for (const tf of tfs) this.tickCb(symbol, tf, tick)
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

  private wsSend(msg: object): void {
    if (this.ws?.readyState === WebSocket.OPEN) this.ws.send(JSON.stringify(msg))
  }
}
