import { useState, useEffect, useRef, useCallback } from 'react'
import { useOrderStore } from '../store/orderStore'
import type { LevelType } from '../store/orderStore'
import { getDataStore } from '../globals'
import { getTheme } from '../themes'
import type { CoordSystem } from '../engine'

interface Props {
  paneId: string
  symbol: string
  timeframe: string
  cs: CoordSystem
  theme: ReturnType<typeof getTheme>
}

// Simulate a tight bid/ask spread from last price
function getBidAsk(lastPrice: number): { bid: number; ask: number } {
  const halfSpread = Math.max(0.01, lastPrice * 0.0001)
  return {
    bid: parseFloat((lastPrice - halfSpread).toFixed(2)),
    ask: parseFloat((lastPrice + halfSpread).toFixed(2)),
  }
}

function fmtPrice(p: number): string {
  if (p >= 1000) return p.toFixed(2)
  if (p >= 10)   return p.toFixed(2)
  return p.toFixed(4)
}

// Qty step sizes
function qtyStep(qty: number): number {
  if (qty >= 1000) return 100
  if (qty >= 100)  return 10
  if (qty >= 10)   return 5
  return 1
}

export function OrderEntry({ paneId, symbol, timeframe, cs, theme }: Props) {
  const { getPane, setQty, setLimitPrice, setOrderType, setLevel, addToast } = useOrderStore()
  const pane = getPane(paneId)

  const [lastPrice, setLastPrice] = useState<number | null>(null)
  const [flash, setFlash] = useState<'buy' | 'sell' | null>(null)
  const flashTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  // Pull live price from DataStore
  useEffect(() => {
    const ds = getDataStore()
    const read = () => {
      const data = ds.getData(symbol, timeframe)
      if (data && data.length > 0) setLastPrice(data.closes[data.length - 1])
    }
    read()
    const unsub = ds.subscribe(symbol, timeframe, read)
    return unsub
  }, [symbol, timeframe])

  // Initialize limitPrice to live price once when it first becomes available (or when reset to null)
  const paneRef = useRef(pane)
  paneRef.current = pane
  useEffect(() => {
    if (lastPrice !== null && paneRef.current.limitPrice === null) {
      setLimitPrice(paneId, lastPrice)
    }
  }, [lastPrice, paneId, setLimitPrice])

  const triggerFlash = useCallback((side: 'buy' | 'sell') => {
    setFlash(side)
    if (flashTimerRef.current) clearTimeout(flashTimerRef.current)
    flashTimerRef.current = setTimeout(() => setFlash(null), 600)
  }, [])

  const submitOrder = useCallback((side: 'buy' | 'sell') => {
    if (lastPrice == null) return
    // Read fresh from store to avoid stale closure on qty/price
    const fresh = useOrderStore.getState().getPane(paneId)
    const isLimit = fresh.orderType === 'limit' && fresh.limitPrice !== null
    const price = isLimit ? fresh.limitPrice! : lastPrice
    const type: LevelType = side
    const qty = fresh.qty

    if (isLimit) {
      setLevel(paneId, { type, price, qty, status: 'placed' })
      console.info(`[ORDER] ${side.toUpperCase()} LIMIT ${qty} ${symbol} @ ${fmtPrice(price)}`)
    } else {
      console.info(`[ORDER] ${side.toUpperCase()} MKT ${qty} ${symbol} @ ${fmtPrice(price)}`)
      addToast({ paneId, type, action: 'executed', price, qty })
    }
    triggerFlash(side)
  }, [lastPrice, symbol, paneId, setLevel, addToast, triggerFlash])

  const handleBuy  = useCallback(() => submitOrder('buy'),  [submitOrder])
  const handleSell = useCallback(() => submitOrder('sell'), [submitOrder])

  useEffect(() => () => { if (flashTimerRef.current) clearTimeout(flashTimerRef.current) }, [])

  if (lastPrice == null) return null

  const { bid, ask } = getBidAsk(lastPrice)
  const limitDisplay = pane.limitPrice ?? lastPrice

  const t = theme
  const bull = t.bull ?? '#26a69a'
  const bear = t.bear ?? '#ef5350'
  const bgBase = t.background
  // blend: semi-transparent panel sitting on the chart
  const panelBg = bgBase + 'cc'  // hex color + alpha ~80%

  const inputStyle: React.CSSProperties = {
    background: 'transparent',
    border: `1px solid ${t.toolbarBorder}55`,
    color: t.axisText,
    fontFamily: 'monospace',
    fontSize: 13,
    width: 68,
    padding: '2px 5px',
    textAlign: 'right',
    outline: 'none',
    borderRadius: 2,
  }

  const labelStyle: React.CSSProperties = {
    color: t.axisText, opacity: 0.45, fontSize: 11, letterSpacing: 0.3,
  }

  const nudgeBtn: React.CSSProperties = {
    background: 'none',
    border: `1px solid ${t.toolbarBorder}44`,
    color: t.axisText,
    fontFamily: 'monospace',
    fontSize: 13,
    width: 20, height: 20,
    cursor: 'pointer',
    padding: 0, borderRadius: 2,
    display: 'flex', alignItems: 'center', justifyContent: 'center',
    flexShrink: 0,
  }

  return (
    <div
      style={{
        position: 'absolute',
        bottom: cs.pb + 6,
        left: 8,
        background: panelBg,
        border: `1px solid ${t.toolbarBorder}44`,
        borderRadius: 4,
        padding: '6px 8px',
        fontFamily: 'monospace',
        userSelect: 'none',
        backdropFilter: 'blur(4px)',
        WebkitBackdropFilter: 'blur(4px)',
        zIndex: 20,
        minWidth: 230,
        pointerEvents: 'all',
      }}
      // Prevent mouse events from propagating to chart pan/zoom handlers
      onMouseDown={e => e.stopPropagation()}
      onMouseMove={e => e.stopPropagation()}
      onWheel={e => e.stopPropagation()}
    >
      {/* Row 1: Qty + Limit price + order type */}
      <div style={{ display: 'flex', alignItems: 'center', gap: 6, marginBottom: 5 }}>
        {/* Qty stepper */}
        <span style={labelStyle}>QTY</span>
        <button style={nudgeBtn} onClick={() => setQty(paneId, pane.qty - qtyStep(pane.qty))}>−</button>
        <input
          style={{ ...inputStyle, width: 52 }}
          type="number"
          value={pane.qty}
          min={1}
          onChange={e => { const v = parseInt(e.target.value); if (!isNaN(v)) setQty(paneId, v) }}
        />
        <button style={nudgeBtn} onClick={() => setQty(paneId, pane.qty + qtyStep(pane.qty))}>+</button>

        {/* Divider */}
        <span style={{ width: 1, height: 14, background: t.toolbarBorder + '55', flexShrink: 0 }} />

        {/* Limit price */}
        <span style={labelStyle}>LMT</span>
        <input
          style={{
            ...inputStyle,
            color: pane.orderType === 'market' ? t.axisText + '55' : t.axisText,
          }}
          type="number"
          step="0.01"
          value={limitDisplay.toFixed(2)}
          onChange={e => {
            const v = parseFloat(e.target.value)
            if (!isNaN(v)) { setLimitPrice(paneId, v); setOrderType(paneId, 'limit') }
          }}
          onFocus={() => { if (pane.orderType === 'market') setOrderType(paneId, 'limit') }}
        />

        {/* Market / Limit toggle */}
        <button
          onClick={() => {
            if (pane.orderType === 'limit') { setOrderType(paneId, 'market'); setLimitPrice(paneId, null) }
            else setOrderType(paneId, 'limit')
          }}
          style={{
            background: pane.orderType === 'limit' ? t.borderActive + '33' : 'none',
            border: `1px solid ${pane.orderType === 'limit' ? t.borderActive + '88' : t.toolbarBorder + '55'}`,
            color: pane.orderType === 'limit' ? t.borderActive : t.axisText,
            fontFamily: 'monospace', fontSize: 11,
            padding: '2px 6px', cursor: 'pointer', borderRadius: 2, flexShrink: 0,
          }}
        >
          {pane.orderType === 'market' ? 'MKT' : 'LMT'}
        </button>
      </div>

      {/* Row 2: Sell + Buy buttons */}
      <div style={{ display: 'flex', gap: 6 }}>
        <button
          onClick={handleSell}
          style={{
            flex: 1,
            background: flash === 'sell' ? bear + 'cc' : bear + '22',
            border: `1px solid ${bear}55`,
            color: flash === 'sell' ? '#fff' : bear,
            fontFamily: 'monospace', fontSize: 13, fontWeight: 'bold',
            padding: '5px 0', cursor: 'pointer', borderRadius: 3,
            transition: 'background 0.15s, color 0.15s',
            letterSpacing: 0.3,
          }}
        >
          ▼ SELL  {fmtPrice(bid)}
        </button>
        <button
          onClick={handleBuy}
          style={{
            flex: 1,
            background: flash === 'buy' ? bull + 'cc' : bull + '22',
            border: `1px solid ${bull}55`,
            color: flash === 'buy' ? '#fff' : bull,
            fontFamily: 'monospace', fontSize: 13, fontWeight: 'bold',
            padding: '5px 0', cursor: 'pointer', borderRadius: 3,
            transition: 'background 0.15s, color 0.15s',
            letterSpacing: 0.3,
          }}
        >
          ▲ BUY  {fmtPrice(ask)}
        </button>
      </div>
    </div>
  )
}
