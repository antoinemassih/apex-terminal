import { useRef, useCallback, useState, useEffect } from 'react'
import { useDrawingStore } from '../store/drawingStore'
import { CoordSystem } from './CoordSystem'
import type { Point, Timeframe } from '../types'
import { v4 as uuid } from 'uuid'

interface Props {
  symbol: string
  timeframe: Timeframe
  cs: CoordSystem
  width: number
  height: number
  viewStart: number
}

export function DrawingOverlay({ symbol, timeframe, cs, width, height, viewStart }: Props) {
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const { activeTool, drawingsFor, addDrawing } = useDrawingStore()
  const [inProgress, setInProgress] = useState<Point | null>(null)
  const mouseRef = useRef({ x: 0, y: 0 })

  const draw = useCallback(() => {
    const canvas = canvasRef.current
    if (!canvas) return
    const ctx = canvas.getContext('2d')!
    ctx.clearRect(0, 0, width, height)

    const drawings = drawingsFor(symbol, timeframe)
    ctx.lineWidth = 1.5

    for (const d of drawings) {
      ctx.strokeStyle = d.color
      if (d.type === 'trendline' && d.points.length === 2) {
        ctx.beginPath()
        ctx.moveTo(cs.barToX(d.points[0].time - viewStart), cs.priceToY(d.points[0].price))
        ctx.lineTo(cs.barToX(d.points[1].time - viewStart), cs.priceToY(d.points[1].price))
        ctx.stroke()
      }
      if (d.type === 'hline' && d.points.length >= 1) {
        const y = cs.priceToY(d.points[0].price)
        ctx.beginPath()
        ctx.moveTo(0, y)
        ctx.lineTo(width - cs.pr, y)
        ctx.stroke()
      }
    }

    if (inProgress && activeTool === 'trendline') {
      ctx.strokeStyle = 'rgba(74,158,255,0.6)'
      ctx.setLineDash([4, 4])
      ctx.beginPath()
      ctx.moveTo(cs.barToX(inProgress.time - viewStart), cs.priceToY(inProgress.price))
      ctx.lineTo(mouseRef.current.x, mouseRef.current.y)
      ctx.stroke()
      ctx.setLineDash([])
    }
  }, [cs, symbol, timeframe, drawingsFor, activeTool, inProgress, width, height, viewStart])

  useEffect(() => { draw() }, [draw])

  const onClick = useCallback((e: React.MouseEvent) => {
    if (activeTool === 'cursor') return
    const rect = canvasRef.current!.getBoundingClientRect()
    const x = e.clientX - rect.left
    const y = e.clientY - rect.top
    const barIdx = Math.round(cs.xToBar(x)) + viewStart
    const price = cs.yToPrice(y)

    if (activeTool === 'trendline') {
      if (!inProgress) {
        setInProgress({ time: barIdx, price })
      } else {
        addDrawing({
          id: uuid(), type: 'trendline',
          points: [inProgress, { time: barIdx, price }],
          color: '#4a9eff', symbol, timeframe,
        })
        setInProgress(null)
      }
    }
    if (activeTool === 'hline') {
      addDrawing({
        id: uuid(), type: 'hline',
        points: [{ time: barIdx, price }],
        color: '#4a9eff', symbol, timeframe,
      })
    }
  }, [activeTool, inProgress, cs, addDrawing, symbol, timeframe, viewStart])

  const onMouseMove = useCallback((e: React.MouseEvent) => {
    const rect = canvasRef.current!.getBoundingClientRect()
    mouseRef.current = { x: e.clientX - rect.left, y: e.clientY - rect.top }
    if (inProgress) draw()
  }, [inProgress, draw])

  return (
    <canvas ref={canvasRef} width={width} height={height}
      style={{
        position: 'absolute', top: 0, left: 0,
        cursor: activeTool !== 'cursor' ? 'crosshair' : 'default',
        pointerEvents: activeTool === 'cursor' ? 'none' : 'auto',
      }}
      onClick={onClick} onMouseMove={onMouseMove} />
  )
}
