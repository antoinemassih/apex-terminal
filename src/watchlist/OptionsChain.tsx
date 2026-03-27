import { useState, useEffect, useRef, useCallback } from 'react'
import { useWatchlistStore } from '../store/watchlistStore'
import { useChartStore } from '../store/chartStore'
import { getTheme } from '../themes'
import { buildChain, optionKey, tradingDateLabel } from '../data/optionsSim'
import type { OptionRow, OptionChain, SavedOption } from '../data/optionsSim'
import { searchSymbols } from './symbols'

function fmtPrice(p: number): string {
  if (p <= 0) return '.00'
  if (p < 1) return p.toFixed(2)
  if (p < 10) return p.toFixed(2)
  return p.toFixed(1)
}

function fmtOI(n: number): string {
  if (n >= 1_000_000) return (n / 1_000_000).toFixed(1) + 'M'
  if (n >= 1_000) return (n / 1_000).toFixed(1) + 'K'
  return String(n)
}

function fmtStrike(k: number): string {
  return k % 1 === 0 ? String(k) : k.toFixed(1)
}

function fmtExpiry(expiry: string, dte: number): string {
  const months = ['Jan','Feb','Mar','Apr','May','Jun','Jul','Aug','Sep','Oct','Nov','Dec']
  const d = new Date(expiry + 'T12:00:00')
  return `${dte}DTE  ${months[d.getMonth()]} ${d.getDate()}`
}

interface RowProps {
  row: OptionRow
  symbol: string
  saved: boolean
  selectMode: boolean
  onToggle: (opt: SavedOption) => void
  bull: string
  bear: string
  accent: string
  text: string
  border: string
}

function OptionRowComp({ row, symbol, saved, selectMode, onToggle, bull, bear, accent, text, border }: RowProps) {
  const [hovered, setHovered] = useState(false)
  const isCall = row.type === 'call'

  const handleClick = (e: React.MouseEvent) => {
    if (selectMode || e.shiftKey) {
      const key = optionKey(symbol, row)
      onToggle({ key, symbol, strike: row.strike, type: row.type, expiry: row.expiry, dte: row.dte })
    }
  }

  const sectionTint = isCall ? bull + '0a' : bear + '0a'

  const background = saved
    ? accent + '30'
    : row.isATM
      ? accent + '16'
      : hovered
        ? accent + '10'
        : sectionTint

  return (
    <div
      onClick={handleClick}
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
      style={{
        display: 'flex', alignItems: 'center',
        padding: '2px 6px 2px 8px',
        borderBottom: `1px solid ${border}18`,
        background,
        cursor: selectMode ? 'pointer' : 'default',
        userSelect: 'none',
      }}
    >
      {/* ATM marker */}
      <span style={{ width: 8, fontSize: 9, color: accent, flexShrink: 0 }}>
        {row.isATM ? '◆' : ''}
      </span>

      {/* Strike */}
      <span style={{ width: 46, textAlign: 'right', color: row.isATM ? accent : text, fontWeight: row.isATM ? 'bold' : 'normal', fontSize: 13 }}>
        {fmtStrike(row.strike)}
      </span>

      {/* Type badge */}
      <span style={{ width: 16, textAlign: 'center', fontSize: 10, color: isCall ? bull : bear, opacity: 0.7, flexShrink: 0 }}>
        {isCall ? 'C' : 'P'}
      </span>

      {/* Bid */}
      <span style={{ width: 40, textAlign: 'right', color: text, opacity: 0.75, fontSize: 13 }}>
        {fmtPrice(row.bid)}
      </span>

      {/* Ask */}
      <span style={{ width: 40, textAlign: 'right', color: text, fontSize: 13 }}>
        {fmtPrice(row.ask)}
      </span>

      {/* OI */}
      <span style={{ width: 46, textAlign: 'right', color: text, opacity: 0.6, fontSize: 12 }}>
        {fmtOI(row.oi)}
      </span>

      {/* Saved indicator */}
      <span style={{ width: 10, textAlign: 'right', color: accent, fontSize: 9, opacity: saved ? 1 : 0, flexShrink: 0 }}>●</span>
    </div>
  )
}

function SectionHeader({ label, color, border }: { label: string; color: string; border: string }) {
  return (
    <div style={{
      padding: '2px 8px', fontSize: 10, letterSpacing: 1,
      color, opacity: 0.6, borderBottom: `1px solid ${border}22`,
      marginTop: 2,
    }}>
      {label}
    </div>
  )
}

function ExpiryBlock({
  chain, underlying, symbol, savedKeys, selectMode, onToggle,
  bull, bear, accent, text, border,
}: {
  chain: OptionChain
  underlying: number
  symbol: string
  savedKeys: Set<string>
  selectMode: boolean
  onToggle: (opt: SavedOption) => void
  bull: string; bear: string; accent: string; text: string; border: string
}) {
  return (
    <div>
      <div style={{
        padding: '4px 8px 3px',
        fontSize: 11, color: accent, opacity: 0.85,
        background: accent + '08',
        borderTop: `1px solid ${border}33`,
        borderBottom: `1px solid ${border}33`,
        letterSpacing: 0.5,
      }}>
        {fmtExpiry(chain.expiry, chain.dte)}
      </div>

      <SectionHeader label="CALLS" color={bull} border={border} />
      {chain.calls.map(row => (
        <OptionRowComp key={row.strike + '-call'}
          row={row} symbol={symbol}
          saved={savedKeys.has(optionKey(symbol, row))}
          selectMode={selectMode}
          onToggle={onToggle}
          bull={bull} bear={bear} accent={accent} text={text} border={border}
        />
      ))}

      {/* Underlying price divider */}
      <div style={{
        padding: '2px 8px', fontSize: 11,
        color: text, opacity: 0.5, textAlign: 'center',
        borderTop: `1px solid ${border}28`, borderBottom: `1px solid ${border}28`,
        background: border + '0a',
      }}>
        {symbol}  ${underlying < 10 ? underlying.toFixed(4) : underlying.toFixed(2)}
      </div>

      <SectionHeader label="PUTS" color={bear} border={border} />
      {chain.puts.map(row => (
        <OptionRowComp key={row.strike + '-put'}
          row={row} symbol={symbol}
          saved={savedKeys.has(optionKey(symbol, row))}
          selectMode={selectMode}
          onToggle={onToggle}
          bull={bull} bear={bear} accent={accent} text={text} border={border}
        />
      ))}
    </div>
  )
}

export function OptionsChain() {
  const {
    chainSymbol, setChainSymbol,
    numStrikes, setNumStrikes,
    farDTE, setFarDTE,
    savedOptions, toggleSavedOption, prices,
  } = useWatchlistStore()
  const { theme: themeName } = useChartStore()
  const theme = getTheme(themeName)

  const [chain0, setChain0] = useState<OptionChain | null>(null)
  const [chainN, setChainN] = useState<OptionChain | null>(null)
  const [selectMode, setSelectMode] = useState(false)

  // Symbol autosuggest
  const [symInput, setSymInput] = useState(chainSymbol)
  const [suggestions, setSuggestions] = useState<{ symbol: string; name: string }[]>([])
  const [suggestIndex, setSuggestIndex] = useState(-1)
  const [focused, setFocused] = useState(false)
  const symInputRef = useRef<HTMLInputElement>(null)

  useEffect(() => {
    if (!focused) setSymInput(chainSymbol)
  }, [chainSymbol, focused])

  const handleSymChange = useCallback((val: string) => {
    setSymInput(val.toUpperCase())
    setSuggestions(searchSymbols(val, 7))
    setSuggestIndex(-1)
  }, [])

  const commitSym = useCallback((sym: string) => {
    const s = sym.trim().toUpperCase()
    if (s) { setChainSymbol(s); setSymInput(s) }
    setSuggestions([])
    setSuggestIndex(-1)
    symInputRef.current?.blur()
  }, [setChainSymbol])

  const handleSymKeyDown = useCallback((e: React.KeyboardEvent) => {
    if (e.key === 'ArrowDown') { e.preventDefault(); setSuggestIndex(i => Math.min(i + 1, suggestions.length - 1)) }
    else if (e.key === 'ArrowUp') { e.preventDefault(); setSuggestIndex(i => Math.max(i - 1, -1)) }
    else if (e.key === 'Enter') {
      e.preventDefault()
      commitSym(suggestIndex >= 0 && suggestions[suggestIndex] ? suggestions[suggestIndex].symbol : symInput)
    } else if (e.key === 'Escape') {
      setSymInput(chainSymbol); setSuggestions([]); setSuggestIndex(-1)
      symInputRef.current?.blur()
    }
  }, [suggestions, suggestIndex, symInput, chainSymbol, commitSym])

  // Rebuild chain
  useEffect(() => {
    const build = () => {
      const price = useWatchlistStore.getState().prices[chainSymbol]?.price ?? 100
      setChain0(buildChain(price, numStrikes, 0))
      setChainN(buildChain(price, numStrikes, farDTE))
    }
    build()
    const id = setInterval(build, 500)
    return () => clearInterval(id)
  }, [chainSymbol, numStrikes, farDTE])

  // Build DTE option labels: show "1DTE - Jun 27" style
  const dteOptions = [1,2,3,4,5,7,10].map(d => ({
    value: d,
    label: `${d}DTE – ${tradingDateLabel(d)}`,
  }))

  const underlying = prices[chainSymbol]?.price ?? 100
  const border = theme.toolbarBorder
  const text   = theme.axisText
  const accent = theme.borderActive
  const bull   = theme.bull ?? '#26a69a'
  const bear   = theme.bear ?? '#ef5350'
  const bg     = theme.watchlistBackground ?? theme.toolbarBackground

  const savedKeys = new Set(savedOptions.map(o => o.key))

  return (
    <div style={{ flex: 1, overflowY: 'auto', overflowX: 'hidden' }}>

      {/* Symbol autosuggest */}
      <div style={{ position: 'relative', borderBottom: `1px solid ${border}` }}>
        <input
          ref={symInputRef}
          value={symInput}
          onChange={e => handleSymChange(e.target.value)}
          onKeyDown={handleSymKeyDown}
          onFocus={() => { setFocused(true); setSymInput(''); setSuggestions(searchSymbols('', 7)) }}
          onBlur={() => setTimeout(() => {
            setFocused(false); setSymInput(chainSymbol); setSuggestions([]); setSuggestIndex(-1)
          }, 150)}
          placeholder={chainSymbol}
          style={{
            width: '100%', background: theme.background, color: accent,
            border: 'none', padding: '5px 8px',
            fontSize: 14, fontFamily: 'monospace', fontWeight: 'bold',
            outline: 'none', boxSizing: 'border-box',
          }}
        />
        {suggestions.length > 0 && (
          <div style={{
            position: 'absolute', top: '100%', left: 0, right: 0, zIndex: 200,
            background: bg, border: `1px solid ${border}`,
            borderTop: 'none', boxShadow: '0 4px 12px rgba(0,0,0,0.4)',
          }}>
            {suggestions.map((s, i) => (
              <div
                key={s.symbol}
                onMouseDown={() => commitSym(s.symbol)}
                style={{
                  display: 'flex', alignItems: 'baseline', gap: 6,
                  padding: '3px 8px', cursor: 'pointer',
                  background: i === suggestIndex ? accent + '22' : 'transparent',
                  borderBottom: i < suggestions.length - 1 ? `1px solid ${border}22` : 'none',
                }}
                onMouseEnter={() => setSuggestIndex(i)}
              >
                <span style={{ color: accent, fontWeight: 'bold', flex: '0 0 52px', fontSize: 13 }}>{s.symbol}</span>
                <span style={{ color: text, opacity: 0.5, fontSize: 11, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                  {s.name}
                </span>
              </div>
            ))}
          </div>
        )}
      </div>

      {/* Controls: strikes ± | DTE selector | select-mode toggle */}
      <div style={{
        display: 'flex', alignItems: 'center', gap: 6,
        padding: '4px 8px', borderBottom: `1px solid ${border}`,
        flexShrink: 0,
      }}>
        <span style={{ color: text, fontSize: 10, opacity: 0.5 }}>strikes</span>
        <button onClick={() => setNumStrikes(numStrikes - 1)} style={smallBtn(theme)}>−</button>
        <span style={{ color: text, fontSize: 12, minWidth: 16, textAlign: 'center' }}>{numStrikes}</span>
        <button onClick={() => setNumStrikes(numStrikes + 1)} style={smallBtn(theme)}>+</button>

        <select
          value={farDTE}
          onChange={e => setFarDTE(Number(e.target.value))}
          style={{
            marginLeft: 'auto',
            background: theme.background, color: text,
            border: `1px solid ${border}`,
            fontSize: 11, fontFamily: 'monospace', padding: '2px 4px', cursor: 'pointer',
          }}
        >
          {dteOptions.map(opt => (
            <option key={opt.value} value={opt.value}>{opt.label}</option>
          ))}
        </select>

        {/* Select-mode toggle */}
        <button
          onClick={() => setSelectMode(m => !m)}
          title={selectMode ? 'Click-to-select ON (click to disable)' : 'Click-to-select OFF (shift+click always works)'}
          style={{
            background: selectMode ? accent + '33' : theme.toolbarBackground,
            border: `1px solid ${selectMode ? accent + '88' : border}`,
            color: selectMode ? accent : text,
            fontSize: 11, fontFamily: 'monospace',
            height: 22, padding: '0 6px',
            cursor: 'pointer', borderRadius: 2, flexShrink: 0,
            fontWeight: selectMode ? 'bold' : 'normal',
          }}
        >
          {selectMode ? '✓ sel' : 'sel'}
        </button>
      </div>

      {/* Column headers */}
      <div style={{
        display: 'flex', alignItems: 'center',
        padding: '2px 6px 2px 8px',
        fontSize: 10, color: text, opacity: 0.35,
        borderBottom: `1px solid ${border}22`,
        userSelect: 'none',
      }}>
        <span style={{ width: 8 }} />
        <span style={{ width: 46, textAlign: 'right' }}>STK</span>
        <span style={{ width: 16 }} />
        <span style={{ width: 40, textAlign: 'right' }}>BID</span>
        <span style={{ width: 40, textAlign: 'right' }}>ASK</span>
        <span style={{ width: 46, textAlign: 'right' }}>OI</span>
        <span style={{ width: 10 }} />
      </div>

      {chain0 && (
        <ExpiryBlock
          chain={chain0} underlying={underlying} symbol={chainSymbol}
          savedKeys={savedKeys} selectMode={selectMode} onToggle={toggleSavedOption}
          bull={bull} bear={bear} accent={accent} text={text} border={border}
        />
      )}

      {chainN && farDTE > 0 && (
        <ExpiryBlock
          chain={chainN} underlying={underlying} symbol={chainSymbol}
          savedKeys={savedKeys} selectMode={selectMode} onToggle={toggleSavedOption}
          bull={bull} bear={bear} accent={accent} text={text} border={border}
        />
      )}

      {!chain0 && (
        <div style={{ padding: 12, color: text, opacity: 0.4, fontSize: 12, textAlign: 'center' }}>
          Loading…
        </div>
      )}
    </div>
  )
}

function smallBtn(theme: ReturnType<typeof getTheme>): React.CSSProperties {
  return {
    background: theme.toolbarBackground,
    border: `1px solid ${theme.toolbarBorder}`,
    color: theme.axisText,
    fontSize: 12, fontFamily: 'monospace',
    width: 20, height: 20,
    display: 'flex', alignItems: 'center', justifyContent: 'center',
    cursor: 'pointer', padding: 0, borderRadius: 2,
  }
}
