import { create } from 'zustand'
import type { Drawing, DrawingTool, Point, Timeframe } from '../types'
import type { DrawingRepository } from '../data/DrawingRepository'

let _repo: DrawingRepository | null = null

/** Call once at bootstrap to wire up the persistence backend */
export function initDrawingStore(repo: DrawingRepository): Promise<void> {
  _repo = repo
  // Load all drawings from the repository into the store
  return repo.loadAll().then(drawings => {
    useDrawingStore.setState({ drawings })
  })
}

interface DrawingStore {
  drawings: Drawing[]
  activeTool: DrawingTool
  lastDrawTool: DrawingTool
  selectedId: string | null
  setActiveTool: (tool: DrawingTool) => void
  toggleDrawTool: () => void
  addDrawing: (d: Drawing) => void
  updateDrawing: (id: string, points: Point[]) => void
  updateDrawingStyle: (id: string, style: Partial<Pick<Drawing, 'color' | 'opacity' | 'lineStyle' | 'thickness'>>) => void
  removeDrawing: (id: string) => void
  selectDrawing: (id: string | null) => void
  drawingsFor: (symbol: string, tf: Timeframe) => Drawing[]
  clear: () => void
}

export const useDrawingStore = create<DrawingStore>()((set, get) => ({
  drawings: [],
  activeTool: 'cursor',
  lastDrawTool: 'trendline',
  selectedId: null,

  setActiveTool: tool => {
    if (tool !== 'cursor') set({ activeTool: tool, lastDrawTool: tool, selectedId: null })
    else set({ activeTool: 'cursor' })
  },

  toggleDrawTool: () => {
    const { activeTool } = get()
    const cycle: DrawingTool[] = ['cursor', 'trendline', 'hline', 'hzone', 'barmarker']
    const idx = cycle.indexOf(activeTool)
    const next = cycle[(idx + 1) % cycle.length]
    if (next !== 'cursor') set({ activeTool: next, lastDrawTool: next, selectedId: null })
    else set({ activeTool: 'cursor', selectedId: null })
  },

  addDrawing: d => {
    const drawing = {
      ...d,
      opacity: d.opacity ?? 1,
      lineStyle: d.lineStyle ?? 'solid',
      thickness: d.thickness ?? 1.5,
    }
    set(s => ({ drawings: [...s.drawings, drawing] }))
    _repo?.save(drawing).catch(e => console.warn('Failed to persist drawing:', e))
  },

  updateDrawing: (id, points) => {
    set(s => ({ drawings: s.drawings.map(d => d.id === id ? { ...d, points } : d) }))
    _repo?.updatePoints(id, points).catch(e => console.warn('Failed to persist drawing update:', e))
  },

  updateDrawingStyle: (id, style) => {
    set(s => ({ drawings: s.drawings.map(d => d.id === id ? { ...d, ...style } : d) }))
    _repo?.updateStyle(id, style).catch(e => console.warn('Failed to persist style update:', e))
  },

  removeDrawing: id => {
    set(s => ({
      drawings: s.drawings.filter(d => d.id !== id),
      selectedId: s.selectedId === id ? null : s.selectedId,
    }))
    _repo?.remove(id).catch(e => console.warn('Failed to persist drawing removal:', e))
  },

  selectDrawing: id => set({ selectedId: id }),

  drawingsFor: (symbol, _tf) => get().drawings.filter(d => d.symbol === symbol),

  clear: () => {
    set({ drawings: [], selectedId: null })
    _repo?.clear().catch(e => console.warn('Failed to clear drawings:', e))
  },
}))
