import { useEffect } from 'react'

interface Props {
  x: number
  y: number
  onReset: () => void
  onDragZoom: () => void
  onClose: () => void
}

const itemStyle: React.CSSProperties = {
  display: 'block', width: '100%', padding: '6px 14px',
  background: 'none', border: 'none', cursor: 'pointer',
  color: '#ccc', fontFamily: 'monospace', fontSize: 12,
  textAlign: 'left', whiteSpace: 'nowrap',
}

export function ChartContextMenu({ x, y, onReset, onDragZoom, onClose }: Props) {
  // Close on any mousedown outside the menu
  useEffect(() => {
    const handler = () => onClose()
    document.addEventListener('mousedown', handler)
    return () => document.removeEventListener('mousedown', handler)
  }, [onClose])

  // Close on Escape
  useEffect(() => {
    const handler = (e: KeyboardEvent) => { if (e.key === 'Escape') onClose() }
    document.addEventListener('keydown', handler)
    return () => document.removeEventListener('keydown', handler)
  }, [onClose])

  return (
    <div
      style={{
        position: 'fixed', left: x, top: y, zIndex: 1000,
        background: '#1e1e2e', border: '1px solid #383860',
        borderRadius: 5, padding: '3px 0',
        boxShadow: '0 6px 18px rgba(0,0,0,0.55)',
        minWidth: 150,
      }}
      onMouseDown={e => e.stopPropagation()}  // don't trigger the close handler above
    >
      <button
        style={itemStyle}
        onMouseEnter={e => (e.currentTarget.style.background = '#2a2a45')}
        onMouseLeave={e => (e.currentTarget.style.background = 'none')}
        onClick={() => { onReset(); onClose() }}
      >
        Reset Chart
      </button>
      <button
        style={itemStyle}
        onMouseEnter={e => (e.currentTarget.style.background = '#2a2a45')}
        onMouseLeave={e => (e.currentTarget.style.background = 'none')}
        onClick={() => { onDragZoom(); onClose() }}
      >
        Drag Zoom
      </button>
    </div>
  )
}
