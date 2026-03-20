import { create } from 'zustand'
import { persist } from 'zustand/middleware'
import type { Drawing, DrawingTool, Timeframe } from '../types'

interface DrawingStore {
  drawings: Drawing[]
  activeTool: DrawingTool
  setActiveTool: (tool: DrawingTool) => void
  addDrawing: (d: Drawing) => void
  removeDrawing: (id: string) => void
  drawingsFor: (symbol: string, tf: Timeframe) => Drawing[]
  clear: () => void
}

export const useDrawingStore = create<DrawingStore>()(
  persist(
    (set, get) => ({
      drawings: [],
      activeTool: 'cursor',
      setActiveTool: tool => set({ activeTool: tool }),
      addDrawing: d => set(s => ({ drawings: [...s.drawings, d] })),
      removeDrawing: id => set(s => ({ drawings: s.drawings.filter(d => d.id !== id) })),
      drawingsFor: (symbol, tf) => get().drawings.filter(d => d.symbol === symbol && d.timeframe === tf),
      clear: () => set({ drawings: [] }),
    }),
    { name: 'apex-drawings' }
  )
)
