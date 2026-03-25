/**
 * OCOCO API client — connects to the signal engine via REST + WebSocket.
 * Implements DrawingRepository interface for annotation CRUD.
 * Also provides WebSocket for real-time signal reception.
 */

import type { Drawing, DrawingGroup, Point } from '../types'
import type { DrawingRepository } from './DrawingRepository'

const LS_GROUPS_KEY = 'apex-groups'

// Map between frontend Drawing type and OCOCO Annotation format
function drawingToAnnotation(d: Drawing): any {
  return {
    id: d.id,
    symbol: d.symbol,
    source: 'user',
    type: d.type,
    points: d.points,
    style: {
      color: d.color,
      opacity: d.opacity,
      lineStyle: d.lineStyle,
      thickness: d.thickness,
    },
    timeframe: d.timeframe,
    visibility: [d.timeframe],
    metadata: { groupId: d.groupId ?? 'default' },
  }
}

function annotationToDrawing(a: any): Drawing {
  return {
    id: a.id,
    symbol: a.symbol,
    type: a.type ?? 'trendline',
    points: a.points ?? [],
    color: a.style?.color ?? '#4a9eff',
    opacity: a.style?.opacity ?? 1,
    lineStyle: a.style?.lineStyle ?? 'solid',
    thickness: a.style?.thickness ?? 1.5,
    timeframe: a.timeframe ?? '5m',
    groupId: a.metadata?.groupId ?? 'default',
  }
}

export class OcocoClient implements DrawingRepository {
  private baseUrl: string
  private ws: WebSocket | null = null
  private wsUrl: string
  private signalListeners = new Map<string, Set<(annotation: any) => void>>()
  private alertListeners = new Set<(alert: any) => void>()
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null
  private subscribedSymbols = new Set<string>()

  constructor(apiUrl: string) {
    this.baseUrl = apiUrl.replace(/\/$/, '')
    this.wsUrl = this.baseUrl.replace(/^http/, 'ws') + '/ws'
  }

  // --- DrawingRepository interface ---

  async loadAll(): Promise<Drawing[]> {
    const ctrl = new AbortController()
    const t = setTimeout(() => ctrl.abort(), 1500)
    try {
      const res = await fetch(`${this.baseUrl}/api/annotations?source=user`, { signal: ctrl.signal })
      clearTimeout(t)
      if (!res.ok) throw new Error(`Load failed: ${res.status}`)
      const annotations = await res.json()
      return annotations.map(annotationToDrawing)
    } finally {
      clearTimeout(t)
    }
  }

  async loadForSymbol(symbol: string): Promise<Drawing[]> {
    const res = await fetch(`${this.baseUrl}/api/annotations?symbol=${encodeURIComponent(symbol)}&source=user`)
    if (!res.ok) throw new Error(`Load failed: ${res.status}`)
    const annotations = await res.json()
    return annotations.map(annotationToDrawing)
  }

  async save(drawing: Drawing): Promise<void> {
    await fetch(`${this.baseUrl}/api/annotations`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(drawingToAnnotation(drawing)),
    })
  }

  async updatePoints(id: string, points: Point[]): Promise<void> {
    await fetch(`${this.baseUrl}/api/annotations/${id}/points`, {
      method: 'PATCH',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ points }),
    })
  }

  async updateStyle(id: string, style: Partial<Pick<Drawing, 'color' | 'opacity' | 'lineStyle' | 'thickness'>>): Promise<void> {
    await fetch(`${this.baseUrl}/api/annotations/${id}/style`, {
      method: 'PATCH',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(style),
    })
  }

  async remove(id: string): Promise<void> {
    await fetch(`${this.baseUrl}/api/annotations/${id}`, { method: 'DELETE' })
  }

  async clear(): Promise<void> {
    await fetch(`${this.baseUrl}/api/annotations?source=user`, { method: 'DELETE' })
  }

  // --- Group operations (localStorage, OCOCO has no /groups endpoint yet) ---

  async loadAllGroups(): Promise<DrawingGroup[]> {
    return this._lsLoadGroups()
  }

  async saveGroup(group: DrawingGroup): Promise<void> {
    const groups = this._lsLoadGroups()
    const idx = groups.findIndex(g => g.id === group.id)
    if (idx >= 0) groups[idx] = group
    else groups.push(group)
    this._lsSaveGroups(groups)
  }

  async removeGroup(id: string): Promise<void> {
    this._lsSaveGroups(this._lsLoadGroups().filter(g => g.id !== id))
  }

  async updateGroupStyle(id: string, style: Partial<Pick<DrawingGroup, 'color' | 'opacity' | 'lineStyle' | 'thickness'>>): Promise<void> {
    const groups = this._lsLoadGroups()
    const g = groups.find(x => x.id === id)
    if (g) Object.assign(g, style)
    this._lsSaveGroups(groups)
  }

  async applyGroupStyle(groupId: string, style: Pick<Drawing, 'color' | 'opacity' | 'lineStyle' | 'thickness'>): Promise<void> {
    // Update group style in localStorage
    await this.updateGroupStyle(groupId, style)
    // OCOCO has no batch endpoint; fire individual drawing updates without awaiting
    // (fire-and-forget — these are low priority since in-memory state is already correct)
    this.loadAll().then(all => {
      all.filter(a => (a.groupId ?? 'default') === groupId)
        .forEach(a => this.updateStyle(a.id, style).catch(() => {}))
    }).catch(() => {})
  }

  private _lsLoadGroups(): DrawingGroup[] {
    try { return JSON.parse(localStorage.getItem(LS_GROUPS_KEY) ?? '[]') }
    catch { return [] }
  }

  private _lsSaveGroups(groups: DrawingGroup[]): void {
    try { localStorage.setItem(LS_GROUPS_KEY, JSON.stringify(groups)) }
    catch { /* quota */ }
  }

  // --- WebSocket for real-time signals ---

  connectWs(): void {
    if (this.ws) return
    this.ws = new WebSocket(this.wsUrl)

    this.ws.onopen = () => {
      console.info('OCOCO WS connected')
      // Re-subscribe to all tracked symbols
      if (this.subscribedSymbols.size > 0) {
        this.ws!.send(JSON.stringify({ type: 'subscribe', symbols: Array.from(this.subscribedSymbols) }))
      }
    }

    this.ws.onmessage = (event) => {
      try {
        const msg = JSON.parse(event.data as string)
        switch (msg.type) {
          case 'signal':
          case 'snapshot': {
            const symbol = msg.symbol ?? msg.annotation?.symbol
            const listeners = this.signalListeners.get(symbol)
            if (listeners) listeners.forEach(cb => cb(msg))
            break
          }
          case 'alert': {
            this.alertListeners.forEach(cb => cb(msg))
            break
          }
        }
      } catch { /* ignore parse errors */ }
    }

    this.ws.onclose = () => {
      this.ws = null
      // Auto-reconnect after 3 seconds
      this.reconnectTimer = setTimeout(() => this.connectWs(), 3000)
    }

    this.ws.onerror = () => {
      this.ws?.close()
    }
  }

  disconnectWs(): void {
    if (this.reconnectTimer) clearTimeout(this.reconnectTimer)
    this.ws?.close()
    this.ws = null
  }

  subscribeSymbol(symbol: string): void {
    this.subscribedSymbols.add(symbol)
    if (this.ws?.readyState === WebSocket.OPEN) {
      this.ws.send(JSON.stringify({ type: 'subscribe', symbols: [symbol] }))
    }
  }

  unsubscribeSymbol(symbol: string): void {
    this.subscribedSymbols.delete(symbol)
    if (this.ws?.readyState === WebSocket.OPEN) {
      this.ws.send(JSON.stringify({ type: 'unsubscribe', symbols: [symbol] }))
    }
  }

  /** Send a price update for hit detection */
  sendPrice(symbol: string, price: number, time: number): void {
    if (this.ws?.readyState === WebSocket.OPEN) {
      this.ws.send(JSON.stringify({ type: 'price', symbol, price, time }))
    }
  }

  onSignal(symbol: string, cb: (msg: any) => void): () => void {
    if (!this.signalListeners.has(symbol)) this.signalListeners.set(symbol, new Set())
    this.signalListeners.get(symbol)!.add(cb)
    return () => { this.signalListeners.get(symbol)?.delete(cb) }
  }

  onAlert(cb: (alert: any) => void): () => void {
    this.alertListeners.add(cb)
    return () => { this.alertListeners.delete(cb) }
  }
}
