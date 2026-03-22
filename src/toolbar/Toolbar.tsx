import { useState } from 'react'
import { useChartStore, type Layout } from '../store/chartStore'
import { useDrawingStore } from '../store/drawingStore'
import { INDICATOR_CATALOG } from '../indicators'
import { THEMES, getTheme } from '../themes'
import { WebviewWindow } from '@tauri-apps/api/webviewWindow'
import type { Timeframe } from '../types'

const TIMEFRAMES: Timeframe[] = ['1m', '5m', '15m', '30m', '1h', '4h', '1d', '1wk']
const LAYOUTS: { key: Layout; label: string }[] = [
  { key: '1', label: '1' },
  { key: '2', label: '2' },
  { key: '2h', label: '2H' },
  { key: '3', label: '3' },
  { key: '4', label: '4' },
  { key: '6', label: '6' },
  { key: '6h', label: '6H' },
  { key: '9', label: '9' },
]

export function Toolbar() {
  const { panes, activePane, setSymbol, setTimeframe, resetAutoScroll,
    toggleVolume, toggleIndicator, layout, setLayout, theme: themeName, setTheme } = useChartStore()
  const { activeTool, setActiveTool } = useDrawingStore()
  const [symbolInput, setSymbolInput] = useState('')
  const pane = panes.find(p => p.id === activePane)
  const theme = getTheme(themeName)

  const btnStyle = (active: boolean) => ({
    background: active ? theme.borderActive + '33' : theme.toolbarBackground,
    color: active ? theme.borderActive : theme.axisText,
    border: `1px solid ${active ? theme.borderActive + '88' : theme.toolbarBorder}`,
    borderRadius: 3,
    padding: '2px 8px',
    fontSize: 11,
    fontFamily: 'monospace',
    cursor: 'pointer' as const,
  })

  return (
    <div style={{
      display: 'flex', alignItems: 'center', gap: 6,
      height: 36, background: theme.toolbarBackground, borderBottom: `1px solid ${theme.toolbarBorder}`,
      padding: '0 12px', flexShrink: 0, fontFamily: 'monospace', fontSize: 12,
    }}>
      <span style={{ color: theme.borderActive, fontWeight: 'bold' }}>{pane?.symbol ?? '—'}</span>
      <form onSubmit={e => {
        e.preventDefault()
        if (symbolInput.trim() && activePane) {
          setSymbol(activePane, symbolInput.trim().toUpperCase())
          setSymbolInput('')
        }
      }} style={{ display: 'flex' }}>
        <input value={symbolInput} onChange={e => setSymbolInput(e.target.value)}
          placeholder="Symbol..."
          style={{ background: theme.background, color: theme.ohlcLabel, border: `1px solid ${theme.toolbarBorder}`,
            padding: '2px 8px', width: 80, fontSize: 12, fontFamily: 'monospace' }} />
      </form>
      {/* Timeframes */}
      <div style={{ display: 'flex', gap: 2 }}>
        {TIMEFRAMES.map(tf => (
          <button key={tf} onClick={() => activePane && setTimeframe(activePane, tf)}
            style={btnStyle(pane?.timeframe === tf)}>
            {tf}
          </button>
        ))}
      </div>
      {/* Indicator & Volume toggles */}
      <div style={{ display: 'flex', gap: 2, marginLeft: 4, borderLeft: `1px solid ${theme.toolbarBorder}`, paddingLeft: 6 }}>
        {Object.entries(INDICATOR_CATALOG).map(([id, { name }]) => (
          <button key={id}
            onClick={() => activePane && toggleIndicator(activePane, id)}
            style={btnStyle(pane?.visibleIndicators?.includes(id) ?? false)}>
            {name}
          </button>
        ))}
        <button onClick={() => activePane && toggleVolume(activePane)}
          style={btnStyle(pane?.showVolume ?? false)}>
          VOL
        </button>
      </div>
      {/* Layout */}
      <div style={{ display: 'flex', gap: 2, marginLeft: 4, borderLeft: `1px solid ${theme.toolbarBorder}`, paddingLeft: 6 }}>
        {LAYOUTS.map(l => (
          <button key={l.key} onClick={() => setLayout(l.key)}
            style={btnStyle(layout === l.key)}>
            {l.label}
          </button>
        ))}
      </div>
      {/* Theme picker */}
      <div style={{ display: 'flex', gap: 2, marginLeft: 4, borderLeft: `1px solid ${theme.toolbarBorder}`, paddingLeft: 6 }}>
        <select value={themeName} onChange={e => setTheme(e.target.value)}
          style={{
            background: theme.background, color: theme.ohlcLabel,
            border: `1px solid ${theme.toolbarBorder}`,
            padding: '2px 4px', fontSize: 11, fontFamily: 'monospace', cursor: 'pointer',
          }}>
          {Object.entries(THEMES).map(([key, t]) => (
            <option key={key} value={key}>{t.name}</option>
          ))}
        </select>
      </div>
      {/* Drawing tools */}
      <div style={{ display: 'flex', gap: 2, marginLeft: 4, borderLeft: `1px solid ${theme.toolbarBorder}`, paddingLeft: 6 }}>
        {(['cursor', 'trendline', 'hline', 'hzone', 'barmarker'] as const).map(tool => (
          <button key={tool} onClick={() => setActiveTool(tool)}
            style={btnStyle(activeTool === tool)}>
            {tool}
          </button>
        ))}
      </div>
      <div style={{ marginLeft: 'auto', color: theme.axisText, fontSize: 10, letterSpacing: 2, display: 'flex', alignItems: 'center', gap: 8 }}>
        <button onClick={() => resetAutoScroll()}
          style={{ ...btnStyle(false), background: theme.background, color: theme.bull, border: `1px solid ${theme.bull}44` }}>
          LIVE
        </button>
        <button onClick={() => {
          const label = `chart-${Date.now()}`
          const w = new WebviewWindow(label, { title: 'Apex Terminal', width: 1920, height: 1080, decorations: true })
          w.once('tauri://error', (e) => console.error('Window creation failed:', e))
        }} style={btnStyle(false)}>
          + Window
        </button>
        APEX
      </div>
    </div>
  )
}
