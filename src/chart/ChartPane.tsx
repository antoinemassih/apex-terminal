import { useEffect, useRef, useCallback, useState } from 'react'
import { getRenderEngine, getDataStore, getIndicatorEngine, getDataProvider } from '../globals'
import type { PaneContext, EngineState } from '../engine'
import { useChartViewport } from './useChartViewport'
import { AxisCanvas, AxisCanvasHandle } from './AxisCanvas'
import { CrosshairOverlay, CrosshairHandle } from './CrosshairOverlay'
import { DrawingOverlay, DrawingOverlayHandle } from './DrawingOverlay'
import { useDrawingStore } from '../store/drawingStore'
import { useChartStore } from '../store/chartStore'
import { useOrderStore } from '../store/orderStore'
import { getTheme } from '../themes'
import { SymbolPicker } from './SymbolPicker'
import { ChartContextMenu } from './ChartContextMenu'
import { OrderEntry } from './OrderEntry'
import { OrderLevels } from './OrderLevels'
import type { Timeframe } from '../types'

interface Props {
  paneIndex: number
  symbol: string
  timeframe: Timeframe
  width: number
  height: number
}

export function ChartPane({ paneIndex, symbol, timeframe, width, height }: Props) {
  const paneId = `${symbol}:${timeframe}`
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const paneRef = useRef<PaneContext | null>(null)
  const crosshairRef = useRef<CrosshairHandle>(null)
  const drawingRef = useRef<DrawingOverlayHandle>(null)
  const axisRef = useRef<AxisCanvasHandle>(null)
  const [engineState, setEngineState] = useState<EngineState>('ready')
  const { viewport, pan, zoomX, zoomY, resetYZoom, resetView, zoomToRect, autoScrolling, pauseAutoScroll, resetAutoScroll, viewStartRef, viewCountRef, computeCs } =
    useChartViewport(symbol, timeframe, width, height)

  const paneConfig = useChartStore(s => s.panes[paneIndex])
  const showVolume = paneConfig?.showVolume ?? true
  const visibleIndicators = paneConfig?.visibleIndicators ?? []
  const themeName = useChartStore(s => s.theme)
  const theme = getTheme(themeName)
  const orderEntryEnabled = useOrderStore(s => s.enabled)

  const [pickerPos, setPickerPos] = useState<{ x: number; y: number } | null>(null)
  const [contextMenu, setContextMenu] = useState<{ x: number; y: number; clickPrice: number | null } | null>(null)
  const [dragZoomMode, setDragZoomMode] = useState(false)
  const [hovered, setHovered] = useState(false)

  // Drag-zoom overlay — updated imperatively to avoid React re-renders on every mousemove
  const dragZoomDivRef = useRef<HTMLDivElement>(null)
  const zoomStartRef   = useRef<{ x: number; y: number } | null>(null)

  // Exit drag zoom on Escape
  useEffect(() => {
    if (!dragZoomMode) return
    const handler = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        setDragZoomMode(false)
        zoomStartRef.current = null
        if (dragZoomDivRef.current) dragZoomDivRef.current.style.display = 'none'
      }
    }
    document.addEventListener('keydown', handler)
    return () => document.removeEventListener('keydown', handler)
  }, [dragZoomMode])

  const { cs } = viewport
  const csRef = useRef(cs)
  csRef.current = cs
  const data = getDataStore().getData(symbol, timeframe)

  // Register/unregister with engine
  useEffect(() => {
    if (!canvasRef.current) return
    const engine = getRenderEngine()
    const pane = engine.registerPane(paneId, canvasRef.current)
    paneRef.current = pane
    const unsub = engine.onStateChange(setEngineState)
    return () => { engine.unregisterPane(paneId); unsub() }
  }, [paneId])

  // Handle resize
  useEffect(() => {
    paneRef.current?.resize(width, height)
  }, [width, height])

  // Load data + subscribe to feed + subscribe to updates
  useEffect(() => {
    const ds = getDataStore()
    const provider = getDataProvider()

    // Subscribe to real-time ticks
    provider.subscribe(symbol, timeframe)

    // Seed the simulation immediately with a default price so ticks start flowing
    // before historical data arrives (avoids 1-3s blank screen on cold start).
    if ('setLastPrice' in provider) {
      ;(provider as any).setLastPrice(symbol, timeframe, 100, Date.now() / 1000)
    }

    // Load historical data, then re-seed simulation with the real last price.
    ds.load(symbol, timeframe).then(({ data: store }) => {
      if ('setLastPrice' in provider && store.length > 0) {
        ;(provider as any).setLastPrice(symbol, timeframe,
          store.closes[store.length - 1], store.times[store.length - 1])
      }
    }).catch(err => console.error(`Failed to load ${symbol}:${timeframe}:`, err))

    // Subscribe to data changes → push to PaneContext
    // This is the hot path — ticks flow directly to GPU without React re-renders
    const unsub = ds.subscribe(symbol, timeframe, () => {
      const d = ds.getData(symbol, timeframe)
      const indicators = ds.getIndicators(symbol, timeframe)
      const action = ds.getLastAction(symbol, timeframe)
      if (d && indicators) paneRef.current?.setData(d, indicators, action)
    })

    // Seed immediately with cached data — handles StrictMode double-invoke where
    // ds.load() returns cached data without firing notify(), leaving pane empty.
    const existingData = ds.getData(symbol, timeframe)
    const existingIndicators = ds.getIndicators(symbol, timeframe)
    if (existingData && existingIndicators) {
      paneRef.current?.setData(existingData, existingIndicators, null)
    }

    return () => {
      unsub()
      provider.unsubscribe(symbol, timeframe)
    }
  }, [symbol, timeframe])

  // Single imperative rAF loop — drives GPU candles, axis canvas, and drawing overlay
  // at up to 60fps during pan without touching React state.
  useEffect(() => {
    let rafId: number
    let lastVs = -1, lastVc = -1, lastMinP = 0, lastMaxP = 0
    let lastDataLen = 0, lastLastClose = 0

    const loop = () => {
      rafId = requestAnimationFrame(loop)
      const vs = viewStartRef.current
      const vc = viewCountRef.current

      const data = getDataStore().getData(symbol, timeframe)
      const dataLen = data?.length ?? 0
      const lastClose = (data && dataLen > 0) ? data.closes[dataLen - 1] : 0

      // Cheap early exit — skip if viewport AND data unchanged
      if (vs === lastVs && vc === lastVc && dataLen === lastDataLen && lastClose === lastLastClose) return

      const cs = computeCs(vs, vc, paneRef.current?.gpuPriceRange)
      if (!cs) return

      // Secondary check: skip if price range also unchanged
      if (vs === lastVs && vc === lastVc && cs.minPrice === lastMinP && cs.maxPrice === lastMaxP) {
        lastDataLen = dataLen; lastLastClose = lastClose
        return
      }

      lastVs = vs; lastVc = vc; lastMinP = cs.minPrice; lastMaxP = cs.maxPrice
      lastDataLen = dataLen; lastLastClose = lastClose

      // 1. GPU candles + indicators
      paneRef.current?.setViewport({ viewStart: Math.floor(vs), viewCount: vc, cs })

      // 2. Axis 2D canvas (text labels)
      if (data) axisRef.current?.draw(cs, data, Math.floor(vs))

      // 3. Drawing overlay
      drawingRef.current?.setViewport(cs, Math.floor(vs))
    }

    rafId = requestAnimationFrame(loop)
    return () => cancelAnimationFrame(rafId)
  }, [computeCs, symbol, timeframe, viewStartRef, viewCountRef])

  // Push visibility settings to PaneContext — only when toggles change, not on every tick
  useEffect(() => {
    const outputs = getIndicatorEngine().getOutputs(symbol, timeframe)
      .filter(out => visibleIndicators.includes(out.indicatorId))
    paneRef.current?.setVisibility(showVolume, outputs)
  }, [showVolume, visibleIndicators, symbol, timeframe])

  // Push theme to PaneContext.
  // Also fires when contextMenu opens — forces a GPU re-render after the compositor repaint
  // that happens when a position:fixed element is first added to the DOM (prevents black flash).
  useEffect(() => {
    paneRef.current?.setTheme(themeName)
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [themeName, contextMenu, paneId])

  // --- Drag handling ---
  const dragRef    = useRef<{ x: number; y: number; zone: 'chart' | 'xaxis' | 'yaxis' } | null>(null)
  // Cached on mousedown — avoids getBoundingClientRect() on every mousemove
  const paneRectRef = useRef<DOMRect | null>(null)

  const getZone = useCallback((x: number, y: number): 'chart' | 'xaxis' | 'yaxis' => {
    if (!cs) return 'chart'
    if (x >= width - cs.pr && y < height - cs.pb) return 'yaxis'
    if (y >= height - cs.pb) return 'xaxis'
    return 'chart'
  }, [cs, width, height])

  // contextmenu is prevented globally in main.tsx; this is a safety-net only.
  const onContextMenu = useCallback((e: React.MouseEvent) => { e.preventDefault() }, [])

  const onMouseDown = useCallback((e: React.MouseEvent) => {
    // Show context menu on right-mousedown — before the OS fires WM_CONTEXTMENU —
    // so the custom menu is already composited when WebView2 processes contextmenu,
    // preventing the whole-app black flash caused by Win32 surface exposure.
    if (e.button === 2) {
      const rect = (e.currentTarget as HTMLElement).getBoundingClientRect()
      const my = e.clientY - rect.top
      const clickPrice = cs ? cs.yToPrice(my) : null
      setContextMenu({ x: e.clientX, y: e.clientY, clickPrice })
      return
    }
    if (e.button !== 0) return
    // Cache rect once per drag — reused on every mousemove, no layout reads during pan
    const rect = (e.currentTarget as HTMLElement).getBoundingClientRect()
    paneRectRef.current = rect
    const mx = e.clientX - rect.left
    const my = e.clientY - rect.top
    if (dragZoomMode) {
      zoomStartRef.current = { x: mx, y: my }
      return
    }
    if (drawingRef.current?.handleMouseDown(mx, my, e.shiftKey)) {
      setContextMenu(null)  // close any open context menu
      e.stopPropagation()   // prevent LineStylePopup's window mousedown handler from resetting selection
      return
    }
    const zone = getZone(mx, my)
    dragRef.current = { x: e.clientX, y: e.clientY, zone }
  }, [dragZoomMode, getZone])

  const onMouseMove = useCallback((e: React.MouseEvent) => {
    const rect = paneRectRef.current ?? (e.currentTarget as HTMLElement).getBoundingClientRect()
    const mx = e.clientX - rect.left
    const my = e.clientY - rect.top
    if (dragZoomMode) {
      const start = zoomStartRef.current
      if (start && dragZoomDivRef.current) {
        const el = dragZoomDivRef.current
        el.style.display = 'block'
        el.style.left   = Math.min(start.x, mx) + 'px'
        el.style.top    = Math.min(start.y, my) + 'px'
        el.style.width  = Math.abs(mx - start.x) + 'px'
        el.style.height = Math.abs(my - start.y) + 'px'
      }
      return
    }
    // During chart drag (pan/zoom): skip all overlay work — pure GPU path
    if (dragRef.current) {
      const dx = e.clientX - dragRef.current.x
      const dy = e.clientY - dragRef.current.y
      switch (dragRef.current.zone) {
        case 'chart': pan(dx); break
        case 'xaxis': if (Math.abs(dx) > 1) zoomX(dx > 0 ? 1.05 : 0.95); break
        case 'yaxis': if (Math.abs(dy) > 1) zoomY(dy > 0 ? 1.05 : 0.95); break
      }
      dragRef.current = { ...dragRef.current, x: e.clientX, y: e.clientY }
      // Immediate GPU render — don't wait for rAF
      const newCs = computeCs(viewStartRef.current, viewCountRef.current, paneRef.current?.gpuPriceRange)
      if (newCs && paneRef.current) {
        paneRef.current.setViewport({ viewStart: Math.floor(viewStartRef.current), viewCount: viewCountRef.current, cs: newCs })
        paneRef.current.forceRender()
      }
      return
    }
    // Not dragging — update crosshair + drawing overlay
    if (cs && data) crosshairRef.current?.update(mx, my)
    drawingRef.current?.handleMouseMove(mx, my)
  }, [dragZoomMode, pan, zoomX, zoomY, cs, data, computeCs])

  const onMouseUp = useCallback((e: React.MouseEvent) => {
    if (dragZoomMode) {
      const start = zoomStartRef.current
      const rect  = paneRectRef.current ?? (e.currentTarget as HTMLElement).getBoundingClientRect()
      if (start && cs) {
        const mx = e.clientX - rect.left
        const my = e.clientY - rect.top
        // Only zoom if the rect is at least 10px in each dimension
        if (Math.abs(mx - start.x) > 10 && Math.abs(my - start.y) > 10) {
          zoomToRect(start.x, start.y, mx, my, cs)
        }
      }
      zoomStartRef.current = null
      paneRectRef.current  = null
      if (dragZoomDivRef.current) dragZoomDivRef.current.style.display = 'none'
      setDragZoomMode(false)
      return
    }
    dragRef.current     = null
    paneRectRef.current = null
    drawingRef.current?.handleMouseUp()
  }, [dragZoomMode, cs, zoomToRect])

  const onMouseLeave = useCallback(() => {
    if (dragZoomMode) return
    dragRef.current = null
    paneRectRef.current = null
    drawingRef.current?.handleMouseUp()
    crosshairRef.current?.clear()
  }, [dragZoomMode])

  // Use a native (non-passive) wheel listener so e.preventDefault() actually works.
  // React attaches onWheel as passive by default, which silently ignores preventDefault.
  const wheelDivRef = useRef<HTMLDivElement>(null)
  useEffect(() => {
    const el = wheelDivRef.current
    if (!el) return
    const handler = (e: WheelEvent) => {
      e.preventDefault()
      if (!csRef.current) return
      const cs = csRef.current
      const rect = el.getBoundingClientRect()
      const x = e.clientX - rect.left
      const y = e.clientY - rect.top
      const factor = e.deltaY > 0 ? 1.1 : 0.9
      if (x >= width - cs.pr && y < height - cs.pb) {
        zoomY(factor, cs.yToPrice(y))
      } else {
        zoomX(factor)
      }
      // Immediate GPU render on wheel
      const newCs = computeCs(viewStartRef.current, viewCountRef.current, paneRef.current?.gpuPriceRange)
      if (newCs && paneRef.current) {
        paneRef.current.setViewport({ viewStart: Math.floor(viewStartRef.current), viewCount: viewCountRef.current, cs: newCs })
        paneRef.current.forceRender()
      }
    }
    el.addEventListener('wheel', handler, { passive: false })
    return () => el.removeEventListener('wheel', handler)
  }, [zoomX, zoomY, width, height, computeCs])

  const onAuxClick = useCallback((e: React.MouseEvent) => {
    if (e.button === 1) { e.preventDefault(); useDrawingStore.getState().toggleDrawTool() }
  }, [])

  const onDoubleClick = useCallback((e: React.MouseEvent) => {
    if (!cs) return
    const rect = (e.currentTarget as HTMLElement).getBoundingClientRect()
    if (e.clientX - rect.left >= width - cs.pr) resetYZoom()
  }, [cs, width, resetYZoom])

  // Cursor — imperative DOM update, zero React re-renders on mousemove
  const cursorRef = useRef('default')
  const onMouseMoveForCursor = useCallback((e: React.MouseEvent) => {
    onMouseMove(e)
    if (dragZoomMode) return
    const rect = paneRectRef.current ?? (e.currentTarget as HTMLElement).getBoundingClientRect()
    const mx = e.clientX - rect.left
    const my = e.clientY - rect.top
    const zone = getZone(mx, my)
    const drawCursor = drawingRef.current?.getCursor()
    let next: string
    if (zone === 'chart' && drawCursor) next = drawCursor
    else if (zone === 'yaxis') next = 'ns-resize'
    else if (zone === 'xaxis') next = 'ew-resize'
    else next = 'default'
    if (next !== cursorRef.current) {
      cursorRef.current = next
      if (wheelDivRef.current) wheelDivRef.current.style.cursor = next
    }
  }, [dragZoomMode, getZone, onMouseMove])

  return (
    <div ref={wheelDivRef}
      style={{ position: 'relative', width, height, background: theme.background,
        cursor: dragZoomMode ? 'crosshair' : 'default' }}
      onMouseDown={onMouseDown}
      onMouseMove={onMouseMoveForCursor}
      onMouseUp={onMouseUp}
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => { onMouseLeave(); setHovered(false) }}
      onDoubleClick={onDoubleClick}
      onAuxClick={onAuxClick}
      onContextMenu={onContextMenu}
    >
      <canvas ref={canvasRef} width={width} height={height}
        style={{ display: 'block', pointerEvents: 'none' }} />
      {/* OHLC label — double-click ticker to open symbol picker */}
      <div style={{
        position: 'absolute', top: 4, left: 8,
        color: theme.axisText, fontSize: 11, fontFamily: 'monospace',
        display: 'flex', alignItems: 'center', gap: 4,
      }}>
        <span
          style={{ cursor: 'pointer', color: theme.borderActive, fontWeight: 'bold' }}
          onDoubleClick={(e) => {
            e.stopPropagation()
            const rect = (e.target as HTMLElement).getBoundingClientRect()
            setPickerPos({ x: rect.left, y: rect.bottom + 4 })
          }}
        >{symbol}</span>
        <span style={{ pointerEvents: 'none' }}>· {timeframe}</span>
        {data && viewport.viewStart + viewport.viewCount - 1 < data.length && (() => {
          const last = viewport.viewStart + viewport.viewCount - 1
          const c = data.closes[last]
          const o = data.opens[last]
          const color = c >= o ? '#2ecc71' : '#e74c3c'
          return <span style={{ color }}>{` · O ${data.opens[last]?.toFixed(2)} H ${data.highs[last]?.toFixed(2)} L ${data.lows[last]?.toFixed(2)} C ${data.closes[last]?.toFixed(2)}`}</span>
        })()}
      </div>
      {cs && data && (
        <CrosshairOverlay ref={crosshairRef} cs={cs} data={data}
          viewStart={viewport.viewStart} width={width} height={height} />
      )}
      <AxisCanvas ref={axisRef} width={width} height={height} />
      {cs && data && (
        <DrawingOverlay ref={drawingRef} symbol={symbol} timeframe={timeframe} cs={cs}
          data={data} width={width} height={height} viewStart={viewport.viewStart}
          onInteraction={pauseAutoScroll} />
      )}
      {/* Play / Pause controls — bottom-right at axis intersection */}
      {(hovered || !autoScrolling) && cs && (
        <div style={{
          position: 'absolute',
          bottom: cs.pb + 3,
          right: cs.pr + 3,
          display: 'flex',
          gap: 3,
          pointerEvents: 'all',
          zIndex: 5,
        }}>
          {!autoScrolling && (
            <div
              style={{
                background: theme.toolbarBackground,
                border: `1px solid ${theme.toolbarBorder}`,
                borderRadius: 3,
                padding: '2px 6px',
                fontSize: 9,
                fontFamily: 'monospace',
                color: '#f59e0b',
                fontWeight: 'bold',
                letterSpacing: 0.5,
                cursor: 'default',
                userSelect: 'none',
              }}
            >
              PAUSED
            </div>
          )}
          <button
            onClick={(e) => { e.stopPropagation(); resetAutoScroll() }}
            onMouseDown={e => e.stopPropagation()}
            style={{
              background: autoScrolling ? (theme.bull ?? '#26a69a') + '22' : theme.toolbarBackground,
              border: `1px solid ${autoScrolling ? (theme.bull ?? '#26a69a') + '66' : theme.toolbarBorder}`,
              borderRadius: 3,
              padding: '2px 6px',
              fontSize: 11,
              fontFamily: 'monospace',
              color: autoScrolling ? (theme.bull ?? '#26a69a') : theme.axisText,
              cursor: 'pointer',
              fontWeight: 'bold',
              display: 'flex',
              alignItems: 'center',
              gap: 3,
              lineHeight: 1,
            }}
            title={autoScrolling ? 'Auto-scroll active' : 'Resume auto-scroll'}
          >
            {autoScrolling ? '▶' : '▶'}
            <span style={{ fontSize: 9, fontWeight: 'normal', letterSpacing: 0.3 }}>LIVE</span>
          </button>
        </div>
      )}
      {engineState === 'recovering' && (
        <div style={{
          position: 'absolute', inset: 0, display: 'flex', alignItems: 'center', justifyContent: 'center',
          background: 'rgba(0,0,0,0.7)', color: '#aaa', fontFamily: 'monospace', fontSize: 12,
        }}>
          Reconnecting GPU...
        </div>
      )}
      {engineState === 'failed' && (
        <div style={{
          position: 'absolute', inset: 0, display: 'flex', alignItems: 'center', justifyContent: 'center',
          background: 'rgba(0,0,0,0.8)', color: '#e74c3c', fontFamily: 'monospace', fontSize: 12, cursor: 'pointer',
        }} onClick={() => getRenderEngine().retry()}>
          GPU unavailable — click to retry
        </div>
      )}
      {pickerPos && (
        <SymbolPicker
          paneId={paneConfig?.id ?? paneId}
          anchorX={pickerPos.x}
          anchorY={pickerPos.y}
          onClose={() => setPickerPos(null)}
        />
      )}
      {/* Drag-zoom selection rectangle — updated imperatively, never triggers re-renders */}
      <div ref={dragZoomDivRef} style={{
        display: 'none', position: 'absolute', pointerEvents: 'none',
        border: '1px solid rgba(110, 190, 255, 0.75)',
        background: 'rgba(110, 190, 255, 0.08)',
        borderRadius: 2,
      }} />
      {contextMenu && (
        <ChartContextMenu
          x={contextMenu.x}
          y={contextMenu.y}
          symbol={symbol}
          paneId={paneConfig?.id ?? paneId}
          orderPaneId={paneId}
          clickPrice={contextMenu.clickPrice}
          orderEntryEnabled={orderEntryEnabled}
          onReset={() => { resetView(); resetYZoom() }}
          onDragZoom={() => setDragZoomMode(true)}
          onClose={() => setContextMenu(null)}
        />
      )}
      {orderEntryEnabled && cs && (
        <OrderEntry
          paneId={paneId}
          symbol={symbol}
          timeframe={timeframe}
          cs={cs}
          theme={theme}
        />
      )}
      {orderEntryEnabled && cs && (
        <OrderLevels paneId={paneId} cs={cs} theme={theme} onPauseScroll={pauseAutoScroll} />
      )}
    </div>
  )
}
