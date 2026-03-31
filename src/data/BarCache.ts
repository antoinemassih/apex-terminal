import type { Bar } from '../types'

const DB_NAME = 'apex-bars'
const DB_VERSION = 2 // bumped for new metadata store
const STORE_NAME = 'bars'
const META_STORE = 'meta'
const MAX_ENTRIES = 50 // max cached symbol:timeframe pairs

function openDB(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const req = indexedDB.open(DB_NAME, DB_VERSION)
    req.onupgradeneeded = () => {
      const db = req.result
      if (!db.objectStoreNames.contains(STORE_NAME)) {
        db.createObjectStore(STORE_NAME)
      }
      if (!db.objectStoreNames.contains(META_STORE)) {
        db.createObjectStore(META_STORE)
      }
    }
    req.onsuccess = () => resolve(req.result)
    req.onerror = () => reject(req.error)
  })
}

export class BarCache {
  private db: IDBDatabase | null = null

  async init(): Promise<void> {
    try {
      this.db = await openDB()
      // Run eviction on startup
      this.evict().catch(() => {})
    } catch (e) {
      console.warn('BarCache: IndexedDB unavailable, caching disabled', e)
    }
  }

  async get(symbol: string, timeframe: string): Promise<Bar[] | null> {
    if (!this.db) return null
    const key = `${symbol}:${timeframe}`
    // Update access time
    this.touch(key).catch(() => {})
    return new Promise((resolve) => {
      try {
        const tx = this.db!.transaction(STORE_NAME, 'readonly')
        const req = tx.objectStore(STORE_NAME).get(key)
        req.onsuccess = () => resolve(req.result ?? null)
        req.onerror = () => resolve(null)
      } catch {
        resolve(null)
      }
    })
  }

  async set(symbol: string, timeframe: string, bars: Bar[]): Promise<void> {
    if (!this.db) return
    const key = `${symbol}:${timeframe}`
    return new Promise((resolve) => {
      try {
        const tx = this.db!.transaction([STORE_NAME, META_STORE], 'readwrite')
        tx.objectStore(STORE_NAME).put(bars, key)
        tx.objectStore(META_STORE).put(Date.now(), key)
        tx.oncomplete = () => { resolve(); this.evict().catch(() => {}) }
        tx.onerror = () => resolve()
      } catch {
        resolve()
      }
    })
  }

  private async touch(key: string): Promise<void> {
    if (!this.db) return
    return new Promise((resolve) => {
      try {
        const tx = this.db!.transaction(META_STORE, 'readwrite')
        tx.objectStore(META_STORE).put(Date.now(), key)
        tx.oncomplete = () => resolve()
        tx.onerror = () => resolve()
      } catch { resolve() }
    })
  }

  /** Remove oldest entries when cache exceeds MAX_ENTRIES */
  private async evict(): Promise<void> {
    if (!this.db) return
    try {
      // Get all metadata entries
      const entries: [string, number][] = await new Promise((resolve) => {
        const tx = this.db!.transaction(META_STORE, 'readonly')
        const req = tx.objectStore(META_STORE).openCursor()
        const result: [string, number][] = []
        req.onsuccess = () => {
          const cursor = req.result
          if (cursor) { result.push([cursor.key as string, cursor.value as number]); cursor.continue() }
          else resolve(result)
        }
        req.onerror = () => resolve(result)
      })

      if (entries.length <= MAX_ENTRIES) return

      // Sort by access time, oldest first
      entries.sort((a, b) => a[1] - b[1])
      const toRemove = entries.slice(0, entries.length - MAX_ENTRIES)

      const tx = this.db!.transaction([STORE_NAME, META_STORE], 'readwrite')
      for (const [key] of toRemove) {
        tx.objectStore(STORE_NAME).delete(key)
        tx.objectStore(META_STORE).delete(key)
      }
      if (toRemove.length > 0) {
        console.info(`[BarCache] Evicted ${toRemove.length} old entries`)
      }
    } catch { /* eviction is best-effort */ }
  }
}
