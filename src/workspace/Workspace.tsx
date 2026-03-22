import { useEffect, useRef, useState, useMemo } from 'react'
import { ChartPane } from '../chart/ChartPane'
import { useChartStore } from '../store/chartStore'
import { getTheme } from '../themes'
import type { Layout } from '../store/chartStore'

const LAYOUT_CONFIG: Record<Layout, { cols: number; maxPanes: number }> = {
  '1': { cols: 1, maxPanes: 1 },
  '2': { cols: 2, maxPanes: 2 },
  '4': { cols: 2, maxPanes: 4 },
  '6': { cols: 3, maxPanes: 6 },
  '9': { cols: 3, maxPanes: 9 },
}

export function Workspace() {
  const { panes, activePane, setActivePane, layout, theme: themeName } = useChartStore()
  const theme = getTheme(themeName)
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

  const { cols, maxPanes } = LAYOUT_CONFIG[layout]

  const visiblePanes = useMemo(() => {
    if (panes.length <= maxPanes) return panes

    const activeIndex = panes.findIndex(p => p.id === activePane)
    const startIndex = activeIndex >= 0 ? activeIndex : 0

    const result = []
    for (let i = 0; i < maxPanes; i++) {
      result.push(panes[(startIndex + i) % panes.length])
    }
    return result
  }, [panes, activePane, maxPanes])

  const rows = Math.ceil(visiblePanes.length / cols)
  const paneW = dims.w > 0 ? Math.floor(dims.w / cols) : 0
  const paneH = dims.h > 0 ? Math.floor(dims.h / rows) : 0

  return (
    <div ref={containerRef} style={{
      display: 'grid',
      gridTemplateColumns: `repeat(${cols}, 1fr)`,
      width: '100%', height: '100%', background: theme.background, gap: 1,
    }}>
      {visiblePanes.map((pane) => {
        const originalIndex = panes.findIndex(p => p.id === pane.id)
        return (
          <div key={pane.id}
            onClick={() => setActivePane(pane.id)}
            style={{
              border: `1px solid ${activePane === pane.id ? theme.borderActive : theme.borderInactive}`,
              overflow: 'hidden',
            }}>
            {paneW > 0 && paneH > 0 && (
              <ChartPane paneIndex={originalIndex} symbol={pane.symbol} timeframe={pane.timeframe}
                width={paneW - 2} height={paneH - 2} />
            )}
          </div>
        )
      })}
    </div>
  )
}
