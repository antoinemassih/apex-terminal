export interface IndicatorOutput {
  /** Display name shown in UI */
  name: string
  /** Parent indicator ID from the registry (e.g., 'sma20', 'bollinger') */
  indicatorId: string
  /** Unique key for this output (e.g., 'sma20', 'bbUpper') */
  key: string
  /** Line color [r, g, b, a] in 0-1 range */
  color: [number, number, number, number]
  /** Line width in pixels */
  width: number
  /** The computed values */
  values: Float64Array
}

export interface IncrementalIndicator {
  readonly id: string
  readonly name: string
  bootstrap(closes: Float64Array, length: number): void
  push(value: number): void
  updateLast(value: number): void
  getOutputs(): IndicatorOutput[]
  getLength(): number
}

// Legacy — kept for backward compat during migration, will be removed
export interface IndicatorSnapshot {
  sma20: Float64Array
  ema50: Float64Array
  bbUpper: Float64Array
  bbLower: Float64Array
  [key: string]: Float64Array  // dynamic indicators
}
