/**
 * Drawing persistence layer.
 *
 * Interface designed to be swappable:
 * - LocalDrawingRepository: IndexedDB (current, offline-first)
 * - ServerDrawingRepository: REST/WebSocket → PostgreSQL (future)
 *
 * All methods are async to support both local and remote implementations.
 * The zustand store stays in-memory for instant UI, and syncs to the
 * repository in the background.
 */

import type { Drawing, Point } from '../types'

// ---------------------------------------------------------------------------
// Interface
// ---------------------------------------------------------------------------

export interface DrawingRepository {
  /** Load all drawings (called once at startup) */
  loadAll(): Promise<Drawing[]>

  /** Load drawings for a specific symbol (any timeframe) */
  loadForSymbol(symbol: string): Promise<Drawing[]>

  /** Save or update a drawing */
  save(drawing: Drawing): Promise<void>

  /** Update just the points of a drawing (most frequent operation during drag) */
  updatePoints(id: string, points: Point[]): Promise<void>

  /** Update style properties */
  updateStyle(id: string, style: Partial<Pick<Drawing, 'color' | 'opacity' | 'lineStyle' | 'thickness'>>): Promise<void>

  /** Delete a drawing */
  remove(id: string): Promise<void>

  /** Delete all drawings */
  clear(): Promise<void>
}

// ---------------------------------------------------------------------------
// IndexedDB Implementation
// ---------------------------------------------------------------------------

const DB_NAME = 'apex-drawings'
const DB_VERSION = 1
const STORE_NAME = 'drawings'

function openDB(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const req = indexedDB.open(DB_NAME, DB_VERSION)
    req.onupgradeneeded = () => {
      const db = req.result
      if (!db.objectStoreNames.contains(STORE_NAME)) {
        const store = db.createObjectStore(STORE_NAME, { keyPath: 'id' })
        store.createIndex('symbol', 'symbol', { unique: false })
        store.createIndex('symbol_timeframe', ['symbol', 'timeframe'], { unique: false })
      }
    }
    req.onsuccess = () => resolve(req.result)
    req.onerror = () => reject(req.error)
  })
}

export class LocalDrawingRepository implements DrawingRepository {
  private db: IDBDatabase | null = null

  async init(): Promise<void> {
    try {
      this.db = await openDB()
    } catch (e) {
      console.warn('DrawingRepository: IndexedDB unavailable', e)
    }
  }

  async loadAll(): Promise<Drawing[]> {
    if (!this.db) return this.loadFromLocalStorage()
    return new Promise((resolve) => {
      const tx = this.db!.transaction(STORE_NAME, 'readonly')
      const req = tx.objectStore(STORE_NAME).getAll()
      req.onsuccess = () => resolve(req.result ?? [])
      req.onerror = () => resolve(this.loadFromLocalStorageSync())
    })
  }

  async loadForSymbol(symbol: string): Promise<Drawing[]> {
    if (!this.db) return (await this.loadAll()).filter(d => d.symbol === symbol)
    return new Promise((resolve) => {
      const tx = this.db!.transaction(STORE_NAME, 'readonly')
      const idx = tx.objectStore(STORE_NAME).index('symbol')
      const req = idx.getAll(symbol)
      req.onsuccess = () => resolve(req.result ?? [])
      req.onerror = () => resolve([])
    })
  }

  async save(drawing: Drawing): Promise<void> {
    if (!this.db) return
    return new Promise((resolve) => {
      const tx = this.db!.transaction(STORE_NAME, 'readwrite')
      tx.objectStore(STORE_NAME).put(drawing)
      tx.oncomplete = () => resolve()
      tx.onerror = () => resolve()
    })
  }

  async updatePoints(id: string, points: Point[]): Promise<void> {
    if (!this.db) return
    const drawing = await this.getById(id)
    if (drawing) {
      drawing.points = points
      await this.save(drawing)
    }
  }

  async updateStyle(id: string, style: Partial<Pick<Drawing, 'color' | 'opacity' | 'lineStyle' | 'thickness'>>): Promise<void> {
    if (!this.db) return
    const drawing = await this.getById(id)
    if (drawing) {
      Object.assign(drawing, style)
      await this.save(drawing)
    }
  }

  async remove(id: string): Promise<void> {
    if (!this.db) return
    return new Promise((resolve) => {
      const tx = this.db!.transaction(STORE_NAME, 'readwrite')
      tx.objectStore(STORE_NAME).delete(id)
      tx.oncomplete = () => resolve()
      tx.onerror = () => resolve()
    })
  }

  async clear(): Promise<void> {
    if (!this.db) return
    return new Promise((resolve) => {
      const tx = this.db!.transaction(STORE_NAME, 'readwrite')
      tx.objectStore(STORE_NAME).clear()
      tx.oncomplete = () => resolve()
      tx.onerror = () => resolve()
    })
  }

  /** Migrate existing localStorage drawings into IndexedDB (one-time) */
  async migrateFromLocalStorage(): Promise<void> {
    if (!this.db) return
    const existing = this.loadFromLocalStorageSync()
    if (existing.length === 0) return

    // Check if we already have data in IndexedDB
    const current = await this.loadAll()
    if (current.length > 0) return // already migrated

    for (const d of existing) {
      await this.save(d)
    }
    // Clean up localStorage
    try { localStorage.removeItem('apex-drawings') } catch { /* */ }
    console.info(`Migrated ${existing.length} drawings from localStorage to IndexedDB`)
  }

  private async getById(id: string): Promise<Drawing | null> {
    if (!this.db) return null
    return new Promise((resolve) => {
      const tx = this.db!.transaction(STORE_NAME, 'readonly')
      const req = tx.objectStore(STORE_NAME).get(id)
      req.onsuccess = () => resolve(req.result ?? null)
      req.onerror = () => resolve(null)
    })
  }

  private loadFromLocalStorageSync(): Drawing[] {
    try {
      const raw = localStorage.getItem('apex-drawings')
      if (!raw) return []
      const parsed = JSON.parse(raw)
      return parsed?.state?.drawings ?? []
    } catch { return [] }
  }

  private async loadFromLocalStorage(): Promise<Drawing[]> {
    return this.loadFromLocalStorageSync()
  }
}
