import { useState, useEffect, useCallback } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { ColumnStore } from '../data/columns'
import { CoordSystem } from './CoordSystem'
import type { Bar, Timeframe } from '../types'

const TF_TO_INTERVAL: Record<Timeframe, { interval: string; period: string }> = {
  '1m':  { interval: '1m',  period: '1d' },
  '5m':  { interval: '5m',  period: '5d' },
  '15m': { interval: '15m', period: '5d' },
  '1h':  { interval: '1h',  period: '1mo' },
  '4h':  { interval: '1h',  period: '3mo' },
  '1d':  { interval: '1d',  period: '1y' },
  '1wk': { interval: '1wk', period: '5y' },
}

export function useChartData(symbol: string, timeframe: Timeframe, width: number, height: number) {
  const [data, setData] = useState<ColumnStore | null>(null)
  const [viewStart, setViewStart] = useState(0)
  const [viewCount, setViewCount] = useState(200)
  const [cs, setCs] = useState<CoordSystem | null>(null)

  useEffect(() => {
    const tf = TF_TO_INTERVAL[timeframe] ?? TF_TO_INTERVAL['5m']
    invoke<Bar[]>('get_bars', { symbol, interval: tf.interval, period: tf.period })
      .then(bars => {
        const store = ColumnStore.fromBars(bars)
        setData(store)
        setViewStart(Math.max(0, store.length - 200))
        setViewCount(Math.min(200, store.length))
      })
      .catch(err => console.error('Failed to load bars:', err))
  }, [symbol, timeframe])

  useEffect(() => {
    if (!data || width === 0 || height === 0) return
    const end = Math.min(viewStart + viewCount, data.length)
    const count = end - viewStart
    if (count <= 0) return
    const { min: minP, max: maxP } = data.priceRange(viewStart, end)
    const pad = (maxP - minP) * 0.05
    setCs(new CoordSystem({
      width, height, barCount: count,
      minPrice: minP - pad, maxPrice: maxP + pad,
    }))
  }, [data, viewStart, viewCount, width, height])

  const pan = useCallback((deltaPixels: number) => {
    if (!data || !cs) return
    const barDelta = Math.round(deltaPixels / cs.barStep)
    setViewStart(v => Math.max(0, Math.min(data.length - viewCount, v - barDelta)))
  }, [data, cs, viewCount])

  const zoom = useCallback((factor: number) => {
    if (!data) return
    setViewCount(v => {
      const newCount = Math.max(20, Math.min(data.length, Math.round(v * factor)))
      setViewStart(s => Math.max(0, Math.min(data.length - newCount, s + Math.round((v - newCount) / 2))))
      return newCount
    })
  }, [data])

  return { data, cs, viewStart, viewCount, pan, zoom }
}
