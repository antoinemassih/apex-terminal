import { useRef, useEffect } from 'react'
import type { CoordSystem } from '../engine'
import type { ColumnStore } from '../data/columns'
import { useChartStore } from '../store/chartStore'
import { getTheme } from '../themes'

interface Props {
  cs: CoordSystem | null
  data: ColumnStore | null
  viewStart: number
  width: number
  height: number
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

export function AxisCanvas({ cs, data, viewStart, width, height }: Props) {
  const ref = useRef<HTMLCanvasElement>(null)
  const themeName = useChartStore(s => s.theme)
  const theme = getTheme(themeName)

  useEffect(() => {
    if (!ref.current || !cs || !data || data.length === 0) return
    const canvas = ref.current
    const dpr = window.devicePixelRatio || 1
    canvas.width = width * dpr
    canvas.height = height * dpr
    canvas.style.width = width + 'px'
    canvas.style.height = height + 'px'
    const ctx = canvas.getContext('2d')!
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0)
    ctx.clearRect(0, 0, width, height)
    ctx.fillStyle = theme.axisText
    ctx.font = '10px monospace'
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
      ctx.fillText(price.toFixed(priceStep < 1 ? 2 : priceStep < 10 ? 1 : 0), width - cs.pr + 4, y + 4)
    }

    // Time axis (bottom) — anchored to round time boundaries
    // Estimate candle duration from data
    let candleSec = 300 // default 5m
    if (data.length >= 2) {
      candleSec = Math.max(1, data.times[1] - data.times[0])
    }

    const interval = pickTimeInterval(cs.barStep, candleSec)

    // Find the first visible time snapped to the interval
    const firstVisibleTime = data.times[viewStart] ?? 0
    const firstLabel = Math.ceil(firstVisibleTime / interval) * interval

    // Walk through round time boundaries and find their bar positions
    const lastVisibleTime = data.times[Math.min(viewStart + cs.barCount, data.length) - 1] ?? firstVisibleTime
    ctx.textAlign = 'center'
    for (let t = firstLabel; t <= lastVisibleTime + interval; t += interval) {
      const barIdx = data.indexAtTime(t)
      const viewIdx = barIdx - viewStart
      if (viewIdx < 0) continue
      if (viewIdx >= cs.barCount) break

      const x = cs.barToX(viewIdx)
      if (x < 0 || x > width - cs.pr) continue

      ctx.fillText(formatTime(t, interval), x, height - cs.pb + 14)
    }
  }, [cs, data, viewStart, width, height, theme])

  return <canvas ref={ref} width={width} height={height}
    style={{ position: 'absolute', top: 0, left: 0, pointerEvents: 'none' }} />
}
