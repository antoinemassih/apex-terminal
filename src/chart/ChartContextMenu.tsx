import { useEffect } from 'react'
import { useDrawingStore } from '../store/drawingStore'
import { useChartStore } from '../store/chartStore'
import { getTheme } from '../themes'

interface Props {
  x: number
  y: number
  symbol: string
  paneId: string
  onReset: () => void
  onDragZoom: () => void
  onClose: () => void
}

export function ChartContextMenu({ x, y, symbol, paneId, onReset, onDragZoom, onClose }: Props) {
  const themeName = useChartStore(s => s.theme)
  const t = getTheme(themeName)
  const toggleAllIndicators = useChartStore(s => s.toggleAllIndicators)
  const paneConfig = useChartStore(s => s.panes.find(p => p.id === paneId))
  const indicatorsVisible = (paneConfig?.visibleIndicators.length ?? 0) > 0

  const toggleHideDrawings = useDrawingStore(s => s.toggleHideDrawings)
  const drawingsHidden = useDrawingStore(s => s.drawingsHidden)
  const removeAllForSymbol = useDrawingStore(s => s.removeAllForSymbol)
  const hidden = drawingsHidden(symbol)

  useEffect(() => {
    const handler = () => onClose()
    document.addEventListener('mousedown', handler)
    return () => document.removeEventListener('mousedown', handler)
  }, [onClose])

  useEffect(() => {
    const handler = (e: KeyboardEvent) => { if (e.key === 'Escape') onClose() }
    document.addEventListener('keydown', handler)
    return () => document.removeEventListener('keydown', handler)
  }, [onClose])

  const menuBg = t.toolbarBackground
  const menuBorder = t.toolbarBorder
  const textColor = t.ohlcLabel
  const dimColor = t.axisText
  const accentColor = t.borderActive
  const dangerColor = '#e05560'

  const sep: React.CSSProperties = {
    height: 1, background: menuBorder, margin: '3px 0',
  }

  const item = (color = textColor): React.CSSProperties => ({
    display: 'flex', alignItems: 'center', gap: 8,
    width: '100%', padding: '6px 14px',
    background: 'none', border: 'none', cursor: 'pointer',
    color, fontFamily: 'monospace', fontSize: 12,
    textAlign: 'left', whiteSpace: 'nowrap', boxSizing: 'border-box',
  })

  const onHover = (e: React.MouseEvent<HTMLButtonElement>) => {
    e.currentTarget.style.background = accentColor + '22'
    e.currentTarget.style.color = '#fff'
  }
  const onLeave = (e: React.MouseEvent<HTMLButtonElement>, color = textColor) => {
    e.currentTarget.style.background = 'none'
    e.currentTarget.style.color = color
  }

  return (
    <div
      style={{
        position: 'fixed', left: x, top: y, zIndex: 2000,
        background: menuBg,
        border: `1px solid ${menuBorder}`,
        borderRadius: 6, padding: '4px 0',
        boxShadow: '0 8px 24px rgba(0,0,0,0.6)',
        minWidth: 190,
        fontFamily: 'monospace',
      }}
      onMouseDown={e => e.stopPropagation()}
    >
      <button style={item(dimColor)}
        onMouseEnter={onHover} onMouseLeave={e => onLeave(e, dimColor)}
        onClick={() => { onReset(); onClose() }}>
        ↺ Reset Chart
      </button>
      <button style={item(dimColor)}
        onMouseEnter={onHover} onMouseLeave={e => onLeave(e, dimColor)}
        onClick={() => { onDragZoom(); onClose() }}>
        ⊡ Drag Zoom
      </button>

      <div style={sep} />

      <button style={item(textColor)}
        onMouseEnter={onHover} onMouseLeave={e => onLeave(e, textColor)}
        onClick={() => { toggleHideDrawings(symbol); onClose() }}>
        {hidden ? '◉ Show All Drawings' : '◎ Hide All Drawings'}
      </button>
      <button style={item(textColor)}
        onMouseEnter={onHover} onMouseLeave={e => onLeave(e, textColor)}
        onClick={() => { toggleAllIndicators(paneId); onClose() }}>
        {indicatorsVisible ? '◎ Hide All Indicators' : '◉ Show All Indicators'}
      </button>

      <div style={sep} />

      <button style={item(dangerColor)}
        onMouseEnter={e => { e.currentTarget.style.background = dangerColor + '22'; e.currentTarget.style.color = dangerColor }}
        onMouseLeave={e => { e.currentTarget.style.background = 'none'; e.currentTarget.style.color = dangerColor }}
        onClick={() => { removeAllForSymbol(symbol); onClose() }}>
        ✕ Delete All Drawings
      </button>
    </div>
  )
}
