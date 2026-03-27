import { useState, useRef } from 'react'
import { useChartStore, type Layout } from '../store/chartStore'
import { useDrawingStore } from '../store/drawingStore'
import { useWatchlistStore } from '../store/watchlistStore'
import { useOrderStore } from '../store/orderStore'
import { INDICATOR_CATALOG } from '../indicators'
import { THEMES, getTheme } from '../themes'
import { SymbolPicker } from '../chart/SymbolPicker'
import { TrendlineFilters } from './TrendlineFilters'
import { ConnectionPanel } from './ConnectionPanel'
import type { Timeframe } from '../types'

const IS_TAURI = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window

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
  // Granular selectors — only re-render when the specific value changes
  const activePane = useChartStore(s => s.activePane)
  const pane = useChartStore(s => s.panes.find(p => p.id === s.activePane))
  const setTimeframe = useChartStore(s => s.setTimeframe)
  const toggleVolume = useChartStore(s => s.toggleVolume)
  const toggleIndicator = useChartStore(s => s.toggleIndicator)
  const layout = useChartStore(s => s.layout)
  const setLayout = useChartStore(s => s.setLayout)
  const themeName = useChartStore(s => s.theme)
  const setTheme = useChartStore(s => s.setTheme)
  const activeTool = useDrawingStore(s => s.activeTool)
  const setActiveTool = useDrawingStore(s => s.setActiveTool)
  const watchlistOpen = useWatchlistStore(s => s.open)
  const toggleWatchlist = useWatchlistStore(s => s.toggleOpen)
  const orderEntryEnabled = useOrderStore(s => s.enabled)
  const toggleOrderEntry = useOrderStore(s => s.toggleEnabled)
  const ordersOpen = useOrderStore(s => s.ordersOpen)
  const toggleOrdersOpen = useOrderStore(s => s.toggleOrdersOpen)
  const [pickerOpen, setPickerOpen] = useState(false)
  const tickerRef = useRef<HTMLSpanElement>(null)
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

  const winBtn = (danger = false): React.CSSProperties => ({
    background: 'none',
    border: 'none',
    color: theme.axisText,
    cursor: 'pointer',
    width: 40,
    height: 36,
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    fontSize: 13,
    fontFamily: 'monospace',
    flexShrink: 0,
    transition: 'background 0.1s, color 0.1s',
    ...(danger ? {} : {}),
  })

  return (
    <div
      style={{
        display: 'flex', alignItems: 'center', gap: 6,
        height: 36, background: theme.toolbarBackground, borderBottom: `1px solid ${theme.toolbarBorder}`,
        padding: '0 0 0 10px', flexShrink: 0, fontFamily: 'monospace', fontSize: 12,
        userSelect: 'none',
      }}
    >
      {/* Logo mark */}
      <svg width="15" height="15" viewBox="0 0 15 15" fill="none" style={{ flexShrink: 0, marginRight: 2 }}>
        <path d="M7.5 1.5L13.5 13H1.5L7.5 1.5Z" stroke={theme.borderActive} strokeWidth="1.3" fill="none" strokeLinejoin="round" />
        <line x1="4.8" y1="9" x2="10.2" y2="9" stroke={theme.borderActive} strokeWidth="1.3" strokeLinecap="round" />
      </svg>

      {/* Symbol picker */}
      <span ref={tickerRef}
        onClick={() => setPickerOpen(!pickerOpen)}
        style={{ color: theme.borderActive, fontWeight: 'bold', cursor: 'pointer', padding: '2px 6px',
          background: pickerOpen ? theme.borderActive + '22' : 'transparent', borderRadius: 3 }}>
        {pane?.symbol ?? '—'} &#9662;
      </span>
      {pickerOpen && activePane && tickerRef.current && (() => {
        const r = tickerRef.current!.getBoundingClientRect()
        return <SymbolPicker paneId={activePane} anchorX={r.left} anchorY={r.bottom + 4} onClose={() => setPickerOpen(false)} />
      })()}

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

      {/* Trendline filters dropdown */}
      <div style={{ marginLeft: 4, borderLeft: `1px solid ${theme.toolbarBorder}`, paddingLeft: 6 }}>
        <TrendlineFilters />
      </div>

      {/* Order entry + orders panel toggles */}
      <div style={{ display: 'flex', gap: 2, marginLeft: 4, borderLeft: `1px solid ${theme.toolbarBorder}`, paddingLeft: 6 }}>
        <button onClick={toggleOrderEntry} style={{
          ...btnStyle(orderEntryEnabled),
          ...(orderEntryEnabled ? {
            background: theme.borderActive + '22',
            color: theme.borderActive,
            border: `1px solid ${theme.borderActive}66`,
          } : {}),
        }}>
          orders
        </button>
        <button onClick={toggleOrdersOpen} style={{
          ...btnStyle(ordersOpen),
          ...(ordersOpen ? {
            background: theme.borderActive + '22',
            color: theme.borderActive,
            border: `1px solid ${theme.borderActive}66`,
          } : {}),
        }}
          title="Order book"
        >
          book
        </button>
      </div>

      {/* Drag region — fills remaining space, window draggable here */}
      <div data-tauri-drag-region style={{ flex: 1, height: '100%', cursor: 'default' }} />

      {/* Right-side actions */}
      <div style={{ display: 'flex', alignItems: 'center', gap: 6, paddingRight: 4 }}>
        <ConnectionPanel />
        {IS_TAURI && (
          <button onClick={() => {
            import('@tauri-apps/api/webviewWindow').then(({ WebviewWindow }) => {
              const label = `chart-${Date.now()}`
              const w = new WebviewWindow(label, { title: 'Apex Terminal', width: 1920, height: 1080, decorations: false })
              w.once('tauri://error', (e: unknown) => console.error('Window creation failed:', e))
            })
          }} style={btnStyle(false)}>
            + Window
          </button>
        )}
        <button onClick={toggleWatchlist} style={btnStyle(watchlistOpen)} title="Toggle watchlist">
          <svg width="12" height="10" viewBox="0 0 12 10" fill="none">
            <line x1="0" y1="1" x2="12" y2="1" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round"/>
            <line x1="0" y1="5" x2="12" y2="5" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round"/>
            <line x1="0" y1="9" x2="12" y2="9" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round"/>
          </svg>
        </button>
      </div>

      {/* Window controls — Tauri only */}
      {IS_TAURI && (
        <div style={{ display: 'flex', borderLeft: `1px solid ${theme.toolbarBorder}`, height: '100%', alignItems: 'stretch' }}>
          <button
            style={winBtn()}
            onMouseEnter={e => { (e.currentTarget as HTMLElement).style.background = theme.toolbarBorder }}
            onMouseLeave={e => { (e.currentTarget as HTMLElement).style.background = 'none' }}
            onClick={() => import('@tauri-apps/api/window').then(({ getCurrentWindow }) => getCurrentWindow().minimize())}
            title="Minimize"
          >
            <svg width="10" height="2" viewBox="0 0 10 2"><line x1="0" y1="1" x2="10" y2="1" stroke="currentColor" strokeWidth="1.2" /></svg>
          </button>
          <button
            style={winBtn()}
            onMouseEnter={e => { (e.currentTarget as HTMLElement).style.background = theme.toolbarBorder }}
            onMouseLeave={e => { (e.currentTarget as HTMLElement).style.background = 'none' }}
            onClick={() => import('@tauri-apps/api/window').then(({ getCurrentWindow }) => getCurrentWindow().toggleMaximize())}
            title="Maximize"
          >
            <svg width="10" height="10" viewBox="0 0 10 10" fill="none"><rect x="0.6" y="0.6" width="8.8" height="8.8" stroke="currentColor" strokeWidth="1.2" /></svg>
          </button>
          <button
            style={winBtn(true)}
            onMouseEnter={e => { (e.currentTarget as HTMLElement).style.background = '#e05560'; (e.currentTarget as HTMLElement).style.color = '#fff' }}
            onMouseLeave={e => { (e.currentTarget as HTMLElement).style.background = 'none'; (e.currentTarget as HTMLElement).style.color = theme.axisText }}
            onClick={() => import('@tauri-apps/api/window').then(({ getCurrentWindow }) => getCurrentWindow().close())}
            title="Close"
          >
            <svg width="10" height="10" viewBox="0 0 10 10"><line x1="0" y1="0" x2="10" y2="10" stroke="currentColor" strokeWidth="1.2" /><line x1="10" y1="0" x2="0" y2="10" stroke="currentColor" strokeWidth="1.2" /></svg>
          </button>
        </div>
      )}
    </div>
  )
}
