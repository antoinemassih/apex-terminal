import { useState } from 'react'
import { useChartStore } from '../store/chartStore'
import type { Timeframe } from '../types'

const TIMEFRAMES: Timeframe[] = ['1m', '5m', '15m', '1h', '1d', '1wk']

export function Toolbar() {
  const { panes, activePane, setSymbol, setTimeframe } = useChartStore()
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
      <div style={{ marginLeft: 'auto', color: '#333', fontSize: 10, letterSpacing: 2 }}>
        APEX TERMINAL
      </div>
    </div>
  )
}
