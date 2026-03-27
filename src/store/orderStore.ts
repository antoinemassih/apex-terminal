import { create } from 'zustand'

export type OrderType = 'market' | 'limit'
export type LevelType = 'buy' | 'sell' | 'stop' | 'oco_target' | 'oco_stop' | 'trigger_buy' | 'trigger_sell'
export type OrderStatus = 'draft' | 'placed' | 'executed' | 'cancelled'

// Paired level types — cancelling/placing one always affects the other
export const LEVEL_PAIR: Partial<Record<LevelType, LevelType>> = {
  oco_target:  'oco_stop',
  oco_stop:    'oco_target',
  trigger_buy:  'trigger_sell',
  trigger_sell: 'trigger_buy',
}

export function isPaired(type: LevelType): boolean {
  return type in LEVEL_PAIR
}

export function isOCO(type: LevelType): boolean {
  return type === 'oco_target' || type === 'oco_stop'
}

export function isTrigger(type: LevelType): boolean {
  return type === 'trigger_buy' || type === 'trigger_sell'
}

export interface OrderLevel {
  type: LevelType
  price: number
  qty: number
  status: OrderStatus
  triggered?: boolean   // trigger_buy only: buy leg has been filled
  executedAt?: number   // timestamp when executed
  cancelledAt?: number  // timestamp when cancelled
}

export interface PaneOrderState {
  qty: number
  limitPrice: number | null
  orderType: OrderType
  levels: OrderLevel[]
}

// Order toast — displayed on chart and in order book
export interface OrderToast {
  id: string
  paneId: string
  type: LevelType
  action: 'executed' | 'cancelled'
  price: number
  qty: number
  timestamp: number
}

export type OrderFilter = 'all' | 'active' | 'executed' | 'cancelled'

interface OrderStore {
  enabled: boolean
  toggleEnabled: () => void

  ordersOpen: boolean
  toggleOrdersOpen: () => void

  filter: OrderFilter
  setFilter: (f: OrderFilter) => void

  // Toasts
  toasts: OrderToast[]
  addToast: (toast: Omit<OrderToast, 'id' | 'timestamp'>) => void
  removeToast: (id: string) => void

  panes: Record<string, PaneOrderState>
  getPane: (paneId: string) => PaneOrderState
  setQty: (paneId: string, qty: number) => void
  setLimitPrice: (paneId: string, price: number | null) => void
  setOrderType: (paneId: string, type: OrderType) => void
  setLevel: (paneId: string, level: Omit<OrderLevel, 'qty' | 'status' | 'triggered'> & { qty?: number; status?: OrderStatus; triggered?: boolean }) => void
  // place/clear always affect paired legs atomically
  placeLevel: (paneId: string, type: LevelType) => void
  // Execute a level (filled)
  executeLevel: (paneId: string, type: LevelType) => void
  // Cancel a level — sets status to cancelled (keeps in history)
  cancelLevel: (paneId: string, type: LevelType) => void
  // Remove a level entirely from the list
  removeLevel: (paneId: string, type: LevelType) => void
  // trigger_buy has been filled — activates trigger_sell
  triggerBuy: (paneId: string) => void
  clearAllLevels: () => void
  placeAllDrafts: () => void
  // Remove all executed/cancelled entries
  clearHistory: () => void
}

const DEFAULT_PANE: PaneOrderState = { qty: 100, limitPrice: null, orderType: 'market', levels: [] }

let _toastId = 0

export const useOrderStore = create<OrderStore>((set, get) => ({
  enabled:    false,
  toggleEnabled: () => set(s => ({ enabled: !s.enabled })),
  ordersOpen: false,
  toggleOrdersOpen: () => set(s => ({ ordersOpen: !s.ordersOpen })),

  filter: 'all',
  setFilter: (filter) => set({ filter }),

  toasts: [],
  addToast: (toast) => set(s => ({
    toasts: [...s.toasts, { ...toast, id: `toast-${++_toastId}`, timestamp: Date.now() }],
  })),
  removeToast: (id) => set(s => ({ toasts: s.toasts.filter(t => t.id !== id) })),

  panes: {},
  getPane: (paneId) => get().panes[paneId] ?? DEFAULT_PANE,

  setQty: (paneId, qty) => set(s => ({
    panes: { ...s.panes, [paneId]: { ...DEFAULT_PANE, ...s.panes[paneId], qty: Math.max(1, qty) } },
  })),
  setLimitPrice: (paneId, price) => set(s => ({
    panes: { ...s.panes, [paneId]: { ...DEFAULT_PANE, ...s.panes[paneId], limitPrice: price } },
  })),
  setOrderType: (paneId, orderType) => set(s => ({
    panes: { ...s.panes, [paneId]: { ...DEFAULT_PANE, ...s.panes[paneId], orderType } },
  })),

  setLevel: (paneId, level) => set(s => {
    const prev = s.panes[paneId] ?? DEFAULT_PANE
    const existing = prev.levels.find(l => l.type === level.type)
    const full: OrderLevel = {
      type:      level.type,
      price:     level.price,
      qty:       level.qty       !== undefined ? level.qty       : (existing?.qty       ?? prev.qty),
      status:    level.status    !== undefined ? level.status    : (existing?.status    ?? 'draft'),
      triggered: level.triggered !== undefined ? level.triggered : existing?.triggered,
    }
    const levels = prev.levels.filter(l => l.type !== level.type).concat(full)
    return { panes: { ...s.panes, [paneId]: { ...prev, levels } } }
  }),

  placeLevel: (paneId, type) => set(s => {
    const prev = s.panes[paneId] ?? DEFAULT_PANE
    const pair = LEVEL_PAIR[type]
    const levels = prev.levels.map(l =>
      l.type === type || l.type === pair ? { ...l, status: 'placed' as OrderStatus } : l
    )
    return { panes: { ...s.panes, [paneId]: { ...prev, levels } } }
  }),

  executeLevel: (paneId, type) => set(s => {
    const prev = s.panes[paneId] ?? DEFAULT_PANE
    const pair = LEVEL_PAIR[type]
    const now = Date.now()
    const levels = prev.levels.map(l =>
      l.type === type || l.type === pair ? { ...l, status: 'executed' as OrderStatus, executedAt: now } : l
    )
    // Auto-add toast for executed level
    const level = prev.levels.find(l => l.type === type)
    const newToasts = level ? [...s.toasts, {
      id: `toast-${++_toastId}`, paneId, type, action: 'executed' as const,
      price: level.price, qty: level.qty, timestamp: now,
    }] : s.toasts
    return { panes: { ...s.panes, [paneId]: { ...prev, levels } }, toasts: newToasts }
  }),

  cancelLevel: (paneId, type) => set(s => {
    const prev = s.panes[paneId] ?? DEFAULT_PANE
    const pair = LEVEL_PAIR[type]
    const now = Date.now()
    const level = prev.levels.find(l => l.type === type)
    const levels = prev.levels.map(l =>
      l.type === type || l.type === pair ? { ...l, status: 'cancelled' as OrderStatus, cancelledAt: now } : l
    )
    const newToasts = level ? [...s.toasts, {
      id: `toast-${++_toastId}`, paneId, type, action: 'cancelled' as const,
      price: level.price, qty: level.qty, timestamp: now,
    }] : s.toasts
    return { panes: { ...s.panes, [paneId]: { ...prev, levels } }, toasts: newToasts }
  }),

  removeLevel: (paneId, type) => set(s => {
    const prev = s.panes[paneId] ?? DEFAULT_PANE
    const pair = LEVEL_PAIR[type]
    const levels = prev.levels.filter(l => l.type !== type && l.type !== pair)
    return { panes: { ...s.panes, [paneId]: { ...prev, levels } } }
  }),

  triggerBuy: (paneId) => set(s => {
    const prev = s.panes[paneId] ?? DEFAULT_PANE
    const levels = prev.levels.map(l =>
      l.type === 'trigger_buy' ? { ...l, triggered: true } : l
    )
    return { panes: { ...s.panes, [paneId]: { ...prev, levels } } }
  }),

  clearAllLevels: () => set(s => ({
    panes: Object.fromEntries(
      Object.entries(s.panes).map(([id, pane]) => [id, { ...pane, levels: [] }])
    ),
  })),

  placeAllDrafts: () => set(s => ({
    panes: Object.fromEntries(
      Object.entries(s.panes).map(([id, pane]) => [
        id, { ...pane, levels: pane.levels.map(l => l.status === 'draft' ? { ...l, status: 'placed' as OrderStatus } : l) },
      ])
    ),
  })),

  clearHistory: () => set(s => ({
    panes: Object.fromEntries(
      Object.entries(s.panes).map(([id, pane]) => [
        id, { ...pane, levels: pane.levels.filter(l => l.status !== 'executed' && l.status !== 'cancelled') },
      ])
    ),
  })),
}))
