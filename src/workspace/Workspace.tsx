import { useEffect, useRef, useState, useMemo } from 'react'
import { ChartPane } from '../chart/ChartPane'
import { useChartStore } from '../store/chartStore'
import { getTheme } from '../themes'
import type { Layout } from '../store/chartStore'

interface LayoutConfig {
  maxPanes: number
  // For simple grid layouts
  cols?: number
  rows?: number
  // For custom layouts — returns CSS grid properties + per-pane grid area
  custom?: boolean
}

const LAYOUT_CONFIG: Record<Layout, LayoutConfig> = {
  '1':  { maxPanes: 1, cols: 1 },
  '2':  { maxPanes: 2, cols: 2 },         // side by side
  '2h': { maxPanes: 2, cols: 1 },         // stacked vertically
  '3':  { maxPanes: 3, custom: true },     // 1 big top + 2 small bottom
  '4':  { maxPanes: 4, cols: 2 },
  '6':  { maxPanes: 6, cols: 3 },         // 3 cols x 2 rows
  '6h': { maxPanes: 6, cols: 2 },         // 2 cols x 3 rows (taller)
  '9':  { maxPanes: 9, cols: 3 },
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

  const config = LAYOUT_CONFIG[layout]

  const visiblePanes = useMemo(() => {
    if (panes.length <= config.maxPanes) return panes
    const activeIndex = panes.findIndex(p => p.id === activePane)
    const startIndex = activeIndex >= 0 ? activeIndex : 0
    const result = []
    for (let i = 0; i < config.maxPanes; i++) {
      result.push(panes[(startIndex + i) % panes.length])
    }
    return result
  }, [panes, activePane, config.maxPanes])

  // Layout "3": 1 big pane on top (full width), 2 smaller on bottom
  if (config.custom && layout === '3') {
    const topH = dims.h > 0 ? Math.floor(dims.h * 0.6) : 0
    const botH = dims.h > 0 ? dims.h - topH : 0
    const halfW = dims.w > 0 ? Math.floor(dims.w / 2) : 0

    return (
      <div ref={containerRef} style={{
        display: 'flex', flexDirection: 'column',
        width: '100%', height: '100%', background: theme.background, gap: 1,
      }}>
        {/* Top: full-width big chart */}
        {visiblePanes[0] && dims.w > 0 && topH > 0 && (
          <div
            onClick={() => setActivePane(visiblePanes[0].id)}
            style={{
              border: `1px solid ${activePane === visiblePanes[0].id ? theme.borderActive : theme.borderInactive}`,
              overflow: 'hidden', height: topH,
            }}>
            <ChartPane
              paneIndex={panes.findIndex(p => p.id === visiblePanes[0].id)}
              symbol={visiblePanes[0].symbol} timeframe={visiblePanes[0].timeframe}
              width={dims.w - 2} height={topH - 2} />
          </div>
        )}
        {/* Bottom: 2 smaller charts */}
        <div style={{ display: 'flex', gap: 1, height: botH }}>
          {visiblePanes.slice(1, 3).map(pane => (
            <div key={pane.id}
              onClick={() => setActivePane(pane.id)}
              style={{
                flex: 1,
                border: `1px solid ${activePane === pane.id ? theme.borderActive : theme.borderInactive}`,
                overflow: 'hidden',
              }}>
              {halfW > 0 && botH > 0 && (
                <ChartPane
                  paneIndex={panes.findIndex(p => p.id === pane.id)}
                  symbol={pane.symbol} timeframe={pane.timeframe}
                  width={halfW - 2} height={botH - 2} />
              )}
            </div>
          ))}
        </div>
      </div>
    )
  }

  // Standard grid layouts
  const cols = config.cols ?? 1
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
