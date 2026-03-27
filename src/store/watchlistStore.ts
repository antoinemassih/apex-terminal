import { create } from 'zustand'
import { persist } from 'zustand/middleware'
import type { SavedOption } from '../data/optionsSim'

export interface WatchPrice {
  price: number
  prevClose: number | null
}

export type WatchlistMode = 'stocks' | 'chain' | 'saved'

interface WatchlistStore {
  // Stocks watchlist
  symbols: string[]
  open: boolean
  prices: Record<string, WatchPrice>
  toggleOpen: () => void
  addSymbol: (s: string) => void
  removeSymbol: (s: string) => void
  setPrice: (symbol: string, price: number) => void
  setPrevClose: (symbol: string, prevClose: number) => void

  // Options chain
  mode: WatchlistMode
  setMode: (m: WatchlistMode) => void
  chainSymbol: string
  setChainSymbol: (s: string) => void
  numStrikes: number
  setNumStrikes: (n: number) => void
  farDTE: number
  setFarDTE: (n: number) => void

  // Saved options watchlist
  savedOptions: SavedOption[]
  toggleSavedOption: (opt: SavedOption) => void
  removeSavedOption: (key: string) => void
}

const DEFAULT_SYMBOLS = [
  'SPY', 'QQQ', 'IWM', 'DIA',
  'AAPL', 'MSFT', 'NVDA', 'TSLA', 'AMZN', 'META', 'GOOGL',
  'GLD',
]

export const useWatchlistStore = create<WatchlistStore>()(
  persist(
    (set) => ({
      symbols: DEFAULT_SYMBOLS,
      open: true,
      prices: {},
      toggleOpen: () => set(s => ({ open: !s.open })),
      addSymbol: (sym) => set(s => ({
        symbols: s.symbols.includes(sym.toUpperCase()) ? s.symbols : [...s.symbols, sym.toUpperCase()],
      })),
      removeSymbol: (sym) => set(s => ({ symbols: s.symbols.filter(x => x !== sym) })),
      setPrice: (symbol, price) => set(s => ({
        prices: { ...s.prices, [symbol]: { ...s.prices[symbol], price } },
      })),
      setPrevClose: (symbol, prevClose) => set(s => ({
        prices: { ...s.prices, [symbol]: { ...s.prices[symbol], prevClose } },
      })),

      mode: 'stocks',
      setMode: (mode) => set({ mode }),
      chainSymbol: 'SPY',
      setChainSymbol: (chainSymbol) => set({ chainSymbol }),
      numStrikes: 5,
      setNumStrikes: (numStrikes) => set({ numStrikes: Math.max(1, Math.min(25, numStrikes)) }),
      farDTE: 1,
      setFarDTE: (farDTE) => set({ farDTE }),

      savedOptions: [],
      toggleSavedOption: (opt) => set(s => {
        const exists = s.savedOptions.some(o => o.key === opt.key)
        return {
          savedOptions: exists
            ? s.savedOptions.filter(o => o.key !== opt.key)
            : [...s.savedOptions, opt],
        }
      }),
      removeSavedOption: (key) => set(s => ({
        savedOptions: s.savedOptions.filter(o => o.key !== key),
      })),
    }),
    {
      name: 'apex-watchlist',
      partialize: (s) => ({
        symbols: s.symbols,
        open: s.open,
        mode: s.mode,
        chainSymbol: s.chainSymbol,
        numStrikes: s.numStrikes,
        farDTE: s.farDTE,
        savedOptions: s.savedOptions,
      }),
    }
  )
)
