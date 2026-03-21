import { create } from 'zustand'
import type { Timeframe } from '../types'

interface PaneConfig {
  id: string
  symbol: string
  timeframe: Timeframe
}

interface ChartStore {
  panes: PaneConfig[]
  activePane: string
  autoScrollVersion: number
  setActivePane: (id: string) => void
  setSymbol: (id: string, symbol: string) => void
  setTimeframe: (id: string, tf: Timeframe) => void
  resetAutoScroll: () => void
}

const DEFAULT_SYMBOLS = ['AAPL', 'MSFT', 'NVDA', 'TSLA', 'SPY', 'QQQ', 'AMZN']

export const useChartStore = create<ChartStore>(set => ({
  panes: DEFAULT_SYMBOLS.map((symbol, i) => ({
    id: `pane-${i}`,
    symbol,
    timeframe: '5m' as Timeframe,
  })),
  activePane: 'pane-0',
  autoScrollVersion: 0,
  setActivePane: (id) => set({ activePane: id }),
  setSymbol: (id, symbol) =>
    set(s => ({ panes: s.panes.map(p => p.id === id ? { ...p, symbol } : p) })),
  setTimeframe: (id, timeframe) =>
    set(s => ({ panes: s.panes.map(p => p.id === id ? { ...p, timeframe } : p) })),
  resetAutoScroll: () => set(s => ({ autoScrollVersion: s.autoScrollVersion + 1 })),
}))
