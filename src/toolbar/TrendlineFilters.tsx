import { useState, useRef, useEffect } from 'react'
import { useChartStore, type AnnotationFilters } from '../store/chartStore'
import { getTheme } from '../themes'

const SECTIONS: { label: string; items: { key: keyof AnnotationFilters; label: string }[] }[] = [
  {
    label: 'Timeframe',
    items: [
      { key: '15m', label: '15 min' },
      { key: '30m', label: '30 min' },
      { key: '1H', label: '1 hour' },
      { key: '4H', label: '4 hour' },
      { key: '1D', label: 'Daily' },
      { key: '1W', label: 'Weekly' },
    ],
  },
  {
    label: 'Source',
    items: [
      { key: 'wick', label: 'Wicks (high/low)' },
      { key: 'body', label: 'Bodies (open/close)' },
    ],
  },
  {
    label: 'Type',
    items: [
      { key: 'support', label: 'Support' },
      { key: 'resistance', label: 'Resistance' },
      { key: 'channel', label: 'Channels' },
    ],
  },
  {
    label: 'Other',
    items: [
      { key: 'user', label: 'My drawings' },
    ],
  },
]

export function TrendlineFilters() {
  const [open, setOpen] = useState(false)
  const ref = useRef<HTMLDivElement>(null)
  const btnRef = useRef<HTMLButtonElement>(null)
  const { annotationFilters, toggleFilter, theme: themeName } = useChartStore()
  const theme = getTheme(themeName)

  // Count active filters
  const total = Object.keys(annotationFilters).length
  const active = Object.values(annotationFilters).filter(Boolean).length

  useEffect(() => {
    if (!open) return
    const onClick = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node) &&
          btnRef.current && !btnRef.current.contains(e.target as Node)) {
        setOpen(false)
      }
    }
    const onKey = (e: KeyboardEvent) => { if (e.key === 'Escape') setOpen(false) }
    window.addEventListener('mousedown', onClick, true)
    window.addEventListener('keydown', onKey)
    return () => {
      window.removeEventListener('mousedown', onClick, true)
      window.removeEventListener('keydown', onKey)
    }
  }, [open])

  return (
    <div style={{ position: 'relative' }}>
      <button ref={btnRef} onClick={() => setOpen(!open)} style={{
        background: open ? theme.borderActive + '22' : theme.toolbarBackground,
        color: active === total ? theme.borderActive : theme.axisText,
        border: `1px solid ${open ? theme.borderActive + '88' : theme.toolbarBorder}`,
        borderRadius: 3, padding: '2px 8px', fontSize: 11, fontFamily: 'monospace', cursor: 'pointer',
      }}>
        Trends {active}/{total} &#9662;
      </button>

      {open && (
        <div ref={ref} style={{
          position: 'absolute', top: '100%', left: 0, marginTop: 4,
          width: 200, background: theme.toolbarBackground,
          border: `1px solid ${theme.toolbarBorder}`, borderRadius: 4,
          boxShadow: '0 4px 16px rgba(0,0,0,0.5)', zIndex: 10000,
          fontFamily: 'monospace', fontSize: 11, overflow: 'hidden',
        }}
        onMouseDown={e => e.stopPropagation()}>
          {SECTIONS.map(section => (
            <div key={section.label}>
              <div style={{
                padding: '5px 10px 2px', color: theme.borderActive,
                fontSize: 9, letterSpacing: 1, textTransform: 'uppercase',
              }}>
                {section.label}
              </div>
              {section.items.map(item => (
                <label key={item.key} style={{
                  display: 'flex', alignItems: 'center', gap: 8,
                  padding: '4px 10px', cursor: 'pointer',
                  color: annotationFilters[item.key] ? theme.ohlcLabel : theme.axisText,
                }}
                onMouseEnter={e => (e.currentTarget.style.background = theme.borderActive + '15')}
                onMouseLeave={e => (e.currentTarget.style.background = 'transparent')}>
                  <input type="checkbox"
                    checked={annotationFilters[item.key]}
                    onChange={() => toggleFilter(item.key)}
                    style={{ accentColor: theme.borderActive, cursor: 'pointer' }}
                  />
                  {item.label}
                </label>
              ))}
            </div>
          ))}

          {/* Quick actions */}
          <div style={{
            display: 'flex', gap: 4, padding: '6px 10px',
            borderTop: `1px solid ${theme.toolbarBorder}`,
          }}>
            <button onClick={() => {
              for (const k of Object.keys(annotationFilters) as (keyof AnnotationFilters)[]) {
                if (!annotationFilters[k]) toggleFilter(k)
              }
            }} style={{
              flex: 1, background: 'transparent', color: theme.axisText,
              border: `1px solid ${theme.toolbarBorder}`, borderRadius: 3,
              padding: '2px 0', fontSize: 9, cursor: 'pointer', fontFamily: 'monospace',
            }}>All On</button>
            <button onClick={() => {
              for (const k of Object.keys(annotationFilters) as (keyof AnnotationFilters)[]) {
                if (k !== 'user' && annotationFilters[k]) toggleFilter(k)
              }
            }} style={{
              flex: 1, background: 'transparent', color: theme.axisText,
              border: `1px solid ${theme.toolbarBorder}`, borderRadius: 3,
              padding: '2px 0', fontSize: 9, cursor: 'pointer', fontFamily: 'monospace',
            }}>All Off</button>
          </div>
        </div>
      )}
    </div>
  )
}
