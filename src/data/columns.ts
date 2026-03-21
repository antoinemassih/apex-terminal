import type { Bar } from '../types'

export class ColumnStore {
  times: Float64Array
  opens: Float64Array
  highs: Float64Array
  lows: Float64Array
  closes: Float64Array
  volumes: Float64Array
  length: number

  private constructor(
    times: Float64Array, opens: Float64Array, highs: Float64Array,
    lows: Float64Array, closes: Float64Array, volumes: Float64Array,
  ) {
    this.times = times; this.opens = opens; this.highs = highs
    this.lows = lows; this.closes = closes; this.volumes = volumes
    this.length = times.length
  }

  static fromBars(bars: Bar[]): ColumnStore {
    // Allocate extra capacity for incoming ticks
    const capacity = bars.length + 512
    const n = bars.length
    const times = new Float64Array(capacity)
    const opens = new Float64Array(capacity)
    const highs = new Float64Array(capacity)
    const lows = new Float64Array(capacity)
    const closes = new Float64Array(capacity)
    const volumes = new Float64Array(capacity)
    for (let i = 0; i < n; i++) {
      times[i] = bars[i].time
      opens[i] = bars[i].open
      highs[i] = bars[i].high
      lows[i] = bars[i].low
      closes[i] = bars[i].close
      volumes[i] = bars[i].volume
    }
    const store = new ColumnStore(times, opens, highs, lows, closes, volumes)
    store.length = n
    return store
  }

  /** Apply a tick to the last candle or start a new one */
  applyTick(price: number, volume: number, time: number, intervalSecs: number): boolean {
    const last = this.length - 1
    const lastTime = this.times[last]
    const nextCandleTime = lastTime + intervalSecs

    if (time < nextCandleTime) {
      // Update existing candle
      this.closes[last] = price
      if (price > this.highs[last]) this.highs[last] = price
      if (price < this.lows[last]) this.lows[last] = price
      this.volumes[last] += volume
      return false // no new candle
    } else {
      // New candle
      const idx = this.length
      if (idx >= this.times.length) return false // at capacity
      this.times[idx] = nextCandleTime
      this.opens[idx] = price
      this.highs[idx] = price
      this.lows[idx] = price
      this.closes[idx] = price
      this.volumes[idx] = volume
      this.length++
      return true // new candle added
    }
  }

  /** Clone for immutable state updates */
  clone(): ColumnStore {
    const store = new ColumnStore(
      new Float64Array(this.times),
      new Float64Array(this.opens),
      new Float64Array(this.highs),
      new Float64Array(this.lows),
      new Float64Array(this.closes),
      new Float64Array(this.volumes),
    )
    store.length = this.length
    return store
  }

  priceRange(start: number, end: number): { min: number; max: number } {
    let min = Infinity, max = -Infinity
    const e = Math.min(end, this.length)
    for (let i = start; i < e; i++) {
      if (this.lows[i] < min) min = this.lows[i]
      if (this.highs[i] > max) max = this.highs[i]
    }
    return { min, max }
  }

  indexAtTime(time: number): number {
    let lo = 0, hi = this.length - 1
    while (lo <= hi) {
      const mid = (lo + hi) >>> 1
      if (this.times[mid] <= time) lo = mid + 1
      else hi = mid - 1
    }
    return Math.max(0, hi)
  }
}
