import { useRef, useCallback, useState, useEffect, forwardRef, useImperativeHandle } from 'react'
import { useDrawingStore } from '../store/drawingStore'
import { useChartStore } from '../store/chartStore'
import { LineStylePopup } from './LineStylePopup'
import { CoordSystem } from './CoordSystem'
import type { Point, Timeframe } from '../types'
import { v4 as uuid } from 'uuid'

const HIT_RADIUS = 8
const HANDLE_RADIUS = 5

interface Props {
  symbol: string
  timeframe: Timeframe
  cs: CoordSystem
  data: import('../data/columns').ColumnStore
  width: number
  height: number
  viewStart: number
  onInteraction?: () => void
}

export interface DrawingOverlayHandle {
  /** Returns true if the drawing layer handled this mousedown (hit a drawing or in draw mode) */
  handleMouseDown: (mx: number, my: number, shiftKey?: boolean) => boolean
  handleMouseMove: (mx: number, my: number) => void
  handleMouseUp: () => void
  /** Returns a cursor string if drawings want to override, or null for default */
  getCursor: () => string | null
  /** Imperatively update viewport (cs + viewStart) and immediately redraw — bypasses React */
  setViewport(newCs: CoordSystem, newViewStart: number): void
}

type DragState = {
  drawingId: string
  mode: 'move' | 'endpoint'
  pointIndex: number
  startMouse: { x: number; y: number }
  origPoints: Point[]
}

function distToSegment(px: number, py: number, x0: number, y0: number, x1: number, y1: number): number {
  const dx = x1 - x0, dy = y1 - y0
  const lenSq = dx * dx + dy * dy
  if (lenSq === 0) return Math.hypot(px - x0, py - y0)
  const t = Math.max(0, Math.min(1, ((px - x0) * dx + (py - y0) * dy) / lenSq))
  return Math.hypot(px - (x0 + t * dx), py - (y0 + t * dy))
}

export const DrawingOverlay = forwardRef<DrawingOverlayHandle, Props>(
  function DrawingOverlay({ symbol, timeframe, cs, data: chartData, width, height, viewStart, onInteraction }, ref) {
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const { activeTool, drawingsFor, addDrawing, updateDrawing, selectedId, selectedIds, selectDrawing, toggleSelectDrawing, setActiveTool } = useDrawingStore()
  const drawingsHidden = useDrawingStore(s => s.drawingsHidden(symbol))
  const hiddenGroups = useDrawingStore(s => s.hiddenGroups)
  // Only show popup for drawings that belong to this pane's symbol
  const selectedOwnerSymbol = useDrawingStore(s => s.selectedId ? s.drawings.find(d => d.id === s.selectedId)?.symbol : null)
  const annotationFilters = useChartStore(s => s.annotationFilters)
  const [serverAnnotations, setServerAnnotations] = useState<any[]>([])
  const [inProgress, setInProgress] = useState<Point | null>(null)
  const mouseRef = useRef({ x: 0, y: 0 })
  const dragRef = useRef<DragState | null>(null)
  const cursorRef = useRef<string | null>(null)
  const prevToolRef = useRef(activeTool)
  // Always-current reference to draw — lets imperative callers (handle, subscriptions) call latest draw
  const drawRef = useRef<() => void>(() => {})

  // Live refs for viewport — updated imperatively by setViewport() to avoid React re-renders during pan
  const csRef = useRef(cs)
  const vsRef = useRef(Math.floor(viewStart))
  // Keep refs in sync with React props (for React-driven updates: zoom, symbol switch, etc.)
  // vsRef must always hold the floored viewStart — pixelOffset in CoordSystem handles the fractional part.
  useEffect(() => { csRef.current = cs }, [cs])
  useEffect(() => { vsRef.current = Math.floor(viewStart) }, [viewStart])

  // Reset inProgress when tool changes
  if (activeTool !== prevToolRef.current) {
    prevToolRef.current = activeTool
    if (inProgress) setInProgress(null)
  }

  // Fetch server annotations (auto-trendlines etc.) periodically
  useEffect(() => {
    let cancelled = false
    const load = async () => {
      try {
        const ctrl = new AbortController()
        const t = setTimeout(() => ctrl.abort(), 1500)
        const resp = await fetch(`http://192.168.1.60:30300/api/annotations?symbol=${symbol}&source=auto-trend`, { signal: ctrl.signal })
        clearTimeout(t)
        if (resp.ok && !cancelled) {
          const anns = await resp.json()
          setServerAnnotations(anns)
        }
      } catch { /* API unreachable */ }
    }
    load()
    const id = setInterval(load, 30000) // refresh every 30s
    return () => { cancelled = true; clearInterval(id) }
  }, [symbol])

  const toPixel = useCallback((p: Point) => {
    const _cs = csRef.current
    const _vs = vsRef.current
    if (!chartData || chartData.length === 0) return { x: 0, y: _cs.priceToY(p.price) }
    const barIdx = chartData.indexAtTime(p.time)
    let frac = 0
    if (barIdx < chartData.length - 1) {
      const t0 = chartData.times[barIdx]
      const t1 = chartData.times[barIdx + 1]
      if (t1 > t0) frac = (p.time - t0) / (t1 - t0)
    }
    // Mirror the GPU shader's integer-pixel layout exactly:
    //   barLeftPx = viewIdx * stepPx - offsetPx  (physical px)
    //   wickCenter = barLeftPx + halfStepPx       (physical px)
    // Then convert back to CSS px so the 2D canvas overlay aligns with WebGPU candles.
    const dpr         = window.devicePixelRatio || 1
    const stepPx      = Math.round(_cs.barStep    * dpr)
    const halfStepPx  = Math.round(stepPx / 2)
    const offsetPx    = Math.round(_cs.pixelOffset * dpr)
    const viewIdx     = (barIdx + frac) - _vs
    return { x: (viewIdx * stepPx + halfStepPx - offsetPx) / dpr, y: _cs.priceToY(p.price) }
  }, [chartData])

  const toPoint = useCallback((px: number, py: number): Point => {
    const _cs = csRef.current
    const _vs = vsRef.current
    if (!chartData || chartData.length === 0) return { time: 0, price: _cs.yToPrice(py) }
    // Invert the GPU formula: px (CSS) → physical → view bar index → absolute bar index
    const dpr        = window.devicePixelRatio || 1
    const stepPx     = Math.round(_cs.barStep    * dpr)
    const halfStepPx = Math.round(stepPx / 2)
    const offsetPx   = Math.round(_cs.pixelOffset * dpr)
    const viewIdx    = (px * dpr - halfStepPx + offsetPx) / stepPx
    const rawIdx     = viewIdx + _vs
    const barIdx     = Math.max(0, Math.min(chartData.length - 1, Math.round(rawIdx)))
    return { time: chartData.times[barIdx], price: _cs.yToPrice(py) }
  }, [chartData])

  const hitTest = useCallback((mx: number, my: number): { id: string; nearEndpoint: number } | null => {
    const _cs = csRef.current
    if (mx >= width - _cs.pr || my >= height - _cs.pb) return null
    const drawings = drawingsFor(symbol, timeframe)
    for (const d of drawings) {
      if (d.type === 'trendline' && d.points.length === 2) {
        const p0 = toPixel(d.points[0]), p1 = toPixel(d.points[1])
        if (Math.hypot(mx - p0.x, my - p0.y) < HANDLE_RADIUS + 3) return { id: d.id, nearEndpoint: 0 }
        if (Math.hypot(mx - p1.x, my - p1.y) < HANDLE_RADIUS + 3) return { id: d.id, nearEndpoint: 1 }
        if (distToSegment(mx, my, p0.x, p0.y, p1.x, p1.y) < HIT_RADIUS) return { id: d.id, nearEndpoint: -1 }
      }
      if (d.type === 'hline' && d.points.length >= 1) {
        const y = _cs.priceToY(d.points[0].price)
        if (Math.abs(my - y) < HIT_RADIUS && mx < width - _cs.pr) return { id: d.id, nearEndpoint: -1 }
      }
      if (d.type === 'hzone' && d.points.length === 2) {
        const y0 = _cs.priceToY(d.points[0].price)
        const y1 = _cs.priceToY(d.points[1].price)
        const top = Math.min(y0, y1), bot = Math.max(y0, y1)
        if (my >= top - 4 && my <= bot + 4 && mx < width - _cs.pr) {
          if (Math.abs(my - y0) < HIT_RADIUS) return { id: d.id, nearEndpoint: 0 }
          if (Math.abs(my - y1) < HIT_RADIUS) return { id: d.id, nearEndpoint: 1 }
          return { id: d.id, nearEndpoint: -1 }
        }
      }
      if (d.type === 'barmarker' && d.points.length >= 1) {
        const px = toPixel(d.points[0])
        if (Math.hypot(mx - px.x, my - px.y) < HIT_RADIUS + 4) return { id: d.id, nearEndpoint: -1 }
      }
    }
    return null
  }, [drawingsFor, symbol, timeframe, toPixel, width, height])

  // --- Drawing ---
  const draw = useCallback(() => {
    const canvas = canvasRef.current
    if (!canvas) return
    const _cs = csRef.current
    const ctx = canvas.getContext('2d')!
    const cw = width - _cs.pr
    const ch = height - _cs.pb
    ctx.clearRect(0, 0, cw, ch)

    if (drawingsHidden) return

    const drawings = drawingsFor(symbol, timeframe)
    const hiddenGroupSet = new Set(hiddenGroups)

    for (const d of drawings) {
      if (hiddenGroupSet.has(d.groupId ?? 'default')) continue
      const _selIds = useDrawingStore.getState().selectedIds
      const isSelected = _selIds.includes(d.id)
      const dOpacity = d.opacity ?? 1
      const dLineStyle = d.lineStyle ?? 'solid'
      const dThickness = d.thickness ?? 1.5

      ctx.globalAlpha = dOpacity
      ctx.setLineDash(dLineStyle === 'dashed' ? [8, 4] : dLineStyle === 'dotted' ? [2, 3] : [])
      ctx.strokeStyle = isSelected ? '#fff' : d.color
      ctx.lineWidth = isSelected ? dThickness + 0.5 : dThickness

      if (d.type === 'trendline' && d.points.length === 2) {
        const p0 = toPixel(d.points[0]), p1 = toPixel(d.points[1])
        ctx.beginPath()
        ctx.moveTo(p0.x, p0.y)
        ctx.lineTo(p1.x, p1.y)
        ctx.stroke()

        if (isSelected) {
          ctx.globalAlpha = 1
          ctx.setLineDash([])
          for (const p of [p0, p1]) {
            ctx.fillStyle = '#4a9eff'
            ctx.beginPath()
            ctx.arc(p.x, p.y, HANDLE_RADIUS, 0, Math.PI * 2)
            ctx.fill()
            ctx.strokeStyle = '#fff'
            ctx.lineWidth = 1
            ctx.stroke()
          }
        }
      }
      if (d.type === 'hline' && d.points.length >= 1) {
        const y = _cs.priceToY(d.points[0].price)
        ctx.beginPath()
        ctx.moveTo(0, y)
        ctx.lineTo(cw, y)
        ctx.stroke()

        if (isSelected) {
          ctx.globalAlpha = 1
          ctx.setLineDash([])
          ctx.fillStyle = '#4a9eff'
          ctx.beginPath()
          ctx.arc(cw - 10, y, HANDLE_RADIUS, 0, Math.PI * 2)
          ctx.fill()
          ctx.strokeStyle = '#fff'
          ctx.lineWidth = 1
          ctx.stroke()
        }
      }

      // Horizontal zone (two hlines with filled area)
      if (d.type === 'hzone' && d.points.length === 2) {
        const y0 = _cs.priceToY(d.points[0].price)
        const y1 = _cs.priceToY(d.points[1].price)
        const top = Math.min(y0, y1), bot = Math.max(y0, y1)

        // Fill
        ctx.fillStyle = isSelected ? 'rgba(255,255,255,0.08)' : (d.color + '18')
        ctx.fillRect(0, top, cw, bot - top)

        // Top and bottom lines
        ctx.beginPath()
        ctx.moveTo(0, y0); ctx.lineTo(cw, y0)
        ctx.moveTo(0, y1); ctx.lineTo(cw, y1)
        ctx.stroke()

        if (isSelected) {
          ctx.globalAlpha = 1
          ctx.setLineDash([])
          ctx.fillStyle = '#4a9eff'
          for (const y of [y0, y1]) {
            ctx.beginPath()
            ctx.arc(cw - 10, y, HANDLE_RADIUS, 0, Math.PI * 2)
            ctx.fill()
            ctx.strokeStyle = '#fff'
            ctx.lineWidth = 1
            ctx.stroke()
          }
        }
      }

      // Bar marker (triangle/arrow anchored to bar high or low)
      if (d.type === 'barmarker' && d.points.length >= 1) {
        const px = toPixel(d.points[0])
        const isTop = d.points[0].price >= (chartData ? (chartData.opens[Math.max(0, Math.min(chartData.length - 1, chartData.indexAtTime(d.points[0].time)))] + chartData.closes[Math.max(0, Math.min(chartData.length - 1, chartData.indexAtTime(d.points[0].time)))]) / 2 : d.points[0].price)
        const dir = isTop ? -1 : 1 // -1 = pointing down from above, 1 = pointing up from below
        const sz = 6

        ctx.fillStyle = isSelected ? '#fff' : d.color
        ctx.beginPath()
        ctx.moveTo(px.x, px.y + dir * 2)
        ctx.lineTo(px.x - sz, px.y + dir * (sz + 4))
        ctx.lineTo(px.x + sz, px.y + dir * (sz + 4))
        ctx.closePath()
        ctx.fill()

        if (isSelected) {
          ctx.strokeStyle = '#fff'
          ctx.lineWidth = 1
          ctx.stroke()
        }
      }

      ctx.globalAlpha = 1
      ctx.setLineDash([])
    }

    // Render server annotations (auto-trendlines) with filter logic
    for (const ann of serverAnnotations) {
      const tags: string[] = ann.tags ?? []
      // Apply filters
      if (!annotationFilters.user && ann.source === 'user') continue
      // Timeframe filters
      if (tags.includes('15m') && !annotationFilters['15m']) continue
      if (tags.includes('30m') && !annotationFilters['30m']) continue
      if (tags.includes('1H') && !annotationFilters['1H']) continue
      if (tags.includes('4H') && !annotationFilters['4H']) continue
      if (tags.includes('1D') && !annotationFilters['1D']) continue
      if (tags.includes('1W') && !annotationFilters['1W']) continue
      // Source filters
      if (tags.includes('wick') && !annotationFilters.wick) continue
      if (tags.includes('body') && !annotationFilters.body) continue
      // Type filters
      if (tags.includes('support') && !annotationFilters.support) continue
      if (tags.includes('resistance') && !annotationFilters.resistance) continue
      if (tags.includes('channel') && !annotationFilters.channel) continue
      // Method filters
      if (tags.includes('pivot') && !annotationFilters.pivot) continue
      if (tags.includes('regression') && !annotationFilters.regression) continue
      if (tags.includes('fractal') && !annotationFilters.fractal) continue
      if (tags.includes('volume') && !annotationFilters.volume) continue
      if (tags.includes('density') && !annotationFilters.density) continue

      const style = ann.style ?? {}
      ctx.globalAlpha = style.opacity ?? 0.5
      ctx.strokeStyle = style.color ?? '#888'
      ctx.lineWidth = style.thickness ?? 1
      ctx.setLineDash(style.lineStyle === 'dashed' ? [6, 3] : style.lineStyle === 'dotted' ? [2, 2] : [])

      if (ann.type === 'trendline' && ann.points?.length === 2) {
        const p0 = toPixel(ann.points[0])
        const p1 = toPixel(ann.points[1])

        // Extrapolate the line across the entire visible viewport
        // A trendline extends infinitely — not just between anchor points
        const dx = p1.x - p0.x
        const dy = p1.y - p0.y

        let x0: number, y0: number, x1: number, y1: number
        if (Math.abs(dx) < 0.001) {
          // Vertical line (shouldn't happen for trendlines, but handle it)
          x0 = p0.x; y0 = 0; x1 = p0.x; y1 = height
        } else {
          // Extend to left edge (x=0) and right edge (x=cw)
          const slope = dy / dx
          x0 = 0
          y0 = p0.y + slope * (0 - p0.x)
          x1 = cw
          y1 = p0.y + slope * (cw - p0.x)
        }

        // Only draw if the line crosses the visible area
        const yMin = Math.min(y0, y1)
        const yMax = Math.max(y0, y1)
        if (yMax >= 0 && yMin <= height) {
          ctx.beginPath()
          ctx.moveTo(x0, y0)
          ctx.lineTo(x1, y1)
          ctx.stroke()

          // Label with strength score at the right edge of the viewport
          const meta = ann.metadata ?? {}
          if (meta.label) {
            const strength = meta.strength?.total ?? Math.round((ann.strength ?? 0) * 100)
            const labelText = `${meta.label} [${strength}]`
            ctx.font = '8px monospace'
            ctx.fillStyle = style.color ?? '#888'
            ctx.globalAlpha = 0.7
            // Position label near the right side of visible area
            const labelX = Math.min(cw - 120, Math.max(10, (p0.x + p1.x) / 2))
            const labelY = p0.y + (dy / dx) * (labelX - p0.x)
            if (labelY > 10 && labelY < height - 20) {
              ctx.fillText(labelText, labelX, labelY - 4)
            }
          }
        }
      }

      ctx.globalAlpha = 1
      ctx.setLineDash([])
    }

    if (inProgress && activeTool === 'trendline') {
      ctx.strokeStyle = 'rgba(74,158,255,0.6)'
      ctx.setLineDash([4, 4])
      ctx.lineWidth = 1.5
      ctx.beginPath()
      const p = toPixel(inProgress)
      ctx.moveTo(p.x, p.y)
      ctx.lineTo(mouseRef.current.x, mouseRef.current.y)
      ctx.stroke()
      ctx.setLineDash([])
    }
    if (inProgress && activeTool === 'hzone') {
      const y0 = _cs.priceToY(inProgress.price)
      const y1 = mouseRef.current.y
      const top = Math.min(y0, y1), bot = Math.max(y0, y1)
      ctx.fillStyle = 'rgba(74,158,255,0.08)'
      ctx.fillRect(0, top, cw, bot - top)
      ctx.strokeStyle = 'rgba(74,158,255,0.6)'
      ctx.setLineDash([4, 4])
      ctx.lineWidth = 1
      ctx.beginPath()
      ctx.moveTo(0, y0); ctx.lineTo(cw, y0)
      ctx.moveTo(0, y1); ctx.lineTo(cw, y1)
      ctx.stroke()
      ctx.setLineDash([])
    }
  }, [symbol, timeframe, drawingsFor, activeTool, inProgress, width, height, selectedIds, toPixel, serverAnnotations, annotationFilters, drawingsHidden, hiddenGroups])

  // Always-current draw ref — updated synchronously so imperative callers never use a stale closure
  drawRef.current = draw

  // Redraw when non-viewport things change (drawings, annotations, active tool, etc.)
  // Viewport changes during pan are handled imperatively via setViewport — no rAF needed here.
  useEffect(() => { draw() }, [draw])

  // Expose imperative handle for ChartPane to call
  useImperativeHandle(ref, () => ({
    handleMouseDown(mx: number, my: number, shiftKey?: boolean): boolean {
      // In draw mode: always handle
      if (activeTool !== 'cursor') {
        onInteraction?.()
        if (activeTool === 'trendline') {
          if (!inProgress) {
            setInProgress(toPoint(mx, my))
          } else {
            addDrawing({
              id: uuid(), type: 'trendline',
              points: [inProgress, toPoint(mx, my)],
              color: '#ff8c00', opacity: 1, lineStyle: 'dashed', thickness: 1,
              symbol, timeframe,
            })
            setInProgress(null)
            setActiveTool('cursor')
          }
        }
        if (activeTool === 'hline') {
          addDrawing({
            id: uuid(), type: 'hline',
            points: [toPoint(mx, my)],
            color: '#ff8c00', opacity: 1, lineStyle: 'dashed', thickness: 1,
            symbol, timeframe,
          })
          setActiveTool('cursor')
        }
        if (activeTool === 'hzone') {
          if (!inProgress) {
            setInProgress(toPoint(mx, my))
          } else {
            addDrawing({
              id: uuid(), type: 'hzone',
              points: [inProgress, toPoint(mx, my)],
              color: '#ff8c00', opacity: 0.5, lineStyle: 'solid', thickness: 1,
              symbol, timeframe,
            })
            setInProgress(null)
            setActiveTool('cursor')
          }
        }
        if (activeTool === 'barmarker') {
          // Snap to bar high or low based on click position
          const pt = toPoint(mx, my)
          if (chartData) {
            const barIdx = chartData.indexAtTime(pt.time)
            if (barIdx >= 0 && barIdx < chartData.length) {
              const high = chartData.highs[barIdx]
              const low = chartData.lows[barIdx]
              const mid = (high + low) / 2
              // Snap to high if clicked above midpoint, low if below
              pt.price = pt.price >= mid ? high : low
            }
          }
          addDrawing({
            id: uuid(), type: 'barmarker',
            points: [pt],
            color: '#ffaa00', opacity: 1, lineStyle: 'solid', thickness: 1,
            symbol, timeframe,
          })
          setActiveTool('cursor')
        }
        return true
      }

      // In cursor mode: check for drawing hit
      const hit = hitTest(mx, my)
      if (hit) {
        onInteraction?.()
        toggleSelectDrawing(hit.id, shiftKey ?? false)
        // Immediately redraw using fresh store state — don't wait for React re-render cycle
        drawRef.current()
        // Only set up drag on plain click — shift-click just toggles selection
        if (!shiftKey) {
          const drawing = drawingsFor(symbol, timeframe).find(d => d.id === hit.id)!
          dragRef.current = {
            drawingId: hit.id,
            mode: hit.nearEndpoint >= 0 ? 'endpoint' : 'move',
            pointIndex: Math.max(0, hit.nearEndpoint),
            startMouse: { x: mx, y: my },
            origPoints: drawing.points.map(p => ({ ...p })),
          }
        }
        return true // consumed — don't start chart pan
      }

      selectDrawing(null)
      return false // not consumed — let chart handle it
    },

    handleMouseMove(mx: number, my: number) {
      mouseRef.current = { x: mx, y: my }

      if (dragRef.current) {
        const { drawingId, mode, pointIndex, origPoints } = dragRef.current
        const drawing = drawingsFor(symbol, timeframe).find(d => d.id === drawingId)
        if (!drawing) return

        if (mode === 'endpoint') {
          const newPoints = origPoints.map((p, i) => i === pointIndex ? toPoint(mx, my) : { ...p })
          updateDrawing(drawingId, newPoints)
        } else {
          const dx = mx - dragRef.current.startMouse.x
          const dy = my - dragRef.current.startMouse.y
          const newPoints = origPoints.map(p => {
            const px = toPixel(p)
            return toPoint(px.x + dx, px.y + dy)
          })
          updateDrawing(drawingId, newPoints)
        }
        drawRef.current()
        return
      }

      // Hover detection for cursor
      if (activeTool === 'cursor') {
        const hit = hitTest(mx, my)
        if (hit && hit.nearEndpoint >= 0) {
          cursorRef.current = 'grab'
        } else if (hit) {
          cursorRef.current = 'move'
        } else {
          cursorRef.current = null
        }
      } else {
        cursorRef.current = 'crosshair'
      }

      if (inProgress) drawRef.current()
    },

    handleMouseUp() {
      dragRef.current = null
    },

    getCursor(): string | null {
      return cursorRef.current
    },

    setViewport(newCs: CoordSystem, newViewStart: number): void {
      csRef.current = newCs
      vsRef.current = newViewStart
      drawRef.current()  // immediate — no rAF scheduling needed
    },
  }))

  // Delete selected drawing with Delete/Backspace
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Delete' || e.key === 'Backspace') {
        const { selectedIds: ids } = useDrawingStore.getState()
        if (ids.length > 0) ids.forEach(id => useDrawingStore.getState().removeDrawing(id))
      }
      if (e.key === 'Escape') {
        setInProgress(null)
        if (activeTool !== 'cursor') setActiveTool('cursor')
      }
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [activeTool, setActiveTool])

  const handleClosePopup = useCallback(() => {
    selectDrawing(null)
  }, [selectDrawing])

  return (
    <>
      <canvas ref={canvasRef} width={width - cs.pr} height={height - cs.pb}
        style={{ position: 'absolute', top: 0, left: 0, pointerEvents: 'none' }} />
      {selectedId && selectedOwnerSymbol === symbol && (
        <LineStylePopup drawingId={selectedId} selectedIds={selectedIds} onClose={handleClosePopup} />
      )}
    </>
  )
})
