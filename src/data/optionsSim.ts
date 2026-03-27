// Black-Scholes options pricing engine for simulation

function normalCDF(x: number): number {
  const t = 1 / (1 + 0.2316419 * Math.abs(x))
  const poly = t * (0.319381530 + t * (-0.356563782 + t * (1.781477937 + t * (-1.821255978 + t * 1.330274429))))
  const phi = Math.exp(-0.5 * x * x) / Math.sqrt(2 * Math.PI)
  const cdf = 1 - phi * poly
  return x >= 0 ? cdf : 1 - cdf
}

function bs(S: number, K: number, T: number, r: number, σ: number, type: 'call' | 'put'): number {
  if (T <= 0) return type === 'call' ? Math.max(S - K, 0) : Math.max(K - S, 0)
  const d1 = (Math.log(S / K) + (r + 0.5 * σ * σ) * T) / (σ * Math.sqrt(T))
  const d2 = d1 - σ * Math.sqrt(T)
  if (type === 'call') return S * normalCDF(d1) - K * Math.exp(-r * T) * normalCDF(d2)
  return K * Math.exp(-r * T) * normalCDF(-d2) - S * normalCDF(-d1)
}

function bsDelta(S: number, K: number, T: number, r: number, σ: number, type: 'call' | 'put'): number {
  if (T <= 0) {
    if (type === 'call') return S > K ? 1 : 0
    return S < K ? -1 : 0
  }
  const d1 = (Math.log(S / K) + (r + 0.5 * σ * σ) * T) / (σ * Math.sqrt(T))
  return type === 'call' ? normalCDF(d1) : normalCDF(d1) - 1
}

function bsGamma(S: number, K: number, T: number, r: number, σ: number): number {
  if (T <= 0) return 0
  const d1 = (Math.log(S / K) + (r + 0.5 * σ * σ) * T) / (σ * Math.sqrt(T))
  return Math.exp(-0.5 * d1 * d1) / (Math.sqrt(2 * Math.PI) * S * σ * Math.sqrt(T))
}

function bsTheta(S: number, K: number, T: number, r: number, σ: number, type: 'call' | 'put'): number {
  if (T <= 0) return 0
  const d1 = (Math.log(S / K) + (r + 0.5 * σ * σ) * T) / (σ * Math.sqrt(T))
  const d2 = d1 - σ * Math.sqrt(T)
  const nd1 = Math.exp(-0.5 * d1 * d1) / Math.sqrt(2 * Math.PI)
  const t1 = -(S * nd1 * σ) / (2 * Math.sqrt(T))
  if (type === 'call') return (t1 - r * K * Math.exp(-r * T) * normalCDF(d2)) / 365
  return (t1 + r * K * Math.exp(-r * T) * normalCDF(-d2)) / 365
}

export function getStrikeInterval(price: number): number {
  if (price < 20) return 0.5
  if (price < 50) return 1
  if (price < 100) return 2.5
  if (price < 200) return 5
  if (price < 500) return 10
  return 25
}

export function getATMStrike(price: number): number {
  const interval = getStrikeInterval(price)
  return Math.round(price / interval) * interval
}

// Vol surface: smile + put skew + near-term premium
function getIV(S: number, K: number, dte: number): number {
  const base = 0.28
  const moneyness = Math.log(K / S)
  const smile = 0.06 * moneyness * moneyness
  const skew = -0.05 * moneyness   // negative skew: higher IV for OTM puts
  const term = dte <= 0 ? 1.25 : dte === 1 ? 1.10 : 1.0
  return Math.max(0.05, (base + smile + skew) * term)
}

// Next N trading days from today
export function tradingDate(daysAhead: number): string {
  const d = new Date()
  let count = 0
  // If today is a weekday and daysAhead===0, return today
  while (count < daysAhead) {
    d.setDate(d.getDate() + 1)
    const day = d.getDay()
    if (day !== 0 && day !== 6) count++
  }
  // If 0DTE requested and today is weekend, still return today
  return d.toISOString().split('T')[0]
}

export function tradingDateLabel(daysAhead: number): string {
  const months = ['Jan','Feb','Mar','Apr','May','Jun','Jul','Aug','Sep','Oct','Nov','Dec']
  const iso = tradingDate(daysAhead)
  const d = new Date(iso + 'T12:00:00')
  return `${months[d.getMonth()]} ${d.getDate()}`
}

export interface OptionRow {
  strike: number
  type: 'call' | 'put'
  expiry: string    // YYYY-MM-DD
  dte: number
  bid: number
  ask: number
  mid: number
  iv: number
  delta: number
  gamma: number
  theta: number
  oi: number        // open interest (simulated)
  isATM: boolean
}

export interface OptionChain {
  expiry: string
  dte: number
  calls: OptionRow[]  // most OTM first, ATM last
  puts: OptionRow[]   // ATM first, most OTM last
}

export interface SavedOption {
  key: string
  symbol: string
  strike: number
  type: 'call' | 'put'
  expiry: string
  dte: number
}

export function optionKey(symbol: string, row: Pick<OptionRow, 'strike' | 'type' | 'expiry'>): string {
  return `${symbol}:${row.expiry}:${row.strike}:${row.type}`
}

export interface PricedOption {
  bid: number; ask: number; mid: number; delta: number; oi: number
}

// Simulated OI: bell curve around ATM, higher for further DTE
function simOI(underlying: number, strike: number, dte: number): number {
  const interval = getStrikeInterval(underlying)
  const atm = getATMStrike(underlying)
  const strikesAway = Math.abs(strike - atm) / interval
  const base = dte <= 0 ? 18000 : dte === 1 ? 35000 : 50000
  // Bell curve: OI falls off with distance from ATM
  const raw = base * Math.exp(-0.35 * strikesAway * strikesAway)
  // Add deterministic noise based on strike to make each row look distinct
  const noise = 1 + 0.3 * Math.sin(strike * 17.3 + dte * 5.7)
  return Math.max(100, Math.round(raw * noise))
}

export function priceOption(underlying: number, strike: number, type: 'call' | 'put', dte: number): PricedOption {
  const r = 0.05
  const T = dte === 0 ? 0.5 / 252 : dte / 252
  const iv = getIV(underlying, strike, dte)
  const mid = Math.max(0, bs(underlying, strike, T, r, iv, type))
  const spread = Math.max(0.01, mid * 0.04 + 0.005)
  return {
    bid: Math.max(0, parseFloat((mid - spread / 2).toFixed(2))),
    ask: parseFloat((mid + spread / 2).toFixed(2)),
    mid: parseFloat(mid.toFixed(2)),
    delta: parseFloat(bsDelta(underlying, strike, T, r, iv, type).toFixed(3)),
    oi: simOI(underlying, strike, dte),
  }
}

export function buildChain(underlying: number, numStrikes: number, dte: number): OptionChain {
  const r = 0.05
  const T = dte === 0 ? 0.5 / 252 : dte / 252   // 0DTE ≈ half-day remaining
  const interval = getStrikeInterval(underlying)
  const atm = getATMStrike(underlying)
  const expiry = tradingDate(dte)

  const makeRow = (K: number, type: 'call' | 'put', isATM: boolean): OptionRow => {
    const iv = getIV(underlying, K, dte)
    const raw = bs(underlying, K, T, r, iv, type)
    const spread = Math.max(0.01, raw * 0.04 + 0.005)
    const mid = Math.max(0, parseFloat(raw.toFixed(2)))
    return {
      strike: K, type, expiry, dte, isATM,
      bid: Math.max(0, parseFloat((raw - spread / 2).toFixed(2))),
      ask: parseFloat((raw + spread / 2).toFixed(2)),
      mid,
      iv: parseFloat(iv.toFixed(4)),
      delta: parseFloat(bsDelta(underlying, K, T, r, iv, type).toFixed(3)),
      gamma: parseFloat(bsGamma(underlying, K, T, r, iv).toFixed(5)),
      theta: parseFloat(bsTheta(underlying, K, T, r, iv, type).toFixed(3)),
      oi: simOI(underlying, K, dte),
    }
  }

  // Calls: most OTM at top (highest strike), ATM at bottom
  const calls: OptionRow[] = []
  for (let i = numStrikes; i >= 1; i--) calls.push(makeRow(atm + i * interval, 'call', false))
  calls.push(makeRow(atm, 'call', true))

  // Puts: ATM at top, most OTM at bottom
  const puts: OptionRow[] = []
  puts.push(makeRow(atm, 'put', true))
  for (let i = 1; i <= numStrikes; i++) puts.push(makeRow(atm - i * interval, 'put', false))

  return { expiry, dte, calls, puts }
}
