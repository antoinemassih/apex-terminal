import { useState, useEffect } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { getTheme } from '../themes'
import { useChartStore } from '../store/chartStore'

interface OptionContract {
  strike: number
  lastPrice: number
  bid: number
  ask: number
  volume: number
  openInterest: number
  impliedVolatility: number
  inTheMoney: boolean
  contractSymbol: string
}

interface OptionsChain {
  expirations: string[]
  date: string | null
  calls: OptionContract[]
  puts: OptionContract[]
}

interface Props {
  symbol: string
  anchorX: number
  anchorY: number
  maxHeight: number
  onClose: () => void
  onSelect: (contractSymbol: string) => void
}

export function OptionsPanel({ symbol, anchorX, anchorY, maxHeight, onClose, onSelect }: Props) {
  const [chain, setChain] = useState<OptionsChain | null>(null)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [dateIdx, setDateIdx] = useState(0)
  const [tab, setTab] = useState<'calls' | 'puts'>('calls')
  const theme = getTheme(useChartStore(s => s.theme))

  const loadChain = async (date?: string) => {
    setLoading(true)
    setError(null)
    try {
      const result = await invoke<OptionsChain>('get_options_chain', { symbol, date: date ?? null })
      setChain(result)
      if (date && result.expirations.length > 0) {
        const idx = result.expirations.indexOf(result.date ?? '')
        if (idx >= 0) setDateIdx(idx)
      }
    } catch (e) {
      setError(String(e))
    }
    setLoading(false)
  }

  useEffect(() => { loadChain() }, [symbol])

  const stepDate = (dir: -1 | 1) => {
    if (!chain) return
    const next = dateIdx + dir
    if (next < 0 || next >= chain.expirations.length) return
    setDateIdx(next)
    loadChain(chain.expirations[next])
  }

  // Close on escape
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => { if (e.key === 'Escape') onClose() }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [onClose])

  const bg = theme.toolbarBackground
  const border = theme.toolbarBorder
  const text = theme.ohlcLabel
  const accent = theme.borderActive
  const dim = theme.axisText

  const contracts = tab === 'calls' ? (chain?.calls ?? []) : (chain?.puts ?? [])

  return (
    <div style={{
      position: 'fixed', left: anchorX, top: anchorY,
      width: 380, height: Math.min(maxHeight, 380),
      background: bg, border: `1px solid ${border}`, borderRadius: 4,
      boxShadow: '0 4px 16px rgba(0,0,0,0.5)',
      zIndex: 10001, fontFamily: 'monospace', fontSize: 10,
      display: 'flex', flexDirection: 'column', overflow: 'hidden',
    }}
    onMouseDown={e => e.stopPropagation()}>

      {/* Header: symbol + date stepper */}
      <div style={{
        display: 'flex', alignItems: 'center', justifyContent: 'space-between',
        padding: '6px 8px', borderBottom: `1px solid ${border}`,
      }}>
        <span style={{ color: accent, fontWeight: 'bold', fontSize: 11 }}>{symbol} Options</span>
        <div style={{ display: 'flex', alignItems: 'center', gap: 4 }}>
          <button onClick={() => stepDate(-1)} disabled={dateIdx <= 0}
            style={{ background: 'none', border: 'none', color: dateIdx > 0 ? text : '#333', cursor: 'pointer', fontSize: 12, padding: '0 4px' }}>
            &#9664;
          </button>
          <span style={{ color: text, fontSize: 10, minWidth: 75, textAlign: 'center' }}>
            {chain?.date ?? '...'}
          </span>
          <button onClick={() => stepDate(1)} disabled={!chain || dateIdx >= chain.expirations.length - 1}
            style={{ background: 'none', border: 'none', color: chain && dateIdx < chain.expirations.length - 1 ? text : '#333', cursor: 'pointer', fontSize: 12, padding: '0 4px' }}>
            &#9654;
          </button>
          <button onClick={onClose} style={{ background: 'none', border: 'none', color: dim, cursor: 'pointer', fontSize: 14, marginLeft: 8 }}>
            &#10005;
          </button>
        </div>
      </div>

      {/* Tabs: Calls / Puts */}
      <div style={{ display: 'flex', borderBottom: `1px solid ${border}` }}>
        {(['calls', 'puts'] as const).map(t => (
          <button key={t} onClick={() => setTab(t)} style={{
            flex: 1, background: tab === t ? accent + '22' : 'transparent',
            color: tab === t ? accent : dim,
            border: 'none', borderBottom: tab === t ? `2px solid ${accent}` : '2px solid transparent',
            padding: '5px 0', cursor: 'pointer', fontFamily: 'monospace', fontSize: 10,
            fontWeight: tab === t ? 'bold' : 'normal',
          }}>
            {t.toUpperCase()} ({tab === t ? contracts.length : (t === 'calls' ? chain?.calls.length ?? 0 : chain?.puts.length ?? 0)})
          </button>
        ))}
      </div>

      {/* Content */}
      {loading && <div style={{ padding: 12, color: dim, textAlign: 'center' }}>Loading options...</div>}
      {error && <div style={{ padding: 12, color: '#e74c3c', textAlign: 'center' }}>{error}</div>}

      {!loading && !error && (
        <div style={{ flex: 1, overflow: 'auto' }}>
          {/* Column headers */}
          <div style={{
            display: 'grid', gridTemplateColumns: '60px 52px 52px 52px 50px 50px 32px',
            padding: '4px 8px', borderBottom: `1px solid ${border}`,
            color: dim, fontSize: 9, position: 'sticky', top: 0, background: bg,
          }}>
            <span>Strike</span>
            <span>Last</span>
            <span>Bid</span>
            <span>Ask</span>
            <span>Vol</span>
            <span>OI</span>
            <span>IV</span>
          </div>

          {contracts.map(c => (
            <ContractRow key={c.contractSymbol} contract={c} accent={accent} text={text} dim={dim} bg={bg}
              onSelect={() => onSelect(c.contractSymbol)} />
          ))}

          {contracts.length === 0 && (
            <div style={{ padding: 12, color: dim, textAlign: 'center' }}>No contracts available</div>
          )}
        </div>
      )}
    </div>
  )
}

function ContractRow({ contract: c, accent, text, dim, bg, onSelect }: {
  contract: OptionContract; accent: string; text: string; dim: string; bg: string; onSelect: () => void
}) {
  const [hover, setHover] = useState(false)
  return (
    <div
      onClick={onSelect}
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => setHover(false)}
      style={{
        display: 'grid', gridTemplateColumns: '60px 52px 52px 52px 50px 50px 32px',
        padding: '3px 8px', cursor: 'pointer',
        background: hover ? accent + '15' : c.inTheMoney ? accent + '08' : 'transparent',
        color: hover ? accent : text,
        borderBottom: `1px solid ${bg}`,
      }}>
      <span style={{ fontWeight: c.inTheMoney ? 'bold' : 'normal' }}>{c.strike.toFixed(1)}</span>
      <span>{c.lastPrice.toFixed(2)}</span>
      <span style={{ color: dim }}>{c.bid.toFixed(2)}</span>
      <span style={{ color: dim }}>{c.ask.toFixed(2)}</span>
      <span>{c.volume > 0 ? c.volume.toLocaleString() : '-'}</span>
      <span style={{ color: dim }}>{c.openInterest > 0 ? c.openInterest.toLocaleString() : '-'}</span>
      <span style={{ color: dim }}>{(c.impliedVolatility * 100).toFixed(0)}%</span>
    </div>
  )
}
