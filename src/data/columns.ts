import type { Bar } from '../types'

export class ColumnStore {
  readonly times: Float64Array
  readonly opens: Float64Array
  readonly highs: Float64Array
  readonly lows: Float64Array
  readonly closes: Float64Array
  readonly volumes: Float64Array
  readonly length: number

  private constructor(
    times: Float64Array, opens: Float64Array, highs: Float64Array,
    lows: Float64Array, closes: Float64Array, volumes: Float64Array,
  ) {
    this.times = times; this.opens = opens; this.highs = highs
    this.lows = lows; this.closes = closes; this.volumes = volumes
    this.length = times.length
  }

  static fromBars(bars: Bar[]): ColumnStore {
    const n = bars.length
    const times = new Float64Array(n)
    const opens = new Float64Array(n)
    const highs = new Float64Array(n)
    const lows = new Float64Array(n)
    const closes = new Float64Array(n)
    const volumes = new Float64Array(n)
    for (let i = 0; i < n; i++) {
      times[i] = bars[i].time
      opens[i] = bars[i].open
      highs[i] = bars[i].high
      lows[i] = bars[i].low
      closes[i] = bars[i].close
      volumes[i] = bars[i].volume
    }
    return new ColumnStore(times, opens, highs, lows, closes, volumes)
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
