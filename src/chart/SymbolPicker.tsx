import { useState, useRef, useEffect, useCallback } from 'react'
import { getTheme } from '../themes'
import { useChartStore } from '../store/chartStore'
import { OptionsPanel } from './OptionsPanel'

const OCOCO_API = 'http://192.168.1.60:30300'

interface SymbolInfo {
  symbol: string
  name: string | null
  type?: string
}

async function apiSearch(q: string): Promise<SymbolInfo[]> {
  try {
    const res = await fetch(`${OCOCO_API}/api/symbols?q=${encodeURIComponent(q)}`)
    if (!res.ok) return []
    return res.json()
  } catch { return [] }
}

async function apiRecents(): Promise<SymbolInfo[]> {
  try {
    const res = await fetch(`${OCOCO_API}/api/recents`)
    if (!res.ok) return []
    return res.json()
  } catch { return [] }
}

async function apiTouchRecent(symbol: string): Promise<void> {
  try {
    await fetch(`${OCOCO_API}/api/recents`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ symbol }),
    })
  } catch { /* */ }
}

async function apiAllSymbols(): Promise<SymbolInfo[]> {
  try {
    const res = await fetch(`${OCOCO_API}/api/symbols`)
    if (!res.ok) return []
    return res.json()
  } catch { return [] }
}

interface Props {
  paneId: string
  anchorX: number
  anchorY: number
  onClose: () => void
}

export function SymbolPicker({ paneId, anchorX, anchorY, onClose }: Props) {
  const [query, setQuery] = useState('')
  const [results, setResults] = useState<SymbolInfo[]>([])
  const [recents, setRecents] = useState<SymbolInfo[]>([])
  const [popular, setPopular] = useState<SymbolInfo[]>([])
  const [loading, setLoading] = useState(false)
  const [optionsSymbol, setOptionsSymbol] = useState<string | null>(null)
  const [optionsAnchor, setOptionsAnchor] = useState({ x: 0, y: 0 })
  const inputRef = useRef<HTMLInputElement>(null)
  const searchTimer = useRef<ReturnType<typeof setTimeout> | null>(null)
  const theme = getTheme(useChartStore(s => s.theme))
  const { setSymbol } = useChartStore()

  // Load recents + popular on open
  useEffect(() => {
    inputRef.current?.focus()
    apiRecents().then(setRecents)
    apiAllSymbols().then(all => setPopular(all.slice(0, 30)))
  }, [])

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

  // Debounced search
  useEffect(() => {
    if (query.length === 0) { setResults([]); return }
    if (searchTimer.current) clearTimeout(searchTimer.current)
    setLoading(true)
    searchTimer.current = setTimeout(() => {
      apiSearch(query).then(r => { setResults(r); setLoading(false) })
    }, 150)
    return () => { if (searchTimer.current) clearTimeout(searchTimer.current) }
  }, [query])

  const select = useCallback((sym: string) => {
    const s = sym.trim().toUpperCase()
    if (!s) return
    setSymbol(paneId, s)
    apiTouchRecent(s)
    onClose()
  }, [paneId, setSymbol, onClose])

  const bg = theme.toolbarBackground
  const border = theme.toolbarBorder
  const text = theme.ohlcLabel
  const accent = theme.borderActive
  const dim = theme.axisText

  return (
    <div id="symbol-picker" style={{
      position: 'fixed', left: Math.min(anchorX, window.innerWidth - 260), top: anchorY,
      width: 250, maxHeight: 380, overflow: 'hidden',
      background: bg, border: `1px solid ${border}`, borderRadius: 4,
      boxShadow: '0 4px 16px rgba(0,0,0,0.5)',
      zIndex: 10000, fontFamily: 'monospace', fontSize: 11,
      display: 'flex', flexDirection: 'column',
    }}
    onMouseDown={e => e.stopPropagation()}>
      <div style={{ padding: '6px 8px', borderBottom: `1px solid ${border}` }}>
        <input ref={inputRef}
          value={query}
          onChange={e => setQuery(e.target.value)}
          onKeyDown={e => { if (e.key === 'Enter' && query.trim()) select(query) }}
          placeholder="Search symbols..."
          style={{
            width: '100%', background: theme.background, color: text,
            border: `1px solid ${border}`, borderRadius: 3,
            padding: '5px 8px', fontSize: 12, fontFamily: 'monospace', outline: 'none',
          }}
        />
      </div>

      <div style={{ overflowY: 'auto', flex: 1 }}>
        {/* Search results */}
        {query.length > 0 && (
          <>
            {loading && <div style={{ padding: '6px 8px', color: dim }}>Searching...</div>}
            {!loading && results.map(s => (
              <Item key={s.symbol} symbol={s.symbol} name={s.name} accent={accent} text={text} dim={dim} bg={bg}
                onSelect={select} onOptions={(x, y) => { setOptionsSymbol(s.symbol); setOptionsAnchor({ x, y }) }} />
            ))}
            {!loading && results.length === 0 && query.length >= 1 && (
              <Item symbol={query.toUpperCase()} name="Custom symbol" accent={accent} text={text} dim={dim} bg={bg} onSelect={select} />
            )}
          </>
        )}

        {/* Recents */}
        {query.length === 0 && recents.length > 0 && (
          <>
            <div style={{ padding: '5px 8px 2px', color: accent, fontSize: 9, letterSpacing: 1 }}>RECENT</div>
            {recents.map(s => (
              <Item key={s.symbol} symbol={s.symbol} name={s.name} accent={accent} text={text} dim={dim} bg={bg}
                onSelect={select} onOptions={(x, y) => { setOptionsSymbol(s.symbol); setOptionsAnchor({ x, y }) }} />
            ))}
          </>
        )}

        {/* Popular */}
        {query.length === 0 && popular.length > 0 && (
          <>
            <div style={{ padding: '5px 8px 2px', color: accent, fontSize: 9, letterSpacing: 1, marginTop: 2 }}>ALL SYMBOLS</div>
            {popular.map(s => (
              <Item key={s.symbol} symbol={s.symbol} name={s.name} accent={accent} text={text} dim={dim} bg={bg}
                onSelect={select} onOptions={(x, y) => { setOptionsSymbol(s.symbol); setOptionsAnchor({ x, y }) }} />
            ))}
          </>
        )}
      </div>

      {/* Options chain slide-out */}
      {optionsSymbol && (
        <OptionsPanel
          symbol={optionsSymbol}
          anchorX={optionsAnchor.x}
          anchorY={anchorY}
          maxHeight={380}
          onClose={() => setOptionsSymbol(null)}
          onSelect={(contract) => {
            select(contract)
            setOptionsSymbol(null)
          }}
        />
      )}
    </div>
  )
}

function Item({ symbol, name, accent, text, dim, bg, onSelect, onOptions }: {
  symbol: string; name: string | null; accent: string; text: string; dim: string; bg: string
  onSelect: (s: string) => void; onOptions?: (x: number, y: number) => void
}) {
  const [hover, setHover] = useState(false)
  const ref = useRef<HTMLDivElement>(null)
  return (
    <div ref={ref}
      onClick={() => onSelect(symbol)}
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => setHover(false)}
      style={{
        padding: '4px 8px', cursor: 'pointer',
        background: hover ? accent + '22' : bg,
        color: hover ? accent : text,
        display: 'flex', justifyContent: 'space-between', alignItems: 'center',
        position: 'relative',
      }}>
      <span style={{ fontWeight: 'bold' }}>{symbol}</span>
      <div style={{ display: 'flex', alignItems: 'center', gap: 4 }}>
        {name && !hover && <span style={{ color: dim, fontSize: 9, maxWidth: 100, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{name}</span>}
        {hover && onOptions && (
          <span
            onClick={e => {
              e.stopPropagation()
              const rect = ref.current?.getBoundingClientRect()
              if (rect) onOptions(rect.right + 4, rect.top)
            }}
            style={{
              fontSize: 9, color: accent, background: accent + '22',
              padding: '1px 6px', borderRadius: 3, cursor: 'pointer',
              border: `1px solid ${accent}44`,
            }}>
            Options
          </span>
        )}
      </div>
    </div>
  )
}
