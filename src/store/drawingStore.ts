import { create } from 'zustand'
import { persist } from 'zustand/middleware'
import type { Drawing, DrawingTool, Point, Timeframe } from '../types'

interface DrawingStore {
  drawings: Drawing[]
  activeTool: DrawingTool
  lastDrawTool: DrawingTool  // remembers the last non-cursor tool for middle-click toggle
  selectedId: string | null
  setActiveTool: (tool: DrawingTool) => void
  toggleDrawTool: () => void  // middle-click: cursor ↔ lastDrawTool
  addDrawing: (d: Drawing) => void
  updateDrawing: (id: string, points: Point[]) => void
  updateDrawingStyle: (id: string, style: Partial<Pick<Drawing, 'color' | 'opacity' | 'lineStyle' | 'thickness'>>) => void
  removeDrawing: (id: string) => void
  selectDrawing: (id: string | null) => void
  drawingsFor: (symbol: string, tf: Timeframe) => Drawing[]
  clear: () => void
}

export const useDrawingStore = create<DrawingStore>()(
  persist(
    (set, get) => ({
      drawings: [],
      activeTool: 'cursor',
      lastDrawTool: 'trendline',
      selectedId: null,
      setActiveTool: tool => {
        if (tool !== 'cursor') set({ activeTool: tool, lastDrawTool: tool, selectedId: null })
        else set({ activeTool: 'cursor' })
      },
      toggleDrawTool: () => {
        const { activeTool, lastDrawTool } = get()
        if (activeTool === 'cursor') {
          set({ activeTool: lastDrawTool, selectedId: null })
        } else {
          set({ activeTool: 'cursor' })
        }
      },
      addDrawing: d => set(s => ({
        drawings: [...s.drawings, {
          ...d,
          opacity: d.opacity ?? 1,
          lineStyle: d.lineStyle ?? 'solid',
          thickness: d.thickness ?? 1.5,
        }],
      })),
      updateDrawing: (id, points) => set(s => ({
        drawings: s.drawings.map(d => d.id === id ? { ...d, points } : d),
      })),
      updateDrawingStyle: (id, style) => set(s => ({
        drawings: s.drawings.map(d => d.id === id ? { ...d, ...style } : d),
      })),
      removeDrawing: id => set(s => ({
        drawings: s.drawings.filter(d => d.id !== id),
        selectedId: s.selectedId === id ? null : s.selectedId,
      })),
      selectDrawing: id => set({ selectedId: id }),
      drawingsFor: (symbol, tf) => get().drawings.filter(d => d.symbol === symbol && d.timeframe === tf),
      clear: () => set({ drawings: [], selectedId: null }),
    }),
    { name: 'apex-drawings', partialize: s => ({ drawings: s.drawings, lastDrawTool: s.lastDrawTool }) }
  )
)
