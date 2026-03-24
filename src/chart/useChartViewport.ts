import { useState, useEffect, useCallback, useRef, useMemo } from 'react'
import { CoordSystem } from '../engine'
import { useChartStore } from '../store/chartStore'
import { getDataStore } from '../globals'
import type { Timeframe } from '../types'

const RIGHT_MARGIN_BARS = 8
const FUTURE_PAN_BARS = 200
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
  }, [])

  // Subscribe to data changes — throttle to 4 updates/sec max to avoid React re-render storm
  useEffect(() => {
    const ds = getDataStore()
    let throttleTimer: ReturnType<typeof setTimeout> | null = null
    let pending = false
    const unsub = ds.subscribe(symbol, timeframe, () => {
      if (throttleTimer) { pending = true; return }
      setDataVersion(v => v + 1)
      throttleTimer = setTimeout(() => {
        throttleTimer = null
        if (pending) { pending = false; setDataVersion(v => v + 1) }
      }, 250) // max 4 React updates per second from tick data
    })
    return () => { unsub(); if (throttleTimer) clearTimeout(throttleTimer) }
  }, [symbol, timeframe])

  // Auto-scroll
  useEffect(() => {
    if (!autoScrolling) return
    const data = getDataStore().getData(symbol, timeframe)
    if (!data) return
    const maxStart = Math.max(0, data.length - viewCount + RIGHT_MARGIN_BARS)
    setViewStart(maxStart)
  }, [autoScrolling, dataVersion, viewCount, symbol, timeframe])

  // Recompute CoordSystem
  useEffect(() => {
    const data = getDataStore().getData(symbol, timeframe)
    if (!data || width === 0 || height === 0) return

    const end = Math.min(viewStart + viewCount, data.length)
    const dataBars = end - viewStart
    if (dataBars <= 0) return
    const totalBars = dataBars + RIGHT_MARGIN_BARS

    let minP: number, maxP: number
    if (priceOverride) {
      minP = priceOverride.min
      maxP = priceOverride.max
    } else {
      const range = data.priceRange(viewStart, end)
      const pad = (range.max - range.min) * 0.05
      minP = range.min - pad
      maxP = range.max + pad
    }

    setCs(CoordSystem.create({ width, height, barCount: totalBars, minPrice: minP, maxPrice: maxP }))
  }, [viewStart, viewCount, width, height, priceOverride, dataVersion, symbol, timeframe])

  // Scroll-left pagination: load more history when viewStart hits 0
  const loadingMoreRef = useRef(false)

  useEffect(() => {
    if (viewStart > 5 || loadingMoreRef.current || autoScrolling) return
    const ds = getDataStore()
    if (!ds.canLoadMore(symbol, timeframe)) return
    loadingMoreRef.current = true
    ds.loadMore(symbol, timeframe).then(added => {
      if (added > 0) {
        // Shift viewStart right by the number of prepended bars so the view doesn't jump
        setViewStart(v => v + added)
      }
      loadingMoreRef.current = false
    }).catch(() => { loadingMoreRef.current = false })
  }, [viewStart, symbol, timeframe, autoScrolling])

  // Reset on symbol/timeframe change
  useEffect(() => {
    const data = getDataStore().getData(symbol, timeframe)
    if (data) {
      setViewStart(Math.max(0, data.length - 200))
      setViewCount(Math.min(200, Math.max(20, data.length)))
    }
    setPriceOverride(null)
    setAutoScrolling(true)
    loadingMoreRef.current = false
  }, [symbol, timeframe])

  const pan = useCallback((deltaPixels: number) => {
    const data = getDataStore().getData(symbol, timeframe)
    if (!data || !cs) return
    const barDelta = Math.round(deltaPixels / cs.barStep)
    if (barDelta === 0) return
    pauseAutoScroll()
    setViewStart(v => Math.max(0, Math.min(data.length - viewCount + FUTURE_PAN_BARS, v - barDelta)))
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
