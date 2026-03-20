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

    useImperativeHandle(ref, () => ({
      update(mouseX: number, mouseY: number) {
        const canvas = canvasRef.current
        if (!canvas) return
        const ctx = canvas.getContext('2d')!
        ctx.clearRect(0, 0, width, height)

        const price = cs.yToPrice(mouseY)
        const barIdx = Math.round(cs.xToBar(mouseX))

        ctx.strokeStyle = 'rgba(255,255,255,0.25)'
        ctx.setLineDash([4, 4])
        ctx.lineWidth = 1

        ctx.beginPath()
        ctx.moveTo(0, mouseY)
        ctx.lineTo(width - cs.pr, mouseY)
        ctx.stroke()

        ctx.beginPath()
        ctx.moveTo(mouseX, cs.pt)
        ctx.lineTo(mouseX, height - cs.pb)
        ctx.stroke()
        ctx.setLineDash([])

        // Price label on right axis
        ctx.fillStyle = '#1a1a2e'
        ctx.fillRect(width - cs.pr, mouseY - 10, cs.pr, 20)
        ctx.fillStyle = '#ccc'
        ctx.font = '11px monospace'
        ctx.textAlign = 'left'
        ctx.fillText(price.toFixed(2), width - cs.pr + 4, mouseY + 4)

        // Time label on bottom axis
        const dataIdx = viewStart + barIdx
        if (dataIdx >= 0 && dataIdx < data.length) {
          const time = data.times[dataIdx]
          const d = new Date(time * 1000)
          const label = `${d.getMonth()+1}/${d.getDate()} ${d.getHours().toString().padStart(2,'0')}:${d.getMinutes().toString().padStart(2,'0')}`
          const tw = ctx.measureText(label).width
          ctx.fillStyle = '#1a1a2e'
          ctx.fillRect(mouseX - tw/2 - 4, height - cs.pb, tw + 8, 18)
          ctx.fillStyle = '#ccc'
          ctx.textAlign = 'center'
          ctx.fillText(label, mouseX, height - cs.pb + 13)
        }
      },
      clear() {
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
