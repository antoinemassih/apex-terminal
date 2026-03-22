import { useState, useRef, useEffect, useCallback } from 'react'
import { getTheme } from '../themes'
import { useChartStore } from '../store/chartStore'

const POPULAR = [
  'AAPL', 'MSFT', 'GOOG', 'AMZN', 'META', 'NVDA', 'TSLA', 'AMD', 'NFLX', 'CRM',
  'SPY', 'QQQ', 'IWM', 'DIA', 'VIX',
  'JPM', 'BAC', 'GS', 'V', 'MA',
  'XOM', 'CVX', 'COIN', 'MARA', 'SQ',
  'BTC-USD', 'ETH-USD', 'SOL-USD',
]

const RECENT_KEY = 'apex-recent-symbols'
const MAX_RECENT = 12

function getRecent(): string[] {
  try {
    const raw = localStorage.getItem(RECENT_KEY)
    return raw ? JSON.parse(raw) : []
  } catch { return [] }
}

function addRecent(symbol: string): void {
  const recent = getRecent().filter(s => s !== symbol)
  recent.unshift(symbol)
  if (recent.length > MAX_RECENT) recent.length = MAX_RECENT
  try { localStorage.setItem(RECENT_KEY, JSON.stringify(recent)) } catch { /* */ }
}

interface Props {
  /** Which pane to change symbol on */
  paneId: string
  /** Position anchor for the dropdown */
  anchorX: number
  anchorY: number
  onClose: () => void
}

export function SymbolPicker({ paneId, anchorX, anchorY, onClose }: Props) {
  const [query, setQuery] = useState('')
  const [recent] = useState(getRecent)
  const inputRef = useRef<HTMLInputElement>(null)
  const theme = getTheme(useChartStore(s => s.theme))
  const { setSymbol } = useChartStore()

  useEffect(() => { inputRef.current?.focus() }, [])

  // Close on escape or click outside
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => { if (e.key === 'Escape') onClose() }
    const onClick = (e: MouseEvent) => {
      const el = document.getElementById('symbol-picker')
      if (el && !el.contains(e.target as Node)) onClose()
    }
    window.addEventListener('keydown', onKey)
    window.addEventListener('mousedown', onClick, true)
    return () => {
      window.removeEventListener('keydown', onKey)
      window.removeEventListener('mousedown', onClick, true)
    }
  }, [onClose])

  const select = useCallback((sym: string) => {
    const s = sym.trim().toUpperCase()
    if (!s) return
    setSymbol(paneId, s)
    addRecent(s)
    onClose()
  }, [paneId, setSymbol, onClose])

  const q = query.toUpperCase()
  const filtered = q.length > 0
    ? POPULAR.filter(s => s.includes(q))
    : []

  const bg = theme.toolbarBackground
  const border = theme.toolbarBorder
  const text = theme.ohlcLabel
  const accent = theme.borderActive

  return (
    <div id="symbol-picker" style={{
      position: 'fixed', left: anchorX, top: anchorY,
      width: 220, maxHeight: 340, overflow: 'hidden',
      background: bg, border: `1px solid ${border}`, borderRadius: 4,
      boxShadow: '0 4px 16px rgba(0,0,0,0.5)',
      zIndex: 10000, fontFamily: 'monospace', fontSize: 11,
      display: 'flex', flexDirection: 'column',
    }}
    onMouseDown={e => e.stopPropagation()}>
      {/* Search input */}
      <div style={{ padding: '6px 8px', borderBottom: `1px solid ${border}` }}>
        <input ref={inputRef}
          value={query}
          onChange={e => setQuery(e.target.value)}
          onKeyDown={e => {
            if (e.key === 'Enter' && query.trim()) select(query)
          }}
          placeholder="Search symbol..."
          style={{
            width: '100%', background: theme.background, color: text,
            border: `1px solid ${border}`, borderRadius: 3,
            padding: '4px 8px', fontSize: 12, fontFamily: 'monospace',
            outline: 'none',
          }}
        />
      </div>

      <div style={{ overflowY: 'auto', flex: 1 }}>
        {/* Search results */}
        {q.length > 0 && (
          <>
            {filtered.map(s => (
              <Item key={s} symbol={s} accent={accent} text={text} bg={bg} onSelect={select} />
            ))}
            {/* Always show typed query as an option */}
            {!filtered.includes(q) && q.length >= 1 && (
              <Item symbol={q} accent={accent} text={text} bg={bg} onSelect={select} suffix="(custom)" />
            )}
          </>
        )}

        {/* Recent (only when not searching) */}
        {q.length === 0 && recent.length > 0 && (
          <>
            <div style={{ padding: '4px 8px', color: accent, fontSize: 9, letterSpacing: 1 }}>RECENT</div>
            {recent.map(s => (
              <Item key={s} symbol={s} accent={accent} text={text} bg={bg} onSelect={select} />
            ))}
          </>
        )}

        {/* Popular (only when not searching) */}
        {q.length === 0 && (
          <>
            <div style={{ padding: '4px 8px', color: accent, fontSize: 9, letterSpacing: 1, marginTop: 4 }}>POPULAR</div>
            {POPULAR.slice(0, 20).map(s => (
              <Item key={s} symbol={s} accent={accent} text={text} bg={bg} onSelect={select} />
            ))}
          </>
        )}
      </div>
    </div>
  )
}

function Item({ symbol, accent, text, bg, onSelect, suffix }: {
  symbol: string; accent: string; text: string; bg: string; onSelect: (s: string) => void; suffix?: string
}) {
  const [hover, setHover] = useState(false)
  return (
    <div
      onClick={() => onSelect(symbol)}
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => setHover(false)}
      style={{
        padding: '4px 8px', cursor: 'pointer',
        background: hover ? accent + '22' : bg,
        color: hover ? accent : text,
        display: 'flex', justifyContent: 'space-between',
      }}>
      <span>{symbol}</span>
      {suffix && <span style={{ color: '#555', fontSize: 9 }}>{suffix}</span>}
    </div>
  )
}
