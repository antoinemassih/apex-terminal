import { useState, useEffect, useCallback, useRef, useMemo } from 'react'
import { CoordSystem } from '../engine'
import { useChartStore } from '../store/chartStore'
import { getDataStore } from '../globals'
import type { Timeframe } from '../types'

const RIGHT_MARGIN_BARS = 8
const AUTO_SCROLL_TIMEOUT = 10_000

export interface Viewport {
  viewStart: number
  viewCount: number
  cs: CoordSystem | null
}

export function useChartViewport(symbol: string, timeframe: Timeframe, width: number, height: number) {
  const [viewStart, setViewStart] = useState(0)
  const [viewCount, setViewCount] = useState(200)
  const [priceOverride, setPriceOverride] = useState<{ min: number; max: number } | null>(null)
  const [cs, setCs] = useState<CoordSystem | null>(null)
  const [autoScrolling, setAutoScrolling] = useState(true)
  const [dataVersion, setDataVersion] = useState(0)
  const idleTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const autoScrollVersion = useChartStore(s => s.autoScrollVersion)

  // Smooth scroll: track wall-clock time of last candle creation
  const smoothRafRef = useRef<number | null>(null)
  const lastCandleRealTimeRef = useRef(performance.now())
  const lastBarCountRef = useRef(0)
  // Tick interval in real ms between candle boundaries (measured, not computed)
  const candleIntervalMsRef = useRef(1000) // will be measured from actual data

  // Global reset → force auto-scroll
  useEffect(() => {
    setAutoScrolling(true)
    setPriceOverride(null)
  }, [autoScrollVersion])

  // Pause auto-scroll on interaction
  const pauseAutoScroll = useCallback(() => {
    setAutoScrolling(false)
    if (idleTimerRef.current) clearTimeout(idleTimerRef.current)
    idleTimerRef.current = setTimeout(() => {
      setAutoScrolling(true)
      setPriceOverride(null)
    }, AUTO_SCROLL_TIMEOUT)
  }, [])

  useEffect(() => () => {
    if (idleTimerRef.current) clearTimeout(idleTimerRef.current)
    if (smoothRafRef.current) cancelAnimationFrame(smoothRafRef.current)
  }, [])

  // Subscribe to data changes
  useEffect(() => {
    const ds = getDataStore()
    return ds.subscribe(symbol, timeframe, () => {
      setDataVersion(v => v + 1)
      // Detect new candle creation → record wall-clock time
      const data = ds.getData(symbol, timeframe)
      if (data && data.length !== lastBarCountRef.current) {
        const now = performance.now()
        if (lastBarCountRef.current > 0) {
          // Measure actual real-time interval between candle creations
          candleIntervalMsRef.current = now - lastCandleRealTimeRef.current
        }
        lastCandleRealTimeRef.current = now
        lastBarCountRef.current = data.length
      }
    })
  }, [symbol, timeframe])

  // Auto-scroll: snap viewStart to end of data
  useEffect(() => {
    if (!autoScrolling) return
    const data = getDataStore().getData(symbol, timeframe)
    if (!data) return
    const maxStart = Math.max(0, data.length - viewCount + RIGHT_MARGIN_BARS)
    setViewStart(maxStart)
  }, [autoScrolling, dataVersion, viewCount, symbol, timeframe])

  // Smooth scroll rAF loop — only runs during auto-scroll
  useEffect(() => {
    if (!autoScrolling) {
      // When not auto-scrolling, compute CS without offset
      const data = getDataStore().getData(symbol, timeframe)
      if (!data || width === 0 || height === 0) return
      const end = Math.min(viewStart + viewCount, data.length)
      const dataBars = end - viewStart
      if (dataBars <= 0) return
      const totalBars = dataBars + RIGHT_MARGIN_BARS
      let minP: number, maxP: number
      if (priceOverride) { minP = priceOverride.min; maxP = priceOverride.max }
      else {
        const range = data.priceRange(viewStart, end)
        const pad = (range.max - range.min) * 0.05
        minP = range.min - pad; maxP = range.max + pad
      }
      setCs(CoordSystem.create({ width, height, barCount: totalBars, minPrice: minP, maxPrice: maxP, scrollOffset: 0 }))
      return
    }

    const tick = () => {
      const data = getDataStore().getData(symbol, timeframe)
      if (!data || data.length === 0 || width === 0 || height === 0) {
        smoothRafRef.current = requestAnimationFrame(tick)
        return
      }

      // Compute fractional offset from wall-clock time since last candle
      const elapsed = performance.now() - lastCandleRealTimeRef.current
      const interval = candleIntervalMsRef.current
      const offset = interval > 0 ? Math.min(1, Math.max(0, elapsed / interval)) : 0

      const end = Math.min(viewStart + viewCount, data.length)
      const dataBars = end - viewStart
      if (dataBars <= 0) {
        smoothRafRef.current = requestAnimationFrame(tick)
        return
      }
      const totalBars = dataBars + RIGHT_MARGIN_BARS

      let minP: number, maxP: number
      if (priceOverride) { minP = priceOverride.min; maxP = priceOverride.max }
      else {
        const range = data.priceRange(viewStart, end)
        const pad = (range.max - range.min) * 0.05
        minP = range.min - pad; maxP = range.max + pad
      }

      setCs(new CoordSystem({ width, height, barCount: totalBars, minPrice: minP, maxPrice: maxP, scrollOffset: offset }))
      smoothRafRef.current = requestAnimationFrame(tick)
    }

    smoothRafRef.current = requestAnimationFrame(tick)
    return () => {
      if (smoothRafRef.current) { cancelAnimationFrame(smoothRafRef.current); smoothRafRef.current = null }
    }
  }, [autoScrolling, viewStart, viewCount, width, height, priceOverride, symbol, timeframe, dataVersion])

  // Reset on symbol/timeframe change
  useEffect(() => {
    const data = getDataStore().getData(symbol, timeframe)
    if (data) {
      setViewStart(Math.max(0, data.length - 200))
      setViewCount(Math.min(200, Math.max(20, data.length)))
    }
    setPriceOverride(null)
    setAutoScrolling(true)
    lastBarCountRef.current = 0
    lastCandleRealTimeRef.current = performance.now()
  }, [symbol, timeframe])

  const pan = useCallback((deltaPixels: number) => {
    const data = getDataStore().getData(symbol, timeframe)
    if (!data || !cs) return
    const barDelta = Math.round(deltaPixels / cs.barStep)
    if (barDelta === 0) return
    pauseAutoScroll()
    setViewStart(v => Math.max(0, Math.min(data.length - viewCount + RIGHT_MARGIN_BARS, v - barDelta)))
  }, [cs, viewCount, pauseAutoScroll, symbol, timeframe])

  const zoomX = useCallback((factor: number) => {
    const data = getDataStore().getData(symbol, timeframe)
    if (!data) return
    pauseAutoScroll()
    setViewCount(v => {
      const newCount = Math.max(20, Math.min(data.length, Math.round(v * factor)))
      setViewStart(s => Math.max(0, Math.min(data.length - newCount + RIGHT_MARGIN_BARS, s + Math.round((v - newCount) / 2))))
      return newCount
    })
  }, [pauseAutoScroll, symbol, timeframe])

  const zoomY = useCallback((factor: number, anchorPrice?: number) => {
    if (!cs) return
    pauseAutoScroll()
    const center = anchorPrice ?? (cs.minPrice + cs.maxPrice) / 2
    const halfRange = ((cs.maxPrice - cs.minPrice) / 2) * factor
    setPriceOverride({ min: center - halfRange, max: center + halfRange })
  }, [cs, pauseAutoScroll])

  const panY = useCallback((deltaPixels: number) => {
    if (!cs) return
    pauseAutoScroll()
    const pricePerPixel = (cs.maxPrice - cs.minPrice) / cs.chartHeight
    const priceDelta = deltaPixels * pricePerPixel
    setPriceOverride({ min: cs.minPrice + priceDelta, max: cs.maxPrice + priceDelta })
  }, [cs, pauseAutoScroll])

  const resetYZoom = useCallback(() => setPriceOverride(null), [])

  const viewport: Viewport = useMemo(() => ({
    viewStart, viewCount, cs
  }), [viewStart, viewCount, cs])

  return { viewport, pan, zoomX, zoomY, panY, resetYZoom, autoScrolling, pauseAutoScroll }
}
