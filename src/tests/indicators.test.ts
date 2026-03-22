import { describe, it, expect } from 'vitest'
import { IncrementalSMA } from '../indicators/incremental/sma'
import { IncrementalEMA } from '../indicators/incremental/ema'
import { IncrementalBollinger } from '../indicators/incremental/bollinger'

/* ── Naive reference implementations ── */

function naiveSMA(values: number[], period: number): number[] {
  const result: number[] = []
  for (let i = 0; i < values.length; i++) {
    if (i < period - 1) { result.push(NaN); continue }
    let sum = 0
    for (let j = i - period + 1; j <= i; j++) sum += values[j]
    result.push(sum / period)
  }
  return result
}

function naiveEMA(values: number[], period: number): number[] {
  const k = 2 / (period + 1)
  const result: number[] = [NaN]
  let prev = values[0]
  for (let i = 1; i < values.length; i++) {
    prev = values[i] * k + prev * (1 - k)
    result.push(i >= period - 1 ? prev : NaN)
  }
  return result
}

function naiveBollinger(values: number[], period: number, stdDevs: number) {
  const upper: number[] = [], lower: number[] = []
  for (let i = 0; i < values.length; i++) {
    if (i < period - 1) { upper.push(NaN); lower.push(NaN); continue }
    let sum = 0
    for (let j = i - period + 1; j <= i; j++) sum += values[j]
    const mean = sum / period
    let sqSum = 0
    for (let j = i - period + 1; j <= i; j++) sqSum += (values[j] - mean) ** 2
    const std = Math.sqrt(sqSum / period)
    upper.push(mean + stdDevs * std)
    lower.push(mean - stdDevs * std)
  }
  return { upper, lower }
}

/* ── SMA tests ── */

describe('IncrementalSMA', () => {
  it('matches naive SMA on bootstrap', () => {
    const prices = Array.from({ length: 200 }, () => 100 + Math.random() * 50)
    const closes = new Float64Array(prices)
    const sma = new IncrementalSMA(20, 300)
    sma.bootstrap(closes, closes.length)

    const naive = naiveSMA(prices, 20)
    const output = sma.getOutput()
    for (let i = 0; i < prices.length; i++) {
      if (isNaN(naive[i])) {
        expect(isNaN(output[i])).toBe(true)
      } else {
        expect(output[i]).toBeCloseTo(naive[i], 10)
      }
    }
  })

  it('matches naive SMA after incremental pushes', () => {
    const prices = Array.from({ length: 100 }, () => 100 + Math.random() * 50)
    const closes = new Float64Array(prices.slice(0, 50))
    const sma = new IncrementalSMA(20, 200)
    sma.bootstrap(closes, 50)

    // Push remaining 50 one by one
    for (let i = 50; i < 100; i++) sma.push(prices[i])

    const naive = naiveSMA(prices, 20)
    const output = sma.getOutput()
    for (let i = 0; i < 100; i++) {
      if (isNaN(naive[i])) {
        expect(isNaN(output[i])).toBe(true)
      } else {
        expect(output[i]).toBeCloseTo(naive[i], 10)
      }
    }
  })

  it('updateLast produces correct SMA', () => {
    const prices = [10, 20, 30, 40, 50]
    const closes = new Float64Array(prices)
    const sma = new IncrementalSMA(3, 20)
    sma.bootstrap(closes, 5)

    // Update last value from 50 to 60
    sma.updateLast(60)
    const output = sma.getOutput()
    // Last SMA should be (30+40+60)/3 = 43.333...
    expect(output[4]).toBeCloseTo((30 + 40 + 60) / 3, 10)
  })
})

/* ── EMA tests ── */

describe('IncrementalEMA', () => {
  it('matches naive EMA on bootstrap', () => {
    const prices = Array.from({ length: 200 }, () => 100 + Math.random() * 50)
    const closes = new Float64Array(prices)
    const ema = new IncrementalEMA(50, 300)
    ema.bootstrap(closes, closes.length)

    const naive = naiveEMA(prices, 50)
    const output = ema.getOutput()
    for (let i = 0; i < prices.length; i++) {
      if (isNaN(naive[i])) {
        expect(isNaN(output[i])).toBe(true)
      } else {
        expect(output[i]).toBeCloseTo(naive[i], 8)
      }
    }
  })

  it('matches naive EMA after incremental pushes', () => {
    const prices = Array.from({ length: 100 }, () => 100 + Math.random() * 50)
    const closes = new Float64Array(prices.slice(0, 50))
    const ema = new IncrementalEMA(20, 200)
    ema.bootstrap(closes, 50)
    for (let i = 50; i < 100; i++) ema.push(prices[i])

    const naive = naiveEMA(prices, 20)
    const output = ema.getOutput()
    for (let i = 0; i < 100; i++) {
      if (isNaN(naive[i])) {
        expect(isNaN(output[i])).toBe(true)
      } else {
        expect(output[i]).toBeCloseTo(naive[i], 8)
      }
    }
  })
})

/* ── Bollinger tests ── */

describe('IncrementalBollinger', () => {
  it('matches naive Bollinger on bootstrap', () => {
    const prices = Array.from({ length: 200 }, () => 100 + Math.random() * 50)
    const closes = new Float64Array(prices)
    const bb = new IncrementalBollinger(20, 2, 300)
    bb.bootstrap(closes, closes.length)

    const naive = naiveBollinger(prices, 20, 2)
    const upper = bb.getUpper(), lower = bb.getLower()
    for (let i = 0; i < prices.length; i++) {
      if (isNaN(naive.upper[i])) {
        expect(isNaN(upper[i])).toBe(true)
      } else {
        expect(upper[i]).toBeCloseTo(naive.upper[i], 6)
        expect(lower[i]).toBeCloseTo(naive.lower[i], 6)
      }
    }
  })

  it('matches after incremental pushes', () => {
    const prices = Array.from({ length: 100 }, () => 100 + Math.random() * 50)
    const closes = new Float64Array(prices.slice(0, 50))
    const bb = new IncrementalBollinger(20, 2, 200)
    bb.bootstrap(closes, 50)
    for (let i = 50; i < 100; i++) bb.push(prices[i])

    const naive = naiveBollinger(prices, 20, 2)
    const upper = bb.getUpper(), lower = bb.getLower()
    for (let i = 0; i < 100; i++) {
      if (isNaN(naive.upper[i])) continue
      expect(upper[i]).toBeCloseTo(naive.upper[i], 5)
      expect(lower[i]).toBeCloseTo(naive.lower[i], 5)
    }
  })

  it('updateLast is correct', () => {
    const prices = Array.from({ length: 30 }, (_, i) => 100 + i)
    const closes = new Float64Array(prices)
    const bb = new IncrementalBollinger(20, 2, 50)
    bb.bootstrap(closes, 30)

    // Update last to a different value and verify against naive
    const modifiedPrices = [...prices]
    modifiedPrices[29] = 150
    bb.updateLast(150)

    const naive = naiveBollinger(modifiedPrices, 20, 2)
    expect(bb.getUpper()[29]).toBeCloseTo(naive.upper[29], 5)
    expect(bb.getLower()[29]).toBeCloseTo(naive.lower[29], 5)
  })
})
