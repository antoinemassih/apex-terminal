import { useRef, useCallback, useState, useEffect } from 'react'
import { useDrawingStore } from '../store/drawingStore'
import { CoordSystem } from './CoordSystem'
import type { Point, Timeframe } from '../types'
import { v4 as uuid } from 'uuid'

const HIT_RADIUS = 8       // px distance to count as "near" a line
const HANDLE_RADIUS = 5    // px radius of endpoint handles

interface Props {
  symbol: string
  timeframe: Timeframe
  cs: CoordSystem
  width: number
  height: number
  viewStart: number
  onInteraction?: () => void
}

type DragState = {
  drawingId: string
  mode: 'move' | 'endpoint'
  pointIndex: number        // which endpoint (for endpoint mode)
  startMouse: { x: number; y: number }
  origPoints: Point[]
}

/** Point-to-segment distance in pixels */
function distToSegment(px: number, py: number, x0: number, y0: number, x1: number, y1: number): number {
  const dx = x1 - x0, dy = y1 - y0
  const lenSq = dx * dx + dy * dy
  if (lenSq === 0) return Math.hypot(px - x0, py - y0)
  const t = Math.max(0, Math.min(1, ((px - x0) * dx + (py - y0) * dy) / lenSq))
  return Math.hypot(px - (x0 + t * dx), py - (y0 + t * dy))
}

export function DrawingOverlay({ symbol, timeframe, cs, width, height, viewStart, onInteraction }: Props) {
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const { activeTool, drawingsFor, addDrawing, updateDrawing, selectedId, selectDrawing, setActiveTool } = useDrawingStore()
  const [inProgress, setInProgress] = useState<Point | null>(null)
  const mouseRef = useRef({ x: 0, y: 0 })
  const dragRef = useRef<DragState | null>(null)
  const [hoveringDrawing, setHoveringDrawing] = useState(false)

  // Convert drawing point to pixel
  const toPixel = useCallback((p: Point) => ({
    x: cs.barToX(p.time - viewStart),
    y: cs.priceToY(p.price),
  }), [cs, viewStart])

  // Convert pixel to drawing point
  const toPoint = useCallback((px: number, py: number): Point => ({
    time: Math.round(cs.xToBar(px)) + viewStart,
    price: cs.yToPrice(py),
  }), [cs, viewStart])

  // Hit test: find drawing near pixel position, returns { id, pointIndex } or null
  const hitTest = useCallback((mx: number, my: number): { id: string; nearEndpoint: number } | null => {
    const drawings = drawingsFor(symbol, timeframe)
    for (const d of drawings) {
      if (d.type === 'trendline' && d.points.length === 2) {
        const p0 = toPixel(d.points[0]), p1 = toPixel(d.points[1])
        // Check endpoints first (higher priority)
        if (Math.hypot(mx - p0.x, my - p0.y) < HANDLE_RADIUS + 3) return { id: d.id, nearEndpoint: 0 }
        if (Math.hypot(mx - p1.x, my - p1.y) < HANDLE_RADIUS + 3) return { id: d.id, nearEndpoint: 1 }
        // Check line body
        if (distToSegment(mx, my, p0.x, p0.y, p1.x, p1.y) < HIT_RADIUS) return { id: d.id, nearEndpoint: -1 }
      }
      if (d.type === 'hline' && d.points.length >= 1) {
        const y = cs.priceToY(d.points[0].price)
        if (Math.abs(my - y) < HIT_RADIUS && mx < width - cs.pr) return { id: d.id, nearEndpoint: -1 }
      }
    }
    return null
  }, [drawingsFor, symbol, timeframe, toPixel, cs, width])

  // --- Drawing ---
  const draw = useCallback(() => {
    const canvas = canvasRef.current
    if (!canvas) return
    const ctx = canvas.getContext('2d')!
    ctx.clearRect(0, 0, width, height)

    const drawings = drawingsFor(symbol, timeframe)

    for (const d of drawings) {
      const isSelected = d.id === selectedId
      ctx.strokeStyle = isSelected ? '#fff' : d.color
      ctx.lineWidth = isSelected ? 2 : 1.5

      if (d.type === 'trendline' && d.points.length === 2) {
        const p0 = toPixel(d.points[0]), p1 = toPixel(d.points[1])
        ctx.beginPath()
        ctx.moveTo(p0.x, p0.y)
        ctx.lineTo(p1.x, p1.y)
        ctx.stroke()

        // Draw endpoint handles when selected
        if (isSelected) {
          for (const p of [p0, p1]) {
            ctx.fillStyle = '#4a9eff'
            ctx.beginPath()
            ctx.arc(p.x, p.y, HANDLE_RADIUS, 0, Math.PI * 2)
            ctx.fill()
            ctx.strokeStyle = '#fff'
            ctx.lineWidth = 1
            ctx.stroke()
          }
          ctx.lineWidth = 2
        }
      }
      if (d.type === 'hline' && d.points.length >= 1) {
        const y = cs.priceToY(d.points[0].price)
        ctx.beginPath()
        ctx.moveTo(0, y)
        ctx.lineTo(width - cs.pr, y)
        ctx.stroke()

        // Handle on the right when selected
        if (isSelected) {
          ctx.fillStyle = '#4a9eff'
          ctx.beginPath()
          ctx.arc(width - cs.pr - 10, y, HANDLE_RADIUS, 0, Math.PI * 2)
          ctx.fill()
          ctx.strokeStyle = '#fff'
          ctx.lineWidth = 1
          ctx.stroke()
        }
      }
    }

    // In-progress drawing
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

  // --- Mouse handlers ---
  const onMouseDown = useCallback((e: React.MouseEvent) => {
    if (e.button !== 0) return // left click only
    const rect = canvasRef.current!.getBoundingClientRect()
    const mx = e.clientX - rect.left
    const my = e.clientY - rect.top

    // In cursor mode: try to select/grab a drawing
    if (activeTool === 'cursor') {
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
        e.stopPropagation()
      } else {
        selectDrawing(null)
      }
      return
    }

    // In draw mode: create drawings
    onInteraction?.()
    if (activeTool === 'trendline') {
      if (!inProgress) {
        setInProgress(toPoint(mx, my))
      } else {
        addDrawing({
          id: uuid(), type: 'trendline',
          points: [inProgress, toPoint(mx, my)],
          color: '#4a9eff', symbol, timeframe,
        })
        setInProgress(null)
        setActiveTool('cursor') // auto-switch back to cursor
      }
    }
    if (activeTool === 'hline') {
      addDrawing({
        id: uuid(), type: 'hline',
        points: [toPoint(mx, my)],
        color: '#4a9eff', symbol, timeframe,
      })
      setActiveTool('cursor') // auto-switch back to cursor
    }
  }, [activeTool, inProgress, cs, addDrawing, symbol, timeframe, viewStart, hitTest, selectDrawing, drawingsFor, toPoint, onInteraction, setActiveTool])

  const onMouseMove = useCallback((e: React.MouseEvent) => {
    const rect = canvasRef.current!.getBoundingClientRect()
    const mx = e.clientX - rect.left
    const my = e.clientY - rect.top
    mouseRef.current = { x: mx, y: my }

    // Dragging a selected drawing
    if (dragRef.current) {
      e.stopPropagation()
      const { drawingId, mode, pointIndex, origPoints } = dragRef.current
      const drawing = drawingsFor(symbol, timeframe).find(d => d.id === drawingId)
      if (!drawing) return

      if (mode === 'endpoint') {
        // Move a single endpoint
        const newPoints = origPoints.map((p, i) => i === pointIndex ? toPoint(mx, my) : { ...p })
        updateDrawing(drawingId, newPoints)
      } else {
        // Move entire drawing
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

    // Update cursor and hover state based on what's under mouse
    if (activeTool === 'cursor') {
      const hit = hitTest(mx, my)
      const canvas = canvasRef.current!
      if (hit && hit.nearEndpoint >= 0) {
        canvas.style.cursor = 'grab'
        setHoveringDrawing(true)
      } else if (hit) {
        canvas.style.cursor = 'move'
        setHoveringDrawing(true)
      } else {
        canvas.style.cursor = 'default'
        setHoveringDrawing(false)
      }
    }

    if (inProgress) draw()
  }, [inProgress, draw, activeTool, hitTest, drawingsFor, symbol, timeframe, toPoint, toPixel, updateDrawing])

  const onMouseUp = useCallback(() => {
    dragRef.current = null
  }, [])

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

  const isDrawMode = activeTool !== 'cursor'

  return (
    <canvas ref={canvasRef} width={width} height={height}
      style={{
        position: 'absolute', top: 0, left: 0,
        cursor: isDrawMode ? 'crosshair' : (hoveringDrawing ? undefined : 'default'),
        pointerEvents: 'auto',
      }}
      onMouseDown={onMouseDown}
      onMouseMove={onMouseMove}
      onMouseUp={onMouseUp} />
  )
}
