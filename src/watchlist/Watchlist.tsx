import { useEffect, useRef, useState, useCallback } from 'react'
import { useWatchlistStore } from '../store/watchlistStore'
import { getTheme } from '../themes'
import { useChartStore } from '../store/chartStore'
import { getDataProvider } from '../globals'
import { searchSymbols } from './symbols'
import { OptionsChain } from './OptionsChain'
import { OptionsWatchlist } from './OptionsWatchlist'

function fmt(price: number): string {
  if (price >= 1000) return price.toLocaleString('en-US', { minimumFractionDigits: 2, maximumFractionDigits: 2 })
  if (price >= 10)   return price.toFixed(2)
  return price.toFixed(4)
}

function pct(price: number, prevClose: number | null): string | null {
  if (prevClose == null || prevClose === 0) return null
  return ((price - prevClose) / prevClose * 100).toFixed(2)
}

export function Watchlist() {
  // Granular selectors — avoid re-render on every price tick for unrelated properties
  const symbols = useWatchlistStore(s => s.symbols)
  const open = useWatchlistStore(s => s.open)
  const prices = useWatchlistStore(s => s.prices)
  const addSymbol = useWatchlistStore(s => s.addSymbol)
  const removeSymbol = useWatchlistStore(s => s.removeSymbol)
  const setPrice = useWatchlistStore(s => s.setPrice)
  const setPrevClose = useWatchlistStore(s => s.setPrevClose)
  const mode = useWatchlistStore(s => s.mode)
  const setMode = useWatchlistStore(s => s.setMode)
  const chainSymbol = useWatchlistStore(s => s.chainSymbol)
  const setChainSymbol = useWatchlistStore(s => s.setChainSymbol)
  const savedOptions = useWatchlistStore(s => s.savedOptions)
  const themeName = useChartStore(s => s.theme)
  const activePane = useChartStore(s => s.activePane)
  const panes = useChartStore(s => s.panes)
  const setChartSymbol = useChartStore(s => s.setSymbol)
  const theme = getTheme(themeName)

  const [input, setInput] = useState('')
  const [suggestions, setSuggestions] = useState<{ symbol: string; name: string }[]>([])
  const [suggestIndex, setSuggestIndex] = useState(-1)
  const inputRef = useRef<HTMLInputElement>(null)

  // Sync chain symbol with active pane when switching to chain mode
  useEffect(() => {
    if (mode === 'chain') {
      const pane = panes.find(p => p.id === activePane)
      if (pane?.symbol && pane.symbol !== chainSymbol) setChainSymbol(pane.symbol)
    }
  }, [mode, activePane])

  // Subscribe to ticks for watchlist symbols
  useEffect(() => {
    let unsub: (() => void) | null = null
    let cancelled = false

    const attach = () => {
      try {
        const provider = getDataProvider()
        symbols.forEach(sym => provider.subscribe(sym, '1m'))
        unsub = provider.onTick((symbol, _tf, tick) => {
          if (symbols.includes(symbol)) setPrice(symbol, tick.price)
        })
        symbols.forEach(async (sym) => {
          try {
            const res = await provider.getHistory({ symbol: sym, timeframe: '1d', limit: 2 })
            if (res.bars.length >= 2) setPrevClose(sym, res.bars[res.bars.length - 2].close)
            else if (res.bars.length === 1) setPrevClose(sym, res.bars[0].open)
          } catch { /* prev close stays null */ }
        })
      } catch {
        if (!cancelled) setTimeout(attach, 500)
      }
    }

    attach()
    return () => {
      cancelled = true
      unsub?.()
      try {
        const provider = getDataProvider()
        symbols.forEach(sym => provider.unsubscribe(sym, '1m'))
      } catch { /* ignore */ }
    }
  }, [symbols])

  const handleInputChange = useCallback((val: string) => {
    setInput(val.toUpperCase())
    setSuggestions(searchSymbols(val, 8))
    setSuggestIndex(-1)
  }, [])

  const commit = useCallback((sym: string) => {
    const s = sym.trim().toUpperCase()
    if (!s) return
    addSymbol(s)
    setInput('')
    setSuggestions([])
    setSuggestIndex(-1)
    inputRef.current?.focus()
  }, [addSymbol])

  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    if (e.key === 'ArrowDown') {
      e.preventDefault()
      setSuggestIndex(i => Math.min(i + 1, suggestions.length - 1))
    } else if (e.key === 'ArrowUp') {
      e.preventDefault()
      setSuggestIndex(i => Math.max(i - 1, -1))
    } else if (e.key === 'Enter') {
      e.preventDefault()
      if (suggestIndex >= 0 && suggestions[suggestIndex]) commit(suggestions[suggestIndex].symbol)
      else commit(input)
    } else if (e.key === 'Escape') {
      setSuggestions([])
      setSuggestIndex(-1)
      setInput('')
    }
  }, [suggestions, suggestIndex, input, commit])

  const bg     = theme.watchlistBackground ?? theme.toolbarBackground
  const border = theme.toolbarBorder
  const text   = theme.axisText
  const accent = theme.borderActive
  const bull   = theme.bull ?? '#26a69a'
  const bear   = theme.bear ?? '#ef5350'

  // Widen panel for chain / saved modes to fit option columns
  const panelWidth = mode === 'stocks' ? 200 : 260

  const tabBtn = (m: typeof mode, label: string, badge?: number) => (
    <button
      key={m}
      onClick={() => setMode(m)}
      style={{
        flex: 1,
        background: mode === m ? accent + '22' : 'transparent',
        color: mode === m ? accent : text,
        border: 'none',
        borderBottom: mode === m ? `2px solid ${accent}` : '2px solid transparent',
        fontSize: 9, fontFamily: 'monospace', letterSpacing: 0.5,
        padding: '4px 0', cursor: 'pointer',
        display: 'flex', alignItems: 'center', justifyContent: 'center', gap: 3,
      }}
    >
      {label}
      {badge != null && badge > 0 && (
        <span style={{
          background: accent + '44', color: accent,
          borderRadius: 8, padding: '0 4px', fontSize: 8, lineHeight: '14px',
        }}>{badge}</span>
      )}
    </button>
  )

  return (
    <div style={{
      width: panelWidth, flexShrink: 0, display: open ? 'flex' : 'none', flexDirection: 'column',
      borderLeft: `1px solid ${border}`, background: bg,
      fontFamily: 'monospace', fontSize: 13, overflow: 'hidden',
    }}>
      {/* Header */}
      <div style={{
        padding: '3px 8px 0', borderBottom: `1px solid ${border}`,
        flexShrink: 0, fontSize: 9, letterSpacing: 1,
      }}>
        <div style={{ color: text, opacity: 0.4, marginBottom: 3 }}>WATCHLIST</div>
        <div style={{ display: 'flex' }}>
          {tabBtn('stocks', 'STOCKS')}
          {tabBtn('chain', 'CHAIN')}
          {tabBtn('saved', 'SAVED', savedOptions.length)}
        </div>
      </div>

      {mode === 'stocks' && (
        <>
          {/* Search input */}
          <div style={{ position: 'relative', flexShrink: 0, borderBottom: `1px solid ${border}` }}>
            <input
              ref={inputRef}
              value={input}
              onChange={e => handleInputChange(e.target.value)}
              onKeyDown={handleKeyDown}
              onBlur={() => setTimeout(() => setSuggestions([]), 150)}
              placeholder="Add symbol…"
              style={{
                width: '100%', background: theme.background, color: text,
                border: 'none', borderBottom: suggestions.length > 0 ? `1px solid ${border}` : 'none',
                padding: '4px 8px', fontSize: 13, fontFamily: 'monospace',
                outline: 'none', boxSizing: 'border-box',
              }}
            />
            {suggestions.length > 0 && (
              <div style={{
                position: 'absolute', top: '100%', left: 0, right: 0, zIndex: 100,
                background: bg, border: `1px solid ${border}`,
                borderTop: 'none', boxShadow: '0 4px 12px rgba(0,0,0,0.4)',
              }}>
                {suggestions.map((s, i) => (
                  <div
                    key={s.symbol}
                    onMouseDown={() => commit(s.symbol)}
                    style={{
                      display: 'flex', alignItems: 'baseline', gap: 6,
                      padding: '3px 8px', cursor: 'pointer',
                      background: i === suggestIndex ? accent + '22' : 'transparent',
                      borderBottom: i < suggestions.length - 1 ? `1px solid ${border}22` : 'none',
                    }}
                    onMouseEnter={() => setSuggestIndex(i)}
                  >
                    <span style={{ color: accent, fontWeight: 'bold', flex: '0 0 52px' }}>{s.symbol}</span>
                    <span style={{ color: text, opacity: 0.5, fontSize: 10, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                      {s.name}
                    </span>
                  </div>
                ))}
              </div>
            )}
          </div>

          {/* Symbol rows */}
          <div style={{ flex: 1, overflowY: 'auto' }}>
            {symbols.map(sym => {
              const p = prices[sym]
              const change = p?.price != null ? pct(p.price, p?.prevClose ?? null) : null
              const changeNum = change != null ? parseFloat(change) : null
              const changeColor = changeNum == null ? text : changeNum >= 0 ? bull : bear

              return (
                <div
                  key={sym}
                  onClick={() => activePane && setChartSymbol(activePane, sym)}
                  style={{
                    display: 'flex', alignItems: 'center',
                    padding: '3px 8px', gap: 4,
                    borderBottom: `1px solid ${border}22`,
                    cursor: 'pointer',
                  }}
                  onMouseEnter={e => (e.currentTarget as HTMLElement).style.background = accent + '11'}
                  onMouseLeave={e => (e.currentTarget as HTMLElement).style.background = 'transparent'}
                >
                  <span style={{ color: text, flex: '0 0 50px', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                    {sym}
                  </span>
                  <span style={{ color: text, flex: 1, textAlign: 'right', opacity: p?.price != null ? 1 : 0.3 }}>
                    {p?.price != null ? fmt(p.price) : '—'}
                  </span>
                  <span style={{ color: changeColor, flex: '0 0 46px', textAlign: 'right' }}>
                    {change != null ? `${changeNum! >= 0 ? '+' : ''}${change}%` : '—'}
                  </span>
                  <button
                    onClick={e => { e.stopPropagation(); removeSymbol(sym) }}
                    style={{
                      background: 'none', border: 'none', color: text,
                      cursor: 'pointer', opacity: 0.25, fontSize: 10,
                      padding: 0, lineHeight: 1, flexShrink: 0,
                    }}
                    onMouseEnter={e => (e.currentTarget as HTMLElement).style.opacity = '1'}
                    onMouseLeave={e => (e.currentTarget as HTMLElement).style.opacity = '0.25'}
                  >✕</button>
                </div>
              )
            })}
          </div>
        </>
      )}

      {mode === 'chain' && <OptionsChain />}
      {mode === 'saved' && <OptionsWatchlist />}
    </div>
  )
}
