import { create } from 'zustand'
import type { Timeframe } from '../types'

export interface PaneConfig {
  id: string
  symbol: string
  timeframe: Timeframe
  showVolume: boolean
  visibleIndicators: string[]
}

export type Layout = '1' | '2' | '2h' | '3' | '4' | '6' | '6h' | '9'

interface ChartStore {
  panes: PaneConfig[]
  activePane: string
  autoScrollVersion: number
  layout: Layout
  theme: string
  setActivePane: (id: string) => void
  setSymbol: (id: string, symbol: string) => void
  setTimeframe: (id: string, tf: Timeframe) => void
  resetAutoScroll: () => void
  toggleVolume: (paneId: string) => void
  toggleIndicator: (paneId: string, indicatorId: string) => void
  setLayout: (layout: Layout) => void
  setTheme: (theme: string) => void
}

const DEFAULT_SYMBOLS = ['AAPL', 'MSFT', 'NVDA', 'TSLA', 'SPY', 'QQQ', 'AMZN', 'GOOG', 'META']
const DEFAULT_INDICATORS = ['sma20', 'ema50', 'bollinger']

export const useChartStore = create<ChartStore>(set => ({
  panes: DEFAULT_SYMBOLS.map((symbol, i) => ({
    id: `pane-${i}`,
    symbol,
    timeframe: '5m' as Timeframe,
    showVolume: true,
    visibleIndicators: [...DEFAULT_INDICATORS],
  })),
  activePane: 'pane-0',
  autoScrollVersion: 0,
  layout: '9' as Layout,
  theme: 'midnight',
  setActivePane: (id) => set({ activePane: id }),
  setSymbol: (id, symbol) =>
    set(s => ({ panes: s.panes.map(p => p.id === id ? { ...p, symbol } : p) })),
  setTimeframe: (id, timeframe) =>
    set(s => ({ panes: s.panes.map(p => p.id === id ? { ...p, timeframe } : p) })),
  resetAutoScroll: () => set(s => ({ autoScrollVersion: s.autoScrollVersion + 1 })),
  toggleVolume: (paneId) =>
    set(s => ({ panes: s.panes.map(p => p.id === paneId ? { ...p, showVolume: !p.showVolume } : p) })),
  toggleIndicator: (paneId, indicatorId) =>
    set(s => ({
      panes: s.panes.map(p => {
        if (p.id !== paneId) return p
        const vis = p.visibleIndicators.includes(indicatorId)
          ? p.visibleIndicators.filter(id => id !== indicatorId)
          : [...p.visibleIndicators, indicatorId]
        return { ...p, visibleIndicators: vis }
      }),
    })),
  setLayout: (layout) => set({ layout }),
  setTheme: (theme) => set({ theme }),
}))
