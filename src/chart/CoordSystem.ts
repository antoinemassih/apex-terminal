export interface CoordConfig {
  width: number
  height: number
  barCount: number
  minPrice: number
  maxPrice: number
  paddingRight?: number
  paddingTop?: number
  paddingBottom?: number
  pixelOffset?: number  // sub-bar scroll offset in pixels (for smooth panning)
}

const cache = new Map<string, CoordSystem>()
const MAX_CACHE = 16 // small — only caches static viewports (symbol/tf switch, resize)

function cacheKey(c: CoordConfig): string {
  const pr = c.paddingRight ?? 42
  const pt = c.paddingTop ?? 20
  const pb = c.paddingBottom ?? 0
  // pixelOffset changes every sub-pixel pan frame — skip cache for dynamic viewports.
  // CoordSystem is a tiny object (~200 bytes), creating one per frame is cheaper than a cache miss path.
  if (c.pixelOffset && c.pixelOffset % 1 !== 0) return ''
  return `${c.width}|${c.height}|${c.barCount}|${c.minPrice.toFixed(6)}|${c.maxPrice.toFixed(6)}|${pr}|${pt}|${pb}`
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
  readonly pixelOffset: number
  constructor(c: CoordConfig) {
    this.width = c.width; this.height = c.height
    this.barCount = c.barCount
    this.minPrice = c.minPrice; this.maxPrice = c.maxPrice
    this.pr = c.paddingRight ?? 42
    this.pt = c.paddingTop ?? 20
    this.pb = c.paddingBottom ?? 0
    this.pixelOffset = c.pixelOffset ?? 0
  }

  static create(config: CoordConfig): CoordSystem {
    const key = cacheKey(config)
    if (key) {
      const cached = cache.get(key)
      if (cached) return cached
    }
    const cs = new CoordSystem(config)
    if (key) {
      if (cache.size >= MAX_CACHE) {
        const first = cache.keys().next().value!
        cache.delete(first)
      }
      cache.set(key, cs)
    }
    return cs
  }

  get chartWidth() { return this.width - this.pr }
  get chartHeight() { return this.height - this.pt - this.pb }
  get barWidth() { return this.barCount > 0 ? (this.chartWidth / this.barCount) * 0.8 : 1 }
  get barStep() { return this.barCount > 0 ? this.chartWidth / this.barCount : 1 }

  barToX(index: number): number { return index * this.barStep + this.barStep * 0.5 - this.pixelOffset }
  xToBar(x: number): number { return (x + this.pixelOffset - this.barStep * 0.5) / this.barStep }
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
      pixelOffset: this.pixelOffset }
  }
}
