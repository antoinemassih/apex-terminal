import { useEffect, useRef, useState } from 'react'
import { ChartPane } from '../chart/ChartPane'
import { useChartStore } from '../store/chartStore'

export function Workspace() {
  const { panes, activePane, setActivePane } = useChartStore()
  const containerRef = useRef<HTMLDivElement>(null)
  const [dims, setDims] = useState({ w: 0, h: 0 })

  useEffect(() => {
    const el = containerRef.current
    if (!el) return
    const ro = new ResizeObserver(() => {
      setDims({ w: el.clientWidth, h: el.clientHeight })
    })
    ro.observe(el)
    setDims({ w: el.clientWidth, h: el.clientHeight })
    return () => ro.disconnect()
  }, [])

  const cols = 3
  const rows = Math.ceil(panes.length / cols)
  const paneW = dims.w > 0 ? Math.floor(dims.w / cols) : 0
  const paneH = dims.h > 0 ? Math.floor(dims.h / rows) : 0

  return (
    <div ref={containerRef} style={{
      display: 'grid',
      gridTemplateColumns: `repeat(${cols}, 1fr)`,
      width: '100%', height: '100%', background: '#0a0a0a', gap: 1,
    }}>
      {panes.map((pane, index) => (
        <div key={pane.id}
          onClick={() => setActivePane(pane.id)}
          style={{
            border: `1px solid ${activePane === pane.id ? '#2a6496' : '#1a1a1a'}`,
            overflow: 'hidden',
          }}>
          {paneW > 0 && paneH > 0 && (
            <ChartPane paneIndex={index} symbol={pane.symbol} timeframe={pane.timeframe}
              width={paneW - 2} height={paneH - 2} />
          )}
        </div>
      ))}
    </div>
  )
}
