import { useRef, forwardRef, useImperativeHandle } from 'react'
import { CoordSystem } from './CoordSystem'
import { ColumnStore } from '../data/columns'

export interface CrosshairHandle {
  update: (mouseX: number, mouseY: number) => void
  clear: () => void
}

interface Props {
  cs: CoordSystem
  data: ColumnStore
  viewStart: number
  width: number
  height: number
}

export const CrosshairOverlay = forwardRef<CrosshairHandle, Props>(
  function CrosshairOverlay({ cs, data, viewStart, width, height }, ref) {
    const canvasRef = useRef<HTMLCanvasElement>(null)

    // ── RAF throttle ──────────────────────────────────────────────────────────
    // mousemove fires at display rate (120-240Hz on high-refresh monitors).
    // We save the latest position and draw once per animation frame.
    const pendingRef = useRef<{ x: number; y: number } | null>(null)
    const rafRef = useRef(0)

    useImperativeHandle(ref, () => ({
      update(mouseX: number, mouseY: number) {
        pendingRef.current = { x: mouseX, y: mouseY }
        if (rafRef.current) return  // already scheduled for this frame
        rafRef.current = requestAnimationFrame(() => {
          rafRef.current = 0
          const pos = pendingRef.current
          const canvas = canvasRef.current
          if (!pos || !canvas) return

          const ctx = canvas.getContext('2d')!
          ctx.clearRect(0, 0, width, height)

          const { x: mx, y: my } = pos
          const price = cs.yToPrice(my)
          const barIdx = Math.round(cs.xToBar(mx))

          ctx.strokeStyle = 'rgba(255,255,255,0.25)'
          ctx.setLineDash([4, 4])
          ctx.lineWidth = 1

          ctx.beginPath()
          ctx.moveTo(0, my)
          ctx.lineTo(width - cs.pr, my)
          ctx.stroke()

          ctx.beginPath()
          ctx.moveTo(mx, cs.pt)
          ctx.lineTo(mx, height - cs.pb)
          ctx.stroke()
          ctx.setLineDash([])

          // Price label on right axis
          ctx.fillStyle = '#1a1a2e'
          ctx.fillRect(width - cs.pr, my - 10, cs.pr, 20)
          ctx.fillStyle = '#ccc'
          ctx.font = '11px monospace'
          ctx.textAlign = 'left'
          ctx.fillText(price.toFixed(2), width - cs.pr + 4, my + 4)

          // Time label on bottom axis
          const dataIdx = viewStart + barIdx
          if (dataIdx >= 0 && dataIdx < data.length) {
            const time = data.times[dataIdx]
            const d = new Date(time * 1000)
            const label = `${d.getMonth()+1}/${d.getDate()} ${d.getHours().toString().padStart(2,'0')}:${d.getMinutes().toString().padStart(2,'0')}`
            const tw = ctx.measureText(label).width
            ctx.fillStyle = '#1a1a2e'
            ctx.fillRect(mx - tw/2 - 4, height - cs.pb, tw + 8, 18)
            ctx.fillStyle = '#ccc'
            ctx.textAlign = 'center'
            ctx.fillText(label, mx, height - cs.pb + 13)
          }
        })
      },
      clear() {
        pendingRef.current = null
        if (rafRef.current) { cancelAnimationFrame(rafRef.current); rafRef.current = 0 }
        canvasRef.current?.getContext('2d')?.clearRect(0, 0, width, height)
      },
    }))

    return (
      <canvas
        ref={canvasRef} width={width} height={height}
        style={{ position: 'absolute', top: 0, left: 0, pointerEvents: 'none' }}
      />
    )
  }
)
