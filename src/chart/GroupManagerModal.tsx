import { useState } from 'react'
import { useDrawingStore } from '../store/drawingStore'
import { useChartStore } from '../store/chartStore'
import { getTheme } from '../themes'
import type { DrawingGroup } from '../types'

interface Props {
  onClose: () => void
}

function nextGroupName(groups: DrawingGroup[]): string {
  const names = new Set(groups.map(g => g.name))
  let n = 1
  while (names.has(`Group ${n}`)) n++
  return `Group ${n}`
}

interface RowProps {
  group: DrawingGroup
  count: number
  onRename: (id: string, name: string) => void
  onDelete: (id: string) => void
  bg: string
  border: string
  text: string
  dim: string
}

function GroupRow({ group, count, onRename, onDelete, bg, border, text, dim }: RowProps) {
  const [name, setName] = useState(group.name)

  const commit = () => {
    const trimmed = name.trim()
    if (trimmed && trimmed !== group.name) onRename(group.id, trimmed)
    else setName(group.name)
  }

  return (
    <div style={{
      display: 'flex', alignItems: 'center', gap: 8,
      padding: '6px 0', borderBottom: `1px solid ${border}`,
    }}>
      {group.color
        ? <div style={{ width: 8, height: 8, borderRadius: '50%', background: group.color, flexShrink: 0 }} />
        : <div style={{ width: 8, flexShrink: 0 }} />
      }
      <input
        value={name}
        onChange={e => setName(e.target.value)}
        onBlur={commit}
        onKeyDown={e => {
          if (e.key === 'Enter') e.currentTarget.blur()
          if (e.key === 'Escape') { setName(group.name); e.currentTarget.blur() }
        }}
        disabled={group.id === 'default'}
        style={{
          flex: 1,
          background: group.id === 'default' ? 'transparent' : bg,
          border: group.id === 'default' ? 'none' : `1px solid ${border}`,
          borderRadius: 3,
          color: group.id === 'default' ? dim : text,
          fontFamily: 'monospace',
          fontSize: 12,
          padding: '2px 6px',
          outline: 'none',
        }}
      />
      <span style={{ color: dim, fontSize: 10, flexShrink: 0, minWidth: 60, textAlign: 'right' }}>
        {count} drawing{count !== 1 ? 's' : ''}
      </span>
      <button
        onClick={() => onDelete(group.id)}
        disabled={group.id === 'default'}
        title={group.id === 'default' ? 'Cannot delete Default group' : 'Delete group (drawings moved to Default)'}
        style={{
          background: 'none', border: 'none', padding: '0 4px',
          color: group.id === 'default' ? border : '#e05560',
          cursor: group.id === 'default' ? 'default' : 'pointer',
          fontSize: 13, flexShrink: 0,
        }}
      >✕</button>
    </div>
  )
}

export function GroupManagerModal({ onClose }: Props) {
  const themeName = useChartStore(s => s.theme)
  const t = getTheme(themeName)
  const groups = useDrawingStore(s => s.groups)
  const createGroup = useDrawingStore(s => s.createGroup)
  const renameGroup = useDrawingStore(s => s.renameGroup)
  const deleteGroup = useDrawingStore(s => s.deleteGroup)
  const drawings = useDrawingStore(s => s.drawings)

  const countByGroup = drawings.reduce<Record<string, number>>((acc, d) => {
    const gid = d.groupId ?? 'default'
    acc[gid] = (acc[gid] ?? 0) + 1
    return acc
  }, {})

  const bg = t.toolbarBackground
  const border = t.toolbarBorder
  const text = t.ohlcLabel
  const dim = t.axisText
  const accent = t.borderActive

  const btnBase: React.CSSProperties = {
    background: 'none',
    border: `1px solid ${border}`,
    borderRadius: 4,
    color: text,
    fontFamily: 'monospace',
    fontSize: 12,
    cursor: 'pointer',
    padding: '5px 12px',
  }

  return (
    <div
      style={{
        position: 'fixed', inset: 0, zIndex: 3000,
        background: 'rgba(0,0,0,0.55)',
        display: 'flex', alignItems: 'center', justifyContent: 'center',
      }}
      onMouseDown={e => {
        e.stopPropagation()
        if (e.target === e.currentTarget) onClose()
      }}
    >
      <div
        style={{
          background: bg,
          border: `1px solid ${border}`,
          borderRadius: 8,
          padding: '20px 24px',
          minWidth: 360,
          maxHeight: '70vh',
          overflowY: 'auto',
          boxShadow: '0 16px 48px rgba(0,0,0,0.7)',
          fontFamily: 'monospace',
        }}
        onMouseDown={e => e.stopPropagation()}
      >
        {/* Header */}
        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 16 }}>
          <span style={{ color: text, fontSize: 13, fontWeight: 'bold' }}>Drawing Groups</span>
          <button onClick={onClose} style={{ background: 'none', border: 'none', color: dim, cursor: 'pointer', fontSize: 16, lineHeight: 1 }}>✕</button>
        </div>

        {/* Group list */}
        <div style={{ marginBottom: 12 }}>
          {groups.map(group => (
            <GroupRow
              key={group.id}
              group={group}
              count={countByGroup[group.id] ?? 0}
              onRename={renameGroup}
              onDelete={deleteGroup}
              bg={bg} border={border} text={text} dim={dim}
            />
          ))}
        </div>

        {/* Footer */}
        <div style={{ display: 'flex', gap: 8, justifyContent: 'space-between', marginTop: 8 }}>
          <button
            onClick={() => createGroup(nextGroupName(groups))}
            style={{ ...btnBase, borderColor: accent, color: accent }}
          >
            ＋ New Group
          </button>
          <button onClick={onClose} style={btnBase}>
            Done
          </button>
        </div>
      </div>
    </div>
  )
}
