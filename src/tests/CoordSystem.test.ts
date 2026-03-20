import { describe, it, expect } from 'vitest'
import { CoordSystem } from '../chart/CoordSystem'

const cs = new CoordSystem({
  width: 1000, height: 600, barCount: 100,
  minPrice: 100, maxPrice: 200,
  paddingRight: 80, paddingTop: 20, paddingBottom: 40,
})

describe('CoordSystem (category axis)', () => {
  it('maps min price to bottom', () => {
    expect(cs.priceToY(100)).toBeCloseTo(560)
  })
  it('maps max price to top', () => {
    expect(cs.priceToY(200)).toBeCloseTo(20)
  })
  it('round-trips price', () => {
    expect(cs.yToPrice(cs.priceToY(150))).toBeCloseTo(150)
  })
  it('round-trips bar index', () => {
    expect(cs.xToBar(cs.barToX(50))).toBeCloseTo(50)
  })
  it('reports bar width > 0', () => {
    expect(cs.barWidth).toBeGreaterThan(0)
  })
  it('converts price to clip Y in [-1,1]', () => {
    const clipY = cs.priceToClipY(150)
    expect(clipY).toBeGreaterThan(-1)
    expect(clipY).toBeLessThan(1)
  })
  it('converts bar index to clip X in [-1,1]', () => {
    const clipX = cs.barToClipX(50)
    expect(clipX).toBeGreaterThan(-1)
    expect(clipX).toBeLessThan(1)
  })
})
