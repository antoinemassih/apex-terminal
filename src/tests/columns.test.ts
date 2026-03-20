import { describe, it, expect } from 'vitest'
import { ColumnStore } from '../data/columns'
import type { Bar } from '../types'

const BARS: Bar[] = [
  { time: 1000, open: 100, high: 110, low: 90, close: 105, volume: 1000 },
  { time: 1060, open: 105, high: 115, low: 95, close: 110, volume: 2000 },
  { time: 1120, open: 110, high: 120, low: 100, close: 108, volume: 1500 },
]

describe('ColumnStore', () => {
  it('converts Bar[] to columnar arrays', () => {
    const store = ColumnStore.fromBars(BARS)
    expect(store.length).toBe(3)
    expect(store.opens[0]).toBe(100)
    expect(store.closes[2]).toBe(108)
    expect(store.highs[1]).toBe(115)
  })
  it('returns min/max for a range', () => {
    const store = ColumnStore.fromBars(BARS)
    const { min, max } = store.priceRange(0, 3)
    expect(min).toBe(90)
    expect(max).toBe(120)
  })
  it('binary searches for time index', () => {
    const store = ColumnStore.fromBars(BARS)
    expect(store.indexAtTime(1060)).toBe(1)
    expect(store.indexAtTime(1090)).toBe(1)
  })
})
