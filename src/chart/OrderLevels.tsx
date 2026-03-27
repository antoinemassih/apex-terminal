import { useState, useEffect, useRef, useCallback, useMemo } from 'react'
import { useOrderStore, isOCO, isTrigger } from '../store/orderStore'
import type { LevelType, OrderLevel } from '../store/orderStore'
import type { CoordSystem } from '../engine'
import { getTheme } from '../themes'

interface Props {
  paneId: string
  cs: CoordSystem
  theme: ReturnType<typeof getTheme>
  onPauseScroll?: () => void
}

interface Meta {
  color: string
  label: string
  icon: string
  dashOn: string
  dashOff: string
}

function getMeta(type: LevelType, t: ReturnType<typeof getTheme>): Meta {
  const bull = t.bull ?? '#26a69a'
  const bear = t.bear ?? '#ef5350'
  switch (type) {
    case 'buy':         return { color: bull,      label: 'BUY',      icon: '▲', dashOn: '8px', dashOff: '4px' }
    case 'sell':        return { color: bear,       label: 'SELL',     icon: '▼', dashOn: '8px', dashOff: '4px' }
    case 'stop':        return { color: '#e05560',  label: 'STOP',     icon: '⊗', dashOn: '4px', dashOff: '4px' }
    case 'oco_target':  return { color: '#a78bfa',  label: 'OCO ↑',    icon: '⇧', dashOn: '6px', dashOff: '4px' }
    case 'oco_stop':    return { color: '#a78bfa',  label: 'OCO ↓',    icon: '⇩', dashOn: '6px', dashOff: '4px' }
    case 'trigger_buy': return { color: bull,       label: 'TRIGGER ▲', icon: '⟲', dashOn: '6px', dashOff: '3px' }
    case 'trigger_sell':return { color: '#f59e0b',  label: 'TARGET',   icon: '◎', dashOn: '6px', dashOff: '3px' }
  }
}

function fmtPrice(p: number): string {
  return p >= 10 ? p.toFixed(2) : p.toFixed(4)
}

export function fireOrder(level: OrderLevel, paneId: string) {
  const { qty, price, type } = level
  switch (type) {
    case 'buy':
      console.info(`[ORDER] BUY LIMIT   ${qty} @ ${fmtPrice(price)}`); break
    case 'sell':
      console.info(`[ORDER] SELL LIMIT  ${qty} @ ${fmtPrice(price)}`); break
    case 'stop':
      console.info(`[ORDER] STOP LOSS   ${qty} @ ${fmtPrice(price)}`); break
    case 'oco_target': {
      const s = useOrderStore.getState().getPane(paneId).levels.find(l => l.type === 'oco_stop')
      if (s) console.info(`[ORDER] OCO BRACKET ${qty} — target @ ${fmtPrice(price)}, stop @ ${fmtPrice(s.price)}`)
      else   console.info(`[ORDER] OCO TARGET  ${qty} @ ${fmtPrice(price)}`)
      break
    }
    case 'oco_stop': break  // always fired with oco_target
    case 'trigger_buy': {
      const s = useOrderStore.getState().getPane(paneId).levels.find(l => l.type === 'trigger_sell')
      if (s) console.info(`[ORDER] TRIGGER     ${qty} — buy @ ${fmtPrice(price)}, target @ ${fmtPrice(s.price)}`)
      else   console.info(`[ORDER] TRIGGER BUY ${qty} @ ${fmtPrice(price)}`)
      break
    }
    case 'trigger_sell': break  // always fired with trigger_buy
  }
}

// ─── Shared button styles ────────────────────────────────────────────────────

const PANEL_W = 460

function actionBtn(color: string): React.CSSProperties {
  return {
    background: color + '33',
    border: `1px solid ${color}`,
    color,
    fontFamily: 'monospace',
    fontSize: 11,
    fontWeight: 'bold',
    padding: '5px 14px',
    borderRadius: 3,
    cursor: 'pointer',
    letterSpacing: 0.3,
    whiteSpace: 'nowrap',
    flexShrink: 0,
  }
}

const cancelBtn: React.CSSProperties = {
  background: 'transparent',
  border: '1px solid #e05560',
  color: '#e05560',
  fontFamily: 'monospace',
  fontSize: 11,
  fontWeight: 'bold',
  padding: '5px 12px',
  borderRadius: 3,
  cursor: 'pointer',
  letterSpacing: 0.3,
  whiteSpace: 'nowrap',
  flexShrink: 0,
}

// ─── Component ───────────────────────────────────────────────────────────────

export function OrderLevels({ paneId, cs, theme, onPauseScroll }: Props) {
  const allLevels  = useOrderStore(s => (s.panes[paneId] ?? { levels: [] }).levels)
  // Only show active (draft/placed) levels on chart — executed/cancelled go to order book
  const levels     = allLevels.filter(l => l.status === 'draft' || l.status === 'placed')
  const removeLevel = useOrderStore(s => s.removeLevel)
  const setLevel   = useOrderStore(s => s.setLevel)
  const placeLevel = useOrderStore(s => s.placeLevel)
  const allToasts  = useOrderStore(s => s.toasts)
  const toasts     = useMemo(() => (allToasts ?? []).filter(t => t.paneId === paneId), [allToasts, paneId])
  const removeToast = useOrderStore(s => s.removeToast)

  // Auto-dismiss toasts after 60 seconds
  useEffect(() => {
    if (toasts.length === 0) return
    const timers = toasts.map(t => {
      const remaining = Math.max(0, 60_000 - (Date.now() - t.timestamp))
      return setTimeout(() => removeToast(t.id), remaining)
    })
    return () => timers.forEach(clearTimeout)
  }, [toasts, removeToast])

  // openLevel: which level type was clicked (paired levels normalize to primary)
  const [openLevel, setOpenLevel] = useState<LevelType | null>(null)

  // Auto-open controls when new level types appear (just placed from context menu)
  const prevTypesRef = useRef('')
  const typesKey = levels.map(l => l.type).sort().join(',')
  useEffect(() => {
    if (typesKey === prevTypesRef.current) return
    const prevSet = new Set(prevTypesRef.current.split(',').filter(Boolean))
    const added = typesKey.split(',').filter(t => t && !prevSet.has(t)) as LevelType[]
    if (added.length > 0) {
      const toOpen = added.includes('oco_target')   ? 'oco_target'
                   : added.includes('trigger_buy')  ? 'trigger_buy'
                   : added[0]
      setOpenLevel(toOpen)
      onPauseScroll?.()
    }
    prevTypesRef.current = typesKey
  }, [typesKey, onPauseScroll])

  const containerRef = useRef<HTMLDivElement>(null)
  const csRef        = useRef(cs)
  csRef.current      = cs

  const grabRef = useRef<{
    type: LevelType; startY: number; moved: boolean; rect: DOMRect
  } | null>(null)

  useEffect(() => {
    const onMove = (e: MouseEvent) => {
      const g = grabRef.current
      if (!g) return
      if (!g.moved && Math.abs(e.clientY - g.startY) > 3) {
        g.moved = true
        setOpenLevel(null)
        document.body.style.cursor = 'ns-resize'
        document.body.style.userSelect = 'none'
      }
      if (!g.moved) return
      const y = e.clientY - g.rect.top
      const price = Math.max(0.0001, csRef.current.yToPrice(y))
      setLevel(paneId, { type: g.type, price })
    }
    const onUp = () => {
      const g = grabRef.current
      if (g && !g.moved) {
        // Normalize paired legs to a single open state
        const canonical = isOCO(g.type) ? 'oco_target'
          : isTrigger(g.type) ? 'trigger_buy'
          : g.type
        setOpenLevel(prev => prev === canonical ? null : canonical)
      }
      grabRef.current = null
      document.body.style.cursor = ''
      document.body.style.userSelect = ''
    }
    document.addEventListener('mousemove', onMove)
    document.addEventListener('mouseup', onUp)
    return () => {
      document.removeEventListener('mousemove', onMove)
      document.removeEventListener('mouseup', onUp)
    }
  }, [paneId, setLevel])

  const startGrab = useCallback((type: LevelType, e: React.MouseEvent) => {
    e.stopPropagation()
    e.preventDefault()
    const rect = containerRef.current?.getBoundingClientRect()
    if (!rect) return
    grabRef.current = { type, startY: e.clientY, moved: false, rect }
    onPauseScroll?.()
  }, [onPauseScroll])

  if (levels.length === 0 && toasts.length === 0) return null

  const chartBottom = cs.height - cs.pb
  const chartWidth  = cs.width - cs.pr

  // Helpers for paired bands
  const ocoTarget  = levels.find(l => l.type === 'oco_target')
  const ocoStop    = levels.find(l => l.type === 'oco_stop')
  const triggerBuy = levels.find(l => l.type === 'trigger_buy')
  const triggerSell= levels.find(l => l.type === 'trigger_sell')

  const panelLeft = Math.max(4, Math.min(chartWidth - PANEL_W - 4, chartWidth / 2 - PANEL_W / 2))

  // ── Controls panel renderer ────────────────────────────────────────────────
  const renderControls = () => {
    if (!openLevel) return null

    // ── OCO combined panel ────────────────────────────────────────────────────
    if (isOCO(openLevel) && ocoTarget && ocoStop) {
      const yA = cs.priceToY(ocoTarget.price)
      const yB = cs.priceToY(ocoStop.price)
      const midY = (yA + yB) / 2
      if (midY < cs.pt || midY > chartBottom) return null
      const isDraft = ocoTarget.status === 'draft'
      // Qty is shared — take from target
      const qty = ocoTarget.qty

      return (
        <div
          style={{
            position: 'absolute',
            top: Math.round(midY) - 36,
            left: panelLeft,
            width: PANEL_W,
            background: theme.toolbarBackground,
            border: `1px solid #a78bfa`,
            borderRadius: 5,
            padding: '8px 14px',
            pointerEvents: 'all',
            zIndex: 20,
            boxShadow: '0 6px 24px rgba(0,0,0,0.7)',
            fontFamily: 'monospace',
          }}
          onMouseDown={e => e.stopPropagation()}
        >
          {/* Header */}
          <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 8 }}>
            <span style={{ color: '#a78bfa', fontWeight: 'bold', fontSize: 11, letterSpacing: 0.5 }}>⇅ OCO BRACKET</span>
            <span style={{ color: theme.axisText, opacity: 0.4, fontSize: 10, flex: 1 }}>
              {isDraft ? 'DRAFT' : '✓ PLACED'}
            </span>
          </div>
          {/* Legs */}
          <div style={{ display: 'flex', gap: 20, marginBottom: 8 }}>
            <div>
              <div style={{ fontSize: 9, color: '#a78bfa', opacity: 0.7, marginBottom: 2, letterSpacing: 0.4 }}>⇧ TARGET</div>
              <div style={{ fontSize: 14, fontWeight: 'bold', color: theme.axisText }}>{fmtPrice(ocoTarget.price)}</div>
            </div>
            <div>
              <div style={{ fontSize: 9, color: '#a78bfa', opacity: 0.7, marginBottom: 2, letterSpacing: 0.4 }}>⇩ STOP</div>
              <div style={{ fontSize: 14, fontWeight: 'bold', color: theme.axisText }}>{fmtPrice(ocoStop.price)}</div>
            </div>
            <div>
              <div style={{ fontSize: 9, color: theme.axisText, opacity: 0.5, marginBottom: 2 }}>spread</div>
              <div style={{ fontSize: 11, color: theme.axisText, opacity: 0.7 }}>
                {fmtPrice(Math.abs(ocoTarget.price - ocoStop.price))}
              </div>
            </div>
            <div style={{ flex: 1 }} />
            {/* Qty */}
            <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
              <span style={{ fontSize: 10, color: theme.axisText, opacity: 0.5 }}>qty</span>
              <input
                type="number" value={qty} min={1}
                onChange={e => {
                  const v = parseInt(e.target.value)
                  if (!isNaN(v) && v > 0) {
                    setLevel(paneId, { type: 'oco_target', price: ocoTarget.price, qty: v })
                    setLevel(paneId, { type: 'oco_stop',   price: ocoStop.price,   qty: v })
                  }
                }}
                style={{
                  background: 'transparent', border: `1px solid ${theme.toolbarBorder}`,
                  color: theme.axisText, fontFamily: 'monospace', fontSize: 13,
                  width: 72, padding: '2px 5px', textAlign: 'right', outline: 'none', borderRadius: 2,
                }}
                // eslint-disable-next-line jsx-a11y/no-autofocus
                autoFocus
              />
            </div>
          </div>
          {/* Actions */}
          <div style={{ display: 'flex', gap: 8, justifyContent: 'flex-end' }}>
            {isDraft && (
              <button style={actionBtn('#a78bfa')} onClick={() => {
                fireOrder(ocoTarget, paneId)
                placeLevel(paneId, 'oco_target')
                setOpenLevel(null)
              }}>⇅ Place OCO Bracket</button>
            )}
            <button style={cancelBtn} onClick={() => { removeLevel(paneId, 'oco_target'); setOpenLevel(null) }}>
              Cancel
            </button>
          </div>
        </div>
      )
    }

    // ── Trigger combined panel ─────────────────────────────────────────────────
    if (isTrigger(openLevel) && triggerBuy && triggerSell) {
      const yBuy = cs.priceToY(triggerBuy.price)
      if (yBuy < cs.pt || yBuy > chartBottom) return null
      const isDraft     = triggerBuy.status === 'draft'
      const isTriggered = !!triggerBuy.triggered
      const qty         = triggerBuy.qty

      return (
        <div
          style={{
            position: 'absolute',
            top: Math.round(yBuy) - 36,
            left: panelLeft,
            width: PANEL_W,
            background: theme.toolbarBackground,
            border: `1px solid ${(theme.bull ?? '#26a69a')}`,
            borderRadius: 5,
            padding: '8px 14px',
            pointerEvents: 'all',
            zIndex: 20,
            boxShadow: '0 6px 24px rgba(0,0,0,0.7)',
            fontFamily: 'monospace',
          }}
          onMouseDown={e => e.stopPropagation()}
        >
          {/* Header */}
          <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 8 }}>
            <span style={{ color: theme.bull ?? '#26a69a', fontWeight: 'bold', fontSize: 11, letterSpacing: 0.5 }}>
              ⟲ TRIGGER ORDER
            </span>
            <span style={{ fontSize: 10, color: theme.axisText, opacity: 0.4, flex: 1 }}>
              {isTriggered ? '✓ BUY FILLED — sell active' : isDraft ? 'DRAFT' : 'LIVE'}
            </span>
          </div>
          {/* Legs */}
          <div style={{ display: 'flex', gap: 20, marginBottom: 8 }}>
            <div>
              <div style={{ fontSize: 9, color: theme.bull ?? '#26a69a', opacity: isTriggered ? 0.45 : 0.8, marginBottom: 2, letterSpacing: 0.4 }}>
                {isTriggered ? '▲ BUY — FILLED' : '▲ BUY ENTRY'}
              </div>
              <div style={{ fontSize: 14, fontWeight: 'bold', color: isTriggered ? (theme.axisText) : (theme.bull ?? '#26a69a'), opacity: isTriggered ? 0.45 : 1 }}>
                {fmtPrice(triggerBuy.price)}
              </div>
            </div>
            <div>
              <div style={{ fontSize: 9, color: '#f59e0b', opacity: 0.8, marginBottom: 2, letterSpacing: 0.4 }}>
                {isTriggered ? '◎ SELL — ACTIVE' : '◎ SELL TARGET'}
              </div>
              <div style={{ fontSize: 14, fontWeight: 'bold', color: '#f59e0b' }}>
                {fmtPrice(triggerSell.price)}
              </div>
            </div>
            <div>
              <div style={{ fontSize: 9, color: theme.axisText, opacity: 0.5, marginBottom: 2 }}>range</div>
              <div style={{ fontSize: 11, color: theme.axisText, opacity: 0.7 }}>
                {fmtPrice(Math.abs(triggerSell.price - triggerBuy.price))}
                <span style={{ opacity: 0.5, marginLeft: 4, fontSize: 9 }}>
                  ({((Math.abs(triggerSell.price - triggerBuy.price) / triggerBuy.price) * 100).toFixed(1)}%)
                </span>
              </div>
            </div>
            <div style={{ flex: 1 }} />
            {/* Qty */}
            <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
              <span style={{ fontSize: 10, color: theme.axisText, opacity: 0.5 }}>qty</span>
              <input
                type="number" value={qty} min={1}
                onChange={e => {
                  const v = parseInt(e.target.value)
                  if (!isNaN(v) && v > 0) {
                    setLevel(paneId, { type: 'trigger_buy',  price: triggerBuy.price,  qty: v })
                    setLevel(paneId, { type: 'trigger_sell', price: triggerSell.price, qty: v })
                  }
                }}
                style={{
                  background: 'transparent', border: `1px solid ${theme.toolbarBorder}`,
                  color: theme.axisText, fontFamily: 'monospace', fontSize: 13,
                  width: 72, padding: '2px 5px', textAlign: 'right', outline: 'none', borderRadius: 2,
                }}
                // eslint-disable-next-line jsx-a11y/no-autofocus
                autoFocus={!isTriggered}
              />
            </div>
          </div>
          {/* Actions */}
          <div style={{ display: 'flex', gap: 8, justifyContent: 'flex-end' }}>
            {isDraft && !isTriggered && (
              <button style={actionBtn(theme.bull ?? '#26a69a')} onClick={() => {
                fireOrder(triggerBuy, paneId)
                placeLevel(paneId, 'trigger_buy')
                setOpenLevel(null)
              }}>⟲ Place Trigger Order</button>
            )}
            <button style={cancelBtn} onClick={() => { removeLevel(paneId, 'trigger_buy'); setOpenLevel(null) }}>
              Cancel
            </button>
          </div>
        </div>
      )
    }

    // ── Single level panel ─────────────────────────────────────────────────────
    const level = levels.find(l => l.type === openLevel)
    if (!level) return null
    const y = cs.priceToY(level.price)
    if (y < cs.pt - 2 || y > chartBottom + 2) return null
    const { color, label, icon } = getMeta(openLevel, theme)
    const isDraft = level.status === 'draft'

    return (
      <div
        style={{
          position: 'absolute',
          top: Math.round(y) - 18,
          left: panelLeft,
          width: PANEL_W,
          display: 'flex',
          alignItems: 'center',
          gap: 12,
          background: theme.toolbarBackground,
          border: `1px solid ${color}`,
          borderRadius: 5,
          padding: '6px 14px',
          pointerEvents: 'all',
          zIndex: 20,
          boxShadow: '0 6px 24px rgba(0,0,0,0.7)',
        }}
        onMouseDown={e => e.stopPropagation()}
      >
        <div style={{ display: 'flex', flexDirection: 'column', minWidth: 0 }}>
          <span style={{ fontFamily: 'monospace', fontSize: 10, color, fontWeight: 'bold', letterSpacing: 0.5, whiteSpace: 'nowrap' }}>
            {icon} {label}
          </span>
          <span style={{ fontFamily: 'monospace', fontSize: 14, color: theme.axisText, fontWeight: 'bold', whiteSpace: 'nowrap' }}>
            {fmtPrice(level.price)}
          </span>
        </div>
        <div style={{ width: 1, height: 32, background: theme.toolbarBorder, flexShrink: 0 }} />
        <span style={{ fontFamily: 'monospace', fontSize: 10, color: theme.axisText, opacity: 0.5, flexShrink: 0 }}>qty</span>
        <input
          type="number" value={level.qty} min={1}
          onChange={e => {
            const v = parseInt(e.target.value)
            if (!isNaN(v) && v > 0) setLevel(paneId, { type: level.type, price: level.price, qty: v })
          }}
          style={{
            background: 'transparent', border: `1px solid ${theme.toolbarBorder}`,
            color: theme.axisText, fontFamily: 'monospace', fontSize: 13,
            width: 72, padding: '3px 5px', textAlign: 'right', outline: 'none', borderRadius: 2, flexShrink: 0,
          }}
          // eslint-disable-next-line jsx-a11y/no-autofocus
          autoFocus
        />
        <div style={{ flex: 1 }} />
        {isDraft && (
          <button style={actionBtn(color)} onClick={() => {
            fireOrder(level, paneId)
            placeLevel(paneId, level.type)
            setOpenLevel(null)
          }}>
            {icon} Place {label}
          </button>
        )}
        <button style={cancelBtn} onClick={() => { removeLevel(paneId, level.type); setOpenLevel(null) }}>
          Cancel
        </button>
      </div>
    )
  }

  // ─────────────────────────────────────────────────────────────────────────────

  return (
    <div ref={containerRef} style={{ position: 'absolute', inset: 0, pointerEvents: 'none', zIndex: 10 }}>

      {/* OCO band */}
      {ocoTarget && ocoStop && (() => {
        const yA = cs.priceToY(ocoTarget.price)
        const yB = cs.priceToY(ocoStop.price)
        const top    = Math.max(cs.pt, Math.min(yA, yB))
        const bottom = Math.min(chartBottom, Math.max(yA, yB))
        if (bottom <= top) return null
        return <div style={{ position: 'absolute', top, left: 0, right: 0, height: bottom - top, background: '#a78bfa0d', pointerEvents: 'none', zIndex: 8 }} />
      })()}

      {/* Trigger band */}
      {triggerBuy && triggerSell && (() => {
        const yA = cs.priceToY(triggerBuy.price)
        const yB = cs.priceToY(triggerSell.price)
        const top    = Math.max(cs.pt, Math.min(yA, yB))
        const bottom = Math.min(chartBottom, Math.max(yA, yB))
        if (bottom <= top) return null
        return <div style={{ position: 'absolute', top, left: 0, right: 0, height: bottom - top, background: '#26a69a08', pointerEvents: 'none', zIndex: 8 }} />
      })()}

      {/* Level lines */}
      {levels.map(level => {
        const y = cs.priceToY(level.price)
        if (y < cs.pt - 2 || y > chartBottom + 2) return null
        const { color, label, icon, dashOn, dashOff } = getMeta(level.type, theme)
        const lineY = Math.round(y)
        const isFilled = level.type === 'trigger_buy' && level.triggered
        const opacity  = (level.status === 'placed' && !isTrigger(level.type)) ? 0.55
          : isFilled ? 0.35
          : 1

        return (
          <div key={level.type} style={{ position: 'absolute', inset: 0, pointerEvents: 'none' }}>

            {/* Dashed line */}
            <div style={{
              position: 'absolute',
              top: lineY - 0.5,
              left: 0,
              right: cs.pr,
              height: 1,
              opacity,
              backgroundImage: isFilled
                ? `none`
                : `repeating-linear-gradient(to right, ${color}cc 0, ${color}cc ${dashOn}, transparent ${dashOn}, transparent calc(${dashOn} + ${dashOff}))`,
              background: isFilled ? color + '44' : undefined,
              pointerEvents: 'none',
            }} />

            {/* Grab zone */}
            <div
              style={{
                position: 'absolute', top: lineY - 7, left: 0, right: cs.pr,
                height: 14, cursor: 'ns-resize', pointerEvents: 'all',
              }}
              onMouseDown={e => startGrab(level.type, e)}
            />

            {/* Y-axis badge */}
            <div
              style={{ position: 'absolute', top: lineY - 18, right: 2, pointerEvents: 'all', cursor: 'ns-resize', opacity }}
              onMouseDown={e => startGrab(level.type, e)}
            >
              <div style={{
                background: theme.background,
                border: `2px solid ${color}`,
                borderRadius: 3,
                padding: '3px 6px 3px 5px',
                fontFamily: 'monospace',
                lineHeight: 1.35,
                whiteSpace: 'nowrap',
              }}>
                <div style={{ fontSize: 10, fontWeight: 'bold', color, letterSpacing: 0.5 }}>{icon} {label}</div>
                <div style={{ fontSize: 12, fontWeight: 'bold', color: theme.axisText }}>{fmtPrice(level.price)}</div>
                <div style={{ display: 'flex', alignItems: 'center', gap: 4 }}>
                  <span style={{ fontSize: 10, color: theme.axisText, opacity: 0.65 }}>×{level.qty}</span>
                  {level.status === 'placed' && !isFilled && <span style={{ fontSize: 8, color: '#26a69a', fontWeight: 'bold' }}>✓</span>}
                  {isFilled && <span style={{ fontSize: 8, color: '#f59e0b', fontWeight: 'bold' }}>FILLED</span>}
                </div>
              </div>
              {/* Remove button — outside drag area */}
              <button
                onClick={e => { e.stopPropagation(); removeLevel(paneId, level.type) }}
                onMouseDown={e => e.stopPropagation()}
                style={{
                  position: 'absolute', top: 0, right: -15,
                  background: 'none', border: 'none', color: color + '77',
                  fontFamily: 'monospace', fontSize: 13, width: 14,
                  padding: 0, cursor: 'pointer', lineHeight: 1,
                }}
              >×</button>
            </div>

          </div>
        )
      })}

      {/* Inline controls — rendered once, outside the levels loop to avoid duplicates */}
      {renderControls()}

      {/* Order toasts — confirmation badges near price level */}
      {toasts.map(toast => {
        const y = cs.priceToY(toast.price)
        if (y < cs.pt - 20 || y > chartBottom + 20) return null
        const isExec = toast.action === 'executed'
        const color = isExec ? '#26a69a' : '#f59e0b'
        const { icon } = getMeta(toast.type, theme)
        return (
          <div
            key={toast.id}
            style={{
              position: 'absolute',
              top: Math.round(y) - 32,
              left: Math.max(4, chartWidth / 2 - 120),
              pointerEvents: 'all',
              zIndex: 25,
              display: 'flex', alignItems: 'center', gap: 8,
              background: theme.toolbarBackground,
              border: `1px solid ${color}88`,
              borderLeft: `3px solid ${color}`,
              borderRadius: 4,
              padding: '6px 10px',
              boxShadow: '0 4px 16px rgba(0,0,0,0.5)',
              fontFamily: 'monospace',
              animation: 'fadeIn 0.2s ease-out',
            }}
            onMouseDown={e => e.stopPropagation()}
          >
            <span style={{ fontSize: 13, color }}>{isExec ? '✓' : '✕'}</span>
            <div>
              <div style={{ fontSize: 10, fontWeight: 'bold', color, letterSpacing: 0.4 }}>
                {icon} {toast.type.toUpperCase().replace('_', ' ')} {isExec ? 'EXECUTED' : 'CANCELLED'}
              </div>
              <div style={{ fontSize: 11, color: theme.axisText }}>
                {fmtPrice(toast.price)} x{toast.qty}
              </div>
            </div>
            <button
              onClick={() => removeToast(toast.id)}
              style={{
                background: 'none', border: 'none', color: theme.axisText,
                opacity: 0.4, fontSize: 12, cursor: 'pointer', padding: '0 2px',
                fontFamily: 'monospace', marginLeft: 4,
              }}
            >x</button>
          </div>
        )
      })}

    </div>
  )
}
