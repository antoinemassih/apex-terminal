import type { Feed } from './Feed'
import type { TickData } from './types'
import { TF_TO_INTERVAL } from './timeframes'

export class SimulatedFeed implements Feed {
  private intervalId: number | null = null
  private subscriptions = new Map<string, { symbol: string; timeframe: string; simTime: number; tickCount: number }>()
  private tickCb: ((symbol: string, tf: string, tick: TickData) => void) | null = null
  private disconnectCb: (() => void) | null = null
  private lastPrices = new Map<string, number>()

  async connect(): Promise<void> {
    this.intervalId = window.setInterval(() => this.tick(), 250)
  }

  disconnect(): void {
    if (this.intervalId !== null) { clearInterval(this.intervalId); this.intervalId = null }
    this.disconnectCb?.()
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

  onDisconnect(cb: () => void): () => void {
    this.disconnectCb = cb
    return () => { this.disconnectCb = null }
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
}
