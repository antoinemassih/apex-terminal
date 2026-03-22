import { useState, useEffect } from 'react'
import { getRenderEngine } from '../globals'
import type { FrameStats as FrameStatsType } from '../engine'

export function FrameStats() {
  const [stats, setStats] = useState<FrameStatsType | null>(null)

  useEffect(() => {
    const id = setInterval(() => {
      try {
        const s = getRenderEngine().scheduler.getStats()
        setStats(s)
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
      top: 8,
      right: 8,
      padding: '4px 8px',
      background: 'rgba(0,0,0,0.75)',
      color: '#0f0',
      fontFamily: 'monospace',
      fontSize: 11,
      lineHeight: '16px',
      borderRadius: 4,
      zIndex: 9999,
      pointerEvents: 'none',
      whiteSpace: 'nowrap',
    }}>
      {stats.fps.toFixed(0)} fps &middot; {stats.frameTimeMs.toFixed(1)}ms &middot; peak {stats.frameTimePeak.toFixed(1)}ms &middot; {stats.dirtyPanes} dirty
    </div>
  )
}
