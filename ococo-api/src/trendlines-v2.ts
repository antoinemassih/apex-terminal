/**
 * Advanced Trendline Detection Engine v2
 *
 * Multiple detection methodologies, self-backtesting for refinement,
 * and multi-dimensional strength scoring.
 *
 * Methodologies:
 * 1. Pivot-based: classical swing high/low connections
 * 2. Linear Regression: statistical best-fit with R² confidence
 * 3. Fractal: Williams fractals for multi-scale detection
 * 4. Volume-Weighted: pivots weighted by volume significance
 * 5. Touch Density: finds lines where price repeatedly tests a level
 *
 * Each detected line is backtested against forward price action to
 * compute a validated strength score.
 */

import { query } from './db.js'
import { publishSignal } from './signalBus.js'
import { invalidate } from './cache.js'
import type { Annotation } from './types.js'
import { v4 as uuid } from 'uuid'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface Bar {
  time: number
  open: number
  high: number
  low: number
  close: number
  volume: number
}

interface Pivot {
  index: number
  time: number
  price: number
  type: 'high' | 'low'
  source: 'wick' | 'body'
  volume: number
  strength: number // 0-1 based on lookback significance
}

interface RawTrendline {
  p1: { index: number; time: number; price: number }
  p2: { index: number; time: number; price: number }
  slope: number
  intercept: number
  direction: 'support' | 'resistance'
  source: 'wick' | 'body'
  method: string
}

interface BacktestResult {
  touches: number
  bounces: number
  breaks: number
  bounceRate: number // bounces / (bounces + breaks)
  avgBounceVolume: number
  avgBreakVolume: number
  maxConsecutiveBounces: number
  forwardTouchCount: number // touches after p2
  stillValid: boolean // hasn't been decisively broken
}

interface ScoredTrendline extends RawTrendline {
  backtest: BacktestResult
  strength: StrengthScore
}

interface StrengthScore {
  total: number // 0-100 composite score
  touchScore: number // 0-20 based on touch count
  bounceScore: number // 0-25 based on bounce rate
  spanScore: number // 0-15 based on time span
  angleScore: number // 0-10 near-horizontal bonus
  volumeScore: number // 0-10 volume at touches
  recencyScore: number // 0-10 recent touches
  validityScore: number // 0-10 still holding
}

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

export interface DetectionConfig {
  methods: {
    pivot: boolean
    regression: boolean
    fractal: boolean
    volumeWeighted: boolean
    touchDensity: boolean
  }
  pivotLookbacks: number[]
  minTouchCount: number
  minStrength: number
  maxLines: number
  backtestForwardBars: number
  touchTolerance: number // % of price
  breakThreshold: number // % beyond line to count as break
}

export const DEFAULT_CONFIG: DetectionConfig = {
  methods: {
    pivot: true,
    regression: true,
    fractal: true,
    volumeWeighted: true,
    touchDensity: true,
  },
  pivotLookbacks: [3, 5, 8, 13, 21],
  minTouchCount: 2,
  minStrength: 15,
  maxLines: 30,
  backtestForwardBars: 50,
  touchTolerance: 0.3, // 0.3%
  breakThreshold: 0.5, // 0.5%
}

// ---------------------------------------------------------------------------
// Utility
// ---------------------------------------------------------------------------

function priceAt(slope: number, intercept: number, index: number): number {
  return slope * index + intercept
}

function lineFromPoints(i1: number, p1: number, i2: number, p2: number) {
  const slope = (p2 - p1) / (i2 - i1)
  const intercept = p1 - slope * i1
  return { slope, intercept }
}

// ---------------------------------------------------------------------------
// Method 1: Pivot-Based Detection
// ---------------------------------------------------------------------------

function detectPivots(bars: Bar[], lookback: number, source: 'wick' | 'body'): Pivot[] {
  const pivots: Pivot[] = []
  const len = bars.length

  for (let i = lookback; i < len - lookback; i++) {
    const high = source === 'wick' ? bars[i].high : Math.max(bars[i].open, bars[i].close)
    const low = source === 'wick' ? bars[i].low : Math.min(bars[i].open, bars[i].close)

    let isHigh = true, isLow = true
    let maxNeighborHigh = -Infinity, minNeighborLow = Infinity

    for (let j = i - lookback; j <= i + lookback; j++) {
      if (j === i) continue
      const nh = source === 'wick' ? bars[j].high : Math.max(bars[j].open, bars[j].close)
      const nl = source === 'wick' ? bars[j].low : Math.min(bars[j].open, bars[j].close)
      if (nh >= high) isHigh = false
      if (nl <= low) isLow = false
      if (nh > maxNeighborHigh) maxNeighborHigh = nh
      if (nl < minNeighborLow) minNeighborLow = nl
    }

    // Pivot strength: how much it stands out from neighbors
    if (isHigh) {
      const prominence = (high - maxNeighborHigh) / high
      pivots.push({
        index: i, time: bars[i].time, price: high,
        type: 'high', source, volume: bars[i].volume,
        strength: Math.min(1, prominence * 100 + lookback / 21),
      })
    }
    if (isLow) {
      const prominence = (minNeighborLow - low) / low
      pivots.push({
        index: i, time: bars[i].time, price: low,
        type: 'low', source, volume: bars[i].volume,
        strength: Math.min(1, prominence * 100 + lookback / 21),
      })
    }
  }
  return pivots
}

function pivotMethod(bars: Bar[], config: DetectionConfig, source: 'wick' | 'body'): RawTrendline[] {
  const allPivots: Pivot[] = []
  for (const lb of config.pivotLookbacks) {
    allPivots.push(...detectPivots(bars, lb, source))
  }

  // Deduplicate pivots at same index
  const seen = new Set<string>()
  const pivots = allPivots.filter(p => {
    const key = `${p.index}-${p.type}-${p.source}`
    if (seen.has(key)) return false
    seen.add(key)
    return true
  })

  const highs = pivots.filter(p => p.type === 'high').sort((a, b) => a.index - b.index)
  const lows = pivots.filter(p => p.type === 'low').sort((a, b) => a.index - b.index)

  const lines: RawTrendline[] = []

  // Connect lows for support
  for (let i = 0; i < lows.length; i++) {
    for (let j = i + 1; j < lows.length; j++) {
      if (lows[j].index - lows[i].index < 5) continue
      const { slope, intercept } = lineFromPoints(lows[i].index, lows[i].price, lows[j].index, lows[j].price)
      lines.push({
        p1: { index: lows[i].index, time: lows[i].time, price: lows[i].price },
        p2: { index: lows[j].index, time: lows[j].time, price: lows[j].price },
        slope, intercept, direction: 'support', source, method: 'pivot',
      })
    }
  }

  // Connect highs for resistance
  for (let i = 0; i < highs.length; i++) {
    for (let j = i + 1; j < highs.length; j++) {
      if (highs[j].index - highs[i].index < 5) continue
      const { slope, intercept } = lineFromPoints(highs[i].index, highs[i].price, highs[j].index, highs[j].price)
      lines.push({
        p1: { index: highs[i].index, time: highs[i].time, price: highs[i].price },
        p2: { index: highs[j].index, time: highs[j].time, price: highs[j].price },
        slope, intercept, direction: 'resistance', source, method: 'pivot',
      })
    }
  }

  return lines
}

// ---------------------------------------------------------------------------
// Method 2: Linear Regression
// ---------------------------------------------------------------------------

function regressionMethod(bars: Bar[], _config: DetectionConfig, source: 'wick' | 'body'): RawTrendline[] {
  const lines: RawTrendline[] = []
  const windows = [20, 50, 100, 200].filter(w => w < bars.length)

  for (const window of windows) {
    for (let start = 0; start <= bars.length - window; start += Math.floor(window / 3)) {
      const end = start + window

      // Compute regression for highs and lows
      for (const dir of ['support', 'resistance'] as const) {
        let sumX = 0, sumY = 0, sumXY = 0, sumX2 = 0
        for (let i = start; i < end; i++) {
          const y = dir === 'support'
            ? (source === 'wick' ? bars[i].low : Math.min(bars[i].open, bars[i].close))
            : (source === 'wick' ? bars[i].high : Math.max(bars[i].open, bars[i].close))
          sumX += i
          sumY += y
          sumXY += i * y
          sumX2 += i * i
        }
        const n = window
        const slope = (n * sumXY - sumX * sumY) / (n * sumX2 - sumX * sumX)
        const intercept = (sumY - slope * sumX) / n

        // Compute R²
        const yMean = sumY / n
        let ssRes = 0, ssTot = 0
        for (let i = start; i < end; i++) {
          const y = dir === 'support'
            ? (source === 'wick' ? bars[i].low : Math.min(bars[i].open, bars[i].close))
            : (source === 'wick' ? bars[i].high : Math.max(bars[i].open, bars[i].close))
          const predicted = priceAt(slope, intercept, i)
          ssRes += (y - predicted) ** 2
          ssTot += (y - yMean) ** 2
        }
        const r2 = ssTot > 0 ? 1 - ssRes / ssTot : 0
        if (r2 < 0.7) continue // weak fit

        lines.push({
          p1: { index: start, time: bars[start].time, price: priceAt(slope, intercept, start) },
          p2: { index: end - 1, time: bars[end - 1].time, price: priceAt(slope, intercept, end - 1) },
          slope, intercept, direction: dir, source, method: 'regression',
        })
      }
    }
  }

  return lines
}

// ---------------------------------------------------------------------------
// Method 3: Fractal (Williams)
// ---------------------------------------------------------------------------

function fractalMethod(bars: Bar[], _config: DetectionConfig, source: 'wick' | 'body'): RawTrendline[] {
  // Williams fractal: 5-bar pattern where middle bar is highest/lowest
  const fractals: Pivot[] = []

  for (let i = 2; i < bars.length - 2; i++) {
    const h = source === 'wick' ? bars[i].high : Math.max(bars[i].open, bars[i].close)
    const l = source === 'wick' ? bars[i].low : Math.min(bars[i].open, bars[i].close)

    const getH = (j: number) => source === 'wick' ? bars[j].high : Math.max(bars[j].open, bars[j].close)
    const getL = (j: number) => source === 'wick' ? bars[j].low : Math.min(bars[j].open, bars[j].close)

    if (h > getH(i-1) && h > getH(i-2) && h > getH(i+1) && h > getH(i+2)) {
      fractals.push({ index: i, time: bars[i].time, price: h, type: 'high', source, volume: bars[i].volume, strength: 0.7 })
    }
    if (l < getL(i-1) && l < getL(i-2) && l < getL(i+1) && l < getL(i+2)) {
      fractals.push({ index: i, time: bars[i].time, price: l, type: 'low', source, volume: bars[i].volume, strength: 0.7 })
    }
  }

  const lines: RawTrendline[] = []
  const highs = fractals.filter(f => f.type === 'high')
  const lows = fractals.filter(f => f.type === 'low')

  for (let i = 0; i < lows.length; i++) {
    for (let j = i + 1; j < Math.min(i + 8, lows.length); j++) {
      if (lows[j].index - lows[i].index < 3) continue
      const { slope, intercept } = lineFromPoints(lows[i].index, lows[i].price, lows[j].index, lows[j].price)
      lines.push({
        p1: { index: lows[i].index, time: lows[i].time, price: lows[i].price },
        p2: { index: lows[j].index, time: lows[j].time, price: lows[j].price },
        slope, intercept, direction: 'support', source, method: 'fractal',
      })
    }
  }
  for (let i = 0; i < highs.length; i++) {
    for (let j = i + 1; j < Math.min(i + 8, highs.length); j++) {
      if (highs[j].index - highs[i].index < 3) continue
      const { slope, intercept } = lineFromPoints(highs[i].index, highs[i].price, highs[j].index, highs[j].price)
      lines.push({
        p1: { index: highs[i].index, time: highs[i].time, price: highs[i].price },
        p2: { index: highs[j].index, time: highs[j].time, price: highs[j].price },
        slope, intercept, direction: 'resistance', source, method: 'fractal',
      })
    }
  }

  return lines
}

// ---------------------------------------------------------------------------
// Method 4: Volume-Weighted Pivots
// ---------------------------------------------------------------------------

function volumeWeightedMethod(bars: Bar[], config: DetectionConfig, source: 'wick' | 'body'): RawTrendline[] {
  // Find pivots but weight by volume — high-volume pivots are more significant
  const avgVolume = bars.reduce((s, b) => s + b.volume, 0) / bars.length

  const pivots = detectPivots(bars, 5, source).filter(p => p.volume > avgVolume * 1.5)

  const lines: RawTrendline[] = []
  const highs = pivots.filter(p => p.type === 'high')
  const lows = pivots.filter(p => p.type === 'low')

  for (let i = 0; i < lows.length; i++) {
    for (let j = i + 1; j < Math.min(i + 6, lows.length); j++) {
      if (lows[j].index - lows[i].index < 5) continue
      const { slope, intercept } = lineFromPoints(lows[i].index, lows[i].price, lows[j].index, lows[j].price)
      lines.push({
        p1: { index: lows[i].index, time: lows[i].time, price: lows[i].price },
        p2: { index: lows[j].index, time: lows[j].time, price: lows[j].price },
        slope, intercept, direction: 'support', source, method: 'volume',
      })
    }
  }
  for (let i = 0; i < highs.length; i++) {
    for (let j = i + 1; j < Math.min(i + 6, highs.length); j++) {
      if (highs[j].index - highs[i].index < 5) continue
      const { slope, intercept } = lineFromPoints(highs[i].index, highs[i].price, highs[j].index, highs[j].price)
      lines.push({
        p1: { index: highs[i].index, time: highs[i].time, price: highs[i].price },
        p2: { index: highs[j].index, time: highs[j].time, price: highs[j].price },
        slope, intercept, direction: 'resistance', source, method: 'volume',
      })
    }
  }

  return lines
}

// ---------------------------------------------------------------------------
// Method 5: Touch Density (horizontal level finder)
// ---------------------------------------------------------------------------

function touchDensityMethod(bars: Bar[], config: DetectionConfig, source: 'wick' | 'body'): RawTrendline[] {
  // Find horizontal price levels where price frequently touches
  const priceMin = Math.min(...bars.map(b => b.low))
  const priceMax = Math.max(...bars.map(b => b.high))
  const range = priceMax - priceMin
  const bucketSize = range * 0.002 // 0.2% price buckets
  const buckets = new Map<number, { count: number; totalVolume: number; indices: number[] }>()

  for (let i = 0; i < bars.length; i++) {
    const prices = source === 'wick'
      ? [bars[i].high, bars[i].low]
      : [Math.max(bars[i].open, bars[i].close), Math.min(bars[i].open, bars[i].close)]

    for (const p of prices) {
      const bucket = Math.round(p / bucketSize)
      const entry = buckets.get(bucket) ?? { count: 0, totalVolume: 0, indices: [] }
      entry.count++
      entry.totalVolume += bars[i].volume
      entry.indices.push(i)
      buckets.set(bucket, entry)
    }
  }

  const lines: RawTrendline[] = []
  const sorted = Array.from(buckets.entries())
    .filter(([_, v]) => v.count >= config.minTouchCount)
    .sort((a, b) => b[1].count - a[1].count)
    .slice(0, 20)

  for (const [bucket, info] of sorted) {
    const price = bucket * bucketSize
    const firstIdx = Math.min(...info.indices)
    const lastIdx = Math.max(...info.indices)
    if (lastIdx - firstIdx < 10) continue

    // Horizontal line (slope = 0)
    lines.push({
      p1: { index: firstIdx, time: bars[firstIdx].time, price },
      p2: { index: lastIdx, time: bars[lastIdx].time, price },
      slope: 0,
      intercept: price,
      direction: price > (priceMin + priceMax) / 2 ? 'resistance' : 'support',
      source,
      method: 'density',
    })
  }

  return lines
}

// ---------------------------------------------------------------------------
// Backtesting Engine
// ---------------------------------------------------------------------------

function backtestLine(line: RawTrendline, bars: Bar[], config: DetectionConfig): BacktestResult {
  const tol = config.touchTolerance / 100
  const breakTol = config.breakThreshold / 100

  let touches = 0, bounces = 0, breaks = 0
  let avgBounceVol = 0, avgBreakVol = 0
  let maxConsecBounces = 0, consecBounces = 0
  let forwardTouches = 0
  let lastBreakIdx = -1

  for (let i = line.p1.index; i < bars.length; i++) {
    const expected = priceAt(line.slope, line.intercept, i)
    const high = bars[i].high
    const low = bars[i].low
    const close = bars[i].close

    const distHigh = Math.abs(high - expected) / expected
    const distLow = Math.abs(low - expected) / expected
    const minDist = Math.min(distHigh, distLow)

    if (minDist < tol) {
      touches++
      if (i > line.p2.index) forwardTouches++

      // Did it bounce (close on the expected side) or break through?
      const isBounce = line.direction === 'support'
        ? close > expected * (1 - breakTol)
        : close < expected * (1 + breakTol)

      if (isBounce) {
        bounces++
        avgBounceVol += bars[i].volume
        consecBounces++
        if (consecBounces > maxConsecBounces) maxConsecBounces = consecBounces
      } else {
        breaks++
        avgBreakVol += bars[i].volume
        consecBounces = 0
        lastBreakIdx = i
      }
    }
  }

  if (bounces > 0) avgBounceVol /= bounces
  if (breaks > 0) avgBreakVol /= breaks

  // Still valid if no decisive break in the last 20% of data
  const recentThreshold = Math.floor(bars.length * 0.8)
  const stillValid = lastBreakIdx < recentThreshold || lastBreakIdx === -1

  return {
    touches,
    bounces,
    breaks,
    bounceRate: touches > 0 ? bounces / touches : 0,
    avgBounceVolume: avgBounceVol,
    avgBreakVolume: avgBreakVol,
    maxConsecutiveBounces: maxConsecBounces,
    forwardTouchCount: forwardTouches,
    stillValid,
  }
}

// ---------------------------------------------------------------------------
// Strength Scoring
// ---------------------------------------------------------------------------

function scoreStrength(line: RawTrendline, bt: BacktestResult, bars: Bar[]): StrengthScore {
  // Touch score: 0-20 (2 touches = 4, 3 = 8, 4 = 12, 5+ = 16-20)
  const touchScore = Math.min(20, bt.touches * 4)

  // Bounce score: 0-25 (bounce rate × 25)
  const bounceScore = Math.round(bt.bounceRate * 25)

  // Span score: 0-15 (line spanning 50%+ of data gets full score)
  const span = line.p2.index - line.p1.index
  const spanScore = Math.min(15, Math.round((span / bars.length) * 30))

  // Angle score: 0-10 (near-horizontal = 10, steep = 0)
  const angleDeg = Math.abs(Math.atan(line.slope / (bars[0]?.close ?? 100)) * 180 / Math.PI)
  const angleScore = Math.max(0, Math.round(10 - angleDeg * 0.5))

  // Volume score: 0-10 (high volume at bounces)
  const avgVol = bars.reduce((s, b) => s + b.volume, 0) / bars.length
  const volumeScore = bt.avgBounceVolume > 0
    ? Math.min(10, Math.round((bt.avgBounceVolume / avgVol) * 5))
    : 0

  // Recency score: 0-10 (forward touches after p2 = still relevant)
  const recencyScore = Math.min(10, bt.forwardTouchCount * 3)

  // Validity score: 0-10 (still holding = 10, broken = 0)
  const validityScore = bt.stillValid ? 10 : 0

  const total = touchScore + bounceScore + spanScore + angleScore + volumeScore + recencyScore + validityScore

  return { total, touchScore, bounceScore, spanScore, angleScore, volumeScore, recencyScore, validityScore }
}

// ---------------------------------------------------------------------------
// Deduplication
// ---------------------------------------------------------------------------

function deduplicateLines(lines: ScoredTrendline[]): ScoredTrendline[] {
  const result: ScoredTrendline[] = []
  for (const line of lines) {
    const isDupe = result.some(existing => {
      // Same direction and similar slope
      if (existing.direction !== line.direction) return false
      const slopeDiff = Math.abs(existing.slope - line.slope)
      const avgPrice = (line.p1.price + line.p2.price) / 2
      const priceDiffP1 = Math.abs(existing.p1.price - line.p1.price) / avgPrice
      const priceDiffP2 = Math.abs(existing.p2.price - line.p2.price) / avgPrice
      return slopeDiff < 0.001 && priceDiffP1 < 0.015 && priceDiffP2 < 0.015
    })
    if (!isDupe) result.push(line)
  }
  return result
}

// ---------------------------------------------------------------------------
// Main Detection Pipeline
// ---------------------------------------------------------------------------

const COLORS: Record<string, Record<string, string>> = {
  pivot:   { support: '#2196f3', resistance: '#f44336' },
  regression: { support: '#00bcd4', resistance: '#ff9800' },
  fractal: { support: '#4caf50', resistance: '#e91e63' },
  volume:  { support: '#9c27b0', resistance: '#ff5722' },
  density: { support: '#607d8b', resistance: '#795548' },
}

const METHOD_LABELS: Record<string, string> = {
  pivot: 'Pivot', regression: 'Regression', fractal: 'Fractal',
  volume: 'Volume', density: 'Density',
}

export async function runAdvancedDetection(
  symbol: string,
  barsMap: Record<string, Bar[]>,
  config: DetectionConfig = DEFAULT_CONFIG,
): Promise<{ trendlines: number; methods: Record<string, number> }> {

  const totalBars = Object.values(barsMap).reduce((s, b) => s + b.length, 0)
  if (totalBars < 30) {
    console.warn(`Skipping detection for ${symbol}: insufficient data (${totalBars} bars)`)
    return { trendlines: 0, methods: {} }
  }

  // Clear old auto-trendlines
  await query('DELETE FROM annotations WHERE symbol = $1 AND source = $2', [symbol, 'auto-trend'])
  await invalidate(symbol)

  const annotations: Annotation[] = []
  const methodCounts: Record<string, number> = {}

  const TF_CONFIGS: { tf: string; label: string }[] = [
    { tf: '15m', label: '15m' },
    { tf: '30m', label: '30m' },
    { tf: '1h', label: '1H' },
    { tf: '4h', label: '4H' },
    { tf: '1d', label: '1D' },
    { tf: '1wk', label: '1W' },
  ]

  for (const { tf, label } of TF_CONFIGS) {
    const bars = barsMap[tf]
    if (!bars || bars.length < 30) continue

    for (const source of ['wick', 'body'] as const) {
      // Collect candidates from all enabled methods
      let candidates: RawTrendline[] = []

      if (config.methods.pivot) candidates.push(...pivotMethod(bars, config, source))
      if (config.methods.regression) candidates.push(...regressionMethod(bars, config, source))
      if (config.methods.fractal) candidates.push(...fractalMethod(bars, config, source))
      if (config.methods.volumeWeighted) candidates.push(...volumeWeightedMethod(bars, config, source))
      if (config.methods.touchDensity) candidates.push(...touchDensityMethod(bars, config, source))

      // Backtest and score all candidates
      const scored: ScoredTrendline[] = candidates.map(line => {
        const bt = backtestLine(line, bars, config)
        const strength = scoreStrength(line, bt, bars)
        return { ...line, backtest: bt, strength }
      })

      // Filter by minimum strength and touch count
      const filtered = scored.filter(s =>
        s.strength.total >= config.minStrength &&
        s.backtest.touches >= config.minTouchCount
      )

      // Sort by strength, deduplicate, take top N
      filtered.sort((a, b) => b.strength.total - a.strength.total)
      const deduped = deduplicateLines(filtered).slice(0, config.maxLines)

      for (const tl of deduped) {
        const color = COLORS[tl.method]?.[tl.direction] ?? '#888'
        methodCounts[tl.method] = (methodCounts[tl.method] ?? 0) + 1

        annotations.push({
          id: uuid(),
          symbol,
          source: 'auto-trend',
          type: 'trendline',
          points: [
            { time: tl.p1.time, price: tl.p1.price },
            { time: tl.p2.time, price: tl.p2.price },
          ],
          style: {
            color,
            opacity: Math.min(0.9, 0.3 + tl.strength.total / 100),
            lineStyle: source === 'body' ? 'dashed' : 'solid',
            thickness: Math.min(2.5, 0.5 + tl.strength.total / 40),
          },
          strength: tl.strength.total / 100,
          group: 'auto-trendlines',
          tags: [label, source, tl.direction, tl.method],
          visibility: ['*'],
          timeframe: tf,
          ttl: null,
          metadata: {
            label: `${label} ${source} ${METHOD_LABELS[tl.method] ?? tl.method}`,
            method: tl.method,
            direction: tl.direction,
            source,
            timeframeLabel: label,
            strength: tl.strength,
            backtest: tl.backtest,
            slope: tl.slope,
          },
          created_at: new Date().toISOString(),
          updated_at: new Date().toISOString(),
        })
      }
    }
  }

  // Persist
  for (const ann of annotations) {
    await query(
      `INSERT INTO annotations (id, symbol, source, type, points, style, strength, "group", tags, visibility, timeframe, metadata)
       VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)`,
      [ann.id, ann.symbol, ann.source, ann.type,
       JSON.stringify(ann.points), JSON.stringify(ann.style),
       ann.strength, ann.group, ann.tags, ann.visibility,
       ann.timeframe, JSON.stringify(ann.metadata)]
    )
    await publishSignal(symbol, ann)
  }

  await invalidate(symbol)
  console.info(`Advanced detection for ${symbol}: ${annotations.length} trendlines (${Object.entries(methodCounts).map(([k,v]) => `${k}:${v}`).join(' ')})`)
  return { trendlines: annotations.length, methods: methodCounts }
}
