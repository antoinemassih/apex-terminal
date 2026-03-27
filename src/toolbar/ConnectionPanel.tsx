import { useState, useEffect, useRef } from 'react'
import { getRenderEngine, getDataProvider } from '../globals'
import { getTheme } from '../themes'
import { useChartStore } from '../store/chartStore'
import type { EngineState } from '../engine'
import type { IBKRProvider } from '../data/IBKRProvider'
import type { OcocoClient } from '../data/OcocoClient'

interface ServiceStatus {
  name: string
  label: string
  status: 'ok' | 'warn' | 'error' | 'off'
  detail: string
}

function getOcocoClient(): OcocoClient | null {
  return (window as any).__ococoClient ?? null
}

export function ConnectionPanel() {
  const themeName = useChartStore(s => s.theme)
  const t = getTheme(themeName)
  const [open, setOpen] = useState(false)
  const [services, setServices] = useState<ServiceStatus[]>([])
  const btnRef = useRef<HTMLButtonElement>(null)
  const panelRef = useRef<HTMLDivElement>(null)

  // Poll service statuses while open (and once on mount for the indicator dot)
  useEffect(() => {
    const poll = () => {
      const statuses: ServiceStatus[] = []

      // GPU Engine
      try {
        const engine = getRenderEngine()
        const state: EngineState = engine.state
        statuses.push({
          name: 'gpu', label: 'GPU Engine',
          status: state === 'ready' ? 'ok' : state === 'recovering' ? 'warn' : 'error',
          detail: state === 'ready' ? 'WebGPU active' : state === 'recovering' ? 'Recovering...' : 'Device lost',
        })
      } catch {
        statuses.push({ name: 'gpu', label: 'GPU Engine', status: 'error', detail: 'Not initialized' })
      }

      // Data Provider (IBKR)
      try {
        const provider = getDataProvider()
        const ibkr = provider as IBKRProvider
        const ready = ibkr.wsReady ?? false
        statuses.push({
          name: 'feed', label: `Market Feed (${provider.name})`,
          status: ready ? 'ok' : 'warn',
          detail: ready ? 'Connected' : 'Simulation mode',
        })
      } catch {
        statuses.push({ name: 'feed', label: 'Market Feed', status: 'error', detail: 'Not initialized' })
      }

      // OCOCO API
      const ococo = getOcocoClient()
      if (ococo) {
        // Check WS readyState
        const ws = (ococo as any).ws as WebSocket | null
        const wsOk = ws?.readyState === WebSocket.OPEN
        statuses.push({
          name: 'ococo', label: 'OCOCO Signals',
          status: wsOk ? 'ok' : 'warn',
          detail: wsOk ? 'WS connected' : 'WS disconnected',
        })
      } else {
        statuses.push({ name: 'ococo', label: 'OCOCO Signals', status: 'off', detail: 'Offline (IndexedDB)' })
      }

      setServices(statuses)
    }

    poll()
    if (!open) return
    const id = setInterval(poll, 2000)
    return () => clearInterval(id)
  }, [open])

  // Close on click outside
  useEffect(() => {
    if (!open) return
    const handler = (e: MouseEvent) => {
      if (panelRef.current?.contains(e.target as Node)) return
      if (btnRef.current?.contains(e.target as Node)) return
      setOpen(false)
    }
    document.addEventListener('mousedown', handler)
    return () => document.removeEventListener('mousedown', handler)
  }, [open])

  const aggregate = services.length === 0 ? 'warn'
    : services.some(s => s.status === 'error') ? 'error'
    : services.some(s => s.status === 'warn') ? 'warn'
    : 'ok'

  const dotColor = aggregate === 'ok' ? '#26a69a' : aggregate === 'warn' ? '#f59e0b' : '#ef5350'

  const statusColor = (s: string) =>
    s === 'ok' ? '#26a69a' : s === 'warn' ? '#f59e0b' : s === 'error' ? '#ef5350' : '#555'

  const handleReconnectFeed = () => {
    try {
      const provider = getDataProvider()
      provider.disconnect()
      provider.connect()
    } catch { /* */ }
  }

  const handleReconnectOcoco = () => {
    const client = getOcocoClient()
    if (client) {
      client.disconnectWs?.()
      client.connectWs?.()
    }
  }

  const handleRetryGPU = () => {
    try { getRenderEngine().retry() } catch { /* */ }
  }

  return (
    <>
      <button
        ref={btnRef}
        onClick={() => setOpen(!open)}
        style={{
          background: open ? t.borderActive + '22' : t.background,
          color: open ? t.borderActive : t.axisText,
          border: `1px solid ${open ? t.borderActive + '88' : aggregate === 'ok' ? t.bull + '44' : dotColor + '66'}`,
          borderRadius: 3,
          padding: '2px 8px',
          fontSize: 11,
          fontFamily: 'monospace',
          cursor: 'pointer',
          display: 'flex',
          alignItems: 'center',
          gap: 5,
        }}
      >
        <span style={{
          width: 6, height: 6, borderRadius: '50%',
          background: dotColor,
          display: 'inline-block',
          boxShadow: `0 0 4px ${dotColor}88`,
        }} />
        CONN
      </button>

      {open && btnRef.current && (() => {
        const r = btnRef.current!.getBoundingClientRect()
        return (
          <div
            ref={panelRef}
            style={{
              position: 'fixed',
              top: r.bottom + 4,
              right: window.innerWidth - r.right,
              zIndex: 2000,
              background: t.toolbarBackground,
              border: `1px solid ${t.toolbarBorder}`,
              borderRadius: 6,
              padding: '10px 0',
              boxShadow: '0 8px 24px rgba(0,0,0,0.6)',
              minWidth: 260,
              fontFamily: 'monospace',
            }}
            onMouseDown={e => e.stopPropagation()}
          >
            <div style={{
              padding: '0 14px 8px',
              fontSize: 10, fontWeight: 'bold', color: t.borderActive,
              letterSpacing: 0.8, borderBottom: `1px solid ${t.toolbarBorder}`,
            }}>
              CONNECTIONS
            </div>

            {services.map(svc => (
              <div key={svc.name} style={{
                padding: '8px 14px',
                display: 'flex', alignItems: 'center', gap: 10,
                borderBottom: `1px solid ${t.toolbarBorder}22`,
              }}>
                {/* Status dot */}
                <span style={{
                  width: 8, height: 8, borderRadius: '50%',
                  background: statusColor(svc.status),
                  flexShrink: 0,
                  boxShadow: svc.status === 'ok' ? `0 0 6px ${statusColor(svc.status)}66` : 'none',
                }} />

                {/* Info */}
                <div style={{ flex: 1, minWidth: 0 }}>
                  <div style={{ fontSize: 11, color: t.ohlcLabel, fontWeight: 'bold' }}>{svc.label}</div>
                  <div style={{ fontSize: 10, color: t.axisText, opacity: 0.6 }}>{svc.detail}</div>
                </div>

                {/* Action button */}
                {svc.name === 'feed' && (
                  <button
                    onClick={handleReconnectFeed}
                    style={{
                      background: 'none', border: `1px solid ${t.toolbarBorder}`,
                      color: t.axisText, fontSize: 10, fontFamily: 'monospace',
                      padding: '2px 8px', borderRadius: 3, cursor: 'pointer',
                    }}
                  >↻</button>
                )}
                {svc.name === 'ococo' && svc.status !== 'off' && (
                  <button
                    onClick={handleReconnectOcoco}
                    style={{
                      background: 'none', border: `1px solid ${t.toolbarBorder}`,
                      color: t.axisText, fontSize: 10, fontFamily: 'monospace',
                      padding: '2px 8px', borderRadius: 3, cursor: 'pointer',
                    }}
                  >↻</button>
                )}
                {svc.name === 'gpu' && svc.status === 'error' && (
                  <button
                    onClick={handleRetryGPU}
                    style={{
                      background: '#ef535022', border: `1px solid #ef535066`,
                      color: '#ef5350', fontSize: 10, fontFamily: 'monospace',
                      padding: '2px 8px', borderRadius: 3, cursor: 'pointer',
                    }}
                  >Retry</button>
                )}
              </div>
            ))}

            {/* Footer */}
            <div style={{
              padding: '6px 14px 0', fontSize: 9, color: t.axisText, opacity: 0.35,
              letterSpacing: 0.3,
            }}>
              ibserver:5000 &middot; ococo:30300 &middot; yfinance:8777
            </div>
          </div>
        )
      })()}
    </>
  )
}
