import { useEffect, useRef, useCallback, useState } from 'react'
import { getRenderEngine, getDataStore, getIndicatorEngine, getFeed } from '../globals'
import type { PaneContext, EngineState } from '../engine'
import { useChartViewport } from './useChartViewport'
import { AxisCanvas } from './AxisCanvas'
import { CrosshairOverlay, CrosshairHandle } from './CrosshairOverlay'
import { DrawingOverlay, DrawingOverlayHandle } from './DrawingOverlay'
import { useDrawingStore } from '../store/drawingStore'
import { useChartStore } from '../store/chartStore'
import { getTheme } from '../themes'
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
  const [engineState, setEngineState] = useState<EngineState>('ready')
  const { viewport, pan, zoomX, zoomY, resetYZoom, autoScrolling, pauseAutoScroll } =
    useChartViewport(symbol, timeframe, width, height)

  const paneConfig = useChartStore(s => s.panes[paneIndex])
  const showVolume = paneConfig?.showVolume ?? true
  const visibleIndicators = paneConfig?.visibleIndicators ?? []
  const themeName = useChartStore(s => s.theme)
  const theme = getTheme(themeName)

  const { cs } = viewport
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
    const feed = getFeed()

    // Subscribe this symbol+timeframe to the feed so ticks arrive
    feed.subscribe(symbol, timeframe)

    // Load historical data, then seed simulation
    ds.load(symbol, timeframe).then(({ data: store }) => {
      if (store.length > 0) {
        feed.setLastPrice(symbol, timeframe, store.closes[store.length - 1], store.times[store.length - 1])
      }
    }).catch(err => console.error(`Failed to load ${symbol}:${timeframe}:`, err))

    // Subscribe to data changes → push to PaneContext
    const unsub = ds.subscribe(symbol, timeframe, () => {
      const d = ds.getData(symbol, timeframe)
      const indicators = ds.getIndicators(symbol, timeframe)
      const action = ds.getLastAction(symbol, timeframe)
      if (d && indicators) paneRef.current?.setData(d, indicators, action)
    })

    return () => {
      unsub()
      feed.unsubscribe(symbol, timeframe)
    }
  }, [symbol, timeframe])

  // Push viewport to PaneContext when it changes
  useEffect(() => {
    if (cs) {
      paneRef.current?.setViewport({ viewStart: viewport.viewStart, viewCount: viewport.viewCount, cs })
    }
  }, [viewport, cs])

  // Push visibility settings to PaneContext
  useEffect(() => {
    const outputs = getIndicatorEngine().getOutputs(symbol, timeframe)
      .filter(out => visibleIndicators.includes(out.indicatorId))
    paneRef.current?.setVisibility(showVolume, outputs)
  }, [showVolume, visibleIndicators, symbol, timeframe, data])

  // Push theme to PaneContext
  useEffect(() => {
    paneRef.current?.setTheme(themeName)
  }, [themeName])

  // --- Drag handling ---
  const dragRef = useRef<{ x: number; y: number; zone: 'chart' | 'xaxis' | 'yaxis' } | null>(null)

  const getZone = useCallback((e: React.MouseEvent): 'chart' | 'xaxis' | 'yaxis' => {
    if (!cs) return 'chart'
    const rect = (e.currentTarget as HTMLElement).getBoundingClientRect()
    const x = e.clientX - rect.left
    const y = e.clientY - rect.top
    if (x >= width - cs.pr && y < height - cs.pb) return 'yaxis'
    if (y >= height - cs.pb) return 'xaxis'
    return 'chart'
  }, [cs, width, height])

  const onMouseDown = useCallback((e: React.MouseEvent) => {
    if (e.button !== 0) return
    const rect = canvasRef.current?.getBoundingClientRect()
    if (!rect) return
    const mx = e.clientX - rect.left
    const my = e.clientY - rect.top
    if (drawingRef.current?.handleMouseDown(mx, my)) return
    const zone = getZone(e)
    dragRef.current = { x: e.clientX, y: e.clientY, zone }
  }, [getZone])

  const onMouseMove = useCallback((e: React.MouseEvent) => {
    const rect = canvasRef.current?.getBoundingClientRect()
    if (!rect) return
    const mx = e.clientX - rect.left
    const my = e.clientY - rect.top
    if (cs && data) crosshairRef.current?.update(mx, my)
    drawingRef.current?.handleMouseMove(mx, my)
    if (!dragRef.current) return
    const dx = e.clientX - dragRef.current.x
    const dy = e.clientY - dragRef.current.y
    switch (dragRef.current.zone) {
      case 'chart': pan(dx); break
      case 'xaxis': if (Math.abs(dx) > 2) zoomX(dx > 0 ? 1.02 : 0.98); break
      case 'yaxis': if (Math.abs(dy) > 2) zoomY(dy > 0 ? 1.02 : 0.98); break
    }
    dragRef.current = { ...dragRef.current, x: e.clientX, y: e.clientY }
  }, [pan, zoomX, zoomY, cs, data])

  const onMouseUp = useCallback(() => {
    dragRef.current = null
    drawingRef.current?.handleMouseUp()
  }, [])

  const onMouseLeave = useCallback(() => {
    dragRef.current = null
    drawingRef.current?.handleMouseUp()
    crosshairRef.current?.clear()
  }, [])

  const onWheel = useCallback((e: React.WheelEvent) => {
    e.preventDefault()
    if (!cs) return
    const rect = (e.currentTarget as HTMLElement).getBoundingClientRect()
    const x = e.clientX - rect.left
    const y = e.clientY - rect.top
    const factor = e.deltaY > 0 ? 1.1 : 0.9
    if (x >= width - cs.pr && y < height - cs.pb) {
      zoomY(factor, cs.yToPrice(y))
    } else {
      zoomX(factor)
    }
  }, [cs, zoomX, zoomY, width, height])

  const onAuxClick = useCallback((e: React.MouseEvent) => {
    if (e.button === 1) { e.preventDefault(); useDrawingStore.getState().toggleDrawTool() }
  }, [])

  const onDoubleClick = useCallback((e: React.MouseEvent) => {
    if (!cs) return
    const rect = (e.currentTarget as HTMLElement).getBoundingClientRect()
    if (e.clientX - rect.left >= width - cs.pr) resetYZoom()
  }, [cs, width, resetYZoom])

  // Cursor
  const [cursorStyle, setCursorStyle] = useState('default')
  const onMouseMoveForCursor = useCallback((e: React.MouseEvent) => {
    const zone = getZone(e)
    onMouseMove(e)
    const drawCursor = drawingRef.current?.getCursor()
    if (zone === 'chart' && drawCursor) setCursorStyle(drawCursor)
    else if (zone === 'yaxis') setCursorStyle('ns-resize')
    else if (zone === 'xaxis') setCursorStyle('ew-resize')
    else setCursorStyle('default')
  }, [getZone, onMouseMove])

  return (
    <div
      style={{ position: 'relative', width, height, background: theme.background, cursor: cursorStyle }}
      onMouseDown={onMouseDown}
      onMouseMove={onMouseMoveForCursor}
      onMouseUp={onMouseUp}
      onMouseLeave={onMouseLeave}
      onWheel={onWheel}
      onDoubleClick={onDoubleClick}
      onAuxClick={onAuxClick}
    >
      <canvas ref={canvasRef} width={width} height={height}
        style={{ display: 'block', pointerEvents: 'none' }} />
      {/* OHLC label */}
      <div style={{
        position: 'absolute', top: 4, left: 8,
        color: theme.axisText, fontSize: 11, fontFamily: 'monospace', pointerEvents: 'none',
      }}>
        {symbol} · {timeframe}
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
      <AxisCanvas cs={cs} data={data} viewStart={viewport.viewStart} width={width} height={height} />
      {cs && data && (
        <DrawingOverlay ref={drawingRef} symbol={symbol} timeframe={timeframe} cs={cs}
          width={width} height={height} viewStart={viewport.viewStart}
          onInteraction={pauseAutoScroll} />
      )}
      {!autoScrolling && (
        <div style={{
          position: 'absolute', bottom: cs ? cs.pb + 4 : 44, right: cs ? cs.pr + 4 : 84,
          color: '#555', fontSize: 9, fontFamily: 'monospace', pointerEvents: 'none',
          background: '#1a1a1a', padding: '1px 4px', borderRadius: 2,
        }}>
          SCROLL PAUSED
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
    </div>
  )
}
