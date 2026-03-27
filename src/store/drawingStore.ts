import { create } from 'zustand'
import type { Drawing, DrawingGroup, DrawingTool, Point, Timeframe } from '../types'
import type { DrawingRepository } from '../data/DrawingRepository'
import { v4 as uuid } from 'uuid'

let _repo: DrawingRepository | null = null

const DEFAULT_GROUP: DrawingGroup = { id: 'default', name: 'Temp' }

/** Call once at bootstrap to wire up the persistence backend */
export function initDrawingStore(repo: DrawingRepository): Promise<void> {
  _repo = repo
  return Promise.all([repo.loadAll(), repo.loadAllGroups()]).then(([drawings, groups]) => {
    // Ensure default group always exists and has the current name
    const hasDefault = groups.some(g => g.id === 'default')
    let finalGroups = hasDefault
      ? groups.map(g => g.id === 'default' && g.name !== DEFAULT_GROUP.name ? DEFAULT_GROUP : g)
      : [DEFAULT_GROUP, ...groups]
    if (!hasDefault) {
      repo.saveGroup(DEFAULT_GROUP).catch(e => console.warn('Failed to seed default group:', e))
    } else if (groups.find(g => g.id === 'default')?.name !== DEFAULT_GROUP.name) {
      repo.saveGroup(DEFAULT_GROUP).catch(e => console.warn('Failed to update default group name:', e))
    }
    useDrawingStore.setState({ drawings, groups: finalGroups })
  })
}

interface DrawingStore {
  drawings: Drawing[]
  groups: DrawingGroup[]
  activeTool: DrawingTool
  lastDrawTool: DrawingTool
  selectedId: string | null        // last-selected (drives style popup)
  selectedIds: string[]            // full multi-selection
  hiddenSymbols: string[]
  hiddenGroups: string[]

  setActiveTool: (tool: DrawingTool) => void
  toggleDrawTool: () => void
  addDrawing: (d: Drawing) => void
  updateDrawing: (id: string, points: Point[]) => void
  updateDrawingStyle: (id: string, style: Partial<Pick<Drawing, 'color' | 'opacity' | 'lineStyle' | 'thickness'>>) => void
  removeDrawing: (id: string) => void
  removeAllForSymbol: (symbol: string) => void
  removeAllInGroup: (groupId: string) => void
  toggleHideDrawings: (symbol: string) => void
  drawingsHidden: (symbol: string) => boolean
  toggleHideGroup: (groupId: string) => void
  groupHidden: (groupId: string) => boolean
  selectDrawing: (id: string | null) => void
  toggleSelectDrawing: (id: string, addToSelection: boolean) => void
  drawingsFor: (symbol: string, tf: Timeframe) => Drawing[]
  clear: () => void

  // Group operations
  createGroup: (name: string) => DrawingGroup
  renameGroup: (id: string, name: string) => void
  deleteGroup: (id: string) => void
  setDrawingGroup: (drawingId: string, groupId: string) => void
  applyStyleToGroup: (groupId: string, style: Pick<Drawing, 'color' | 'opacity' | 'lineStyle' | 'thickness'>) => void
}

export const useDrawingStore = create<DrawingStore>()((set, get) => ({
  drawings: [],
  groups: [DEFAULT_GROUP],
  activeTool: 'cursor',
  lastDrawTool: 'trendline',
  selectedId: null,
  selectedIds: [],
  hiddenSymbols: [],
  hiddenGroups: [],

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
      groupId: d.groupId ?? 'default',
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
      selectedIds: s.selectedIds.filter(x => x !== id),
    }))
    _repo?.remove(id).catch(e => console.warn('Failed to persist drawing removal:', e))
  },

  removeAllForSymbol: symbol => {
    const ids = get().drawings.filter(d => d.symbol === symbol).map(d => d.id)
    set(s => ({ drawings: s.drawings.filter(d => d.symbol !== symbol), selectedId: null }))
    ids.forEach(id => _repo?.remove(id).catch(e => console.warn('Failed to remove drawing:', e)))
  },

  removeAllInGroup: groupId => {
    const ids = get().drawings.filter(d => (d.groupId ?? 'default') === groupId).map(d => d.id)
    set(s => ({
      drawings: s.drawings.filter(d => (d.groupId ?? 'default') !== groupId),
      selectedId: ids.includes(s.selectedId ?? '') ? null : s.selectedId,
    }))
    ids.forEach(id => _repo?.remove(id).catch(e => console.warn('Failed to remove drawing:', e)))
  },

  toggleHideDrawings: symbol => {
    set(s => ({
      hiddenSymbols: s.hiddenSymbols.includes(symbol)
        ? s.hiddenSymbols.filter(sym => sym !== symbol)
        : [...s.hiddenSymbols, symbol],
    }))
  },

  drawingsHidden: symbol => get().hiddenSymbols.includes(symbol),

  toggleHideGroup: (groupId) => {
    set(s => ({
      hiddenGroups: s.hiddenGroups.includes(groupId)
        ? s.hiddenGroups.filter(id => id !== groupId)
        : [...s.hiddenGroups, groupId],
    }))
  },

  groupHidden: (groupId) => get().hiddenGroups.includes(groupId),

  selectDrawing: id => set({ selectedId: id, selectedIds: id ? [id] : [] }),

  toggleSelectDrawing: (id, addToSelection) => {
    if (!addToSelection) {
      set({ selectedId: id, selectedIds: [id] })
    } else {
      set(s => {
        const already = s.selectedIds.includes(id)
        const selectedIds = already
          ? s.selectedIds.filter(x => x !== id)
          : [...s.selectedIds, id]
        return { selectedIds, selectedId: selectedIds[selectedIds.length - 1] ?? null }
      })
    }
  },

  drawingsFor: (symbol, _tf) => get().drawings.filter(d => d.symbol === symbol),

  clear: () => {
    set({ drawings: [], selectedId: null })
    _repo?.clear().catch(e => console.warn('Failed to clear drawings:', e))
  },

  // --- Group operations ---

  createGroup: (name: string): DrawingGroup => {
    const group: DrawingGroup = { id: uuid(), name }
    set(s => ({ groups: [...s.groups, group] }))
    _repo?.saveGroup(group).catch(e => console.warn('Failed to persist group:', e))
    return group
  },

  renameGroup: (id: string, name: string) => {
    const existing = get().groups.find(g => g.id === id)
    if (!existing || id === 'default') return
    const updated: DrawingGroup = { ...existing, name }
    set(s => ({ groups: s.groups.map(g => g.id === id ? updated : g) }))
    _repo?.saveGroup(updated).catch(e => console.warn('Failed to persist group rename:', e))
  },

  deleteGroup: (id: string) => {
    if (id === 'default') return
    const affectedIds = get().drawings
      .filter(d => (d.groupId ?? 'default') === id)
      .map(d => d.id)

    set(s => ({
      groups: s.groups.filter(g => g.id !== id),
      drawings: s.drawings.map(d =>
        affectedIds.includes(d.id) ? { ...d, groupId: 'default' } : d
      ),
    }))

    // Persist moved drawings + remove group
    const currentDrawings = get().drawings
    affectedIds.forEach(did => {
      const d = currentDrawings.find(x => x.id === did)
      if (d) _repo?.save(d).catch(e => console.warn('Failed to persist drawing group reset:', e))
    })
    _repo?.removeGroup(id).catch(e => console.warn('Failed to remove group:', e))
  },

  setDrawingGroup: (drawingId: string, groupId: string) => {
    set(s => ({
      drawings: s.drawings.map(d => d.id === drawingId ? { ...d, groupId } : d),
    }))
    const updated = get().drawings.find(d => d.id === drawingId)
    if (updated) _repo?.save(updated).catch(e => console.warn('Failed to persist group assignment:', e))
  },

  applyStyleToGroup: (groupId: string, style: Pick<Drawing, 'color' | 'opacity' | 'lineStyle' | 'thickness'>) => {
    set(s => ({
      drawings: s.drawings.map(d =>
        (d.groupId ?? 'default') === groupId ? { ...d, ...style } : d
      ),
      groups: s.groups.map(g =>
        g.id === groupId ? { ...g, ...style } : g
      ),
    }))
    // Single batch call — avoids N concurrent IPC/fetch calls that freeze the UI
    _repo?.applyGroupStyle(groupId, style).catch(e => console.warn('Failed to persist group style batch:', e))
  },
}))
