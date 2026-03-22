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

describe('applyTick', () => {
  it('updates existing candle when within interval', () => {
    const bars: Bar[] = [{ time: 1000, open: 100, high: 110, low: 90, close: 105, volume: 500 }]
    const store = ColumnStore.fromBars(bars)
    const action = store.applyTick(115, 100, 1030, 60) // within 60s interval
    expect(action).toBe('updated')
    expect(store.length).toBe(1)
    expect(store.closes[0]).toBe(115)
    expect(store.highs[0]).toBe(115) // new high
    expect(store.lows[0]).toBe(90) // unchanged
    expect(store.volumes[0]).toBe(600) // accumulated
  })

  it('creates new candle when past interval', () => {
    const bars: Bar[] = [{ time: 1000, open: 100, high: 110, low: 90, close: 105, volume: 500 }]
    const store = ColumnStore.fromBars(bars)
    const action = store.applyTick(120, 200, 1060, 60) // at 60s boundary
    expect(action).toBe('created')
    expect(store.length).toBe(2)
    expect(store.opens[1]).toBe(120)
    expect(store.closes[1]).toBe(120)
    expect(store.times[1]).toBe(1060) // nextCandleTime = 1000 + 60
  })
})

describe('grow and evict', () => {
  it('grows capacity when full', () => {
    const bars: Bar[] = Array.from({ length: 512 }, (_, i) => ({
      time: i * 60, open: 100, high: 110, low: 90, close: 105, volume: 100,
    }))
    const store = ColumnStore.fromBars(bars) // capacity = 512 + 512 = 1024
    // Fill to capacity and beyond — 513 new candles bring length to 1025
    for (let i = 0; i < 513; i++) {
      store.applyTick(100, 100, (512 + i) * 60 + 60, 60)
    }
    expect(store.length).toBe(1025)
    // Should have grown without data loss
    expect(store.times[0]).toBe(0)
    expect(store.times[1024]).toBe(1024 * 60)
  })

  it('evicts oldest when at max capacity', () => {
    // Create store that fills to max capacity (50000)
    // fromBars with 49999 bars => capacity = min(49999+512, 50000) = 50000, length = 49999
    const bars: Bar[] = Array.from({ length: 49999 }, (_, i) => ({
      time: i * 60, open: 100, high: 110, low: 90, close: 105, volume: 100,
    }))
    const store = ColumnStore.fromBars(bars)
    // First push fills to 50000 (capacity)
    store.applyTick(100, 100, 49999 * 60 + 60, 60)
    expect(store.length).toBe(50000)
    // Second push triggers grow -> evict since capacity is at max
    store.applyTick(100, 100, 50000 * 60 + 60, 60)
    // After eviction: kept 75% of 50000 = 37500, then added 1 = 37501
    expect(store.length).toBeLessThan(50000)
    expect(store.length).toBeGreaterThan(37000)
    // Newest data preserved — last candle is the one just created
    expect(store.times[store.length - 1]).toBe(50000 * 60)
  })
})

describe('priceRange edge cases', () => {
  it('returns epsilon range when min equals max', () => {
    const bars: Bar[] = [
      { time: 0, open: 100, high: 100, low: 100, close: 100, volume: 0 },
      { time: 60, open: 100, high: 100, low: 100, close: 100, volume: 0 },
    ]
    const store = ColumnStore.fromBars(bars)
    const range = store.priceRange(0, 2)
    expect(range.max).toBeGreaterThan(range.min)
  })

  it('handles empty range gracefully', () => {
    const bars: Bar[] = [{ time: 0, open: 100, high: 110, low: 90, close: 105, volume: 100 }]
    const store = ColumnStore.fromBars(bars)
    const range = store.priceRange(5, 10) // out of bounds
    expect(range.min).toBeDefined()
    expect(range.max).toBeGreaterThan(range.min)
  })
})
