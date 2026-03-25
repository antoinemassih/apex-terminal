import { useEffect, useRef, useState } from 'react'
import { useDrawingStore } from '../store/drawingStore'
import { useChartStore } from '../store/chartStore'
import { getTheme } from '../themes'
import { GroupManagerModal } from './GroupManagerModal'

interface Props {
  drawingId: string
  onClose: () => void
}

const COLORS = [
  '#4a9eff', '#e74c3c', '#2ecc71', '#f39c12',
  '#9b59b6', '#1abc9c', '#ffffff', '#e67e22',
]
const OPACITIES = [1, 0.75, 0.5, 0.25] as const
const LINE_STYLES = ['solid', 'dashed', 'dotted'] as const
const THICKNESSES = [0.5, 1, 1.5, 2.5] as const

const styleLabel: Record<string, string> = {
  solid: '\u2500\u2500\u2500',
  dashed: '\u2504\u2504\u2504',
  dotted: '\u00B7 \u00B7 \u00B7',
}

const thicknessLabel = (t: number) =>
  t <= 0.5 ? '\u2504' : t <= 1 ? '\u2500' : t <= 1.5 ? '\u2501' : '\u2588'

function nextGroupName(groups: { name: string }[]): string {
  const names = new Set(groups.map(g => g.name))
  let n = 1
  while (names.has(`Group ${n}`)) n++
  return `Group ${n}`
}

export function LineStylePopup({ drawingId, onClose }: Props) {
  const ref = useRef<HTMLDivElement>(null)
  const groupDropRef = useRef<HTMLDivElement>(null)
  const drawing = useDrawingStore(s => s.drawings.find(d => d.id === drawingId))
  const groups = useDrawingStore(s => s.groups)
  const updateStyle = useDrawingStore(s => s.updateDrawingStyle)
  const removeDrawing = useDrawingStore(s => s.removeDrawing)
  const createGroup = useDrawingStore(s => s.createGroup)
  const setDrawingGroup = useDrawingStore(s => s.setDrawingGroup)
  const applyStyleToGroup = useDrawingStore(s => s.applyStyleToGroup)
  const themeName = useChartStore(s => s.theme)
  const t = getTheme(themeName)

  const [groupOpen, setGroupOpen] = useState(false)
  const [showManager, setShowManager] = useState(false)

  // Close on outside click or Escape (two-stage: manager first, then popup)
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        if (showManager) { setShowManager(false); return }
        onClose()
      }
    }
    const onMouse = (e: MouseEvent) => {
      // If group dropdown is open, close it first
      if (groupOpen && groupDropRef.current && !groupDropRef.current.contains(e.target as Node)) {
        setGroupOpen(false)
        return
      }
      if (!groupOpen && !showManager && ref.current && !ref.current.contains(e.target as Node)) {
        onClose()
      }
    }
    window.addEventListener('keydown', onKey)
    const timer = setTimeout(() => window.addEventListener('mousedown', onMouse), 0)
    return () => {
      window.removeEventListener('keydown', onKey)
      window.removeEventListener('mousedown', onMouse)
      clearTimeout(timer)
    }
  }, [onClose, groupOpen, showManager])

  if (!drawing) return null

  const color = drawing.color
  const opacity = drawing.opacity ?? 1
  const lineStyle = drawing.lineStyle ?? 'solid'
  const thickness = drawing.thickness ?? 1.5
  const groupId = drawing.groupId ?? 'default'
  const currentGroup = groups.find(g => g.id === groupId) ?? groups[0]

  const accent = t.borderActive
  const bg = t.toolbarBackground
  const border = t.toolbarBorder
  const text = t.ohlcLabel

  const btnBase: React.CSSProperties = {
    background: 'none',
    border: `1px solid ${border}`,
    borderRadius: 3,
    color: text,
    fontFamily: 'monospace',
    fontSize: 11,
    cursor: 'pointer',
    padding: '2px 6px',
    lineHeight: '16px',
  }

  const activeBtn: React.CSSProperties = {
    ...btnBase,
    borderColor: accent,
    background: accent + '26',
    color: '#fff',
  }

  const sep: React.CSSProperties = {
    width: 1, height: 18, background: border, margin: '0 4px', flexShrink: 0,
  }

  const rowSep: React.CSSProperties = {
    height: 1, background: border, margin: '5px 0',
  }

  const handleGroupSelect = (id: string) => {
    setGroupOpen(false)
    if (id === '__new__') {
      const g = createGroup(nextGroupName(groups))
      setDrawingGroup(drawingId, g.id)
      setShowManager(true)
    } else if (id === '__manage__') {
      setShowManager(true)
    } else {
      setDrawingGroup(drawingId, id)
    }
  }

  const dropItemStyle: React.CSSProperties = {
    padding: '5px 10px',
    cursor: 'pointer',
    fontFamily: 'monospace',
    fontSize: 11,
    color: text,
    whiteSpace: 'nowrap',
  }

  return (
    <>
      <div
        ref={ref}
        style={{
          position: 'absolute',
          top: 6,
          left: '50%',
          transform: 'translateX(-50%)',
          zIndex: 100,
          background: bg,
          border: `1px solid ${border}`,
          borderRadius: 6,
          padding: '5px 10px',
          boxShadow: '0 4px 16px rgba(0,0,0,0.55)',
          display: 'flex',
          flexDirection: 'column',
          fontFamily: 'monospace',
          fontSize: 11,
          userSelect: 'none',
          whiteSpace: 'nowrap',
        }}
        onMouseDown={e => e.stopPropagation()}
      >
        {/* Row 1: style controls */}
        <div style={{ display: 'flex', alignItems: 'center', gap: 4 }}>
          {/* Colors */}
          {COLORS.map(c => (
            <div
              key={c}
              onClick={() => updateStyle(drawingId, { color: c })}
              title={c}
              style={{
                width: 13, height: 13, borderRadius: '50%', background: c,
                border: c === color ? '2px solid #fff' : '2px solid transparent',
                cursor: 'pointer', boxSizing: 'border-box', flexShrink: 0,
                outline: c === color ? `1px solid ${accent}` : 'none',
                outlineOffset: 1,
              }}
            />
          ))}

          <div style={sep} />

          {/* Line styles */}
          {LINE_STYLES.map(s => (
            <button key={s} onClick={() => updateStyle(drawingId, { lineStyle: s })}
              style={s === lineStyle ? activeBtn : btnBase} title={s}>
              {styleLabel[s]}
            </button>
          ))}

          <div style={sep} />

          {/* Thicknesses */}
          {THICKNESSES.map(tk => (
            <button key={tk} onClick={() => updateStyle(drawingId, { thickness: tk })}
              style={tk === thickness ? activeBtn : btnBase} title={`${tk}px`}>
              {thicknessLabel(tk)}
            </button>
          ))}

          <div style={sep} />

          {/* Opacities */}
          {OPACITIES.map(o => (
            <button key={o} onClick={() => updateStyle(drawingId, { opacity: o })}
              style={o === opacity ? activeBtn : btnBase} title={`${Math.round(o * 100)}%`}>
              {Math.round(o * 100)}%
            </button>
          ))}

          <div style={sep} />

          {/* Delete */}
          <button
            onClick={() => { removeDrawing(drawingId); onClose() }}
            style={{ ...btnBase, color: '#e05560', borderColor: '#e05560' }}
            title="Delete drawing"
          >
            ✕
          </button>
        </div>

        {/* Separator */}
        <div style={rowSep} />

        {/* Row 2: group controls */}
        <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
          {/* Group dropdown */}
          <div ref={groupDropRef} style={{ position: 'relative' }}>
            <button
              onClick={() => setGroupOpen(v => !v)}
              style={{
                ...btnBase,
                display: 'flex', alignItems: 'center', gap: 4,
                borderColor: groupOpen ? accent : border,
                minWidth: 90, maxWidth: 140, overflow: 'hidden',
              }}
              title="Assign to group"
            >
              {currentGroup?.color && (
                <span style={{ width: 7, height: 7, borderRadius: '50%', background: currentGroup.color, display: 'inline-block', flexShrink: 0 }} />
              )}
              <span style={{ overflow: 'hidden', textOverflow: 'ellipsis', flex: 1, textAlign: 'left' }}>
                {currentGroup?.name ?? 'Default'}
              </span>
              <span style={{ color: t.axisText, fontSize: 9, flexShrink: 0 }}>▾</span>
            </button>

            {groupOpen && (
              <div style={{
                position: 'absolute', top: 'calc(100% + 3px)', left: 0,
                background: bg, border: `1px solid ${border}`,
                borderRadius: 4, boxShadow: '0 6px 20px rgba(0,0,0,0.5)',
                zIndex: 200, minWidth: 140,
              }}>
                {groups.map(g => (
                  <div
                    key={g.id}
                    onMouseDown={() => handleGroupSelect(g.id)}
                    style={{
                      ...dropItemStyle,
                      background: g.id === groupId ? accent + '22' : 'none',
                      color: g.id === groupId ? '#fff' : text,
                      display: 'flex', alignItems: 'center', gap: 6,
                    }}
                  >
                    {g.color
                      ? <span style={{ width: 7, height: 7, borderRadius: '50%', background: g.color, flexShrink: 0, display: 'inline-block' }} />
                      : <span style={{ width: 7, flexShrink: 0 }} />
                    }
                    {g.name}
                  </div>
                ))}
                <div style={{ height: 1, background: border, margin: '2px 0' }} />
                <div onMouseDown={() => handleGroupSelect('__new__')} style={{ ...dropItemStyle, color: accent }}>
                  ＋ New group
                </div>
                <div onMouseDown={() => handleGroupSelect('__manage__')} style={{ ...dropItemStyle, color: t.axisText }}>
                  ⚙ Manage groups...
                </div>
              </div>
            )}
          </div>

          {/* Apply style to entire group */}
          <button
            onClick={() => applyStyleToGroup(groupId, { color, opacity, lineStyle, thickness })}
            style={{ ...btnBase, color: t.axisText }}
            title={`Apply current style to all drawings in "${currentGroup?.name ?? 'Default'}"`}
          >
            → group
          </button>
        </div>
      </div>

      {showManager && (
        <GroupManagerModal onClose={() => setShowManager(false)} />
      )}
    </>
  )
}
