import type { IncrementalIndicator, IndicatorOutput } from './types'
import { IncrementalSMA } from './incremental/sma'
import { IncrementalEMA } from './incremental/ema'
import { IncrementalBollinger } from './incremental/bollinger'

const DEFAULT_CAPACITY = 2048

/** Wraps IncrementalSMA into the IncrementalIndicator interface */
class SMAIndicator implements IncrementalIndicator {
  readonly id: string
  readonly name: string
  private sma: IncrementalSMA

  constructor(
    private period: number,
    private color: [number, number, number, number],
    private lineWidth: number,
  ) {
    this.id = `sma${period}`
    this.name = `SMA ${period}`
    this.sma = new IncrementalSMA(period, DEFAULT_CAPACITY)
  }

  bootstrap(closes: Float64Array, length: number): void {
    this.sma = new IncrementalSMA(this.period, Math.max(length + 512, DEFAULT_CAPACITY))
    this.sma.bootstrap(closes, length)
  }

  push(value: number): void { this.sma.push(value) }
  updateLast(value: number): void { this.sma.updateLast(value) }
  getLength(): number { return this.sma.getLength() }

  getOutputs(): IndicatorOutput[] {
    return [{
      name: this.name,
      indicatorId: this.id,
      key: this.id,
      color: this.color,
      width: this.lineWidth,
      values: this.sma.getOutput(),
    }]
  }
}

/** Wraps IncrementalEMA into the IncrementalIndicator interface */
class EMAIndicator implements IncrementalIndicator {
  readonly id: string
  readonly name: string
  private ema: IncrementalEMA

  constructor(
    private period: number,
    private color: [number, number, number, number],
    private lineWidth: number,
  ) {
    this.id = `ema${period}`
    this.name = `EMA ${period}`
    this.ema = new IncrementalEMA(period, DEFAULT_CAPACITY)
  }

  bootstrap(closes: Float64Array, length: number): void {
    this.ema = new IncrementalEMA(this.period, Math.max(length + 512, DEFAULT_CAPACITY))
    this.ema.bootstrap(closes, length)
  }

  push(value: number): void { this.ema.push(value) }
  updateLast(value: number): void { this.ema.updateLast(value) }
  getLength(): number { return this.ema.getLength() }

  getOutputs(): IndicatorOutput[] {
    return [{
      name: this.name,
      indicatorId: this.id,
      key: this.id,
      color: this.color,
      width: this.lineWidth,
      values: this.ema.getOutput(),
    }]
  }
}

/** Wraps IncrementalBollinger into the IncrementalIndicator interface (produces two outputs) */
class BollingerIndicator implements IncrementalIndicator {
  readonly id = 'bollinger'
  readonly name = 'Bollinger Bands'
  private bb: IncrementalBollinger

  constructor(
    private period: number,
    private stdDevs: number,
    private color: [number, number, number, number],
    private lineWidth: number,
  ) {
    this.bb = new IncrementalBollinger(period, stdDevs, DEFAULT_CAPACITY)
  }

  bootstrap(closes: Float64Array, length: number): void {
    this.bb = new IncrementalBollinger(this.period, this.stdDevs, Math.max(length + 512, DEFAULT_CAPACITY))
    this.bb.bootstrap(closes, length)
  }

  push(value: number): void { this.bb.push(value) }
  updateLast(value: number): void { this.bb.updateLast(value) }
  getLength(): number { return this.bb.getLength() }

  getOutputs(): IndicatorOutput[] {
    return [
      { name: 'BB Upper', indicatorId: this.id, key: 'bbUpper', color: this.color, width: this.lineWidth, values: this.bb.getUpper() },
      { name: 'BB Lower', indicatorId: this.id, key: 'bbLower', color: this.color, width: this.lineWidth, values: this.bb.getLower() },
    ]
  }
}

export type IndicatorFactory = () => IncrementalIndicator

/** Registry of available indicators. Use getDefaultIndicatorIds() for the standard set. */
export const INDICATOR_CATALOG: Record<string, { name: string; factory: IndicatorFactory }> = {
  sma20:     { name: 'SMA 20',    factory: () => new SMAIndicator(20, [0.3, 0.6, 1.0, 0.8], 1.0) },
  ema50:     { name: 'EMA 50',    factory: () => new EMAIndicator(50, [1.0, 0.6, 0.2, 0.8], 1.0) },
  bollinger: { name: 'Bollinger', factory: () => new BollingerIndicator(20, 2, [0.5, 0.5, 0.5, 0.4], 0.5) },
}

export function getDefaultIndicatorIds(): string[] {
  return ['sma20', 'ema50', 'bollinger']
}
