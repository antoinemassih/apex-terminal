import { useState, useEffect } from 'react'
import { getRenderEngine } from '../globals'
import type { FrameStats as FrameStatsType } from '../engine'

export function FrameStats() {
  const [stats, setStats] = useState<FrameStatsType | null>(null)

  useEffect(() => {
    const id = setInterval(() => {
      try {
        setStats(getRenderEngine().scheduler.getStats())
      } catch {
        // Engine not ready yet
      }
    }, 500)
    return () => clearInterval(id)
  }, [])

  if (!stats) return null

  return (
    <div style={{
      position: 'fixed',
      top: 40,
      right: 8,
      padding: '4px 8px',
      background: 'rgba(0,0,0,0.75)',
      color: '#0f0',
      fontFamily: 'monospace',
      fontSize: 10,
      lineHeight: '14px',
      borderRadius: 4,
      zIndex: 9999,
      pointerEvents: 'none',
      whiteSpace: 'nowrap',
    }}>
      {stats.updatesPerSec} upd/s &middot; {stats.renderTimeMs.toFixed(1)}ms &middot; peak {stats.renderTimePeak.toFixed(1)}ms &middot; {stats.panesRendered}/{stats.panesTotal}
    </div>
  )
}
