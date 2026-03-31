import { useRef, forwardRef, useImperativeHandle } from 'react'
import type { CoordSystem } from '../engine'
import type { ColumnStore } from '../data/columns'
import { useChartStore } from '../store/chartStore'
import { getTheme } from '../themes'

interface Props {
  width: number
  height: number
}

export interface AxisCanvasHandle {
  draw(cs: CoordSystem, data: ColumnStore, viewStart: number): void
}

/** Pick a nice round time interval (in seconds) so labels don't crowd */
function pickTimeInterval(barStepPx: number, candleSeconds: number): number {
  // We want roughly 1 label per 100-150 pixels
  const barsPerLabel = Math.max(1, Math.round(120 / barStepPx))
  const rawSeconds = barsPerLabel * candleSeconds

  // Snap to round intervals
  const nice = [
    60, 120, 300, 600, 900, 1800, 3600,       // 1m, 2m, 5m, 10m, 15m, 30m, 1h
    7200, 14400, 28800, 43200, 86400,          // 2h, 4h, 8h, 12h, 1d
    172800, 604800, 2592000,                    // 2d, 1w, 30d
  ]
  for (const n of nice) {
    if (n >= rawSeconds) return n
  }
  return nice[nice.length - 1]
}

function formatTime(ts: number, intervalSec: number): string {
  const d = new Date(ts * 1000)
  if (intervalSec >= 86400) {
    return `${(d.getMonth() + 1).toString().padStart(2, '0')}/${d.getDate().toString().padStart(2, '0')}`
  }
  return `${d.getHours().toString().padStart(2, '0')}:${d.getMinutes().toString().padStart(2, '0')}`
}

export const AxisCanvas = forwardRef<AxisCanvasHandle, Props>(
  function AxisCanvas({ width, height }, ref) {
    const canvasRef = useRef<HTMLCanvasElement>(null)
    const themeName = useChartStore(s => s.theme)
    // Track last canvas pixel dimensions — only resize when they actually change
    const canvasSizeRef = useRef({ w: 0, h: 0 })

    // ── Time-dependent cache ────────────────────────────────────────────────────
    // candleSec and session break indices only change when which bars are visible
    // changes (viewStart / barCount / dataLen) — not on every price tick.
    const timeKeyRef = useRef('')
    const cachedCandleSecRef = useRef(300)
    /** viewIdx (relative to viewStart) of each session break */
    const cachedBreakViewIdxRef = useRef<number[]>([])

    useImperativeHandle(ref, () => ({
      draw(cs: CoordSystem, data: ColumnStore, viewStart: number) {
        if (!canvasRef.current || data.length === 0) return
        const canvas = canvasRef.current
        const theme = getTheme(themeName)
        const dpr = window.devicePixelRatio || 1
        const pw = Math.round(width * dpr)
        const ph = Math.round(height * dpr)
        // Only resize when dimensions actually changed — resizing clears & is expensive
        if (canvasSizeRef.current.w !== pw || canvasSizeRef.current.h !== ph) {
          canvas.width = pw
          canvas.height = ph
          canvas.style.width = width + 'px'
          canvas.style.height = height + 'px'
          canvasSizeRef.current = { w: pw, h: ph }
        }
        const ctx = canvas.getContext('2d')!
        ctx.setTransform(dpr, 0, 0, dpr, 0, 0)
        ctx.clearRect(0, 0, width, height)
        ctx.fillStyle = theme.axisText
        ctx.font = '8.5px monospace'
        ctx.textAlign = 'left'

        // Price axis (right side) — anchored to round price values
        const priceRange = cs.maxPrice - cs.minPrice
        const rawPriceStep = priceRange / 8
        // Snap to nice round numbers
        const mag = Math.pow(10, Math.floor(Math.log10(rawPriceStep)))
        const niceSteps = [1, 2, 2.5, 5, 10]
        let priceStep = mag
        for (const n of niceSteps) {
          if (n * mag >= rawPriceStep) { priceStep = n * mag; break }
        }

        const firstPrice = Math.ceil(cs.minPrice / priceStep) * priceStep
        for (let price = firstPrice; price <= cs.maxPrice; price += priceStep) {
          const y = cs.priceToY(price)
          if (y < cs.pt || y > height - cs.pb) continue
          ctx.fillText(price.toFixed(priceStep < 1 ? 2 : priceStep < 10 ? 1 : 0), width - cs.pr + 3, y + 3)
        }

        // ── Time-dependent data — recompute only when visible bar set changes ──
        const visEnd = Math.min(viewStart + cs.barCount, data.length)
        const timeKey = `${viewStart}|${cs.barCount}|${data.length}`
        if (timeKey !== timeKeyRef.current) {
          timeKeyRef.current = timeKey

          // Sample up to 20 gaps to estimate candleSec — O(20) vs O(n log n) sort
          const sampleEnd = Math.min(viewStart + 21, visEnd)
          const sampleGaps: number[] = []
          for (let i = viewStart + 1; i < sampleEnd && i < data.length; i++) {
            sampleGaps.push(data.times[i] - data.times[i - 1])
          }
          sampleGaps.sort((a, b) => a - b)
          cachedCandleSecRef.current = sampleGaps.length > 0
            ? Math.max(1, sampleGaps[Math.floor(sampleGaps.length / 2)])
            : 300

          // Find session break bar indices (relative to viewStart)
          const threshold = cachedCandleSecRef.current * 2.5
          const breaks: number[] = []
          for (let i = viewStart + 1; i < visEnd && i < data.length; i++) {
            if (data.times[i] - data.times[i - 1] > threshold) {
              breaks.push(i - viewStart)
            }
          }
          cachedBreakViewIdxRef.current = breaks
        }

        const candleSec = cachedCandleSecRef.current

        // Session break lines — from cached indices, x positions computed from current cs
        ctx.strokeStyle = theme.axisText + '40'
        ctx.lineWidth = 1
        ctx.setLineDash([3, 3])
        for (const viewIdx of cachedBreakViewIdxRef.current) {
          const x = cs.barToX(viewIdx) - cs.barStep * 0.5
          if (x > 0 && x < width - cs.pr) {
            ctx.beginPath()
            ctx.moveTo(x, cs.pt)
            ctx.lineTo(x, height - cs.pb)
            ctx.stroke()
          }
        }
        ctx.setLineDash([])

        // Time axis (bottom) — anchored to round time boundaries
        const interval = pickTimeInterval(cs.barStep, candleSec)
        const firstVisibleTime = data.times[viewStart] ?? 0
        const firstLabel = Math.ceil(firstVisibleTime / interval) * interval

        const lastVisibleTime = data.times[Math.min(viewStart + cs.barCount, data.length) - 1] ?? firstVisibleTime
        ctx.textAlign = 'center'
        ctx.font = '8px monospace'
        // Semi-transparent overlay on volume area
        ctx.fillStyle = theme.axisText + '99' // ~60% opacity via hex alpha
        for (let t = firstLabel; t <= lastVisibleTime + interval; t += interval) {
          const barIdx = data.indexAtTime(t)
          const viewIdx = barIdx - viewStart
          if (viewIdx < 0) continue
          if (viewIdx >= cs.barCount) break

          const x = cs.barToX(viewIdx)
          if (x < 0 || x > width - cs.pr) continue

          ctx.fillText(formatTime(t, interval), x, height - 10)
        }
      }
    }), [width, height, themeName])

    return <canvas ref={canvasRef} width={width} height={height}
      style={{ position: 'absolute', top: 0, left: 0, pointerEvents: 'none' }} />
  }
)
