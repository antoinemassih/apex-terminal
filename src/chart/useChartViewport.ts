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
  // viewStart is a float — fractional part drives sub-pixel pan offset in CoordSystem
  const [viewStart, setViewStart] = useState(0)
  // Live ref updated synchronously during pan — avoids React re-render on every mouse event.
  // setViewStart is called via rAF, capping React renders at 60/sec regardless of mouse Hz.
  const viewStartRef = useRef(0)
  const panRafRef = useRef<number | null>(null)
  const [viewCount, setViewCount] = useState(200)
  const viewCountRef = useRef(200)
  const [priceOverride, setPriceOverride] = useState<{ min: number; max: number } | null>(null)
  // Live ref for priceOverride — lets computeCs read the latest value without being a dep
  const priceOverrideRef = useRef<{ min: number; max: number } | null>(null)
  useEffect(() => { priceOverrideRef.current = priceOverride }, [priceOverride])
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
    if (panRafRef.current) cancelAnimationFrame(panRafRef.current)
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

  // Keep viewCountRef in sync
  useEffect(() => { viewCountRef.current = viewCount }, [viewCount])

  // Auto-scroll
  useEffect(() => {
    if (!autoScrolling) return
    const data = getDataStore().getData(symbol, timeframe)
    if (!data) return
    const maxStart = Math.max(0, data.length - viewCount + RIGHT_MARGIN_BARS)
    viewStartRef.current = maxStart
    setViewStart(maxStart)
  }, [autoScrolling, dataVersion, viewCount, symbol, timeframe])

  // Stable computeCs function — called by ChartPane's rAF loop using live refs.
  // Reads priceOverrideRef so it doesn't take priceOverride as a closure dep
  // (avoids invalidating the rAF loop on every price drag).
  const computeCs = useCallback((
    vs: number,
    vc: number,
    gpuRange?: { min: number; max: number } | null,
  ): CoordSystem | null => {
    const data = getDataStore().getData(symbol, timeframe)
    if (!data || width === 0 || height === 0) return null
    const iStart = Math.floor(vs)
    const end = Math.min(iStart + vc, data.length)
    if (end - iStart <= 0) return null
    const totalBars = vc + RIGHT_MARGIN_BARS
    let minP: number, maxP: number
    if (priceOverrideRef.current) {
      minP = priceOverrideRef.current.min; maxP = priceOverrideRef.current.max
    } else if (gpuRange) {
      // GPU-computed range from last frame (1-frame delay) — zero CPU scan
      // Guard against degenerate range (single price level → 0/0 NaN in priceToY)
      let gMin = gpuRange.min, gMax = gpuRange.max
      if (gMin === gMax) { gMin -= 0.5; gMax += 0.5 }
      const pad = (gMax - gMin) * 0.05
      minP = gMin - pad; maxP = gMax + pad
    } else {
      const range = data.priceRange(iStart, end)
      const pad = (range.max - range.min) * 0.05
      minP = range.min - pad; maxP = range.max + pad
    }
    const chartWidth = width - 80
    const barStep = chartWidth / totalBars
    const pixelOffset = (vs - Math.floor(vs)) * barStep
    return CoordSystem.create({ width, height, barCount: totalBars, minPrice: minP, maxPrice: maxP, pixelOffset })
  }, [symbol, timeframe, width, height])

  // CoordSystem is a derived value — useMemo keeps it in the same render cycle as viewStart.
  // This eliminates the extra render that useState+useEffect caused (halving render cost per pan).
  // Now delegates to computeCs so computation is not duplicated.
  const cs = useMemo<CoordSystem | null>(
    () => computeCs(viewStart, viewCount),
  // eslint-disable-next-line react-hooks/exhaustive-deps
  [viewStart, viewCount, width, height, priceOverride, dataVersion, symbol, timeframe, computeCs])

  // Scroll-left pagination: load more history when viewStart hits 0
  const loadingMoreRef = useRef(false)

  useEffect(() => {
    if (Math.floor(viewStart) > 5 || loadingMoreRef.current || autoScrolling) return
    const ds = getDataStore()
    if (!ds.canLoadMore(symbol, timeframe)) return
    loadingMoreRef.current = true
    ds.loadMore(symbol, timeframe).then(added => {
      if (added > 0) {
        viewStartRef.current += added
        setViewStart(viewStartRef.current)
      }
      loadingMoreRef.current = false
    }).catch(() => { loadingMoreRef.current = false })
  }, [viewStart, symbol, timeframe, autoScrolling])

  // Reset on symbol/timeframe change
  useEffect(() => {
    const data = getDataStore().getData(symbol, timeframe)
    if (data) {
      const vs = Math.max(0, data.length - 200)
      const vc = Math.min(200, Math.max(20, data.length))
      viewStartRef.current = vs
      viewCountRef.current = vc
      setViewStart(vs)
      setViewCount(vc)
    }
    setPriceOverride(null)
    setAutoScrolling(true)
    loadingMoreRef.current = false
  }, [symbol, timeframe])

  const pan = useCallback((deltaPixels: number) => {
    const data = getDataStore().getData(symbol, timeframe)
    if (!data) return
    // Compute barStep directly from geometry — no cs dependency, stays stable during pan.
    const barStep = (width - 80) / (viewCountRef.current + RIGHT_MARGIN_BARS)
    const barDelta = deltaPixels / barStep
    if (Math.abs(barDelta) < 0.0001) return
    pauseAutoScroll()
    const maxVS = data.length - viewCountRef.current + FUTURE_PAN_BARS
    viewStartRef.current = Math.max(0, Math.min(maxVS, viewStartRef.current - barDelta))
    // Flush to React state at most once per animation frame (caps renders at 60/sec)
    if (!panRafRef.current) {
      panRafRef.current = requestAnimationFrame(() => {
        panRafRef.current = null
        setViewStart(viewStartRef.current)
      })
    }
  }, [pauseAutoScroll, symbol, timeframe, width])

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
    viewStart: Math.floor(viewStart),  // integer for GPU array indexing
    viewCount,
    cs
  }), [viewStart, viewCount, cs])

  return { viewport, pan, zoomX, zoomY, panY, resetYZoom, autoScrolling, pauseAutoScroll, viewStartRef, viewCountRef, computeCs }
}
