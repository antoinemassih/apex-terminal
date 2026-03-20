export function sma(closes: Float64Array, period: number): Float64Array {
  const out = new Float64Array(closes.length)
  let sum = 0
  for (let i = 0; i < closes.length; i++) {
    sum += closes[i]
    if (i >= period) sum -= closes[i - period]
    out[i] = i >= period - 1 ? sum / period : NaN
  }
  return out
}

export function ema(closes: Float64Array, period: number): Float64Array {
  const out = new Float64Array(closes.length)
  const k = 2 / (period + 1)
  out[0] = closes[0]
  for (let i = 1; i < closes.length; i++) {
    out[i] = closes[i] * k + out[i - 1] * (1 - k)
  }
  for (let i = 0; i < period - 1; i++) out[i] = NaN
  return out
}
