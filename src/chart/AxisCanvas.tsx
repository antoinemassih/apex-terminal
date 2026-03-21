import { useRef, useEffect } from 'react'
import type { CoordSystem } from '../engine'
import type { ColumnStore } from '../data/columns'

interface Props {
  cs: CoordSystem | null
  data: ColumnStore | null
  viewStart: number
  width: number
  height: number
}

export function AxisCanvas({ cs, data, viewStart, width, height }: Props) {
  const ref = useRef<HTMLCanvasElement>(null)

  useEffect(() => {
    if (!ref.current || !cs || !data) return
    const canvas = ref.current
    const dpr = window.devicePixelRatio || 1
    canvas.width = width * dpr
    canvas.height = height * dpr
    canvas.style.width = width + 'px'
    canvas.style.height = height + 'px'
    const ctx = canvas.getContext('2d')!
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0)
    ctx.clearRect(0, 0, width, height)
    ctx.fillStyle = '#444'
    ctx.font = '10px monospace'
    ctx.textAlign = 'left'

    const priceStep = (cs.maxPrice - cs.minPrice) / 8
    for (let i = 0; i <= 8; i++) {
      const price = cs.minPrice + i * priceStep
      ctx.fillText(price.toFixed(2), width - cs.pr + 4, cs.priceToY(price) + 4)
    }

    const barStep = Math.max(1, Math.floor(100 / cs.barStep))
    for (let i = 0; i < cs.barCount; i += barStep) {
      const dataIdx = viewStart + i
      if (dataIdx < data.length) {
        const d = new Date(data.times[dataIdx] * 1000)
        const label = `${d.getHours().toString().padStart(2, '0')}:${d.getMinutes().toString().padStart(2, '0')}`
        ctx.fillText(label, cs.barToX(i) - 16, height - cs.pb + 14)
      }
    }
  }, [cs, data, viewStart, width, height])

  return <canvas ref={ref} width={width} height={height}
    style={{ position: 'absolute', top: 0, left: 0, pointerEvents: 'none' }} />
}
