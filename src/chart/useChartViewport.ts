import { useState, useEffect, useCallback, useRef, useMemo } from 'react'
import { CoordSystem } from '../engine'
import { useChartStore } from '../store/chartStore'
import { getDataStore } from '../globals'
import type { Timeframe } from '../types'

const RIGHT_MARGIN_BARS = 8
const FUTURE_PAN_BARS = 200
const INTERACTION_IDLE_MS = 1_000 // resume auto-scroll 1s after last interaction

export interface Viewport {
  viewStart: number
  viewCount: number
  cs: CoordSystem | null
}

export function useChartViewport(symbol: string, timeframe: Timeframe, width: number, height: number) {
  // ── Core viewport state ─────────────────────────────────────────────────────
  // Live refs are the single source of truth during interaction.
  // React state is flushed at most once per rAF frame for rendering.
  const [viewStart, setViewStart] = useState(0)
  const viewStartRef = useRef(0)
  const [viewCount, setViewCount] = useState(20)
  const viewCountRef = useRef(20)
  const [priceOverride, setPriceOverride] = useState<{ min: number; max: number } | null>(null)
  const priceOverrideRef = useRef<{ min: number; max: number } | null>(null)
  useEffect(() => { priceOverrideRef.current = priceOverride }, [priceOverride])

  const [autoScrolling, setAutoScrolling] = useState(true)
  const autoScrollingRef = useRef(true)
  const [dataVersion, setDataVersion] = useState(0)

  // Single rAF handle for ALL interaction flushes (pan + zoom combined)
  const flushRafRef = useRef<number | null>(null)
  const idleTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  const autoScrollVersion = useChartStore(s => s.autoScrollVersion)

  // ── Flush: batch all ref changes into one React render per frame ────────────
  const scheduleFlush = useCallback(() => {
    if (flushRafRef.current) return // already scheduled
    flushRafRef.current = requestAnimationFrame(() => {
      flushRafRef.current = null
      setViewStart(viewStartRef.current)
      setViewCount(viewCountRef.current)
      setPriceOverride(priceOverrideRef.current)
    })
  }, [])

  // ── Pause auto-scroll ───────────────────────────────────────────────────────
  // Uses ref to avoid React re-render on every mousemove.
  // React state only updates once (when first paused) and once (when resuming).
  const pauseAutoScroll = useCallback(() => {
    if (autoScrollingRef.current) {
      autoScrollingRef.current = false
      setAutoScrolling(false)
    }
    // Reset idle timer — resume after INTERACTION_IDLE_MS of no activity
    if (idleTimerRef.current) clearTimeout(idleTimerRef.current)
    idleTimerRef.current = setTimeout(() => {
      autoScrollingRef.current = true
      setAutoScrolling(true)
      priceOverrideRef.current = null
      setPriceOverride(null)
    }, INTERACTION_IDLE_MS)
  }, [])

  // Global reset → force auto-scroll
  useEffect(() => {
    autoScrollingRef.current = true
    setAutoScrolling(true)
    priceOverrideRef.current = null
    setPriceOverride(null)
  }, [autoScrollVersion])

  // Reset viewport when symbol/timeframe changes
  useEffect(() => {
    autoScrollingRef.current = true
    setAutoScrolling(true)
    priceOverrideRef.current = null
    setPriceOverride(null)
    viewStartRef.current = 0
    setViewStart(0)
    if (idleTimerRef.current) clearTimeout(idleTimerRef.current)
  }, [symbol, timeframe])

  // Cleanup
  useEffect(() => () => {
    if (idleTimerRef.current) clearTimeout(idleTimerRef.current)
    if (flushRafRef.current) cancelAnimationFrame(flushRafRef.current)
  }, [])

  // Subscribe to data changes — throttle to 4 updates/sec max
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
      }, 250)
    })
    return () => { unsub(); if (throttleTimer) clearTimeout(throttleTimer) }
  }, [symbol, timeframe])

  // Keep viewCountRef in sync when React state changes (e.g., from auto-scroll)
  useEffect(() => { viewCountRef.current = viewCount }, [viewCount])

  // ── Auto-scroll ─────────────────────────────────────────────────────────────
  useEffect(() => {
    if (!autoScrolling) return
    const data = getDataStore().getData(symbol, timeframe)
    if (!data) return
    const targetVc = Math.min(200, Math.max(viewCount, data.length + RIGHT_MARGIN_BARS))
    if (targetVc !== viewCount) {
      viewCountRef.current = targetVc
      setViewCount(targetVc)
    }
    const maxStart = Math.max(0, data.length - targetVc + RIGHT_MARGIN_BARS)
    viewStartRef.current = maxStart
    setViewStart(maxStart)
  }, [autoScrolling, dataVersion, viewCount, symbol, timeframe])

  // ── computeCs — called by ChartPane's rAF loop ─────────────────────────────
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

  // CoordSystem derived from React state (for non-imperative consumers)
  const cs = useMemo<CoordSystem | null>(
    () => computeCs(viewStart, viewCount),
  // eslint-disable-next-line react-hooks/exhaustive-deps
  [viewStart, viewCount, width, height, priceOverride, dataVersion, symbol, timeframe, computeCs])

  // ── Scroll-left pagination ──────────────────────────────────────────────────
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

  // Reset on symbol/timeframe change (data-aware)
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
    priceOverrideRef.current = null
    setPriceOverride(null)
    autoScrollingRef.current = true
    setAutoScrolling(true)
    loadingMoreRef.current = false
  }, [symbol, timeframe])

  // ── Interaction handlers (all ref-driven, flushed via rAF) ──────────────────

  const pan = useCallback((deltaPixels: number) => {
    const data = getDataStore().getData(symbol, timeframe)
    if (!data) return
    const barStep = (width - 80) / (viewCountRef.current + RIGHT_MARGIN_BARS)
    const barDelta = deltaPixels / barStep
    if (Math.abs(barDelta) < 0.0001) return
    pauseAutoScroll()
    const maxVS = data.length - viewCountRef.current + FUTURE_PAN_BARS
    viewStartRef.current = Math.max(0, Math.min(maxVS, viewStartRef.current - barDelta))
    scheduleFlush()
  }, [pauseAutoScroll, scheduleFlush, symbol, timeframe, width])

  const zoomX = useCallback((factor: number) => {
    const data = getDataStore().getData(symbol, timeframe)
    if (!data) return
    pauseAutoScroll()
    const oldVc = viewCountRef.current
    const newVc = Math.max(20, Math.min(data.length, Math.round(oldVc * factor)))
    if (newVc === oldVc) return
    // Center the zoom around current viewport midpoint
    const delta = Math.round((oldVc - newVc) / 2)
    viewCountRef.current = newVc
    viewStartRef.current = Math.max(0, Math.min(
      data.length - newVc + RIGHT_MARGIN_BARS,
      viewStartRef.current + delta
    ))
    scheduleFlush()
  }, [pauseAutoScroll, scheduleFlush, symbol, timeframe])

  const zoomY = useCallback((factor: number, anchorPrice?: number) => {
    // Read from ref for latest price range (no cs dependency = stable callback)
    const currentCs = computeCs(viewStartRef.current, viewCountRef.current)
    if (!currentCs) return
    pauseAutoScroll()
    const center = anchorPrice ?? (currentCs.minPrice + currentCs.maxPrice) / 2
    const halfRange = ((currentCs.maxPrice - currentCs.minPrice) / 2) * factor
    priceOverrideRef.current = { min: center - halfRange, max: center + halfRange }
    scheduleFlush()
  }, [pauseAutoScroll, scheduleFlush, computeCs])

  const panY = useCallback((deltaPixels: number) => {
    const currentCs = computeCs(viewStartRef.current, viewCountRef.current)
    if (!currentCs) return
    pauseAutoScroll()
    const pricePerPixel = (currentCs.maxPrice - currentCs.minPrice) / currentCs.chartHeight
    const priceDelta = deltaPixels * pricePerPixel
    priceOverrideRef.current = {
      min: currentCs.minPrice + priceDelta,
      max: currentCs.maxPrice + priceDelta,
    }
    scheduleFlush()
  }, [pauseAutoScroll, scheduleFlush, computeCs])

  const resetYZoom = useCallback(() => {
    priceOverrideRef.current = null
    setPriceOverride(null)
  }, [])

  const resetView = useCallback(() => {
    const data = getDataStore().getData(symbol, timeframe)
    const vc = 200
    const vs = data ? Math.max(0, data.length - vc + RIGHT_MARGIN_BARS) : 0
    viewStartRef.current = vs
    viewCountRef.current = vc
    setViewStart(vs)
    setViewCount(vc)
    priceOverrideRef.current = null
    setPriceOverride(null)
    autoScrollingRef.current = true
    setAutoScrolling(true)
    if (idleTimerRef.current) { clearTimeout(idleTimerRef.current); idleTimerRef.current = null }
  }, [symbol, timeframe])

  const zoomToRect = useCallback((
    x1: number, y1: number, x2: number, y2: number,
    currentCs: CoordSystem,
  ) => {
    const left   = Math.min(x1, x2)
    const right  = Math.max(x1, x2)
    const top    = Math.min(y1, y2)
    const bottom = Math.max(y1, y2)

    const barLeft  = currentCs.xToBar(left)
    const barRight = currentCs.xToBar(right)
    const newCount = Math.max(5, Math.ceil(barRight - barLeft))
    const newStart = Math.max(0, Math.floor(viewStartRef.current) + Math.floor(barLeft))

    const priceHigh = currentCs.yToPrice(top)
    const priceLow  = currentCs.yToPrice(bottom)

    pauseAutoScroll()
    viewStartRef.current = newStart
    viewCountRef.current = newCount
    priceOverrideRef.current = { min: Math.min(priceHigh, priceLow), max: Math.max(priceHigh, priceLow) }
    scheduleFlush()
  }, [pauseAutoScroll, scheduleFlush])

  /** Per-pane auto-scroll reset */
  const resetAutoScroll = useCallback(() => {
    autoScrollingRef.current = true
    setAutoScrolling(true)
    priceOverrideRef.current = null
    setPriceOverride(null)
    if (idleTimerRef.current) { clearTimeout(idleTimerRef.current); idleTimerRef.current = null }
  }, [])

  const viewport: Viewport = useMemo(() => ({
    viewStart: Math.floor(viewStart),
    viewCount,
    cs
  }), [viewStart, viewCount, cs])

  return { viewport, pan, zoomX, zoomY, panY, resetYZoom, resetView, zoomToRect, autoScrolling, pauseAutoScroll, resetAutoScroll, viewStartRef, viewCountRef, computeCs }
}
