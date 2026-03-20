export interface CoordConfig {
  width: number
  height: number
  barCount: number
  minPrice: number
  maxPrice: number
  paddingRight?: number
  paddingTop?: number
  paddingBottom?: number
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

  constructor(c: CoordConfig) {
    this.width = c.width; this.height = c.height
    this.barCount = c.barCount
    this.minPrice = c.minPrice; this.maxPrice = c.maxPrice
    this.pr = c.paddingRight ?? 80
    this.pt = c.paddingTop ?? 20
    this.pb = c.paddingBottom ?? 40
  }

  get chartWidth() { return this.width - this.pr }
  get chartHeight() { return this.height - this.pt - this.pb }
  get barWidth() { return this.barCount > 0 ? (this.chartWidth / this.barCount) * 0.8 : 1 }
  get barStep() { return this.barCount > 0 ? this.chartWidth / this.barCount : 1 }

  barToX(index: number): number { return index * this.barStep + this.barStep * 0.5 }
  xToBar(x: number): number { return (x - this.barStep * 0.5) / this.barStep }
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
    return new CoordSystem({ ...this.toConfig(), width: w, height: h })
  }
  withPriceRange(min: number, max: number): CoordSystem {
    return new CoordSystem({ ...this.toConfig(), minPrice: min, maxPrice: max })
  }
  withBarCount(count: number): CoordSystem {
    return new CoordSystem({ ...this.toConfig(), barCount: count })
  }
  private toConfig(): CoordConfig {
    return { width: this.width, height: this.height, barCount: this.barCount,
      minPrice: this.minPrice, maxPrice: this.maxPrice,
      paddingRight: this.pr, paddingTop: this.pt, paddingBottom: this.pb }
  }
}
