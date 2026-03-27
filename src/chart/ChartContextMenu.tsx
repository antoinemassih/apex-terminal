import { useEffect, useState, useMemo, useRef } from 'react'
import { useDrawingStore } from '../store/drawingStore'
import { useChartStore } from '../store/chartStore'
import { useOrderStore } from '../store/orderStore'
import { getTheme } from '../themes'

interface Props {
  x: number
  y: number
  symbol: string
  paneId: string       // chartStore pane id (pane-0 etc) — for indicator toggles
  orderPaneId: string  // orderStore pane id (symbol:timeframe) — for order levels
  clickPrice: number | null
  orderEntryEnabled: boolean
  onReset: () => void
  onDragZoom: () => void
  onClose: () => void
}

export function ChartContextMenu({ x, y, symbol, paneId, orderPaneId, clickPrice, orderEntryEnabled, onReset, onDragZoom, onClose }: Props) {
  const themeName = useChartStore(s => s.theme)
  const t = getTheme(themeName)
  const toggleAllIndicators = useChartStore(s => s.toggleAllIndicators)
  const paneConfig = useChartStore(s => s.panes.find(p => p.id === paneId))
  const indicatorsVisible = (paneConfig?.visibleIndicators.length ?? 0) > 0

  const { setLevel } = useOrderStore()

  const toggleHideDrawings = useDrawingStore(s => s.toggleHideDrawings)
  const drawingsHidden = useDrawingStore(s => s.drawingsHidden)
  const removeAllForSymbol = useDrawingStore(s => s.removeAllForSymbol)
  const removeAllInGroup = useDrawingStore(s => s.removeAllInGroup)
  const groups = useDrawingStore(s => s.groups)
  const toggleHideGroup = useDrawingStore(s => s.toggleHideGroup)
  const groupHidden = useDrawingStore(s => s.groupHidden)
  // Stable selector — s.drawings returns the same array reference when unchanged.
  // The reduce below always produces a new object so it must live in useMemo, not
  // directly in a Zustand selector (which would cause an infinite re-render loop).
  const drawings = useDrawingStore(s => s.drawings)
  const groupCounts = useMemo(
    () => drawings.filter(d => d.symbol === symbol).reduce<Record<string, number>>((acc, d) => {
      const gid = d.groupId ?? 'default'
      acc[gid] = (acc[gid] ?? 0) + 1
      return acc
    }, {}),
    [drawings, symbol]
  )
  const hidden = drawingsHidden(symbol)

  const [groupsPopout, setGroupsPopout] = useState<{ left: number; top: number } | null>(null)
  const popoutTimer = useRef<ReturnType<typeof setTimeout> | null>(null)

  useEffect(() => {
    // Only close on left-click outside — right-click is handled by ChartPane's onMouseDown
    // to reposition the menu rather than close-then-reopen (avoids double flash).
    const handler = (e: MouseEvent) => { if (e.button === 0) onClose() }
    document.addEventListener('mousedown', handler)
    return () => document.removeEventListener('mousedown', handler)
  }, [onClose])

  useEffect(() => {
    const handler = (e: KeyboardEvent) => { if (e.key === 'Escape') onClose() }
    document.addEventListener('keydown', handler)
    return () => document.removeEventListener('keydown', handler)
  }, [onClose])

  const fmtClickPrice = (p: number) => p >= 10 ? p.toFixed(2) : p.toFixed(4)

  const menuBg = t.toolbarBackground
  const menuBorder = t.toolbarBorder
  const textColor = t.ohlcLabel
  const dimColor = t.axisText
  const accentColor = t.borderActive
  const dangerColor = '#e05560'

  const sep: React.CSSProperties = {
    height: 1, background: menuBorder, margin: '3px 0',
  }

  const item = (color = textColor): React.CSSProperties => ({
    display: 'flex', alignItems: 'center', gap: 8,
    width: '100%', padding: '6px 14px',
    background: 'none', border: 'none', cursor: 'pointer',
    color, fontFamily: 'monospace', fontSize: 12,
    textAlign: 'left', whiteSpace: 'nowrap', boxSizing: 'border-box',
  })

  const onHover = (e: React.MouseEvent<HTMLButtonElement>) => {
    e.currentTarget.style.background = accentColor + '22'
    e.currentTarget.style.color = '#fff'
  }
  const onLeave = (e: React.MouseEvent<HTMLButtonElement>, color = textColor) => {
    e.currentTarget.style.background = 'none'
    e.currentTarget.style.color = color
  }

  return (
    <div
      style={{
        position: 'fixed', left: x, top: y, zIndex: 2000,
        background: menuBg,
        border: `1px solid ${menuBorder}`,
        borderRadius: 6, padding: '4px 0',
        boxShadow: '0 8px 24px rgba(0,0,0,0.6)',
        minWidth: 190,
        fontFamily: 'monospace',
      }}
      onMouseDown={e => e.stopPropagation()}
    >
      <button style={item(dimColor)}
        onMouseEnter={onHover} onMouseLeave={e => onLeave(e, dimColor)}
        onClick={() => { onReset(); onClose() }}>
        ↺ Reset Chart
      </button>
      <button style={item(dimColor)}
        onMouseEnter={onHover} onMouseLeave={e => onLeave(e, dimColor)}
        onClick={() => { onDragZoom(); onClose() }}>
        ⊡ Drag Zoom
      </button>

      <div style={sep} />

      <button style={item(textColor)}
        onMouseEnter={onHover} onMouseLeave={e => onLeave(e, textColor)}
        onClick={() => { toggleHideDrawings(symbol); onClose() }}>
        {hidden ? '◉ Show All Drawings' : '◎ Hide All Drawings'}
      </button>
      <button style={item(textColor)}
        onMouseEnter={onHover} onMouseLeave={e => onLeave(e, textColor)}
        onClick={() => { toggleAllIndicators(paneId); onClose() }}>
        {indicatorsVisible ? '◎ Hide All Indicators' : '◉ Show All Indicators'}
      </button>

      <div style={sep} />

      {/* Groups submenu — flyout to the right on hover */}
      <div
        style={{ position: 'relative' }}
        onMouseEnter={e => {
          if (popoutTimer.current) { clearTimeout(popoutTimer.current); popoutTimer.current = null }
          const r = e.currentTarget.getBoundingClientRect()
          setGroupsPopout({ left: r.right, top: r.top })
        }}
        onMouseLeave={() => {
          popoutTimer.current = setTimeout(() => setGroupsPopout(null), 150)
        }}
      >
        <button
          style={{ ...item(textColor), justifyContent: 'space-between' }}
          onMouseEnter={onHover} onMouseLeave={e => onLeave(e, textColor)}
        >
          <span>◫ Groups</span>
          <span style={{ opacity: 0.5, fontSize: 10 }}>▶</span>
        </button>

        {groupsPopout && (
          <div
            style={{
              position: 'fixed',
              left: groupsPopout.left,
              top: groupsPopout.top,
              zIndex: 2001,
              background: menuBg,
              border: `1px solid ${menuBorder}`,
              borderRadius: 6,
              padding: '4px 0',
              boxShadow: '0 8px 24px rgba(0,0,0,0.6)',
              minWidth: 170,
              fontFamily: 'monospace',
            }}
            onMouseEnter={() => {
              if (popoutTimer.current) { clearTimeout(popoutTimer.current); popoutTimer.current = null }
            }}
            onMouseLeave={() => setGroupsPopout(null)}
          >
            {groups.map(g => {
              const isHidden = groupHidden(g.id)
              const count = groupCounts[g.id] ?? 0
              return (
                <button
                  key={g.id}
                  style={{
                    ...item(isHidden ? dimColor : textColor),
                    justifyContent: 'space-between',
                  }}
                  onMouseEnter={onHover}
                  onMouseLeave={e => onLeave(e, isHidden ? dimColor : textColor)}
                  onClick={() => toggleHideGroup(g.id)}
                >
                  <span style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
                    {g.color && <span style={{ width: 7, height: 7, borderRadius: '50%', background: g.color, display: 'inline-block', flexShrink: 0 }} />}
                    {isHidden ? '◎' : '◉'} {g.name}
                  </span>
                  <span style={{ opacity: 0.4, fontSize: 10 }}>{count}</span>
                </button>
              )
            })}
          </div>
        )}
      </div>

      <div style={sep} />

      <button style={item(dangerColor)}
        onMouseEnter={e => { e.currentTarget.style.background = dangerColor + '22'; e.currentTarget.style.color = dangerColor }}
        onMouseLeave={e => { e.currentTarget.style.background = 'none'; e.currentTarget.style.color = dangerColor }}
        onClick={() => { removeAllForSymbol(symbol); onClose() }}>
        ✕ Delete All Drawings
      </button>
      <button style={item(dangerColor)}
        onMouseEnter={e => { e.currentTarget.style.background = dangerColor + '22'; e.currentTarget.style.color = dangerColor }}
        onMouseLeave={e => { e.currentTarget.style.background = 'none'; e.currentTarget.style.color = dangerColor }}
        onClick={() => { removeAllInGroup('default'); onClose() }}>
        ✕ Delete All Temporary
      </button>

      {orderEntryEnabled && clickPrice !== null && (
        <>
          <div style={sep} />
          <div style={{
            padding: '3px 14px 2px',
            fontSize: 9, fontFamily: 'monospace',
            color: accentColor, opacity: 0.7, letterSpacing: 0.8,
          }}>
            ORDERS @ {fmtClickPrice(clickPrice)}
          </div>
          <button style={item(t.bull ?? '#26a69a')}
            onMouseEnter={onHover} onMouseLeave={e => onLeave(e, t.bull ?? '#26a69a')}
            onClick={() => { setLevel(orderPaneId, { type: 'buy', price: clickPrice }); onClose() }}>
            ▲ Set Buy Order
          </button>
          <button style={item(t.bear ?? '#ef5350')}
            onMouseEnter={onHover} onMouseLeave={e => onLeave(e, t.bear ?? '#ef5350')}
            onClick={() => { setLevel(orderPaneId, { type: 'sell', price: clickPrice }); onClose() }}>
            ▼ Set Sell Order
          </button>
          <button style={item(textColor)}
            onMouseEnter={onHover} onMouseLeave={e => onLeave(e, textColor)}
            onClick={() => { setLevel(orderPaneId, { type: 'stop', price: clickPrice }); onClose() }}>
            ⊗ Set Stop Loss
          </button>
          <button style={item('#a78bfa')}
            onMouseEnter={onHover} onMouseLeave={e => onLeave(e, '#a78bfa')}
            onClick={() => {
              const offset = Math.max(0.01, clickPrice * 0.01)
              setLevel(orderPaneId, { type: 'oco_target', price: Math.round((clickPrice + offset) * 100) / 100 })
              setLevel(orderPaneId, { type: 'oco_stop',   price: Math.round((clickPrice - offset) * 100) / 100 })
              onClose()
            }}>
            ⇅ Set OCO Bracket
          </button>
          <button style={item(t.bull ?? '#26a69a')}
            onMouseEnter={onHover} onMouseLeave={e => onLeave(e, t.bull ?? '#26a69a')}
            onClick={() => {
              const target = Math.round(clickPrice * 1.02 * 100) / 100
              setLevel(orderPaneId, { type: 'trigger_buy',  price: clickPrice })
              setLevel(orderPaneId, { type: 'trigger_sell', price: target })
              onClose()
            }}>
            ⟲ Set Trigger Order
          </button>
        </>
      )}
    </div>
  )
}
