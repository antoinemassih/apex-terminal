import { describe, it, expect, vi } from 'vitest'
import { IndicatorEngine } from '../indicators/IndicatorEngine'

function makeStore(length: number) {
  const closes = new Float64Array(length)
  for (let i = 0; i < length; i++) closes[i] = 100 + Math.sin(i * 0.1) * 20
  return { closes, length }
}

describe('IndicatorEngine', () => {
  it('bootstrap produces valid snapshot', () => {
    const engine = new IndicatorEngine()
    const store = makeStore(200)
    const snapshot = engine.bootstrap('AAPL', '5m', store)

    expect(snapshot.sma20).toBeInstanceOf(Float64Array)
    expect(snapshot.ema50).toBeInstanceOf(Float64Array)
    expect(snapshot.bbUpper).toBeInstanceOf(Float64Array)
    expect(snapshot.bbLower).toBeInstanceOf(Float64Array)
    // First 19 SMA values should be NaN (period=20)
    expect(isNaN(snapshot.sma20[18])).toBe(true)
    expect(isNaN(snapshot.sma20[19])).toBe(false)
  })

  it('onTick creates new indicator values', () => {
    const engine = new IndicatorEngine()
    const store = makeStore(100)
    engine.bootstrap('AAPL', '5m', store)

    const snapshot = engine.onTick('AAPL', '5m', 125, 'created')
    // Should have one more data point
    expect(snapshot.sma20.length).toBeGreaterThanOrEqual(101)
  })

  it('onTick updates last indicator value', () => {
    const engine = new IndicatorEngine()
    const store = makeStore(100)
    engine.bootstrap('AAPL', '5m', store)

    const snap1 = engine.onTick('AAPL', '5m', 130, 'updated')
    // Capture value before second update (same underlying buffer is reused)
    const sma20AfterFirst = snap1.sma20[99]
    const snap2 = engine.onTick('AAPL', '5m', 140, 'updated')
    // Last SMA should differ between updates since price changed
    expect(snap2.sma20[99]).not.toBe(sma20AfterFirst)
  })

  it('subscribe notifies on tick', () => {
    const engine = new IndicatorEngine()
    const store = makeStore(100)
    engine.bootstrap('AAPL', '5m', store)

    const cb = vi.fn()
    engine.subscribe('AAPL', '5m', cb)
    engine.onTick('AAPL', '5m', 130, 'created')
    expect(cb).toHaveBeenCalledTimes(1)
  })

  it('remove cleans up state', () => {
    const engine = new IndicatorEngine()
    const store = makeStore(100)
    engine.bootstrap('AAPL', '5m', store)
    engine.remove('AAPL', '5m')
    expect(engine.getSnapshot('AAPL', '5m')).toBeNull()
  })

  it('throws on onTick without bootstrap', () => {
    const engine = new IndicatorEngine()
    expect(() => engine.onTick('AAPL', '5m', 100, 'created')).toThrow()
  })
})
