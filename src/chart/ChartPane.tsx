import { useEffect, useRef, useCallback } from 'react'
import { getGPUContext, configureCanvas } from '../renderer/gpu'
import { CandleRenderer } from '../renderer/CandleRenderer'
import { GridRenderer } from '../renderer/GridRenderer'
import { LineRenderer } from '../renderer/LineRenderer'
import { sma, ema } from '../data/indicators'
import { useChartData } from './useChartData'
import { CrosshairOverlay, CrosshairHandle } from './CrosshairOverlay'
import { DrawingOverlay } from './DrawingOverlay'
import type { Timeframe } from '../types'

interface Props {
  symbol: string
  timeframe: Timeframe
  width: number
  height: number
}

export function ChartPane({ symbol, timeframe, width, height }: Props) {
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const renderers = useRef<{ candle: CandleRenderer; grid: GridRenderer; sma20: LineRenderer; ema50: LineRenderer } | null>(null)
  const gpuCanvas = useRef<GPUCanvasContext | null>(null)
  const crosshairRef = useRef<CrosshairHandle>(null)
  const axisRef = useRef<HTMLCanvasElement>(null)
  const { data, cs, viewStart, viewCount, pan, zoom } = useChartData(symbol, timeframe, width, height)

  // Init GPU
  useEffect(() => {
    if (!canvasRef.current) return
    let cancelled = false
    getGPUContext().then(ctx => {
      if (cancelled) return
      gpuCanvas.current = configureCanvas(canvasRef.current!, ctx)
      renderers.current = {
        candle: new CandleRenderer(ctx),
        grid: new GridRenderer(ctx),
        sma20: new LineRenderer(ctx),
        ema50: new LineRenderer(ctx),
      }
    })
    return () => {
      cancelled = true
      renderers.current?.candle.destroy()
      renderers.current?.grid.destroy()
      renderers.current?.sma20.destroy()
      renderers.current?.ema50.destroy()
    }
  }, [])

  // Render frame
  useEffect(() => {
    if (!renderers.current || !gpuCanvas.current || !cs || !data) return
    const { candle, grid, sma20: sma20R, ema50: ema50R } = renderers.current

    getGPUContext().then(({ device }) => {
      grid.upload(cs)
      candle.upload(data, cs, viewStart, viewCount)

      const sma20 = sma(data.closes, 20)
      const ema50 = ema(data.closes, 50)
      sma20R.upload(sma20, cs, viewStart, viewCount, [0.3, 0.6, 1.0, 0.8], 1.5)
      ema50R.upload(ema50, cs, viewStart, viewCount, [1.0, 0.6, 0.2, 0.8], 1.5)

      const encoder = device.createCommandEncoder()
      const view = gpuCanvas.current!.getCurrentTexture().createView()

      const pass = encoder.beginRenderPass({
        colorAttachments: [{
          view, loadOp: 'clear',
          clearValue: { r: 0.05, g: 0.05, b: 0.05, a: 1 },
          storeOp: 'store',
        }],
      })
      grid.render(pass)
      candle.render(pass)
      sma20R.render(pass)
      ema50R.render(pass)
      pass.end()

      device.queue.submit([encoder.finish()])
    })
  }, [data, cs, viewStart, viewCount])

  // Axis labels
  useEffect(() => {
    if (!axisRef.current || !cs || !data) return
    const ctx = axisRef.current.getContext('2d')!
    ctx.clearRect(0, 0, width, height)
    ctx.fillStyle = '#444'
    ctx.font = '10px monospace'
    ctx.textAlign = 'left'

    // Price labels
    const priceStep = (cs.maxPrice - cs.minPrice) / 8
    for (let i = 0; i <= 8; i++) {
      const price = cs.minPrice + i * priceStep
      ctx.fillText(price.toFixed(2), width - cs.pr + 4, cs.priceToY(price) + 4)
    }

    // Time labels
    const barStep = Math.max(1, Math.floor(100 / cs.barStep))
    for (let i = 0; i < cs.barCount; i += barStep) {
      const dataIdx = viewStart + i
      if (dataIdx < data.length) {
        const d = new Date(data.times[dataIdx] * 1000)
        const label = `${d.getHours().toString().padStart(2,'0')}:${d.getMinutes().toString().padStart(2,'0')}`
        ctx.fillText(label, cs.barToX(i) - 16, height - cs.pb + 14)
      }
    }
  }, [cs, data, viewStart, width, height])

  // Pan on drag
  const dragRef = useRef<{ x: number } | null>(null)
  const onMouseDown = useCallback((e: React.MouseEvent) => {
    dragRef.current = { x: e.clientX }
  }, [])
  const onMouseMove = useCallback((e: React.MouseEvent) => {
    const rect = canvasRef.current?.getBoundingClientRect()
    if (rect && cs && data) {
      crosshairRef.current?.update(e.clientX - rect.left, e.clientY - rect.top)
    }
    if (!dragRef.current) return
    pan(e.clientX - dragRef.current.x)
    dragRef.current = { x: e.clientX }
  }, [pan, cs, data])
  const onMouseLeave = useCallback(() => {
    dragRef.current = null
    crosshairRef.current?.clear()
  }, [])
  const onWheel = useCallback((e: React.WheelEvent) => {
    e.preventDefault()
    zoom(e.deltaY > 0 ? 1.1 : 0.9)
  }, [zoom])

  return (
    <div style={{ position: 'relative', width, height, background: '#0d0d0d' }}>
      <canvas
        ref={canvasRef} width={width} height={height}
        style={{ display: 'block' }}
        onMouseDown={onMouseDown} onMouseMove={onMouseMove}
        onMouseUp={() => { dragRef.current = null }}
        onMouseLeave={onMouseLeave}
        onWheel={onWheel}
      />
      <div style={{
        position: 'absolute', top: 4, left: 8,
        color: '#666', fontSize: 11, fontFamily: 'monospace', pointerEvents: 'none',
      }}>
        {symbol} · {timeframe}
        {data && viewStart + viewCount - 1 < data.length && (() => {
          const last = viewStart + viewCount - 1
          return ` · O ${data.opens[last]?.toFixed(2)} H ${data.highs[last]?.toFixed(2)} L ${data.lows[last]?.toFixed(2)} C ${data.closes[last]?.toFixed(2)}`
        })()}
      </div>
      {cs && data && (
        <CrosshairOverlay ref={crosshairRef} cs={cs} data={data}
          viewStart={viewStart} width={width} height={height} />
      )}
      <canvas ref={axisRef} width={width} height={height}
        style={{ position: 'absolute', top: 0, left: 0, pointerEvents: 'none' }} />
      {cs && data && (
        <DrawingOverlay symbol={symbol} timeframe={timeframe} cs={cs}
          width={width} height={height} viewStart={viewStart} />
      )}
    </div>
  )
}
