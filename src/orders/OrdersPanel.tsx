import { useEffect, useRef, useMemo } from 'react'
import { useOrderStore, isOCO, isTrigger } from '../store/orderStore'
import type { LevelType, OrderLevel } from '../store/orderStore'
import { useChartStore } from '../store/chartStore'
import { getTheme } from '../themes'
import { fireOrder } from '../chart/OrderLevels'
import { getDataStore } from '../globals'

// ─── Types ───────────────────────────────────────────────────────────────────

interface Base { paneId: string; symbol: string; timeframe: string }
interface SingleGroup extends Base { kind: 'single'; level: OrderLevel }
interface OCOGroup   extends Base { kind: 'oco';     target: OrderLevel; stop: OrderLevel }
interface TrigGroup  extends Base { kind: 'trigger'; buy: OrderLevel;    sell: OrderLevel }
type OrderGroup = SingleGroup | OCOGroup | TrigGroup

interface FlatOrder extends Base { level: OrderLevel }

function groupOrders(flat: FlatOrder[]): OrderGroup[] {
  const groups: OrderGroup[] = []
  const seen = new Set<string>()

  flat.forEach(order => {
    const { paneId, level } = order
    const key = `${paneId}:${level.type}`
    if (seen.has(key)) return

    if (isOCO(level.type)) {
      const pairType = level.type === 'oco_target' ? 'oco_stop' : 'oco_target' as LevelType
      const pair = flat.find(o => o.paneId === paneId && o.level.type === pairType)
      if (pair && !seen.has(`${paneId}:${pairType}`)) {
        const [tgt, stp] = level.type === 'oco_target'
          ? [level, pair.level]
          : [pair.level, level]
        groups.push({ kind: 'oco', paneId, symbol: order.symbol, timeframe: order.timeframe, target: tgt, stop: stp })
        seen.add(`${paneId}:oco_target`)
        seen.add(`${paneId}:oco_stop`)
        return
      }
    }

    if (isTrigger(level.type)) {
      const pairType = level.type === 'trigger_buy' ? 'trigger_sell' : 'trigger_buy' as LevelType
      const pair = flat.find(o => o.paneId === paneId && o.level.type === pairType)
      if (pair && !seen.has(`${paneId}:${pairType}`)) {
        const [buy, sell] = level.type === 'trigger_buy'
          ? [level, pair.level]
          : [pair.level, level]
        groups.push({ kind: 'trigger', paneId, symbol: order.symbol, timeframe: order.timeframe, buy, sell })
        seen.add(`${paneId}:trigger_buy`)
        seen.add(`${paneId}:trigger_sell`)
        return
      }
    }

    groups.push({ kind: 'single', paneId, symbol: order.symbol, timeframe: order.timeframe, level })
    seen.add(key)
  })

  return groups
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

function fmtPrice(p: number): string {
  return p >= 10 ? p.toFixed(2) : p.toFixed(4)
}

function fmtNotional(price: number, qty: number): string {
  const n = price * qty
  if (n >= 1_000_000) return `$${(n / 1_000_000).toFixed(2)}M`
  if (n >= 1_000)     return `$${(n / 1_000).toFixed(1)}K`
  return `$${n.toFixed(2)}`
}

const TYPE_COLOR: Partial<Record<LevelType, string>> = {
  buy: '#26a69a', sell: '#ef5350', stop: '#e05560',
  oco_target: '#a78bfa', oco_stop: '#a78bfa',
  trigger_buy: '#26a69a', trigger_sell: '#f59e0b',
}
const TYPE_ICON: Partial<Record<LevelType, string>> = {
  buy: '▲', sell: '▼', stop: '⊗',
  oco_target: '⇧', oco_stop: '⇩',
  trigger_buy: '⟲', trigger_sell: '◎',
}

// ─── Component ───────────────────────────────────────────────────────────────

export function OrdersPanel() {
  const {
    ordersOpen, panes, filter, setFilter,
    placeLevel, cancelLevel, removeLevel, clearAllLevels, placeAllDrafts, clearHistory,
  } = useOrderStore()
  const { theme: themeName } = useChartStore()
  const theme = getTheme(themeName)

  // Auto-trigger: monitor prices and fire trigger_buy when price crosses entry
  const prevPricesRef = useRef<Record<string, number>>({})
  useEffect(() => {
    const id = setInterval(() => {
      try {
        const { panes: latestPanes, triggerBuy: doTrigger } = useOrderStore.getState()
        const ds = getDataStore()
        Object.entries(latestPanes).forEach(([paneId, pane]) => {
          const parts = paneId.split(':')
          if (parts.length !== 2) return
          const [sym, tf] = parts
          const data = ds.getData(sym, tf)
          if (!data || data.length === 0 || !data.closes) return
          const last = data.closes[data.length - 1]
          const prev = prevPricesRef.current[paneId] ?? last
          prevPricesRef.current[paneId] = last

          pane.levels.forEach(level => {
            if (level.type !== 'trigger_buy' || level.status !== 'placed' || level.triggered) return
            const p = level.price
            const crossedUp   = prev < p && last >= p
            const crossedDown = prev > p && last <= p
            if (crossedUp || crossedDown) {
              const sell = pane.levels.find(l => l.type === 'trigger_sell')
              console.info(
                `[ORDER] TRIGGER FILL — BUY ${level.qty} @ ${fmtPrice(p)} (auto-triggered)` +
                (sell ? ` — sell active @ ${fmtPrice(sell.price)}` : '')
              )
              doTrigger(paneId)
            }
          })
        })
      } catch (e) {
        // Globals may not be ready yet during bootstrap
        if (!(e instanceof Error && e.message.includes('not initialized'))) {
          console.warn('[OrdersPanel] trigger monitor error:', e)
        }
      }
    }, 250)
    return () => clearInterval(id)
  }, []) // intentionally empty — reads from store directly

  // Flatten all levels — memoized to avoid recomputing on every render
  const flat: FlatOrder[] = useMemo(() =>
    Object.entries(panes).flatMap(([paneId, pane]) => {
      const parts = paneId.split(':')
      if (parts.length !== 2) return []
      const [symbol, timeframe] = parts
      return pane.levels.map(level => ({ paneId, symbol, timeframe, level }))
    }),
  [panes])

  const allGroups = useMemo(() => groupOrders(flat), [flat])

  // Filter groups based on active filter
  const groups = allGroups.filter(g => {
    const status = g.kind === 'single' ? g.level.status : g.kind === 'oco' ? g.target.status : g.buy.status
    if (filter === 'all') return true
    if (filter === 'active') return status === 'draft' || status === 'placed'
    return status === filter
  })

  const draftCount = allGroups.filter(g => {
    if (g.kind === 'single')  return g.level.status === 'draft'
    if (g.kind === 'oco')     return g.target.status === 'draft'
    return g.buy.status === 'draft'
  }).length

  const execCount = allGroups.filter(g => {
    const s = g.kind === 'single' ? g.level.status : g.kind === 'oco' ? g.target.status : g.buy.status
    return s === 'executed'
  }).length

  const cancelCount = allGroups.filter(g => {
    const s = g.kind === 'single' ? g.level.status : g.kind === 'oco' ? g.target.status : g.buy.status
    return s === 'cancelled'
  }).length

  const t      = theme
  const bg     = t.toolbarBackground
  const border = t.toolbarBorder
  const text   = t.axisText
  const accent = t.borderActive ?? '#4fc3f7'

  // Shared card container style
  const card = (accentColor: string, isDraft: boolean): React.CSSProperties => ({
    background: t.background,
    border: `1px solid ${isDraft ? accentColor + '66' : border}`,
    borderLeft: `3px solid ${accentColor}`,
    borderRadius: 4,
    padding: '7px 8px 6px',
    opacity: isDraft ? 1 : 0.65,
  })

  const placeBtn = (color: string): React.CSSProperties => ({
    width: '100%',
    background: color + '22',
    border: `1px solid ${color}88`,
    color,
    fontFamily: 'monospace',
    fontSize: 11,
    fontWeight: 'bold',
    padding: '4px 0',
    borderRadius: 3,
    cursor: 'pointer',
    letterSpacing: 0.3,
    marginTop: 6,
  })

  const removeBtn: React.CSSProperties = {
    background: 'none', border: 'none', color: text,
    opacity: 0.4, fontSize: 14, cursor: 'pointer',
    padding: '0 2px', lineHeight: 1, fontFamily: 'monospace',
  }

  const statusBadge = (status: string, isTriggered?: boolean): React.CSSProperties => {
    const c = isTriggered ? '#f59e0b'
      : status === 'draft' ? '#f59e0b'
      : status === 'executed' ? '#26a69a'
      : status === 'cancelled' ? '#ef5350'
      : '#26a69a'
    return {
      fontSize: 9, fontWeight: 'bold', letterSpacing: 0.5,
      color: c, background: c + '18', border: `1px solid ${c}44`,
      borderRadius: 2, padding: '1px 4px',
    }
  }

  return (
    <div style={{
      width: 270, height: '100%', background: bg,
      borderLeft: `1px solid ${border}`,
      display: ordersOpen ? 'flex' : 'none', flexDirection: 'column',
      fontFamily: 'monospace', flexShrink: 0, overflow: 'hidden',
    }}>

      {/* ── Header ── */}
      <div style={{ padding: '8px 10px 6px', borderBottom: `1px solid ${border}`, display: 'flex', flexDirection: 'column', gap: 6 }}>
        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
          <span style={{ color: accent, fontWeight: 'bold', fontSize: 11, letterSpacing: 0.8 }}>ORDERS</span>
          <span style={{ fontSize: 10, color: text, opacity: 0.5 }}>
            {draftCount}d · {allGroups.length - draftCount - execCount - cancelCount}p · {execCount}e · {cancelCount}c
          </span>
        </div>

        {/* Filter bar */}
        <div style={{ display: 'flex', gap: 2 }}>
          {([['all', 'All'], ['active', 'Active'], ['executed', 'Exec'], ['cancelled', 'Cxl']] as const).map(([key, label]) => (
            <button
              key={key}
              onClick={() => setFilter(key)}
              style={{
                flex: 1,
                background: filter === key ? accent + '22' : 'transparent',
                border: `1px solid ${filter === key ? accent + '66' : border + '44'}`,
                borderBottom: filter === key ? `2px solid ${accent}` : '2px solid transparent',
                color: filter === key ? accent : text,
                fontFamily: 'monospace', fontSize: 9,
                padding: '2px 0', borderRadius: 2,
                cursor: 'pointer', letterSpacing: 0.3,
              }}
            >{label}</button>
          ))}
        </div>

        {allGroups.length > 0 && (
          <div style={{ display: 'flex', gap: 6 }}>
            <button
              disabled={draftCount === 0}
              onClick={() => {
                Object.entries(panes).forEach(([paneId, pane]) => {
                  pane.levels.filter(l => l.status === 'draft' && l.type !== 'oco_stop' && l.type !== 'trigger_sell')
                    .forEach(level => fireOrder(level, paneId))
                })
                placeAllDrafts()
              }}
              style={{
                flex: 1,
                background: draftCount > 0 ? accent + '22' : 'transparent',
                border: `1px solid ${draftCount > 0 ? accent + '88' : border}`,
                color: draftCount > 0 ? accent : text,
                fontFamily: 'monospace', fontSize: 10,
                padding: '3px 0', borderRadius: 3,
                cursor: draftCount > 0 ? 'pointer' : 'default',
                opacity: draftCount > 0 ? 1 : 0.4,
                fontWeight: 'bold', letterSpacing: 0.3,
              }}
            >▶ Place All ({draftCount})</button>
            <button
              onClick={clearAllLevels}
              style={{
                flex: 1, background: 'transparent',
                border: '1px solid #e0556066', color: '#e05560',
                fontFamily: 'monospace', fontSize: 10,
                padding: '3px 0', borderRadius: 3, cursor: 'pointer',
                fontWeight: 'bold', letterSpacing: 0.3,
              }}
            >✕ Cancel All</button>
            {(execCount > 0 || cancelCount > 0) && (
              <button
                onClick={clearHistory}
                style={{
                  background: 'transparent',
                  border: `1px solid ${border}`,
                  color: text, opacity: 0.6,
                  fontFamily: 'monospace', fontSize: 10,
                  padding: '3px 6px', borderRadius: 3, cursor: 'pointer',
                  letterSpacing: 0.3,
                }}
              >Clear</button>
            )}
          </div>
        )}
      </div>

      {/* ── Order list ── */}
      <div style={{ flex: 1, overflowY: 'auto', padding: '6px 8px', display: 'flex', flexDirection: 'column', gap: 5 }}>
        {groups.length === 0 ? (
          <div style={{ color: text, opacity: 0.3, fontSize: 11, textAlign: 'center', marginTop: 24, letterSpacing: 0.3 }}>
            No orders
            <div style={{ fontSize: 9, marginTop: 4, opacity: 0.7 }}>Right-click chart to place orders</div>
          </div>
        ) : groups.map((group, i) => {

          // ── Single order card ──────────────────────────────────────────────
          if (group.kind === 'single') {
            const { level, paneId, symbol, timeframe } = group
            const color    = (level.type === 'buy' ? t.bull : level.type === 'sell' ? t.bear : undefined) ?? TYPE_COLOR[level.type] ?? '#aaa'
            const icon     = TYPE_ICON[level.type] ?? '·'
            const isDraft  = level.status === 'draft'
            return (
              <div key={i} style={card(color, isDraft)}>
                <div style={{ display: 'flex', alignItems: 'center', gap: 6, marginBottom: 4 }}>
                  <span style={{ color, fontWeight: 'bold', fontSize: 11 }}>{icon} {level.type.toUpperCase()}</span>
                  <span style={{ color: text, fontSize: 10, opacity: 0.7, flex: 1 }}>
                    {symbol}<span style={{ opacity: 0.45, marginLeft: 3 }}>{timeframe}</span>
                  </span>
                  <span style={statusBadge(level.status)}>{level.status.toUpperCase()}</span>
                  <button style={removeBtn} onClick={() => {
                    if (level.status === 'executed' || level.status === 'cancelled') removeLevel(paneId, level.type)
                    else cancelLevel(paneId, level.type)
                  }} title={level.status === 'executed' || level.status === 'cancelled' ? 'Remove' : 'Cancel'}>×</button>
                </div>
                <div style={{ display: 'flex', alignItems: 'baseline', gap: 8 }}>
                  <span style={{ color, fontWeight: 'bold', fontSize: 14 }}>{fmtPrice(level.price)}</span>
                  <span style={{ color: text, opacity: 0.6, fontSize: 10 }}>×{level.qty}</span>
                  <span style={{ color: text, opacity: 0.5, fontSize: 10, marginLeft: 'auto' }}>{fmtNotional(level.price, level.qty)}</span>
                </div>
                {isDraft && (
                  <button style={placeBtn(color)} onClick={() => { fireOrder(level, paneId); placeLevel(paneId, level.type) }}>
                    {icon} Place — {fmtPrice(level.price)}
                  </button>
                )}
              </div>
            )
          }

          // ── OCO card ──────────────────────────────────────────────────────
          if (group.kind === 'oco') {
            const { target, stop, paneId, symbol, timeframe } = group
            const isDraft = target.status === 'draft'
            const qty = target.qty
            return (
              <div key={i} style={card('#a78bfa', isDraft)}>
                <div style={{ display: 'flex', alignItems: 'center', gap: 6, marginBottom: 6 }}>
                  <span style={{ color: '#a78bfa', fontWeight: 'bold', fontSize: 11 }}>⇅ OCO BRACKET</span>
                  <span style={{ color: text, fontSize: 10, opacity: 0.7, flex: 1 }}>
                    {symbol}<span style={{ opacity: 0.45, marginLeft: 3 }}>{timeframe}</span>
                  </span>
                  <span style={statusBadge(target.status)}>{target.status.toUpperCase()}</span>
                  <button style={removeBtn} onClick={() => {
                    if (target.status === 'executed' || target.status === 'cancelled') removeLevel(paneId, 'oco_target')
                    else cancelLevel(paneId, 'oco_target')
                  }} title={target.status === 'executed' || target.status === 'cancelled' ? 'Remove' : 'Cancel'}>×</button>
                </div>
                <div style={{ display: 'flex', gap: 12, marginBottom: 2 }}>
                  <div>
                    <div style={{ fontSize: 9, color: '#a78bfa', opacity: 0.6, marginBottom: 1 }}>⇧ TARGET</div>
                    <div style={{ fontSize: 13, fontWeight: 'bold', color: text }}>{fmtPrice(target.price)}</div>
                  </div>
                  <div>
                    <div style={{ fontSize: 9, color: '#a78bfa', opacity: 0.6, marginBottom: 1 }}>⇩ STOP</div>
                    <div style={{ fontSize: 13, fontWeight: 'bold', color: text }}>{fmtPrice(stop.price)}</div>
                  </div>
                  <div>
                    <div style={{ fontSize: 9, color: text, opacity: 0.4, marginBottom: 1 }}>SPREAD</div>
                    <div style={{ fontSize: 11, color: text, opacity: 0.65 }}>{fmtPrice(Math.abs(target.price - stop.price))}</div>
                  </div>
                  <div style={{ marginLeft: 'auto', textAlign: 'right' }}>
                    <div style={{ fontSize: 9, color: text, opacity: 0.4, marginBottom: 1 }}>QTY · NOTIONAL</div>
                    <div style={{ fontSize: 11, color: text, opacity: 0.65 }}>×{qty}  {fmtNotional(target.price, qty)}</div>
                  </div>
                </div>
                {isDraft && (
                  <button style={placeBtn('#a78bfa')} onClick={() => { fireOrder(target, paneId); placeLevel(paneId, 'oco_target') }}>
                    ⇅ Place OCO Bracket
                  </button>
                )}
              </div>
            )
          }

          // ── Trigger card ──────────────────────────────────────────────────
          if (group.kind === 'trigger') {
            const { buy, sell, paneId, symbol, timeframe } = group
            const isDraft     = buy.status === 'draft'
            const isTriggered = !!buy.triggered
            const qty         = buy.qty
            const statusText  = isTriggered ? 'TRIGGERED' : isDraft ? 'DRAFT' : 'LIVE'
            return (
              <div key={i} style={card(isTriggered ? '#f59e0b' : (t.bull ?? '#26a69a'), isDraft)}>
                <div style={{ display: 'flex', alignItems: 'center', gap: 6, marginBottom: 6 }}>
                  <span style={{ color: isTriggered ? '#f59e0b' : (t.bull ?? '#26a69a'), fontWeight: 'bold', fontSize: 11 }}>
                    ⟲ TRIGGER ORDER
                  </span>
                  <span style={{ color: text, fontSize: 10, opacity: 0.7, flex: 1 }}>
                    {symbol}<span style={{ opacity: 0.45, marginLeft: 3 }}>{timeframe}</span>
                  </span>
                  <span style={statusBadge(buy.status, isTriggered)}>{statusText}</span>
                  <button style={removeBtn} onClick={() => {
                    if (buy.status === 'executed' || buy.status === 'cancelled') removeLevel(paneId, 'trigger_buy')
                    else cancelLevel(paneId, 'trigger_buy')
                  }} title={buy.status === 'executed' || buy.status === 'cancelled' ? 'Remove' : 'Cancel'}>×</button>
                </div>
                <div style={{ display: 'flex', gap: 12, marginBottom: 2 }}>
                  <div>
                    <div style={{ fontSize: 9, color: t.bull ?? '#26a69a', opacity: isTriggered ? 0.4 : 0.7, marginBottom: 1 }}>
                      {isTriggered ? '▲ BUY — FILLED' : '▲ BUY ENTRY'}
                    </div>
                    <div style={{ fontSize: 13, fontWeight: 'bold', color: text, opacity: isTriggered ? 0.4 : 1 }}>
                      {fmtPrice(buy.price)}
                    </div>
                  </div>
                  <div>
                    <div style={{ fontSize: 9, color: '#f59e0b', opacity: 0.7, marginBottom: 1 }}>
                      {isTriggered ? '◎ SELL — ACTIVE' : '◎ SELL TARGET'}
                    </div>
                    <div style={{ fontSize: 13, fontWeight: 'bold', color: '#f59e0b' }}>{fmtPrice(sell.price)}</div>
                  </div>
                  <div style={{ marginLeft: 'auto', textAlign: 'right' }}>
                    <div style={{ fontSize: 9, color: text, opacity: 0.4, marginBottom: 1 }}>QTY · RANGE</div>
                    <div style={{ fontSize: 11, color: text, opacity: 0.65 }}>
                      ×{qty}  {fmtPrice(Math.abs(sell.price - buy.price))}
                      <span style={{ opacity: 0.5, marginLeft: 3, fontSize: 9 }}>
                        ({((Math.abs(sell.price - buy.price) / buy.price) * 100).toFixed(1)}%)
                      </span>
                    </div>
                  </div>
                </div>
                {isDraft && !isTriggered && (
                  <button style={placeBtn(t.bull ?? '#26a69a')} onClick={() => { fireOrder(buy, paneId); placeLevel(paneId, 'trigger_buy') }}>
                    ⟲ Place Trigger Order
                  </button>
                )}
                {isTriggered && (
                  <div style={{ marginTop: 5, fontSize: 10, color: '#f59e0b', opacity: 0.8, letterSpacing: 0.3 }}>
                    ◎ Sell order active — drag the target line to adjust
                  </div>
                )}
              </div>
            )
          }

          return null
        })}
      </div>
    </div>
  )
}
