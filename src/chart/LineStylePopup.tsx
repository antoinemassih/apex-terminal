import { useEffect, useRef } from 'react'
import { useDrawingStore } from '../store/drawingStore'
import { useChartStore } from '../store/chartStore'
import { getTheme } from '../themes'

interface Props {
  drawingId: string
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
  solid: '\u2500\u2500\u2500',
  dashed: '\u2504\u2504\u2504',
  dotted: '\u00B7 \u00B7 \u00B7',
}

const thicknessLabel = (t: number) =>
  t <= 0.5 ? '\u2504' : t <= 1 ? '\u2500' : t <= 1.5 ? '\u2501' : '\u2588'

export function LineStylePopup({ drawingId, onClose }: Props) {
  const ref = useRef<HTMLDivElement>(null)
  const drawing = useDrawingStore(s => s.drawings.find(d => d.id === drawingId))
  const updateStyle = useDrawingStore(s => s.updateDrawingStyle)
  const removeDrawing = useDrawingStore(s => s.removeDrawing)
  const themeName = useChartStore(s => s.theme)
  const t = getTheme(themeName)

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => { if (e.key === 'Escape') onClose() }
    const onClick = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) onClose()
    }
    window.addEventListener('keydown', onKey)
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

  const accent = t.borderActive
  const bg = t.toolbarBackground
  const border = t.toolbarBorder
  const text = t.ohlcLabel

  const btnBase: React.CSSProperties = {
    background: 'none',
    border: `1px solid ${border}`,
    borderRadius: 3,
    color: text,
    fontFamily: 'monospace',
    fontSize: 11,
    cursor: 'pointer',
    padding: '2px 6px',
    lineHeight: '16px',
    transition: 'border-color 0.1s, background 0.1s',
  }

  const activeBtn: React.CSSProperties = {
    ...btnBase,
    borderColor: accent,
    background: accent + '26',
    color: '#fff',
  }

  const sep: React.CSSProperties = {
    width: 1,
    height: 18,
    background: border,
    margin: '0 4px',
    flexShrink: 0,
  }

  return (
    <div
      ref={ref}
      style={{
        position: 'absolute',
        top: 6,
        left: '50%',
        transform: 'translateX(-50%)',
        zIndex: 100,
        background: bg,
        border: `1px solid ${border}`,
        borderRadius: 6,
        padding: '5px 10px',
        boxShadow: '0 4px 16px rgba(0,0,0,0.55)',
        display: 'flex',
        alignItems: 'center',
        gap: 4,
        fontFamily: 'monospace',
        fontSize: 11,
        userSelect: 'none',
        whiteSpace: 'nowrap',
      }}
      onMouseDown={e => e.stopPropagation()}
    >
      {/* Colors */}
      {COLORS.map(c => (
        <div
          key={c}
          onClick={() => updateStyle(drawingId, { color: c })}
          title={c}
          style={{
            width: 13,
            height: 13,
            borderRadius: '50%',
            background: c,
            border: c === color ? `2px solid #fff` : `2px solid transparent`,
            cursor: 'pointer',
            boxSizing: 'border-box',
            flexShrink: 0,
            outline: c === color ? `1px solid ${accent}` : 'none',
            outlineOffset: 1,
          }}
        />
      ))}

      <div style={sep} />

      {/* Line styles */}
      {LINE_STYLES.map(s => (
        <button key={s} onClick={() => updateStyle(drawingId, { lineStyle: s })}
          style={s === lineStyle ? activeBtn : btnBase} title={s}>
          {styleLabel[s]}
        </button>
      ))}

      <div style={sep} />

      {/* Thicknesses */}
      {THICKNESSES.map(tk => (
        <button key={tk} onClick={() => updateStyle(drawingId, { thickness: tk })}
          style={tk === thickness ? activeBtn : btnBase} title={`${tk}px`}>
          {thicknessLabel(tk)}
        </button>
      ))}

      <div style={sep} />

      {/* Opacities */}
      {OPACITIES.map(o => (
        <button key={o} onClick={() => updateStyle(drawingId, { opacity: o })}
          style={o === opacity ? activeBtn : btnBase} title={`${Math.round(o * 100)}%`}>
          {Math.round(o * 100)}%
        </button>
      ))}

      <div style={sep} />

      {/* Delete */}
      <button
        onClick={() => { removeDrawing(drawingId); onClose() }}
        style={{ ...btnBase, color: '#e05560', borderColor: '#e05560' }}
        title="Delete drawing"
      >
        ✕
      </button>
    </div>
  )
}
