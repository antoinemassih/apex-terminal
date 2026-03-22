import type { Timeframe } from '../types'

export const TF_TO_INTERVAL: Record<Timeframe, { interval: string; period: string; seconds: number }> = {
  '1m':  { interval: '1m',  period: '1d',  seconds: 60 },
  '5m':  { interval: '5m',  period: '5d',  seconds: 300 },
  '15m': { interval: '15m', period: '5d',  seconds: 900 },
  '30m': { interval: '30m', period: '5d',  seconds: 1800 },
  '1h':  { interval: '1h',  period: '1mo', seconds: 3600 },
  '4h':  { interval: '1h',  period: '3mo', seconds: 14400 },
  '1d':  { interval: '1d',  period: '1y',  seconds: 86400 },
  '1wk': { interval: '1wk', period: '5y',  seconds: 604800 },
}
