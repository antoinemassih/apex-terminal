export interface CoordConfig {
  width: number
  height: number
  barCount: number
  minPrice: number
  maxPrice: number
  paddingRight?: number
  paddingTop?: number
  paddingBottom?: number
  /** Fractional bar offset for smooth scrolling (0.0 = aligned, 0.5 = half bar shifted left) */
  scrollOffset?: number
}

const MAX_CACHE = 64
const cache = new Map<string, CoordSystem>()

function cacheKey(c: CoordConfig): string {
  const pr = c.paddingRight ?? 80
  const pt = c.paddingTop ?? 20
  const pb = c.paddingBottom ?? 40
  const so = c.scrollOffset ?? 0
  return `${c.width}|${c.height}|${c.barCount}|${c.minPrice.toFixed(6)}|${c.maxPrice.toFixed(6)}|${pr}|${pt}|${pb}|${so.toFixed(4)}`
}

export class CoordSystem {
  readonly width: number
  readonly height: number
  readonly barCount: number
  readonly minPrice: number
  readonly maxPrice: number
  readonly pr: number
  readonly pt: number
  readonly pb: number
  readonly scrollOffset: number

  constructor(c: CoordConfig) {
    this.width = c.width; this.height = c.height
    this.barCount = c.barCount
    this.minPrice = c.minPrice; this.maxPrice = c.maxPrice
    this.pr = c.paddingRight ?? 80
    this.pt = c.paddingTop ?? 20
    this.pb = c.paddingBottom ?? 40
    this.scrollOffset = c.scrollOffset ?? 0
  }

  static create(config: CoordConfig): CoordSystem {
    const key = cacheKey(config)
    const cached = cache.get(key)
    if (cached) return cached

    const cs = new CoordSystem(config)
    if (cache.size >= MAX_CACHE) {
      // Evict oldest entry (first inserted)
      const first = cache.keys().next().value!
      cache.delete(first)
    }
    cache.set(key, cs)
    return cs
  }

  get chartWidth() { return this.width - this.pr }
  get chartHeight() { return this.height - this.pt - this.pb }
  get barWidth() { return this.barCount > 0 ? (this.chartWidth / this.barCount) * 0.8 : 1 }
  get barStep() { return this.barCount > 0 ? this.chartWidth / this.barCount : 1 }

  barToX(index: number): number { return (index - this.scrollOffset) * this.barStep + this.barStep * 0.5 }
  xToBar(x: number): number { return (x - this.barStep * 0.5) / this.barStep + this.scrollOffset }
  priceToY(price: number): number {
    const ratio = (price - this.minPrice) / (this.maxPrice - this.minPrice)
    return this.pt + this.chartHeight * (1 - ratio)
  }
  yToPrice(y: number): number {
    const ratio = 1 - (y - this.pt) / this.chartHeight
    return this.minPrice + ratio * (this.maxPrice - this.minPrice)
  }
  barToClipX(index: number): number { return (this.barToX(index) / this.width) * 2 - 1 }
  priceToClipY(price: number): number { return 1 - (this.priceToY(price) / this.height) * 2 }
  clipBarWidth(): number { return (this.barWidth / this.width) * 2 }

  withSize(w: number, h: number): CoordSystem {
    return CoordSystem.create({ ...this.toConfig(), width: w, height: h })
  }
  withPriceRange(min: number, max: number): CoordSystem {
    return CoordSystem.create({ ...this.toConfig(), minPrice: min, maxPrice: max })
  }
  withBarCount(count: number): CoordSystem {
    return CoordSystem.create({ ...this.toConfig(), barCount: count })
  }
  private toConfig(): CoordConfig {
    return { width: this.width, height: this.height, barCount: this.barCount,
      minPrice: this.minPrice, maxPrice: this.maxPrice,
      paddingRight: this.pr, paddingTop: this.pt, paddingBottom: this.pb,
      scrollOffset: this.scrollOffset }
  }
}
