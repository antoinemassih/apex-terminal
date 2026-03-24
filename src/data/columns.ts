import type { Bar } from '../types'

const MAX_CAPACITY = 50_000
const EVICT_KEEP_RATIO = 0.75

export class ColumnStore {
  times: Float64Array
  opens: Float64Array
  highs: Float64Array
  lows: Float64Array
  closes: Float64Array
  volumes: Float64Array
  length: number
  lastEvictCount: number = 0
  private capacity: number

  private constructor(
    times: Float64Array, opens: Float64Array, highs: Float64Array,
    lows: Float64Array, closes: Float64Array, volumes: Float64Array,
    length: number,
  ) {
    this.times = times; this.opens = opens; this.highs = highs
    this.lows = lows; this.closes = closes; this.volumes = volumes
    this.length = length
    this.capacity = times.length
  }

  static fromBars(bars: Bar[]): ColumnStore {
    const capacity = Math.min(bars.length + 512, MAX_CAPACITY)
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
    return new ColumnStore(times, opens, highs, lows, closes, volumes, n)
  }

  /** Apply a tick to the last candle or start a new one */
  applyTick(price: number, volume: number, time: number, intervalSecs: number): 'updated' | 'created' {
    this.lastEvictCount = 0

    // Empty store — first bar ever; times[-1] is undefined so nextCandleTime would be NaN
    if (this.length === 0) {
      if (this.capacity === 0) this.grow()
      this.times[0] = time
      this.opens[0] = price; this.highs[0] = price
      this.lows[0] = price; this.closes[0] = price
      this.volumes[0] = volume
      this.length = 1
      return 'created'
    }

    const last = this.length - 1
    const lastTime = this.times[last]
    const nextCandleTime = lastTime + intervalSecs

    if (time < nextCandleTime) {
      // Update existing candle
      this.closes[last] = price
      if (price > this.highs[last]) this.highs[last] = price
      if (price < this.lows[last]) this.lows[last] = price
      this.volumes[last] += volume
      return 'updated'
    } else {
      // New candle — ensure capacity
      if (this.length >= this.capacity) this.grow()
      const idx = this.length
      this.times[idx] = nextCandleTime
      this.opens[idx] = price
      this.highs[idx] = price
      this.lows[idx] = price
      this.closes[idx] = price
      this.volumes[idx] = volume
      this.length++
      return 'created'
    }
  }

  /** Double array capacity, evicting oldest data if at max */
  private grow(): void {
    if (this.capacity >= MAX_CAPACITY) {
      // At max: evict oldest 25%, then continue with same capacity
      this.evict()
      // After eviction, length is reduced — there's room now
      return
    }
    const newCap = Math.min(this.capacity * 2, MAX_CAPACITY)
    const names = ['times', 'opens', 'highs', 'lows', 'closes', 'volumes'] as const
    for (const name of names) {
      const old = this[name]
      const arr = new Float64Array(newCap)
      arr.set(old.subarray(0, this.length))
      this[name] = arr
    }
    this.capacity = newCap
  }

  /** Evict oldest data, keeping the most recent portion */
  private evict(): void {
    const keep = Math.floor(this.length * EVICT_KEEP_RATIO)
    const drop = this.length - keep
    const names = ['times', 'opens', 'highs', 'lows', 'closes', 'volumes'] as const
    for (const name of names) {
      const old = this[name]
      // Copy in-place: shift data left by `drop` positions
      // This avoids allocating new arrays during eviction
      old.copyWithin(0, drop, drop + keep)
    }
    this.lastEvictCount = drop
    this.length = keep
    // capacity stays the same — we freed up (capacity - keep) slots
  }

  /** Clone for immutable state updates */
  clone(): ColumnStore {
    return new ColumnStore(
      new Float64Array(this.times.subarray(0, this.length)),
      new Float64Array(this.opens.subarray(0, this.length)),
      new Float64Array(this.highs.subarray(0, this.length)),
      new Float64Array(this.lows.subarray(0, this.length)),
      new Float64Array(this.closes.subarray(0, this.length)),
      new Float64Array(this.volumes.subarray(0, this.length)),
      this.length,
    )
  }

  priceRange(start: number, end: number): { min: number; max: number } {
    let min = Infinity, max = -Infinity
    const e = Math.min(end, this.length)
    for (let i = Math.max(0, start); i < e; i++) {
      if (this.lows[i] < min) min = this.lows[i]
      if (this.highs[i] > max) max = this.highs[i]
    }
    if (!isFinite(min) || !isFinite(max)) { return { min: 0, max: 1 } }
    if (min === max) { min -= 0.005; max += 0.005 }
    return { min, max }
  }

  /** Prepend older bars at the beginning. Returns count actually prepended. */
  prepend(bars: Bar[]): number {
    if (bars.length === 0) return 0
    // Limit: don't exceed MAX_CAPACITY
    const maxPrepend = Math.max(0, MAX_CAPACITY - this.length)
    const actualBars = bars.length <= maxPrepend ? bars : bars.slice(-maxPrepend)
    if (actualBars.length === 0) return 0

    const newLen = this.length + actualBars.length
    const names = ['times', 'opens', 'highs', 'lows', 'closes', 'volumes'] as const
    const barKeys = ['time', 'open', 'high', 'low', 'close', 'volume'] as const

    if (newLen > this.capacity) {
      const newCap = Math.min(Math.max(newLen, this.capacity * 2), MAX_CAPACITY)
      for (const name of names) {
        const old = this[name]
        const arr = new Float64Array(newCap)
        arr.set(old.subarray(0, this.length), actualBars.length)
        this[name] = arr
      }
      this.capacity = newCap
    } else {
      for (const name of names) {
        this[name].copyWithin(actualBars.length, 0, this.length)
      }
    }

    for (let i = 0; i < actualBars.length; i++) {
      for (let j = 0; j < names.length; j++) {
        this[names[j]][i] = actualBars[i][barKeys[j] as keyof Bar] as number
      }
    }
    this.length = newLen
    return actualBars.length
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
