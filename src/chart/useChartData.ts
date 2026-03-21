import { useState, useEffect, useCallback, useRef } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { ColumnStore } from '../data/columns'
import { CoordSystem } from './CoordSystem'
import { useChartStore } from '../store/chartStore'
import type { Bar, Timeframe } from '../types'

const TF_TO_INTERVAL: Record<Timeframe, { interval: string; period: string; secs: number }> = {
  '1m':  { interval: '1m',  period: '1d',  secs: 60 },
  '5m':  { interval: '5m',  period: '5d',  secs: 300 },
  '15m': { interval: '15m', period: '5d',  secs: 900 },
  '1h':  { interval: '1h',  period: '1mo', secs: 3600 },
  '4h':  { interval: '1h',  period: '3mo', secs: 14400 },
  '1d':  { interval: '1d',  period: '1y',  secs: 86400 },
  '1wk': { interval: '1wk', period: '5y',  secs: 604800 },
}

// Extra empty bars to the right of the last candle so it's clearly visible
const RIGHT_MARGIN_BARS = 8
const AUTO_SCROLL_TIMEOUT = 10_000

export function useChartData(symbol: string, timeframe: Timeframe, width: number, height: number) {
  const [data, setData] = useState<ColumnStore | null>(null)
  const [viewStart, setViewStart] = useState(0)
  const [viewCount, setViewCount] = useState(200)
  const [cs, setCs] = useState<CoordSystem | null>(null)
  const [priceOverride, setPriceOverride] = useState<{ min: number; max: number } | null>(null)
  const [tickVersion, setTickVersion] = useState(0)
  const [autoScrolling, setAutoScrolling] = useState(true)
  const dataRef = useRef<ColumnStore | null>(null)
  const idleTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const autoScrollVersion = useChartStore(s => s.autoScrollVersion)

  // Global reset button → force auto-scroll back on
  useEffect(() => {
    setAutoScrolling(true)
    setPriceOverride(null)
  }, [autoScrollVersion])

  // User interaction → pause auto-scroll for 10s
  const pauseAutoScroll = useCallback(() => {
    setAutoScrolling(false)
    if (idleTimerRef.current) clearTimeout(idleTimerRef.current)
    idleTimerRef.current = setTimeout(() => {
      setAutoScrolling(true)
      setPriceOverride(null) // also reset Y zoom on auto-resume
    }, AUTO_SCROLL_TIMEOUT)
  }, [])

  // Cleanup timer
  useEffect(() => () => {
    if (idleTimerRef.current) clearTimeout(idleTimerRef.current)
  }, [])

  // Load data
  useEffect(() => {
    const tf = TF_TO_INTERVAL[timeframe] ?? TF_TO_INTERVAL['5m']
    invoke<Bar[]>('get_bars', { symbol, interval: tf.interval, period: tf.period })
      .then(bars => {
        const store = ColumnStore.fromBars(bars)
        dataRef.current = store
        setData(store)
        setViewStart(Math.max(0, store.length - 200))
        setViewCount(Math.min(200, store.length))
        setPriceOverride(null)
        setAutoScrolling(true)
      })
      .catch(err => console.error('Failed to load bars:', err))
  }, [symbol, timeframe])

  // Tick simulation
  useEffect(() => {
    if (!dataRef.current) return
    const tf = TF_TO_INTERVAL[timeframe] ?? TF_TO_INTERVAL['5m']
    let simTime = dataRef.current.times[dataRef.current.length - 1]
    let tickCount = 0

    const interval = setInterval(() => {
      const store = dataRef.current
      if (!store || store.length === 0) return

      const lastClose = store.closes[store.length - 1]
      const change = lastClose * (Math.random() - 0.495) * 0.003
      const newPrice = Math.max(0.01, lastClose + change)
      const volume = Math.random() * 500

      tickCount++
      if (tickCount % 20 === 0) {
        simTime += tf.secs
      } else {
        simTime += tf.secs / 20
      }

      store.applyTick(newPrice, volume, simTime, tf.secs)
      setTickVersion(v => v + 1)
    }, 250)

    return () => clearInterval(interval)
  }, [data, timeframe])

  // Auto-scroll: pin viewStart so latest candle is visible
  useEffect(() => {
    if (!autoScrolling || !dataRef.current) return
    const store = dataRef.current
    const maxStart = Math.max(0, store.length - viewCount + RIGHT_MARGIN_BARS)
    setViewStart(maxStart)
  }, [autoScrolling, tickVersion, viewCount])

  // Recompute coordinate system
  useEffect(() => {
    const store = dataRef.current
    if (!store || width === 0 || height === 0) return

    // barCount includes margin bars so there's empty space on the right
    const end = Math.min(viewStart + viewCount, store.length)
    const dataBars = end - viewStart
    if (dataBars <= 0) return
    const totalBars = dataBars + RIGHT_MARGIN_BARS

    let minP: number, maxP: number
    if (priceOverride) {
      minP = priceOverride.min
      maxP = priceOverride.max
    } else {
      const range = store.priceRange(viewStart, end)
      const pad = (range.max - range.min) * 0.05
      minP = range.min - pad
      maxP = range.max + pad
    }

    setCs(new CoordSystem({
      width, height, barCount: totalBars,
      minPrice: minP, maxPrice: maxP,
    }))
  }, [dataRef.current, viewStart, viewCount, width, height, priceOverride, tickVersion])

  // Pan (X axis) — pauses auto-scroll
  const pan = useCallback((deltaPixels: number) => {
    const store = dataRef.current
    if (!store || !cs) return
    const barDelta = Math.round(deltaPixels / cs.barStep)
    if (barDelta === 0) return
    pauseAutoScroll()
    setViewStart(v => Math.max(0, Math.min(store.length - viewCount + RIGHT_MARGIN_BARS, v - barDelta)))
  }, [cs, viewCount, pauseAutoScroll])

  // Zoom X — pauses auto-scroll
  const zoomX = useCallback((factor: number) => {
    const store = dataRef.current
    if (!store) return
    pauseAutoScroll()
    setViewCount(v => {
      const newCount = Math.max(20, Math.min(store.length, Math.round(v * factor)))
      setViewStart(s => Math.max(0, Math.min(store.length - newCount + RIGHT_MARGIN_BARS, s + Math.round((v - newCount) / 2))))
      return newCount
    })
  }, [pauseAutoScroll])

  // Zoom Y — pauses auto-scroll
  const zoomY = useCallback((factor: number, anchorPrice?: number) => {
    if (!cs) return
    pauseAutoScroll()
    const center = anchorPrice ?? (cs.minPrice + cs.maxPrice) / 2
    const halfRange = ((cs.maxPrice - cs.minPrice) / 2) * factor
    setPriceOverride({ min: center - halfRange, max: center + halfRange })
  }, [cs, pauseAutoScroll])

  // Pan Y — pauses auto-scroll
  const panY = useCallback((deltaPixels: number) => {
    if (!cs) return
    pauseAutoScroll()
    const pricePerPixel = (cs.maxPrice - cs.minPrice) / cs.chartHeight
    const priceDelta = deltaPixels * pricePerPixel
    setPriceOverride({ min: cs.minPrice + priceDelta, max: cs.maxPrice + priceDelta })
  }, [cs, pauseAutoScroll])

  const resetYZoom = useCallback(() => {
    setPriceOverride(null)
  }, [])

  return {
    data: dataRef.current, cs, viewStart, viewCount,
    pan, zoomX, zoomY, panY, resetYZoom, tickVersion,
    autoScrolling, pauseAutoScroll,
  }
}
