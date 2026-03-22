import { useState, useEffect } from 'react'
import { getRenderEngine, getDataStore } from '../globals'

export function FrameStats() {
  const [line1, setLine1] = useState('')
  const [line2, setLine2] = useState('')

  useEffect(() => {
    const id = setInterval(() => {
      try {
        const rs = getRenderEngine().scheduler.getStats()
        setLine1(`${rs.updatesPerSec} upd/s \u00b7 ${rs.renderTimeMs.toFixed(1)}ms \u00b7 pk ${rs.renderTimePeak.toFixed(1)}ms \u00b7 ${rs.panesRendered}/${rs.panesTotal}`)

        const dm = getDataStore().getMetrics()
        setLine2(`loads:${dm.loadCount} \u00b7 avg ${dm.avgLoadMs}ms \u00b7 pk ${Math.round(dm.loadPeakMs)}ms \u00b7 ticks:${dm.tickCount} \u00b7 ${dm.avgTickMs}ms/t \u00b7 cache ${dm.cacheHits}/${dm.cacheHits + dm.cacheMisses} \u00b7 page:${dm.paginationCount}`)
      } catch { /* not ready */ }
    }, 500)
    return () => clearInterval(id)
  }, [])

  if (!line1) return null

  return (
    <div style={{
      position: 'fixed', top: 40, right: 8,
      padding: '4px 8px', background: 'rgba(0,0,0,0.75)',
      color: '#0f0', fontFamily: 'monospace', fontSize: 9,
      lineHeight: '13px', borderRadius: 4, zIndex: 9999,
      pointerEvents: 'none', whiteSpace: 'nowrap',
    }}>
      <div>{line1}</div>
      <div style={{ color: '#0a0' }}>{line2}</div>
    </div>
  )
}
