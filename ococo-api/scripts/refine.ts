/**
 * Trendline refinement script — tests multiple configurations,
 * compares quality metrics, and outputs the optimal settings.
 */

import { runAdvancedDetection, DEFAULT_CONFIG, type DetectionConfig } from '../src/trendlines-v2.js'
import { query, pool } from '../src/db.js'

const YFINANCE = 'http://127.0.0.1:8777'

interface Bar {
  time: number; open: number; high: number; low: number; close: number; volume: number
}

async function fetchBars(symbol: string, interval: string, period: string): Promise<Bar[]> {
  const r = await fetch(`${YFINANCE}/bars?symbol=${symbol}&interval=${interval}&period=${period}`)
  return r.json() as Promise<Bar[]>
}

function aggregate4h(bars: Bar[]): Bar[] {
  const result: Bar[] = []
  for (let i = 0; i < bars.length; i += 4) {
    const c = bars.slice(i, i + 4)
    if (!c.length) continue
    result.push({ time: c[0].time, open: c[0].open, high: Math.max(...c.map(b => b.high)), low: Math.min(...c.map(b => b.low)), close: c[c.length - 1].close, volume: c.reduce((s, b) => s + b.volume, 0) })
  }
  return result
}

async function loadBars(symbol: string): Promise<Record<string, Bar[]>> {
  const barsMap: Record<string, Bar[]> = {}
  const configs = [
    { tf: '1h', interval: '1h', period: '1mo' },
    { tf: '4h', interval: '1h', period: '3mo' },
    { tf: '1d', interval: '1d', period: '1y' },
    { tf: '1wk', interval: '1wk', period: '5y' },
  ]
  for (const c of configs) {
    const bars = await fetchBars(symbol, c.interval, c.period)
    if (c.tf === '4h') barsMap[c.tf] = aggregate4h(bars)
    else barsMap[c.tf] = bars
  }
  return barsMap
}

async function evaluate(symbol: string): Promise<{ count: number; avgStr: number; avgBounce: number; high: number; med: number; low: number }> {
  const rows = await query('SELECT metadata FROM annotations WHERE symbol = $1 AND source = $2', [symbol, 'auto-trend'])
  const strengths = rows.rows.map((r: any) => r.metadata?.strength?.total ?? 0)
  const bounces = rows.rows.map((r: any) => r.metadata?.backtest?.bounceRate ?? 0)
  const count = strengths.length
  const avgStr = count ? Math.round(strengths.reduce((a: number, b: number) => a + b, 0) / count) : 0
  const avgBounce = count ? Math.round(bounces.reduce((a: number, b: number) => a + b, 0) / count * 100) : 0
  return {
    count,
    avgStr,
    avgBounce,
    high: strengths.filter((s: number) => s >= 60).length,
    med: strengths.filter((s: number) => s >= 30 && s < 60).length,
    low: strengths.filter((s: number) => s < 30).length,
  }
}

async function main() {
  const symbol = 'AAPL'
  console.log(`Loading bars for ${symbol}...`)
  const barsMap = await loadBars(symbol)
  console.log(`Loaded: ${Object.entries(barsMap).map(([k, v]) => `${k}:${v.length}`).join(' ')}`)

  const configs: { name: string; config: DetectionConfig }[] = [
    { name: 'DEFAULT                    ', config: { ...DEFAULT_CONFIG } },
    { name: 'STRICT (str=30,touch=3)    ', config: { ...DEFAULT_CONFIG, minStrength: 30, minTouchCount: 3 } },
    { name: 'VERY_STRICT (str=50,touch=4)', config: { ...DEFAULT_CONFIG, minStrength: 50, minTouchCount: 4 } },
    { name: 'LOOSE (str=10,touch=2)     ', config: { ...DEFAULT_CONFIG, minStrength: 10, minTouchCount: 2 } },
    { name: 'TIGHT_TOL (0.15%)          ', config: { ...DEFAULT_CONFIG, touchTolerance: 0.15 } },
    { name: 'WIDE_TOL (0.5%)            ', config: { ...DEFAULT_CONFIG, touchTolerance: 0.5 } },
    { name: 'MANY_LOOKBACKS (2-34)      ', config: { ...DEFAULT_CONFIG, pivotLookbacks: [2, 3, 5, 8, 13, 21, 34] } },
    { name: 'FEW_LOOKBACKS (5,13)       ', config: { ...DEFAULT_CONFIG, pivotLookbacks: [5, 13] } },
    { name: 'HIGH_MAX (50 lines)        ', config: { ...DEFAULT_CONFIG, maxLines: 50 } },
    { name: 'LOW_MAX (10 lines)         ', config: { ...DEFAULT_CONFIG, maxLines: 10 } },
    { name: 'OPTIMAL_CANDIDATE          ', config: { ...DEFAULT_CONFIG, minStrength: 25, minTouchCount: 3, touchTolerance: 0.3, maxLines: 25, pivotLookbacks: [3, 5, 8, 13, 21] } },
  ]

  console.log('\n' + 'Config'.padEnd(35) + 'Lines  AvgStr  Bounce%  High  Med  Low')
  console.log('-'.repeat(80))

  let bestName = ''
  let bestScore = 0

  for (const { name, config } of configs) {
    await runAdvancedDetection(symbol, barsMap, config)
    const e = await evaluate(symbol)
    // Quality score: prefer high avg strength + high bounce rate + reasonable count
    const qualityScore = e.avgStr * 0.4 + e.avgBounce * 0.3 + Math.min(30, e.count) * 0.3
    const line = `${name} ${String(e.count).padStart(5)}  ${String(e.avgStr).padStart(6)}  ${String(e.avgBounce).padStart(7)}  ${String(e.high).padStart(4)}  ${String(e.med).padStart(3)}  ${String(e.low).padStart(3)}  Q=${qualityScore.toFixed(1)}`
    console.log(line)
    if (qualityScore > bestScore) { bestScore = qualityScore; bestName = name.trim() }
  }

  console.log('\n' + '='.repeat(80))
  console.log(`BEST CONFIG: ${bestName} (quality=${bestScore.toFixed(1)})`)

  // Now test the best config across multiple symbols
  console.log('\n--- Cross-validation across symbols ---')
  const bestConfig = configs.find(c => c.name.trim() === bestName)!.config
  const symbols = ['MSFT', 'NVDA', 'TSLA', 'SPY']

  for (const sym of symbols) {
    const bars = await loadBars(sym)
    await runAdvancedDetection(sym, bars, bestConfig)
    const e = await evaluate(sym)
    console.log(`${sym}: ${e.count} lines, avgStr=${e.avgStr}, bounce=${e.avgBounce}%, high=${e.high}`)
  }

  await pool.end()
}

main().catch(e => { console.error(e); process.exit(1) })
