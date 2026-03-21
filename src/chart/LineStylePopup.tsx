import { useEffect, useRef } from 'react'
import { useDrawingStore } from '../store/drawingStore'

interface Props {
  drawingId: string
  position: { x: number; y: number }
  onClose: () => void
}

const COLORS = [
  '#4a9eff', '#e74c3c', '#2ecc71', '#f39c12',
  '#9b59b6', '#1abc9c', '#ffffff', '#e67e22',
]
const OPACITIES = [1, 0.75, 0.5, 0.25] as const
const LINE_STYLES = ['solid', 'dashed', 'dotted'] as const
const THICKNESSES = [0.5, 1, 1.5, 2.5] as const

const styleLabel: Record<string, string> = {
  solid: '\u2500\u2500',
  dashed: '\u2504\u2504',
  dotted: '\u00B7\u00B7\u00B7',
}

const thicknessLabel = (t: number) =>
  t <= 0.5 ? '\u2504' : t <= 1 ? '\u2500' : t <= 1.5 ? '\u2501' : '\u2588'

export function LineStylePopup({ drawingId, position, onClose }: Props) {
  const ref = useRef<HTMLDivElement>(null)
  const drawing = useDrawingStore(s => s.drawings.find(d => d.id === drawingId))
  const updateStyle = useDrawingStore(s => s.updateDrawingStyle)
  const removeDrawing = useDrawingStore(s => s.removeDrawing)

  // Close on click outside or Escape
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose()
    }
    const onClick = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) {
        onClose()
      }
    }
    window.addEventListener('keydown', onKey)
    // Use timeout to avoid the same click that opened the popup from closing it
    const timer = setTimeout(() => window.addEventListener('mousedown', onClick), 0)
    return () => {
      window.removeEventListener('keydown', onKey)
      window.removeEventListener('mousedown', onClick)
      clearTimeout(timer)
    }
  }, [onClose])

  if (!drawing) return null

  const color = drawing.color
  const opacity = drawing.opacity ?? 1
  const lineStyle = drawing.lineStyle ?? 'solid'
  const thickness = drawing.thickness ?? 1.5

  const sectionStyle: React.CSSProperties = {
    display: 'flex', gap: 4, alignItems: 'center', marginBottom: 4,
  }

  const btnBase: React.CSSProperties = {
    background: 'transparent', border: '1px solid #444', borderRadius: 3,
    color: '#ccc', fontFamily: 'monospace', fontSize: 11, cursor: 'pointer',
    padding: '2px 6px', lineHeight: '16px',
  }

  const activeBtn: React.CSSProperties = {
    ...btnBase,
    borderColor: '#4a9eff', background: 'rgba(74,158,255,0.15)', color: '#fff',
  }

  return (
    <div ref={ref} style={{
      position: 'absolute',
      left: position.x,
      top: position.y,
      background: '#1a1a1e',
      border: '1px solid #333',
      borderRadius: 6,
      padding: 8,
      zIndex: 100,
      boxShadow: '0 4px 12px rgba(0,0,0,0.5)',
      fontFamily: 'monospace',
      fontSize: 11,
      minWidth: 200,
      userSelect: 'none',
    }}
    onMouseDown={e => e.stopPropagation()}
    >
      {/* Colors */}
      <div style={sectionStyle}>
        {COLORS.map(c => (
          <div key={c} onClick={() => updateStyle(drawingId, { color: c })} style={{
            width: 16, height: 16, borderRadius: '50%', background: c,
            border: c === color ? '2px solid #fff' : '2px solid transparent',
            cursor: 'pointer', boxSizing: 'border-box',
          }} />
        ))}
      </div>

      {/* Opacity */}
      <div style={sectionStyle}>
        {OPACITIES.map(o => (
          <button key={o} onClick={() => updateStyle(drawingId, { opacity: o })}
            style={o === opacity ? activeBtn : btnBase}>
            {Math.round(o * 100)}%
          </button>
        ))}
      </div>

      {/* Line style */}
      <div style={sectionStyle}>
        {LINE_STYLES.map(s => (
          <button key={s} onClick={() => updateStyle(drawingId, { lineStyle: s })}
            style={s === lineStyle ? activeBtn : btnBase}>
            {styleLabel[s]}
          </button>
        ))}
      </div>

      {/* Thickness */}
      <div style={sectionStyle}>
        {THICKNESSES.map(t => (
          <button key={t} onClick={() => updateStyle(drawingId, { thickness: t })}
            style={t === thickness ? activeBtn : btnBase}>
            {thicknessLabel(t)}
          </button>
        ))}
      </div>

      {/* Delete */}
      <div style={{ marginTop: 4 }}>
        <button onClick={() => { removeDrawing(drawingId); onClose() }}
          style={{ ...btnBase, color: '#e74c3c', borderColor: '#e74c3c', width: '100%' }}>
          Delete
        </button>
      </div>
    </div>
  )
}
