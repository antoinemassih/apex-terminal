import { useState } from 'react'
import { useChartStore } from '../store/chartStore'
import { useDrawingStore } from '../store/drawingStore'
import { INDICATOR_CATALOG } from '../indicators'
import { WebviewWindow } from '@tauri-apps/api/webviewWindow'
import type { Timeframe } from '../types'

const TIMEFRAMES: Timeframe[] = ['1m', '5m', '15m', '1h', '1d', '1wk']

const toggleStyle = (active: boolean) => ({
  background: active ? '#1a3a5c' : '#1a1a1a',
  color: active ? '#4a9eff' : '#555',
  border: `1px solid ${active ? '#2a5a8c' : '#333'}`,
  borderRadius: 3,
  padding: '2px 8px',
  fontSize: 11,
  fontFamily: 'monospace',
  cursor: 'pointer' as const,
})

export function Toolbar() {
  const { panes, activePane, setSymbol, setTimeframe, resetAutoScroll, toggleVolume, toggleIndicator } = useChartStore()
  const { activeTool, setActiveTool } = useDrawingStore()
  const [symbolInput, setSymbolInput] = useState('')
  const pane = panes.find(p => p.id === activePane)

  return (
    <div style={{
      display: 'flex', alignItems: 'center', gap: 8,
      height: 36, background: '#111', borderBottom: '1px solid #222',
      padding: '0 12px', flexShrink: 0, fontFamily: 'monospace', fontSize: 12,
    }}>
      <span style={{ color: '#4a9eff', fontWeight: 'bold' }}>{pane?.symbol ?? '—'}</span>
      <form onSubmit={e => {
        e.preventDefault()
        if (symbolInput.trim() && activePane) {
          setSymbol(activePane, symbolInput.trim().toUpperCase())
          setSymbolInput('')
        }
      }} style={{ display: 'flex' }}>
        <input value={symbolInput} onChange={e => setSymbolInput(e.target.value)}
          placeholder="Symbol..."
          style={{ background: '#1a1a1a', color: '#ccc', border: '1px solid #333',
            padding: '2px 8px', width: 80, fontSize: 12, fontFamily: 'monospace' }} />
      </form>
      <div style={{ display: 'flex', gap: 2 }}>
        {TIMEFRAMES.map(tf => (
          <button key={tf} onClick={() => activePane && setTimeframe(activePane, tf)}
            style={{ background: pane?.timeframe === tf ? '#2a6496' : '#1a1a1a',
              color: '#ccc', border: '1px solid #333',
              padding: '2px 8px', cursor: 'pointer', fontSize: 11, fontFamily: 'monospace' }}>
            {tf}
          </button>
        ))}
      </div>
      {/* Indicator & Volume toggles */}
      <div style={{ display: 'flex', gap: 2, marginLeft: 8, borderLeft: '1px solid #333', paddingLeft: 8 }}>
        {Object.entries(INDICATOR_CATALOG).map(([id, { name }]) => {
          const active = pane?.visibleIndicators?.includes(id) ?? false
          return (
            <button key={id}
              onClick={() => activePane && toggleIndicator(activePane, id)}
              style={toggleStyle(active)}>
              {name}
            </button>
          )
        })}
        <button
          onClick={() => activePane && toggleVolume(activePane)}
          style={toggleStyle(pane?.showVolume ?? false)}>
          VOL
        </button>
      </div>
      {/* Drawing tools */}
      <div style={{ display: 'flex', gap: 2, marginLeft: 8, borderLeft: '1px solid #333', paddingLeft: 8 }}>
        {(['cursor', 'trendline', 'hline'] as const).map(tool => (
          <button key={tool}
            onClick={() => setActiveTool(tool)}
            style={{
              background: activeTool === tool ? '#2a6496' : '#1a1a1a',
              color: '#ccc', border: '1px solid #333',
              padding: '2px 8px', cursor: 'pointer', fontSize: 11, fontFamily: 'monospace',
            }}
          >{tool}</button>
        ))}
      </div>
      <div style={{ marginLeft: 'auto', color: '#333', fontSize: 10, letterSpacing: 2, display: 'flex', alignItems: 'center', gap: 8 }}>
        <button
          onClick={() => resetAutoScroll()}
          style={{
            background: '#1a3a1a', color: '#4caf50', border: '1px solid #2e7d32',
            padding: '2px 8px', cursor: 'pointer', fontSize: 11, fontFamily: 'monospace',
          }}
        >LIVE</button>
        <button
          onClick={async () => {
            const label = `chart-${Date.now()}`
            new WebviewWindow(label, {
              title: 'Apex Terminal',
              width: 1920,
              height: 1080,
              decorations: true,
            })
          }}
          style={{
            background: '#1a1a1a', color: '#555', border: '1px solid #333',
            padding: '2px 8px', cursor: 'pointer', fontSize: 11, fontFamily: 'monospace',
            marginLeft: 8,
          }}
        >+ Window</button>
        APEX TERMINAL
      </div>
    </div>
  )
}
