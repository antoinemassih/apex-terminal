import { useState, useEffect } from 'react'
import { useWatchlistStore } from '../store/watchlistStore'
import { useChartStore } from '../store/chartStore'
import { getTheme } from '../themes'
import { priceOption } from '../data/optionsSim'

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

interface LiveRow {
  key: string
  symbol: string
  strike: number
  type: 'call' | 'put'
  dte: number
  expiry: string
  bid: number
  ask: number
  mid: number
  oi: number
}

export function OptionsWatchlist() {
  const { savedOptions, removeSavedOption } = useWatchlistStore()
  const { theme: themeName } = useChartStore()
  const theme = getTheme(themeName)

  const [rows, setRows] = useState<LiveRow[]>([])

  useEffect(() => {
    const compute = () => {
      const storeState = useWatchlistStore.getState()
      const next: LiveRow[] = storeState.savedOptions.map(opt => {
        const underlying = storeState.prices[opt.symbol]?.price ?? 100
        const priced = priceOption(underlying, opt.strike, opt.type, opt.dte)
        return { key: opt.key, symbol: opt.symbol, strike: opt.strike, type: opt.type, dte: opt.dte, expiry: opt.expiry, ...priced }
      })
      setRows(next)
    }

    compute()
    const id = setInterval(compute, 500)
    return () => clearInterval(id)
  }, [savedOptions])

  const border = theme.toolbarBorder
  const text   = theme.axisText
  const accent = theme.borderActive
  const bull   = theme.bull ?? '#26a69a'
  const bear   = theme.bear ?? '#ef5350'

  if (rows.length === 0) {
    return (
      <div style={{ flex: 1, display: 'flex', alignItems: 'center', justifyContent: 'center', flexDirection: 'column', gap: 8 }}>
        <div style={{ color: text, opacity: 0.3, fontSize: 12, textAlign: 'center', padding: '0 16px' }}>
          No saved options
        </div>
        <div style={{ color: text, opacity: 0.2, fontSize: 11, textAlign: 'center', padding: '0 16px' }}>
          Shift+click any option in the chain to add it here
        </div>
      </div>
    )
  }

  return (
    <div style={{ flex: 1, overflowY: 'auto' }}>
      {/* Column header */}
      <div style={{
        display: 'flex', alignItems: 'center',
        padding: '2px 6px 2px 8px',
        fontSize: 9, color: text, opacity: 0.35,
        borderBottom: `1px solid ${border}22`,
      }}>
        <span style={{ flex: 1 }}>OPTION</span>
        <span style={{ width: 38, textAlign: 'right' }}>BID</span>
        <span style={{ width: 38, textAlign: 'right' }}>ASK</span>
        <span style={{ width: 44, textAlign: 'right' }}>OI</span>
        <span style={{ width: 18 }} />
      </div>

      {rows.map(r => {
        const isCall = r.type === 'call'
        const label = `${r.symbol} ${r.strike}${isCall ? 'C' : 'P'}`
        const dteLabel = `${r.dte}DTE`

        return (
          <div
            key={r.key}
            style={{
              display: 'flex', alignItems: 'center',
              padding: '3px 6px 3px 8px',
              borderBottom: `1px solid ${border}22`,
              fontSize: 13,
            }}
            onMouseEnter={e => (e.currentTarget as HTMLElement).style.background = accent + '0e'}
            onMouseLeave={e => (e.currentTarget as HTMLElement).style.background = 'transparent'}
          >
            <div style={{ flex: 1, minWidth: 0 }}>
              <div style={{ color: isCall ? bull : bear, fontWeight: 'bold', fontSize: 13, whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>
                {label}
              </div>
              <div style={{ color: text, opacity: 0.4, fontSize: 10 }}>{dteLabel}</div>
            </div>
            <span style={{ width: 38, textAlign: 'right', color: text, opacity: 0.75 }}>
              {fmtPrice(r.bid)}
            </span>
            <span style={{ width: 38, textAlign: 'right', color: text }}>
              {fmtPrice(r.ask)}
            </span>
            <span style={{ width: 44, textAlign: 'right', color: text, opacity: 0.6, fontSize: 12 }}>
              {fmtOI(r.oi)}
            </span>
            <button
              onClick={() => removeSavedOption(r.key)}
              style={{
                width: 18, background: 'none', border: 'none',
                color: text, opacity: 0.25, cursor: 'pointer',
                fontSize: 10, padding: 0, flexShrink: 0,
              }}
              onMouseEnter={e => (e.currentTarget as HTMLElement).style.opacity = '1'}
              onMouseLeave={e => (e.currentTarget as HTMLElement).style.opacity = '0.25'}
            >✕</button>
          </div>
        )
      })}
    </div>
  )
}

