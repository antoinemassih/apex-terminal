import { useRef, useCallback, useState, useEffect, useMemo, forwardRef, useImperativeHandle } from 'react'
import { useDrawingStore } from '../store/drawingStore'
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
  width: number
  height: number
  viewStart: number
  onInteraction?: () => void
}

export interface DrawingOverlayHandle {
  /** Returns true if the drawing layer handled this mousedown (hit a drawing or in draw mode) */
  handleMouseDown: (mx: number, my: number) => boolean
  handleMouseMove: (mx: number, my: number) => void
  handleMouseUp: () => void
  /** Returns a cursor string if drawings want to override, or null for default */
  getCursor: () => string | null
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
  function DrawingOverlay({ symbol, timeframe, cs, width, height, viewStart, onInteraction }, ref) {
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const { activeTool, drawingsFor, addDrawing, updateDrawing, selectedId, selectDrawing, setActiveTool } = useDrawingStore()
  const [inProgress, setInProgress] = useState<Point | null>(null)
  const mouseRef = useRef({ x: 0, y: 0 })
  const dragRef = useRef<DragState | null>(null)
  const cursorRef = useRef<string | null>(null)

  const toPixel = useCallback((p: Point) => ({
    x: cs.barToX(p.time - viewStart),
    y: cs.priceToY(p.price),
  }), [cs, viewStart])

  const toPoint = useCallback((px: number, py: number): Point => ({
    time: Math.round(cs.xToBar(px)) + viewStart,
    price: cs.yToPrice(py),
  }), [cs, viewStart])

  const hitTest = useCallback((mx: number, my: number): { id: string; nearEndpoint: number } | null => {
    if (mx >= width - cs.pr || my >= height - cs.pb) return null
    const drawings = drawingsFor(symbol, timeframe)
    for (const d of drawings) {
      if (d.type === 'trendline' && d.points.length === 2) {
        const p0 = toPixel(d.points[0]), p1 = toPixel(d.points[1])
        if (Math.hypot(mx - p0.x, my - p0.y) < HANDLE_RADIUS + 3) return { id: d.id, nearEndpoint: 0 }
        if (Math.hypot(mx - p1.x, my - p1.y) < HANDLE_RADIUS + 3) return { id: d.id, nearEndpoint: 1 }
        if (distToSegment(mx, my, p0.x, p0.y, p1.x, p1.y) < HIT_RADIUS) return { id: d.id, nearEndpoint: -1 }
      }
      if (d.type === 'hline' && d.points.length >= 1) {
        const y = cs.priceToY(d.points[0].price)
        if (Math.abs(my - y) < HIT_RADIUS && mx < width - cs.pr) return { id: d.id, nearEndpoint: -1 }
      }
    }
    return null
  }, [drawingsFor, symbol, timeframe, toPixel, cs, width, height])

  // --- Drawing ---
  const draw = useCallback(() => {
    const canvas = canvasRef.current
    if (!canvas) return
    const ctx = canvas.getContext('2d')!
    const cw = width - cs.pr
    const ch = height - cs.pb
    ctx.clearRect(0, 0, cw, ch)

    const drawings = drawingsFor(symbol, timeframe)

    for (const d of drawings) {
      const isSelected = d.id === selectedId
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
        const y = cs.priceToY(d.points[0].price)
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

      ctx.globalAlpha = 1
      ctx.setLineDash([])
    }

    if (inProgress && (activeTool === 'trendline')) {
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
  }, [cs, symbol, timeframe, drawingsFor, activeTool, inProgress, width, height, viewStart, selectedId, toPixel])

  useEffect(() => { draw() }, [draw])

  // Expose imperative handle for ChartPane to call
  useImperativeHandle(ref, () => ({
    handleMouseDown(mx: number, my: number): boolean {
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
              color: '#4a9eff', opacity: 1, lineStyle: 'solid', thickness: 1.5,
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
            color: '#4a9eff', opacity: 1, lineStyle: 'solid', thickness: 1.5,
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
        selectDrawing(hit.id)
        const drawing = drawingsFor(symbol, timeframe).find(d => d.id === hit.id)!
        dragRef.current = {
          drawingId: hit.id,
          mode: hit.nearEndpoint >= 0 ? 'endpoint' : 'move',
          pointIndex: Math.max(0, hit.nearEndpoint),
          startMouse: { x: mx, y: my },
          origPoints: drawing.points.map(p => ({ ...p })),
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
        draw()
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

      if (inProgress) draw()
    },

    handleMouseUp() {
      dragRef.current = null
    },

    getCursor(): string | null {
      return cursorRef.current
    },
  }))

  // Delete selected drawing with Delete/Backspace
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.key === 'Delete' || e.key === 'Backspace') && selectedId) {
        useDrawingStore.getState().removeDrawing(selectedId)
      }
      if (e.key === 'Escape') {
        setInProgress(null)
        if (activeTool !== 'cursor') setActiveTool('cursor')
      }
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [selectedId, activeTool, setActiveTool])

  // Compute popup position for selected drawing
  const popupPos = useMemo(() => {
    if (!selectedId) return null
    const drawings = drawingsFor(symbol, timeframe)
    const d = drawings.find(dr => dr.id === selectedId)
    if (!d) return null

    if (d.type === 'trendline' && d.points.length === 2) {
      const p0 = toPixel(d.points[0])
      const p1 = toPixel(d.points[1])
      let x = (p0.x + p1.x) / 2 + 12
      let y = (p0.y + p1.y) / 2 - 80
      if (x + 210 > width) x = width - 220
      if (x < 4) x = 4
      if (y < 4) y = 4
      if (y + 160 > height) y = height - 170
      return { x, y }
    }
    if (d.type === 'hline' && d.points.length >= 1) {
      const py = cs.priceToY(d.points[0].price)
      let x = width / 2 - 100
      let y = py - 170
      if (x < 4) x = 4
      if (y < 4) y = py + 12
      return { x, y }
    }
    return null
  }, [selectedId, drawingsFor, symbol, timeframe, toPixel, cs, width, height])

  const handleClosePopup = useCallback(() => {
    selectDrawing(null)
  }, [selectDrawing])

  return (
    <>
      <canvas ref={canvasRef} width={width - cs.pr} height={height - cs.pb}
        style={{ position: 'absolute', top: 0, left: 0, pointerEvents: 'none' }} />
      {selectedId && popupPos && (
        <LineStylePopup drawingId={selectedId} position={popupPos} onClose={handleClosePopup} />
      )}
    </>
  )
})
