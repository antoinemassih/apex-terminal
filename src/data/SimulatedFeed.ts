import type { Feed } from './Feed'
import type { TickData } from './types'
import { TF_TO_INTERVAL } from './timeframes'

export class SimulatedFeed implements Feed {
  private intervalId: number | null = null
  private subscriptions = new Map<string, { symbol: string; timeframe: string; simTime: number; tickCount: number }>()
  private tickCbs = new Set<(symbol: string, tf: string, tick: TickData) => void>()
  private disconnectListeners = new Set<() => void>()
  private reconnectListeners = new Set<() => void>()
  private lastPrices = new Map<string, number>()
  private connected = false

  async connect(): Promise<void> {
    if (this.connected) return
    this.connected = true
    this.startTicking()
  }

  disconnect(): void {
    this.connected = false
    if (this.intervalId !== null) { clearInterval(this.intervalId); this.intervalId = null }
    for (const cb of this.disconnectListeners) { try { cb() } catch (e) { /* */ } }
  }

  async reconnect(): Promise<void> {
    if (this.connected) return
    this.connected = true
    this.startTicking()
    for (const cb of this.reconnectListeners) { try { cb() } catch (e) { /* */ } }
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
    this.tickCbs.add(cb)
    return () => { this.tickCbs.delete(cb) }
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

  private startTicking(): void {
    if (this.intervalId !== null) clearInterval(this.intervalId)
    this.intervalId = window.setInterval(() => this.tick(), 50)
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
      for (const cb of this.tickCbs) cb(sub.symbol, sub.timeframe, { price, volume, time: sub.simTime })
    }
  }
}
